use super::*;
use crate::game_run::Phase;
use crate::ui::Tone;
use eclipse_launcher::instances::Instance;

pub(super) struct PrepareFailed {
    pub(super) missing: bool,
    pub(super) title: String,
    pub(super) cause: String,
}

impl PrepareFailed {
    pub(super) fn of(error: &str, version: &str) -> Self {
        let args = [("version", version)];
        match minecraft_missing(error, version) {
            true => Self {
                missing: true,
                title: trf("Minecraft {version} not found", &args),
                cause: trf(
                    "Play {version} once in your Minecraft launcher, then try again.",
                    &args,
                ),
            },
            false => Self {
                missing: false,
                title: tr("Couldn't set up the game").into(),
                cause: ui::told_cause(error).unwrap_or_else(|| {
                    trf("Eclipse couldn't get Minecraft {version} ready.", &args)
                }),
            },
        }
    }
}

pub(super) fn minecraft_missing(error: &str, version: &str) -> bool {
    error.starts_with(&eclipse_launcher::sources::not_found_line(version))
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
                Tone::Off,
                trf(
                    "Built with mods · {version}",
                    &[("version", &build.eclipse_short())],
                ),
            ),
            InstallState::Ready { pinned: Some(tag) } => (
                Tone::Off,
                trf(
                    "Ready · {version}",
                    &[("version", &eclipse_launcher::nightly::display_name(tag))],
                ),
            ),
            InstallState::Ready { pinned: None } => (Tone::Off, tr("Ready").into()),
        }
    }

    pub(super) fn short_line(&self) -> (Tone, String) {
        match self {
            InstallState::NotSetUp { .. } => (Tone::Off, tr("Needs Minecraft").into()),
            _ => self.line(),
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
            if self.versions.queued(tag) {
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
}
