//! The Servers page's state. Every action runs `eclipse-server` or calls the running server's
//! control API (`eclipse_launcher::servers`).
use crate::scrollbar::Scrollbar;
use crate::{ui, Launcher, Pick};
use eclipse_launcher::account::{Accounts, Player};
use eclipse_launcher::backend;
use eclipse_launcher::i18n::{tr, trf};
use eclipse_launcher::servers::{
    self, Applied, Backup, BackupKind, Entry, Key, Phase, Problem, Status,
};
use gpui::prelude::*;
use gpui::{Div, ScrollHandle, SharedString};
use serde_json::Value;
use std::collections::{BTreeSet, HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The servers this launcher started in this session, stopped when it closes unless they run as a
/// service (`stop_on_quit`).
pub(super) static STARTED_HERE: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// How often the open server's status and log are read, and the list's.
pub(super) const POLL_OPEN: Duration = Duration::from_millis(1000);
pub(super) const POLL_LIST: Duration = Duration::from_secs(3);

/// Lines of the server's log kept on the page.
pub(super) const CONSOLE_KEEP: usize = 1000;

pub(super) const FACE: f32 = ui::TILE_SM;

pub(super) const DELETE_CONTROLS: f32 = 260.;

#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Tab {
    Players,
    Settings,
    Backups,
}

impl Tab {
    pub(super) const ALL: [Tab; 3] = [Tab::Players, Tab::Settings, Tab::Backups];

    pub(super) fn label(self) -> &'static str {
        match self {
            Tab::Players => tr("Players"),
            Tab::Settings => tr("Settings"),
            Tab::Backups => tr("Backups"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum GameMode {
    Survival,
    Creative,
    Adventure,
}

impl GameMode {
    pub(super) const ALL: [GameMode; 3] =
        [GameMode::Survival, GameMode::Creative, GameMode::Adventure];

    pub(super) fn id(self) -> &'static str {
        match self {
            GameMode::Survival => "survival",
            GameMode::Creative => "creative",
            GameMode::Adventure => "adventure",
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            GameMode::Survival => tr("Survival"),
            GameMode::Creative => tr("Creative"),
            GameMode::Adventure => tr("Adventure"),
        }
    }

    pub(super) fn options() -> Vec<(&'static str, SharedString)> {
        GameMode::ALL
            .map(|m| (m.id(), SharedString::from(m.label())))
            .to_vec()
    }

    pub(super) fn label_of(id: &str) -> String {
        GameMode::ALL
            .into_iter()
            .find(|m| m.id() == id)
            .map_or_else(|| id.to_string(), |m| m.label().to_string())
    }
}

pub(super) const DIFFICULTIES: [(&str, &str); 4] = [
    ("peaceful", "Peaceful"),
    ("easy", "Easy"),
    ("normal", "Normal"),
    ("hard", "Hard"),
];

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

#[derive(Clone, Debug, PartialEq)]
pub(super) enum ServerJob {
    Starting,
    Restarting,
    Stopping,
    Trashing,
    CopyingGameFiles,
    ChangingOperators,
    UpdatingList,
    LettingIn(String),
    RunInBackground(bool),
    UpdatingTo(String),
    BackingUp,
    Restoring,
}

impl ServerJob {
    pub(super) fn label(&self) -> String {
        match self {
            ServerJob::Starting => tr("Starting").into(),
            ServerJob::Restarting => tr("Restarting").into(),
            ServerJob::Stopping => tr("Stopping").into(),
            ServerJob::Trashing => tr("Moving server to trash").into(),
            ServerJob::CopyingGameFiles => tr("Copying the game files").into(),
            ServerJob::ChangingOperators => tr("Changing operators").into(),
            ServerJob::UpdatingList => tr("Updating the list").into(),
            ServerJob::LettingIn(name) => trf("Letting {name} in", &[("name", name)]),
            ServerJob::RunInBackground(true) => tr("Setting it to run in the background").into(),
            ServerJob::RunInBackground(false) => tr("Turning off running in the background").into(),
            ServerJob::UpdatingTo(tag) => trf("Updating to {tag}", &[("tag", tag)]),
            ServerJob::BackingUp => tr("Backing up").into(),
            ServerJob::Restoring => tr("Restoring").into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Needs {
    Eclipse,
    GameFiles { minecraft: String },
    ServerProgram { version: String },
}

impl Needs {
    pub(super) fn line(&self) -> String {
        match self {
            Needs::Eclipse => tr("Servers run Eclipse, which isn't installed yet.").into(),
            Needs::GameFiles { minecraft } => trf(
                "Servers need Minecraft {version} from your Minecraft launcher.",
                &[("version", minecraft)],
            ),
            Needs::ServerProgram { version } => trf(
                "{version} can't host servers. Install a newer version.",
                &[("version", version)],
            ),
        }
    }
}

pub(super) fn set_up_label(minecraft: &str) -> String {
    trf("Set up Minecraft {version}", &[("version", minecraft)])
}

fn key_label(key: Key) -> Option<&'static str> {
    match key {
        Key::Motd => Some(tr("the message in the server list")),
        Key::Port => Some(tr("the port")),
        Key::MaxPlayers => Some(tr("max players")),
        Key::ViewDistance => Some(tr("view distance")),
        Key::SimulationDistance => Some(tr("simulation distance")),
        Key::OnlineMode => Some(tr("Microsoft sign-in")),
        _ => None,
    }
}

/// The Create dialog's answers.
pub(super) struct Create {
    pub(super) new_world: bool,
    pub(super) copy: Option<PathBuf>,
    pub(super) worlds: Vec<(String, PathBuf)>,
    /// The dialog's line under each world: its name and "Eclipse · mode · size", read
    /// once when the dialog opens.
    pub(super) details: HashMap<PathBuf, (String, String)>,
    pub(super) mode: GameMode,
    pub(super) difficulty: &'static str,
    pub(super) max_players: u32,
    pub(super) friends_only: bool,
    pub(super) check_accounts: bool,
    pub(super) offline: bool,
    pub(super) eula: bool,
    /// Create was pressed without the EULA ticked.
    pub(super) eula_missing: bool,
    /// The dialog's friends let in, the first an operator.
    pub(super) friends: Vec<String>,
}

/// What the sidebar shows of the Servers page: its list, and whether a server answers.
impl Servers {
    /// A stopped server's line in the list: "Survival · stopped". The open
    /// server's mode is the one its settings show now, once read; the others' (and the open one's
    /// until then) come from their files.
    pub(super) fn stopped_line(&mut self, dir: &std::path::Path) -> String {
        let open = Some(self.shown(Key::GameMode))
            .filter(|m| !m.is_empty() && self.open.as_deref() == Some(dir));
        let mode = open.or_else(|| {
            self.modes
                .entry(dir.to_path_buf())
                .or_insert_with(|| servers::game_mode(dir))
                .clone()
        });
        match mode {
            Some(mode) => trf("{mode} · stopped", &[("mode", &GameMode::label_of(&mode))]),
            None => tr("Stopped").to_string(),
        }
    }

    pub(super) fn listed(&self) -> &[Entry] {
        &self.list
    }

    pub(super) fn is_up(&self, dir: &std::path::Path) -> bool {
        self.statuses.get(dir).is_some_and(Status::serving)
    }
}

pub(super) struct Servers {
    pub(super) list: Vec<Entry>,
    pub(super) loaded: bool,
    pub(super) open: Option<PathBuf>,
    pub(super) tab: Tab,
    pub(super) statuses: HashMap<PathBuf, Status>,
    /// Each listed server's game mode from its `server.toml`, read once while it is listed.
    pub(super) modes: HashMap<PathBuf, Option<String>>,
    pub(super) polling: bool,
    pub(super) polled: Option<Instant>,
    pub(super) log: VecDeque<String>,
    pub(super) log_next: u64,
    pub(super) log_dir: Option<PathBuf>,
    pub(super) console_scroll: ScrollHandle,
    pub(super) create_scroll: Scrollbar,
    /// The action under way, and what it printed.
    pub(super) busy: Option<ServerJob>,
    pub(super) job_log: VecDeque<String>,
    pub(super) problem: Option<Problem>,
    pub(super) note: Option<String>,
    pub(super) create: Option<Create>,
    pub(super) creating: bool,
    pub(super) config: Option<Value>,
    /// Settings waiting for `config set`, and whether one runs.
    pub(super) queued: Vec<(Key, String)>,
    pub(super) applying: bool,
    /// Settings saved while the server ran that wait for its next start.
    pub(super) restart_needed: BTreeSet<String>,
    pub(super) backups: Vec<Backup>,
    pub(super) service: Option<bool>,
    pub(super) update: Option<Value>,
    /// `update --check` is under way.
    pub(super) checking: bool,
    pub(super) pvp: Option<bool>,
    pub(super) tx: Sender<Event>,
    pub(super) rx: Receiver<Event>,
}

impl Servers {
    pub(super) fn new() -> Self {
        let (tx, rx) = channel();
        Self {
            list: servers::list(),
            loaded: false,
            open: None,
            tab: Tab::Players,
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
            service: None,
            update: None,
            checking: false,
            pvp: None,
            tx,
            rx,
        }
    }

    pub(super) fn status(&self) -> Status {
        self.open
            .as_ref()
            .and_then(|d| self.statuses.get(d).cloned())
            .unwrap_or_else(Status::stopped)
    }

    /// A setting from server.toml (as `config --json` gave it).
    pub(super) fn setting(&self, key: Key) -> Option<&Value> {
        let (section, name) = key.path();
        self.config
            .as_ref()
            .map(|c| &c[section][name])
            .filter(|v| !v.is_null())
    }

    /// A setting as shown: what was just asked for, else what server.toml holds.
    pub(super) fn shown(&self, key: Key) -> String {
        if let Some((_, v)) = self.queued.iter().rev().find(|(k, _)| *k == key) {
            return v.clone();
        }
        match self.setting(key) {
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
            let _ = api.stop();
        }
    }
}

/// Update closes the window to open the new launcher at once: a server is its own process and the
/// new window finds it running, so an update doesn't end anyone's game. It stays up when that new
/// window closes too, until it is stopped on the Servers page.
pub(super) fn keep_running_through_update() {
    STARTED_HERE.lock().unwrap().clear();
}

impl Launcher {
    /// The player's own name, to let them in on a friends-only server: the Microsoft account or
    /// the offline name that plays, else the name the game made up for them, once it has.
    pub(super) fn own_name(&self) -> Option<String> {
        let name = match self.accounts.player() {
            Player::GameName => self.offline_name.clone(),
            _ => Some(self.who_plays().name),
        };
        name.filter(|name| !name.is_empty())
    }
}

pub(super) fn plays_offline(accounts: &Accounts) -> bool {
    !matches!(accounts.player(), Player::Microsoft(_))
}

pub(super) fn bytes(n: u64) -> String {
    match n {
        0..1_000_000 => format!("{} kB", n.div_ceil(1000)),
        1_000_000..1_000_000_000 => format!("{:.1} MB", n as f64 / 1e6),
        _ => format!("{:.2} GB", n as f64 / 1e9),
    }
}

/// When a backup was made and why, for people; a name eclipse-server didn't make, as it is.
pub(super) fn backup_label(b: &Backup) -> (String, String) {
    let Some((when, kind)) = b.made() else {
        return (b.name.trim_end_matches(".zip").to_string(), String::new());
    };
    let kind = match kind {
        BackupKind::Auto => tr("Automatic").to_string(),
        BackupKind::Manual => tr("Made by hand").to_string(),
        BackupKind::BeforeUpdate => tr("Before an update").to_string(),
        BackupKind::BeforeRelease(release) => {
            trf("Before updating to {release}", &[("release", &release)])
        }
        BackupKind::Other(kind) => kind,
    };
    (when, kind)
}

pub(super) fn uptime(secs: u64) -> String {
    match secs {
        0..60 => format!("{secs} s"),
        60..3600 => format!("{} min", secs / 60),
        3600..86_400 => format!("{} h {:02} min", secs / 3600, secs / 60 % 60),
        _ => format!("{} d {} h", secs / 86_400, secs / 3600 % 24),
    }
}

impl Launcher {
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
        self.servers_view(cx)
    }

    pub(super) fn server_needs(&self) -> Option<Needs> {
        if servers::program().is_some() && servers::game_files_ready() {
            return None;
        }
        let Some(tag) = self.selected_version() else {
            return Some(Needs::Eclipse);
        };
        if !backend::is_prepared(tag) {
            return Some(Needs::GameFiles {
                minecraft: backend::game_version(tag),
            });
        }
        servers::can_host(tag).err().map(|_| Needs::ServerProgram {
            version: eclipse_launcher::nightly::display_name(tag),
        })
    }

    pub(super) fn add_server_folder(&mut self, cx: &mut Context<Self>) {
        self.pick_paths(Pick::Folder, tr("Add"), cx, |this, mut paths, _| {
            if let Some(dir) = paths.pop() {
                this.add_existing(dir);
            }
        });
    }

    /// A folder with a server.toml joins the list; a vanilla server folder is set up first (its
    /// server.properties becomes server.toml; its world, lists and EULA stay).
    fn add_existing(&mut self, dir: PathBuf) {
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Server".into());
        let tag = self.selected_version().cloned();
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
}

pub(super) fn phase_label(status: &Status) -> String {
    if let Some(busy) = &status.update_busy {
        return trf("Updating: {step}", &[("step", busy)]);
    }
    match status.phase() {
        Phase::Running if status.crashes > 0 => tr("Running · restarted after a crash").into(),
        Phase::Running if status.paused => tr("Running · paused while nobody plays").into(),
        Phase::Running => tr("Running").into(),
        Phase::Starting => tr("Starting").into(),
        Phase::Restarting => tr("Restarting").into(),
        Phase::Stopping => tr("Stopping").into(),
        Phase::Stopped => tr("Stopped").into(),
    }
}

pub(super) fn setting_name(dotted: &str) -> String {
    Key::ALL
        .into_iter()
        .find(|k| k.dotted() == dotted)
        .and_then(key_label)
        .unwrap_or(dotted)
        .to_string()
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
            backup_label(&b("2026-10-19_13-35-36-before-v2026-10-19-10.zip")).1,
            "Before updating to v2026.10.19.10"
        );
        assert_eq!(backup_label(&b("odd.zip")), ("odd".into(), String::new()));
        assert_eq!(bytes(5_292_542), "5.3 MB");
        assert_eq!(uptime(3725), "1 h 02 min");
    }

    #[test]
    fn settings_and_modes_read_for_people() {
        assert_eq!(setting_name("server.port"), "the port");
        assert_eq!(setting_name("world.seed"), "world.seed");
        assert_eq!(GameMode::label_of("creative"), "Creative");
        assert_eq!(GameMode::label_of("hardcore"), "hardcore");
    }
}
