use crate::messages::Msg;
use crate::{game_run, Launcher};
use eclipse_launcher::i18n::tr;
use eclipse_launcher::{backend, native_mods, native_status};
use std::collections::HashMap;
use std::time::Instant;

#[derive(Default)]
pub(super) enum Phase {
    #[default]
    Idle,
    Confirm {
        mods: Result<Vec<native_mods::Bundle>, String>,
        update: Option<String>,
    },
    Mismatch(String),
    Building {
        step: String,
        since: Instant,
        shown: u64,
    },
}

#[derive(Default)]
pub(super) struct NativeBuild {
    pub(super) phase: Phase,
    pub(super) for_instance: Option<String>,
    pub(super) ended: Option<Result<String, String>>,
    pub(super) then_play: Option<game_run::Start>,
    /// Instances (ids) whose native mods the automatic update moved on: Play builds their game
    /// first, then plays it.
    pub(super) rebuild: Vec<String>,
    pub(super) builds: std::cell::RefCell<HashMap<String, Option<native_status::Build>>>,
    /// The selected instance's state, the mods pinning an older build's commit
    /// (`native_status::pinning`), and the `native_status::fingerprint` they were read for.
    pub(super) state: std::cell::RefCell<Option<(String, native_status::State, Vec<String>)>>,
}

impl NativeBuild {
    pub(super) fn building(&self) -> bool {
        matches!(self.phase, Phase::Building { .. })
    }

    pub(super) fn step(&self) -> Option<&str> {
        self.step_since().map(|(step, _)| step)
    }

    pub(super) fn step_since(&self) -> Option<(&str, u64)> {
        match &self.phase {
            Phase::Building { step, since, .. } => Some((step, since.elapsed().as_secs())),
            _ => None,
        }
    }

    pub(super) fn tick(&mut self) -> bool {
        let Phase::Building { since, shown, .. } = &mut self.phase else {
            return false;
        };
        let now = since.elapsed().as_secs();
        std::mem::replace(shown, now) != now
    }

    pub(super) fn confirm(
        &mut self,
        mods: Result<Vec<native_mods::Bundle>, String>,
        update: Option<String>,
    ) {
        self.phase = Phase::Confirm { mods, update };
    }

    pub(super) fn close_warning(&mut self) {
        if matches!(self.phase, Phase::Confirm { .. }) {
            self.phase = Phase::Idle;
        }
    }
}

impl Launcher {
    /// The native mods turned on in the instance Play starts, unpacked and checked, with their
    /// bundles: what the warning names and the build compiles.
    pub(super) fn native_mods_to_build(
        &self,
    ) -> Result<(native_mods::Unpacked, Vec<native_mods::Bundle>), String> {
        let home = self
            .selected_instance()
            .map_or_else(backend::game_home, |i| i.home.clone());
        let folder = native_mods::installed_mods(&home)?;
        let found = native_mods::bundles(folder.path())?;
        Ok((folder, found))
    }

    /// What installed native build `version` was made from (`native_status::Build`), read once.
    pub(super) fn native_build_of(&self, version: &str) -> Option<native_status::Build> {
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
    pub(super) fn build_native(&mut self, anyway: bool, then_play: Option<game_run::Start>) {
        if self.native.building() {
            return;
        }
        let Some(instance) = self.selected_instance().cloned() else {
            return;
        };
        let update = match std::mem::take(&mut self.native.phase) {
            Phase::Confirm { update, .. } => update,
            _ => None,
        };
        self.native.ended = None;
        // This attempt's own log, starting empty: an open panel shows it rather than the last
        // attempt's (`native_build_log`).
        self.native_log_reset();
        self.native.for_instance = Some(instance.id.clone());
        let plays = self.version_of(&instance).cloned();
        let on_base = update.is_some();
        let base = match update {
            Some(newer) => Some(newer),
            None => native_mods::base_for(plays.as_deref(), &self.versions.installed, false),
        };
        let Some(base) = base else {
            let error = tr("Install Eclipse first.").to_string();
            self.native_log_preflight(&error);
            self.native.ended = Some(Err(error));
            return;
        };
        let folder = match self.native_mods_to_build() {
            Ok((folder, _)) => folder,
            Err(e) => {
                self.native_log_preflight(&e);
                self.native.ended = Some(Err(e));
                return;
            }
        };
        self.native.phase = Phase::Building {
            step: tr("Starting the build").into(),
            since: Instant::now(),
            shown: 0,
        };
        self.native.then_play = then_play;
        let settings = self.settings.clone();
        self.in_background(move |tell| {
            let result = native_mods::build(
                &base,
                folder.path(),
                &instance,
                anyway,
                on_base,
                &settings,
                |step| tell.send(Msg::NativeProgress(step.label())),
            );
            tell.send(Msg::Native(result));
        });
    }

    pub(super) fn native_moved(&mut self, now: String) {
        if let Phase::Building { step, .. } = &mut self.native.phase {
            *step = now;
        }
    }

    pub(super) fn native_built(&mut self, result: Result<String, native_mods::BuildError>) {
        // The last step of a build that worked says how it went.
        let last = match std::mem::take(&mut self.native.phase) {
            Phase::Building { step, .. } => Some(step),
            _ => None,
        };
        self.versions.installed = backend::installed();
        self.reload_instances();
        // A build attempt ended: an open panel reads the log it left.
        self.native_log.stale = true;
        let then_play = resumed(self.native.then_play.take(), &result, |tag| {
            self.can_start(tag)
        });
        match result {
            Err(native_mods::BuildError::Mismatch(list)) => {
                self.native.phase = Phase::Mismatch(list)
            }
            Err(native_mods::BuildError::Failed(e)) => self.native.ended = Some(Err(e)),
            Ok(_) => self.native.ended = last.map(Ok),
        }
        if let Some(start) = then_play {
            self.start_client(start);
        }
    }
}

pub(super) fn resumed(
    then_play: Option<game_run::Start>,
    built: &Result<String, native_mods::BuildError>,
    can_start: impl Fn(&str) -> bool,
) -> Option<game_run::Start> {
    then_play.filter(|start| built.is_ok() && can_start(&start.tag))
}
