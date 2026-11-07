//! Eclipse launcher: installs Eclipse builds, prepares them from the player's own Minecraft,
//! and starts the client or a dedicated server.

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod accounts_switch;
mod accounts_ui;
mod agents_page;
mod art;
mod cli;
mod cli_tools;
mod clipboard;
mod collection;
mod console;
mod console_page;
mod cull;
mod datapacks_page;
mod dates;
mod dev_options;
mod export_page;
mod fields;
mod frame;
mod game_run;
mod game_tools;
mod github_mods_page;
mod home;
mod home_head;
mod home_lists;
mod home_mods;
mod home_multiplayer_state;
mod home_packs;
mod home_packs_state;
mod home_screenshots;
mod home_screenshots_state;
mod home_tabs;
mod home_worlds_state;
mod icons;
mod import_page;
mod instance_icons;
mod instance_mods;
mod instance_page;
mod instance_state;
mod instance_versions;
mod internet;
mod launch_page;
mod layout;
mod line_edit;
mod location_page;
mod markdown;
mod mcp;
mod memory;
mod menus;
mod messages;
mod microsoft;
mod modpacks_page;
mod mods_page;
mod mods_project;
mod motion;
mod native_build;
mod native_log_page;
mod notes_page;
mod owner_page;
mod palette;
mod perf;
mod picture_cache;
mod pictures;
mod profile_heads;
mod scene;
mod scrollbar;
mod servers_page;
mod settings_account;
mod settings_appearance;
mod settings_language;
mod settings_page;
#[cfg(windows)]
mod setup_page;
mod shortcut_page;
mod shot_blur;
mod skin_page;
mod sticky;
mod tab_strip;
mod theme;
mod titlebar;
mod toast;
mod ui;
mod versions_page;
mod versions_state;
mod while_playing;

use eclipse_launcher::account::{self, Accounts, SignIn, SignInState};
use eclipse_launcher::backend::{self, Job, Msg, Release, Running, Settings, Updates};
use eclipse_launcher::build_import;
use eclipse_launcher::i18n::{self, tr, trf};
use eclipse_launcher::instances;
use eclipse_launcher::native_mods;
use eclipse_launcher::native_status;
use eclipse_launcher::telemetry;
use fields::Field;
use game_run::PlayState;
use gpui::{
    actions, div, prelude::*, px, size, AnimationExt, AnyElement, App, Bounds, ClickEvent, Context,
    Div, ElementId, FocusHandle, FontWeight, Hsla, KeyBinding, KeyDownEvent, Menu, MenuItem,
    MouseDownEvent, PathPromptOptions, Pixels, Point, SharedString, Stateful, Subscription, Window,
    WindowBounds, WindowOptions,
};
use scrollbar::Scrollbar;
use std::borrow::Cow;
use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use theme::Theme;
use ui::{ButtonKind, Press};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Page {
    Home,
    Versions,
    /// Mods, resource packs and modpacks (imported ones and the browser's).
    Mods,
    Servers,
    Notes,
    Settings,
    InstanceEditor,
}

impl Page {
    /// `ECLIPSE_LAUNCHER_PAGE=mods` (or `modpacks`, `resourcepacks`, `shaders`, `servers`, `versions`, `notes`,
    /// `settings`) opens on that page, for screenshots; Home otherwise (`getting-ready`: Home
    /// preparing, `home::getting_ready_preview`; `sign-in`: the owner gate's sheet with a
    /// made-up code, `owner_page::sign_in_preview`; `new-instance`: the New instance page on
    /// Modded; `edit-instance`, `duplicate-instance`: the selected one's Edit or Duplicate page; `worlds`, `packs`...: Home on that tab, `HomeTab::from_env`).
    fn from_env() -> Page {
        let page = match std::env::var("ECLIPSE_LAUNCHER_PAGE").as_deref() {
            Ok("mods" | "modpacks" | "resourcepacks" | "shaders" | "datapacks") => Page::Mods,
            Ok("servers") => Page::Servers,
            Ok("versions") => Page::Versions,
            Ok("notes") => Page::Notes,
            Ok("settings") => Page::Settings,
            _ => Page::Home,
        };
        // The data folder did not open: start on Settings, where the message and the two
        // recoveries are (an explicit ECLIPSE_LAUNCHER_PAGE still wins, for screenshots).
        page_after_startup(
            page,
            eclipse_launcher::data_location::startup_error().is_some(),
        )
    }

    const NAV: [Page; 6] = [
        Page::Home,
        Page::Mods,
        Page::Servers,
        Page::Versions,
        Page::Notes,
        Page::Settings,
    ];

    fn label(self) -> &'static str {
        match self {
            Page::Home => tr("Home"),
            Page::Mods => tr("Mods"),
            Page::Servers => tr("Servers"),
            Page::Versions => tr("Versions"),
            Page::Notes => tr("Patch notes"),
            Page::Settings => tr("Settings"),
            Page::InstanceEditor => tr("New instance"),
        }
    }

    fn title(self) -> &'static str {
        match self {
            Page::Versions => tr("Eclipse versions"),
            page => page.label(),
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Page::Home => crate::icons::HOME,
            Page::Mods => "puzzle",
            Page::Servers => crate::icons::SERVER,
            Page::Versions => crate::icons::VERSION,
            Page::Notes => crate::icons::NOTES,
            Page::Settings => crate::icons::SETTINGS,
            Page::InstanceEditor => "plus-sign",
        }
    }
}

/// The launcher's own update, as the footer and Settings show it.
#[derive(Clone, PartialEq)]
enum SelfUpdate {
    Idle,
    Downloading(String, f32),
    /// Downloaded and checked: "A new version is available" and Update, which hands it to the
    /// installer (`update_handoff`). Closing the window hands it over too.
    Ready(String),
    /// A newer launcher can't go in here: this copy can't replace itself
    /// (`backend::cannot_replace_itself`), or the installer said why (the error). The player
    /// follows the recovery advice for its install location.
    Stuck(Option<String>),
    Failed(String),
}

/// The last game the launcher started ended badly: what it left behind.
#[derive(Clone)]
struct Crash {
    /// The crash report the run wrote, if it wrote one.
    report: Option<std::path::PathBuf>,
    /// One line on what went wrong: the report's error, else how the process ended.
    summary: String,
    home: std::path::PathBuf,
}

#[derive(Clone, Debug, PartialEq)]
enum QuickPlay {
    World(String),
    Server(String),
}

enum Load<T> {
    Loading,
    Ready(T),
    Failed(String),
}

const LOG_LINES: usize = 2000;

struct Launcher {
    theme: Theme,
    page: Page,
    /// How many times the page changed, for the page transition.
    switches: usize,
    content_scroll: Scrollbar,
    /// The Settings page's own scroll, its jumps and the accent picker (`settings_page`).
    settings_ui: settings_page::SettingsUi,
    /// The Mods page's project card, kept in view beside its list.
    mods_sticky: sticky::Sticky,
    /// The rows of the page's long list (Versions, Patch notes) last drawn.
    long_list: cull::Cull,
    updates: Updates,
    update_check: versions_state::UpdateCheck,
    self_update: SelfUpdate,
    self_update_hidden: bool,
    versions: versions_state::Versions,
    native: native_build::NativeBuild,
    /// The Mod build log panel, opened from the sidebar's foot (`native_log_page.rs`).
    native_log: native_log_page::NativeLog,
    open_menu: Option<menus::Menu>,
    asking: Option<menus::Asking>,
    /// Where the sidebar's rows were drawn, for the ⋯ menu beside them (`frame.rs`).
    row_menu: frame::RowMenu,
    menu_opener: Option<FocusHandle>,
    /// Where the mouse went down to close a menu by pressing outside it, so that the
    /// same press, ending on the menu's own button, doesn't open it again.
    menu_closed_at: Option<Point<Pixels>>,
    /// Settings > Account's skin panel for an offline name (`skin_page.rs`).
    skin: Option<skin_page::SkinPanel>,
    /// Work the look out again on the next frame (a launcher theme put on or taken off).
    restyle: bool,
    /// Why the last picture for Home's background was refused.
    background_error: Option<String>,
    /// The data folder's startup problem or the last move's result.
    location_error: Option<String>,
    location_note: Option<String>,
    settings: Settings,
    settings_seen: serde_json::Value,
    /// The last failed settings write, kept across page changes until a save succeeds.
    settings_save_error: Option<String>,
    /// The Discord, sponsor and website links: built in until links.json on main has been read.
    links: backend::Links,
    game: game_run::GameRun,
    fields: fields::Fields,
    /// The Servers page (`servers_page.rs`).
    servers: servers_page::Servers,
    /// Its Friends over the internet panel (`internet.rs`).
    internet: internet::Internet,
    /// The palette (⌘K), while open (`palette`).
    palette: Option<palette::Palette>,
    /// The toast last drawn (`toast`), by its number.
    toast_seen: Option<u64>,
    /// The window's own focus, so Tab reaches the controls when nothing else has the keyboard.
    root_focus: FocusHandle,
    frame_focus: FocusHandle,
    side_focus: FocusHandle,
    content_focus: FocusHandle,
    play_focus: FocusHandle,
    focus_play: bool,
    focus_page: bool,
    ring_pending: Option<FocusHandle>,
    /// Microsoft accounts (accounts.json), the sign-in under way and what it last showed.
    accounts: Accounts,
    accounts_ui: accounts_ui::AccountsUi,
    profile_heads: profile_heads::Heads,
    /// Whether an account here owns the game, by the keychain's last answers (the owner gate);
    /// none until the keychain has been read.
    owner: Option<bool>,
    /// Play was refused (or Home's Sign in pressed): the owner gate's screen is open.
    owner_screen_open: bool,
    /// Clicks on the sidebar's launcher version, and whether they revealed the developer options
    /// (`dev_options`) for this run.
    dev_clicks: u8,
    dev_options: bool,
    /// The window's width at the last frame: the sidebar and Home's head lay out by it.
    window_width: f32,
    /// The window's longer side in points: the size pictures are made at (`picture_cache`).
    window_long_side: f32,
    joining: Option<String>,
    /// Where the next start goes at once: `--world <folder>` or `--server <address>`.
    quick_play: Option<QuickPlay>,
    /// Opt-in data collection: the player's answer and Settings' section (`collection`).
    collection: collection::Collection,
    /// Settings' "Connect an AI agent" (`agents_page`).
    agents: agents_page::AgentsUi,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    _appearance: Subscription,
    /// Redraws when the window gains or loses focus: the sky moves only in front.
    _activation: Subscription,
    /// The session's measurements (`perf.rs`), and when the steps they time began.
    perf: perf::Perf,
    /// The download under way as first seen (when, files done), for the time left.
    download_seen: Option<(Instant, usize)>,
    /// The tab open under Home's head (`home_tabs.rs`).
    home_tab: home_tabs::HomeTab,
    /// The selected instance's mods, for Home's Mods tab (`instance_mods.rs`).
    instance_mods: instance_mods::InstanceMods,
    /// Modrinth projects' icon URLs (`instance_mods.rs`).
    project_icons: instance_mods::ProjectIcons,
    /// `eclipse-launcher benchmark` started this window to measure it.
    bench: Option<perf::Bench>,
    modpacks: modpacks_page::Modpacks,
    /// The Mods page: the browser's search, installs and updates (`mods_page.rs`).
    mods: mods_page::Mods,
    github: github_mods_page::Github,
    /// The instances, for the sidebar and Play, and their dialog (`instance_state.rs`).
    instances: instance_state::Instances,
    /// Home's Worlds section, of the selected instance (`home_worlds_state.rs`).
    worlds: home_worlds_state::Worlds,
    multiplayer: home_multiplayer_state::Multiplayer,
    screenshots: home_screenshots_state::Screenshots,
    shot_blur: shot_blur::ShotBlur,
    /// Pictures from disk made at their drawn size (`picture_cache`).
    pictures: picture_cache::Pictures,
    game_packs: home_packs_state::Packs,
    world_packs: datapacks_page::WorldPacks,
    export: Option<export_page::ExportPanel>,
    launch: Option<launch_page::LaunchPanel>,
    /// The memory row on Edit instance.
    memory_slider: launch_page::MemorySlider,
    importing: import_page::Importing,
    console: console_page::Console,
    /// What the window did when the game started (`while_playing.rs`).
    aside: while_playing::Aside,
    /// The selected instance's desktop shortcut (`shortcut_page.rs`).
    shortcuts: shortcut_page::Shortcuts,
    /// The name the game made up for an offline player (`eclipse-player.txt`), for the sidebar's
    /// account chip.
    offline_name: Option<String>,
    /// The eclipse night behind the launcher: its own view, so a frame of it alone redraws
    /// nothing else.
    scene: gpui::Entity<scene::Scene>,
    /// The window's scale, as of its last frame, for art drawn outside `render`'s reach.
    scale: f32,
}

impl Launcher {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (tx, rx) = channel();
        let appearance = cx.observe_window_appearance(window, |this, _, cx| {
            this.restyle();
            cx.notify();
        });
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(80))
                .await;
            if this.update(cx, |this, cx| this.drain(cx)).is_err() {
                break;
            }
        })
        .detach();
        let mut settings = Settings::load();
        let settings_seen = serde_json::to_value(&settings).unwrap_or_default();
        i18n::set(settings.language());
        motion::set_reduce_motion(settings.reduce_motion);
        if settings.minecraft_dir.is_none() {
            settings.minecraft_dir = backend::default_minecraft_dir();
        }
        // Release builds on Linux put their entry back in the applications menu when there is
        // none or it starts a program that is gone (a moved or deleted binary), never over one
        // that works; `ECLIPSE_LAUNCHER_DESKTOP_ENTRY=1` does it in a development build.
        if cfg!(target_os = "linux")
            && settings.desktop_entry()
            && (!cfg!(debug_assertions)
                || std::env::var_os("ECLIPSE_LAUNCHER_DESKTOP_ENTRY").is_some())
        {
            std::thread::spawn(|| {
                let _ = eclipse_launcher::desktop::repair(backend::exe_path());
            });
        }
        let installed = backend::installed();
        if settings
            .default_version
            .as_ref()
            .is_none_or(|s| !installed.contains(s))
        {
            settings.default_version = installed.first().cloned();
        }
        // Pin the window's chrome (macOS title bar) to the choice before the theme reads
        // `window.appearance()`, so both draw the same look.
        titlebar::apply_appearance(window, settings.appearance);
        let page = Page::from_env();
        let mut this = Self {
            theme: Theme::for_appearance(settings.appearance, settings.accent, window.appearance()),
            page,
            switches: 0,
            content_scroll: Scrollbar::default(),
            settings_ui: Default::default(),
            mods_sticky: sticky::Sticky::default(),
            long_list: cull::Cull::default(),
            updates: Updates::default(),
            update_check: versions_state::UpdateCheck::Never,
            // The installer the last Update started could not put the new launcher in place.
            self_update: eclipse_launcher::update_handoff::take_failure()
                .map_or(SelfUpdate::Idle, |e| SelfUpdate::Stuck(Some(e))),
            self_update_hidden: false,
            versions: versions_state::Versions::new(installed),
            native: Default::default(),
            native_log: Default::default(),
            open_menu: None,
            asking: None,
            row_menu: Default::default(),
            menu_opener: None,
            menu_closed_at: None,
            skin: None,
            // The first draw takes the launcher theme from Settings.
            restyle: true,
            background_error: None,
            location_error: location_page::startup_error(),
            location_note: location_page::startup_note(),
            settings,
            settings_seen,
            settings_save_error: None,
            links: backend::Links::default(),
            game: Default::default(),
            fields: Default::default(),
            servers: servers_page::Servers::new(),
            internet: internet::Internet::new(),
            root_focus: cx.focus_handle(),
            frame_focus: cx.focus_handle(),
            side_focus: cx.focus_handle().tab_index(0).tab_stop(false),
            content_focus: cx.focus_handle().tab_index(0).tab_stop(false),
            play_focus: cx.focus_handle().tab_index(0).tab_stop(true),
            focus_play: true,
            focus_page: page != Page::Home,
            ring_pending: None,
            palette: None,
            toast_seen: None,
            accounts: Accounts::load(),
            accounts_ui: Default::default(),
            profile_heads: profile_heads::Heads::default(),
            owner: None,
            owner_screen_open: false,
            dev_clicks: 0,
            dev_options: false,
            window_width: 0.,
            window_long_side: 0.,
            joining: None,
            quick_play: None,
            collection: collection::Collection::load(),
            agents: agents_page::AgentsUi::default(),
            tx,
            rx,
            _appearance: appearance,
            _activation: cx.observe_window_activation(window, |_, _, cx| cx.notify()),
            perf: perf::Perf::new("session"),
            download_seen: None,
            home_tab: home_tabs::HomeTab::from_env(),
            instance_mods: Default::default(),
            project_icons: Default::default(),
            bench: perf::Bench::from_env(),
            modpacks: Default::default(),
            mods: mods_page::Mods::new(),
            github: github_mods_page::Github::new(),
            instances: instance_state::Instances::default(),
            worlds: home_worlds_state::Worlds::default(),
            multiplayer: home_multiplayer_state::Multiplayer::default(),
            screenshots: home_screenshots_state::Screenshots::default(),
            shot_blur: Default::default(),
            pictures: Default::default(),
            game_packs: home_packs_state::Packs::default(),
            world_packs: datapacks_page::WorldPacks::default(),
            export: None,
            launch: None,
            memory_slider: Default::default(),
            importing: Default::default(),
            console: Default::default(),
            aside: while_playing::Aside::new(window.window_handle()),
            shortcuts: Default::default(),
            offline_name: offline_name(),
            scene: cx.new(|_| scene::Scene::new()),
            scale: window.scale_factor(),
        };
        if this.location_error.is_some() || this.location_note.is_some() {
            this.page = Page::Settings;
        }
        let (args, proxy) = (
            this.settings.game_args.clone(),
            this.settings.proxy.clone().unwrap_or_default(),
        );
        this.fields.text(Field::GameArgs).set(args);
        this.fields.text(Field::Proxy).set(proxy);
        this.reload_instances();
        if this.bench.is_some() {
            this.perf.keep_local_log(false);
        }
        let tx = this.tx.clone();
        let keychain_tx = this.tx.clone();
        // Asking the keychain can block (D-Bus), so not on this thread.
        let accounts = this.accounts.clone();
        std::thread::spawn(move || {
            let store = account::store();
            let _ = keychain_tx.send(Msg::Keychain(account::no_keychain()));
            let _ = keychain_tx.send(Msg::Owner(accounts.has_owner(store)));
        });
        std::thread::spawn(move || {
            let reused = backend::reuse_prepared_everywhere();
            if !reused.is_empty() {
                let _ = tx.send(Msg::Reused(reused));
            }
        });
        this.load_profile_heads();
        this.load_auth_statuses(true);
        this.fetch_startup_releases();
        this.mods_auto_update();
        this.fetch_links();
        if home::getting_ready_preview() {
            let (selected, tag) = (
                this.selected_instance().cloned(),
                this.selected_version().cloned(),
            );
            this.game.preparing_now(selected.as_ref(), tag);
        }
        // `ECLIPSE_LAUNCHER_PAGE=new-instance`: the New instance page on Modded, for screenshots
        // (`new-instance-eclipse`, `new-instance-vanilla`: on those).
        let kind = match std::env::var("ECLIPSE_LAUNCHER_PAGE").as_deref() {
            Ok("new-instance") => Some(instance_state::NewKind::Modded),
            Ok("new-instance-eclipse") => Some(instance_state::NewKind::Eclipse),
            Ok("new-instance-vanilla") => Some(instance_state::NewKind::Vanilla),
            _ => None,
        };
        if let Some(kind) = kind {
            this.open_instance_page(None);
            this.pick_kind(kind, cx);
        }
        match std::env::var("ECLIPSE_LAUNCHER_PAGE").as_deref() {
            Ok("edit-instance") => this.open_instance_page(this.selected_instance().cloned()),
            Ok("duplicate-instance") => this.start_copy(),
            _ => {}
        }
        // What the game and the launcher kept, if the player allows it and there is a server.
        telemetry::send_in_background();
        this
    }

    fn restyle(&mut self) {
        self.restyle = true;
    }

    /// Keeps the current choices in memory if writing fails; every page offers the same retry.
    fn save_settings(&mut self) -> bool {
        let now = serde_json::to_value(&self.settings).unwrap_or_default();
        let seen = self.settings_seen.clone();
        let saved = Settings::update(|disk| {
            let merged = changes_over(
                &seen,
                &now,
                serde_json::to_value(&*disk).unwrap_or_default(),
            );
            if let Ok(settings) = serde_json::from_value::<Settings>(merged.clone()) {
                *disk = settings;
            }
            merged
        });
        match saved {
            Ok(merged) => {
                if let Ok(settings) = serde_json::from_value::<Settings>(merged.clone()) {
                    self.settings = settings;
                }
                self.settings_seen = merged;
                self.settings_save_error = None;
            }
            Err(e) => self.settings_save_error = Some(e),
        }
        self.settings_save_error.is_none()
    }

    fn settings_save_notice(&self, cx: &mut Context<Self>) -> Option<Div> {
        let error = self.settings_save_error.as_ref()?;
        let retry = ui::button(
            "settings-save-retry",
            Some(crate::icons::RETRY),
            tr("Try again"),
            ButtonKind::Secondary,
            ui::ButtonSize::Md,
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.save_settings();
            cx.notify();
        }));
        Some(
            div()
                .flex_none()
                .px(px(layout::gutter(self.window_width)))
                .py(px(eclipse_theme::glass::space::MD))
                .child(ui::card().p(px(ui::PANEL_INSET)).child(ui::error_line_for(
                    "settings-save",
                    tr("Couldn't save settings"),
                    error,
                    Some(retry),
                    Vec::new(),
                ))),
        )
    }

    fn go(&mut self, page: Page) {
        // The game may have made up a name since, and the CLI or another window may have
        // changed the accounts: read them again (small files, once a switch).
        self.offline_name = offline_name();
        self.accounts = Accounts::load();
        // Each check reads the keychain (a prompt on macOS): page switches check every account
        // at most once a minute, and an account added meanwhile at once.
        if self
            .accounts_ui
            .auth_checked
            .is_none_or(|t| t.elapsed() >= Duration::from_secs(60))
            || (self.accounts.accounts.iter())
                .any(|a| !self.accounts_ui.auth_status.contains_key(&a.uuid))
        {
            self.load_auth_statuses(false);
        }
        self.load_profile_heads();
        if page != Page::InstanceEditor && self.instances.draft.as_ref().is_some_and(|d| !d.busy) {
            self.instances.draft = None;
        }
        if page != self.page {
            self.page = page;
            self.focus_play = page == Page::Home;
            self.focus_page = page != Page::Home;
            self.switches += 1;
            self.content_scroll = Scrollbar::default();
            self.long_list = cull::Cull::default();
            self.mods_sticky.reset();
            if page == Page::Mods {
                self.mods.reset_lists();
            }
        }
        self.open_menu = None;
    }

    fn check_updates(&mut self) {
        self.update_check = versions_state::UpdateCheck::Checking(Instant::now());
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Msg::Updates(backend::check_updates().map(Box::new)));
        });
    }

    /// Recovery text shared by both sidebar styles and Settings.
    fn launcher_update_help(&self, error: Option<&str>) -> String {
        if let Some(error) = error {
            return trf("the update failed: {error}", &[("error", error)]);
        }
        if backend::package_managed_launcher() {
            return tr("Update this launcher with your package manager.").to_string();
        }
        let folder = dirs::download_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| std::path::PathBuf::from(tr("Downloads")));
        trf(
            "Close this launcher, move it to {folder}, and open it there to enable updates.",
            &[("folder", &folder.display().to_string())],
        )
    }

    fn launcher_download_available(&self) -> bool {
        !backend::package_managed_launcher()
    }

    fn update_launcher(&mut self, release: Release) {
        if !backend::self_update_allowed()
            || matches!(
                self.self_update,
                SelfUpdate::Downloading(..) | SelfUpdate::Ready(_) | SelfUpdate::Stuck(_)
            )
        {
            return;
        }
        if backend::cannot_replace_itself() {
            self.self_update = SelfUpdate::Stuck(None);
            return;
        }
        let version = backend::launcher_version(&release.tag)
            .unwrap_or_default()
            .to_string();
        self.self_update = SelfUpdate::Downloading(version, 0.);
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let progress = |p| {
                let _ = tx.send(Msg::LauncherProgress(p));
            };
            let _ = tx.send(Msg::LauncherUpdated(backend::stage_update(
                &release, progress,
            )));
        });
    }

    /// Update: the window closes, the installer puts the new launcher in place and starts it.
    /// When another launcher already put it there, this one only restarts.
    fn apply_launcher_update(&mut self, cx: &mut App) {
        match eclipse_launcher::update_handoff::start_now() {
            Ok(true) => {
                servers_page::keep_running_through_update();
                shut_down(cx)
            }
            Ok(false) => self.self_update = SelfUpdate::Failed(backend::restart()),
            Err(e) => self.self_update = SelfUpdate::Failed(e),
        }
    }

    /// Installs the Eclipse version the last check found, keeping the player's choice of
    /// version unless they were on the newest one.
    fn install_update(&mut self) {
        let Some(release) = self.updates.eclipse.clone() else {
            return;
        };
        if self.versions.installing.is_some() || self.versions.removing.contains(&release.tag) {
            return;
        }
        let downgrade = backend::nightly_downgrade(&release, &self.versions.installed);
        if let Some(warning) = &downgrade {
            crate::toast::say(warning.clone());
        }
        let select = downgrade.is_none()
            && backend::select_after_update(
                self.settings.default_version.as_deref(),
                &self.versions.installed,
            );
        self.versions.update_install = Some((release.tag.clone(), select));
        self.install(&release);
    }

    /// The native mods turned on in the instance Play starts, unpacked and checked, with their
    /// bundles: what the warning names and the build compiles.
    fn native_mods_to_build(
        &self,
    ) -> Result<(native_mods::Unpacked, Vec<native_mods::Bundle>), String> {
        let folder = native_mods::installed_mods(&instances::selected().home)?;
        let found = native_mods::bundles(folder.path())?;
        Ok((folder, found))
    }

    /// What installed native build `version` was made from (`native_status::Build`), read once.
    fn native_build_of(&self, version: &str) -> Option<native_status::Build> {
        if !native_mods::is_native(version) {
            return None;
        }
        self.native
            .builds
            .borrow_mut()
            .entry(version.to_owned())
            .or_insert_with(|| native_status::Build::read(version))
            .clone()
    }

    /// Builds the selected instance's native mods, once the player accepted the warning. The
    /// build stays on the Eclipse the instance plays (`native_mods::base_for`), unless the
    /// warning was Update's: then it moves to that newer Eclipse.
    /// `anyway`: the player agreed to build mods made for other Eclipse versions together.
    fn build_native(&mut self, anyway: bool) {
        if self.native.step.is_some() {
            return;
        }
        self.native.confirm = None;
        self.native.error = None;
        self.native.mismatch = None;
        // This attempt's own log, starting empty: an open panel shows it rather than the last
        // attempt's (`native_log_page`).
        self.native_log_reset();
        let instance = instances::selected();
        self.native.for_instance = Some(instance.id.clone());
        let plays = self.version_of(&instance).cloned();
        let on_base = std::mem::take(&mut self.native.on_base);
        let base = match self.native.update.take() {
            Some(newer) => Some(newer),
            None => native_mods::base_for(plays.as_deref(), &self.versions.installed, false),
        };
        let Some(base) = base else {
            let error = tr("Install Eclipse first.").to_string();
            self.native_log_preflight(&error);
            self.native.error = Some(error);
            return;
        };
        let folder = match self.native_mods_to_build() {
            Ok((folder, _)) => folder,
            Err(e) => {
                self.native_log_preflight(&e);
                self.native.error = Some(e);
                return;
            }
        };
        self.native.step = Some(tr("Starting the build").into());
        self.native.started = Some((Instant::now(), 0));
        self.native.done = None;
        let profile = native_mods::Profile::chosen(&self.settings);
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let result = native_mods::build(
                &base,
                folder.path(),
                &instance,
                anyway,
                on_base,
                profile,
                |step| {
                    let _ = tx.send(Msg::NativeProgress(step.label()));
                },
            );
            let _ = tx.send(Msg::Native(result));
        });
    }

    fn fetch_releases(&mut self) {
        self.versions.releases = Load::Loading;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Msg::Releases(backend::releases()));
        });
    }

    fn fetch_startup_releases(&mut self) {
        self.update_check = versions_state::UpdateCheck::Checking(Instant::now());
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let updates = backend::releases_and_updates(|releases| {
                let _ = tx.send(Msg::Releases(releases));
            });
            let _ = tx.send(Msg::Updates(updates.map(Box::new)));
        });
    }

    fn fetch_links(&self) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Msg::Links(backend::links()));
        });
    }

    /// After a crash with no report: when the game's output ends while its Vulkan renderer
    /// started, a native crash in the graphics driver (#746), switches the game to
    /// `backend::vulkan_crash_fallback` (DirectX 12 on Windows, OpenGL on Linux; only where the
    /// player left the graphics API on Auto). The API's label when it did.
    fn switched_from_vulkan(&self, run_home: &std::path::Path) -> Option<&'static str> {
        let (api, label) = backend::vulkan_crash_fallback(std::env::consts::OS)?;
        if !backend::vulkan_start_crashed(&self.game.log) {
            return None;
        }
        // The GPU is the same for every profile: the game home's setting and the crashed run's.
        let home = backend::game_home();
        let mut homes = vec![home.clone()];
        homes.extend((run_home != home).then(|| run_home.to_path_buf()));
        let mut switched = false;
        for home in homes {
            match backend::switch_from_auto(&home, api) {
                Ok(changed) => switched |= changed,
                Err(e) => eprintln!("Eclipse launcher: could not switch to {label}: {e}"),
            }
        }
        switched.then_some(label)
    }

    /// The version Play starts: the selected instance's own, else the launcher's.
    fn selected_version(&self) -> Option<&String> {
        match self.selected_instance().and_then(|i| self.plays(i)) {
            Some(plays) => plays.installed_tag(),
            None => self.default_version(),
        }
    }

    /// The launcher's version: what instances without one of their own play.
    fn default_version(&self) -> Option<&String> {
        backend::followed_version(
            self.settings.default_version.as_ref(),
            &self.versions.installed,
        )
    }

    fn use_version(&mut self, tag: String) {
        let eclipse = self.selected_instance().is_some_and(|i| i.java.is_none());
        if eclipse {
            self.pin_version((Some(&tag) != self.default_version()).then_some(tag));
        } else {
            self.settings.default_version = Some(tag);
            self.save_settings();
        }
    }

    fn reinstall(&mut self, tag: &str) {
        let Some(release) = self.versions.release(tag).cloned() else {
            return;
        };
        let playing = self
            .instances
            .list
            .iter()
            .filter(|i| self.version_of(i).map(String::as_str) == Some(tag))
            .any(|i| self.play_state(i) != PlayState::Idle);
        if playing || self.versions.installing.is_some() {
            return;
        }
        self.game.prepare_error = None;
        self.install(&release);
    }

    /// Installs `release` for an instance made or edited to play it, after the install running
    /// now if there is one.
    fn install_for_instance(&mut self, release: Release) {
        match &self.versions.installing {
            Some(now) if now.tag == release.tag => {}
            Some(_) => self.versions.install_next = Some(release),
            None => {
                self.versions.instance_install = Some(release.tag.clone());
                self.install(&release);
            }
        }
    }

    fn install(&mut self, release: &Release) {
        if release.asset.is_none() || self.versions.removing.contains(&release.tag) {
            return;
        }
        let (release, tx) = (release.clone(), self.tx.clone());
        let tag = release.tag.clone();
        self.versions.installing = Some(versions_state::Install {
            tag: tag.clone(),
            bar: (0., 0.),
        });
        self.versions.install_started = Some(Instant::now());
        self.versions.install_error = None;
        std::thread::spawn(move || {
            let progress = |p| {
                let _ = tx.send(Msg::Progress(tag.clone(), p));
            };
            let result = backend::install(&release, progress);
            let _ = tx.send(Msg::Installed(tag, result));
        });
    }

    /// Deletes an installed version on a background thread: a version is thousands of files and
    /// the window must keep drawing meanwhile. `Msg::Removed` ends it.
    fn remove(&mut self, tag: &str) {
        self.stop_asking(|a| matches!(a, menus::Asking::Version(_)));
        let installing = self.versions.installing.as_ref().map(|i| i.tag.as_str());
        if !removable(
            &self.versions.installed,
            installing,
            &self.versions.removing,
            tag,
        ) {
            return;
        }
        self.versions.install_error = None;
        self.versions.removing.push(tag.to_string());
        self.versions.installed.retain(|t| t != tag);
        self.settings.default_version = selection_after_removal(
            &self.versions.installed,
            self.settings.default_version.take(),
        );
        let (tag, tx) = (tag.to_string(), self.tx.clone());
        std::thread::spawn(move || {
            let result = backend::remove_version(&tag);
            let _ = tx.send(Msg::Removed(tag, result));
        });
    }

    /// Asks for a game program (or a zip holding one) and installs it as its own version on a
    /// thread: a version is thousands of files. `Msg::Imported` ends it.
    fn import_build(&mut self, cx: &mut Context<Self>) {
        if !self.can_import_build() {
            return;
        }
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr("Import").into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(mut paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.pop() else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                // Another picker's import, or a removal, may have started meanwhile.
                this.start_build_import(path, cx);
            });
        })
        .detach();
    }
    /// Whether a build may be imported now. A removal still running could take
    /// the folder of the same program imported again.
    fn can_import_build(&self) -> bool {
        !self.versions.importing && self.versions.removing.is_empty()
    }
    /// Starts importing the build at `path` on a thread, as the picker does.
    /// Separate from [`Self::import_build`] so a file dropped on the window can
    /// start the same import (`import_dropped`) without a picker. Does nothing
    /// when one is already running. `Msg::Imported` ends it.
    fn start_build_import(&mut self, path: std::path::PathBuf, cx: &mut Context<Self>) {
        if !self.can_import_build() {
            return;
        }
        self.versions.importing = true;
        self.versions.import_error = None;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Msg::Imported(build_import::import(&path, None)));
        });
        cx.notify();
    }

    /// Prepares the selected version, its Minecraft files from the player's install or, for an
    /// owner who asks, from Mojang.
    fn prepare(&mut self, from: backend::MinecraftFrom, then_play: bool) {
        let Some(tag) = self.selected_version().cloned() else {
            return;
        };
        if self.game.busy() {
            return;
        }
        let minecraft = self
            .settings
            .minecraft_dir
            .clone()
            .unwrap_or_else(|| eclipse_prepare::artifacts::minecraft_dir(None));
        let selected = self.selected_instance().cloned();
        self.game
            .preparing_now(selected.as_ref(), then_play.then(|| tag.clone()));
        self.game.prepare_error = None;
        self.game
            .log
            .push_back("Checking Java and your Minecraft install".into());
        let tx = self.tx.clone();
        // The checks may download the server jar, so they run on the thread too.
        std::thread::spawn(move || {
            let log = |line| {
                let _ = tx.send(Msg::Line(Job::Prepare, line));
            };
            let result = backend::preparer(&tag, &minecraft, from, &log)
                .inspect(|(_, minecraft)| backend::sounds(&tag, minecraft, &log))
                .map(|(p, _)| {
                    backend::set_prepare_step(backend::PrepareStep::Prepare);
                    p
                })
                .and_then(|p| match p {
                    backend::Preparer::Process(cmd) => {
                        backend::spawn(Job::Prepare, cmd, tx.clone()).map(|_running| false)
                    }
                    backend::Preparer::InProcess(options) => {
                        backend::prepare_in_process(&options, &log).map(|_| true)
                    }
                });
            match result {
                // A process sends its own Exit when it ends.
                Ok(false) => {}
                Ok(true) => {
                    let _ = tx.send(Msg::Exit(Job::Prepare, Ok(())));
                }
                Err(e) => {
                    let _ = tx.send(Msg::Exit(Job::Prepare, Err(e)));
                }
            }
        });
        self.download_seen = None;
    }

    fn play(&mut self) {
        let java = self
            .selected_instance()
            .filter(|i| i.java.is_some())
            .cloned();
        if let Some(instance) = java {
            // Mojang's game: no Eclipse version or preparation, only no game already running.
            // It joins a server; a friend's code and opening a world are Eclipse's.
            self.joining = None;
            let server = match self.quick_play.take() {
                Some(QuickPlay::Server(address)) => Some(address),
                _ => None,
            };
            if self.game.busy() {
                return;
            }
            self.game.pack = None;
            self.start_java(instance, server);
            return;
        }
        let Some(tag) = self.selected_version().cloned() else {
            return;
        };
        if !self.can_start(&tag) {
            return;
        }
        // Native mods the start's automatic update moved on: the game they are built into is
        // built again first (the Mods page's Build), then plays.
        let rebuild = self
            .selected_instance()
            .map(|i| i.id.clone())
            .filter(|id| self.mods.rebuild.contains(id));
        if let Some(id) = rebuild.filter(|_| self.native.step.is_none()) {
            self.mods.rebuild.retain(|r| *r != id);
            if native_mods::is_native(&tag) {
                self.mods.play_after_build = true;
                self.build_native(false);
                return;
            }
        }
        self.game.pack = None;
        self.start_client(tag);
    }

    /// Starts a Java instance's Minecraft, joining `server` if given: its files and Java first
    /// (downloaded the first time, their lines in the log), then `drain` starts the game when
    /// the command arrives.
    fn start_java(&mut self, instance: instances::Instance, server: Option<String>) {
        self.game.starting_now(&instance, None);
        let play = backend::Play {
            server,
            ..backend::Play::of(instance)
        };
        self.launch_in_background(play, None);
    }

    fn launch_in_background(&mut self, play: backend::Play, auto: Option<mods_page::AutoTarget>) {
        let (tx, starts, mods_tx) = (
            self.tx.clone(),
            self.game.starts.tx.clone(),
            self.mods.tx.clone(),
        );
        std::thread::spawn(move || {
            if let Some(auto) = auto {
                let (lines, _) = mods_page::auto_update(&[auto]);
                if !lines.is_empty() {
                    let _ = mods_tx.send(mods_page::Event::AutoUpdated(lines, Vec::new()));
                }
            }
            let log = |line: String| {
                let _ = tx.send(Msg::Line(Job::Client, line));
            };
            let eclipse = play.instance.java.is_none();
            let result = backend::launch(play, &log);
            let _ = starts.send(game_run::Started { eclipse, result });
        });
    }

    /// Whether `tag` can start now ([`starts_now`]): prepared, no preparation under way, no game
    /// running or starting.
    pub(crate) fn can_start(&self, tag: &str) -> bool {
        starts_now(
            backend::is_prepared(tag),
            self.game.preparing(),
            self.game.active(),
        )
    }

    /// Whether the selected instance's version can start now, from a shortcut (a server's Join,
    /// a world's Play, a friend's code): Eclipse's only, never a Java instance.
    pub(crate) fn can_start_selected(&self) -> bool {
        self.selected_instance().is_none_or(|i| i.java.is_none())
            && self
                .selected_version()
                .is_some_and(|tag| self.can_start(tag))
    }

    /// Whether a server's Join starts the selected instance now: a Java instance too, as Play
    /// would (no game running or starting).
    pub(crate) fn can_join_selected(&self) -> bool {
        match self.selected_instance() {
            Some(i) if i.java.is_some() => !self.game.busy(),
            _ => self.can_start_selected(),
        }
    }

    /// Starts the game: the session first (a network round trip when the token needs
    /// renewing), then `drain` starts the process when it arrives. A pack's Play plays the
    /// instance made for that pack; Play the selected one.
    fn start_client(&mut self, tag: String) {
        let pack = self.game.pack.take();
        let instance = match &pack {
            Some(work) => self
                .instances
                .list
                .iter()
                .find(|i| i.work == *work)
                .cloned(),
            None => self.selected_instance().cloned(),
        };
        let Some(instance) = instance else {
            return;
        };
        self.game.starting_now(&instance, Some(Instant::now()));
        // Registry mods update before the game reads them (`eclipse::auto`); native mods only
        // at start, where a build can follow.
        let auto =
            (self.settings.auto_updates_mods() && pack.is_none()).then(|| mods_page::AutoTarget {
                instance: instance.clone(),
                level: Some(backend::mod_api_level(&tag)),
                natives: false,
            });
        let (world, server) = match self.quick_play.take() {
            Some(QuickPlay::World(world)) => (Some(world), None),
            Some(QuickPlay::Server(server)) => (None, Some(server)),
            None => (None, None),
        };
        let play = backend::Play {
            version: Some(tag),
            offline_without_owner: self.settings.dev_offline,
            join: self.joining.take(),
            world,
            server,
            ..backend::Play::of(instance)
        };
        self.launch_in_background(play, auto);
    }

    fn load_profile_heads(&mut self) {
        let resources = self
            .selected_version()
            .or_else(|| self.versions.installed.first())
            .map(|tag| backend::resources(tag))
            .unwrap_or_default();
        // Offline names wear their own skin's face, or the default skin of their offline UUID.
        let offline: Vec<String> = self
            .players()
            .into_iter()
            .filter(|(who, _, _)| !matches!(who, account::Player::Microsoft(_)))
            .map(|(_, name, _)| name)
            .collect();
        self.profile_heads
            .request(&self.accounts.accounts, &offline, resources, &self.tx);
    }

    fn begin_auth_job(&mut self, uuid: &str) -> Option<u64> {
        if self.accounts_ui.auth_busy.contains_key(uuid) {
            return None;
        }
        self.accounts_ui.auth_next += 1;
        self.accounts_ui
            .auth_busy
            .insert(uuid.into(), self.accounts_ui.auth_next);
        self.accounts_ui
            .auth_progress
            .insert(uuid.into(), account::session::Phase::Queued);
        Some(self.accounts_ui.auth_next)
    }

    /// Only workers read the keychain or contact authentication services. Cached public status
    /// keeps drawing independent of keychain prompts, network timeouts and refresh operations.
    fn load_auth_statuses(&mut self, renew_due: bool) {
        self.accounts_ui.auth_checked = Some(Instant::now());
        for account in self.accounts.accounts.clone() {
            let Some(generation) = self.begin_auth_job(&account.uuid) else {
                continue;
            };
            let tx = self.tx.clone();
            std::thread::spawn(move || {
                let progress = |phase| {
                    let _ = tx.send(Msg::AccountProgress(
                        account.uuid.clone(),
                        generation,
                        phase,
                    ));
                };
                progress(account::session::Phase::Keychain);
                let store = account::store();
                let mut status =
                    account::session::status_with_progress(store, &account.uuid, &progress);
                if renew_due && status.state == account::session::State::NeedsRefresh {
                    if let Ok(renewed) =
                        account::session::refresh_with_progress(&account.uuid, &progress)
                    {
                        status = renewed;
                    }
                }
                let _ = tx.send(Msg::AccountStatus(account.uuid, generation, status));
            });
        }
    }

    fn refresh_account(&mut self, uuid: String) {
        let Some(generation) = self.begin_auth_job(&uuid) else {
            return;
        };
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let progress = |phase| {
                let _ = tx.send(Msg::AccountProgress(uuid.clone(), generation, phase));
            };
            progress(account::session::Phase::Keychain);
            if let Ok(status) = account::session::refresh_with_progress(&uuid, &progress) {
                let _ = tx.send(Msg::AccountStatus(uuid, generation, status));
            } else {
                // It may have been removed through the CLI while this window was open.
                let _ = tx.send(Msg::AccountRemoved(
                    uuid,
                    generation,
                    Ok(Accounts::load().has_owner(account::store())),
                ));
            }
        });
    }

    /// Generic Add account and targeted re-sign-in share the launcher-owned device-code flow.
    fn start_sign_in(&mut self) {
        self.begin_sign_in(None);
    }

    fn re_sign_in(&mut self, uuid: &str) {
        if let Some(account) = self.accounts.find(uuid).cloned() {
            self.begin_sign_in(Some(account));
        }
    }

    fn begin_sign_in(&mut self, target: Option<account::Account>) {
        self.cancel_sign_in();
        self.accounts_ui.sign_in = Some(match &target {
            Some(account) => SignIn::start_for(account.clone()),
            None => SignIn::start(),
        });
        self.accounts_ui.sign_in_target = target;
        self.accounts_ui.sign_in_state = Some(SignInState::Starting);
    }

    fn cancel_sign_in(&mut self) {
        if let Some(old) = self.accounts_ui.sign_in.take() {
            old.cancel();
        }
        self.accounts_ui.sign_in_state = None;
        self.accounts_ui.sign_in_target = None;
    }

    fn choose_folder(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(tr("Choose").into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(mut paths))) = paths.await {
                if let Some(path) = paths.pop() {
                    let _ = this.update(cx, |this, cx| {
                        crate::toast::say(trf(
                            "Eclipse looks for Minecraft in {path}",
                            &[("path", &crate::toast::tilde(&path))],
                        ));
                        this.settings.minecraft_dir = Some(path);
                        this.save_settings();
                        if this.page == Page::Home && this.shown_prepare_error().is_some() {
                            this.press_play();
                        }
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    /// Plays the selected instance straight into a world (`--world`) or a server (`--server`):
    /// flags every Eclipse release takes.
    pub(crate) fn quick_play(&mut self, to: QuickPlay) {
        self.quick_play = Some(to);
        self.play();
        if !self.game.starting() {
            self.quick_play = None;
        }
    }

    /// Join a friend: a code field and Join, which starts the game straight
    /// into the friend's world (errors show in the game).
    fn join_friend(&mut self) {
        let code = self.fields.value(Field::InviteCode).to_string();
        if eclipse_friends::invite::parse(&code).is_none() {
            return;
        }
        self.joining = Some(code);
        self.play();
        if !self.game.starting() {
            self.joining = None;
        }
    }
}

// ----- Building blocks ---------------------------------------------------------------------------

/// What the Play button does for the selected version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlayPress {
    Play,
    /// Not prepared: prepare it (from the player's install, else downloaded from Mojang for an
    /// account that owns the game), then play, as the official launcher does.
    PrepareThenPlay,
    /// Busy preparing or playing.
    Nothing,
}

/// Polling must handle each sign-in transition once. Replaying a completed sign-in
/// restarts every account's status worker and leaves the UI perpetually checking.
fn sign_in_change(shown: &mut Option<SignInState>, next: SignInState) -> Option<SignInState> {
    if shown.as_ref() == Some(&next) {
        return None;
    }
    *shown = Some(next.clone());
    Some(next)
}

fn play_press(prepared: bool, busy: bool) -> PlayPress {
    match prepared {
        _ if busy => PlayPress::Nothing,
        true => PlayPress::Play,
        false => PlayPress::PrepareThenPlay,
    }
}

/// Whether a shortcut that starts the game at once (a server's Join, a world's Play, a friend's
/// code, a pack's Play) starts it now: only when Play would play. Never while a preparation runs:
/// the game would open on half-written files.
fn starts_now(prepared: bool, preparing: bool, running: bool) -> bool {
    play_press(prepared, preparing || running) == PlayPress::Play
}

/// Whether `tag` can start being deleted now: it is installed, no install is writing it, and it
/// is not already going. Another version deleting at the same time does not stop it.
fn removable(
    installed: &[String],
    installing: Option<&str>,
    removing: &[String],
    tag: &str,
) -> bool {
    installing != Some(tag)
        && installed.iter().any(|t| t == tag)
        && !removing.iter().any(|t| t == tag)
}

/// The selected version once one has been removed: the saved choice if it is still installed,
/// else the newest left.
fn selection_after_removal(installed: &[String], saved: Option<String>) -> Option<String> {
    saved
        .filter(|s| installed.contains(s))
        .or_else(|| installed.first().cloned())
}

fn becomes_default(
    tag: &str,
    update: Option<bool>,
    for_instance: bool,
    default: Option<&str>,
) -> bool {
    match (update, default) {
        (Some(select), _) => select && (!for_instance || default.is_none()),
        (None, None) => true,
        (None, Some(_)) if for_instance => false,
        (None, Some(now)) => backend::compare_versions(tag, now).is_gt(),
    }
}

/// The name the game made up for a player without an account, once it has run.
fn offline_name() -> Option<String> {
    player_name(&std::fs::read_to_string(backend::game_home().join("eclipse-player.txt")).ok()?)
}

/// The name in `eclipse-player.txt`: the game writes `name:Player12345` (eclipse-client's
/// offline_name.rs).
fn player_name(text: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.trim().strip_prefix("name:"))
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_owned)
}

fn changes_over(
    seen: &serde_json::Value,
    now: &serde_json::Value,
    onto: serde_json::Value,
) -> serde_json::Value {
    let (Some(seen), Some(now), serde_json::Value::Object(mut merged)) =
        (seen.as_object(), now.as_object(), onto)
    else {
        return now.clone();
    };
    for (key, value) in now {
        if seen.get(key) != Some(value) {
            merged.insert(key.clone(), value.clone());
        }
    }
    for key in seen.keys().filter(|k| !now.contains_key(*k)) {
        merged.remove(key);
    }
    serde_json::Value::Object(merged)
}

/// `1234` as `1,234`.
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn short(path: &std::path::Path) -> String {
    let home = dirs::home_dir().unwrap_or_default();
    match path.strip_prefix(&home) {
        Ok(rest) if !home.as_os_str().is_empty() => format!("~/{}", rest.display()),
        _ => path.display().to_string(),
    }
}

fn megabytes(bytes: u64) -> String {
    format!("{:.0} MB", bytes as f64 / 1_000_000.0)
}

impl Render for Launcher {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.restyle) {
            titlebar::apply_appearance(window, self.settings.appearance);
            self.theme = self.settings_theme(window);
        }
        let t = self.theme;
        let size = window.viewport_size();
        ui::begin_frame(
            &self.theme,
            f32::from(size.height) < layout::SHORT_WINDOW,
            window,
            cx,
        );
        self.scale = window.scale_factor();
        self.perf.frame();
        if self.bench.as_ref().is_some_and(perf::Bench::scrolling) {
            self.bench_scroll(window);
        }
        self.fields.note_focus(window, cx);
        self.return_focus_from_menu(window, cx);
        if self.page == Page::Home && std::mem::take(&mut self.focus_play) {
            window.focus(&self.play_focus, cx);
            self.ring_pending =
                (!window.last_input_was_keyboard()).then(|| self.play_focus.clone());
        }
        if std::mem::take(&mut self.focus_page) && self.fields.focus_soon.is_none() {
            cx.on_next_frame(window, |this, window, cx| {
                window.focus(&this.content_focus, cx);
                window.focus_next(cx);
                this.ring_pending = window
                    .focused(cx)
                    .filter(|_| !window.last_input_was_keyboard());
            });
        }
        self.window_width = f32::from(size.width);
        self.window_long_side = f32::from(size.width).max(f32::from(size.height));
        let content = match self.page {
            Page::Home => self.home_page(cx),
            Page::Versions => self.versions_page(cx),
            Page::Mods => self.mods_page(cx),
            Page::Servers => self.servers_page(cx),
            Page::Notes => self.notes_page(cx),
            Page::Settings => self.settings_page(cx),
            Page::InstanceEditor => self.instance_page(cx),
        };
        self.pictures.free_retired(window);
        if let Some(section) = self.settings_ui.enter.take() {
            cx.on_next_frame(window, move |_, window, cx| {
                window.focus(&section, cx);
                window.focus_next(cx);
            });
        }
        // The new page fades in and rises a few pixels, as the website's tabs do.
        let content = div()
            .relative()
            .w_full()
            // Let the page shrink inside the padded flex row so long notices can wrap.
            .min_w_0()
            .child(content)
            .child(scene::track_content(&self.scene))
            .with_animation(
                ElementId::named_usize("page", self.switches),
                motion::eased(motion::ARRIVE),
                |d, p| d.opacity(p).top(px(motion::ARRIVE_RISE * (1. - p))),
            );
        if window.focused(cx).is_none() {
            window.focus(&self.root_focus, cx);
        } else if !self.frame_focus.contains_focused(window, cx) {
            cx.on_next_frame(window, |this, window, cx| {
                if !this.frame_focus.contains_focused(window, cx) {
                    window.focus(&this.content_focus, cx);
                }
            });
        }
        self.scene.update(cx, |scene, _| scene.set_look(&t.night));
        let sky = self.scene.clone().into_any_element();
        let sidebar = self.sidebar(cx);
        let row_menu = self.row_menu(cx);
        let mut root = div()
            .id("root")
            .track_focus(&self.root_focus)
            .on_key_down(cx.listener(Self::root_key_down))
            // ⌘K (Ctrl+K elsewhere) wherever the focus is, even in a field: the palette.
            .capture_key_down(cx.listener(Self::palette_key_down))
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .font_family(theme::FONT)
            .text_color(ui::white(eclipse_theme::glass::ink::FULL))
            .overflow_hidden()
            .child(
                div()
                    .relative()
                    .flex()
                    .size_full()
                    .child(sidebar.track_focus(&self.side_focus))
                    .child(
                        div().flex().flex_col().flex_1().min_w_0().child(
                            self.content_scroll
                                .wrap(
                                    ui::scroll_pane("content", &self.content_scroll.handle)
                                        .role(gpui::Role::Main)
                                        .aria_label(self.page.label())
                                        .track_focus(&self.content_focus)
                                        .relative()
                                        .flex_1()
                                        .min_h_0()
                                        // A scrolled page is drawn from its layer, not
                                        // rendered: the Mods page's card asks, once the
                                        // offset has moved, whether it must follow.
                                        .on_scroll(cx.listener(
                                            |this, _: &gpui::ScrollEvent, window, cx| {
                                                if this.page != Page::Mods {
                                                    return;
                                                }
                                                cx.on_next_frame(window, |this, _, cx| {
                                                    if this
                                                        .mods_sticky
                                                        .moved(&this.content_scroll.handle)
                                                    {
                                                        cx.notify();
                                                    }
                                                });
                                            },
                                        ))
                                        .child(
                                            div()
                                                .flex()
                                                .w_full()
                                                .min_w_0()
                                                .px(px(layout::gutter(self.window_width)))
                                                .pb(px(layout::PAGE_FOOT))
                                                // Settings and the New instance page scroll
                                                // their own pane under a still title.
                                                .when(
                                                    matches!(
                                                        self.page,
                                                        Page::Settings | Page::InstanceEditor
                                                    ),
                                                    |d| d.h_full().pb_0(),
                                                )
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_1()
                                                        .min_w_0()
                                                        .max_w(px(layout::PAGE_MAX))
                                                        .mx_auto()
                                                        .child(content),
                                                ),
                                        ),
                                )
                                .flex_1()
                                .min_h_0()
                                .mt(px(layout::CONTENT_INSET)),
                        ),
                    ),
            );
        root = root.children(row_menu);
        if self.page == Page::Servers {
            if let Some(dialog) = self.create_dialog(cx) {
                root = root.child(dialog);
            }
        }
        root = self.import_drop_target(root, cx);
        root = root.children(self.import_note(cx));
        if let Some(panel) = self.native_log_panel(cx) {
            root = root.child(panel);
        }
        if self.page == Page::Home {
            if let Some(viewer) = self.screenshot_viewer(cx) {
                root = root.child(viewer);
            }
        }
        root = root.children(self.toast_float());
        if let Some(palette) = self.palette_overlay(window, cx) {
            root = root.child(palette);
        }
        if self.asking_consent() {
            root = root.child(self.consent_screen(cx));
        }
        if self.asking_owner() {
            root = root.child(self.owner_screen(cx));
        }
        root = root.children(ui::focus_tip(window));
        ui::end_frame(window, cx);
        div()
            .track_focus(&self.frame_focus)
            .flex()
            .flex_col()
            .relative()
            .size_full()
            .bg(t.side)
            .child(sky)
            .children(titlebar::render(&t, window))
            .children(self.settings_save_notice(cx))
            .child(root.flex_1().min_h_0())
            .children(titlebar::resize_handles(window))
    }
}

impl Launcher {
    /// `eclipse-launcher benchmark`: scroll the page a little on every frame, back to the top at
    /// the bottom, so the frame times measure scrolling.
    fn bench_scroll(&mut self, window: &mut Window) {
        let handle = &self.content_scroll.handle;
        let mut offset = handle.offset();
        offset.y -= px(24.);
        if -offset.y > handle.max_offset().y {
            offset.y = px(0.);
        }
        handle.set_offset(offset);
        window.request_animation_frame();
    }
}

actions!(eclipse_launcher, [Quit, CloseWindow]);

/// What closing the last window does: stop the servers this launcher started, send the telemetry,
/// exit. A game that is running keeps running. Cmd+Q goes through here too.
fn shut_down(cx: &mut App) {
    servers_page::stop_on_quit();
    telemetry::send_before_exit();
    // A staged launcher update goes in once this process has exited.
    eclipse_launcher::update_handoff::on_quit();
    cx.quit()
}

/// The macOS shortcuts: Cmd+Q quits, Cmd+W closes the window.
#[cfg(target_os = "macos")]
fn key_bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-w", CloseWindow, None),
    ]
}

#[cfg(not(target_os = "macos"))]
fn key_bindings() -> Vec<KeyBinding> {
    Vec::new()
}

/// A data folder that did not open starts on Settings, where the message, Retry and
/// "Use the default folder" are; an explicit page still wins.
fn page_after_startup(page: Page, data_folder_failed: bool) -> Page {
    if page == Page::Home && data_folder_failed {
        Page::Settings
    } else {
        page
    }
}

/// The menu bar on macOS, where "Quit" belongs to the app menu.
fn menu_bar() -> Vec<Menu> {
    vec![
        Menu {
            name: "Eclipse Launcher".into(),
            items: vec![MenuItem::action("Quit Eclipse Launcher", Quit)],
            disabled: false,
        },
        Menu {
            name: "Window".into(),
            items: vec![MenuItem::action("Close Window", CloseWindow)],
            disabled: false,
        },
    ]
}

fn main() {
    if let Err(error) = console::attach_for_cli() {
        eprintln!("Could not restore the launcher's terminal streams: {error}");
        std::process::exit(1);
    }
    // Windows setup: `setup --quiet` / `uninstall --quiet` run here without a window, and never
    // reach the CLI parser below (so neither the MCP tools nor the completions list them).
    #[cfg(windows)]
    if let Some(code) = eclipse_launcher::installer::run_quiet() {
        std::process::exit(code);
    }
    if let Some(code) = eclipse_launcher::relocation::run_worker() {
        std::process::exit(code);
    }
    // A broken location must still open a window: startup() falls back to the default
    // folders and records the error, which Settings shows with Retry and Use the default folder.
    // It keeps the migration lease for the life of the process.
    let _location_error = eclipse_launcher::data_location::startup();
    perf::process_started();
    backend::exe_path();
    // The launcher's own installer (`update_handoff`): no window, no CLI parser.
    if let Some(code) = eclipse_launcher::update_handoff::run() {
        std::process::exit(code);
    }
    #[cfg(windows)]
    backend::remove_replaced_binaries();
    backend::cleanup_launcher_updates();
    // What the launcher downloads goes through the player's proxy, if they set one: the parts
    // that build their own agents (`eclipse-port-pack`) are told before anything starts.
    eclipse_launcher::proxy::announce(eclipse_launcher::backend::Settings::load().proxy);
    // The setup wizard and `eclipse-launcher.exe uninstall` (what Control Panel runs) are windows,
    // not CLI commands: asked of the CLI parser first, `uninstall` was refused as an unknown
    // subcommand, with the error going to the console that Control Panel's start doesn't have.
    #[cfg(windows)]
    let setup_window = eclipse_launcher::installer::window_mode();
    #[cfg(not(windows))]
    let setup_window: Option<eclipse_launcher::setup_args::Mode> = None;
    if setup_window.is_none() {
        if let Some(code) = cli::run() {
            std::process::exit(code);
        }
    }
    // The GPU's texture limit, which caps the pictures made from disk (`picture_cache`).
    pictures::probe_gpu_limit();
    gpui_platform::application()
        .with_assets(icons::Assets)
        // `move`: the closure owns `setup_window`; `run` wants it to outlive this frame.
        .run(move |cx: &mut App| {
            cx.set_text_rendering_mode(gpui::TextRenderingMode::Grayscale);
            cx.text_system()
                .add_fonts(vec![
                    Cow::Borrowed(eclipse_theme::fonts::GEIST_MONO),
                    Cow::Borrowed(eclipse_theme::fonts::GEIST_REGULAR),
                    Cow::Borrowed(eclipse_theme::fonts::GEIST_MEDIUM),
                    Cow::Borrowed(eclipse_theme::fonts::GEIST_SEMIBOLD),
                    Cow::Borrowed(eclipse_theme::fonts::MONOCRAFT_REGULAR),
                ])
                .expect("bundled fonts");
            cx.bind_keys(key_bindings());
            cx.on_action(|_: &Quit, cx| shut_down(cx));
            // The key press is being handled inside the window, so the window can only be
            // removed once that update has finished.
            cx.on_action(|_: &CloseWindow, cx| {
                cx.defer(|cx| {
                    for window in cx.windows() {
                        let _ = window.update(cx, |_, window, _| window.remove_window());
                    }
                })
            });
            cx.set_menus(menu_bar());
            // The setup exe (and `eclipse-launcher.exe uninstall`) opens its own window instead.
            #[cfg(windows)]
            if let Some(mode) = setup_window {
                setup_page::open(mode, cx);
                return;
            }
            #[cfg(not(windows))]
            let _ = setup_window;
            let display = cx.primary_display().map(|d| {
                let shown = d.visible_bounds().size;
                (f32::from(shown.width), f32::from(shown.height))
            });
            let (width, height) = layout::default_window(display);
            let (min_width, min_height) = layout::min_window(display);
            let bounds = Bounds::centered(None, size(px(width), px(height)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(min_width), px(min_height))),
                    titlebar: Some(titlebar::options()),
                    #[cfg(target_os = "linux")]
                    window_decorations: Some(gpui::WindowDecorations::Client),
                    app_id: Some("eclipse-launcher".into()),
                    // GPUI draws a window without focus at most every 33 ms to save energy. A
                    // benchmark's window often starts without it (KDE keeps focus from windows
                    // a background process opens), and it measures drawing, not that limit.
                    inactive_frame_interval: if perf::Bench::wanted() {
                        None
                    } else {
                        WindowOptions::default().inactive_frame_interval
                    },
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| Launcher::new(window, cx)),
            )
            .expect("window");
            cx.on_window_closed(|cx, _| shut_down(cx)).detach();
            cx.activate(true);
        });
}

#[cfg(test)]
mod tests {
    #[test]
    fn an_unopenable_data_folder_starts_on_settings_instead_of_exiting() {
        use super::{page_after_startup, Page};
        assert!(matches!(
            page_after_startup(Page::Home, true),
            Page::Settings
        ));
        assert!(matches!(page_after_startup(Page::Home, false), Page::Home));
        assert!(matches!(page_after_startup(Page::Mods, true), Page::Mods));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn cmd_q_quits_and_cmd_w_closes_the_window() {
        use super::{key_bindings, CloseWindow, Quit};
        use gpui::{Keymap, Keystroke};
        let keymap = Keymap::new(key_bindings());
        let key = |s: &'static str| [Keystroke::parse(s).unwrap()];
        let (hit, _) = keymap.bindings_for_input(&key("cmd-q"), &[]);
        assert!(hit.iter().any(|b| b.action().partial_eq(&Quit)));
        let (hit, _) = keymap.bindings_for_input(&key("cmd-w"), &[]);
        assert!(hit.iter().any(|b| b.action().partial_eq(&CloseWindow)));
    }

    #[test]
    fn a_second_version_can_be_removed_while_another_deletes() {
        let installed = ["v0.2.97".to_string(), "v0.2.76".to_string()];
        let removing = ["v0.2.97".to_string()];
        assert!(super::removable(&installed, None, &removing, "v0.2.76"));
        assert!(!super::removable(&installed, None, &removing, "v0.2.97"));
        assert!(!super::removable(
            &installed,
            Some("v0.2.76"),
            &[],
            "v0.2.76"
        ));
        assert!(!super::removable(&installed, None, &[], "v0.2.85"));
    }

    #[test]
    fn an_install_moves_the_default_only_forward() {
        use super::becomes_default;
        let newer = "nightly-2026-11-01-4aa6854";
        let older = "nightly-2026-10-20-1b2c3d4";
        assert!(becomes_default(older, None, false, None));
        assert!(becomes_default(newer, None, false, Some(older)));
        assert!(!becomes_default(older, None, false, Some(newer)));
        assert!(!becomes_default(newer, None, true, Some(older)));
        assert!(becomes_default(newer, None, true, None));
        assert!(becomes_default(newer, Some(true), false, Some(older)));
        assert!(!becomes_default(newer, Some(false), false, Some(older)));
    }

    #[test]
    fn the_selection_survives_a_removal_or_moves_to_the_newest_left() {
        let left = ["v0.2.49".to_string(), "v0.2.48".to_string()];
        let pick =
            |saved: Option<&str>| super::selection_after_removal(&left, saved.map(String::from));
        assert_eq!(pick(Some("v0.2.48")).as_deref(), Some("v0.2.48"));
        assert_eq!(pick(Some("v0.2.47")).as_deref(), Some("v0.2.49"));
        assert_eq!(pick(None).as_deref(), Some("v0.2.49"));
        assert_eq!(
            super::selection_after_removal(&[], Some("v0.2.47".into())),
            None
        );
    }

    /// Play works on a version not prepared yet: it prepares it (downloading Minecraft from
    /// Mojang for an owner when this computer has no install), then plays.
    #[test]
    fn play_prepares_first() {
        use super::{play_press, PlayPress};
        assert_eq!(play_press(true, false), PlayPress::Play);
        assert_eq!(play_press(false, false), PlayPress::PrepareThenPlay);
        assert_eq!(play_press(false, true), PlayPress::Nothing);
        assert_eq!(play_press(true, true), PlayPress::Nothing);
    }

    /// A server's Join (and every other shortcut) waits like Play: not while the version's
    /// first download and preparation run (#eclipse-bugs, 0.2.23: a white window opened).
    #[test]
    fn shortcuts_start_only_when_play_would() {
        use super::starts_now;
        assert!(starts_now(true, false, false));
        assert!(
            !starts_now(false, true, false),
            "first-time setup under way"
        );
        assert!(!starts_now(true, true, false), "prepared again under way");
        assert!(!starts_now(false, false, false), "not prepared");
        assert!(!starts_now(true, false, true), "already running");
    }

    #[test]
    fn a_completed_sign_in_is_handled_once_across_ui_polls() {
        use super::{sign_in_change, SignInState};
        let mut shown = None;
        let completed = SignInState::Done(Ok(eclipse_launcher::account::Account {
            uuid: "11111111111111111111111111111111".into(),
            name: "TestAccount".into(),
            skin: None,
            cape: None,
        }));
        assert!(sign_in_change(&mut shown, SignInState::Starting).is_some());
        assert!(sign_in_change(&mut shown, SignInState::Starting).is_none());
        let mut account_reloads = 0;
        for _ in 0..100 {
            if matches!(
                sign_in_change(&mut shown, completed.clone()),
                Some(SignInState::Done(Ok(_)))
            ) {
                account_reloads += 1;
            }
        }
        assert_eq!(
            account_reloads, 1,
            "completed sign-in must not restart account checks every frame"
        );
        let failed = SignInState::Done(Err("Sign-in cancelled".into()));
        assert!(sign_in_change(&mut shown, failed.clone()).is_some());
        assert!(sign_in_change(&mut shown, failed).is_none());
    }

    #[test]
    fn the_app_menu_has_quit() {
        let menus = super::menu_bar();
        assert_eq!(menus[0].name, "Eclipse Launcher");
        assert!(matches!(
            &menus[0].items[0],
            gpui::MenuItem::Action { name, .. } if name == "Quit Eclipse Launcher"
        ));
    }

    #[test]
    fn the_offline_name_is_read_from_its_key() {
        use super::player_name;
        assert_eq!(
            player_name("name:Player63010\n").as_deref(),
            Some("Player63010")
        );
        assert_eq!(player_name("name:Steve\r\n").as_deref(), Some("Steve"));
        assert_eq!(player_name("Player63010\n"), None);
        assert_eq!(player_name(""), None);
    }

    #[test]
    fn a_save_keeps_what_another_process_wrote_meanwhile() {
        use serde_json::json;
        let seen = json!({"language": "en", "proxy": "http://old", "accent": "amber"});
        let now = json!({"language": "de", "accent": "amber"});
        let disk =
            json!({"language": "en", "proxy": "http://old", "accent": "blue", "game_args": "-x"});
        assert_eq!(
            super::changes_over(&seen, &now, disk),
            json!({"language": "de", "accent": "blue", "game_args": "-x"})
        );
    }

    #[test]
    fn counts_get_thousands_separators() {
        let shown: Vec<String> = [0, 7, 999, 1000, 12_300, 1_234_500]
            .into_iter()
            .map(super::thousands)
            .collect();
        assert_eq!(shown, ["0", "7", "999", "1,000", "12,300", "1,234,500"]);
    }
}
