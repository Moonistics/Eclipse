use super::*;
use eclipse_launcher::instances::Instance;

pub(super) const START_SLACK: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, PartialEq)]
pub(super) enum QuickPlay {
    World(String),
    Server(String),
    Friend(String),
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Start {
    pub(super) instance: Instance,
    pub(super) tag: String,
    pub(super) to: Option<QuickPlay>,
    pub(super) updates_mods: bool,
}

pub(super) struct Started {
    pub(super) eclipse: bool,
    pub(super) result: Result<backend::Launched, backend::PlayError>,
}

#[derive(Default)]
pub(super) struct Log(VecDeque<String>);

impl Log {
    pub(super) fn push(&mut self, line: String) {
        self.0.push_back(line);
        if self.0.len() > LOG_LINES {
            self.0.pop_front();
        }
    }

    pub(super) fn clear(&mut self) {
        self.0.clear();
    }

    pub(super) fn last_line(&self) -> Option<&String> {
        self.0.iter().rev().find(|l| !l.trim().is_empty())
    }

    pub(super) fn with_last_line(&self, error: &str) -> String {
        self.last_line()
            .map_or_else(|| error.to_string(), |t| format!("{error}: {t}"))
    }
}

impl std::ops::Deref for Log {
    type Target = VecDeque<String>;

    fn deref(&self) -> &VecDeque<String> {
        &self.0
    }
}

/// The last game the launcher started ended badly: what it left behind.
#[derive(Clone)]
pub(super) struct Crash {
    pub(super) report: Option<std::path::PathBuf>,
    /// One line on what went wrong: the report's error, else how the process ended.
    pub(super) summary: String,
    pub(super) home: std::path::PathBuf,
}

#[derive(Default)]
pub(super) struct GameRun {
    pub(super) owner: Option<String>,
    pub(super) phase: Phase,
    pub(super) home: Option<std::path::PathBuf>,
    pub(super) problem: Option<GameProblem>,
    pub(super) crash: Option<Crash>,
    pub(super) prepare_error: Option<String>,
    pub(super) log: Log,
}

#[derive(Default)]
pub(super) enum Phase {
    #[default]
    Idle,
    Preparing {
        started: Instant,
        then_play: Option<Box<Start>>,
    },
    Starting {
        pressed: Option<Instant>,
        reports_since: std::time::SystemTime,
    },
    Running(Run),
}

pub(super) struct Run {
    pub(super) client: Running,
    pub(super) since: Instant,
    pub(super) pressed: Option<Instant>,
    pub(super) reports_since: std::time::SystemTime,
    pub(super) vars: eclipse_launcher::launch::Vars,
}

impl GameRun {
    pub(super) fn begin(&mut self, instance: Option<&Instance>) {
        self.log.clear();
        self.owner = instance.map(|i| i.id.clone());
    }

    pub(super) fn belongs_to(&self, instance: Option<&Instance>) -> bool {
        self.owner.as_deref() == instance.map(|i| i.id.as_str())
    }

    pub(super) fn active(&self) -> bool {
        matches!(self.phase, Phase::Starting { .. } | Phase::Running(_))
    }

    pub(super) fn busy(&self) -> bool {
        !matches!(self.phase, Phase::Idle)
    }

    pub(super) fn preparing(&self) -> bool {
        matches!(self.phase, Phase::Preparing { .. })
    }

    pub(super) fn plays_when_prepared(&self) -> bool {
        matches!(
            self.phase,
            Phase::Preparing {
                then_play: Some(_),
                ..
            }
        )
    }

    pub(super) fn run(&self) -> Option<&Run> {
        match &self.phase {
            Phase::Running(run) => Some(run),
            _ => None,
        }
    }

    pub(super) fn preparing_now(&mut self, instance: Option<&Instance>, then_play: Option<Start>) {
        self.begin(instance);
        self.phase = Phase::Preparing {
            started: Instant::now(),
            then_play: then_play.map(Box::new),
        };
    }

    pub(super) fn starting_now(&mut self, instance: &Instance, pressed: Option<Instant>) {
        self.problem = None;
        self.crash = None;
        self.begin(Some(instance));
        self.phase = Phase::Starting {
            pressed,
            reports_since: std::time::SystemTime::now() - START_SLACK,
        };
    }

    pub(super) fn end_preparing(&mut self) -> Option<(Instant, Option<Start>)> {
        match std::mem::take(&mut self.phase) {
            Phase::Preparing { started, then_play } => Some((started, then_play.map(|s| *s))),
            other => {
                self.phase = other;
                None
            }
        }
    }

    pub(super) fn end_starting(&mut self) -> Option<(Option<Instant>, std::time::SystemTime)> {
        match std::mem::take(&mut self.phase) {
            Phase::Starting {
                pressed,
                reports_since,
            } => Some((pressed, reports_since)),
            other => {
                self.phase = other;
                None
            }
        }
    }

    pub(super) fn end_run(&mut self) -> Option<Run> {
        match std::mem::take(&mut self.phase) {
            Phase::Running(run) => Some(run),
            other => {
                self.phase = other;
                None
            }
        }
    }

    pub(super) fn take_pressed(&mut self) -> Option<Instant> {
        match &mut self.phase {
            Phase::Running(run) => run.pressed.take(),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum GameProblem {
    NeedsOwner,
    Failed(String),
    Notice(String),
}

impl GameProblem {
    pub(super) fn failure(&self) -> Option<String> {
        match self {
            GameProblem::NeedsOwner => Some(account::NoOwner::in_window()),
            GameProblem::Failed(e) => Some(e.clone()),
            GameProblem::Notice(_) => None,
        }
    }
}

impl Launcher {
    pub(super) fn console_log(&self) -> Option<&VecDeque<String>> {
        let running = self.selected_play_state() == PlayState::Running;
        (self.game.belongs_to(self.selected_instance()) && (running || !self.game.log.is_empty()))
            .then_some(&self.game.log)
    }

    pub(super) fn shown_prepare_error(&self) -> Option<&String> {
        let java = self.selected_instance().is_some_and(|i| i.java.is_some());
        self.game
            .prepare_error
            .as_ref()
            .filter(|_| !java && self.game.belongs_to(self.selected_instance()))
    }

    pub(super) fn shown_game_problem(&self) -> Option<&GameProblem> {
        self.game
            .problem
            .as_ref()
            .filter(|_| self.game.owner.is_none() || self.game.belongs_to(self.selected_instance()))
    }

    pub(super) fn shown_crash(&self) -> Option<&Crash> {
        self.game
            .crash
            .as_ref()
            .filter(|_| self.game.belongs_to(self.selected_instance()))
    }

    pub(super) fn busy_reason(&self) -> Option<String> {
        let name = self
            .game
            .owner
            .as_ref()
            .and_then(|id| self.instances.list.iter().find(|i| i.id == *id))
            .map_or("Eclipse", |i| i.name.as_str());
        let name = [("name", name)];
        Some(match self.game.phase {
            Phase::Idle => return None,
            Phase::Running(_) => trf("{name} is still running: close it first", &name),
            Phase::Starting { .. } => trf("{name} is still starting: wait for it to open", &name),
            Phase::Preparing { .. } => {
                trf("{name} is still preparing: wait for it to finish", &name)
            }
        })
    }

    pub(super) fn why_not_start(&self) -> String {
        self.busy_reason()
            .unwrap_or_else(|| tr("Press Play once first to set up the game").into())
    }
}

#[cfg(test)]
pub(super) fn test_instance(id: &str, kind: instances::Kind) -> Instance {
    Instance {
        id: id.into(),
        name: id.into(),
        kind,
        work: id.into(),
        home: id.into(),
        icon: None,
        group: None,
        version: None,
        played: Duration::ZERO,
        last_played: None,
        launch: Default::default(),
        game_args: None,
        java: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instance(id: &str) -> Instance {
        test_instance(id, instances::Kind::Own)
    }

    #[test]
    fn a_run_belongs_to_the_instance_that_started_it() {
        let (mine, other) = (instance("survival"), instance("creative"));
        let mut run = GameRun::default();
        let start = Start {
            instance: mine.clone(),
            tag: "v0.2.0".into(),
            to: None,
            updates_mods: true,
        };
        run.preparing_now(Some(&mine), Some(start.clone()));
        run.log.push("Minecraft 26.3 was not found".into());
        assert!(run.belongs_to(Some(&mine)));
        assert!(!run.belongs_to(Some(&other)));
        assert!(run.busy() && run.preparing() && run.plays_when_prepared() && !run.active());
        assert_eq!(run.end_preparing().map(|(_, then)| then), Some(Some(start)));
        assert!(
            !run.busy() && run.belongs_to(Some(&mine)),
            "its log stays with it"
        );
        run.starting_now(&other, None);
        assert!(run.belongs_to(Some(&other)) && run.log.is_empty());
        assert!(run.busy() && run.active() && !run.preparing());
        assert!(
            run.end_run().is_none(),
            "a run that never started doesn't end"
        );
        assert!(run.active(), "and the start goes on");
        assert!(run.end_starting().is_some());
        assert!(!run.busy() && run.end_starting().is_none());
    }

    #[test]
    fn a_log_keeps_its_newest_lines_and_explains_with_the_last_said() {
        let mut log = Log::default();
        for n in 0..LOG_LINES + 5 {
            log.push(format!("line {n}"));
        }
        log.push("   ".into());
        assert_eq!(log.len(), LOG_LINES);
        assert_eq!(log.front().map(String::as_str), Some("line 6"));
        assert_eq!(
            log.with_last_line("Exited with exit status: 1"),
            format!("Exited with exit status: 1: line {}", LOG_LINES + 4)
        );
        assert_eq!(Log::default().with_last_line("gone"), "gone");
    }
}
