//! Whether an instance plays its native mods (docs/native-mods.md): the mods turned on in its
//! folder against the ones its version was built with (`native-mods.json`, written by
//! `native_mods::build`). The Mods page shows the answer beside its Build button.
use crate::{browser::java, game_folder, native_mods};
use std::fs;
use std::path::Path;

/// Shown once, before the first native mod is installed.
pub const NOTICE: &str = "Native mods run with full access to your computer. Install only mods whose authors you trust. The first build takes 20 to 30 minutes.";

/// A native mod: its id, its version and a digest of its files (None for a build from before
/// `native-mods.json` kept them).
pub type Mod = (String, String, Option<String>);

/// Whether an installed file (`installed::Entry::file`) is a native mod.
pub fn is_native_file(file: &str) -> bool {
    file.strip_prefix(native_mods::INSTALLED)
        .is_some_and(|rest| rest.starts_with('/') && rest.ends_with(".hnmod"))
}

/// Where an instance's native mods stand.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    /// No native mod is turned on: nothing to build.
    NoMods,
    /// The version it plays isn't a native build.
    NotBuilt,
    /// The version it plays is a native build, made with other mods than those turned on.
    Outdated,
    /// The version it plays was built with exactly these mods.
    UpToDate,
}

/// The state for the mods turned on, `enabled`, and those of the version played, `built` (None
/// when it is no native build). Order doesn't matter; a mod at another version, or with other
/// files at the same version (a reinstalled local `.hnmod`), needs a build.
pub fn state(enabled: &[Mod], built: Option<&[Mod]>) -> State {
    if enabled.is_empty() {
        return State::NoMods;
    }
    let sorted = |mods: &[Mod]| {
        let mut v = mods.to_vec();
        v.sort();
        v.dedup();
        v
    };
    let same = |a: &Mod, b: &Mod| {
        a.0 == b.0 && a.1 == b.1 && (a.2.is_none() || b.2.is_none() || a.2 == b.2)
    };
    match built {
        None => State::NotBuilt,
        Some(built) => {
            let (built, enabled) = (sorted(built), sorted(enabled));
            if built.len() == enabled.len() && built.iter().zip(&enabled).all(|(a, b)| same(a, b)) {
                State::UpToDate
            } else {
                State::Outdated
            }
        }
    }
}

/// What a native build was made from, as the Mods page and the instance list show it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Build {
    /// The release or Nightly it was built on (`native_mods::About::base`).
    pub base: String,
    /// The Eclipse commit it compiled.
    pub eclipse: String,
    /// The mods whose pin made it compile `eclipse` rather than `base`'s own commit.
    pub pinned_by: Vec<String>,
    /// It compiled another commit than its Nightly's, with no record of why: built before the
    /// launcher kept which mods pinned it ([`pinning`] reads them from the instance's mods).
    pub pinned_unknown: bool,
    /// The mods turned on that it was built without, and why.
    pub left_out: Vec<native_mods::LeftOut>,
}

impl Build {
    /// Installed native build `version`'s, from its `native-mods.json`; None for another version.
    pub fn read(version: &str) -> Option<Build> {
        Some(Build::of(native_mods::about(version)?))
    }

    /// What build record `about` says.
    fn of(about: native_mods::About) -> Build {
        Build {
            pinned_unknown: about.off_its_nightly(),
            base: about.base,
            eclipse: about.eclipse,
            pinned_by: about.pinned_by,
            left_out: about.left_out,
        }
    }

    /// The Eclipse it was built on, as a player reads it (`eclipse_label`).
    pub fn eclipse(&self) -> String {
        eclipse_label(&self.base)
    }

    /// The same, short enough for the instance list: `Nightly 10-28`.
    pub fn eclipse_short(&self) -> String {
        match crate::nightly::date_of(&self.base) {
            Some(date) => format!("Nightly {}", &date[5..]),
            None => self.base.clone(),
        }
    }
}

/// A release or Nightly as a player reads it beside "Eclipse": `v0.2.93` as it is,
/// `nightly-2026-10-29-4baa571` as `Nightly 2026-10-29`.
pub fn eclipse_label(version: &str) -> String {
    match crate::nightly::date_of(version) {
        Some(date) => format!("Nightly {date}"),
        None => version.to_owned(),
    }
}

/// The native mods turned on in game folder `home`, read from their `.hnmod`s. One that can't be
/// read always counts, with its file name and no version, so it needs a build (which says why):
/// the on/off list is keyed by the manifest id, which an unreadable file can't give.
pub fn enabled(home: &Path) -> Vec<Mod> {
    let mut out = Vec::new();
    for path in native_mods::installed_files(home) {
        match eclipse_mods::index::native::Bundle::read(&path) {
            Ok(b) => {
                if java::is_enabled(home, &b.manifest.id) {
                    let digest = native_mods::digest(
                        b.files.iter().map(|(n, d)| (n.as_str(), d.as_slice())),
                    );
                    out.push((b.manifest.id, b.manifest.version, Some(digest)));
                }
            }
            Err(_) => out.push((stem(&path), String::new(), None)),
        }
    }
    out
}

/// The native mods turned on in game folder `home` that pin Eclipse commit `revision` (those naming
/// a hook points' version follow the others, so they pin nothing).
pub fn pinning(home: &Path, revision: &str) -> Vec<String> {
    bundles_on(home)
        .into_iter()
        .filter(|b| native_mods::pin(b.manifest.hooks, Some(&b.manifest.eclipse)) == Some(revision))
        .map(|b| b.manifest.id)
        .collect()
}

/// The readable native mods turned on in game folder `home`.
pub fn bundles_on(home: &Path) -> Vec<eclipse_mods::index::native::Bundle> {
    native_mods::installed_files(home)
        .into_iter()
        .filter_map(|path| eclipse_mods::index::native::Bundle::read(&path).ok())
        .filter(|b| java::is_enabled(home, &b.manifest.id))
        .collect()
}

/// The mods installed version `version` was built for, or None when it isn't a native build
/// (or its `native-mods.json` can't be read: the Mods page then offers Build): those it holds and
/// those it left out (a build already tried them: it is up to date with the same mods turned on).
pub fn built(version: &str) -> Option<Vec<Mod>> {
    native_mods::about(version).map(built_mods)
}

fn built_mods(about: native_mods::About) -> Vec<Mod> {
    let left_out = about
        .left_out
        .into_iter()
        .map(|left| (left.id, left.version, None));
    about
        .mods
        .into_iter()
        .map(|m| (m.id, m.version, m.sha256))
        .chain(left_out)
        .collect()
}

/// What [`state`] depends on, cheap to read on every frame: the folder, the version, the on/off
/// list and each `.hnmod`'s size and time. The Mods page reads the mods again when it changes.
pub fn fingerprint(home: &Path, version: Option<&str>) -> String {
    let mut out = format!("{}\n{version:?}\n", home.display());
    out += &fs::read_to_string(home.join(game_folder::MODS_LIST)).unwrap_or_default();
    for path in native_mods::installed_files(home)
        .into_iter()
        .flat_map(|p| [p.clone(), p.with_extension("origin.json")])
        .chain([home.join(game_folder::BROWSER_LIST)])
    {
        let meta = fs::metadata(&path).ok();
        let time = meta.as_ref().and_then(|m| m.modified().ok());
        out += &format!("\n{} {:?} {time:?}", path.display(), meta.map(|m| m.len()));
    }
    out
}

pub(crate) fn stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(id: &str, version: &str) -> Mod {
        (id.into(), version.into(), None)
    }

    fn d(id: &str, version: &str, digest: &str) -> Mod {
        (id.into(), version.into(), Some(digest.into()))
    }

    #[test]
    fn a_build_is_needed_until_the_version_holds_the_mods_turned_on() {
        let on = [m("veinminer", "1.0"), m("zoom", "2.1")];
        assert_eq!(state(&[], None), State::NoMods);
        assert_eq!(
            state(&[], Some(&on)),
            State::NoMods,
            "nothing on: nothing to build"
        );
        assert_eq!(state(&on, None), State::NotBuilt, "not a native build");
        let reordered = [m("zoom", "2.1"), m("veinminer", "1.0")];
        assert_eq!(state(&on, Some(&reordered)), State::UpToDate);
        assert_eq!(
            state(&on, Some(&[m("veinminer", "1.0")])),
            State::Outdated,
            "a mod turned on since"
        );
        assert_eq!(
            state(&on[..1], Some(&on)),
            State::Outdated,
            "a mod turned off since"
        );
        assert_eq!(
            state(&on, Some(&[m("veinminer", "1.1"), m("zoom", "2.1")])),
            State::Outdated,
            "a mod updated since"
        );
    }

    #[test]
    fn the_same_version_with_other_files_needs_a_build() {
        let on = [d("zoom", "2.1", "aa")];
        assert_eq!(state(&on, Some(&[d("zoom", "2.1", "aa")])), State::UpToDate);
        assert_eq!(
            state(&on, Some(&[d("zoom", "2.1", "bb")])),
            State::Outdated,
            "reinstalled without a version bump"
        );
        assert_eq!(
            state(&on, Some(&[m("zoom", "2.1")])),
            State::UpToDate,
            "a build from before the digest was kept"
        );
    }

    #[test]
    fn native_mods_json_gives_the_built_mods() {
        let record = |json: serde_json::Value| -> native_mods::About {
            serde_json::from_value(json).unwrap()
        };
        let about = record(serde_json::json!({
            "base": "v0.1.0", "eclipse": "abc", "rust": "1.99.1",
            "mods": [{"id": "zoom", "version": "2.1"}, {"id": "x"}, {"id": "y", "version": "1", "sha256": "ab"}],
        }));
        assert_eq!(built_mods(about), [m("zoom", "2.1"), d("y", "1", "ab")]);
        let bare = record(serde_json::json!({"base": "v0.1.0", "eclipse": "abc"}));
        assert!(built_mods(bare).is_empty());
        // A mod the build left out was tried: the same mods turned on need no new build, a new
        // version of it does.
        let about = record(serde_json::json!({
            "base": "v0.1.0", "eclipse": "abc",
            "mods": [{"id": "zoom", "version": "2.1"}],
            "left_out": [{"id": "hitbox_plus", "version": "1.3.0", "why": "other-version"}],
        }));
        let build = built_mods(about);
        let on = [m("zoom", "2.1"), m("hitbox_plus", "1.3.0")];
        assert_eq!(state(&on, Some(build.as_slice())), State::UpToDate);
        let on = [m("zoom", "2.1"), m("hitbox_plus", "1.3.1")];
        assert_eq!(state(&on, Some(build.as_slice())), State::Outdated);
        assert_eq!(built("v0.1.0"), None, "not a native build");
    }

    #[test]
    fn builds_name_the_eclipse_they_were_built_on() {
        assert_eq!(eclipse_label("v0.2.93"), "v0.2.93");
        assert_eq!(
            eclipse_label("nightly-2026-10-29-4baa571"),
            "Nightly 2026-10-29"
        );
        assert_eq!(eclipse_label("nightly-2026-10-29"), "Nightly 2026-10-29");
        let build = Build {
            base: "nightly-2026-10-28-0c02067".into(),
            ..Build::default()
        };
        assert_eq!(build.eclipse(), "Nightly 2026-10-28");
        assert_eq!(build.eclipse_short(), "Nightly 10-28");
        let release = Build {
            base: "v0.2.93".into(),
            ..Build::default()
        };
        assert_eq!(release.eclipse_short(), "v0.2.93");
        assert_eq!(Build::read("v0.1.0"), None, "not a native build");
    }

    /// A build from before pins were recorded that compiled another commit than its Nightly's
    /// was pinned by a mod (the Mods page then reads which from the instance's mods); one on its
    /// Nightly's commit, a release's, or a newer record's, says itself.
    #[test]
    fn an_older_build_off_its_nightlys_commit_was_pinned() {
        let about = |base: &str, eclipse: &str, base_eclipse: Option<&str>| native_mods::About {
            base: base.into(),
            eclipse: eclipse.into(),
            base_eclipse: base_eclipse.map(str::to_owned),
            ..Default::default()
        };
        let pin = "9d979caa48a3bb58203319b9ea044db58c38e0d8";
        let own = "d91cab6000000000000000000000000000000000";
        let nightly = "nightly-2026-11-01-d91cab6";
        assert!(Build::of(about(nightly, pin, None)).pinned_unknown);
        assert!(!Build::of(about(nightly, own, None)).pinned_unknown);
        assert!(!Build::of(about("v0.2.93", pin, None)).pinned_unknown);
        assert!(!Build::of(about(nightly, pin, Some(pin))).pinned_unknown);
    }

    #[test]
    fn only_hnmods_in_the_native_folder_are_native() {
        assert!(is_native_file("eclipse-native-mods/veinminer.hnmod"));
        assert!(!is_native_file("eclipse-native-mods-old/x.hnmod"));
        assert!(!is_native_file("eclipse-mods/zoom.emod"));
        assert!(!is_native_file("resourcepacks/x.zip"));
    }
}
