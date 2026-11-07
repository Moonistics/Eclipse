use super::*;

#[derive(Default)]
pub(super) struct NativeBuild {
    pub(super) step: Option<String>,
    pub(super) started: Option<(Instant, u64)>,
    pub(super) done: Option<String>,
    pub(super) error: Option<String>,
    pub(super) confirm: Option<Result<Vec<native_mods::Bundle>, String>>,
    pub(super) for_instance: Option<String>,
    pub(super) update: Option<String>,
    pub(super) on_base: bool,
    pub(super) builds: std::cell::RefCell<HashMap<String, Option<native_status::Build>>>,
    pub(super) mismatch: Option<String>,
    pub(super) then_play: Option<game_run::Start>,
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
        if self.native.step.is_some() {
            return;
        }
        let Some(instance) = self.selected_instance().cloned() else {
            return;
        };
        self.native.confirm = None;
        self.native.error = None;
        self.native.mismatch = None;
        // This attempt's own log, starting empty: an open panel shows it rather than the last
        // attempt's (`native_log_page`).
        self.native_log_reset();
        self.native.for_instance = Some(instance.id.clone());
        let plays = self.version_of(&instance).cloned();
        let on_base = std::mem::take(&mut self.native.on_base);
        let base = match self.native.update.take() {
            Some(newer) => Some(newer),
            None => native_mods::base_for(plays.as_deref(), &self.versions.installed, false),
        };
        let Some(base) = base else {
            let error = tr("Install Eclipse first.").to_string();
            self.native_log_preflight(&error);
            self.native.error = Some(error);
            return;
        };
        let folder = match self.native_mods_to_build() {
            Ok((folder, _)) => folder,
            Err(e) => {
                self.native_log_preflight(&e);
                self.native.error = Some(e);
                return;
            }
        };
        self.native.step = Some(tr("Starting the build").into());
        self.native.started = Some((Instant::now(), 0));
        self.native.done = None;
        self.native.then_play = then_play;
        let profile = native_mods::Profile::chosen(&self.settings);
        self.in_background(move |tell| {
            let result = native_mods::build(
                &base,
                folder.path(),
                &instance,
                anyway,
                on_base,
                profile,
                |step| tell.send(Msg::NativeProgress(step.label())),
            );
            tell.send(Msg::Native(result));
        });
    }

    pub(super) fn native_built(&mut self, result: Result<String, native_mods::BuildError>) {
        // The last step of a build that worked says how it went.
        let last = self.native.step.take();
        self.native.started = None;
        self.native.done = result.is_ok().then_some(last).flatten();
        self.versions.installed = backend::installed();
        self.reload_instances();
        // A build attempt ended: an open panel reads the log it left.
        self.native_log.stale = true;
        let then_play = self.native.then_play.take();
        match result {
            Err(native_mods::BuildError::Mismatch(list)) => self.native.mismatch = Some(list),
            Err(native_mods::BuildError::Failed(e)) => self.native.error = Some(e),
            // Play waited for this build (`Mods::rebuild`).
            Ok(_) => {
                if let Some(start) = then_play.filter(|s| self.can_start(&s.tag)) {
                    self.start_client(start);
                }
            }
        }
    }
}
