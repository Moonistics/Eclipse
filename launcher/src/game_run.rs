use super::*;
use crate::ui::Tone;
use eclipse_launcher::instances::Instance;

pub(super) const START_SLACK: Duration = Duration::from_secs(2);

pub(super) struct Started {
    pub(super) eclipse: bool,
    pub(super) result: Result<backend::Launched, backend::PlayError>,
}

pub(super) struct Starts {
    pub(super) tx: std::sync::mpsc::Sender<Started>,
    pub(super) rx: std::sync::mpsc::Receiver<Started>,
}

impl Default for Starts {
    fn default() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Self { tx, rx }
    }
}

#[derive(Default)]
pub(super) struct GameRun {
    pub(super) starts: Starts,
    pub(super) owner: Option<String>,
    pub(super) phase: Phase,
    pub(super) home: Option<std::path::PathBuf>,
    pub(super) problem: Option<GameProblem>,
    pub(super) pack: Option<std::path::PathBuf>,
    pub(super) crash: Option<Crash>,
    pub(super) prepare_error: Option<String>,
    pub(super) log: VecDeque<String>,
}

#[derive(Default)]
pub(super) enum Phase {
    #[default]
    Idle,
    Preparing {
        started: Instant,
        then_play: Option<String>,
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

    fn belongs_to(&self, instance: Option<&Instance>) -> bool {
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

    pub(super) fn starting(&self) -> bool {
        matches!(self.phase, Phase::Starting { .. })
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

    pub(super) fn preparing_now(&mut self, instance: Option<&Instance>, then_play: Option<String>) {
        self.begin(instance);
        self.phase = Phase::Preparing {
            started: Instant::now(),
            then_play,
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

    pub(super) fn end_preparing(&mut self) -> Option<(Instant, Option<String>)> {
        match std::mem::take(&mut self.phase) {
            Phase::Preparing { started, then_play } => Some((started, then_play)),
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum PlayState {
    Idle,
    Preparing(Option<f32>),
    Installing(Option<f32>),
    Updating(f32),
    Starting,
    Running,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum InstallState {
    NotInstalled,
    UpdateReady(String),
    NotSetUp { minecraft: String },
    Built(eclipse_launcher::native_status::Build),
    Ready { pinned: Option<String> },
}

impl PlayState {
    pub(super) fn playing(self) -> bool {
        matches!(self, PlayState::Starting | PlayState::Running)
    }

    pub(super) fn busy_line(self) -> Option<String> {
        let (label, progress) = self.busy()?;
        Some(match progress {
            Some(p) => format!("{label} {:.0}%", p * 100.),
            None => label.to_string(),
        })
    }

    pub(super) fn busy(self) -> Option<(&'static str, Option<f32>)> {
        match self {
            PlayState::Updating(p) => Some((tr("Updating"), Some(p))),
            PlayState::Installing(p) => Some((tr("Installing"), p)),
            PlayState::Preparing(p) => Some((tr("Preparing"), p)),
            PlayState::Starting => Some((tr("Starting"), None)),
            PlayState::Idle | PlayState::Running => None,
        }
    }
}

impl InstallState {
    pub(super) fn line(&self) -> (Tone, String) {
        match self {
            InstallState::NotInstalled => (Tone::Off, tr("Not installed").into()),
            InstallState::UpdateReady(tag) => (
                Tone::Accent(ui::look().accent_soft),
                trf(
                    "{version} available",
                    &[("version", &eclipse_launcher::nightly::display_name(tag))],
                ),
            ),
            InstallState::NotSetUp { minecraft } => (
                Tone::Off,
                trf("Needs Minecraft {version}", &[("version", minecraft)]),
            ),
            InstallState::Built(build) => (
                Tone::Live,
                trf(
                    "Built with mods · {version}",
                    &[("version", &build.eclipse_short())],
                ),
            ),
            InstallState::Ready { pinned: Some(tag) } => (
                Tone::Live,
                trf(
                    "Ready · {version}",
                    &[("version", &eclipse_launcher::nightly::display_name(tag))],
                ),
            ),
            InstallState::Ready { pinned: None } => (Tone::Live, tr("Ready").into()),
        }
    }
}

impl Launcher {
    pub(super) fn update_for(&self, i: &Instance) -> Option<&Release> {
        let follows = self.plays(i).is_some_and(|p| p.follows);
        self.updates.eclipse.as_ref().filter(|_| follows)
    }

    pub(super) fn install_state(&self, i: &Instance) -> InstallState {
        let Some(plays) = self.plays(i) else {
            return InstallState::NotInstalled;
        };
        let Some(tag) = plays.installed_tag() else {
            return InstallState::NotInstalled;
        };
        if let Some(update) = self.update_for(i) {
            return InstallState::UpdateReady(update.tag.clone());
        }
        if !backend::is_prepared(tag) {
            return InstallState::NotSetUp {
                minecraft: backend::game_version(tag),
            };
        }
        match self.native_build_of(tag) {
            Some(build) => InstallState::Built(build),
            None => InstallState::Ready {
                pinned: (!plays.follows).then(|| tag.clone()),
            },
        }
    }

    pub(super) fn play_state(&self, i: &Instance) -> PlayState {
        let ours = self.game.belongs_to(Some(i));
        if ours && self.game.run().is_some() {
            return PlayState::Running;
        }
        let update = self.update_for(i);
        let installing = |tag: &str| self.versions.installing(tag).map(|i| i.done());
        if let Some(p) = update.and_then(|u| installing(&u.tag)) {
            return PlayState::Updating(p);
        }
        if let Some(tag) = self.plays(i).and_then(|p| p.tag) {
            if let Some(p) = installing(tag) {
                return PlayState::Installing(Some(p));
            }
            if self
                .versions
                .install_next
                .as_ref()
                .is_some_and(|r| r.tag == *tag)
            {
                return PlayState::Installing(None);
            }
        }
        match self.game.phase {
            Phase::Preparing { .. } if ours => PlayState::Preparing(
                eclipse_launcher::mojang::progress().map(|(d, n)| d as f32 / n.max(1) as f32),
            ),
            Phase::Starting { .. } if ours => PlayState::Starting,
            _ => PlayState::Idle,
        }
    }

    pub(super) fn selected_play_state(&self) -> PlayState {
        self.selected_instance()
            .map_or(PlayState::Idle, |i| self.play_state(i))
    }

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
mod tests {
    use super::*;

    fn instance(id: &str) -> Instance {
        Instance {
            id: id.into(),
            name: id.into(),
            kind: instances::Kind::Own,
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

    #[test]
    fn a_run_belongs_to_the_instance_that_started_it() {
        let (mine, other) = (instance("survival"), instance("creative"));
        let mut run = GameRun::default();
        run.preparing_now(Some(&mine), Some("v0.2.0".into()));
        run.log.push_back("Minecraft 26.3 was not found".into());
        assert!(run.belongs_to(Some(&mine)));
        assert!(!run.belongs_to(Some(&other)));
        assert!(run.busy() && run.preparing() && run.plays_when_prepared() && !run.active());
        assert_eq!(
            run.end_preparing().map(|(_, tag)| tag),
            Some(Some("v0.2.0".into()))
        );
        assert!(
            !run.busy() && run.belongs_to(Some(&mine)),
            "its log stays with it"
        );
        run.starting_now(&other, None);
        assert!(run.belongs_to(Some(&other)) && run.log.is_empty());
        assert!(run.busy() && run.active() && run.starting() && !run.preparing());
        assert!(
            run.end_run().is_none(),
            "a run that never started doesn't end"
        );
        assert!(run.starting(), "and the start goes on");
        assert!(run.end_starting().is_some());
        assert!(!run.busy() && run.end_starting().is_none());
    }
}
