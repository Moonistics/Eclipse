//! Eclipse launcher: installs Eclipse builds, prepares them from the player's own Minecraft,
//! and starts the client or a dedicated server.

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod accounts_switch;
mod accounts_ui;
mod art;
mod cli;
mod cli_tools;
mod clipboard;
mod consent_sheet;
mod console;
mod cull;
mod dates;
mod dropped_files;
mod fields;
mod folder_list;
mod frame;
mod game_run;
mod game_tools;
mod home_console;
mod home_head;
mod home_mods;
mod home_mods_state;
mod home_multiplayer;
mod home_multiplayer_state;
mod home_packs;
mod home_packs_state;
mod home_page;
mod home_screenshots;
mod home_screenshots_state;
mod home_tabs;
mod home_world_packs;
mod home_worlds;
mod home_worlds_state;
mod icons;
mod instance_export;
mod instance_icons;
mod instance_launch;
mod instance_page;
mod instance_state;
mod instance_versions;
mod launcher_update;
mod layout;
mod line_edit;
mod mod_rows;
mod mods_actions;
mod mods_github;
mod servers_actions;
mod servers_create;
mod servers_internet;
mod servers_page;
mod servers_tabs;
mod settings_ai;
mod settings_data_collection;
mod settings_developer;
mod startup_location;

mod instance_memory;
mod mcp;
mod menus;
mod messages;
mod microsoft;
mod mods_modpacks;
mod mods_page;
mod mods_project;
mod mods_state;
mod motion;
mod native_build;
mod native_build_log;
mod native_build_view;
mod notes_page;
mod owner_gate;
mod page;
mod palette;
mod perf;

mod instance_shortcut;
mod pictures;
mod play;
mod play_state;
mod preview;
mod profile_heads;
mod scene;
mod scrollbar;
mod servers_state;
mod settings_account;
mod settings_appearance;
mod settings_game;
mod settings_language;
mod settings_page;
mod settings_skin;
#[cfg(windows)]
mod setup_window;
mod sticky;
mod tab_strip;
mod theme;
mod titlebar;
mod toast;
mod ui;
mod versions_page;
mod versions_state;
mod window_aside;

use eclipse_launcher::account::{self, Accounts, SignIn, SignInState};
use eclipse_launcher::backend::{self, Release, Running, Settings, Updates};
use eclipse_launcher::i18n::{self, tr, trf};
use eclipse_launcher::instances;
use eclipse_launcher::native_mods;
use eclipse_launcher::telemetry;
use fields::Field;
use game_run::Crash;
use gpui::{
    actions, div, prelude::*, px, size, AnimationExt, AnyElement, App, Bounds, ClickEvent, Context,
    Div, ElementId, FocusHandle, FontWeight, KeyBinding, KeyDownEvent, Menu, MenuItem,
    MouseDownEvent, PathPromptOptions, Pixels, Point, SharedString, Stateful, Subscription, Window,
    WindowBounds, WindowOptions,
};
use launcher_update::SelfUpdate;
use messages::{Job, Msg};
use page::Page;
use play_state::PlayState;
use scrollbar::Scrollbar;
use std::borrow::Cow;
use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};
use theme::Theme;
use ui::{ButtonKind, Press};

enum Load<T> {
    Loading,
    Ready(T),
    Failed(String),
}

const LOG_LINES: usize = 2000;

#[derive(Clone, Copy, PartialEq)]
enum Pick {
    File,
    Files,
    Folder,
}

struct Launcher {
    theme: Theme,
    theme_for: Option<theme::Inputs>,
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
    /// The Mod build log panel, opened from the sidebar's foot (`native_build_log.rs`).
    native_log: native_build_log::NativeLog,
    open_menu: Option<menus::Menu>,
    asking: Option<menus::Asking>,
    /// Where the sidebar's rows were drawn, for the ⋯ menu beside them (`frame.rs`).
    row_menu: frame::RowMenu,
    menu_opener: Option<FocusHandle>,
    /// Where the mouse went down to close a menu by pressing outside it, so that the
    /// same press, ending on the menu's own button, doesn't open it again.
    menu_closed_at: Option<Point<Pixels>>,
    /// Settings > Account's skin panel for an offline name (`settings_skin.rs`).
    skin: Option<settings_skin::SkinPanel>,
    /// Why the last picture for Home's background was refused.
    background_error: Option<String>,
    /// The data folder's startup problem.
    location_error: Option<String>,
    settings: Settings,
    settings_seen: serde_json::Value,
    /// The last failed settings write, kept across page changes until a save succeeds.
    settings_save_error: Option<String>,
    /// The Discord, sponsor and website links: built in until links.json on main has been read.
    links: backend::Links,
    game: game_run::GameRun,
    fields: fields::Fields,
    /// The Servers page (`servers_state.rs`).
    servers: servers_state::Servers,
    /// Its Friends over the internet panel (`servers_internet.rs`).
    internet: servers_internet::Internet,
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
    /// (`settings_developer`) for this run.
    dev_clicks: u8,
    dev_options: bool,
    /// The window's width at the last frame: the sidebar and Home's head lay out by it.
    window_width: f32,
    /// The window's longer side in points: the size pictures are made at (`pictures`).
    window_long_side: f32,
    /// Opt-in data collection: the player's answer and Settings' section (`settings_data_collection`).
    collection: settings_data_collection::Collection,
    /// Settings' "Connect an AI agent" (`settings_ai`).
    agents: settings_ai::AgentsUi,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    _appearance: Subscription,
    /// Redraws when the window gains or loses focus: the sky moves only in front.
    _activation: Subscription,
    /// The session's measurements (`perf.rs`), and when the steps they time began.
    perf: perf::Perf,
    /// The download under way as first seen (when, files done), for the time left.
    download_seen: Option<(Instant, usize)>,
    /// The tab open under Home's head (`home_tabs.rs`); None opens the first one.
    home_tab: Option<home_tabs::HomeTab>,
    home_tabs_drawn: Option<Vec<home_tabs::HomeTab>>,
    /// The selected instance's mods, for Home's Mods tab (`home_mods_state.rs`).
    instance_mods: home_mods_state::InstanceMods,
    /// Modrinth projects' icon URLs (`mod_rows.rs`).
    project_icons: mod_rows::ProjectIcons,
    /// `eclipse-launcher benchmark` started this window to measure it.
    bench: Option<perf::Bench>,
    modpacks: mods_modpacks::Modpacks,
    /// The Mods page: the browser's search, installs and updates (`mods_state.rs`).
    mods: mods_state::Mods,
    github: mods_github::Github,
    /// The instances, for the sidebar and Play, and their dialog (`instance_state.rs`).
    instances: instance_state::Instances,
    /// Home's Worlds section, of the selected instance (`home_worlds_state.rs`).
    worlds: home_worlds_state::Worlds,
    multiplayer: home_multiplayer_state::Multiplayer,
    screenshots: home_screenshots_state::Screenshots,
    shot_blur: home_screenshots_state::ShotBlur,
    /// Pictures from disk made at their drawn size (`pictures`).
    pictures: pictures::Pictures,
    game_packs: home_packs_state::Packs,
    world_packs: home_world_packs::WorldPacks,
    export: Option<instance_export::ExportPanel>,
    launch: Option<instance_launch::LaunchDraft>,
    /// The memory row on Edit instance.
    memory_slider: instance_launch::MemorySlider,
    importing: dropped_files::Importing,
    console: home_console::Console,
    /// What the window did when the game started (`window_aside.rs`).
    aside: window_aside::Aside,
    /// The selected instance's desktop shortcut (`instance_shortcut.rs`).
    shortcuts: instance_shortcut::Shortcuts,
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
        let appearance = cx.observe_window_appearance(window, |_, _, cx| cx.notify());
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
        settings.default_version =
            backend::followed_version(settings.default_version.as_ref(), &installed).cloned();
        // Pin the window's chrome (macOS title bar) to the choice before the theme reads
        // `window.appearance()`, so both draw the same look.
        titlebar::apply_appearance(window, settings.appearance);
        let location_error = startup_location::startup_error();
        let preview = preview::preview();
        let (page, section) = preview::start_at(preview, location_error.is_some());
        let mut this = Self {
            theme: Theme::for_settings(&settings, window.appearance()),
            theme_for: None,
            page,
            switches: 0,
            content_scroll: Scrollbar::default(),
            settings_ui: settings_page::SettingsUi {
                open: section,
                ..Default::default()
            },
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
            background_error: None,
            location_error,
            settings,
            settings_seen,
            settings_save_error: None,
            links: backend::Links::default(),
            game: Default::default(),
            fields: Default::default(),
            servers: servers_state::Servers::new(),
            internet: servers_internet::Internet::new(),
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
            collection: settings_data_collection::Collection::load(),
            agents: settings_ai::AgentsUi::default(),
            tx,
            rx,
            _appearance: appearance,
            _activation: cx.observe_window_activation(window, |_, _, cx| cx.notify()),
            perf: perf::Perf::new("session"),
            download_seen: None,
            home_tab: match preview {
                Some(preview::Preview::HomeTab(tab)) => Some(tab),
                _ => None,
            },
            home_tabs_drawn: None,
            instance_mods: Default::default(),
            project_icons: Default::default(),
            bench: perf::Bench::from_env(),
            modpacks: Default::default(),
            mods: mods_state::Mods::new(),
            github: mods_github::Github::new(),
            instances: instance_state::Instances::default(),
            worlds: home_worlds_state::Worlds::default(),
            multiplayer: home_multiplayer_state::Multiplayer::default(),
            screenshots: home_screenshots_state::Screenshots::default(),
            shot_blur: Default::default(),
            pictures: Default::default(),
            game_packs: home_packs_state::Packs::default(),
            world_packs: home_world_packs::WorldPacks::default(),
            export: None,
            launch: None,
            memory_slider: Default::default(),
            importing: Default::default(),
            console: Default::default(),
            aside: window_aside::Aside::new(window.window_handle()),
            shortcuts: Default::default(),
            offline_name: offline_name(),
            scene: cx.new(|_| scene::Scene::new()),
            scale: window.scale_factor(),
        };
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
        // Asking the keychain can block (D-Bus), so not on this thread.
        let accounts = this.accounts.clone();
        this.in_background(move |tell| {
            let store = account::store();
            tell.send(Msg::Keychain(account::no_keychain()));
            tell.send(Msg::Owner(accounts.has_owner(store)));
        });
        this.in_background(|tell| {
            if !backend::reuse_prepared_everywhere().is_empty() {
                tell.send(Msg::Reused);
            }
        });
        this.load_profile_heads();
        this.load_auth_statuses(true);
        this.fetch_startup_releases();
        this.mods_auto_update();
        this.fetch_links();
        if preview::shows(preview::Preview::GettingReady) {
            let selected = this.selected_instance().cloned();
            let start =
                selected
                    .clone()
                    .zip(this.selected_version().cloned())
                    .map(|(instance, tag)| game_run::Start {
                        instance,
                        tag,
                        to: None,
                        updates_mods: true,
                    });
            this.game.preparing_now(selected.as_ref(), start);
        }
        match preview {
            Some(preview::Preview::NewInstance(kind)) => {
                this.open_instance_page(None);
                this.pick_kind(kind, cx);
            }
            Some(preview::Preview::EditInstance) => {
                this.open_instance_page(this.selected_instance().cloned())
            }
            Some(preview::Preview::DuplicateInstance) => this.start_copy(),
            _ => {}
        }
        // What the game and the launcher kept, if the player allows it and there is a server.
        telemetry::send_in_background();
        this
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

    fn pull_settings(&mut self) {
        let disk = serde_json::to_value(Settings::load()).unwrap_or_default();
        let now = serde_json::to_value(&self.settings).unwrap_or_default();
        let merged = changes_over(&self.settings_seen, &now, disk.clone());
        if let Ok(settings) = serde_json::from_value::<Settings>(merged) {
            self.settings = settings;
        }
        self.settings_seen = disk;
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
        self.reload_accounts();
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
            self.page_opened();
        }
        self.open_menu = None;
    }

    fn fetch_links(&self) {
        self.in_background(|tell| tell.send(Msg::Links(backend::links())));
    }

    fn choose_folder(&mut self, cx: &mut Context<Self>) {
        self.pick_paths(Pick::Folder, tr("Choose"), cx, |this, mut paths, _| {
            let Some(path) = paths.pop() else {
                return;
            };
            crate::toast::say(trf(
                "Eclipse looks for Minecraft in {path}",
                &[("path", &short(&path))],
            ));
            this.settings.minecraft_dir = Some(path);
            this.save_settings();
            if this.page == Page::Home && this.shown_prepare_error().is_some() {
                this.press_play();
            }
        });
    }

    fn pick_paths(
        &self,
        pick: Pick,
        prompt: &str,
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut Self, Vec<std::path::PathBuf>, &mut Context<Self>) + 'static,
    ) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: pick != Pick::Folder,
            directories: pick == Pick::Folder,
            multiple: pick == Pick::Files,
            prompt: Some(prompt.to_string().into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = paths.await {
                let _ = this.update(cx, |this, cx| {
                    then(this, paths, cx);
                    cx.notify();
                });
            }
        })
        .detach();
    }
}

// ----- Building blocks ---------------------------------------------------------------------------

/// The name the game made up for a player without an account, once it has run.
fn offline_name() -> Option<String> {
    eclipse_launcher::game_folder::player_name(&backend::game_home())
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

/// `/Users/me/x` as `~/x`.
fn short(path: &std::path::Path) -> String {
    short_from(path, &dirs::home_dir().unwrap_or_default())
}

fn short_from(path: &std::path::Path, home: &std::path::Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) if !home.as_os_str().is_empty() => format!("~/{}", rest.display()),
        _ => path.display().to_string(),
    }
}

fn megabytes(bytes: u64) -> String {
    format!("{:.0} MB", bytes as f64 / 1_000_000.0)
}

impl Render for Launcher {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.theme_for != Some(theme::inputs(&self.settings, window.appearance())) {
            titlebar::apply_appearance(window, self.settings.appearance);
            self.theme = Theme::for_settings(&self.settings, window.appearance());
            self.theme_for = Some(theme::inputs(&self.settings, window.appearance()));
        }
        let t = self.theme;
        let short = f32::from(window.viewport_size().height) < layout::SHORT_WINDOW;
        let (root, sky) = ui::frame(&t, short, window, cx, |window, cx| self.draw(window, cx));
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
    fn draw(&mut self, window: &mut Window, cx: &mut Context<Self>) -> (Stateful<Div>, AnyElement) {
        let t = self.theme;
        let size = window.viewport_size();
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
            .when(!self.page.scrolls_itself(), |d| {
                d.child(scene::track_content(&self.scene))
            })
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
                                        .on_scroll(cx.listener(
                                            |this, _: &gpui::ScrollEvent, window, cx| {
                                                this.page_scrolled(window, cx)
                                            },
                                        ))
                                        .child(
                                            div()
                                                .flex()
                                                .w_full()
                                                .min_w_0()
                                                .px(px(layout::gutter(self.window_width)))
                                                .pb(px(layout::PAGE_FOOT))
                                                .when(self.page.scrolls_itself(), |d| {
                                                    d.h_full().pb_0()
                                                })
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
        root = root.children(self.page_dialog(cx));
        root = self.import_drop_target(root, cx);
        root = root.children(self.import_note(cx));
        if let Some(panel) = self.native_log_panel(cx) {
            root = root.child(panel);
        }
        root = root.children(self.page_viewer(cx));
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
        (root, sky)
    }

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
    servers_state::stop_on_quit();
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
    perf::process_started();
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
    // A broken location must still open a window: startup() falls back to the default
    // folders and records the error.
    eclipse_launcher::data_location::startup();
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
    let setup = eclipse_launcher::installer::window_mode();
    #[cfg(not(windows))]
    let setup: Option<eclipse_launcher::setup_args::Mode> = None;
    if setup.is_none() {
        if let Some(code) = cli::run() {
            std::process::exit(code);
        }
    }
    // The GPU's texture limit, which caps the pictures made from disk (`pictures`).
    pictures::probe_gpu_limit();
    gpui_platform::application()
        .with_assets(icons::Assets)
        // `move`: the closure owns `setup`; `run` wants it to outlive this frame.
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
            if let Some(mode) = setup {
                setup_window::open(mode, cx);
                return;
            }
            #[cfg(not(windows))]
            let _ = setup;
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
    fn a_path_is_shortened_only_inside_the_home_folder() {
        use super::short_from;
        use std::path::Path;
        let home = Path::new("/home/me");
        assert_eq!(short_from(Path::new("/home/me/saves"), home), "~/saves");
        assert_eq!(
            short_from(Path::new("/home/meg/saves"), home),
            "/home/meg/saves"
        );
        assert_eq!(
            short_from(Path::new("/home/meg/saves"), Path::new("")),
            "/home/meg/saves"
        );
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
