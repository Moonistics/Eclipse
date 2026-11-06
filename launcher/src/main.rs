//! Eclipse launcher: installs Eclipse builds, prepares them from the player's own Minecraft,
//! and starts the client or a dedicated server.

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod agents_page;
mod art;
mod blocks;
mod cli;
mod cli_tools;
mod collection;
mod console;
mod console_page;
mod cull;
mod datapacks_page;
mod dev_options;
mod export_page;
mod frame;
mod game_tools;
mod github_mods_page;
mod glass;
mod glass_account;
mod glass_home;
mod glass_home_lists;
mod glass_home_mods;
mod glass_home_packs;
mod glass_home_tabs;
mod glass_instance;
mod glass_instance_java;
mod glass_notes;
mod glass_palette;
mod glass_settings;
mod glass_shot_blur;
mod glass_toast;
mod glass_ui;
mod glass_versions;
mod home;
mod home_tabs;
mod import_page;
mod instance_icons;
mod instance_mods;
mod instances_page;
mod internet;
mod language_page;
mod launch_page;
mod layout;
mod line_edit;
mod location_page;
mod markdown;
mod mcp;
mod microsoft;
mod modpacks_page;
mod mods_page;
mod mods_project;
mod motion;
mod multiplayer_page;
mod native_log_page;
mod new_instance;
mod owner_page;
mod packs_page;
mod perf;
mod picture_cache;
mod profile_heads;
mod scene;
mod screenshots_page;
mod scrollbar;
mod servers_page;
#[cfg(windows)]
mod setup_page;
mod shortcut_page;
mod skin_page;
#[cfg(windows)]
mod sky;
mod sticky;
mod switch_page;
mod theme;
mod themes_page;
mod titlebar;
mod while_playing;
mod worlds_page;

use eclipse_launcher::account::{self, Accounts, SignIn, SignInState};
use eclipse_launcher::backend::{self, Job, Msg, Release, Running, Settings, Updates};
use eclipse_launcher::build_import;
use eclipse_launcher::i18n::{self, tr, trf};
use eclipse_launcher::instances;
use eclipse_launcher::native_mods;
use eclipse_launcher::native_status;
use eclipse_launcher::telemetry;
use glass_ui::Press;
use gpui::{
    actions, div, ease_out_quint, prelude::*, px, relative, size, Animation, AnimationExt,
    AnyElement, App, Bounds, ClickEvent, Context, Div, ElementId, FocusHandle, FontWeight, Hsla,
    KeyBinding, KeyDownEvent, Menu, MenuItem, MouseDownEvent, PathPromptOptions, Pixels, Point,
    SharedString, Stateful, Subscription, Window, WindowBounds, WindowOptions,
};
use scrollbar::Scrollbar;
use std::borrow::Cow;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::Ordering;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use theme::Theme;

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Play,
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
    /// Modded; `edit-instance`: the selected one's Edit page; `worlds`, `packs`...: Home on that tab, `HomeTab::from_env`).
    fn from_env() -> Page {
        let page = match std::env::var("ECLIPSE_LAUNCHER_PAGE").as_deref() {
            Ok("mods" | "modpacks" | "resourcepacks" | "shaders" | "datapacks") => Page::Mods,
            Ok("servers") => Page::Servers,
            Ok("versions") => Page::Versions,
            Ok("notes") => Page::Notes,
            Ok("settings") => Page::Settings,
            _ => Page::Play,
        };
        // The data folder did not open: start on Settings, where the message and the two
        // recoveries are (an explicit ECLIPSE_LAUNCHER_PAGE still wins, for screenshots).
        page_after_startup(
            page,
            eclipse_launcher::data_location::startup_error().is_some(),
        )
    }

    /// Every page, with its name and icon.
    const ALL: [(Page, &'static str, &'static str); 6] = [
        (Page::Play, "Home", "home"),
        (Page::Mods, "Mods", "package"),
        (Page::Servers, "Servers", "server"),
        (Page::Versions, "Versions", "archive"),
        (Page::Notes, "Patch notes", "notes"),
        (Page::Settings, "Settings", "sliders"),
    ];

    const NAV: [(Page, &'static str, &'static str); 5] = [
        (Page::Play, "Home", "dashboard-square-01"),
        (Page::Mods, "Mods", "puzzle"),
        (Page::Servers, "Servers", "computer"),
        (Page::Versions, "Versions", "package-01"),
        (Page::Notes, "Patch notes", "information-circle"),
    ];
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
    copied: bool,
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
    /// The glass Settings page's own scroll: its sections beside their list.
    settings_pane: gpui::ScrollHandle,
    /// The section the glass Settings list marked when last drawn (`glass_settings`).
    settings_marked: usize,
    /// The Mods page's project card, kept in view beside its list.
    mods_sticky: sticky::Sticky,
    /// The rows of the page's long list (Versions, Patch notes) last drawn.
    long_list: cull::Cull,
    updates: Updates,
    checked: Option<Instant>,
    update_error: Option<String>,
    self_update: SelfUpdate,
    self_update_hidden: bool,
    /// The Eclipse version being installed as an update, and whether it becomes the selected one.
    update_install: Option<(String, bool)>,
    /// The Eclipse version being installed because an instance was made or saved to play it: it
    /// becomes the launcher's version only when the launcher has none.
    instance_install: Option<String>,
    /// One for an instance waiting for the install running now to end.
    install_next: Option<Release>,
    /// The install bar's previous and current fraction, so it moves rather than jumps.
    bar: (f32, f32),
    releases: Load<Vec<Release>>,
    installed: Vec<String>,
    installing: Option<(String, f32)>,
    /// The versions being deleted on background threads. Each is already out of `installed`, so
    /// nothing can launch, select or prepare it, and its row can't install or remove it again.
    /// More than one may run at once: a row's Remove stays usable while another version goes.
    removing: Vec<String>,
    /// What each installed version takes on disk, measured on a thread for the Versions page, and
    /// the installed list it was measured for.
    disk: Vec<(String, u64)>,
    disk_for: Vec<String>,
    install_error: Option<(String, String)>,
    /// Import a game build (`build_import`, on the Versions page): whether one is being copied in,
    /// and why the last one failed.
    importing_build: bool,
    import_error: Option<String>,
    /// Native mods (`native_mods`, on the Mods page): the build's step while it runs, why it
    /// failed, and the warning before a build while it is open, with the mods it names.
    native_step: Option<String>,
    /// When the running native build started, and the whole seconds last drawn (the status shows
    /// the time it has taken).
    native_started: Option<(Instant, u64)>,
    /// How the last native build ended ("Built in 42 s (fast, incremental)", or "Up to date").
    native_done: Option<String>,
    native_error: Option<String>,
    native_confirm: Option<Result<Vec<native_mods::Bundle>, String>>,
    /// The instance (id) the build under way, or the last one's error, is for.
    native_for: Option<String>,
    /// The newer Eclipse the open warning's build moves to (Update), else None: a build stays
    /// on the Eclipse the instance plays.
    native_update: Option<String>,
    /// The build waiting for the trust warning builds every mod on its Eclipse's own commit,
    /// pinned or not (`native_mods::build`'s `on_base`): Rebuild on Eclipse's.
    native_on_base: bool,
    /// What each installed native build was made from, by version. A build's version names its
    /// contents, so an entry never goes stale.
    native_builds:
        std::cell::RefCell<std::collections::HashMap<String, Option<native_status::Build>>>,
    /// The mods of a build made for other Eclipse versions (`native_mods::MISMATCH`): the build
    /// waits for "Build anyway".
    native_mismatch: Option<String>,
    /// The top bar's Build log panel (`native_log_page.rs`).
    native_log: native_log_page::NativeLog,
    confirm_remove: Option<String>,
    picker_open: bool,
    /// Settings' any-colour swatch shows the hue strip.
    accent_strip: bool,
    /// Where the accent picker's hue strip is on screen (`themes_page.rs`), to map a click to a
    /// hue.
    hue_area: std::rc::Rc<std::cell::Cell<Option<Bounds<gpui::Pixels>>>>,
    /// A drag on that strip is in flight, so a release anywhere writes the accent down.
    hue_dragging: bool,
    /// Settings > Language's dropdown (`language_page.rs`).
    language_menu: bool,
    /// The top bar's quick switch of who plays (`switch_page.rs`).
    account_menu: bool,
    /// The glass sidebar's ⋮ menu (`frame.rs`).
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
    /// The last failed settings write, kept across page changes until a save succeeds.
    settings_save_error: Option<String>,
    /// The Discord, sponsor and website links: built in until links.json on main has been read.
    links: backend::Links,
    preparing: bool,
    prepare_log: VecDeque<String>,
    prepare_error: Option<String>,
    client: Option<Running>,
    client_error: Option<String>,
    /// When the running client started, to tell its crash report from older ones.
    client_started: Option<std::time::SystemTime>,
    /// The folder the running game keeps its settings in: the game home, or a modpack's.
    client_home: Option<std::path::PathBuf>,
    crash: Option<Crash>,
    /// The Servers page (`servers_page.rs`).
    servers: servers_page::Servers,
    /// Its Friends over the internet panel (`internet.rs`).
    internet: internet::Internet,
    /// The window's own focus, so Tab reaches the controls when nothing else has the keyboard.
    /// The palette (⌘K), while open (`glass_palette`).
    palette: Option<glass_palette::Palette>,
    /// The toast last drawn (`glass_toast`), by its number.
    toast_seen: Option<u64>,
    root_focus: FocusHandle,
    /// Microsoft accounts (accounts.json), the sign-in under way and what it last showed.
    accounts: Accounts,
    auth_status: HashMap<String, account::session::Status>,
    auth_progress: HashMap<String, account::session::Phase>,
    profile_heads: profile_heads::Heads,
    /// When page switches last checked every account's sign-in.
    auth_checked: Option<Instant>,
    sign_in_target: Option<account::Account>,
    auth_busy: HashMap<String, u64>,
    auth_next: u64,
    sign_in: Option<SignIn>,
    sign_in_state: Option<SignInState>,
    /// What to say when the keychain doesn't keep sign-ins ([`account::no_keychain`]).
    keychain: Option<String>,
    /// Whether an account here owns the game, by the keychain's last answers (the owner gate);
    /// none until the keychain has been read.
    owner: Option<bool>,
    /// Play was refused (or Home's Sign in pressed): the owner gate's screen is open.
    owner_screen_open: bool,
    /// Clicks on the sidebar's launcher version, and whether they revealed the developer options
    /// (`dev_options`) for this run.
    dev_clicks: u8,
    dev_options: bool,
    /// The game is waiting for its session (a token renewal), or a Java instance for its files
    /// and Java (`start_java`), before it starts.
    starting_client: Option<String>,
    /// Play was pressed on a version not prepared yet: it starts once this preparation ends well.
    play_when_prepared: Option<String>,
    /// Join a friend: the code typed on the Play page, and the one the game
    /// starting now joins.
    friend_code: line_edit::LineEdit,
    friend_focused: bool,
    friend_focus: FocusHandle,
    /// The window's width at the last frame: Home's head lays out by it, as the mockup's `xl`.
    window_width: f32,
    /// The window's longer side in points, and the screen's pixels per point: the size pictures
    /// are made at (`picture_cache`).
    window_long_side: f32,
    scale_factor: f32,
    joining: Option<String>,
    /// Where the next start goes at once: `--world <folder>` or `--server <address>`.
    quick_play: Option<(&'static str, String)>,
    /// The instance the running game plays and when it started: its play time grows on exit.
    client_run: Option<(String, Instant)>,
    /// The running game's instance variables, for its post-exit command.
    client_vars: eclipse_launcher::launch::Vars,
    /// Settings' Game arguments field: the line, whether it has focus, and its focus handle.
    args_input: line_edit::LineEdit,
    args_focused: bool,
    args_focus: FocusHandle,
    /// Settings' Proxy field, the same way (`proxy`).
    proxy_input: line_edit::LineEdit,
    proxy_focused: bool,
    proxy_focus: FocusHandle,
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
    install_started: Option<Instant>,
    prepare_started: Option<Instant>,
    /// The download under way as first seen (when, files done), for the time left.
    download_seen: Option<(Instant, usize)>,
    /// The tab open under Home's hero (`home_tabs.rs`).
    home_tab: home_tabs::HomeTab,
    /// Home's Play or folder menu, while open.
    home_menu: glass_home::HomeMenu,
    /// The list row under the pointer in Home's glass tabs (a world, a server): it shows its
    /// actions then.
    list_hover: Option<SharedString>,
    /// A Home list row's ⋯ menu, while open, and whether its Delete… was clicked once.
    list_menu: Option<(SharedString, bool)>,
    /// When the running game was first seen running, for Home's "Playing for…".
    client_since: Option<Instant>,
    /// The selected instance's mods, for Home's Mods tab (`instance_mods.rs`).
    instance_mods: instance_mods::InstanceMods,
    /// Modrinth projects' icon URLs (`instance_mods.rs`).
    project_icons: instance_mods::ProjectIcons,
    play_pressed: Option<Instant>,
    /// `eclipse-launcher benchmark` started this window to measure it.
    bench: Option<perf::Bench>,
    /// The Modpacks page: the imported packs (read when the page first opens), the one shown,
    /// the report's filter, and the pipeline step running now with its log.
    packs: Vec<eclipse_launcher::modpacks::Imported>,
    /// Modpacks installed from Eclipse's registry, each its own profile.
    registry_packs: Vec<eclipse_launcher::browser::eclipse::pack::Installed>,
    packs_loaded: Option<Instant>,
    pack_open: Option<String>,
    pack_filter: Option<eclipse_port_pack::report::Status>,
    pack_busy: bool,
    pack_log: VecDeque<String>,
    pack_error: Option<String>,
    /// The game starting now plays this pack's profile (its work folder).
    starting_pack: Option<std::path::PathBuf>,
    /// The Mods page: the browser's search, installs and updates (`mods_page.rs`).
    mods: mods_page::Mods,
    github: github_mods_page::Github,
    /// The instances, for the sidebar and Play, and their dialog (`instances_page.rs`).
    instances: instances_page::Instances,
    /// Home's Worlds section, of the selected instance (`worlds_page.rs`).
    worlds: worlds_page::Worlds,
    multiplayer: multiplayer_page::Multiplayer,
    screenshots: screenshots_page::Screenshots,
    shot_blur: glass_shot_blur::ShotBlur,
    /// Pictures from disk made at their drawn size (`picture_cache`).
    pictures: picture_cache::Pictures,
    game_packs: packs_page::Packs,
    world_packs: datapacks_page::WorldPacks,
    export: Option<export_page::ExportPanel>,
    launch: Option<launch_page::LaunchPanel>,
    /// The memory row on Home.
    memory_slider: launch_page::MemorySlider,
    importing: import_page::Importing,
    console: console_page::Console,
    /// What the window did when the game started (`while_playing.rs`).
    aside: while_playing::Aside,
    /// The selected instance's desktop shortcut (`shortcut_page.rs`).
    shortcuts: shortcut_page::Shortcuts,
    /// The name the game made up for an offline player (`eclipse-player.txt`), for the top bar.
    offline_name: Option<String>,
    /// The player's servers, for the sidebar until the Servers page has read them.
    side_servers: Vec<eclipse_launcher::servers::Entry>,
    /// The pixel sunset of the new design: its own view, so a frame of it
    /// alone redraws nothing else.
    scene: gpui::Entity<scene::Scene>,
    /// The window's scale, as of its last frame, for art drawn outside `render`'s reach.
    scale: f32,
}

impl Launcher {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (tx, rx) = channel();
        let appearance = cx.observe_window_appearance(window, |this, window, cx| {
            this.theme = this.settings_theme(window);
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
            .selected
            .as_ref()
            .is_none_or(|s| !installed.contains(s))
        {
            settings.selected = installed.first().cloned();
        }
        // Pin the window's chrome (macOS title bar) to the choice before the theme reads
        // `window.appearance()`, so both draw the same look.
        titlebar::apply_appearance(window, settings.appearance);
        glass_ui::set_menu_focus(cx.focus_handle());
        let mut this = Self {
            theme: Theme::for_appearance(settings.appearance, settings.accent, window.appearance()),
            page: Page::from_env(),
            switches: 0,
            content_scroll: Scrollbar::default(),
            settings_pane: gpui::ScrollHandle::new(),
            settings_marked: 0,
            mods_sticky: sticky::Sticky::default(),
            long_list: cull::Cull::default(),
            updates: Updates::default(),
            checked: None,
            update_error: None,
            // The installer the last Update started could not put the new launcher in place.
            self_update: eclipse_launcher::update_handoff::take_failure()
                .map_or(SelfUpdate::Idle, |e| SelfUpdate::Stuck(Some(e))),
            self_update_hidden: false,
            update_install: None,
            instance_install: None,
            install_next: None,
            bar: (0., 0.),
            releases: Load::Loading,
            installed,
            installing: None,
            removing: Vec::new(),
            disk: Vec::new(),
            disk_for: Vec::new(),
            install_error: None,
            importing_build: false,
            import_error: None,
            native_step: None,
            native_started: None,
            native_done: None,
            native_error: None,
            native_confirm: None,
            native_for: None,
            native_update: None,
            native_on_base: false,
            native_builds: Default::default(),
            native_mismatch: None,
            native_log: Default::default(),
            confirm_remove: None,
            picker_open: false,
            accent_strip: false,
            hue_area: std::rc::Rc::new(std::cell::Cell::new(None)),
            hue_dragging: false,
            language_menu: false,
            account_menu: false,
            row_menu: Default::default(),
            menu_opener: None,
            menu_closed_at: None,
            skin: None,
            // The first draw takes the launcher theme from Settings.
            restyle: true,
            background_error: None,
            location_error: location_page::startup_error(),
            location_note: location_page::startup_note(),
            args_input: line_edit::LineEdit::new(settings.game_args.clone(), 1024, 38),
            args_focused: false,
            args_focus: cx.focus_handle(),
            proxy_input: line_edit::LineEdit::new(
                settings.proxy.clone().unwrap_or_default(),
                256,
                44,
            ),
            proxy_focused: false,
            proxy_focus: cx.focus_handle(),
            settings,
            settings_save_error: None,
            links: backend::Links::default(),
            preparing: false,
            prepare_log: VecDeque::new(),
            prepare_error: None,
            client: None,
            client_error: None,
            client_started: None,
            client_home: None,
            crash: None,
            servers: servers_page::Servers::new(),
            internet: internet::Internet::new(),
            root_focus: cx.focus_handle(),
            palette: None,
            toast_seen: None,
            accounts: Accounts::load(),
            auth_status: HashMap::new(),
            auth_progress: HashMap::new(),
            profile_heads: profile_heads::Heads::default(),
            auth_checked: None,
            sign_in_target: None,
            auth_busy: HashMap::new(),
            auth_next: 0,
            sign_in: None,
            sign_in_state: None,
            keychain: None,
            owner: None,
            owner_screen_open: false,
            dev_clicks: 0,
            dev_options: false,
            starting_client: None,
            play_when_prepared: None,
            friend_code: line_edit::LineEdit::new(String::new(), 256, 32),
            friend_focused: false,
            window_width: 0.,
            window_long_side: 0.,
            scale_factor: 1.,
            friend_focus: cx.focus_handle(),
            joining: None,
            quick_play: None,
            client_run: None,
            client_vars: Vec::new(),
            collection: collection::Collection::load(),
            agents: agents_page::AgentsUi::default(),
            tx,
            rx,
            _appearance: appearance,
            _activation: cx.observe_window_activation(window, |_, _, cx| cx.notify()),
            perf: perf::Perf::new("session"),
            install_started: None,
            prepare_started: None,
            download_seen: None,
            home_tab: home_tabs::HomeTab::from_env(),
            home_menu: Default::default(),
            list_hover: None,
            list_menu: None,
            client_since: None,
            instance_mods: Default::default(),
            project_icons: Default::default(),
            play_pressed: None,
            bench: perf::Bench::from_env(),
            packs: Vec::new(),
            registry_packs: Vec::new(),
            packs_loaded: None,
            pack_open: None,
            pack_filter: None,
            pack_busy: false,
            pack_log: VecDeque::new(),
            pack_error: None,
            starting_pack: None,
            mods: mods_page::Mods::new(cx),
            github: github_mods_page::Github::new(cx),
            instances: instances_page::Instances::default(),
            worlds: worlds_page::Worlds::default(),
            multiplayer: multiplayer_page::Multiplayer::default(),
            screenshots: screenshots_page::Screenshots::default(),
            shot_blur: Default::default(),
            pictures: Default::default(),
            game_packs: packs_page::Packs::default(),
            world_packs: datapacks_page::WorldPacks::default(),
            export: None,
            launch: None,
            memory_slider: Default::default(),
            importing: Default::default(),
            console: Default::default(),
            aside: while_playing::Aside::new(window.window_handle()),
            shortcuts: Default::default(),
            offline_name: offline_name(),
            side_servers: eclipse_launcher::servers::list(),
            scene: cx.new(|_| scene::Scene::new()),
            scale: window.scale_factor(),
        };
        if this.location_error.is_some() || this.location_note.is_some() {
            this.page = Page::Settings;
        }
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
            this.preparing = true;
            this.play_when_prepared = this.selected().cloned();
        }
        // `ECLIPSE_LAUNCHER_PAGE=new-instance`: the New instance page on Modded, for screenshots
        // (`new-instance-eclipse`, `new-instance-vanilla`: on those).
        let kind = match std::env::var("ECLIPSE_LAUNCHER_PAGE").as_deref() {
            Ok("new-instance") => Some(new_instance::NewKind::Modded),
            Ok("new-instance-eclipse") => Some(new_instance::NewKind::Eclipse),
            Ok("new-instance-vanilla") => Some(new_instance::NewKind::Vanilla),
            _ => None,
        };
        if let Some(kind) = kind {
            this.open_instance_page(None);
            this.pick_kind(kind, cx);
        }
        if std::env::var("ECLIPSE_LAUNCHER_PAGE").as_deref() == Ok("edit-instance") {
            this.open_instance_page(this.selected_instance().cloned());
        }
        // What the game and the launcher kept, if the player allows it and there is a server.
        telemetry::send_in_background();
        this
    }

    /// Keeps the current choices in memory if writing fails; every page offers the same retry.
    fn save_settings(&mut self) -> bool {
        self.settings_save_error = self.settings.save().err();
        self.settings_save_error.is_none()
    }

    fn settings_save_notice(&self, cx: &mut Context<Self>) -> Option<Div> {
        use eclipse_theme::glass::{ink, radius, step};
        let error = self.settings_save_error.as_ref()?;
        Some(
            div()
                .flex_none()
                .px(px(theme::PAGE_GUTTER))
                .py(px(12.))
                .child(
                    glass::plate(radius::PANEL)
                        .flex()
                        .items_center()
                        .gap(px(16.))
                        .p(px(glass_ui::PANEL_INSET))
                        .child(
                            div()
                                .relative()
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                .flex_1()
                                .min_w_0()
                                .child(error_note(tr("Settings were not saved.")))
                                .child(
                                    glass_ui::text(step::NOTE, FontWeight::NORMAL, ink::BODY)
                                        .child(trf(
                                            "Your changes are still in this window. Retry to save them. {error}",
                                            &[("error", error)],
                                        )),
                                ),
                        )
                        .child(
                            button("settings-save-retry", "Retry", Kind::Secondary)
                                .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.save_settings();
                                    cx.notify();
                                })),
                        ),
                ),
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
            .auth_checked
            .is_none_or(|t| t.elapsed() >= Duration::from_secs(60))
            || (self.accounts.accounts.iter()).any(|a| !self.auth_status.contains_key(&a.uuid))
        {
            self.load_auth_statuses(false);
        }
        self.load_profile_heads();
        if page != Page::InstanceEditor && self.instances.draft.as_ref().is_some_and(|d| !d.busy) {
            self.instances.draft = None;
        }
        if page != self.page {
            self.page = page;
            self.switches += 1;
            self.content_scroll = Scrollbar::default();
            self.long_list = cull::Cull::default();
            self.mods_sticky.reset();
            if page == Page::Mods {
                self.mods.reset_lists();
            }
        }
        self.picker_open = false;
        self.language_menu = false;
        self.mods.browse.sort_menu = false;
        self.account_menu = false;
    }

    fn check_updates(&mut self) {
        self.checked = Some(Instant::now());
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
        if self.installing.is_some() || self.removing.contains(&release.tag) {
            return;
        }
        let select =
            backend::select_after_update(self.settings.selected.as_deref(), &self.installed);
        self.update_install = Some((release.tag.clone(), select));
        self.install(&release);
    }

    /// The native mods turned on in the instance Play starts, unpacked and checked, with their
    /// bundles: what the warning names and the build compiles.
    fn native_mods_to_build(
        &self,
    ) -> Result<(std::path::PathBuf, Vec<native_mods::Bundle>), String> {
        let folder = native_mods::installed_mods(&instances::selected().home)?;
        let found = native_mods::bundles(&folder)?;
        Ok((folder, found))
    }

    /// What installed native build `version` was made from (`native_status::Build`), read once.
    fn native_build_of(&self, version: &str) -> Option<native_status::Build> {
        if !native_mods::is_native(version) {
            return None;
        }
        self.native_builds
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
        if self.native_step.is_some() {
            return;
        }
        self.native_confirm = None;
        self.native_error = None;
        self.native_mismatch = None;
        // This attempt's own log, starting empty: an open panel shows it rather than the last
        // attempt's (`native_log_page`).
        self.native_log_reset();
        let instance = instances::selected();
        self.native_for = Some(instance.id.clone());
        let plays = self.version_of(&instance).cloned();
        let on_base = std::mem::take(&mut self.native_on_base);
        let base = match self.native_update.take() {
            Some(newer) => Some(newer),
            None => native_mods::base_for(plays.as_deref(), &self.installed, false),
        };
        let Some(base) = base else {
            let error = tr("Install Eclipse first.").to_string();
            self.native_log_preflight(&error);
            self.native_error = Some(error);
            return;
        };
        let folder = match self.native_mods_to_build() {
            Ok((folder, _)) => folder,
            Err(e) => {
                self.native_log_preflight(&e);
                self.native_error = Some(e);
                return;
            }
        };
        self.native_step = Some(tr("Starting the build").into());
        self.native_started = Some((Instant::now(), 0));
        self.native_done = None;
        let profile = native_mods::Profile::chosen(&self.settings);
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let result = native_mods::build(
                &base,
                &folder,
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
        self.releases = Load::Loading;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Msg::Releases(backend::releases()));
        });
    }

    fn fetch_startup_releases(&mut self) {
        self.checked = Some(Instant::now());
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

    fn drain(&mut self, cx: &mut Context<Self>) {
        let mut changed = false;
        self.perf.tick();
        match self.bench.as_mut().map(|b| b.tick(&self.perf)) {
            Some(perf::Step::Go(page)) => {
                let pages: Vec<Page> = Page::ALL.iter().map(|p| p.0).collect();
                self.go(pages[page % pages.len()]);
                changed = true;
            }
            Some(perf::Step::Scroll) => {
                self.go(Page::Versions);
                changed = true;
            }
            Some(perf::Step::Quit) => cx.quit(),
            Some(perf::Step::Wait) | None => {}
        }
        // The running build's clock moves on each second.
        if let Some((started, shown)) = &mut self.native_started {
            if started.elapsed().as_secs() != *shown {
                *shown = started.elapsed().as_secs();
                changed = true;
            }
        }
        while let Ok(msg) = self.rx.try_recv() {
            changed = true;
            match msg {
                Msg::Releases(r) => {
                    if let (Ok(_), Some(checked)) = (&r, self.checked) {
                        self.perf.record_since("launcher.release_list_ms", checked);
                    }
                    self.releases = r.map_or_else(Load::Failed, Load::Ready)
                }
                Msg::Links(links) => {
                    self.multiplayer.stale();
                    self.links = links;
                }
                // The page reads is_prepared when drawn; `changed` redraws it.
                Msg::Reused(_) => self.installed = backend::installed(),
                Msg::Keychain(said) => self.keychain = said,
                Msg::Owner(owns) => self.owner = Some(owns),
                Msg::AccountHead(uuid, url, face, failed) => {
                    self.profile_heads.loaded(&uuid, &url, face, failed)
                }
                Msg::AccountProgress(uuid, generation, phase) => {
                    if self.auth_busy.get(&uuid) == Some(&generation) {
                        self.auth_progress.insert(uuid, phase);
                    }
                }
                Msg::AccountStatus(uuid, generation, status) => {
                    if self.auth_busy.get(&uuid) != Some(&generation) {
                        continue;
                    }
                    self.auth_busy.remove(&uuid);
                    self.auth_progress.remove(&uuid);
                    if self.accounts.find(&uuid).is_some() {
                        if status.state == account::session::State::Ready {
                            self.owner = Some(true);
                        }
                        self.auth_status.insert(uuid, status);
                        // A refresh may have saved a new name or skin: show it now.
                        let saved = Accounts::load();
                        if saved != self.accounts {
                            self.accounts = saved;
                            self.load_profile_heads();
                        }
                    }
                }
                Msg::AccountRemoved(uuid, generation, result) => {
                    if self.auth_busy.get(&uuid) != Some(&generation) {
                        continue;
                    }
                    self.auth_busy.remove(&uuid);
                    self.auth_progress.remove(&uuid);
                    match result {
                        Ok(owns) => {
                            self.accounts = Accounts::load();
                            self.auth_status.remove(&uuid);
                            self.owner = Some(owns);
                        }
                        Err(e) => self.sign_in_state = Some(SignInState::Done(Err(e))),
                    }
                }
                Msg::JavaGame(id, home, cmd) => {
                    self.starting_client = None;
                    let spawned = cmd.and_then(|(cmd, vars)| {
                        self.client_vars = vars;
                        backend::spawn(Job::Client, cmd, self.tx.clone())
                    });
                    match spawned {
                        Ok(r) => {
                            self.client = Some(r);
                            self.client_home = Some(home);
                            self.client_run = Some((id, Instant::now()));
                            self.client_error = account::take_unsaved();
                            self.step_aside(cx);
                        }
                        Err(e) => {
                            if e == account::NoOwner.to_string() {
                                self.owner = Some(false);
                                self.owner_screen_open = true;
                                self.client_error = Some(account::NoOwner::in_window());
                            } else {
                                self.client_error = Some(e);
                            }
                        }
                    }
                }
                Msg::Session(session) => {
                    self.load_auth_statuses(false);
                    if let Some(tag) = self.starting_client.take() {
                        let mut cmd = backend::client_command(&tag);
                        let account::GameSession { file, offline, env } = match session {
                            Ok(pass) => pass,
                            Err(account::NoOwner) => {
                                self.owner = Some(false);
                                self.owner_screen_open = true;
                                self.client_error = Some(account::NoOwner::in_window());
                                self.starting_pack = None;
                                self.joining = None;
                                self.quick_play = None;
                                continue;
                            }
                        };
                        // Past the gate by the developer option alone is no owner.
                        if !self.settings.dev_offline {
                            self.owner = Some(true);
                        }
                        if let Some(file) = file {
                            cmd.arg("--session-file").arg(file);
                        }
                        cmd.envs(env);
                        // Still play, offline, and say why.
                        if let Some(e) = offline {
                            self.client_error = Some(format!("Playing offline: {e}"));
                        } else if let Some(e) = account::take_unsaved() {
                            self.client_error = Some(e);
                        }
                        self.accounts = Accounts::load();
                        // An offline name the player picked (the quick switch).
                        if let Some(name) = self.accounts.name_arg() {
                            cmd.arg("--name").arg(name);
                        }
                        cmd.envs(self.accounts.look_env());
                        // A pack's Play button plays that pack; Play plays the selected instance.
                        let (profile, run) = match self.starting_pack.take() {
                            Some(work) => {
                                let id = instances::list()
                                    .into_iter()
                                    .find(|i| i.work == work)
                                    .map(|i| i.id);
                                let applied =
                                    eclipse_launcher::modpacks::apply_profile(&mut cmd, &work);
                                (applied, id)
                            }
                            None => {
                                let instance = instances::selected();
                                (instances::apply(&mut cmd, &instance), Some(instance.id))
                            }
                        };
                        if let Err(e) = profile {
                            self.client_error = Some(e);
                            account::discard_unread_sessions();
                            continue;
                        }
                        if let Some(code) = self.joining.take() {
                            if let Err(e) = backend::join_friend(&mut cmd, &code) {
                                self.client_error = Some(e);
                            }
                        }
                        if let Some((flag, value)) = self.quick_play.take() {
                            cmd.arg(flag).arg(value);
                        }
                        backend::append_game_args(&mut cmd);
                        // Last, around every argument: the instance's pre-launch command,
                        // wrapper and variables.
                        let instance = run
                            .as_ref()
                            .and_then(|id| instances::list().into_iter().find(|i| &i.id == id));
                        self.client_vars = instance
                            .as_ref()
                            .map(|i| i.vars(cmd.get_program()))
                            .unwrap_or_default();
                        cmd.envs(self.client_vars.iter().map(|(k, v)| (k, v)));
                        let launch = instance.map(|i| i.launch).unwrap_or_default();
                        let cmd = eclipse_launcher::launch::wrap(cmd, &launch);
                        self.client_home = cmd.get_current_dir().map(|d| d.to_path_buf());
                        match backend::spawn(Job::Client, cmd, self.tx.clone()) {
                            Ok(r) => {
                                self.client = Some(r);
                                self.client_run = run.map(|id| (id, Instant::now()));
                                self.step_aside(cx);
                            }
                            Err(e) => {
                                self.client_error = Some(e);
                                account::discard_unread_sessions();
                            }
                        }
                    } else {
                        // The start was called off: the sessions written for it go.
                        account::discard_unread_sessions();
                    }
                }
                Msg::Progress(tag, p) => {
                    self.bar = (self.bar.1, p);
                    self.installing = Some((tag, p));
                }
                Msg::Installed(tag, result) => {
                    self.installing = None;
                    self.bar = (0., 0.);
                    self.installed = backend::installed();
                    let as_update = self.update_install.take().filter(|(t, _)| *t == tag);
                    let for_instance = self.instance_install.take_if(|t| *t == tag).is_some();
                    if let (Ok(()), Some(started)) = (&result, self.install_started.take()) {
                        let name = if as_update.is_some() {
                            "launcher.update_ms"
                        } else {
                            "launcher.install_ms"
                        };
                        self.perf.record_since(name, started);
                    }
                    match result {
                        Ok(()) => {
                            if self.updates.eclipse.as_ref().is_some_and(|r| r.tag == tag) {
                                self.updates.eclipse = None;
                            }
                            // An instance's own version leaves the other instances on theirs.
                            let select = as_update.is_none_or(|(_, select)| select)
                                && (!for_instance || self.launcher_version().is_none());
                            if select {
                                self.settings.selected = Some(tag);
                            }
                            // Pruning reads settings from disk: protect the selection only once
                            // it has been saved there. A failed save leaves all versions in place.
                            if self.save_settings() {
                                let tx = self.tx.clone();
                                std::thread::spawn(move || {
                                    let _ = tx.send(Msg::Pruned(backend::prune_old_versions()));
                                });
                            }
                        }
                        Err(e) => self.install_error = Some((tag, e)),
                    }
                    if let Some(next) = self.install_next.take() {
                        self.install_for_instance(next);
                    }
                }
                Msg::Removed(tag, result) => {
                    self.removing.retain(|t| *t != tag);
                    self.installed = backend::installed();
                    let selected = if self.settings_save_error.is_some() {
                        self.settings.selected.take()
                    } else {
                        Settings::load().selected
                    };
                    self.settings.selected = selection_after_removal(&self.installed, selected);
                    if let Err(e) = result {
                        self.install_error = Some((tag, e));
                    }
                }
                Msg::DiskUse(disk) => self.disk = disk,
                Msg::Imported(result) => {
                    self.importing_build = false;
                    self.installed = backend::installed();
                    match result {
                        // `build_import::import` saved the selection; this keeps the window's copy.
                        Ok(tag) => self.settings.selected = Some(tag),
                        Err(e) => self.import_error = Some(e),
                    }
                }
                Msg::Pruned(gone) => {
                    if !gone.is_empty() {
                        self.installed = backend::installed();
                    }
                }
                Msg::Updates(Ok(updates)) => {
                    self.update_error = None;
                    self.updates = *updates;
                    if self.settings.auto_install() {
                        self.install_update();
                    }
                    if let Some(release) = self.updates.launcher.clone() {
                        self.update_launcher(release);
                    }
                }
                Msg::Updates(Err(e)) => self.update_error = Some(e),
                Msg::NativeProgress(step) => self.native_step = Some(step),
                Msg::Native(result) => {
                    // The last step of a build that worked says how it went.
                    let last = self.native_step.take();
                    self.native_started = None;
                    self.native_done = result.is_ok().then_some(last).flatten();
                    self.installed = backend::installed();
                    self.reload_instances();
                    // A build attempt ended: an open panel reads the log it left.
                    self.native_log.stale = true;
                    match result.map_err(|e| match e.strip_prefix(native_mods::MISMATCH) {
                        Some(list) => Ok(list.to_owned()),
                        None => Err(e),
                    }) {
                        Err(Ok(list)) => self.native_mismatch = Some(list),
                        Err(Err(e)) => self.native_error = Some(e),
                        // Play waited for this build (`Mods::rebuild`).
                        Ok(_) if std::mem::take(&mut self.mods.play_after_build) => self.play(),
                        Ok(_) => {}
                    }
                    self.mods.play_after_build = false;
                }
                Msg::LauncherProgress(p) => {
                    if let SelfUpdate::Downloading(_, f) = &mut self.self_update {
                        *f = p;
                    }
                }
                Msg::LauncherUpdated(result) => {
                    let version = match &self.self_update {
                        SelfUpdate::Downloading(v, _) => v.clone(),
                        _ => String::new(),
                    };
                    self.self_update = match result {
                        Ok(staged) => {
                            eclipse_launcher::update_handoff::set_pending(staged);
                            SelfUpdate::Ready(version)
                        }
                        Err(e) => SelfUpdate::Failed(e),
                    };
                }
                Msg::PackLine(line) => {
                    self.pack_log.push_back(line);
                    if self.pack_log.len() > LOG_LINES {
                        self.pack_log.pop_front();
                    }
                }
                Msg::PackDone(result) => self.pack_done(result),
                // Old versions log their session; it never reaches the window.
                Msg::Line(_, line) if line.contains("Session ID is") => {}
                Msg::Line(job, line) => {
                    if job == Job::Client && line.starts_with(perf::GAME_FIRST_FRAME) {
                        if let Some(pressed) = self.play_pressed.take() {
                            self.perf
                                .record_since("launcher.play_to_game_frame_ms", pressed);
                        }
                    }
                    // The MCP server keeps its own server's log; the launcher's are the preparer's
                    // and the game's.
                    if job != Job::Server {
                        self.prepare_log.push_back(line);
                        if self.prepare_log.len() > LOG_LINES {
                            self.prepare_log.pop_front();
                        }
                    }
                }
                Msg::Exit(job, result) => {
                    let error = result.err();
                    match job {
                        Job::Prepare => {
                            self.preparing = false;
                            // Play pressed before the version was prepared plays now.
                            if let Some(tag) = self.play_when_prepared.take() {
                                if error.is_none() && backend::is_prepared(&tag) {
                                    self.starting_pack = None;
                                    self.start_client(tag);
                                }
                            }
                            if let (None, Some(started)) = (&error, self.prepare_started.take()) {
                                self.perf.record_since("launcher.prepare_ms", started);
                            }
                            // A preparer process's exit status says little, so its last line
                            // follows; the launcher's own errors are full sentences already.
                            self.prepare_error = error.map(|e| {
                                let tail = self
                                    .prepare_log
                                    .iter()
                                    .rev()
                                    .find(|l| !l.trim().is_empty())
                                    .filter(|_| e.starts_with("Exited with"));
                                tail.map_or(e.clone(), |t| format!("{e}: {t}"))
                            });
                        }
                        Job::Client => {
                            self.client = None;
                            // Any session file the game didn't read (it crashed first).
                            account::discard_unread_sessions();
                            self.come_back(cx);
                            // The game saved: last played, sizes and pictures changed.
                            self.worlds.stale();
                            self.multiplayer.stale();
                            self.screenshots.stale();
                            self.game_packs.stale();
                            let vars = std::mem::take(&mut self.client_vars);
                            if let Some((id, started)) = self.client_run.take() {
                                if let Err(e) = instances::record_play(&id, started.elapsed()) {
                                    eprintln!("Play time not saved: {e}");
                                }
                                if let Some(i) = instances::list().into_iter().find(|i| i.id == id)
                                {
                                    eclipse_launcher::launch::run_after_exit(
                                        &i.launch, &i.home, &vars,
                                    );
                                }
                                self.reload_instances();
                            }
                            // The run's own game folder: an instance's or a pack's keeps its reports.
                            let home = self.client_home.clone().unwrap_or_else(backend::game_home);
                            let report = self
                                .client_started
                                .take()
                                .and_then(|since| backend::crash_report_since(&home, since));
                            let switched = if error.is_some() && report.is_none() {
                                self.switched_from_vulkan()
                            } else {
                                None
                            };
                            if let Some(api) = switched {
                                self.client_error = Some(trf(
                                    "Eclipse's Vulkan renderer crashed while starting on this \
                                     graphics card, so the game now uses {api}. Press Play to \
                                     start it again. Video Settings > Graphics API switches back.",
                                    &[("api", api)],
                                ));
                            } else if error.is_some() || report.is_some() {
                                let summary = report
                                    .as_ref()
                                    .and_then(|p| std::fs::read_to_string(p).ok())
                                    .and_then(|text| backend::crash_summary(&text))
                                    .or_else(|| {
                                        let tail = self
                                            .prepare_log
                                            .iter()
                                            .rev()
                                            .find(|l| !l.trim().is_empty());
                                        error.as_ref().map(|e| {
                                            tail.map_or(e.clone(), |t| format!("{e}: {t}"))
                                        })
                                    })
                                    .unwrap_or_default();
                                self.crash = Some(Crash {
                                    report,
                                    summary,
                                    copied: false,
                                });
                            }
                        }
                        Job::Server => {}
                    }
                }
            }
        }
        changed |= self.servers_tick();
        changed |= self.mods_tick();
        changed |= self.github_tick();
        changed |= self.native_log_read(cx);
        changed |= self.toast_tick();
        if let Some(sign_in) = &self.sign_in {
            if let Some(state) = sign_in_change(&mut self.sign_in_state, sign_in.state()) {
                if let SignInState::Done(Ok(ref account)) = state {
                    self.auth_busy.remove(&account.uuid);
                    self.sign_in_target = None;
                    self.accounts = Accounts::load();
                    self.load_profile_heads();
                    self.load_auth_statuses(false);
                    // Sign-in refuses accounts that don't own the game.
                    self.owner = Some(true);
                    self.owner_screen_open = false;
                    self.client_error = None;
                    if !self.settings.owner_notice_seen {
                        self.settings.owner_notice_seen = true;
                        self.save_settings();
                    }
                }
                changed = true;
            }
        }
        if self
            .checked
            .is_some_and(|at| at.elapsed() > backend::CHECK_EVERY)
        {
            self.check_updates();
        }
        if changed {
            cx.notify();
        }
    }

    /// After a crash with no report: when the game's output ends while its Vulkan renderer
    /// started, a native crash in the graphics driver (#746), switches the game to
    /// `backend::vulkan_crash_fallback` (DirectX 12 on Windows, OpenGL on Linux; only where the
    /// player left the graphics API on Auto). The API's label when it did.
    fn switched_from_vulkan(&self) -> Option<&'static str> {
        let (api, label) = backend::vulkan_crash_fallback(std::env::consts::OS)?;
        if !backend::vulkan_start_crashed(&self.prepare_log) {
            return None;
        }
        // The GPU is the same for every profile: the game home's setting and the crashed run's.
        let home = backend::game_home();
        let mut homes = vec![home.clone()];
        homes.extend(self.client_home.clone().filter(|h| *h != home));
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
    fn selected(&self) -> Option<&String> {
        self.selected_instance()
            .and_then(|i| self.version_of(i))
            .or_else(|| self.launcher_version())
    }

    /// The launcher's version: what instances without one of their own play.
    fn launcher_version(&self) -> Option<&String> {
        self.settings
            .selected
            .as_ref()
            .filter(|s| self.installed.contains(s))
    }

    /// Installs `release` for an instance made or edited to play it, after the install running
    /// now if there is one.
    fn install_for_instance(&mut self, release: Release) {
        match &self.installing {
            Some((now, _)) if *now == release.tag => {}
            Some(_) => self.install_next = Some(release),
            None => {
                self.instance_install = Some(release.tag.clone());
                self.install(&release);
            }
        }
    }

    fn install(&mut self, release: &Release) {
        if release.asset.is_none() || self.removing.contains(&release.tag) {
            return;
        }
        let (release, tx) = (release.clone(), self.tx.clone());
        let tag = release.tag.clone();
        self.installing = Some((tag.clone(), 0.0));
        self.install_started = Some(Instant::now());
        self.bar = (0., 0.);
        self.install_error = None;
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
        self.confirm_remove = None;
        let installing = self.installing.as_ref().map(|(t, _)| t.as_str());
        if !removable(&self.installed, installing, &self.removing, tag) {
            return;
        }
        self.install_error = None;
        self.removing.push(tag.to_string());
        self.installed.retain(|t| t != tag);
        self.settings.selected =
            selection_after_removal(&self.installed, self.settings.selected.take());
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
        !self.importing_build && self.removing.is_empty()
    }
    /// Starts importing the build at `path` on a thread, as the picker does.
    /// Separate from [`Self::import_build`] so a file dropped on the window can
    /// start the same import (`import_dropped`) without a picker. Does nothing
    /// when one is already running. `Msg::Imported` ends it.
    fn start_build_import(&mut self, path: std::path::PathBuf, cx: &mut Context<Self>) {
        if !self.can_import_build() {
            return;
        }
        self.importing_build = true;
        self.import_error = None;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Msg::Imported(build_import::import(&path, None)));
        });
        cx.notify();
    }

    /// Prepares the selected version, its Minecraft files from the player's install or, for an
    /// owner who asks, from Mojang.
    fn prepare(&mut self, from: backend::MinecraftFrom) {
        let Some(tag) = self.selected().cloned() else {
            return;
        };
        eclipse_launcher::mojang::CANCEL.store(false, Ordering::Relaxed);
        let minecraft = self
            .settings
            .minecraft_dir
            .clone()
            .unwrap_or_else(|| eclipse_prepare::artifacts::minecraft_dir(None));
        self.prepare_log.clear();
        self.prepare_error = None;
        self.prepare_log
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
        self.preparing = true;
        self.prepare_started = Some(Instant::now());
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
            let server = self
                .quick_play
                .take()
                .filter(|(flag, _)| *flag == "--server")
                .map(|(_, ip)| ip);
            if self.client.is_some() || self.starting_client.is_some() {
                return;
            }
            self.starting_pack = None;
            self.start_java(instance, server);
            return;
        }
        let Some(tag) = self.selected().cloned() else {
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
        if let Some(id) = rebuild.filter(|_| self.native_step.is_none()) {
            self.mods.rebuild.retain(|r| *r != id);
            if native_mods::is_native(&tag) {
                self.mods.play_after_build = true;
                self.build_native(false);
                return;
            }
        }
        self.starting_pack = None;
        self.start_client(tag);
    }

    /// Starts a Java instance's Minecraft, joining `server` if given: its files and Java first
    /// (downloaded the first time, their lines in the log), then `drain` starts the game when
    /// the command arrives.
    fn start_java(&mut self, instance: instances::Instance, server: Option<String>) {
        self.client_error = None;
        self.crash = None;
        self.prepare_log.clear();
        self.client_started =
            Some(std::time::SystemTime::now() - std::time::Duration::from_secs(2));
        self.starting_client = Some(instance.id.clone());
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let log = |line: String| {
                let _ = tx.send(Msg::Line(Job::Client, line));
            };
            let cmd = backend::java_game(&instance, server.as_deref(), &log).map(|mut cmd| {
                let vars = instance.vars(cmd.get_program());
                cmd.envs(vars.iter().map(|(k, v)| (k, v)));
                (eclipse_launcher::launch::wrap(cmd, &instance.launch), vars)
            });
            let _ = tx.send(Msg::JavaGame(instance.id, instance.home, cmd));
        });
    }

    /// Whether `tag` can start now ([`starts_now`]): prepared, no preparation under way, no game
    /// running or starting.
    pub(crate) fn can_start(&self, tag: &str) -> bool {
        let running = self.client.is_some() || self.starting_client.is_some();
        starts_now(backend::is_prepared(tag), self.preparing, running)
    }

    /// Whether the selected instance's version can start now, from a shortcut (a server's Join,
    /// a world's Play, a friend's code): Eclipse's only, never a Java instance.
    pub(crate) fn can_start_selected(&self) -> bool {
        self.selected_instance().is_none_or(|i| i.java.is_none())
            && self.selected().is_some_and(|tag| self.can_start(tag))
    }

    /// Whether a server's Join starts the selected instance now: a Java instance too, as Play
    /// would (no game running or starting).
    pub(crate) fn can_join_selected(&self) -> bool {
        match self.selected_instance() {
            Some(i) if i.java.is_some() => self.client.is_none() && self.starting_client.is_none(),
            _ => self.can_start_selected(),
        }
    }

    /// Starts the game: the session first (a network round trip when the token needs
    /// renewing), then `drain` starts the process when it arrives.
    fn start_client(&mut self, tag: String) {
        self.client_error = None;
        self.crash = None;
        self.prepare_log.clear();
        // A little earlier than the spawn: file times can be coarse.
        self.client_started =
            Some(std::time::SystemTime::now() - std::time::Duration::from_secs(2));
        // What the resources lack (built-in packs, panorama, sounds) and the session (a network
        // round trip when the token needs renewing) come first; `drain` starts the game when the
        // session arrives.
        self.starting_client = Some(tag.clone());
        self.play_pressed = Some(Instant::now());
        let tx = self.tx.clone();
        let offline_without_owner = self.settings.dev_offline;
        // Registry mods update before the game reads them (`eclipse::auto`); native mods only
        // at start, where a build can follow.
        let auto = self
            .selected_instance()
            .filter(|_| self.settings.auto_updates_mods() && self.starting_pack.is_none())
            .map(|i| mods_page::AutoTarget {
                instance: i.clone(),
                level: Some(backend::mod_api_level(&tag)),
                natives: false,
            });
        let mods_tx = self.mods.tx.clone();
        std::thread::spawn(move || {
            backend::complete_assets(&tag);
            if let Some(auto) = auto {
                let (lines, _) = mods_page::auto_update(&[auto]);
                if !lines.is_empty() {
                    let _ = mods_tx.send(mods_page::Event::AutoUpdated(lines, Vec::new()));
                }
            }
            let session = account::session_for_game(&backend::game_home(), offline_without_owner);
            let _ = tx.send(Msg::Session(session));
        });
    }

    fn load_profile_heads(&mut self) {
        let resources = self
            .selected()
            .or_else(|| self.installed.first())
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
        if self.auth_busy.contains_key(uuid) {
            return None;
        }
        self.auth_next += 1;
        self.auth_busy.insert(uuid.into(), self.auth_next);
        self.auth_progress
            .insert(uuid.into(), account::session::Phase::Queued);
        Some(self.auth_next)
    }

    /// Only workers read the keychain or contact authentication services. Cached public status
    /// keeps drawing independent of keychain prompts, network timeouts and refresh operations.
    fn load_auth_statuses(&mut self, renew_due: bool) {
        self.auth_checked = Some(Instant::now());
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
        self.sign_in = Some(match &target {
            Some(account) => SignIn::start_for(account.clone()),
            None => SignIn::start(),
        });
        self.sign_in_target = target;
        self.sign_in_state = Some(SignInState::Starting);
    }

    fn cancel_sign_in(&mut self) {
        if let Some(old) = self.sign_in.take() {
            old.cancel();
        }
        self.sign_in_state = None;
        self.sign_in_target = None;
    }

    fn choose_folder(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Choose".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(mut paths))) = paths.await {
                if let Some(path) = paths.pop() {
                    let _ = this.update(cx, |this, cx| {
                        this.settings.minecraft_dir = Some(path);
                        this.save_settings();
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }
}

// ----- Building blocks ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Primary,
    Secondary,
    Ghost,
}

fn mark(size: f32, scale: f32) -> Div {
    thread_local! {
        static MARKS: std::cell::RefCell<HashMap<(u32, u32), std::sync::Arc<gpui::RenderImage>>> =
            Default::default();
    }
    let image = MARKS.with_borrow_mut(|marks| {
        marks
            .entry((size.to_bits(), scale.to_bits()))
            .or_insert_with(|| art::bake(&eclipse_theme::mark::texels(), 16, 16, size / 16., scale))
            .clone()
    });
    div()
        .flex_none()
        .size(px(size))
        .child(gpui::img(image).size(px(size)))
}

/// A medium button with its label translated (`glass_ui::button`).
fn button(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    kind: Kind,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let label: SharedString = label.into();
    let label = SharedString::from(tr(&label).to_string());
    glass_ui::button(id, None, label, kind, glass_ui::Size::Md)
}

/// Copies to the clipboard, or into `ECLIPSE_LAUNCHER_CLIPBOARD` when a test run sets it
/// (`eclipse_launcher::desktop_seams`).
fn copy_to_clipboard(cx: &mut App, item: gpui::ClipboardItem) {
    match item.text() {
        Some(text) => glass_toast::say(trf("Copied {text}", &[("text", &text)])),
        None => glass_toast::say(tr("Copied the picture")),
    }
    if let Some(file) = eclipse_launcher::desktop_seams::clipboard_file() {
        let bytes = match item.entries().first() {
            Some(gpui::ClipboardEntry::Image(image)) => image.bytes.clone(),
            _ => item.text().unwrap_or_default().into_bytes(),
        };
        if let Err(e) = std::fs::write(&file, bytes) {
            eprintln!("{}: {e}", file.display());
        }
        return;
    }
    cx.write_to_clipboard(item.clone());
    #[cfg(target_os = "linux")]
    if let Some(text) = item.text() {
        sync_system_clipboard(text);
    }
}

/// GPUI's Wayland clipboard sends the last key press's serial, which a compositor ignores
/// after a mouse click: hand the text to the desktop's own tool too, the first one present
/// for this session (`wl-copy` on Wayland, `xclip` or `xsel` on X11).
#[cfg(target_os = "linux")]
fn sync_system_clipboard(text: String) {
    use std::io::Write;
    use std::process::{Command, Stdio};
    const TOOLS: [(&str, &str, &[&str]); 3] = [
        ("WAYLAND_DISPLAY", "wl-copy", &[]),
        ("DISPLAY", "xclip", &["-selection", "clipboard"]),
        ("DISPLAY", "xsel", &["--clipboard", "--input"]),
    ];
    if text.is_empty() {
        return;
    }
    // Off the UI thread: a tool can take its time.
    std::thread::spawn(move || {
        for (session, tool, args) in TOOLS {
            if std::env::var_os(session).is_none() {
                continue;
            }
            let Ok(mut child) = Command::new(tool)
                .args(args)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            else {
                continue;
            };
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            if child.wait().is_ok_and(|status| status.success()) {
                return;
            }
        }
    });
}

/// What the Play button does for the selected version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlayPress {
    Play,
    /// Not prepared, for an account that owns the game: prepare it (from the player's install,
    /// else downloaded from Mojang for that account), then play, as the official launcher does.
    PrepareThenPlay,
    /// Not prepared and no owner (signing in comes first), or busy preparing or playing.
    Nothing,
}

/// Account state uses the same semantic colors as the launcher's other notices. Keep a
/// distinct icon and the full label so color is never the only way to recognize a state.
fn account_status_line(status: Option<&account::session::Status>, progress: Option<&str>) -> Div {
    use account::session::State;
    use eclipse_theme::glass::ink;
    let muted = glass_ui::white(ink::MUTED);
    let (icon, color, label) = if let Some(progress) = progress {
        ("clock", muted, progress)
    } else {
        let (icon, color) = match status.map(|s| s.state) {
            Some(State::Ready) => ("tick-02", glass_ui::live()),
            Some(State::NeedsRefresh | State::Retry) => ("refresh", glass_ui::warn()),
            Some(State::SignInRequired) => ("user", glass_ui::danger()),
            Some(State::OwnershipRequired) => ("alert-circle", glass_ui::danger()),
            None => ("information-circle", muted),
        };
        (
            icon,
            color,
            status.map_or("Sign-in not checked", |s| s.label()),
        )
    };
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .text_color(color)
        .child(glass_ui::icon(icon, glass_ui::IconSize::Px16, color))
        .child(tr(label).to_owned())
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

/// `owner`: whether an account here owns the game, by the keychain (`Launcher::owner`).
fn play_press(prepared: bool, owner: Option<bool>, busy: bool) -> PlayPress {
    match (prepared, owner) {
        _ if busy => PlayPress::Nothing,
        (true, _) => PlayPress::Play,
        (false, Some(true)) => PlayPress::PrepareThenPlay,
        (false, _) => PlayPress::Nothing,
    }
}

/// Whether a shortcut that starts the game at once (a server's Join, a world's Play, a friend's
/// code, a pack's Play) starts it now: only when Play would play. Never while a preparation runs:
/// the game would open on half-written files.
fn starts_now(prepared: bool, preparing: bool, running: bool) -> bool {
    play_press(prepared, None, preparing || running) == PlayPress::Play
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

/// The row menu a press on a sidebar row leaves open: that row's, or none when it was already
/// that row's (the ⋮ and a right click on the row toggle the same way).
fn row_menu_after_press(open: Option<(String, bool)>, id: &str) -> Option<(String, bool)> {
    if open.as_ref().is_some_and(|(current, _)| current == id) {
        return None;
    }
    Some((id.to_owned(), false))
}

fn disabled(b: Stateful<Div>) -> Stateful<Div> {
    b.opacity(theme::DISABLED).cursor_default().tab_stop(false)
}

fn page_header(title: &str, description: &str) -> Div {
    glass_ui::page_head(tr(title), tr(description))
}

/// How an available update is named: a Nightly by its title ("Nightly 2026-10-28 · 0c02067"), a
/// release by its tag.
fn update_name(release: &Release) -> &str {
    match release.nightly {
        Some(_) => &release.name,
        None => &release.tag,
    }
}

/// Microsoft account actions go below the profile in the fixed-width glass card.
fn account_row(slot: Div, name: impl IntoElement, detail: Option<AnyElement>, actions: Div) -> Div {
    div()
        .flex()
        .flex_col()
        .min_w_0()
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(14.))
                .px(px(16.))
                .py(px(12.))
                .child(
                    slot.rounded(px(eclipse_theme::glass::radius::ROW))
                        .overflow_hidden(),
                )
                .child(glass_ui::setting_row(name, detail, None).p_0().flex_1()),
        )
        .child(actions.flex_wrap().px(px(16.)).pb(px(12.)))
}

fn error_card(said: impl Into<SharedString>) -> Div {
    glass::plate(eclipse_theme::glass::radius::PANEL)
        .p(px(glass_ui::PANEL_INSET))
        .child(error_note(said))
}

fn native_panel() -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .mt(px(12.))
        .pt(px(12.))
        .border_t_1()
        .border_color(glass_ui::white(eclipse_theme::glass::EDGE))
}

fn error_note(said: impl Into<SharedString>) -> Div {
    use eclipse_theme::glass::{ink, step};
    div()
        .flex()
        .items_start()
        .gap(px(10.))
        .child(
            div()
                .flex()
                .items_center()
                .h(px(step::BODY.line))
                .child(glass_ui::icon(
                    "alert-circle",
                    glass_ui::IconSize::Px16,
                    glass_ui::danger(),
                )),
        )
        .child(
            glass_ui::text(step::BODY, FontWeight::NORMAL, ink::FULL)
                .flex_1()
                .min_w_0()
                .child(said.into()),
        )
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

// ----- Pages -------------------------------------------------------------------------------------

impl Launcher {
    /// Whether `event` is the press that just closed a menu from outside it (on the menu's
    /// own button): the button then leaves the menu closed instead of opening it again.
    fn closed_by_this_press(&mut self, event: &ClickEvent) -> bool {
        closed_by_press(self.menu_closed_at.take(), event)
    }

    fn menu_open(&self) -> bool {
        self.account_menu
            || self.row_menu.open.is_some()
            || self.home_menu != glass_home::HomeMenu::None
            || self.picker_open
            || self.language_menu
            || self.list_menu.is_some()
            || self.mods.target_menu
            || self.mods.browse.sort_menu
            || self.mod_detail_floats()
    }

    fn mod_detail_floats(&self) -> bool {
        self.page == Page::Mods
            && self.mods.picked.is_some()
            && self.window_width < layout::MODS_DETAIL_BESIDE
    }

    fn close_menus(&mut self) -> bool {
        let open = self.menu_open();
        self.account_menu = false;
        self.row_menu.open = None;
        self.home_menu = glass_home::HomeMenu::None;
        self.picker_open = false;
        self.language_menu = false;
        self.list_menu = None;
        self.mods.target_menu = false;
        self.mods.browse.sort_menu = false;
        if self.mod_detail_floats() {
            self.mods.picked = None;
        }
        open
    }

    fn opened_menu(&mut self, window: &Window, cx: &App) {
        self.menu_opener = window.focused(cx);
        glass_ui::menu_opened();
    }

    fn return_focus_from_menu(&mut self, window: &mut Window, cx: &mut App) {
        if self.menu_open() {
            return;
        }
        let Some(opener) = self.menu_opener.take() else {
            return;
        };
        let lost = window.focused(cx).is_none()
            || glass_ui::menu_focus().is_some_and(|m| m.contains_focused(window, cx));
        if lost {
            window.focus(&opener, cx);
        }
    }

    fn play_page(&mut self, cx: &mut Context<Self>) -> Div {
        // A Java instance plays Mojang's game: no Eclipse version, nothing of Eclipse's to prepare.
        let java = self.selected_instance().and_then(|i| i.java.clone());
        let Some(tag) = self
            .selected()
            .cloned()
            .or_else(|| java.is_some().then(String::new))
        else {
            // No hero without a version: the page starts below the top bar like the others.
            let memory = self.memory_row(cx);
            // The first version, downloading since an instance was made to play it.
            let waiting = match &self.installing {
                Some((_, p)) => format!("{} {:.0}%", tr("Installing"), p * 100.),
                None => tr("No version installed").to_string(),
            };
            let choose = button("go-versions", "Choose a version", Kind::Primary).on_press(
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.go(Page::Versions);
                    cx.notify();
                }),
            );
            return div()
                .flex()
                .flex_col()
                .gap(px(24.))
                .pt(px(28.))
                .child(page_header("Play", ""))
                .child(glass_ui::empty_card(glass_ui::empty_state(
                    "play-no-version",
                    "package-01",
                    waiting,
                    "",
                    vec![choose],
                )))
                .children(memory);
        };
        let prepared = backend::is_prepared(&tag);
        let version = java
            .as_ref()
            .map_or_else(|| backend::game_version(&tag), |j| j.minecraft.clone());
        let left = self.download_left();
        let steps = self.preparing.then(|| self.steps(&version, left));
        let banner = div()
            .flex()
            .flex_col()
            .gap(px(24.))
            .child(self.glass_home_head(&tag, prepared, cx))
            .children(steps);
        let mut page = div().flex().flex_col().gap(px(28.)).child(banner);
        if let Some(e) = self.prepare_error.clone() {
            page = page.child(error_card(e));
        }
        page = page
            .children(self.export_panel(cx))
            .children(self.launch_panel(cx))
            .child(self.glass_home_tabs(cx));
        if let Some(e) = self.client_error.clone() {
            page = page.child(error_card(e));
        }
        if let Some(crash) = self.crash.clone() {
            page = page.child(self.crash_notice(crash, cx));
        }
        page
    }

    /// "Eclipse crashed": the report and the logs, and where to report it.
    fn crash_notice(&self, crash: Crash, cx: &mut Context<Self>) -> Div {
        use eclipse_theme::glass::{ink, pixel_text, step};
        let mut actions = div().flex().flex_wrap().gap(px(8.)).mt(px(12.));
        if let Some(report) = crash.report.clone() {
            let open = report.clone();
            actions = actions
                .child(
                    button("crash-open", "Open report", Kind::Secondary)
                        .on_press(move |_, _, cx| crate::glass_toast::open(cx, &open)),
                )
                .child(
                    button(
                        "crash-copy",
                        if crash.copied {
                            "Copied"
                        } else {
                            "Copy report"
                        },
                        Kind::Secondary,
                    )
                    .on_press(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            if let Ok(text) = std::fs::read_to_string(&report) {
                                copy_to_clipboard(cx, gpui::ClipboardItem::new_string(text));
                                if let Some(c) = &mut this.crash {
                                    c.copied = true;
                                }
                            }
                            cx.notify();
                        },
                    )),
                );
        }
        actions = actions.child(
            button("crash-logs", "Open logs folder", Kind::Secondary).on_press(|_, _, cx| {
                let dir = backend::logs_dir();
                let _ = std::fs::create_dir_all(&dir);
                crate::glass_toast::open(cx, &dir);
            }),
        );
        actions = actions.child(button("crash-dismiss", "Dismiss", Kind::Ghost).on_press(
            cx.listener(|this, _: &ClickEvent, _, cx| {
                this.crash = None;
                cx.notify();
            }),
        ));
        let help = crash
            .report
            .is_none()
            .then(|| tr("The game ended without a crash report."));
        let mut body = div().id("crash").flex().flex_col().gap(px(4.)).child(
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .child(glass_ui::icon(
                    "alert-circle",
                    glass_ui::IconSize::Px16,
                    glass_ui::danger(),
                ))
                .child(glass_ui::entity_title(
                    tr("Eclipse crashed"),
                    pixel_text::CARD_TITLE,
                )),
        );
        if !crash.summary.is_empty() {
            body = body.child(
                glass_ui::text(step::BODY, FontWeight::NORMAL, ink::BODY).child(crash.summary),
            );
        }
        if let Some(report) = &crash.report {
            body = body.child(
                glass_ui::text(step::CAPTION, FontWeight::NORMAL, ink::MUTED)
                    .font_family(theme::MONO)
                    .truncate()
                    .child(short(report)),
            );
        }
        body = body.child(actions).children(help.map(glass_ui::card_note));
        glass::plate(eclipse_theme::glass::radius::PANEL)
            .p(px(glass_ui::CARD_PADDING))
            .child(body)
    }

    fn settings_page(&mut self, cx: &mut Context<Self>) -> Div {
        let toggle = glass_ui::toggle_row(
            "auto-install",
            tr("Install new Eclipse versions automatically"),
            None,
            self.settings.auto_install(),
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.settings.install_new_versions = Some(!this.settings.auto_install());
            this.save_settings();
            if this.settings.auto_install() {
                this.install_update();
            }
            cx.notify();
        }));
        let keeps = trf(
            "Keeps the {count} newest and any version an instance plays.",
            &[("count", &self.settings.keep_newest().to_string())],
        );
        let prune = glass_ui::toggle_row(
            "auto-prune",
            tr("Delete old Eclipse versions automatically"),
            Some(div().child(keeps).into_any_element()),
            self.settings.auto_prune(),
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.settings.prune_old_versions = Some(!this.settings.auto_prune());
            this.save_settings();
            cx.notify();
        }));
        // Linux: the launcher's entry in the applications menu (`eclipse_launcher::desktop`).
        let menu_entry = glass_ui::toggle_row(
            "desktop-entry",
            tr("Show in applications menu"),
            None,
            self.settings.desktop_entry(),
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            let on = !this.settings.desktop_entry();
            this.settings.desktop_entry = Some(on);
            this.save_settings();
            let _ = if on {
                eclipse_launcher::desktop::install(backend::exe_path()).map(|_| ())
            } else {
                eclipse_launcher::desktop::remove()
            };
            cx.notify();
        }));
        let eclipse = match (&self.updates.eclipse, &self.update_error) {
            (Some(r), _) => trf("{version} available", &[("version", update_name(r))]),
            (None, Some(e)) => glass_ui::plain_error(e).to_string(),
            (None, None) if self.checked.is_some() => tr("Up to date").to_string(),
            _ => tr("Checking…").to_string(),
        };
        let eclipse_action = self
            .updates
            .eclipse
            .as_ref()
            .filter(|_| self.installing.is_none())
            .map(|_| {
                button("settings-install", "Install", Kind::Secondary)
                    .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.install_update();
                        cx.notify();
                    }))
                    .into_any_element()
            });
        let launcher = match &self.self_update {
            SelfUpdate::Downloading(v, p) => {
                let done = format!("{:.0}", p * 100.);
                let said = trf(
                    "downloading {version}, {percent}%",
                    &[("version", v), ("percent", &done)],
                );
                format!("{}  ·  {said}", backend::VERSION)
            }
            SelfUpdate::Ready(v) => format!(
                "{}  ·  {}",
                backend::VERSION,
                trf("{version} available", &[("version", v)])
            ),
            SelfUpdate::Stuck(Some(e)) => format!(
                "{}  ·  {}",
                backend::VERSION,
                trf("the update failed: {error}", &[("error", e)])
            ),
            SelfUpdate::Stuck(None) => format!(
                "{}  ·  {}",
                backend::VERSION,
                self.launcher_update_help(None)
            ),
            SelfUpdate::Failed(e) => format!(
                "{}  ·  {}",
                backend::VERSION,
                trf("the update failed: {error}", &[("error", e)])
            ),
            SelfUpdate::Idle => match &self.updates.launcher {
                Some(r) => format!(
                    "{}  ·  {}",
                    backend::VERSION,
                    trf(
                        "{version} available",
                        &[(
                            "version",
                            backend::launcher_version(&r.tag).unwrap_or_default()
                        )]
                    )
                ),
                None if self.checked.is_some() && self.update_error.is_none() => {
                    format!("{}  ·  {}", backend::VERSION, tr("up to date"))
                }
                None => backend::VERSION.to_string(),
            },
        };
        let launcher_action = match &self.self_update {
            SelfUpdate::Ready(_) => Some(
                button("update-settings", "Update", Kind::Secondary)
                    .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.apply_launcher_update(cx);
                        cx.notify();
                    }))
                    .into_any_element(),
            ),
            SelfUpdate::Stuck(_) if self.launcher_download_available() => {
                let url = backend::download_page(&self.links.website);
                Some(
                    button("download-settings", "Download again", Kind::Secondary)
                        .on_press(move |_, _, cx| cx.open_url(&url))
                        .into_any_element(),
                )
            }
            SelfUpdate::Downloading(..) | SelfUpdate::Stuck(_) => None,
            _ => Some(
                button("check", "Check now", Kind::Secondary)
                    .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.check_updates();
                        cx.notify();
                    }))
                    .into_any_element(),
            ),
        };
        let appearance = crate::glass_ui::segments(
            "appearance",
            &[
                (backend::Appearance::Dark, "Dark"),
                (backend::Appearance::Light, "Light"),
                (backend::Appearance::System, "System"),
            ],
            self.settings.appearance,
            {
                let this = cx.entity().downgrade();
                move |choice, window, cx| {
                    let _ = this.update(cx, |this, cx| {
                        this.settings.appearance = choice;
                        this.save_settings();
                        titlebar::apply_appearance(window, choice);
                        this.theme = this.settings_theme(window);
                        cx.notify();
                    });
                }
            },
        );
        self.glass_settings(
            glass_settings::Built {
                install: toggle,
                prune,
                menu_entry,
                appearance: appearance.into_any_element(),
                eclipse,
                eclipse_action,
                launcher,
                launcher_action,
            },
            cx,
        )
    }

    /// The warning before a native build: what the mods can do and which mods. The build starts
    /// only from its button.
    fn native_warning(
        &self,
        mods: &Result<Vec<native_mods::Bundle>, String>,
        cx: &mut Context<Self>,
    ) -> Div {
        let list = match mods {
            Ok(found) => found
                .iter()
                .map(|b| format!("{} {}", b.id, b.version))
                .collect::<Vec<_>>()
                .join(", "),
            Err(e) => e.clone(),
        };
        let panel = native_panel()
            .child(error_note(tr("These mods will run with full access to your computer")))
            .child(tr("They can read, change or delete your files and use your accounts. Build them only if you trust their authors.").to_string())
            .child(trf("Mods: {mods}", &[("mods", &list)]))
            .child(tr("The first build takes 20 to 30 minutes and a few GB of disk. Later ones take a few minutes.").to_string())
            .children(self.native_update.as_ref().map(|newer| {
                div().text_color(glass_ui::warn()).child(trf(
                    "This moves the instance to Eclipse {version}. It compiles the game from scratch, so it takes longer than a normal rebuild: up to 30 minutes.",
                    &[("version", &native_status::eclipse_label(newer))],
                ))
            }));
        let mut buttons = div().flex().gap(px(8.)).mt(px(4.));
        if mods.is_ok() {
            buttons = buttons.child(
                button("native-trust", "I trust these mods: build", Kind::Primary).on_press(
                    cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.build_native(false);
                        cx.notify();
                    }),
                ),
            );
        }
        panel.child(
            buttons.child(button("native-cancel", "Cancel", Kind::Secondary).on_press(
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.native_confirm = None;
                    this.native_update = None;
                    this.native_on_base = false;
                    cx.notify();
                }),
            )),
        )
    }

    /// Mods made for other Eclipse versions: which, and the version the build uses. They often
    /// still work, so the player may build anyway.
    fn native_mismatch_panel(&self, list: &str, cx: &mut Context<Self>) -> Div {
        native_panel()
            .child(error_note(tr("These mods were made for different Eclipse versions")))
            .child(list.to_owned())
            .child(tr("They often still work together. If the build fails, install versions made for the same Nightly.").to_string())
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .mt(px(4.))
                    .child(
                        button("native-anyway", "Build anyway", Kind::Primary).on_press(
                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.build_native(true);
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        button("native-mismatch-cancel", "Cancel", Kind::Secondary).on_press(
                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.native_mismatch = None;
                                cx.notify();
                            }),
                        ),
                    ),
            )
    }

    /// Settings' Windows ARM game architecture choice; release installation, updates and launch
    /// use the saved choice, while launcher updates continue to follow this process.
    fn game_architecture_row(&self, cx: &mut Context<Self>) -> Div {
        if !backend::can_choose_game_architecture() {
            return div();
        }
        let picker = crate::glass_ui::segments(
            "game-architecture",
            &[
                (backend::GameArchitecture::Auto, "Automatic"),
                (backend::GameArchitecture::X64, "x64"),
                (backend::GameArchitecture::Arm64, "ARM64"),
            ],
            self.settings.game_architecture,
            {
                let this = cx.entity().downgrade();
                move |choice, _, cx| {
                    let _ = this.update(cx, |this, cx| {
                        if this.settings.game_architecture == choice {
                            return;
                        }
                        this.settings.game_architecture = choice;
                        this.save_settings();
                        // The release and update lists contain the selected game's assets.
                        this.releases = Load::Loading;
                        this.updates = backend::Updates::default();
                        this.fetch_startup_releases();
                        cx.notify();
                    });
                }
            },
        );
        crate::glass_ui::setting_row(
            tr("Game architecture"),
            Some(
                div()
                    .child(tr("Only change this on Windows on ARM."))
                    .into_any_element(),
            ),
            Some(picker.into_any_element()),
        )
    }

    fn game_args_row(&self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let focused = self.args_focused;
        let field = div()
            .id("game-args")
            .track_focus(&self.args_focus)
            .flex()
            .items_center()
            .overflow_hidden()
            .cursor_text()
            .child(line_edit::content(
                &self.args_input,
                &t,
                focused,
                false,
                false,
                tr("For example --server-thread"),
            ))
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                window.focus(&this.args_focus, cx);
                cx.notify();
            }))
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                match line_edit::handle(&mut this.args_input, ev, cx) {
                    line_edit::Key::Pass => return,
                    line_edit::Key::Escape => window.blur(cx),
                    line_edit::Key::Edited => {
                        this.settings.game_args = this.args_input.text().to_string();
                        this.save_settings();
                    }
                    _ => {}
                }
                cx.stop_propagation();
                cx.notify();
            }));
        let field = line_edit::mouse(field, cx, |this| &mut this.args_input);
        glass_ui::setting_row_stacked(
            tr("Game arguments"),
            None,
            glass_ui::wide_field(field, focused).into_any_element(),
        )
    }

    /// Settings' Proxy row: the proxy every download and sign-in goes through, kept in
    /// `settings.json` and read back by `proxy.rs` on each request.
    fn proxy_row(&self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let focused = self.proxy_focused;
        let field = div()
            .id("proxy")
            .track_focus(&self.proxy_focus)
            .flex()
            .items_center()
            .overflow_hidden()
            .cursor_text()
            .child(line_edit::content(
                &self.proxy_input,
                &t,
                focused,
                false,
                false,
                tr("For example http://127.0.0.1:7890"),
            ))
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                window.focus(&this.proxy_focus, cx);
                cx.notify();
            }))
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                match line_edit::handle(&mut this.proxy_input, ev, cx) {
                    line_edit::Key::Pass => return,
                    line_edit::Key::Escape => window.blur(cx),
                    line_edit::Key::Edited => this.save_proxy(),
                    _ => {}
                }
                cx.stop_propagation();
                cx.notify();
            }));
        let field = line_edit::mouse(field, cx, |this| &mut this.proxy_input);
        // A half-typed address is no reason to complain; one left behind is.
        let detail = match (
            focused,
            eclipse_launcher::proxy::refuse(self.proxy_input.text()),
        ) {
            (false, Some(why)) => Some(
                div()
                    .text_color(glass_ui::danger())
                    .child(why)
                    .into_any_element(),
            ),
            _ => None,
        };
        glass_ui::setting_row_stacked(
            tr("Proxy"),
            detail,
            glass_ui::wide_field(field, focused).into_any_element(),
        )
    }

    /// Keeps the Proxy field in `settings.json`, and tells the parts that build their own agents
    /// (`eclipse-port-pack`) which proxy to use.
    fn save_proxy(&mut self) {
        let value = self.proxy_input.text().trim().to_owned();
        self.settings.proxy = (!value.is_empty()).then_some(value);
        self.save_settings();
        eclipse_launcher::proxy::announce(self.settings.proxy.clone());
    }

    /// The Microsoft accounts signed in here: each one's head, name and state, Play as,
    /// Refresh and Sign out.
    fn microsoft_rows(&mut self, cx: &mut Context<Self>) -> (Div, usize) {
        let mut rows = div().flex().flex_col();
        let mut n = 0;
        let selected = self.accounts.selected.clone();
        for (i, a) in self.accounts.accounts.clone().into_iter().enumerate() {
            let plays = selected.as_ref() == Some(&a.uuid);
            let uuid = a.uuid.clone();
            let mut actions = div().flex().gap(px(4.));
            if plays {
                actions = actions.child(
                    button(format!("offline-{i}"), "Play offline", Kind::Ghost).on_press(
                        cx.listener(|this, _: &ClickEvent, _, cx| {
                            let mut accounts = Accounts::load();
                            accounts.selected = None;
                            if accounts.save().is_ok() {
                                this.accounts = accounts;
                            }
                            cx.notify();
                        }),
                    ),
                );
            } else {
                let uuid = uuid.clone();
                actions = actions.child(
                    button(format!("select-{i}"), "Play as", Kind::Secondary).on_press(
                        cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.play_as(account::Player::Microsoft(uuid.clone()));
                            cx.notify();
                        }),
                    ),
                );
            }
            let sign_in_progress = self
                .sign_in_target
                .as_ref()
                .filter(|a| a.uuid == uuid)
                .and_then(|_| match self.sign_in_state.as_ref() {
                    Some(SignInState::Starting) => Some("Asking Microsoft…"),
                    Some(SignInState::Code(_)) => Some("Waiting for browser sign-in…"),
                    Some(SignInState::Working(phase)) => Some(phase.label()),
                    _ => None,
                });
            let progress = sign_in_progress
                .or_else(|| self.auth_progress.get(&uuid).map(|phase| phase.label()));
            let busy = self.auth_busy.contains_key(&uuid) || sign_in_progress.is_some();
            let status = self.auth_status.get(&uuid);
            let needs_sign_in =
                status.is_some_and(|s| s.state == account::session::State::SignInRequired);
            let refresh_id = uuid.clone();
            let refresh = button(
                format!("refresh-{i}"),
                if busy {
                    "Checking…"
                } else if needs_sign_in {
                    "Sign in again"
                } else {
                    "Refresh"
                },
                Kind::Ghost,
            );
            actions = actions.child(if busy {
                disabled(refresh)
            } else {
                refresh.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if needs_sign_in {
                        this.re_sign_in(&refresh_id);
                    } else {
                        this.refresh_account(refresh_id.clone());
                    }
                    cx.notify();
                }))
            });
            let sign_out = button(format!("sign-out-{i}"), "Sign out", Kind::Ghost);
            actions = actions.child(if busy {
                disabled(sign_out)
            } else {
                sign_out.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    let Some(generation) = this.begin_auth_job(&uuid) else {
                        return;
                    };
                    let (tx, uuid) = (this.tx.clone(), uuid.clone());
                    std::thread::spawn(move || {
                        let mut accounts = Accounts::load();
                        let _ = tx.send(Msg::AccountProgress(
                            uuid.clone(),
                            generation,
                            account::session::Phase::SigningOut,
                        ));
                        let result = accounts
                            .remove(account::store(), &uuid)
                            .and_then(|()| accounts.save())
                            .map(|()| accounts.has_owner(account::store()));
                        let _ = tx.send(Msg::AccountRemoved(uuid, generation, result));
                    });
                    cx.notify();
                }))
            });
            n += 1;
            rows = rows.child(account_row(
                self.profile_heads.draw(Some(&a.uuid), 32., self.scale),
                a.name.clone(),
                Some(
                    div()
                        .child(tr(if plays {
                            "Microsoft account · in use"
                        } else {
                            "Microsoft account"
                        }))
                        .child(account_status_line(status, progress))
                        .when(!busy, |row| {
                            row.when_some(status.filter(|s| s.message != s.label()), |row, s| {
                                row.child(div().child(tr(&s.message).to_string()))
                            })
                        })
                        .into_any_element(),
                ),
                actions,
            ));
        }
        (rows, n)
    }

    /// Adding a Microsoft account, or signing one in again: what the row is called, what is
    /// happening (the code to enter, a problem), and its buttons.
    fn sign_in_parts(&self, cx: &mut Context<Self>) -> (String, Div, Option<AnyElement>) {
        let t = self.theme;
        let configured = account::client_id().is_some();
        let busy = matches!(
            self.sign_in_state,
            Some(SignInState::Starting | SignInState::Code(_) | SignInState::Working(_))
        );
        let detail = match &self.sign_in_state {
            _ if !configured => div().child(tr(account::NOT_CONFIGURED)),
            Some(SignInState::Starting) => div().child(tr("Asking Microsoft…")),
            Some(SignInState::Working(phase)) => div().child(tr(phase.label())),
            Some(SignInState::Code(code)) => div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(trf(
                    "Open {link} in your browser and enter this code:",
                    &[("link", &code.verification_uri)],
                ))
                .child(
                    glass_ui::text(
                        eclipse_theme::glass::step::FIGURE,
                        FontWeight::MEDIUM,
                        eclipse_theme::glass::ink::FULL,
                    )
                    .font_family(theme::MONO)
                    .child(code.user_code.clone()),
                ),
            Some(SignInState::Done(Ok(a))) => {
                div().child(trf("Signed in as {name}.", &[("name", &a.name)]))
            }
            Some(SignInState::Done(Err(e))) => {
                div().text_color(glass_ui::danger()).child(e.clone())
            }
            None if self.accounts.accounts.is_empty() => div().child(tr(
                "Sign in with the Microsoft account that owns Minecraft: Java Edition.",
            )),
            None => div(),
        };
        let detail = if let Some(target) = &self.sign_in_target {
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(trf(
                    "Use the Microsoft account for {name}.",
                    &[("name", &target.name)],
                ))
                .child(detail)
        } else {
            detail
        };
        let detail = match self.keychain.clone() {
            Some(said) if !self.accounts.accounts.is_empty() => div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(detail)
                .child(div().child(said)),
            _ => detail,
        };
        let action =
            if !configured {
                None
            } else if let Some(SignInState::Code(code)) = &self.sign_in_state {
                let (uri, user_code) = (code.verification_uri.clone(), code.user_code.clone());
                Some(
                    div()
                        .flex()
                        .gap(px(4.))
                        .child(
                            button("sign-in-copy", "Copy code", Kind::Secondary).on_press(
                                move |_, _, cx| {
                                    copy_to_clipboard(
                                        cx,
                                        gpui::ClipboardItem::new_string(user_code.clone()),
                                    )
                                },
                            ),
                        )
                        .child(
                            button("sign-in-open", "Open link", Kind::Secondary)
                                .on_press(move |_, _, cx| cx.open_url(&uri)),
                        )
                        .child(button("sign-in-cancel", "Cancel", Kind::Ghost).on_press(
                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.cancel_sign_in();
                                cx.notify();
                            }),
                        ))
                        .into_any_element(),
                )
            } else if busy {
                Some(
                    button("sign-in-cancel-working", "Cancel", Kind::Ghost)
                        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.cancel_sign_in();
                            cx.notify();
                        }))
                        .into_any_element(),
                )
            } else {
                let target = self.sign_in_target.as_ref().map(|a| a.uuid.clone());
                let targeted = target.is_some();
                // Re-signing one account names it; adding one is Microsoft's own button.
                let sign_in = if targeted {
                    button("sign-in", "Sign in again", Kind::Secondary)
                } else {
                    microsoft::sign_in_button("sign-in", &t)
                };
                let mut actions = div().flex().gap(px(4.)).child(sign_in.on_press(cx.listener(
                    move |this, _: &ClickEvent, _, cx| {
                        if let Some(uuid) = &target {
                            this.re_sign_in(uuid);
                        } else {
                            this.start_sign_in();
                        }
                        cx.notify();
                    },
                )));
                if targeted {
                    actions = actions.child(
                        button("sign-in-cancel-target", "Cancel", Kind::Ghost).on_press(
                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.cancel_sign_in();
                                cx.notify();
                            }),
                        ),
                    );
                }
                Some(actions.into_any_element())
            };
        let title = match &self.sign_in_target {
            Some(target) => trf("Sign in again as {name}", &[("name", &target.name)]),
            None if self.accounts.accounts.is_empty() => tr("Microsoft account").to_owned(),
            None => tr("Another account").to_owned(),
        };
        (title, detail, action)
    }

    /// Join a friend: a code field and Join, which starts the game straight
    /// into the friend's world (errors show in the game).
    /// Plays the selected instance straight into a world (`--world`) or a server (`--server`):
    /// flags every Eclipse release takes.
    pub(crate) fn quick_play(&mut self, flag: &'static str, value: String) {
        self.quick_play = Some((flag, value));
        self.play();
        if self.starting_client.is_none() {
            self.quick_play = None;
        }
    }

    fn join_friend(&mut self) {
        if eclipse_friends::invite::parse(self.friend_code.text()).is_none() {
            return;
        }
        self.joining = Some(self.friend_code.text().to_string());
        self.play();
        if self.starting_client.is_none() {
            self.joining = None;
        }
    }

    fn versions_page(&mut self, cx: &mut Context<Self>) -> Div {
        // Measured again when the installed versions change.
        if self.disk_for != self.installed {
            self.disk_for = self.installed.clone();
            let (tags, tx) = (self.installed.clone(), self.tx.clone());
            std::thread::spawn(move || {
                let disk = tags.into_iter().map(|t| {
                    let n = backend::disk_use(&t);
                    (t, n)
                });
                let _ = tx.send(Msg::DiskUse(disk.collect()));
            });
        }
        let on_disk = |tag: &String| {
            let found = self.disk.iter().find(|(t, _)| t == tag);
            found
                .filter(|_| self.installed.contains(tag))
                .map(|(_, n)| *n)
        };
        let used: u64 = self.installed.iter().filter_map(on_disk).sum();
        self.glass_versions(&on_disk, used, cx)
    }

    fn notes_page(&mut self, cx: &mut Context<Self>) -> Div {
        self.glass_notes(cx)
    }
}

/// A thin bar. A known fraction eases from its previous value; an unknown one shuttles slowly.
fn progress_bar(fraction: Option<(f32, f32)>) -> Div {
    let bar = div()
        .absolute()
        .top_0()
        .bottom_0()
        .rounded_full()
        .bg(glass_ui::color(glass_ui::look().accent_soft));
    let track = div()
        .relative()
        .flex_1()
        .h(px(4.))
        .rounded_full()
        .overflow_hidden()
        .bg(glass_ui::white(eclipse_theme::glass::fill::ACTIVE));
    match fraction {
        Some((from, to)) => {
            let (from, to) = (from.clamp(0.02, 1.), to.clamp(0.02, 1.));
            track.child(bar.left_0().with_animation(
                ElementId::named_usize("bar", (to * 1000.) as usize),
                Animation::new(theme::MOVE).with_easing(ease_out_quint()),
                move |d, p| d.w(relative(from + (to - from) * p)),
            ))
        }
        None => track.child(
            bar.w(relative(0.3)).with_animation(
                "shuttle",
                Animation::new(Duration::from_millis(1600))
                    .repeat()
                    .with_easing(gpui::ease_in_out),
                |d, p| d.left(relative(-0.3 + 1.3 * p)),
            ),
        ),
    }
}

impl Render for Launcher {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        if std::mem::take(&mut self.restyle) {
            self.theme = self.settings_theme(window);
        }
        let size = window.viewport_size();
        glass_ui::begin_frame(&self.theme, f32::from(size.height) < layout::SHORT_WINDOW);
        self.scale = window.scale_factor();
        self.perf.frame();
        if self.bench.as_ref().is_some_and(perf::Bench::scrolling) {
            self.bench_scroll(window);
        }
        self.servers.note_focus(window, cx);
        self.friend_focused = self.friend_focus.is_focused(window);
        self.mods.note_focus(window);
        self.github.note_focus(window);
        self.args_focused = self.args_focus.is_focused(window);
        self.proxy_focused = self.proxy_focus.is_focused(window);
        self.return_focus_from_menu(window, cx);
        self.window_width = f32::from(size.width);
        self.window_long_side = f32::from(size.width).max(f32::from(size.height));
        self.scale_factor = window.scale_factor();
        let content = match self.page {
            Page::Play => self.play_page(cx),
            Page::Versions => self.versions_page(cx),
            Page::Mods => self.mods_page(cx),
            Page::Servers => self.servers_page(cx),
            Page::Notes => self.notes_page(cx),
            Page::Settings => self.settings_page(cx),
            Page::InstanceEditor => self.instance_page(cx),
        };
        self.pictures.free_retired(window);
        // The new page fades in and rises a few pixels, as the website's tabs do (`hzl-pop`).
        let content = div()
            .relative()
            .w_full()
            // Let the page shrink inside the padded flex row so long notices can wrap.
            .min_w_0()
            .child(content)
            .with_animation(
                ElementId::named_usize("page", self.switches),
                motion::eased(motion::POP),
                |d, p| d.opacity(p).top(px(motion::POP_RISE * (1. - p))),
            );
        if window.focused(cx).is_none() {
            window.focus(&self.root_focus, cx);
        }
        self.scene.update(cx, |scene, _| scene.set_look(&t.night));
        let sky = self.scene.clone().into_any_element();
        let sidebar = self.glass_sidebar(cx);
        let row_menu = self.row_menu(cx);
        let mut root = div()
            .id("root")
            .track_focus(&self.root_focus)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                let k = &ev.keystroke;
                match k.key.as_str() {
                    "tab" => {
                        if k.modifiers.shift {
                            window.focus_prev(cx);
                        } else {
                            window.focus_next(cx);
                        }
                    }
                    "escape" if this.close_menus() => {
                        if let Some(opener) = this.menu_opener.take() {
                            window.focus(&opener, cx);
                        }
                    }
                    "escape" if this.confirm_escape() => {}
                    "escape" if this.native_log_escape() => {}
                    "escape" if this.servers_escape() => {}
                    "escape" if this.instance_escape() => {}
                    "escape" if this.screenshot_escape() => {}
                    "left" if this.screenshot_step(false) => {}
                    "right" if this.screenshot_step(true) => {}
                    _ => return,
                }
                cx.stop_propagation();
                cx.notify();
            }))
            // ⌘K (Ctrl+K elsewhere) wherever the focus is, even in a field: the palette.
            .capture_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                let k = &ev.keystroke;
                let command = if cfg!(target_os = "macos") {
                    k.modifiers.platform
                } else {
                    k.modifiers.control
                };
                if command && !k.modifiers.shift && k.key == "k" {
                    this.palette_toggle(window, cx);
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .font_family(theme::FONT)
            .text_color(glass_ui::white(eclipse_theme::glass::ink::FULL))
            .overflow_hidden()
            .child(
                div().relative().flex().size_full().child(sidebar).child(
                    div().flex().flex_col().flex_1().min_w_0().child(
                        self.content_scroll
                            .wrap(
                                glass_ui::scroll_pane("content", &self.content_scroll.handle)
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
                                            .px(px(layout::PAGE_GUTTER))
                                            .pb(px(layout::PAGE_FOOT))
                                            // Settings and the New instance page scroll
                                            // their own pane under a still title.
                                            .when(
                                                matches!(
                                                    self.page,
                                                    Page::Settings | Page::InstanceEditor
                                                ),
                                                |d| d.h_full().pb(px(14.)),
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
        if self.page == Page::Play {
            if let Some(viewer) = self.screenshot_viewer(cx) {
                root = root.child(viewer);
            }
        }
        root = root.children(self.toast_float());
        root = root.children(glass_ui::focus_tip(window));
        if let Some(palette) = self.palette_overlay(window, cx) {
            root = root.child(palette);
        }
        if self.asking_consent() {
            root = root.child(self.consent_screen(cx));
        }
        if self.asking_owner() {
            root = root.child(self.owner_screen(cx));
        }
        div()
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
    if page == Page::Play && data_folder_failed {
        Page::Settings
    } else {
        page
    }
}

/// The menu bar on macOS, where "Quit" belongs to the app menu.
fn menus() -> Vec<Menu> {
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
    eclipse_launcher::pictures::probe_gpu_limit();
    gpui_platform::application()
        .with_assets(blocks::Assets)
        // `move`: the closure owns `setup_window`; `run` wants it to outlive this frame.
        .run(move |cx: &mut App| {
            cx.text_system()
                .add_fonts(vec![
                    Cow::Borrowed(eclipse_theme::fonts::GEIST_MONO),
                    Cow::Borrowed(eclipse_theme::fonts::GEIST_REGULAR),
                    Cow::Borrowed(eclipse_theme::fonts::GEIST_MEDIUM),
                    Cow::Borrowed(eclipse_theme::fonts::GEIST_SEMIBOLD),
                    Cow::Borrowed(eclipse_theme::fonts::MONOCRAFT_REGULAR),
                    Cow::Borrowed(eclipse_theme::fonts::MONOCRAFT_SEMIBOLD),
                    Cow::Borrowed(eclipse_theme::fonts::MONOCRAFT_BOLD),
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
            cx.set_menus(menus());
            // The setup exe (and `eclipse-launcher.exe uninstall`) opens its own window instead.
            #[cfg(windows)]
            if let Some(mode) = setup_window {
                setup_page::open(mode, cx);
                return;
            }
            #[cfg(not(windows))]
            let _ = setup_window;
            let (width, height) = layout::DEFAULT_WINDOW;
            let bounds = Bounds::centered(None, size(px(width), px(height)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(layout::MIN_WINDOW.0), px(layout::MIN_WINDOW.1))),
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

/// Whether the click `event` began with the mouse-down that closed a menu at `closed_at`.
/// A keyboard click never did.
fn closed_by_press(closed_at: Option<Point<Pixels>>, event: &ClickEvent) -> bool {
    matches!(event, ClickEvent::Mouse(click) if closed_at == Some(click.down.position))
}

#[cfg(test)]
mod tests {
    #[test]
    fn an_unopenable_data_folder_starts_on_settings_instead_of_exiting() {
        use super::{page_after_startup, Page};
        assert!(matches!(
            page_after_startup(Page::Play, true),
            Page::Settings
        ));
        assert!(matches!(page_after_startup(Page::Play, false), Page::Play));
        assert!(matches!(page_after_startup(Page::Mods, true), Page::Mods));
    }

    #[test]
    fn a_menu_button_stays_closed_after_the_press_that_closed_its_menu() {
        use super::closed_by_press;
        use gpui::{point, px, ClickEvent, KeyboardClickEvent, MouseClickEvent, MouseDownEvent};
        let click = |x: f32| {
            ClickEvent::Mouse(MouseClickEvent {
                down: MouseDownEvent {
                    position: point(px(x), px(20.)),
                    ..MouseDownEvent::default()
                },
                ..MouseClickEvent::default()
            })
        };
        let closed_at = Some(point(px(10.), px(20.)));
        // The press on the button that closed the open menu: it stays closed.
        assert!(closed_by_press(closed_at, &click(10.)));
        // A later press, or one with no menu closed, toggles as before.
        assert!(!closed_by_press(closed_at, &click(11.)));
        assert!(!closed_by_press(None, &click(10.)));
        let keyboard = ClickEvent::Keyboard(KeyboardClickEvent::default());
        assert!(!closed_by_press(closed_at, &keyboard));
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
    fn a_press_on_a_sidebar_row_opens_its_menu_or_closes_it() {
        let open = |id: &str| Some((id.to_string(), false));
        assert_eq!(
            super::row_menu_after_press(None, "Eclipse"),
            open("Eclipse")
        );
        assert_eq!(
            super::row_menu_after_press(open("Other"), "Eclipse"),
            open("Eclipse")
        );
        assert_eq!(
            super::row_menu_after_press(open("Eclipse"), "Eclipse"),
            None
        );
        // A Delete… already pressed on another row does not carry over.
        assert_eq!(
            super::row_menu_after_press(Some(("Other".into(), true)), "Eclipse"),
            open("Eclipse")
        );
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

    /// An owner's Play works on a version not prepared yet: it prepares it (downloading
    /// Minecraft from Mojang when this computer has no install), then plays. Without an owner
    /// it stays off: signing in comes first.
    #[test]
    fn play_prepares_first_for_an_owner() {
        use super::{play_press, PlayPress};
        assert_eq!(play_press(true, Some(true), false), PlayPress::Play);
        assert_eq!(play_press(true, None, false), PlayPress::Play);
        assert_eq!(
            play_press(false, Some(true), false),
            PlayPress::PrepareThenPlay
        );
        assert_eq!(play_press(false, Some(false), false), PlayPress::Nothing);
        assert_eq!(play_press(false, None, false), PlayPress::Nothing);
        assert_eq!(play_press(false, Some(true), true), PlayPress::Nothing);
        assert_eq!(play_press(true, Some(true), true), PlayPress::Nothing);
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
        let menus = super::menus();
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
    fn counts_get_thousands_separators() {
        let shown: Vec<String> = [0, 7, 999, 1000, 12_300, 1_234_500]
            .into_iter()
            .map(super::thousands)
            .collect();
        assert_eq!(shown, ["0", "7", "999", "1,000", "12,300", "1,234,500"]);
    }
}
