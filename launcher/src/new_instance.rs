//! New instance's kinds: Eclipse, Vanilla (Minecraft: Java Edition as Mojang ships it) and Modded
//! (with Fabric, Quilt, Forge or NeoForge). A Java kind picks its version from Mojang's public
//! list (`minecraft_versions`), fetched once when the page first needs it; the game itself comes
//! with the first Play, for an account that owns it.
use super::*;
use eclipse_launcher::loaders::{self, Kind as Loader};
use eclipse_launcher::minecraft_versions::Versions;
use fields::Field;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NewKind {
    Eclipse,
    Vanilla,
    Modded,
}

/// What a new Java instance plays.
#[derive(Clone, Debug)]
pub(super) struct JavaChoice {
    pub(super) kind: NewKind,
    pub(super) loader: Loader,
    /// None: the newest release.
    pub(super) version: Option<String>,
    pub(super) snapshots: bool,
    pub(super) old: bool,
    /// The loader's version; None: its newest stable one.
    pub(super) loader_version: Option<String>,
    /// The loader's versions for the chosen Minecraft version, asked of its service once.
    pub(super) loader_versions: LoaderVersions,
}

/// A loader's versions for one Minecraft version, as the page has them.
#[derive(Clone, Debug, Default)]
pub(super) enum LoaderVersions {
    #[default]
    NotAsked,
    Loading(Loader, String),
    Ready(Loader, String, Arc<Vec<loaders::Listed>>),
    Failed(Loader, String),
}

impl LoaderVersions {
    /// The loader and Minecraft version they are for.
    fn of(&self) -> Option<(Loader, &str)> {
        match self {
            LoaderVersions::NotAsked => None,
            LoaderVersions::Loading(l, mc)
            | LoaderVersions::Ready(l, mc, _)
            | LoaderVersions::Failed(l, mc) => Some((*l, mc)),
        }
    }
}

impl Default for JavaChoice {
    fn default() -> Self {
        JavaChoice {
            kind: NewKind::Eclipse,
            loader: Loader::Fabric,
            version: None,
            snapshots: false,
            old: false,
            loader_version: None,
            loader_versions: LoaderVersions::NotAsked,
        }
    }
}

impl JavaChoice {
    /// The Minecraft version it makes: the chosen one, else the newest release.
    pub(super) fn minecraft(&self, versions: Option<&Versions>) -> Option<String> {
        self.version
            .clone()
            .or_else(|| versions.map(|v| v.latest_release.clone()))
    }

    /// The name it gets when none is typed: "Vanilla 1.21.4", "Fabric 1.21.4".
    pub(super) fn default_name(&self, minecraft: &str) -> String {
        match self.kind {
            NewKind::Modded => format!("{} {minecraft}", self.loader.name()),
            _ => format!("Vanilla {minecraft}"),
        }
    }
}

/// `base`, or "base 2", "base 3"… when an instance already has it (names ignore case, as
/// `instances::create` does).
fn free_name(base: String, taken: &[&str]) -> String {
    let free = |n: &String| !taken.iter().any(|t| t.eq_ignore_ascii_case(n));
    if free(&base) {
        return base;
    }
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(free)
        .unwrap_or(base)
}

/// Mojang's list as the page has it.
#[derive(Default)]
pub(super) enum VersionList {
    #[default]
    NotAsked,
    Loading,
    Ready(Versions),
    Failed(String),
}

impl Launcher {
    fn java_choice(&mut self) -> Option<&mut JavaChoice> {
        self.instances.draft.as_mut().map(|d| &mut d.java)
    }

    /// The name a new instance gets for what it plays now: "My Eclipse", "Vanilla 1.21.4",
    /// "Fabric 1.21.4", numbered past the ones already taken.
    fn default_name(&self) -> Option<String> {
        let j = &self.instances.draft.as_ref()?.java;
        let versions = match &self.instances.versions {
            VersionList::Ready(v) => Some(v),
            _ => None,
        };
        let base = match j.kind {
            NewKind::Eclipse => tr("My Eclipse").to_string(),
            _ => j.default_name(&j.minecraft(versions)?),
        };
        let taken: Vec<&str> = self
            .instances
            .list
            .iter()
            .map(|i| i.name.as_str())
            .collect();
        Some(free_name(base, &taken))
    }

    /// Changes what the new instance plays. Its name follows while it is the one the page gave
    /// (or none), never one the player typed.
    pub(super) fn change(&mut self, f: impl FnOnce(&mut JavaChoice)) {
        let before = self.default_name();
        if let Some(j) = self.java_choice() {
            let was = (j.loader, j.version.clone());
            f(j);
            if was != (j.loader, j.version.clone()) {
                j.loader_version = None;
            }
        }
        // An instance being edited keeps its name.
        if self
            .instances
            .draft
            .as_ref()
            .is_some_and(|d| d.editing.is_some())
        {
            return;
        }
        let typed = self.fields.value(Field::InstanceName).trim().to_string();
        if typed.is_empty() || Some(&typed) == before.as_ref() {
            let after = self.default_name().unwrap_or_default();
            self.fields.text(Field::InstanceName).set(after);
        }
    }

    /// Picks the page's kind; a Java one fetches Mojang's list the first time.
    pub(super) fn pick_kind(&mut self, kind: NewKind, cx: &mut Context<Self>) {
        self.change(|j| j.kind = kind);
        if kind != NewKind::Eclipse {
            self.load_versions(cx);
        }
    }

    /// Asks Mojang for its versions, the first time the page needs them.
    pub(super) fn load_versions(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.instances.versions, VersionList::NotAsked) {
            return;
        }
        self.instances.versions = VersionList::Loading;
        let task = cx
            .background_executor()
            .spawn(async { eclipse_launcher::minecraft_versions::load() });
        cx.spawn(async move |this, cx| {
            let list = task.await;
            let _ = this.update(cx, |this, cx| {
                let before = this.fields.value(Field::InstanceName).trim().is_empty();
                this.instances.versions = match list {
                    Ok(v) => VersionList::Ready(v),
                    Err(e) => VersionList::Failed(e),
                };
                // The newest release is known now: so is the name.
                if before {
                    this.change(|_| {});
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Asks the chosen loader's service for its versions for `minecraft`, unless the page has
    /// them or is asking already.
    pub(super) fn ask_loader_versions(&mut self, minecraft: &str, cx: &mut Context<Self>) {
        let Some(j) = self.java_choice() else {
            return;
        };
        let loader = j.loader;
        if j.kind != NewKind::Modded || j.loader_versions.of() == Some((loader, minecraft)) {
            return;
        }
        j.loader_versions = LoaderVersions::Loading(loader, minecraft.to_string());
        let mc = minecraft.to_string();
        let task = cx.background_executor().spawn(async move {
            loaders::versions(loader, &mc, &eclipse_launcher::backend::get_bytes)
        });
        let mc = minecraft.to_string();
        cx.spawn(async move |this, cx| {
            let listed = task.await;
            let _ = this.update(cx, |this, cx| {
                let Some(j) = this.java_choice() else {
                    return;
                };
                // A list for what is no longer chosen is dropped.
                if j.loader_versions.of() != Some((loader, mc.as_str())) {
                    return;
                }
                j.loader_versions = match listed {
                    Ok(l) => LoaderVersions::Ready(loader, mc, Arc::new(l)),
                    Err(_) => LoaderVersions::Failed(loader, mc),
                };
                cx.notify();
            });
        })
        .detach();
    }
}

/// Makes a Java instance: `name`, playing `minecraft`, with the loader version chosen for Modded
/// (none: its newest stable one), asked of its service now, so a version it doesn't support
/// says so here.
pub(super) fn create_java(
    choice: &JavaChoice,
    name: &str,
    minecraft: &str,
) -> Result<instances::Instance, String> {
    let loader = (choice.kind == NewKind::Modded)
        .then(|| {
            loaders::choose(
                choice.loader.id(),
                minecraft,
                choice.loader_version.as_deref(),
            )
        })
        .transpose()?;
    let made = instances::create_java(name, minecraft)?;
    match loader {
        Some(loader) => instances::edit(&made.id, |m| {
            m.java.get_or_insert_with(Default::default).loader = Some(loader)
        }),
        None => Ok(made),
    }
}

/// `to`, with the newest stable version of a loader it names without one (asked of its service
/// now, so a Minecraft version the loader doesn't support says so here).
pub(super) fn with_loader(to: instances::Plays) -> Result<instances::Plays, String> {
    match to {
        instances::Plays::Java {
            minecraft,
            loader: Some(l),
        } if l.version.is_empty() => Ok(instances::Plays::Java {
            loader: Some(loaders::choose(l.kind.id(), &minecraft, None)?),
            minecraft,
        }),
        other => Ok(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_java_instance_is_named_after_what_it_plays() {
        let mut choice = JavaChoice {
            kind: NewKind::Vanilla,
            ..Default::default()
        };
        assert_eq!(choice.default_name("1.21.4"), "Vanilla 1.21.4");
        choice.kind = NewKind::Modded;
        choice.loader = Loader::Quilt;
        assert_eq!(choice.default_name("1.21.4"), "Quilt 1.21.4");
        assert_eq!(choice.minecraft(None), None);
        choice.version = Some("1.20.1".into());
        assert_eq!(choice.minecraft(None).as_deref(), Some("1.20.1"));
    }

    #[test]
    fn a_default_name_skips_the_ones_taken() {
        assert_eq!(free_name("My Eclipse".into(), &["Eclipse"]), "My Eclipse");
        assert_eq!(
            free_name("My Eclipse".into(), &["my eclipse", "My Eclipse 2"]),
            "My Eclipse 3"
        );
    }
}
