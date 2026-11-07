use crate::messages::Msg;
use crate::play_state::PlayState;
use crate::{menus, Launcher, Load, Pick};
use eclipse_launcher::backend::{Release, Updates};
use eclipse_launcher::i18n::tr;
use eclipse_launcher::{backend, build_import};
use gpui::prelude::*;
use std::time::Instant;

pub(super) struct Versions {
    pub(super) releases: Load<Vec<Release>>,
    pub(super) installed: Vec<String>,
    pub(super) installing: Option<Install>,
    pub(super) removing: Vec<String>,
    pub(super) disk: Vec<(String, u64)>,
    pub(super) disk_for: Vec<String>,
    pub(super) install_error: Option<(String, String)>,
    pub(super) importing: bool,
    pub(super) import_error: Option<String>,
    pub(super) install_next: Option<Release>,
}

pub(super) struct Install {
    pub(super) tag: String,
    pub(super) bar: (f32, f32),
    started: Instant,
    why: backend::InstallWhy,
}

impl Install {
    pub(super) fn done(&self) -> f32 {
        self.bar.1
    }
}

impl Versions {
    pub(super) fn new(installed: Vec<String>) -> Self {
        Self {
            releases: Load::Loading,
            installed,
            installing: None,
            removing: Vec::new(),
            disk: Vec::new(),
            disk_for: Vec::new(),
            install_error: None,
            importing: false,
            import_error: None,
            install_next: None,
        }
    }

    pub(super) fn installing(&self, tag: &str) -> Option<&Install> {
        self.installing.as_ref().filter(|i| i.tag == tag)
    }

    pub(super) fn queued(&self, tag: &str) -> bool {
        self.install_next.as_ref().is_some_and(|r| r.tag == tag)
    }

    pub(super) fn update_error(&self, updates: &Updates) -> Option<&str> {
        let newest = updates.eclipse.as_ref()?;
        (self.install_error.as_ref())
            .filter(|(tag, _)| self.installing.is_none() && *tag == newest.tag)
            .map(|(_, e)| e.as_str())
    }

    pub(super) fn release(&self, tag: &str) -> Option<&Release> {
        match &self.releases {
            Load::Ready(list) => list.iter().find(|r| r.tag == tag && r.asset.is_some()),
            _ => None,
        }
    }
}

/// Whether `tag` can start being deleted now: it is installed, no install is writing it, and it
/// is not already going. Another version deleting at the same time does not stop it.
fn removable(
    installed: &[String],
    installing: Option<&str>,
    removing: &[String],
    tag: &str,
) -> bool {
    installing != Some(tag)
        && installed.iter().any(|t| t == tag)
        && !removing.iter().any(|t| t == tag)
}

impl Launcher {
    /// The version Play starts: the selected instance's own, else the launcher's.
    pub(super) fn selected_version(&self) -> Option<&String> {
        match self.selected_instance().and_then(|i| self.plays(i)) {
            Some(plays) => plays.installed_tag(),
            None => self.default_version(),
        }
    }

    /// The launcher's version: what instances without one of their own play.
    pub(super) fn default_version(&self) -> Option<&String> {
        backend::followed_version(
            self.settings.default_version.as_ref(),
            &self.versions.installed,
        )
    }

    pub(super) fn use_version(&mut self, tag: String) {
        let eclipse = self.selected_instance().is_some_and(|i| i.java.is_none());
        if eclipse {
            self.pin_version((Some(&tag) != self.default_version()).then_some(tag));
        } else {
            self.settings.default_version = Some(tag);
            self.save_settings();
        }
    }

    pub(super) fn check_updates(&mut self) {
        self.update_check = UpdateCheck::Checking(Instant::now());
        self.in_background(|tell| {
            tell.send(Msg::Updates(backend::check_updates().map(Box::new)));
        });
    }

    pub(super) fn fetch_releases(&mut self) {
        self.versions.releases = Load::Loading;
        self.in_background(|tell| tell.send(Msg::Releases(backend::releases())));
    }

    pub(super) fn fetch_startup_releases(&mut self) {
        self.update_check = UpdateCheck::Checking(Instant::now());
        self.in_background(|tell| {
            let updates =
                backend::releases_and_updates(|releases| tell.send(Msg::Releases(releases)));
            tell.send(Msg::Updates(updates.map(Box::new)));
        });
    }

    pub(super) fn releases_arrived(&mut self, releases: Result<Vec<Release>, String>) {
        if let (Ok(_), Some(checked)) = (&releases, self.update_check.since()) {
            self.perf.record_since("launcher.release_list_ms", checked);
        }
        self.versions.releases = releases.map_or_else(Load::Failed, Load::Ready)
    }

    pub(super) fn updates_arrived(&mut self, updates: Result<Box<Updates>, String>) {
        let updates = match updates {
            Ok(updates) => updates,
            Err(e) => return self.update_check.answered(Err(e)),
        };
        self.update_check.answered(Ok(()));
        self.updates = *updates;
        if self.settings.auto_install() {
            self.install_update();
        }
        if let Some(release) = self.updates.launcher.clone() {
            self.update_launcher(release);
        }
    }

    /// Installs the Eclipse version the last check found, keeping the player's choice of
    /// version unless they were on the newest one.
    pub(super) fn install_update(&mut self) {
        let Some(release) = self.updates.eclipse.clone() else {
            return;
        };
        if self.versions.installing.is_some() || self.versions.removing.contains(&release.tag) {
            return;
        }
        if let Some(warning) = backend::nightly_downgrade(&release, &self.versions.installed) {
            crate::toast::say(warning);
        }
        let select = backend::update_selects(
            &release,
            self.settings.default_version.as_deref(),
            &self.versions.installed,
        );
        self.install(&release, backend::InstallWhy::Update { select });
    }

    pub(super) fn reinstall(&mut self, tag: &str) {
        let Some(release) = self.versions.release(tag).cloned() else {
            return;
        };
        let playing = self
            .instances
            .list
            .iter()
            .filter(|i| self.version_of(i).map(String::as_str) == Some(tag))
            .any(|i| self.play_state(i) != PlayState::Idle);
        if playing || self.versions.installing.is_some() {
            return;
        }
        self.game.prepare_error = None;
        self.install(&release, backend::InstallWhy::Added);
    }

    /// Installs `release` for an instance made or edited to play it, after the install running
    /// now if there is one.
    pub(super) fn install_for_instance(&mut self, release: Release) {
        match &self.versions.installing {
            Some(now) if now.tag == release.tag => {}
            Some(_) => self.versions.install_next = Some(release),
            None => self.install(&release, backend::InstallWhy::ForInstance),
        }
    }

    pub(super) fn install(&mut self, release: &Release, why: backend::InstallWhy) {
        if release.asset.is_none() || self.versions.removing.contains(&release.tag) {
            return;
        }
        let release = release.clone();
        let tag = release.tag.clone();
        self.versions.installing = Some(Install {
            tag: tag.clone(),
            bar: (0., 0.),
            started: Instant::now(),
            why,
        });
        self.versions.install_error = None;
        self.in_background(move |tell| {
            let result = backend::install(&release, |p| tell.send(Msg::Progress(tag.clone(), p)));
            tell.send(Msg::Installed(tag, result));
        });
    }

    pub(super) fn install_moved(&mut self, tag: String, done: f32) {
        if let Some(install) = self.versions.installing.as_mut().filter(|i| i.tag == tag) {
            install.bar = (install.bar.1, done);
        }
    }

    pub(super) fn installed(&mut self, tag: String, result: Result<(), String>) {
        let install = self.versions.installing.take().filter(|i| i.tag == tag);
        self.versions.installed = backend::installed();
        let why = install
            .as_ref()
            .map_or(backend::InstallWhy::Added, |i| i.why);
        let update = matches!(why, backend::InstallWhy::Update { .. });
        if let (Ok(()), Some(install)) = (&result, &install) {
            let name = if update {
                "launcher.update_ms"
            } else {
                "launcher.install_ms"
            };
            self.perf.record_since(name, install.started);
        }
        match result {
            Ok(()) => {
                if self.updates.eclipse.as_ref().is_some_and(|r| r.tag == tag) {
                    self.updates.eclipse = None;
                }
                // An instance's own version leaves the other instances on theirs.
                let selected = self.default_version().map(String::as_str);
                if backend::install_selects(&tag, why, selected) {
                    self.settings.default_version = Some(tag.clone());
                }
                // Pruning reads settings from disk: protect the selection only once it has been
                // saved there. A failed save leaves all versions in place.
                if self.save_settings() {
                    self.in_background(move |tell| {
                        tell.send(Msg::Pruned(backend::after_install(&tag, update)))
                    });
                }
            }
            Err(e) => self.versions.install_error = Some((tag, e)),
        }
        if let Some(next) = self.versions.install_next.take() {
            self.install_for_instance(next);
        }
    }

    pub(super) fn pruned(&mut self, gone: Vec<String>) {
        if !gone.is_empty() {
            self.versions.installed = backend::installed();
        }
    }

    /// Deletes an installed version on a background thread: a version is thousands of files and
    /// the window must keep drawing meanwhile. `Msg::Removed` ends it.
    pub(super) fn remove(&mut self, tag: &str) {
        self.stop_asking(|a| matches!(a, menus::Asking::Version(_)));
        let installing = self.versions.installing.as_ref().map(|i| i.tag.as_str());
        if !removable(
            &self.versions.installed,
            installing,
            &self.versions.removing,
            tag,
        ) {
            return;
        }
        self.versions.install_error = None;
        self.versions.removing.push(tag.to_string());
        self.versions.installed.retain(|t| t != tag);
        self.settings.default_version = self.default_version().cloned();
        let tag = tag.to_string();
        self.in_background(move |tell| {
            let result = backend::remove_version(&tag);
            tell.send(Msg::Removed(tag, result));
        });
    }

    pub(super) fn removed(&mut self, tag: String, result: Result<(), String>) {
        self.versions.removing.retain(|t| *t != tag);
        self.versions.installed = backend::installed();
        self.pull_settings();
        self.settings.default_version = self.default_version().cloned();
        if let Err(e) = result {
            self.versions.install_error = Some((tag, e));
        }
    }

    /// Asks for a game program (or a zip holding one) and installs it as its own version on a
    /// thread: a version is thousands of files. `Msg::Imported` ends it.
    pub(super) fn import_build(&mut self, cx: &mut Context<Self>) {
        if !self.can_import_build() {
            return;
        }
        self.pick_paths(Pick::File, tr("Import"), cx, |this, mut paths, cx| {
            // Another picker's import, or a removal, may have started meanwhile.
            if let Some(path) = paths.pop() {
                this.start_build_import(path, cx);
            }
        });
    }
    /// Whether a build may be imported now. A removal still running could take
    /// the folder of the same program imported again.
    pub(super) fn can_import_build(&self) -> bool {
        !self.versions.importing && self.versions.removing.is_empty()
    }
    /// Starts importing the build at `path` on a thread, as the picker does.
    /// Separate from [`Self::import_build`] so a file dropped on the window can
    /// start the same import (`import_dropped`) without a picker. Does nothing
    /// when one is already running. `Msg::Imported` ends it.
    pub(super) fn start_build_import(&mut self, path: std::path::PathBuf, cx: &mut Context<Self>) {
        if !self.can_import_build() {
            return;
        }
        self.versions.importing = true;
        self.versions.import_error = None;
        self.in_background(move |tell| {
            tell.send(Msg::Imported(build_import::import(&path, None)));
        });
        cx.notify();
    }

    pub(super) fn imported(&mut self, result: Result<String, String>) {
        self.versions.importing = false;
        self.versions.installed = backend::installed();
        match result {
            // `build_import::import` saved the selection.
            Ok(_) => self.pull_settings(),
            Err(e) => self.versions.import_error = Some(e),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum UpdateCheck {
    Never,
    Checking(Instant),
    Checked(Instant),
    Failed(Instant, String),
}

impl UpdateCheck {
    pub(super) fn since(&self) -> Option<Instant> {
        match self {
            UpdateCheck::Never => None,
            UpdateCheck::Checking(at) | UpdateCheck::Checked(at) | UpdateCheck::Failed(at, _) => {
                Some(*at)
            }
        }
    }

    pub(super) fn checking(&self) -> bool {
        matches!(self, UpdateCheck::Checking(_))
    }

    pub(super) fn clean(&self) -> bool {
        matches!(self, UpdateCheck::Checked(_))
    }

    pub(super) fn error(&self) -> Option<&str> {
        match self {
            UpdateCheck::Failed(_, e) => Some(e),
            _ => None,
        }
    }

    pub(super) fn answered(&mut self, result: Result<(), String>) {
        let at = self.since().unwrap_or_else(Instant::now);
        *self = match result {
            Ok(()) => UpdateCheck::Checked(at),
            Err(e) => UpdateCheck::Failed(at, e),
        };
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_second_version_can_be_removed_while_another_deletes() {
        let installed = ["v0.2.97".to_string(), "v0.2.76".to_string()];
        let removing = ["v0.2.97".to_string()];
        assert!(super::removable(&installed, None, &removing, "v0.2.76"));
        assert!(!super::removable(&installed, None, &removing, "v0.2.97"));
        assert!(!super::removable(
            &installed,
            Some("v0.2.76"),
            &[],
            "v0.2.76"
        ));
        assert!(!super::removable(&installed, None, &[], "v0.2.85"));
    }
}
