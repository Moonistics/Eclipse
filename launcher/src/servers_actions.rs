use crate::fields::Field;
use crate::menus::Asking;
use crate::page::Page;
use crate::scrollbar::Scrollbar;
use crate::servers_state::{
    bytes, plays_offline, Create, Event, GameMode, ServerJob, Tab, CONSOLE_KEEP, POLL_LIST,
    POLL_OPEN, STARTED_HERE,
};
use crate::ui::{ButtonKind, Press};
use crate::{ui, Launcher};
use eclipse_launcher::i18n::{tr, trf};
use eclipse_launcher::servers::{Fix, Key, NewServer, Phase, Problem, Status, World};
use eclipse_launcher::{account, backend, instances, servers};
use gpui::prelude::*;
use gpui::{ClickEvent, Div, SharedString, Stateful};
use serde_json::Value;
use std::path::PathBuf;
use std::time::Instant;

impl Launcher {
    // ----- Background work ------------------------------------------------------------------

    /// Runs `job` on a thread with the page busy; its lines and result arrive as events.
    pub(super) fn server_job(
        &mut self,
        what: ServerJob,
        job: impl FnOnce(&dyn Fn(String)) -> Result<String, Problem> + Send + 'static,
    ) {
        if self.servers.busy.is_some() {
            return;
        }
        self.servers.busy = Some(what);
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
    pub(super) fn server_command(&mut self, what: ServerJob, args: Vec<String>) {
        let Some(dir) = self.servers.open.clone() else {
            return;
        };
        self.server_job(what, move |log| {
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            servers::run_streamed(&dir, &args, log).map(|_| String::new())
        });
    }

    /// Reads the open server's settings, backups and service state again.
    pub(super) fn load_details(&self) {
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
        self.stop_asking(|a| matches!(a, Asking::ServerRestore(_) | Asking::ServerDelete));
        let s = &mut self.servers;
        s.open = Some(dir);
        // Its settings may have changed while it was open.
        s.modes.clear();
        s.tab = Tab::Players;
        s.config = None;
        s.backups.clear();
        s.service = None;
        s.update = None;
        s.pvp = None;
        s.problem = None;
        s.note = None;
        s.restart_needed.clear();
        s.polled = None;
        self.fields.clear(Field::ServerMotd);
        self.fields.clear(Field::ServerPort);
        self.fields.invalid = None;
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

    pub(super) fn servers_event(&mut self, event: Event) {
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
                let read_pvp = status.phase() == Phase::Running && s.pvp.is_none();
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
                        self.servers.note = Some(trf(
                            "{name} is ready. Start it and share the address with friends.",
                            &[("name", &entry.name)],
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
                if self.asking == Some(Asking::ServerDelete) {
                    self.asking = None;
                }
                let name = (s.list.iter().find(|entry| entry.dir == dir))
                    .map_or_else(|| tr("Server").to_string(), |entry| entry.name.clone());
                s.list = servers::list();
                s.modes.clear();
                s.statuses.remove(&dir);
                STARTED_HERE
                    .lock()
                    .unwrap()
                    .retain(|started| started != &dir);
                if !s.list.iter().any(|entry| entry.dir == dir) {
                    s.open = None;
                }
                match result {
                    Ok(()) => s.note = Some(trf("{name} moved to the trash", &[("name", &name)])),
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
    pub(super) fn read_pvp(&self) {
        let Some(dir) = self.servers.open.clone() else {
            return;
        };
        let tx = self.servers.tx.clone();
        std::thread::spawn(move || {
            let on = servers::Api::open(&dir).and_then(|a| a.pvp());
            let _ = tx.send(Event::Pvp(on));
        });
    }

    /// Queues a setting for `eclipse-server config set`; they go one batch at a time.
    pub(super) fn set_setting(&mut self, key: Key, value: String) {
        self.servers.queued.retain(|(k, _)| *k != key);
        self.servers.queued.push((key, value));
        self.servers.note = None;
        if !self.servers.applying {
            self.apply_settings();
        }
    }

    pub(super) fn apply_settings(&mut self) {
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
                let (section, field) = key.path();
                let new = match &config[section][field] {
                    Value::Bool(_) => Value::Bool(value == "true"),
                    Value::Number(_) => value.parse::<i64>().map_or(Value::Null, Value::from),
                    _ => Value::String(value.clone()),
                };
                config[section][field] = new;
            }
        }
        let batch: Vec<(String, String)> =
            batch.into_iter().map(|(k, v)| (k.dotted(), v)).collect();
        let tx = self.servers.tx.clone();
        std::thread::spawn(move || {
            let pairs: Vec<(&str, String)> =
                batch.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
            let _ = tx.send(Event::Applied(servers::set_config(&dir, &pairs)));
        });
    }

    /// A command for the running game, through the control API; its answer shows in the console.
    pub(super) fn api_command(&mut self, line: String) {
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

    pub(super) fn start_server(&mut self) {
        let Some(dir) = self.servers.open.clone() else {
            return;
        };
        STARTED_HERE.lock().unwrap().push(dir.clone());
        self.servers.restart_needed.clear();
        self.server_job(ServerJob::Starting, move |log| {
            log("A new world takes a minute the first time".into());
            servers::run_streamed(&dir, &["start"], log).map(|_| String::new())
        });
    }

    pub(super) fn fix(&mut self, fix: Fix, cx: &mut Context<Self>) {
        self.servers.problem = None;
        match fix {
            Fix::UsePort(port) => {
                self.set_setting(Key::Port, port.to_string());
                self.fields.clear(Field::ServerPort);
                self.servers.note = Some(trf(
                    "The server now uses port {port}. Press Start again.",
                    &[("port", &port.to_string())],
                ));
            }
            Fix::Prepare => match self.selected_version().cloned() {
                Some(tag) if backend::is_prepared(&tag) => {
                    self.server_job(ServerJob::CopyingGameFiles, move |log| {
                        log(format!("Copying the game files prepared for Eclipse {tag}"));
                        servers::set_up(&tag)
                            .map(|_| "The game files are ready. Press Start again.".to_string())
                            .map_err(Problem::plain)
                    });
                }
                _ => {
                    let eclipse = self
                        .instances
                        .list
                        .iter()
                        .find(|i| i.kind == instances::Kind::Default)
                        .cloned()
                        .filter(|_| self.selected_instance().is_some_and(|i| i.java.is_some()));
                    if let Some(eclipse) = eclipse {
                        self.select_instance(&eclipse);
                    }
                    self.prepare(backend::MinecraftFrom::Install, false);
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
                self.servers.tab = Tab::Players;
                if let Some(dir) = &self.servers.open {
                    ui::open_folder(&backend::logs_dir_in(dir), cx);
                }
            }
        }
    }

    // ----- Creating -------------------------------------------------------------------------

    pub(super) fn open_create(&mut self, cx: &mut Context<Self>) {
        self.fields.handle(Field::NewServerName, cx);
        self.fields.focus_soon = Some(Field::NewServerName);
        let worlds = servers::player_worlds();
        // The dialog starts with an empty name (its placeholder says the default) and the player
        // as the first friend.
        let own = self.own_name();
        self.fields.text(Field::NewServerName).set(String::new());
        self.fields.selected = None;
        self.fields.text(Field::NewServerSeed).set(String::new());
        self.fields.text(Field::NewServerFriend).set(String::new());
        let details = eclipse_launcher::worlds::list(&backend::game_home())
            .into_iter()
            .map(|w| {
                let line = format!("Eclipse · {} · {}", tr(w.mode), bytes(w.size));
                (w.path, (w.name, line))
            })
            .collect();
        let offline = plays_offline(&self.accounts);
        let start = NewServer::for_owner(offline);
        self.servers.problem = None;
        self.servers.create_scroll = Scrollbar::default();
        self.servers.create = Some(Create {
            new_world: true,
            copy: worlds.first().map(|w| w.1.clone()),
            worlds,
            mode: GameMode::Survival,
            difficulty: "normal",
            max_players: start.max_players,
            friends_only: start.friends_only && own.is_some(),
            check_accounts: start.check_accounts,
            offline,
            eula: false,
            eula_missing: false,
            friends: own.into_iter().collect(),
            details,
        });
    }

    pub(super) fn create_server(&mut self) {
        if self.servers.creating || self.server_needs().is_some() {
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
                seed: self.fields.value(Field::NewServerSeed).to_string(),
            },
        };
        let mut players = c.friends.clone();
        for p in self.fields.value(Field::NewServerFriend).split([',', ' ']) {
            if !p.is_empty() && !players.iter().any(|q| q == p) {
                players.push(p.to_string());
            }
        }
        let name = self.fields.value(Field::NewServerName).trim().to_string();
        let spec = NewServer {
            name: if !name.is_empty() {
                name
            } else if self.servers.list.is_empty() {
                "My server".into()
            } else {
                format!("Server {}", self.servers.list.len() + 1)
            },
            world,
            game_mode: c.mode.id().into(),
            difficulty: c.difficulty.into(),
            max_players: c.max_players,
            friends_only: c.friends_only,
            check_accounts: c.check_accounts,
            players,
            eula: c.eula,
        };
        let tag = self.selected_version().cloned();
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

    /// Escape closes the Create dialog. Whether it did.
    pub(super) fn servers_escape(&mut self) -> bool {
        if self.servers.create.is_none() || self.servers.creating {
            return false;
        }
        self.servers.create = None;
        self.servers.problem = None;
        true
    }

    pub(super) fn server_field_enter(&mut self, f: Field) {
        match f {
            // The dialog's friend field adds the name typed.
            Field::NewServerFriend => self.add_create_friend(),
            Field::NewServerName | Field::NewServerSeed => self.create_server(),
            Field::ServerCommand => {
                let line = self.fields.text(Field::ServerCommand).take();
                let line = line.trim().trim_start_matches('/').to_string();
                if !line.is_empty() {
                    self.api_command(line);
                }
            }
            Field::ServerPlayer => self.add_player(),
            Field::ServerMotd => {
                let motd = self.fields.value(Field::ServerMotd).trim().to_string();
                if motd.is_empty() {
                    self.servers.problem =
                        Some(Problem::plain("Type the line players see under the name."));
                } else {
                    self.set_setting(Key::Motd, motd);
                }
            }
            Field::ServerPort => self.save_port(),
            _ => {}
        }
    }

    pub(super) fn save_port(&mut self) {
        match self.fields.value(Field::ServerPort).trim().parse::<u16>() {
            Ok(port) if port >= 1024 => self.set_setting(Key::Port, port.to_string()),
            _ => {
                self.fields.invalid = Some((
                    Field::ServerPort,
                    Some("Use a number from 1024 to 65535 (25565 is Minecraft's own)."),
                ))
            }
        }
    }

    pub(super) fn add_player(&mut self) {
        let name = self.fields.value(Field::ServerPlayer).trim().to_string();
        if !account::valid_offline_name(&name) {
            self.fields.invalid = Some((
                Field::ServerPlayer,
                Some("A Minecraft name has 3 to 16 letters, digits or underscores."),
            ));
            return;
        }
        self.fields.text(Field::ServerPlayer).set(String::new());
        self.server_command(
            ServerJob::LettingIn(name.clone()),
            vec!["whitelist".into(), "add".into(), name],
        );
    }

    pub(super) fn player_face(&mut self, uuid: &str, size: f32) -> Div {
        let resources = servers::game_files(&servers::runtime_root(), servers::GAME_VERSION);
        ui::face(
            self.profile_heads
                .draw_player(uuid, &resources, size, self.scale),
            size,
        )
    }

    /// A row's small button.
    pub(super) fn player_action(
        &self,
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        idle: bool,
        why: Option<SharedString>,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self) + 'static,
    ) -> Stateful<Div> {
        let b = ui::button(id, None, label, ButtonKind::Secondary, ui::ButtonSize::Sm);
        match why {
            Some(why) => ui::unavailable(b, why),
            None if idle => b.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                action(this);
                cx.notify();
            })),
            None => ui::disabled(b),
        }
    }

    // ----- The page -------------------------------------------------------------------------
}
