use super::isolate::LeftOut;
use super::run::log_warning;
use super::target::Target;
use super::{is_native, Profile};
use crate::backend;
use crate::i18n::{tr, trf};
use crate::native_cache::Dirs;
use eclipse_mods::index::native as hnmod;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// What a native build was made from, kept per profile in `native/built.json` after it compiled:
/// what the next build is compared with to say what changed.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Record {
    pub eclipse: String,
    pub mixins: String,
    /// Each mod's id and the hash of its files.
    pub mods: std::collections::BTreeMap<String, String>,
}

/// What a build is going to do, said before it starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Plan {
    /// The build already exists: nothing compiles.
    UpToDate,
    /// `native/target/` holds an earlier build of this profile: only what changed recompiles. The
    /// list says what changed.
    Incremental(Vec<String>),
    /// Nothing built yet in this profile: the whole game compiles.
    First,
}

impl Plan {
    pub fn label(&self) -> String {
        match self {
            Self::UpToDate => tr("Up to date (no build needed)").into(),
            Self::Incremental(changes) => trf(
                "Incremental rebuild: {changes}",
                &[("changes", &changes.join("; "))],
            ),
            Self::First => tr("First build: the whole game (several minutes)").into(),
        }
    }

    /// The word in "Built in 42 s (performance, incremental)".
    pub(super) fn kind(&self) -> &'static str {
        match self {
            Self::UpToDate => "up to date",
            Self::Incremental(_) => "incremental",
            Self::First => "first build",
        }
    }
}

/// Which case a build is: `exists` (its version is installed), else whether an earlier build of
/// the profile (`before`) is there to rebuild from, and what differs from `now`.
pub fn classify(exists: bool, before: Option<&Record>, now: &Record) -> Plan {
    if exists {
        return Plan::UpToDate;
    }
    let Some(before) = before else {
        return Plan::First;
    };
    let mut changes = Vec::new();
    if before.eclipse != now.eclipse {
        let short = |r: &str| r.chars().take(7).collect::<String>();
        changes.push(format!(
            "Eclipse commit {} to {}",
            short(&before.eclipse),
            short(&now.eclipse)
        ));
    }
    let changed: Vec<&str> = now
        .mods
        .iter()
        .filter(|(id, hash)| before.mods.get(*id) != Some(hash))
        .map(|(id, _)| id.as_str())
        .chain(
            before
                .mods
                .keys()
                .filter(|id| !now.mods.contains_key(*id))
                .map(String::as_str),
        )
        .collect();
    if !changed.is_empty() {
        changes.push(format!("mods {}", changed.join(", ")));
    }
    if before.mixins != now.mixins {
        changes.push("horizon-mixins".into());
    }
    if changes.is_empty() {
        changes.push("the build settings".into());
    }
    Plan::Incremental(changes)
}

/// `native/built.json`: each profile's last build, by [`record_key`]. A missing or unreadable
/// file is no record: the next build plans as a first one and writes it again.
#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
struct Records(BTreeMap<String, Record>);

impl Records {
    fn read() -> Self {
        fs::read(Dirs::current().records())
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    fn write(&self) -> Result<(), String> {
        crate::files::write_json(&Dirs::current().records(), self)
    }
}

/// A build record's key in `built.json`: the profile's folder in `native/target/` for a game
/// build (as it always was), the target's own name before it for a server one, so the two never
/// read each other's record. `Performance` and `Dev` share one: their last build in that folder.
fn record_key(target: Target, profile: Profile, platform: Option<&str>) -> String {
    let folder = profile.cargo().0;
    let key = match target {
        Target::Client => folder.into(),
        Target::Server => format!("{}-{folder}", target.name()),
    };
    match platform {
        Some(platform) => format!("{platform}-{key}"),
        None => key,
    }
}

/// The last build of `target` in `profile`'s folder in `native/target/`, if that folder (the
/// compiled crates both build from) is still there.
pub(super) fn last_record(
    target: Target,
    profile: Profile,
    platform: Option<&str>,
) -> Option<Record> {
    if !Dirs::current()
        .target(platform)
        .join(profile.cargo().0)
        .is_dir()
    {
        return None;
    }
    Records::read()
        .0
        .remove(&record_key(target, profile, platform))
}

/// Keeps `record` as the profile's last build; a failure only costs the next build's plan.
pub(super) fn save_record(
    target: Target,
    profile: Profile,
    platform: Option<&str>,
    record: &Record,
) {
    let mut all = Records::read();
    all.0
        .insert(record_key(target, profile, platform), record.clone());
    if let Err(e) = all.write() {
        log_warning(&e);
    }
}

pub(super) const RECORD: &str = "native-mods.json";

/// What a native build was made from: its `native-mods.json`, written by the build, read by the
/// Mods page and by the next build. Every field but `base` and `eclipse` may be missing from a
/// build made before it was recorded, and a list's entry that doesn't read is left out.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct About {
    /// The release or Nightly it started from.
    pub base: String,
    /// The Eclipse commit it compiled.
    pub eclipse: String,
    /// The commit of `base` itself, when the build knew it: `eclipse` differs from it when mods
    /// pin another.
    #[serde(default)]
    pub base_eclipse: Option<String>,
    /// The mods whose pin made the build use `eclipse` instead of `base_eclipse` (empty when the
    /// build is on `base`'s own commit).
    #[serde(default, deserialize_with = "readable")]
    pub pinned_by: Vec<String>,
    /// The player chose to build on `base`'s own commit even where mods pin another: a rebuild
    /// from this build keeps doing so.
    #[serde(default)]
    pub on_base: bool,
    #[serde(default)]
    pub horizon_mixins: String,
    #[serde(default)]
    pub hooks: Option<u32>,
    #[serde(default)]
    pub rust: String,
    #[serde(default)]
    pub profile: String,
    /// What it compiles: the game the player plays (also what a record from before targets were
    /// recorded built), or the headless engine a server runs.
    #[serde(default)]
    pub target: Target,
    #[serde(default)]
    pub rust_target: Option<String>,
    #[serde(default, deserialize_with = "readable")]
    pub mods: Vec<BuiltMod>,
    /// The mods turned on that it was built without, and why.
    #[serde(default, deserialize_with = "readable")]
    pub left_out: Vec<LeftOut>,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BuiltMod {
    pub id: String,
    pub version: String,
    #[serde(default)]
    pub sha256: Option<String>,
}

fn readable<'de, D, T>(from: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    let values: Option<Vec<serde_json::Value>> = serde::Deserialize::deserialize(from)?;
    Ok(values
        .into_iter()
        .flatten()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect())
}

impl About {
    fn read(dir: &Path) -> Option<Self> {
        let mut about: Self = serde_json::from_slice(&fs::read(dir.join(RECORD)).ok()?).ok()?;
        if !hnmod::revision(&about.eclipse) {
            return None;
        }
        about.base_eclipse = about.base_eclipse.filter(|r| hnmod::revision(r));
        Some(about)
    }

    pub(super) fn write(&self, dir: &Path) -> Result<(), String> {
        let text = serde_json::to_string(self).map_err(|e| e.to_string())?;
        let path = dir.join(RECORD);
        fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// A build from before `base_eclipse` was recorded that compiled another commit than its
    /// Nightly's: mods pinned it.
    pub fn off_its_nightly(&self) -> bool {
        self.base_eclipse.is_none()
            && crate::nightly::commit_of(&self.base)
                .is_some_and(|commit| !self.eclipse.starts_with(commit))
    }
}

/// What installed native build `version` was made from; None for any other version.
pub fn about(version: &str) -> Option<About> {
    if !is_native(version) {
        return None;
    }
    About::read(&backend::version_dir(version))
}

#[cfg(test)]
mod tests {
    use super::super::isolate;
    use super::super::key::version_for;
    use super::super::manifest::bundles;
    use super::*;
    use crate::native_mods::tests::{dir, write_bundle, A, B};
    use crate::rust_toolchain;

    #[test]
    fn each_profile_has_its_own_key_and_performance_shares_its_folder_with_dev() {
        let d = dir("profile-key");
        write_bundle(&d, "hello", "fn hook() {}");
        let found = bundles(&d).unwrap();
        let keys: std::collections::BTreeSet<_> =
            [Profile::Performance, Profile::Dev, Profile::Release]
                .into_iter()
                .map(|p| version_for(A, B, p, Target::Client, None, &found).unwrap())
                .collect();
        assert_eq!(keys.len(), 3);
        // The last build in `native/target/fast/` is either's, as `fast`'s was before `dev`.
        assert_eq!(
            record_key(Target::Client, Profile::Performance, None),
            "fast"
        );
        assert_eq!(record_key(Target::Client, Profile::Dev, None), "fast");
        assert_eq!(
            record_key(Target::Server, Profile::Release, None),
            "server-release"
        );
        assert_eq!(
            record_key(
                Target::Client,
                Profile::Performance,
                Some(rust_toolchain::WINDOWS_ARM64_TARGET)
            ),
            "aarch64-pc-windows-gnullvm-fast"
        );
    }

    /// A record (`native-mods.json`) says what it compiles; one from before targets were
    /// recorded is a client build, so it still reads ([`about`]).
    #[test]
    fn a_builds_record_names_its_target_and_older_ones_still_read() {
        let d = dir("about-target");
        let record = |target: Option<&str>| {
            let json = match target {
                Some(target) => {
                    format!(r#"{{"base":"v2026.10.13","eclipse":"{A}","target":"{target}"}}"#)
                }
                None => format!(r#"{{"base":"v2026.10.13","eclipse":"{A}"}}"#),
            };
            fs::write(d.join(RECORD), json).unwrap();
            About::read(&d).unwrap()
        };
        assert_eq!(
            record(None).target,
            Target::Client,
            "a build from before targets"
        );
        assert_eq!(record(Some("client")).target, Target::Client);
        assert_eq!(record(Some("server")).target, Target::Server);
        assert_eq!(record(Some("anything-else")).target, Target::Client);
        assert_eq!(record(None).eclipse, A);
        let old = record(None);
        assert_eq!(
            (old.base_eclipse, old.pinned_by, old.on_base),
            (None, Vec::new(), false),
            "a build from before pins were recorded"
        );
        fs::write(
            d.join("native-mods.json"),
            format!(
                r#"{{"base":"nightly-2026-11-04-bbbbbbb","eclipse":"{A}","base_eclipse":"{B}","pinned_by":["right_click_crops"],"on_base":false}}"#
            ),
        )
        .unwrap();
        let pinned = About::read(&d).unwrap();
        assert_eq!(pinned.base_eclipse.as_deref(), Some(B));
        assert_eq!(pinned.pinned_by, ["right_click_crops"]);
        // What a build writes reads back whole.
        let written = About {
            base: "nightly-2026-11-04-bbbbbbb".into(),
            eclipse: A.into(),
            base_eclipse: Some(B.into()),
            pinned_by: vec!["right_click_crops".into()],
            target: Target::Server,
            mods: vec![BuiltMod {
                id: "zoom".into(),
                version: "1.0.0".into(),
                sha256: Some("ab".repeat(32)),
            }],
            left_out: vec![LeftOut {
                id: "minimap".into(),
                version: "2.0.0".into(),
                why: isolate::Why::Needs("zoom".into()),
            }],
            ..Default::default()
        };
        written.write(&d).unwrap();
        assert_eq!(About::read(&d).unwrap(), written);
    }

    fn record(eclipse: &str, mods: &[(&str, &str)]) -> Record {
        Record {
            eclipse: eclipse.into(),
            mixins: "m1".into(),
            mods: mods
                .iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect(),
        }
    }

    #[test]
    fn a_build_is_up_to_date_incremental_or_the_first() {
        let now = record("aaaaaaa1111", &[("zoom", "1"), ("keys", "1")]);
        assert_eq!(classify(true, None, &now), Plan::UpToDate);
        assert_eq!(classify(false, None, &now), Plan::First);
        assert_eq!(
            Plan::First.label(),
            "First build: the whole game (several minutes)"
        );
        let before = record("aaaaaaa1111", &[("zoom", "1"), ("keys", "0")]);
        assert_eq!(
            classify(false, Some(&before), &now),
            Plan::Incremental(vec!["mods keys".into()])
        );
        let mut next = record("bbbbbbb2222", &[("zoom", "2"), ("keys", "1")]);
        let plan = classify(false, Some(&before), &next);
        assert_eq!(
            plan.label(),
            "Incremental rebuild: Eclipse commit aaaaaaa to bbbbbbb; mods keys, zoom"
        );
        // A removed mod counts as a change too.
        next = record("aaaaaaa1111", &[("zoom", "1")]);
        let plan = classify(false, Some(&now), &next);
        assert_eq!(plan, Plan::Incremental(vec!["mods keys".into()]));
        assert_eq!(
            classify(false, Some(&now), &now),
            Plan::Incremental(vec!["the build settings".into()])
        );
    }
}
