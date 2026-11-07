use super::*;
use crate::game_run::{Crash, GameProblem, Phase, QuickPlay, Run, Start, Started};
use eclipse_launcher::browser::eclipse;
use eclipse_launcher::instances::Instance;

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

fn pack_instance<'a>(
    list: &'a [Instance],
    work: &std::path::Path,
    default: Option<&'a String>,
    installed: &'a [String],
) -> Option<(&'a Instance, &'a String)> {
    let instance = list.iter().find(|i| i.work == work)?;
    let tag = instance
        .eclipse_version(default, installed)?
        .installed_tag()?;
    Some((instance, tag))
}

fn launch_of(start: Start, offline_without_owner: bool) -> backend::Play {
    let (world, server, join) = match start.to {
        Some(QuickPlay::World(world)) => (Some(world), None, None),
        Some(QuickPlay::Server(server)) => (None, Some(server), None),
        Some(QuickPlay::Friend(code)) => (None, None, Some(code)),
        None => (None, None, None),
    };
    backend::Play {
        version: Some(start.tag),
        offline_without_owner,
        join,
        world,
        server,
        ..backend::Play::of(start.instance)
    }
}

impl Launcher {
    pub(super) fn press_play(&mut self) {
        if self.cant_sign_in_to_play() {
            self.owner_screen_open = true;
            self.game.problem = Some(GameProblem::NeedsOwner);
            return;
        }
        let ready = self.selected_instance().is_some_and(|i| i.java.is_some())
            || self
                .selected_version()
                .is_some_and(|tag| backend::is_prepared(tag));
        match play_press(ready, self.game.busy()) {
            PlayPress::Play => self.play(None),
            PlayPress::PrepareThenPlay => self.prepare(backend::MinecraftFrom::Install, true),
            PlayPress::Nothing => {
                if let Some(why) = self.busy_reason() {
                    crate::toast::say(why);
                }
            }
        }
    }

    pub(super) fn minecraft_dir(&self) -> std::path::PathBuf {
        backend::minecraft_dir(self.settings.minecraft_dir.clone()).unwrap_or_default()
    }

    /// Prepares the selected version, its Minecraft files from the player's install or, for an
    /// owner who asks, from Mojang.
    pub(super) fn prepare(&mut self, from: backend::MinecraftFrom, then_play: bool) {
        let Some(tag) = self.selected_version().cloned() else {
            return;
        };
        if self.game.busy() {
            return;
        }
        let minecraft = self.minecraft_dir();
        let selected = self.selected_instance().cloned();
        let then_play = selected
            .clone()
            .filter(|_| then_play)
            .map(|instance| Start {
                instance,
                tag: tag.clone(),
                to: None,
                updates_mods: true,
            });
        self.game.preparing_now(selected.as_ref(), then_play);
        self.game.prepare_error = None;
        self.game
            .log
            .push("Checking Java and your Minecraft install".into());
        // The checks may download the server jar, so they run on the thread too.
        self.in_background(move |tell| {
            let log = |line| tell.send(Msg::Line(Job::Prepare, line));
            let result = backend::preparer(&tag, &minecraft, from, &log)
                .inspect(|(_, minecraft)| backend::sounds(&tag, minecraft, &log))
                .map(|(p, _)| {
                    backend::set_prepare_step(backend::PrepareStep::Prepare);
                    p
                })
                .and_then(|p| match p {
                    backend::Preparer::Process(cmd) => {
                        backend::spawn(cmd, tell.clone().output(Job::Prepare)).map(|_running| false)
                    }
                    backend::Preparer::InProcess(options) => {
                        backend::prepare_in_process(&options, &log).map(|_| true)
                    }
                });
            match result {
                // A process sends its own Exit when it ends.
                Ok(false) => {}
                Ok(true) => tell.send(Msg::Exit(Job::Prepare, Ok(()))),
                Err(e) => tell.send(Msg::Exit(Job::Prepare, Err(backend::Ended::Failed(e)))),
            }
        });
        self.download_seen = None;
    }

    pub(super) fn prepare_ended(&mut self, error: Option<backend::Ended>) {
        let (started, then_play) = self.game.end_preparing().unzip();
        // Play pressed before the version was prepared plays now.
        if let Some(start) = then_play.flatten() {
            if error.is_none() && backend::is_prepared(&start.tag) {
                self.start_client(start);
            }
        }
        if let (None, Some(started)) = (&error, started) {
            self.perf.record_since("launcher.prepare_ms", started);
        }
        // A preparer process's exit status says little, so its last line follows; the
        // launcher's own errors are full sentences already.
        self.game.prepare_error = error.map(|e| match e {
            backend::Ended::Status(_) => self.game.log.with_last_line(&e.to_string()),
            backend::Ended::Failed(e) => e,
        });
    }

    pub(super) fn play(&mut self, to: Option<QuickPlay>) {
        let Some(instance) = self.selected_instance().cloned() else {
            return;
        };
        if instance.java.is_some() {
            // Mojang's game: no Eclipse version or preparation, only no game already running.
            // It joins a server; a friend's code and opening a world are Eclipse's.
            if self.game.busy() {
                return;
            }
            let server = match to {
                Some(QuickPlay::Server(address)) => Some(address),
                _ => None,
            };
            self.start_java(instance, server);
            return;
        }
        let Some(tag) = self.selected_version().cloned() else {
            return;
        };
        if !self.can_start(&tag) {
            return;
        }
        let start = Start {
            instance,
            tag,
            to,
            updates_mods: true,
        };
        // Native mods the start's automatic update moved on: the game they are built into is
        // built again first (the Mods page's Build), then plays.
        let id = start.instance.id.clone();
        if self.native.rebuild.contains(&id) && !self.native.building() {
            self.native.rebuild.retain(|r| *r != id);
            if native_mods::is_native(&start.tag) {
                self.build_native(false, Some(start));
                return;
            }
        }
        self.start_client(start);
    }

    /// Plays the selected instance straight into a world (`--world`) or a server (`--server`):
    /// flags every Eclipse release takes.
    pub(crate) fn quick_play(&mut self, to: QuickPlay) {
        self.play(Some(to));
    }

    /// Join a friend: a code field and Join, which starts the game straight
    /// into the friend's world (errors show in the game).
    pub(super) fn join_friend(&mut self) {
        let code = self.fields.value(Field::InviteCode).to_string();
        if eclipse_friends::invite::parse(&code).is_none() {
            return;
        }
        self.play(Some(QuickPlay::Friend(code)));
    }

    pub(super) fn pack_start(&self, work: &std::path::Path) -> Result<Start, String> {
        let found = pack_instance(
            &self.instances.list,
            work,
            self.settings.default_version.as_ref(),
            &self.versions.installed,
        );
        match found {
            Some((instance, tag)) if self.can_start(tag) => Ok(Start {
                instance: instance.clone(),
                tag: tag.clone(),
                to: None,
                updates_mods: false,
            }),
            _ => Err(self.why_not_start()),
        }
    }

    pub(super) fn play_pack(&mut self, work: &std::path::Path) {
        if let Ok(start) = self.pack_start(work) {
            self.start_client(start);
        }
    }

    /// Starts a Java instance's Minecraft, joining `server` if given: its files and Java first
    /// (downloaded the first time, their lines in the log), then `drain` starts the game when
    /// the command arrives.
    fn start_java(&mut self, instance: Instance, server: Option<String>) {
        self.game.starting_now(&instance, None);
        let play = backend::Play {
            server,
            ..backend::Play::of(instance)
        };
        self.launch_in_background(play, None);
    }

    /// Starts the game: the session first (a network round trip when the token needs
    /// renewing), then `drain` starts the process when it arrives.
    pub(super) fn start_client(&mut self, start: Start) {
        self.game
            .starting_now(&start.instance, Some(Instant::now()));
        // Registry mods update before the game reads them (`eclipse::auto`); native mods only
        // at start, where a build can follow.
        let auto = (self.settings.auto_updates_mods() && start.updates_mods).then(|| {
            eclipse::auto::Target {
                instance: start.instance.clone(),
                level: Some(backend::mod_api_level(&start.tag)),
                natives: false,
            }
        });
        let play = launch_of(start, self.settings.dev_offline);
        self.launch_in_background(play, auto);
    }

    fn launch_in_background(&mut self, play: backend::Play, auto: Option<eclipse::auto::Target>) {
        let mods_tx = self.mods.tx.clone();
        self.in_background(move |tell| {
            if let Some(auto) = auto {
                let (lines, _) = eclipse::auto::update_all(&[auto]);
                if !lines.is_empty() {
                    let _ = mods_tx.send(mods_state::Event::AutoUpdated(lines, Vec::new()));
                }
            }
            let log = |line: String| tell.send(Msg::Line(Job::Client, line));
            let eclipse = play.instance.java.is_none();
            let result = backend::launch(play, &log);
            tell.send(Msg::Started(Started { eclipse, result }));
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

    pub(super) fn start_refusal(&self, to: &QuickPlay) -> Option<String> {
        let server = matches!(to, QuickPlay::Server(_));
        if !server && self.selected_instance().is_some_and(|i| i.java.is_some()) {
            return Some(tr("Start it from the game's menu").into());
        }
        if let Some(why) = self.why_no_sign_in() {
            return Some(why.into());
        }
        let ready = match server {
            true => self.can_join_selected(),
            false => self.can_start_selected(),
        };
        (!ready).then(|| self.why_not_start())
    }

    /// Whether a server's Join starts the selected instance now: a Java instance too, as Play
    /// would (no game running or starting).
    pub(crate) fn can_join_selected(&self) -> bool {
        match self.selected_instance() {
            Some(i) if i.java.is_some() => !self.game.busy(),
            _ => self.can_start_selected(),
        }
    }

    pub(super) fn game_started(&mut self, started: Started, cx: &mut Context<Self>) {
        self.load_auth_statuses(false);
        let Some((pressed, reports_since)) = self.game.end_starting() else {
            account::discard_unread_sessions();
            return;
        };
        let launched = match started.result {
            Ok(launched) => launched,
            Err(backend::PlayError::NoOwner) => {
                self.owner = Some(false);
                self.owner_screen_open = true;
                self.game.problem = Some(GameProblem::NeedsOwner);
                return;
            }
            Err(e) => {
                self.game.problem = Some(GameProblem::Failed(e.to_string()));
                return;
            }
        };
        // Past the gate by the developer option alone is no owner.
        if started.eclipse && !self.settings.dev_offline {
            self.owner = Some(true);
        }
        self.reload_accounts();
        let notices: Vec<String> = launched.notices().iter().map(|n| n.said()).collect();
        self.game.problem = (!notices.is_empty()).then(|| GameProblem::Notice(notices.join("\n")));
        self.game.home = Some(launched.home);
        match backend::spawn(launched.cmd, self.tell().output(Job::Client)) {
            Ok(client) => {
                self.game.phase = Phase::Running(Run {
                    client,
                    since: Instant::now(),
                    pressed,
                    reports_since,
                    vars: launched.vars,
                });
                self.step_aside(cx);
            }
            Err(e) => {
                self.game.problem = Some(GameProblem::Failed(e));
                account::discard_unread_sessions();
            }
        }
    }

    pub(super) fn run_ended(&mut self, error: Option<backend::Ended>, cx: &mut Context<Self>) {
        let run = self.game.end_run();
        // Any session file the game didn't read (it crashed first).
        account::discard_unread_sessions();
        self.come_back(cx);
        // The game saved: last played, sizes and pictures changed.
        self.game_folder_changed();
        let owner = self.game.owner.clone();
        if let (Some(run), Some(id)) = (&run, owner) {
            if let Some(i) = self.instances.list.iter().find(|i| i.id == id) {
                instances::game_ended(i, run.since.elapsed(), &run.vars);
            }
            self.reload_instances();
        }
        // The run's own game folder: an instance's or a pack's keeps its reports.
        let home = self.game.home.clone().unwrap_or_else(backend::game_home);
        let report = run
            .as_ref()
            .and_then(|r| backend::crash_report_since(&home, r.reports_since));
        let switched = if error.is_some() && report.is_none() {
            self.switched_from_vulkan(&home)
        } else {
            None
        };
        if let Some(api) = switched {
            self.game.problem = Some(GameProblem::Notice(trf(
                "Eclipse's Vulkan renderer crashed while starting on this \
                 graphics card, so the game now uses {api}. Press Play to \
                 start it again. Video Settings > Graphics API switches back.",
                &[("api", api)],
            )));
        } else if error.is_some() || report.is_some() {
            let summary = report
                .as_ref()
                .and_then(|p| std::fs::read_to_string(p).ok())
                .and_then(|text| backend::crash_summary(&text))
                .or_else(|| {
                    error
                        .as_ref()
                        .map(|e| self.game.log.with_last_line(&e.to_string()))
                })
                .unwrap_or_default();
            self.game.crash = Some(Crash {
                report,
                summary,
                home,
            });
        }
    }

    /// After a crash with no report: when the game's output ends while its Vulkan renderer
    /// started, a native crash in the graphics driver (#746), switches the game to
    /// `backend::vulkan_crash_fallback` (DirectX 12 on Windows, OpenGL on Linux; only where the
    /// player left the graphics API on Auto). The API's label when it did.
    fn switched_from_vulkan(&self, run_home: &std::path::Path) -> Option<&'static str> {
        let (api, label) = backend::vulkan_crash_fallback(std::env::consts::OS)?;
        if !backend::vulkan_start_crashed(self.game.log.iter()) {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_run::test_instance;

    #[test]
    fn a_pack_plays_its_own_instance_on_the_version_that_instance_plays() {
        let mut selected = test_instance("survival", instances::Kind::Own);
        selected.version = Some("v0.3.0".into());
        let mut pack = test_instance("pack:create", instances::Kind::Own);
        pack.work = "packs/create".into();
        let mut pinned = test_instance("port:skyfactory", instances::Kind::Own);
        pinned.work = "ports/skyfactory".into();
        pinned.version = Some("v0.1.0".into());
        let list = [selected, pack, pinned];
        let installed = ["v0.3.0".to_string(), "v0.1.0".to_string()];
        let default = Some(&installed[0]);
        let plays = |work: &str| {
            pack_instance(&list, std::path::Path::new(work), default, &installed)
                .map(|(i, tag)| (i.id.as_str(), tag.as_str()))
        };
        assert_eq!(plays("packs/create"), Some(("pack:create", "v0.3.0")));
        assert_eq!(
            plays("ports/skyfactory"),
            Some(("port:skyfactory", "v0.1.0"))
        );
        assert_eq!(plays("packs/gone"), None);
    }

    #[test]
    fn a_play_that_waits_for_a_native_build_still_goes_where_it_was_sent() {
        let start = |to| Start {
            instance: test_instance("survival", instances::Kind::Own),
            tag: "v0.2.0".into(),
            to: Some(to),
            updates_mods: true,
        };
        let waited = |to| {
            let mut native = native_build::NativeBuild {
                then_play: Some(start(to)),
                ..Default::default()
            };
            launch_of(native.then_play.take().unwrap(), false)
        };
        let world = waited(QuickPlay::World("New World".into()));
        assert_eq!(world.world.as_deref(), Some("New World"));
        assert_eq!(world.version.as_deref(), Some("v0.2.0"));
        let server = waited(QuickPlay::Server("play.example.org".into()));
        assert_eq!(server.server.as_deref(), Some("play.example.org"));
        let friend = waited(QuickPlay::Friend("ECL-4MXR-K7Q".into()));
        assert_eq!(friend.join.as_deref(), Some("ECL-4MXR-K7Q"));
        assert_eq!((friend.world, friend.server), (None, None));
    }

    /// A server's Join (and every other shortcut) waits like Play: not while the version's
    /// first download and preparation run (#eclipse-bugs, 0.2.23: a white window opened).
    #[test]
    fn shortcuts_start_only_when_play_would() {
        assert!(starts_now(true, false, false));
        assert!(
            !starts_now(false, true, false),
            "first-time setup under way"
        );
        assert!(!starts_now(true, true, false), "prepared again under way");
        assert!(!starts_now(false, false, false), "not prepared");
        assert!(!starts_now(true, false, true), "already running");
    }
}
