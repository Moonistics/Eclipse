//! The Servers page: create a server in one dialog, then start, stop and watch it, give friends its
//! address, manage who may join, change its settings, back it up and update it, all with buttons. Every
//! action runs `eclipse-server` or calls the running server's control API (`eclipse_launcher::servers`).
use super::line_edit::{self, Key, LineEdit};
use super::*;
use crate::glass_ui as ui;
use eclipse_launcher::heads;
use eclipse_launcher::servers::{
    self, Applied, Backup, Entry, Fix, NewServer, Problem, Status, World,
};
use eclipse_theme::glass::{fill, ink, radius, step};
use gpui::ScrollHandle;
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

#[path = "glass_server_dialog.rs"]
mod glass_server_dialog;
#[path = "glass_servers.rs"]
mod glass_servers;
pub(super) use glass_servers::notice_line;

/// The servers this launcher started in this session, stopped when it closes unless they run as a
/// service (`stop_on_quit`).
static STARTED_HERE: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// How often the open server's status and log are read, and the list's.
const POLL_OPEN: Duration = Duration::from_millis(1000);
const POLL_LIST: Duration = Duration::from_secs(3);

/// Lines of the server's log kept on the page.
const CONSOLE_KEEP: usize = 1000;

const FACE: f32 = 28.;

const DELETE_CONTROLS: f32 = 260.;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) enum Field {
    Name,
    Seed,
    Players,
    Command,
    AddPlayer,
    Motd,
    Port,
    Token,
    Email,
    /// The New instance page's (`instances_page.rs`).
    InstanceName,
    InstanceGroup,
    /// Its search in Mojang's versions (`new_instance.rs`).
    InstanceVersion,
    /// Home's Mods tab's filter (`instance_mods.rs`).
    InstanceModsFilter,
    /// Home's Multiplayer section's (`multiplayer_page.rs`).
    ServerName,
    ServerAddress,
    /// Home's Console's search (`console_page.rs`).
    ConsoleSearch,
    /// The Launch panel's (`launch_page.rs`).
    PreLaunch,
    Wrapper,
    LaunchEnv,
    PostExit,
    /// Home's memory row (`launch_page.rs`).
    MemoryLimit,
    /// Settings' new offline name (`switch_page.rs`).
    OfflineName,
    DeleteServer,
    /// Settings' accent as hex (`themes_page.rs`).
    AccentHex,
}

impl Field {
    const ALL: [Field; 24] = [
        Field::Name,
        Field::Seed,
        Field::Players,
        Field::Command,
        Field::AddPlayer,
        Field::Motd,
        Field::Port,
        Field::Token,
        Field::Email,
        Field::InstanceName,
        Field::InstanceGroup,
        Field::InstanceVersion,
        Field::InstanceModsFilter,
        Field::ServerName,
        Field::ServerAddress,
        Field::ConsoleSearch,
        Field::PreLaunch,
        Field::Wrapper,
        Field::LaunchEnv,
        Field::PostExit,
        Field::MemoryLimit,
        Field::OfflineName,
        Field::DeleteServer,
        Field::AccentHex,
    ];
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Tab {
    Overview,
    Players,
    Settings,
    Backups,
}

const MODES: [(&str, &str); 3] = [
    ("survival", "Survival"),
    ("creative", "Creative"),
    ("adventure", "Adventure"),
];

const DIFFICULTIES: [(&str, &str); 4] = [
    ("peaceful", "Peaceful"),
    ("easy", "Easy"),
    ("normal", "Normal"),
    ("hard", "Hard"),
];

/// What the background work reports.
pub(super) enum Event {
    Polled {
        dir: PathBuf,
        status: Box<Status>,
        logs: Option<(u64, Vec<String>)>,
    },
    Statuses(Vec<(PathBuf, Status)>),
    Line(String),
    Done(Result<String, Problem>),
    Created(Result<Entry, Problem>),
    Deleted {
        dir: PathBuf,
        result: Result<(), String>,
    },
    Details {
        dir: PathBuf,
        config: Option<Value>,
        backups: Vec<Backup>,
        service: bool,
    },
    Applied(Result<Applied, Problem>),
    Pvp(Option<bool>),
    UpdateCheck(Result<Value, Problem>),
    Failed(String),
}

/// The Create dialog's answers.
pub(super) struct Create {
    new_world: bool,
    copy: Option<PathBuf>,
    worlds: Vec<(String, PathBuf)>,
    /// The dialog's line under each world: its name and "Eclipse · mode · size", read
    /// once when the dialog opens.
    details: HashMap<PathBuf, (String, String)>,
    mode: &'static str,
    difficulty: &'static str,
    max_players: u32,
    friends_only: bool,
    check_accounts: bool,
    eula: bool,
    /// Create was pressed without the EULA ticked.
    eula_missing: bool,
    /// The dialog's friends let in, the first an operator.
    friends: Vec<String>,
}

/// What the sidebar shows of the Servers page: its list once the page has read it, and whether
/// a server answers.
impl Servers {
    /// A stopped server's line in the list: "Survival · stopped". The open
    /// server's mode is the one its settings show now, once read; the others' (and the open one's
    /// until then) come from their files.
    pub(super) fn stopped_line(&mut self, dir: &std::path::Path) -> String {
        let open = Some(self.shown("world", "game_mode"))
            .filter(|m| !m.is_empty() && self.open.as_deref() == Some(dir));
        let mode = open.or_else(|| {
            self.modes
                .entry(dir.to_path_buf())
                .or_insert_with(|| servers::game_mode(dir))
                .clone()
        });
        match mode {
            Some(mode) => {
                let label = MODES
                    .iter()
                    .find(|(id, _)| *id == mode)
                    .map_or(mode.clone(), |(_, label)| tr(label).to_string());
                trf("{mode} · stopped", &[("mode", &label)])
            }
            None => tr("Stopped").to_string(),
        }
    }

    pub(super) fn sidebar_list(&self) -> Option<&[Entry]> {
        self.loaded.then_some(&self.list[..])
    }

    pub(super) fn is_up(&self, dir: &std::path::Path) -> bool {
        self.statuses.get(dir).is_some_and(|s| s.phase == "running")
    }
}

pub(super) struct Servers {
    list: Vec<Entry>,
    loaded: bool,
    pub(super) open: Option<PathBuf>,
    tab: Tab,
    statuses: HashMap<PathBuf, Status>,
    /// Each listed server's game mode from its `server.toml`, read once while it is listed.
    modes: HashMap<PathBuf, Option<String>>,
    polling: bool,
    polled: Option<Instant>,
    log: VecDeque<String>,
    log_next: u64,
    log_dir: Option<PathBuf>,
    console_scroll: ScrollHandle,
    create_scroll: Scrollbar,
    /// The action under way, and what it printed.
    busy: Option<String>,
    job_log: VecDeque<String>,
    problem: Option<Problem>,
    note: Option<String>,
    pub(super) create: Option<Create>,
    creating: bool,
    config: Option<Value>,
    /// Settings waiting for `config set`, and whether one runs.
    queued: Vec<(String, String)>,
    applying: bool,
    /// Settings saved while the server ran that wait for its next start.
    restart_needed: BTreeSet<String>,
    backups: Vec<Backup>,
    confirm_restore: Option<String>,
    confirm_delete: bool,
    service: Option<bool>,
    update: Option<Value>,
    /// `update --check` is under way.
    checking: bool,
    pvp: Option<bool>,
    texts: HashMap<Field, LineEdit>,
    focus: HashMap<Field, FocusHandle>,
    focused: Option<Field>,
    /// The field to put the keyboard in on the next frame.
    pub(super) focus_soon: Option<Field>,
    /// A suggested value shown selected: the first key typed replaces it.
    selected: Option<Field>,
    /// A field whose value was refused, and why: shown under it until it's edited.
    invalid: Option<(Field, &'static str)>,
    copied: Option<Instant>,
    faces: HashMap<String, Option<Vec<[u8; 4]>>>,
    tx: Sender<Event>,
    rx: Receiver<Event>,
}

impl Servers {
    pub(super) fn new() -> Self {
        let (tx, rx) = channel();
        Self {
            list: Vec::new(),
            loaded: false,
            open: None,
            tab: Tab::Overview,
            statuses: HashMap::new(),
            modes: HashMap::new(),
            polling: false,
            polled: None,
            log: VecDeque::new(),
            log_next: 0,
            log_dir: None,
            console_scroll: ScrollHandle::new(),
            create_scroll: Scrollbar::default(),
            busy: None,
            job_log: VecDeque::new(),
            problem: None,
            note: None,
            create: None,
            creating: false,
            config: None,
            queued: Vec::new(),
            applying: false,
            restart_needed: BTreeSet::new(),
            backups: Vec::new(),
            confirm_restore: None,
            confirm_delete: false,
            service: None,
            update: None,
            checking: false,
            pvp: None,
            texts: HashMap::new(),
            focus: HashMap::new(),
            focused: None,
            focus_soon: None,
            selected: None,
            invalid: None,
            copied: None,
            faces: HashMap::new(),
            tx,
            rx,
        }
    }

    pub(super) fn text(&mut self, f: Field) -> &mut LineEdit {
        self.texts.entry(f).or_default()
    }

    pub(super) fn value(&self, f: Field) -> &str {
        self.texts.get(&f).map_or("", LineEdit::text)
    }

    /// Mark a field invalid without an explanatory message, or clear its previous error.
    pub(super) fn set_field_invalid(&mut self, field: Field, invalid: bool) {
        if invalid {
            self.invalid = Some((field, ""));
        } else if self.invalid.is_some_and(|(current, _)| current == field) {
            self.invalid = None;
        }
    }

    /// Whether this field has the keyboard, as of this frame.
    pub(super) fn has_focus(&self, f: Field) -> bool {
        self.focused == Some(f)
    }

    /// Which field has the keyboard, read every frame.
    pub(super) fn note_focus(&mut self, window: &mut Window, cx: &mut App) {
        if let Some(h) = self.focus_soon.take().and_then(|f| self.focus.get(&f)) {
            window.focus(h, cx);
        }
        self.focused = Field::ALL
            .into_iter()
            .find(|f| self.focus.get(f).is_some_and(|h| h.is_focused(window)));
        if self.focused.is_some() && self.focused != self.selected {
            self.selected = None;
        }
    }

    fn status(&self) -> Status {
        self.open
            .as_ref()
            .and_then(|d| self.statuses.get(d).cloned())
            .unwrap_or_else(Status::stopped)
    }

    /// A setting from server.toml (as `config --json` gave it).
    fn setting(&self, section: &str, key: &str) -> Option<&Value> {
        self.config
            .as_ref()
            .map(|c| &c[section][key])
            .filter(|v| !v.is_null())
    }

    /// A setting as shown: what was just asked for, else what server.toml holds.
    fn shown(&self, section: &str, key: &str) -> String {
        let full = format!("{section}.{key}");
        if let Some((_, v)) = self.queued.iter().rev().find(|(k, _)| *k == full) {
            return v.clone();
        }
        match self.setting(section, key) {
            Some(Value::String(s)) => s.clone(),
            None => String::new(),
            Some(v) => v.to_string(),
        }
    }
}

/// Stops the servers this launcher started, unless they run in the background as a service.
pub(super) fn stop_on_quit() {
    for dir in STARTED_HERE.lock().unwrap().drain(..) {
        if servers::service_installed(&dir) {
            continue;
        }
        if let Some(api) = servers::Api::open(&dir) {
            let _ = api.call("POST", "/api/v1/stop", None);
        }
    }
}

/// Update closes the window to open the new launcher at once: a server is its own process and the
/// new window finds it running, so an update doesn't end anyone's game. It stays up when that new
/// window closes too, until it is stopped on the Servers page.
pub(super) fn keep_running_through_update() {
    STARTED_HERE.lock().unwrap().clear();
}

/// The player's own name, to let them in on a friends-only server: the Microsoft account that plays,
/// else the name the game made up for them (`eclipse-player.txt`).
fn own_name(accounts: &Accounts) -> Option<String> {
    accounts
        .selected()
        .map(|a| a.name.clone())
        .or_else(crate::offline_name)
}

fn bytes(n: u64) -> String {
    match n {
        0..1_000_000 => format!("{} kB", n.div_ceil(1000)),
        1_000_000..1_000_000_000 => format!("{:.1} MB", n as f64 / 1e6),
        _ => format!("{:.2} GB", n as f64 / 1e9),
    }
}

/// A backup's date and kind from its name (`2026-10-19_11-28-57-manual.zip`, UTC), for people.
fn backup_label(b: &Backup) -> (String, String) {
    let stem = b.name.trim_end_matches(".zip");
    let (when, kind) = match stem.get(..19) {
        Some(when) if stem.len() > 20 => (when, &stem[20..]),
        _ => return (stem.to_string(), String::new()),
    };
    let (date, time) = when.split_once('_').unwrap_or((when, ""));
    let label = format!("{date} {} UTC", time.replace('-', ":"));
    let kind = match kind {
        "auto" => "Automatic".to_string(),
        "manual" => "Made by hand".to_string(),
        "before-update" => "Before an update".to_string(),
        // eclipse-server writes the tag with dashes (before-v2026-10-19-10).
        k if k.starts_with("before-v")
            && k[8..].chars().all(|c| c.is_ascii_digit() || c == '-') =>
        {
            format!("Before updating to {}", k[7..].replace('-', "."))
        }
        k if k.starts_with("before-") => format!("Before updating to {}", &k[7..]),
        k => k.to_string(),
    };
    (label, kind)
}

fn uptime(secs: u64) -> String {
    match secs {
        0..60 => format!("{secs} s"),
        60..3600 => format!("{} min", secs / 60),
        3600..86_400 => format!("{} h {:02} min", secs / 3600, secs / 60 % 60),
        _ => format!("{} d {} h", secs / 86_400, secs / 3600 % 24),
    }
}

impl Launcher {
    // ----- Background work ------------------------------------------------------------------

    /// Runs `job` on a thread with the page busy; its lines and result arrive as events.
    fn server_job(
        &mut self,
        label: impl Into<String>,
        job: impl FnOnce(&dyn Fn(String)) -> Result<String, Problem> + Send + 'static,
    ) {
        if self.servers.busy.is_some() {
            return;
        }
        self.servers.busy = Some(label.into());
        self.servers.job_log.clear();
        self.servers.problem = None;
        self.servers.note = None;
        let tx = self.servers.tx.clone();
        std::thread::spawn(move || {
            let lines = tx.clone();
            let log = move |l: String| {
                let _ = lines.send(Event::Line(l));
            };
            let _ = tx.send(Event::Done(job(&log)));
        });
    }

    /// `eclipse-server <args>` on the open server, as a job.
    fn server_command(&mut self, label: &str, args: Vec<String>) {
        let Some(dir) = self.servers.open.clone() else {
            return;
        };
        self.server_job(label.to_string(), move |log| {
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            servers::run_streamed(&dir, &args, log).map(|_| String::new())
        });
    }

    /// Reads the open server's settings, backups and service state again.
    fn load_details(&self) {
        let Some(dir) = self.servers.open.clone() else {
            return;
        };
        let tx = self.servers.tx.clone();
        std::thread::spawn(move || {
            let config = servers::config(&dir);
            let backups = servers::backups(&dir).unwrap_or_default();
            let service = servers::service_installed(&dir);
            let _ = tx.send(Event::Details {
                dir,
                config,
                backups,
                service,
            });
        });
    }

    pub(super) fn open_server(&mut self, dir: PathBuf) {
        if self.servers.open.as_ref() == Some(&dir) {
            return;
        }
        let s = &mut self.servers;
        s.open = Some(dir);
        // Its settings may have changed while it was open.
        s.modes.clear();
        s.tab = Tab::Overview;
        s.config = None;
        s.backups.clear();
        s.service = None;
        s.update = None;
        s.pvp = None;
        s.problem = None;
        s.note = None;
        s.confirm_restore = None;
        s.confirm_delete = false;
        s.texts.remove(&Field::DeleteServer);
        s.restart_needed.clear();
        s.texts.remove(&Field::Motd);
        s.texts.remove(&Field::Port);
        s.invalid = None;
        s.polled = None;
        self.load_details();
    }

    /// Called with the launcher's other work: what the background sent, and the next status poll.
    pub(super) fn servers_tick(&mut self) -> bool {
        // The page shows how friends reach the server beside it, whatever the tab.
        let overview = self.page == Page::Servers && self.servers.open.is_some();
        let mut changed = self.internet_tick(overview);
        while let Ok(event) = self.servers.rx.try_recv() {
            changed = true;
            self.servers_event(event);
        }
        if self.page != Page::Servers || self.servers.polling || !self.servers.loaded {
            return changed;
        }
        let every = if self.servers.open.is_some() {
            POLL_OPEN
        } else {
            POLL_LIST
        };
        if self.servers.polled.is_some_and(|at| at.elapsed() < every) {
            return changed;
        }
        self.servers.polled = Some(Instant::now());
        self.servers.polling = true;
        let tx = self.servers.tx.clone();
        match self.servers.open.clone() {
            Some(dir) => {
                let since = if self.servers.log_dir.as_ref() == Some(&dir) {
                    self.servers.log_next
                } else {
                    0
                };
                std::thread::spawn(move || {
                    let api = servers::Api::open(&dir);
                    let status = api
                        .as_ref()
                        .and_then(|a| a.status())
                        .unwrap_or_else(Status::stopped);
                    let logs = api.and_then(|a| a.logs(since));
                    let _ = tx.send(Event::Polled {
                        dir,
                        status: Box::new(status),
                        logs,
                    });
                });
            }
            None => {
                let dirs: Vec<PathBuf> = self.servers.list.iter().map(|e| e.dir.clone()).collect();
                std::thread::spawn(move || {
                    let all = dirs
                        .into_iter()
                        .map(|d| {
                            let s = servers::status(&d);
                            (d, s)
                        })
                        .collect();
                    let _ = tx.send(Event::Statuses(all));
                });
            }
        }
        changed
    }

    fn servers_event(&mut self, event: Event) {
        let s = &mut self.servers;
        match event {
            Event::Polled { dir, status, logs } => {
                s.polling = false;
                let was = s.statuses.get(&dir).map(|st| st.phase.clone());
                if was.as_deref() != Some("running") && status.phase == "running" {
                    // A fresh start took every saved setting.
                    s.restart_needed.clear();
                    s.pvp = None;
                }
                if let Some((next, lines)) = logs {
                    if s.log_dir.as_ref() != Some(&dir) {
                        s.log.clear();
                        s.log_dir = Some(dir.clone());
                    }
                    if !lines.is_empty() {
                        s.log.extend(lines);
                        while s.log.len() > CONSOLE_KEEP {
                            s.log.pop_front();
                        }
                        s.console_scroll.scroll_to_bottom();
                    }
                    s.log_next = next;
                }
                let read_pvp = status.phase == "running" && s.pvp.is_none();
                s.statuses.insert(dir, *status);
                if read_pvp {
                    self.read_pvp();
                }
            }
            Event::Statuses(all) => {
                s.polling = false;
                s.statuses.extend(all);
            }
            Event::Line(line) => {
                s.job_log.push_back(line);
                while s.job_log.len() > 6 {
                    s.job_log.pop_front();
                }
            }
            Event::Done(result) => {
                s.busy = None;
                s.polled = None;
                match result {
                    Ok(note) if !note.is_empty() => s.note = Some(note),
                    Ok(_) => {}
                    Err(p) => s.problem = Some(p),
                }
                self.load_details();
            }
            Event::Created(result) => {
                s.creating = false;
                match result {
                    Ok(entry) => {
                        s.create = None;
                        s.list = servers::list();
                        s.modes.clear();
                        s.modes.clear();
                        self.open_server(entry.dir.clone());
                        self.servers.note = Some(format!(
                            "{} is ready. Start it and share the address with friends.",
                            entry.name
                        ));
                    }
                    Err(p) => s.problem = Some(p),
                }
            }
            Event::Details {
                dir,
                config,
                backups,
                service,
            } => {
                if s.open.as_ref() == Some(&dir) {
                    s.config = config;
                    s.backups = backups;
                    s.service = Some(service);
                }
            }
            Event::Deleted { dir, result } => {
                s.busy = None;
                s.confirm_delete = false;
                s.list = servers::list();
                s.modes.clear();
                s.statuses.remove(&dir);
                STARTED_HERE
                    .lock()
                    .unwrap()
                    .retain(|started| started != &dir);
                if !s.list.iter().any(|entry| entry.dir == dir) {
                    s.open = None;
                    self.side_servers = s.list.clone();
                }
                match result {
                    Ok(()) => s.note = Some("Server moved to the trash.".into()),
                    Err(e) => s.problem = Some(Problem::plain(e)),
                }
            }
            Event::Applied(result) => {
                s.applying = false;
                match result {
                    Ok(applied) if applied.running => s.restart_needed.extend(applied.restart),
                    Ok(_) => {}
                    Err(p) => s.problem = Some(p),
                }
                if s.queued.is_empty() {
                    self.load_details();
                } else {
                    self.apply_settings();
                }
            }
            Event::Pvp(on) => s.pvp = on,
            Event::UpdateCheck(result) => {
                s.checking = false;
                match result {
                    Ok(v) => s.update = Some(v),
                    Err(p) => s.problem = Some(p),
                }
            }
            Event::Failed(e) => s.problem = Some(Problem::plain(e)),
        }
    }

    /// The `pvp` game rule, asked of the running game (it's a rule of the world, not a setting).
    fn read_pvp(&self) {
        let Some(dir) = self.servers.open.clone() else {
            return;
        };
        let tx = self.servers.tx.clone();
        std::thread::spawn(move || {
            let on = servers::Api::open(&dir)
                .and_then(|a| a.command("gamerule pvp").ok())
                .and_then(|out| {
                    let text = out.join(" ");
                    let text = text.trim_end();
                    if text.ends_with("true") {
                        Some(true)
                    } else if text.ends_with("false") {
                        Some(false)
                    } else {
                        None
                    }
                });
            let _ = tx.send(Event::Pvp(on));
        });
    }

    /// Queues a setting for `eclipse-server config set`; they go one batch at a time.
    fn set_setting(&mut self, key: &str, value: String) {
        self.servers.queued.retain(|(k, _)| k != key);
        self.servers.queued.push((key.to_string(), value));
        self.servers.note = None;
        if !self.servers.applying {
            self.apply_settings();
        }
    }

    fn apply_settings(&mut self) {
        let Some(dir) = self.servers.open.clone() else {
            return;
        };
        let batch = std::mem::take(&mut self.servers.queued);
        if batch.is_empty() {
            return;
        }
        self.servers.applying = true;
        // Shown as asked until the file is read back.
        if let Some(config) = &mut self.servers.config {
            for (key, value) in &batch {
                if let Some((section, field)) = key.split_once('.') {
                    let new = match &config[section][field] {
                        Value::Bool(_) => Value::Bool(value == "true"),
                        Value::Number(_) => value.parse::<i64>().map_or(Value::Null, Value::from),
                        _ => Value::String(value.clone()),
                    };
                    config[section][field] = new;
                }
            }
        }
        let tx = self.servers.tx.clone();
        std::thread::spawn(move || {
            let pairs: Vec<(&str, String)> =
                batch.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
            let _ = tx.send(Event::Applied(servers::set_config(&dir, &pairs)));
        });
    }

    /// A command for the running game, through the control API; its answer shows in the console.
    fn api_command(&mut self, line: String) {
        let Some(dir) = self.servers.open.clone() else {
            return;
        };
        let tx = self.servers.tx.clone();
        self.servers.log.push_back(format!("> {line}"));
        self.servers.console_scroll.scroll_to_bottom();
        std::thread::spawn(move || {
            let result = servers::Api::open(&dir)
                .ok_or_else(|| "The server isn't running.".to_string())
                .and_then(|a| a.command(&line));
            if let Err(e) = result {
                let _ = tx.send(Event::Failed(e));
            }
        });
    }

    fn start_server(&mut self) {
        let Some(dir) = self.servers.open.clone() else {
            return;
        };
        STARTED_HERE.lock().unwrap().push(dir.clone());
        self.servers.restart_needed.clear();
        self.server_job("Starting", move |log| {
            log("A new world takes a minute the first time".into());
            servers::run_streamed(&dir, &["start"], log).map(|_| String::new())
        });
    }

    fn fix(&mut self, fix: Fix, cx: &mut Context<Self>) {
        self.servers.problem = None;
        match fix {
            Fix::UsePort(port) => {
                self.set_setting("server.port", port.to_string());
                self.servers.texts.remove(&Field::Port);
                self.servers.note = Some(format!(
                    "The server now uses port {port}. Press Start again."
                ));
            }
            Fix::Prepare => match self.selected().cloned() {
                Some(tag) if backend::is_prepared(&tag) => {
                    self.server_job("Copying the game files", move |log| {
                        log(format!("Copying the game files prepared for Eclipse {tag}"));
                        servers::set_up(&tag)
                            .map(|_| "The game files are ready. Press Start again.".to_string())
                            .map_err(Problem::plain)
                    });
                }
                _ => {
                    self.servers.create = None;
                    self.go(Page::Play);
                    if !self.preparing {
                        self.prepare(backend::MinecraftFrom::Install);
                    }
                }
            },
            Fix::AgreeEula => {
                if let Some(c) = &mut self.servers.create {
                    c.eula = true;
                    c.eula_missing = false;
                } else if let Some(dir) = &self.servers.open {
                    if let Err(e) = backend::agree_eula(dir) {
                        self.servers.problem = Some(Problem::plain(e));
                    }
                }
            }
            Fix::ShowLog => {
                self.servers.tab = Tab::Overview;
                if let Some(dir) = &self.servers.open {
                    crate::glass_toast::open(cx, &dir.join("logs"));
                }
            }
        }
    }

    // ----- Creating -------------------------------------------------------------------------

    fn open_create(&mut self, cx: &mut Context<Self>) {
        self.focus_handle(Field::Name, cx);
        self.servers.focus_soon = Some(Field::Name);
        let worlds = servers::player_worlds();
        // The dialog starts with an empty name (its placeholder says the default) and the player
        // as the first friend.
        let own = own_name(&self.accounts).unwrap_or_default();
        self.servers.text(Field::Name).set(String::new());
        self.servers.selected = None;
        self.servers.text(Field::Seed).set(String::new());
        self.servers.text(Field::Players).set(String::new());
        let friends = if own.is_empty() {
            Vec::new()
        } else {
            vec![own]
        };
        let details = eclipse_launcher::worlds::list(&backend::game_home())
            .into_iter()
            .map(|w| {
                let line = format!("Eclipse · {} · {}", tr(w.mode), bytes(w.size));
                (w.path, (w.name, line))
            })
            .collect();
        self.servers.problem = None;
        self.servers.create_scroll = Scrollbar::default();
        self.servers.create = Some(Create {
            new_world: true,
            copy: worlds.first().map(|w| w.1.clone()),
            worlds,
            mode: "survival",
            difficulty: "normal",
            max_players: 10,
            friends_only: true,
            check_accounts: true,
            eula: false,
            eula_missing: false,
            friends,
            details,
        });
    }

    fn create_server(&mut self) {
        if self.servers.creating {
            return;
        }
        let Some(c) = &mut self.servers.create else {
            return;
        };
        if !c.eula {
            c.eula_missing = true;
            return;
        }
        let Some(c) = self.servers.create.as_ref() else {
            return;
        };
        let world = match (&c.copy, c.new_world) {
            (Some(dir), false) => World::Copy(dir.clone()),
            _ => World::New {
                seed: self.servers.value(Field::Seed).to_string(),
            },
        };
        let mut players = c.friends.clone();
        for p in self.servers.value(Field::Players).split([',', ' ']) {
            if !p.is_empty() && !players.iter().any(|q| q == p) {
                players.push(p.to_string());
            }
        }
        let name = self.servers.value(Field::Name).trim().to_string();
        let spec = NewServer {
            name: if !name.is_empty() {
                name
            } else if self.servers.list.is_empty() {
                "My server".into()
            } else {
                format!("Server {}", self.servers.list.len() + 1)
            },
            world,
            game_mode: c.mode.into(),
            difficulty: c.difficulty.into(),
            max_players: c.max_players,
            friends_only: c.friends_only,
            check_accounts: c.check_accounts,
            players,
            eula: c.eula,
        };
        let tag = self.selected().cloned();
        let tx = self.servers.tx.clone();
        self.servers.creating = true;
        self.servers.problem = None;
        self.servers.job_log.clear();
        std::thread::spawn(move || {
            let lines = tx.clone();
            let log = move |l: String| {
                let _ = lines.send(Event::Line(l));
            };
            let _ = tx.send(Event::Created(servers::create(&spec, tag.as_deref(), &log)));
        });
    }

    // ----- Building blocks ------------------------------------------------------------------

    fn focus_handle(&mut self, f: Field, cx: &mut Context<Self>) -> FocusHandle {
        self.servers
            .focus
            .entry(f)
            .or_insert_with(|| cx.focus_handle().tab_index(0).tab_stop(true))
            .clone()
    }

    /// A one-line text field: typing, paste, Backspace, Enter to confirm, Escape, Tab to the next.
    pub(super) fn field(
        &mut self,
        f: Field,
        placeholder: &'static str,
        width: f32,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let t = self.theme;
        let handle = self.focus_handle(f, cx);
        let focused = self.servers.focused == Some(f);
        let selected = focused && self.servers.selected == Some(f);
        let invalid = self.servers.invalid.is_some_and(|(g, _)| g == f);
        self.servers.text(f).show(((width - 24.) / 7.5) as usize);
        // The mods filter and the log search sit by buttons: small fields, as tall as they are.
        let base = crate::glass_ui::field(focused, invalid)
            .when(
                matches!(
                    f,
                    Field::InstanceModsFilter
                        | Field::ConsoleSearch
                        | Field::Motd
                        | Field::Port
                        | Field::Token
                        | Field::AddPlayer
                        | Field::DeleteServer
                ),
                |d| {
                    d.h(px(ui::Size::Md.height()))
                        .px(px(12.))
                        .rounded(px(radius::CONTROL))
                },
            )
            // The instance and server dialogs' are large, with larger text.
            .when(
                !invalid
                    && matches!(
                        f,
                        Field::InstanceName
                            | Field::InstanceGroup
                            | Field::InstanceVersion
                            | Field::Name
                            | Field::Seed
                    ),
                |d| d.h(px(48.)).text_size(px(step::LEAD.size)),
            )
            .when(f == Field::AccentHex, |d| {
                d.h(px(ui::Size::Md.height()))
                    .px(px(12.))
                    .rounded(px(radius::CONTROL))
                    .text_size(px(step::NOTE.size))
                    .font_weight(FontWeight::MEDIUM)
            })
            // The friend field sits in its chip, on the chip's fill.
            .when(f == Field::Players, |d| {
                d.h_full()
                    .px(px(14.))
                    .border_0()
                    .bg(gpui::transparent_black())
            })
            // The server's command line: the field is the line's middle, on its fill, in mono.
            .when(f == Field::Command, |d| {
                d.h_full()
                    .px_0()
                    .rounded_none()
                    .border_0()
                    .bg(gpui::transparent_black())
                    .font_family(theme::MONO)
                    .text_size(px(step::NOTE.size))
            })
            // The focus ring on a field of its own, drawn inside its edge: GPUI has no outline.
            .when(
                focused && !invalid && !matches!(f, Field::Players | Field::Command),
                |d| {
                    let small = matches!(
                        f,
                        Field::InstanceModsFilter
                            | Field::ConsoleSearch
                            | Field::AccentHex
                            | Field::Motd
                            | Field::Port
                            | Field::Token
                            | Field::AddPlayer
                            | Field::DeleteServer
                    );
                    d.child(crate::glass_ui::focus_ring(if small {
                        radius::CONTROL
                    } else {
                        radius::ROW
                    }))
                },
            );
        let field = base
            .id(SharedString::from(format!("field-{f:?}")))
            .track_focus(&handle)
            .w(px(width))
            .cursor_text()
            .child(line_edit::content(
                self.servers.text(f),
                &t,
                focused,
                selected,
                f == Field::Token,
                placeholder,
            ))
            .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
                // A click puts the caret in: the suggestion is no longer selected.
                if matches!(ev, ClickEvent::Mouse(_)) {
                    this.servers.selected = None;
                }
                if let Some(h) = this.servers.focus.get(&f) {
                    window.focus(h, cx);
                }
                cx.notify();
            }))
            .on_key_down(cx.listener(move |this, ev: &KeyDownEvent, window, cx| {
                if ev.keystroke.key != "enter" && this.servers.invalid.is_some_and(|(g, _)| g == f)
                {
                    this.servers.invalid = None;
                }
                // A suggestion is selected: what's typed or pasted replaces it.
                let selected = this.servers.selected.take() == Some(f);
                if selected {
                    this.servers.text(f).select_all();
                }
                match line_edit::handle(this.servers.text(f), ev, cx) {
                    Key::Enter if matches!(f, Field::InstanceName | Field::InstanceGroup) => {
                        this.save_instance(cx)
                    }
                    Key::Enter => this.field_enter(f),
                    Key::Escape => {
                        this.servers.confirm_delete = false;
                        if this.servers.create.is_some() && !this.servers.creating {
                            this.servers.create = None;
                        }
                        this.instance_escape();
                        window.blur(cx);
                    }
                    // Tab and the rest go on to the window: Tab moves the focus.
                    Key::Pass => return,
                    _ => {}
                }
                cx.stop_propagation();
                cx.notify();
            }));
        line_edit::mouse(field, cx, move |this| this.servers.text(f))
    }

    /// Escape closes the Create dialog. Whether it did.
    pub(super) fn servers_escape(&mut self) -> bool {
        if self.page == Page::Servers && self.servers.confirm_delete {
            self.servers.confirm_delete = false;
            return true;
        }
        if self.page != Page::Servers || self.servers.create.is_none() || self.servers.creating {
            return false;
        }
        self.servers.create = None;
        self.servers.problem = None;
        true
    }

    fn field_enter(&mut self, f: Field) {
        match f {
            // The dialog's friend field adds the name typed.
            Field::Players => self.add_create_friend(),
            Field::Name | Field::Seed => self.create_server(),
            Field::Command => {
                let line = self.servers.text(Field::Command).take();
                let line = line.trim().trim_start_matches('/').to_string();
                if !line.is_empty() {
                    self.api_command(line);
                }
            }
            Field::AddPlayer => self.add_player(),
            Field::Motd => {
                let motd = self.servers.value(Field::Motd).trim().to_string();
                if motd.is_empty() {
                    self.servers.problem =
                        Some(Problem::plain("Type the line players see under the name."));
                } else {
                    self.set_setting("server.motd", motd);
                }
            }
            Field::Port => self.save_port(),
            Field::Token => self.save_token(),
            Field::Email => self.invite_friend(),
            Field::InstanceName
            | Field::InstanceGroup
            | Field::InstanceVersion
            | Field::InstanceModsFilter
            | Field::ConsoleSearch
            | Field::DeleteServer
            | Field::AccentHex => {}
            Field::PreLaunch | Field::Wrapper | Field::LaunchEnv | Field::PostExit => {
                self.save_launch()
            }
            Field::MemoryLimit => self.memory_enter(),
            Field::OfflineName => self.offline_name_enter(),
            Field::ServerName | Field::ServerAddress => self.add_server(),
        }
    }

    fn save_port(&mut self) {
        match self.servers.value(Field::Port).trim().parse::<u16>() {
            Ok(port) if port >= 1024 => self.set_setting("server.port", port.to_string()),
            _ => {
                self.servers.invalid = Some((
                    Field::Port,
                    "Use a number from 1024 to 65535 (25565 is Minecraft's own).",
                ))
            }
        }
    }

    fn add_player(&mut self) {
        let name = self.servers.value(Field::AddPlayer).trim().to_string();
        let valid = (3..=16).contains(&name.len())
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !valid {
            self.servers.invalid = Some((
                Field::AddPlayer,
                "A Minecraft name has 3 to 16 letters, digits or underscores.",
            ));
            return;
        }
        self.servers.text(Field::AddPlayer).set(String::new());
        self.server_command(
            &format!("Letting {name} in"),
            vec!["whitelist".into(), "add".into(), name],
        );
    }

    /// A player's face, from the default skin their UUID picks (the server knows no other).
    pub(super) fn head(&mut self, uuid: &str, size: f32) -> Div {
        let resources = servers::game_files(&servers::runtime_root(), "26.3");
        let face = self
            .servers
            .faces
            .entry(uuid.to_string())
            .or_insert_with(|| heads::default_face(&resources, uuid))
            .clone();
        let cell = size / 8.;
        let mut d = div()
            .flex_none()
            .flex()
            .flex_col()
            .size(px(size))
            .rounded(px(radius::ROW))
            .overflow_hidden()
            .bg(ui::white(fill::BUTTON));
        if let Some(pixels) = face {
            for row in pixels.chunks(8) {
                d = d.child(div().flex().children(row.iter().map(|p| {
                    div().size(px(cell)).bg(Hsla::from(gpui::Rgba {
                        r: p[0] as f32 / 255.,
                        g: p[1] as f32 / 255.,
                        b: p[2] as f32 / 255.,
                        a: 1.,
                    }))
                })));
            }
        }
        d
    }

    /// A row's small button.
    fn small(
        &self,
        id: impl Into<SharedString>,
        label: &'static str,
        enabled: bool,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self) + 'static,
    ) -> Stateful<Div> {
        let b = ui::button(id, None, tr(label), Kind::Ghost, ui::Size::Sm);
        if enabled {
            b.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                action(this);
                cx.notify();
            }))
        } else {
            crate::disabled(b)
        }
    }

    // ----- The page -------------------------------------------------------------------------

    pub(super) fn servers_page(&mut self, cx: &mut Context<Self>) -> Div {
        if !self.servers.loaded {
            self.servers.list = servers::list();
            self.servers.modes.clear();
            self.servers.loaded = true;
            if self.servers.open.is_none() && self.servers.list.len() == 1 {
                let dir = self.servers.list[0].dir.clone();
                self.open_server(dir);
            }
        }
        self.glass_servers_page(cx)
    }

    fn add_server_folder(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Add".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(mut paths))) = paths.await {
                if let Some(dir) = paths.pop() {
                    let _ = this.update(cx, |this, cx| {
                        this.add_existing(dir);
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    /// A folder with a server.toml joins the list; a vanilla server folder is set up first (its
    /// server.properties becomes server.toml; its world, lists and EULA stay).
    fn add_existing(&mut self, dir: PathBuf) {
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Server".into());
        let tag = self.selected().cloned();
        let tx = self.servers.tx.clone();
        self.servers.creating = true;
        self.servers.problem = None;
        std::thread::spawn(move || {
            let result = (|| {
                if servers::program().is_none() {
                    let tag =
                        tag.ok_or_else(|| Problem::plain("Install an Eclipse version first."))?;
                    servers::set_up(&tag).map_err(Problem::plain)?;
                }
                if !dir.join("server.toml").is_file() {
                    servers::run(&dir, &["init", "--yes"])?;
                }
                let entry = Entry { name, dir };
                servers::add(entry.clone()).map_err(Problem::plain)?;
                Ok(entry)
            })();
            let _ = tx.send(Event::Created(result));
        });
    }

    fn players_tab(&mut self, dir: &Path, status: &Status, cx: &mut Context<Self>) -> Div {
        let running = status.running();
        let idle = self.servers.busy.is_none();
        let ops = servers::names(dir, "ops.json");
        let whitelist = servers::entries(dir, "whitelist.json");
        let banned = servers::names(dir, "banned-players.json");
        let is_op = |n: &str| ops.iter().any(|o| o.eq_ignore_ascii_case(n));
        let mut tab = div().flex().flex_col().gap(px(28.));

        // Online now.
        let mut online = div().flex().flex_col();
        for (i, p) in status.players.clone().into_iter().enumerate() {
            let op = is_op(&p.name);
            let (n1, n2, n3) = (p.name.clone(), p.name.clone(), p.name.clone());
            let actions = div()
                .flex()
                .flex_none()
                .gap(px(2.))
                .child(self.small(
                    format!("online-op-{i}"),
                    if op {
                        "Remove operator"
                    } else {
                        "Make operator"
                    },
                    idle,
                    cx,
                    move |this| {
                        let verb = if op { "deop" } else { "op" };
                        this.server_command("Changing operators", vec![verb.into(), n1.clone()]);
                    },
                ))
                .child(self.small(
                    format!("online-kick-{i}"),
                    "Kick",
                    running,
                    cx,
                    move |this| this.api_command(format!("kick {n2}")),
                ))
                .child(
                    self.small(format!("online-ban-{i}"), "Ban", running, cx, move |this| {
                        this.api_command(format!("ban {n3}"))
                    }),
                );
            let ping = p
                .ping
                .map(|ms| trf("{ms} ms ping", &[("ms", &ms.to_string())]));
            let about = match (op, ping) {
                (true, Some(ping)) => Some(trf("Operator · {ping}", &[("ping", &ping)])),
                (true, None) => Some(tr("Operator").to_string()),
                (false, ping) => ping,
            };
            let face = self.head(&p.uuid, FACE);
            online = online.child(self.list_row(
                Some(face),
                p.name.clone(),
                about,
                Some(actions.into_any_element()),
            ));
        }
        tab = tab.child(
            div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(ui::label(trf(
                    "Online now ({count})",
                    &[("count", &status.players.len().to_string())],
                )))
                .child(if status.players.is_empty() {
                    ui::text(step::BODY, FontWeight::NORMAL, ink::MUTED)
                        .child(if running {
                            tr("Nobody is playing right now.")
                        } else {
                            tr("The server is stopped.")
                        })
                        .into_any_element()
                } else {
                    online.into_any_element()
                }),
        );

        // Who may join.
        let friends_only = self
            .servers
            .setting("server", "whitelist")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let friends = ui::toggle_row(
            "friends-only",
            tr("Friends only"),
            Some(
                div()
                    .child(if friends_only {
                        tr("On: only players on the list can join.")
                    } else {
                        tr("Off: anyone who has the address can join.")
                    })
                    .into_any_element(),
            ),
            friends_only,
        )
        .mx(px(-ui::SETTING_ROW_PADDING))
        .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.set_setting("server.whitelist", (!friends_only).to_string());
            cx.notify();
        }));
        let let_in = ui::button(
            "player-add",
            None,
            tr("Let in"),
            Kind::Secondary,
            ui::Size::Md,
        );
        let add = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(self.field(Field::AddPlayer, tr("A player's name"), 240., cx))
            .child(if idle {
                let_in.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.add_player();
                    cx.notify();
                }))
            } else {
                crate::disabled(let_in)
            });
        let mut people: Vec<(String, String)> = whitelist.clone();
        for op in &ops {
            if !people.iter().any(|(n, _)| n.eq_ignore_ascii_case(op)) {
                people.push((op.clone(), String::new()));
            }
        }
        let mut allowed = div().flex().flex_col();
        for (i, (name, uuid)) in people.into_iter().enumerate() {
            let op = is_op(&name);
            let listed = whitelist.iter().any(|(n, _)| n == &name);
            let (n1, n2) = (name.clone(), name.clone());
            let mut actions = div().flex().flex_none().gap(px(2.)).child(self.small(
                format!("allowed-op-{i}"),
                if op {
                    "Remove operator"
                } else {
                    "Make operator"
                },
                idle,
                cx,
                move |this| {
                    let verb = if op { "deop" } else { "op" };
                    this.server_command("Changing operators", vec![verb.into(), n1.clone()]);
                },
            ));
            if listed {
                actions = actions.child(self.small(
                    format!("allowed-remove-{i}"),
                    "Remove",
                    idle,
                    cx,
                    move |this| {
                        this.server_command(
                            "Updating the list",
                            vec!["whitelist".into(), "remove".into(), n2.clone()],
                        );
                    },
                ));
            }
            let face = if uuid.is_empty() {
                div().size(px(FACE)).flex_none()
            } else {
                self.head(&uuid, FACE)
            };
            let about = match (op, listed) {
                (true, true) => Some(tr("Operator: can run every command").to_string()),
                (true, false) => {
                    Some(tr("Operator (not on the list: can join anyway)").to_string())
                }
                _ => None,
            };
            allowed = allowed.child(self.list_row(
                Some(face),
                name,
                about,
                Some(actions.into_any_element()),
            ));
        }
        let refused = self.servers.invalid.and_then(|(f, e)| {
            (f == Field::AddPlayer).then(|| ui::card_note(tr(e)).text_color(ui::danger()))
        });
        tab = tab.child(
            div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(ui::label(tr("Who may join")))
                .child(friends)
                .child(add)
                .children(refused)
                .child(allowed),
        );

        // Banned.
        if !banned.is_empty() {
            let mut list = div().flex().flex_col();
            for (i, name) in banned.into_iter().enumerate() {
                let n = name.clone();
                let unban = self.small(format!("unban-{i}"), "Unban", running, cx, move |this| {
                    this.api_command(format!("pardon {n}"))
                });
                list = list.child(self.list_row(None, name, None, Some(unban.into_any_element())));
            }
            tab = tab.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(ui::label(tr("Banned")))
                    .when(!running, |d| {
                        d.child(ui::card_note(tr("Start the server to unban someone.")))
                    })
                    .child(list),
            );
        }
        tab
    }

    /// A row of the settings form: a name, one line on it, and its control.
    fn setting_row(&self, name: &str, about: String, control: AnyElement) -> Div {
        self.setting_row_checked(name, about, None, control)
    }

    /// A setting row whose text gives way to `error` when the value was refused.
    fn setting_row_checked(
        &self,
        name: &str,
        about: String,
        error: Option<&'static str>,
        control: AnyElement,
    ) -> Div {
        let note = match error {
            Some(e) => div().text_color(ui::danger()).child(tr(e)),
            None => div().child(about),
        };
        ui::setting_row(
            SharedString::from(tr(name).to_string()),
            Some(note.into_any_element()),
            Some(control),
        )
    }

    fn switch_row(
        &self,
        id: &'static str,
        name: &'static str,
        about: String,
        on: bool,
        press: Option<impl Fn(&ClickEvent, &mut Window, &mut App) + 'static>,
    ) -> AnyElement {
        let Some(press) = press else {
            let switch = crate::disabled(ui::toggle(id, on));
            return self
                .setting_row(name, about, switch.into_any_element())
                .into_any_element();
        };
        ui::toggle_row(
            id,
            tr(name),
            Some(div().child(about).into_any_element()),
            on,
        )
        .on_press(press)
        .into_any_element()
    }

    /// − value + for a number setting within `range`.
    fn stepper(
        &self,
        key: &'static str,
        range: std::ops::RangeInclusive<i64>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (section, field) = key.split_once('.').unwrap_or_default();
        let value: i64 = self.servers.shown(section, field).parse().unwrap_or(0);
        self.glass_stepper(key, value, range, cx, move |this, to| {
            this.set_setting(key, to.to_string())
        })
    }

    fn choice(
        &self,
        id: &'static str,
        key: &'static str,
        options: &[(&'static str, &'static str)],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (section, field) = key.split_once('.').unwrap_or_default();
        let current = self.servers.shown(section, field);
        let current: &'static str = options
            .iter()
            .find(|(v, _)| *v == current)
            .map_or("", |(v, _)| v);
        let this = cx.entity().downgrade();
        ui::segments(id, options, current, move |value, _, cx| {
            let _ = this.update(cx, |this, cx| {
                this.set_setting(key, value.to_string());
                cx.notify();
            });
        })
        .into_any_element()
    }

    fn settings_tab(&mut self, status: &Status, cx: &mut Context<Self>) -> Div {
        let running = status.running();
        if self.servers.config.is_none() {
            return div().child(if self.older_eclipse() {
                ui::text(step::BODY, FontWeight::NORMAL, ink::MUTED)
                    .child(tr(
                        "This server's Eclipse is too old to show its settings here. Update it on \
                     the Backups tab, then they appear.",
                    ))
                    .into_any_element()
            } else {
                ui::loading_state("server-settings-loading", tr("Reading the settings…"))
                    .into_any_element()
            });
        }
        if self.servers.focused != Some(Field::Motd)
            && !self.servers.texts.contains_key(&Field::Motd)
        {
            let motd = self.servers.shown("server", "motd");
            self.servers.text(Field::Motd).set(motd);
        }
        if self.servers.focused != Some(Field::Port)
            && !self.servers.texts.contains_key(&Field::Port)
        {
            let port = self.servers.shown("server", "port");
            self.servers.text(Field::Port).set(port);
        }
        let save = |id: &'static str, f: Field, cx: &mut Context<Self>| {
            button(id, "Save", Kind::Secondary).on_press(cx.listener(
                move |this, _: &ClickEvent, _, cx| {
                    this.field_enter(f);
                    cx.notify();
                },
            ))
        };
        let motd = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(self.field(Field::Motd, tr("A Minecraft Server"), 260., cx))
            .child(save("motd-save", Field::Motd, cx))
            .into_any_element();
        let port = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(self.field(Field::Port, "25565", 100., cx))
            .child(save("port-save", Field::Port, cx))
            .into_any_element();
        let pvp_known = self.servers.pvp.filter(|_| running);
        let pvp = self.switch_row(
            "pvp",
            "Players can hurt each other (PvP)",
            if running {
                tr("Applies at once.").into()
            } else {
                tr("A rule of the world: start the server to change it.").into()
            },
            pvp_known.unwrap_or(true),
            pvp_known.map(|on| {
                cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.servers.pvp = Some(!on);
                    this.api_command(format!("gamerule pvp {}", !on));
                    cx.notify();
                })
            }),
        );
        let accounts = self
            .servers
            .setting("server", "online_mode")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let live = if running {
            tr("Applies at once.")
        } else {
            tr("Used from the next start.")
        };
        let later = if running {
            tr("Saved now, used after a restart.")
        } else {
            tr("Used from the next start.")
        };
        let accounts = self.switch_row(
            "online-mode",
            "Require Microsoft sign-in",
            if accounts {
                format!(
                    "{} {later}",
                    tr("On: players must own Minecraft and sign in with Microsoft.")
                )
            } else {
                format!(
                    "{} {later}",
                    tr("Off: anyone can play under any name, so keep Friends only on.")
                )
            },
            accounts,
            Some(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.set_setting("server.online_mode", (!accounts).to_string());
                cx.notify();
            })),
        );
        div()
            .flex()
            .flex_col()
            .mx(px(-ui::SETTING_ROW_PADDING))
            .child(self.setting_row("Message in the server list", later.into(), motd))
            .child(self.setting_row(
                "Game mode",
                format!("{} {live}", tr("For players who join for the first time.")),
                self.choice("set-mode", "world.game_mode", &MODES, cx),
            ))
            .child(self.setting_row(
                "Difficulty",
                live.into(),
                self.choice("set-difficulty", "world.difficulty", &DIFFICULTIES, cx),
            ))
            .child(pvp)
            .child(self.setting_row(
                "Max players",
                format!("{} {later}", tr("Operators can always join.")),
                self.stepper("server.max_players", 1..=100, cx),
            ))
            .child(self.setting_row(
                "View distance",
                format!("{} {later}", tr("In chunks, 2 to 32. Lower runs lighter.")),
                self.stepper("server.view_distance", 2..=32, cx),
            ))
            .child(self.setting_row(
                "Simulation distance",
                format!(
                    "{} {later}",
                    tr("How far from players crops grow and mobs move, in chunks.")
                ),
                self.stepper("server.simulation_distance", 2..=32, cx),
            ))
            .child(
                self.setting_row_checked(
                    "Port",
                    format!(
                        "{} {later}",
                        tr("1024 to 65535. Friends type it after the address unless it's 25565.")
                    ),
                    self.servers
                        .invalid
                        .and_then(|(f, e)| (f == Field::Port).then_some(e)),
                    port,
                ),
            )
            .child(accounts)
            .child(self.background_row(cx))
            .child(self.delete_server_section(status, cx))
    }

    fn delete_server_section(&mut self, status: &Status, cx: &mut Context<Self>) -> Div {
        let name = self
            .servers
            .list
            .iter()
            .find(|entry| Some(&entry.dir) == self.servers.open.as_ref())
            .map(|entry| entry.name.clone())
            .unwrap_or_default();
        let ready = !status.running()
            && self.servers.busy.is_none()
            && !self.servers.applying
            && self.servers.queued.is_empty()
            && self.servers.service == Some(false);
        let hint = if status.running() {
            Some(tr("Stop the server before deleting it."))
        } else if self.servers.service == Some(true) {
            Some(tr("Turn off running in the background before deleting it."))
        } else if !ready {
            Some(tr("Wait for the current operation to finish."))
        } else {
            None
        };
        let mut controls = div()
            .flex()
            .flex_col()
            .items_end()
            .flex_none()
            .w(px(DELETE_CONTROLS))
            .gap(px(12.));
        if self.servers.confirm_delete {
            let matches = self.servers.value(Field::DeleteServer) == name && !name.is_empty();
            let field = self.field(Field::DeleteServer, tr("Server name"), 225., cx);
            let confirm = button("server-delete-confirm", "Move to trash", Kind::Secondary)
                .text_color(ui::danger());
            let confirm = if ready && matches {
                confirm.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    if let Some(dir) = this.servers.open.clone() {
                        this.servers.busy = Some("Moving server to trash".into());
                        let tx = this.servers.tx.clone();
                        std::thread::spawn(move || {
                            let result = servers::delete(&dir);
                            let _ = tx.send(Event::Deleted { dir, result });
                        });
                    }
                    cx.notify();
                }))
            } else {
                crate::disabled(confirm)
            };
            let line = tr("Type {name} to confirm.");
            let (before, after) = line.split_once("{name}").unwrap_or((line, ""));
            let confirm_text = format!("{before}{name}{after}");
            let lit = before.len()..before.len() + name.len();
            controls = controls
                .child(field)
                .child(ui::text(step::NOTE, FontWeight::NORMAL, ink::MUTED).child(
                    gpui::StyledText::new(confirm_text).with_highlights([(
                        lit,
                        gpui::HighlightStyle {
                            font_weight: Some(FontWeight::BOLD),
                            ..Default::default()
                        },
                    )]),
                ))
                .child(
                    div()
                        .flex()
                        .gap(px(8.))
                        .child(
                            button("server-delete-cancel", "Cancel", Kind::Ghost).on_press(
                                cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.servers.confirm_delete = false;
                                    cx.notify();
                                }),
                            ),
                        )
                        .child(confirm),
                );
        } else {
            let delete =
                button("server-delete", "Delete server…", Kind::Secondary).text_color(ui::danger());
            let delete = if ready {
                delete.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.servers.confirm_delete = true;
                    this.servers.text(Field::DeleteServer).set(String::new());
                    this.focus_handle(Field::DeleteServer, cx);
                    this.servers.focus_soon = Some(Field::DeleteServer);
                    cx.notify();
                }))
            } else {
                crate::disabled(delete)
            };
            controls = controls.child(delete);
        }
        if let Some(hint) = hint {
            controls = controls.child(
                ui::card_note(hint)
                    .w(px(DELETE_CONTROLS))
                    .text_align(gpui::TextAlign::Right),
            );
        }
        self.setting_row(
            "Delete server",
            tr("Remove this server from Eclipse and move its worlds, settings and backups to the trash.").into(),
            controls.into_any_element(),
        )
        .items_start()
    }

    fn background_row(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let on = self.servers.service.unwrap_or(false);
        let idle = self.servers.busy.is_none() && self.servers.service.is_some();
        self.switch_row(
            "background",
            "Run in the background and at login",
            if on {
                tr("On: starts at login and keeps running when the launcher closes.").into()
            } else {
                tr("Off: the server stops when you close the launcher.").into()
            },
            on,
            idle.then(|| {
                cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.server_command(
                        if on {
                            "Turning off running in the background"
                        } else {
                            "Setting it to run in the background"
                        },
                        vec![
                            "service".into(),
                            if on { "uninstall" } else { "install" }.into(),
                        ],
                    );
                    cx.notify();
                })
            }),
        )
    }

    fn updates_section(&mut self, status: &Status, cx: &mut Context<Self>) -> Div {
        let idle = self.servers.busy.is_none();
        let check = self.servers.update.clone();
        let release = status
            .release
            .clone()
            .or_else(|| {
                check
                    .as_ref()
                    .and_then(|u| u["installed"].as_str().map(str::to_string))
            })
            .or_else(servers::runtime_release)
            .unwrap_or_else(|| "?".into());
        let available = status.update_available.clone().or_else(|| {
            check
                .as_ref()
                .filter(|u| u["available"] == true)
                .and_then(|u| u["newest"].as_str().map(str::to_string))
        });
        let about = match (&available, &check, &status.update_last) {
            (Some(tag), _, _) => trf(
                "Update available: Eclipse {tag} (this server runs {release}). Updating backs up \
                 the world first, and goes back by itself if the new version doesn't start.",
                &[("tag", tag), ("release", &release)],
            ),
            (None, Some(_), _) => trf("Up to date: Eclipse {release}.", &[("release", &release)]),
            (None, None, Some((last, _))) => format!("Eclipse {release}. {last}"),
            (None, None, None) => format!("Eclipse {release}."),
        };
        let control = match available {
            Some(tag) => {
                let b = ui::primary("update-now", tr("Update now"), ui::Size::Md);
                if idle {
                    b.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.servers.update = None;
                        if let Some(dir) = this.servers.open.clone() {
                            let done =
                                format!("Updated to Eclipse {tag}. The world was backed up first.");
                            this.server_job(format!("Updating to {tag}"), move |log| {
                                servers::run_streamed(&dir, &["update"], log).map(|_| done)
                            });
                        }
                        cx.notify();
                    }))
                } else {
                    crate::disabled(b)
                }
            }
            None if self.servers.checking => {
                crate::disabled(button("update-check", "Checking…", Kind::Secondary))
            }
            None if !idle => {
                crate::disabled(button("update-check", "Check for updates", Kind::Secondary))
            }
            None => button("update-check", "Check for updates", Kind::Secondary).on_press(
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    if let Some(dir) = this.servers.open.clone() {
                        this.servers.checking = true;
                        let tx = this.servers.tx.clone();
                        std::thread::spawn(move || {
                            let result = servers::run(&dir, &["update", "--check", "--json"])
                                .and_then(|out| {
                                    serde_json::from_str(out.trim())
                                        .map_err(|e| Problem::plain(e.to_string()))
                                });
                            let _ = tx.send(Event::UpdateCheck(result));
                        });
                    }
                    cx.notify();
                }),
            ),
        };
        let mut rows = div()
            .flex()
            .flex_col()
            .mx(px(-ui::SETTING_ROW_PADDING))
            .child(self.setting_row("Eclipse version", about, control.into_any_element()));
        if self.servers.setting("updates", "keep").is_some() {
            rows = rows.child(self.setting_row(
                "Older versions kept",
                tr("Downloaded versions kept besides the one in use and the one an update can go back \
                 to.")
                    .into(),
                self.stepper("updates.keep", 0..=5, cx),
            ));
        }
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(ui::label(tr("Updates")))
            .child(rows)
    }

    /// The server's details came back without its settings: its eclipse-server predates
    /// `config` (and `backups --json`).
    fn older_eclipse(&self) -> bool {
        self.servers.service.is_some() && self.servers.config.is_none()
    }

    fn backups_tab(&mut self, status: &Status, cx: &mut Context<Self>) -> Div {
        let idle = self.servers.busy.is_none();
        let every = self.servers.shown("backups", "every");
        let auto = !every.is_empty() && every != "0";
        let keep = self.servers.shown("backups", "keep");
        let now = {
            let b = ui::primary("backup-now", tr("Back up now"), ui::Size::Md);
            if idle {
                b.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.server_command("Backing up", vec!["backup".into()]);
                    cx.notify();
                }))
            } else {
                crate::disabled(b)
            }
        };
        let mut list = div().flex().flex_col();
        for (i, b) in self.servers.backups.clone().into_iter().enumerate() {
            let (when, kind) = backup_label(&b);
            let confirming = self.servers.confirm_restore.as_deref() == Some(b.name.as_str());
            let name = b.name.clone();
            let restored = format!(
                "The world is back as it was on {when}. The one it replaced is in the backups \
                 folder."
            );
            let restore = button(
                SharedString::from(format!("restore-{i}")),
                if confirming {
                    "Replace the world"
                } else {
                    "Restore"
                },
                if confirming {
                    Kind::Primary
                } else {
                    Kind::Ghost
                },
            );
            let restore = if idle {
                restore.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if this.servers.confirm_restore.as_ref() == Some(&name) {
                        this.servers.confirm_restore = None;
                        if let Some(dir) = this.servers.open.clone() {
                            let (name, restored) = (name.clone(), restored.clone());
                            this.server_job("Restoring", move |log| {
                                servers::run_streamed(&dir, &["restore", &name, "--yes"], log)
                                    .map(|_| restored)
                            });
                        }
                    } else {
                        this.servers.confirm_restore = Some(name.clone());
                    }
                    cx.notify();
                }))
            } else {
                crate::disabled(restore)
            };
            let mut right = div().flex().flex_none().items_center().gap(px(8.));
            if confirming {
                right = right.child(button("restore-cancel", "Cancel", Kind::Ghost).on_press(
                    cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.servers.confirm_restore = None;
                        cx.notify();
                    }),
                ));
            }
            right = right.child(restore);
            let detail = if kind.is_empty() {
                bytes(b.bytes)
            } else {
                format!("{kind} · {}", bytes(b.bytes))
            };
            list =
                list.child(self.list_row(None, when, Some(detail), Some(right.into_any_element())));
        }
        let mut tab = div()
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(div().flex().items_center().gap(px(16.)).child(now));
        if self.older_eclipse() {
            return tab
                .child(
                    ui::text(step::BODY, FontWeight::NORMAL, ink::MUTED).child(tr(
                        "This server's Eclipse is too old to list its backups here: they're in its \
                     backups folder. Update it below.",
                    )),
                )
                .child(self.updates_section(status, cx));
        }
        tab = tab.child(
            ui::toggle_row(
                "auto-backup",
                tr("Back up automatically"),
                Some(
                    div()
                        .child(if auto {
                            trf(
                                "On: every {every} while the server runs, keeping the last {keep}.",
                                &[("every", &every), ("keep", &keep)],
                            )
                        } else {
                            tr("Off: only the backups you make.").to_string()
                        })
                        .into_any_element(),
                ),
                auto,
            )
            .mx(px(-ui::SETTING_ROW_PADDING))
            .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.set_setting("backups.every", if auto { "0".into() } else { "6h".into() });
                cx.notify();
            })),
        );
        if self.servers.confirm_restore.is_some() {
            tab = tab.child(notice_line(
                ui::warn(),
                tr("The current world moves to the backups folder, and a running server restarts."),
            ));
        }
        tab = if self.servers.backups.is_empty() {
            tab.child(
                ui::text(step::BODY, FontWeight::NORMAL, ink::MUTED).child(tr("No backup yet.")),
            )
        } else {
            tab.child(list)
        };
        tab.child(self.updates_section(status, cx))
    }

    // ----- The Create dialog ----------------------------------------------------------------

    pub(super) fn create_dialog(&mut self, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        self.glass_create_dialog(cx)
    }
}

/// The state, in a few words.
fn phase_label(status: &Status) -> String {
    if let Some(busy) = &status.update_busy {
        return trf("Updating: {step}", &[("step", busy)]);
    }
    match status.phase.as_str() {
        "running" if status.crashes > 0 => tr("Running · restarted after a crash").into(),
        "running" if status.paused => tr("Running · paused while nobody plays").into(),
        "running" => tr("Running").into(),
        "starting" => tr("Starting").into(),
        "restarting" => tr("Restarting").into(),
        "stopping" => tr("Stopping").into(),
        _ => tr("Stopped").into(),
    }
}

/// A setting's name for people.
fn setting_name(key: &str) -> &str {
    match key {
        "server.motd" => "the message in the server list",
        "server.port" => "the port",
        "server.max_players" => "max players",
        "server.view_distance" => "view distance",
        "server.simulation_distance" => "simulation distance",
        "server.online_mode" => "Microsoft sign-in",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backups_read_for_people() {
        let b = |name: &str| Backup {
            name: name.into(),
            bytes: 1,
            modified: None,
        };
        assert_eq!(
            backup_label(&b("2026-10-19_11-28-57-manual.zip")),
            ("2026-10-19 11:28:57 UTC".into(), "Made by hand".into())
        );
        assert_eq!(
            backup_label(&b("2026-10-19_11-28-57-before-v2026.10.20.zip")).1,
            "Before updating to v2026.10.20"
        );
        assert_eq!(
            backup_label(&b("2026-10-19_13-35-36-before-v2026-10-19-10.zip")).1,
            "Before updating to v2026.10.19.10"
        );
        assert_eq!(
            backup_label(&b("2026-10-14_17-46-40-before-update.zip")).1,
            "Before an update"
        );
        assert_eq!(
            backup_label(&b("2026-10-19_11-28-57-auto.zip")).1,
            "Automatic"
        );
        assert_eq!(backup_label(&b("odd.zip")), ("odd".into(), String::new()));
        assert_eq!(bytes(5_292_542), "5.3 MB");
        assert_eq!(uptime(3725), "1 h 02 min");
    }
}
