use super::*;
use eclipse_launcher::backend::Staged;

/// The launcher's own update, as the footer and Settings show it.
#[derive(Clone, PartialEq)]
pub(super) enum SelfUpdate {
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

impl Launcher {
    /// Recovery text shared by both sidebar styles and Settings.
    pub(super) fn launcher_update_help(&self, error: Option<&str>) -> String {
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

    pub(super) fn update_launcher(&mut self, release: Release) {
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
        self.in_background(move |tell| {
            let staged = backend::stage_update(&release, |p| tell.send(Msg::LauncherProgress(p)));
            tell.send(Msg::LauncherUpdated(staged));
        });
    }

    /// Update: the window closes, the installer puts the new launcher in place and starts it.
    /// When another launcher already put it there, this one only restarts.
    pub(super) fn apply_launcher_update(&mut self, cx: &mut App) {
        match eclipse_launcher::update_handoff::start_now() {
            Ok(true) => {
                servers_page::keep_running_through_update();
                shut_down(cx)
            }
            Ok(false) => self.self_update = SelfUpdate::Failed(backend::restart()),
            Err(e) => self.self_update = SelfUpdate::Failed(e),
        }
    }

    pub(super) fn launcher_update_moved(&mut self, done: f32) {
        if let SelfUpdate::Downloading(_, f) = &mut self.self_update {
            *f = done;
        }
    }

    pub(super) fn launcher_updated(&mut self, result: Result<Option<Staged>, String>) {
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
}
