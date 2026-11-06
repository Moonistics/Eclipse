use super::*;
use eclipse_launcher::instances::Instance;

#[derive(Default)]
pub(super) struct GameRun {
    pub(super) client: Option<Running>,
    pub(super) error: Option<String>,
    pub(super) started: Option<std::time::SystemTime>,
    pub(super) home: Option<std::path::PathBuf>,
    pub(super) run: Option<(String, Instant)>,
    pub(super) vars: eclipse_launcher::launch::Vars,
    pub(super) since: Option<Instant>,
    pub(super) starting: Option<String>,
    pub(super) pack: Option<std::path::PathBuf>,
    pub(super) crash: Option<Crash>,
    pub(super) preparing: bool,
    pub(super) play_when_prepared: Option<String>,
    pub(super) prepare_started: Option<Instant>,
    pub(super) prepare_error: Option<String>,
    pub(super) pressed: Option<Instant>,
    pub(super) log: VecDeque<String>,
    pub(super) log_for: Option<String>,
}

impl GameRun {
    pub(super) fn begin(&mut self, instance: Option<&Instance>) {
        self.log.clear();
        self.log_for = instance.map(|i| i.id.clone());
    }

    fn belongs_to(&self, instance: Option<&Instance>) -> bool {
        self.log_for.as_deref() == instance.map(|i| i.id.as_str())
    }

    fn plays(&self, instance: &Instance) -> bool {
        match &self.run {
            Some((id, _)) => *id == instance.id,
            None => self.log_for.as_deref() == Some(&instance.id),
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

impl PlayState {
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

impl Launcher {
    pub(super) fn play_state(&self, i: &Instance) -> PlayState {
        if self.game.client.is_some() && self.game.plays(i) {
            return PlayState::Running;
        }
        let follows = i.version.is_none() && i.java.is_none();
        let update = self.updates.eclipse.as_ref().filter(|_| follows);
        let installing = |tag: &str| {
            self.installing
                .as_ref()
                .filter(|(t, _)| t == tag)
                .map(|(_, p)| *p)
        };
        if let Some(p) = update.and_then(|u| installing(&u.tag)) {
            return PlayState::Updating(p);
        }
        if let Some(own) = &i.version {
            if let Some(p) = installing(own) {
                return PlayState::Installing(Some(p));
            }
            if self.install_next.as_ref().is_some_and(|r| r.tag == *own) {
                return PlayState::Installing(None);
            }
        }
        if self.game.preparing && self.game.belongs_to(Some(i)) {
            return PlayState::Preparing(
                eclipse_launcher::mojang::progress().map(|(d, n)| d as f32 / n.max(1) as f32),
            );
        }
        if self.game.starting.is_some() && self.game.belongs_to(Some(i)) {
            return PlayState::Starting;
        }
        PlayState::Idle
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

    pub(super) fn shown_game_error(&self) -> Option<&String> {
        self.game.error.as_ref().filter(|_| {
            self.game.log_for.is_none() || self.game.belongs_to(self.selected_instance())
        })
    }

    pub(super) fn shown_crash(&self) -> Option<&Crash> {
        self.game
            .crash
            .as_ref()
            .filter(|_| self.game.belongs_to(self.selected_instance()))
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
            java: None,
        }
    }

    #[test]
    fn a_run_belongs_to_the_instance_that_started_it() {
        let (mine, other) = (instance("survival"), instance("creative"));
        let mut run = GameRun::default();
        run.begin(Some(&mine));
        run.log.push_back("Minecraft 26.3 was not found".into());
        assert!(run.belongs_to(Some(&mine)));
        assert!(!run.belongs_to(Some(&other)));
        assert!(run.plays(&mine) && !run.plays(&other));
        run.run = Some((other.id.clone(), Instant::now()));
        assert!(run.plays(&other) && !run.plays(&mine));
    }
}
