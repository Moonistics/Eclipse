use super::manifest::Bundle;
use super::mixins::MIXINS_REV;
use super::record::About;
use super::run::log_warning;
use crate::backend;
use crate::native_cache::Dirs;
use eclipse_mods::index::native as hnmod;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Pinned {
    Eclipse,
    Mixins,
}

impl Pinned {
    fn name(self) -> &'static str {
        match self {
            Self::Eclipse => "Eclipse",
            Self::Mixins => "horizon-mixins",
        }
    }

    fn of(self, bundle: &Bundle) -> Option<&String> {
        match self {
            Self::Eclipse => bundle.eclipse.as_ref(),
            Self::Mixins => bundle.mixins.as_ref(),
        }
    }

    fn repo(self) -> &'static str {
        match self {
            Self::Eclipse => backend::REPO,
            Self::Mixins => super::mixins::REPOSITORY,
        }
    }

    fn own(self) -> Option<&'static str> {
        match self {
            Self::Eclipse => None,
            Self::Mixins => Some(MIXINS_REV),
        }
    }
}

/// The one value the bundles pin for `field`, else `fallback`; mods that pin
/// different ones can't be built together, and the error says which.
fn agreed(
    bundles: &[Bundle],
    field: Pinned,
    fallback: impl FnOnce() -> Result<String, String>,
) -> Result<String, String> {
    let which = field.name();
    let mut pins: Vec<(&String, &str)> = bundles
        .iter()
        .filter_map(|b| field.of(b).map(|r| (r, b.id.as_str())))
        .collect();
    pins.sort();
    let revision = match pins.first() {
        None => fallback()?,
        Some((first, _)) if pins.iter().all(|(r, _)| r == first) => (*first).clone(),
        Some(_) => {
            let list: Vec<String> = pins
                .iter()
                .map(|(r, id)| format!("{id} → {}", short(r)))
                .collect();
            return Err(format!(
                "These mods were made for different {which} revisions and can't be built together: {}. Install versions made for the same Nightly.",
                list.join(", ")
            ));
        }
    };
    if !hnmod::revision(&revision) {
        return Err(format!(
            "{revision} is not a {which} commit (40 hex digits)."
        ));
    }
    Ok(revision)
}

/// The revision `bundles` build on for `field` when their pins differ: the newest of the pins and
/// the launcher's own horizon-mixins, by its commit's time (`date`), so no mod gets an
/// older one than it was made for. With one pin or none, it is [`agreed`]'s.
fn newest(
    bundles: &[Bundle],
    field: Pinned,
    fallback: impl FnOnce() -> Result<String, String>,
    date: impl Fn(&str) -> Result<String, String>,
) -> Result<String, String> {
    let which = field.name();
    let mut pins: Vec<&str> = bundles
        .iter()
        .filter_map(|b| field.of(b).map(String::as_str))
        .collect();
    pins.sort();
    pins.dedup();
    if pins.len() < 2 {
        return agreed(bundles, field, fallback);
    }
    if let Some(bad) = pins.iter().find(|p| !hnmod::revision(p)) {
        return Err(format!("{bad} is not a {which} commit (40 hex digits)."));
    }
    pins.extend(field.own().filter(|a| !pins.contains(a)));
    let mut dated = Vec::new();
    for pin in pins {
        let when = date(pin).map_err(|e| {
            format!("Can't tell which of the mods' {which} revisions is newest: {e}")
        })?;
        dated.push((when, pin));
    }
    Ok(dated
        .into_iter()
        .max()
        .map(|(_, p)| p.to_owned())
        .unwrap_or_default())
}

/// When commit `revision` of GitHub's `repo` was made (ISO 8601, so it sorts), from the dates kept
/// in [`Dirs::dates`] when it was asked before: a commit's date never changes, so a build of mods
/// made for different revisions can be used again offline or past GitHub's rate limit (GitHub's
/// own answers, [`crate::github`], are asked again after a few minutes).
fn committed(repo: &str, revision: &str) -> Result<String, String> {
    remembered(&Dirs::current().dates(), repo, revision, ask_github)
}

/// The date `file` (`{"repo@revision": date}`) keeps for `repo`'s `revision`, else `ask`'s, then
/// kept in `file`.
fn remembered(
    file: &Path,
    repo: &str,
    revision: &str,
    ask: impl FnOnce(&str, &str) -> Result<String, String>,
) -> Result<String, String> {
    let key = format!("{repo}@{revision}");
    let mut dates: BTreeMap<String, String> = fs::read(file)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    if let Some(date) = dates.get(&key) {
        return Ok(date.clone());
    }
    let date = ask(repo, revision)?;
    dates.insert(key, date.clone());
    if let Err(e) = crate::files::write_json(file, &dates) {
        log_warning(&e);
    }
    Ok(date)
}

fn ask_github(repo: &str, revision: &str) -> Result<String, String> {
    let commit = crate::github::api(&format!("repos/{repo}/commits/{revision}"))?;
    commit["commit"]["committer"]["date"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("GitHub doesn't know {repo} commit {revision}."))
}

/// Each pinned mod whose Eclipse revision isn't `revision`, as `id → pin`, and the build's.
fn mismatch(mods: &[Bundle], revision: &str) -> Option<String> {
    let list: Vec<String> = mods
        .iter()
        .filter_map(|b| b.pin().map(|r| (b, r)))
        .filter(|(_, r)| *r != revision)
        .map(|(b, r)| format!("{} → {}", b.id, short(r)))
        .collect();
    (!list.is_empty()).then(|| format!("{}; the build uses {}", list.join(", "), short(revision)))
}

/// Where Eclipse declares its native hook points' version, and the line that does.
const HOOKS_FILE: &str = "crates/eclipse-client/src/game/native_hooks.rs";
const HOOKS_LINE: &str = "pub(crate) const NATIVE_HOOKS_API: u32 = ";

/// The native hook points' version in `HOOKS_FILE`'s text.
fn hooks_in(source: &str) -> Option<u32> {
    let line = source.lines().find_map(|l| l.strip_prefix(HOOKS_LINE))?;
    line.trim_end().strip_suffix(';')?.parse().ok()
}

/// The native hook points' version of Eclipse at `revision`, from GitHub (None: it has none), or
/// why GitHub couldn't say.
pub(crate) fn hooks_at(revision: &str) -> Result<Option<u32>, String> {
    let text = crate::github::raw(backend::REPO, revision, HOOKS_FILE)?;
    Ok(text.as_deref().and_then(hooks_in))
}

/// The named hook points (name, version) a `native-hooks.json` lists; None when it isn't one.
fn catalog_in(text: &str) -> Option<HookCatalog> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    value["hooks"]
        .as_array()?
        .iter()
        .map(|h| {
            let version = u32::try_from(h["version"].as_u64()?).ok()?;
            Some((h["name"].as_str()?.to_owned(), version))
        })
        .collect()
}

/// The named hook points of Eclipse at `revision`, from GitHub (None: it has no catalog, as
/// revisions from before d3e0774 that already had hook points v2), or why GitHub couldn't say.
fn catalog_at(revision: &str) -> Result<Option<HookCatalog>, String> {
    let text = crate::github::raw(backend::REPO, revision, CATALOG)?;
    Ok(text.as_deref().and_then(catalog_in))
}

/// Where Eclipse lists its named hook points, at the workspace root (horizon-mixins reads it there).
const CATALOG: &str = "native-hooks.json";
pub(crate) type HookCatalog = std::collections::BTreeSet<(String, u32)>;

/// The revision to build `mods` on: `pinned` (the pins' agreement, else the installed version's)
/// when every mod naming a hook points' version finds it or a later one there (`at` reads a
/// revision's), and finds each named hook point it injects at in that revision's catalog
/// (`catalog`): hook points v2 came before `native-hooks.json`, so a pin on such a revision
/// would fail in horizon-mixins (E_HOOK_CATALOG). Else, when nothing pins it, the revision those mods were tested on. Else the
/// revision of the release or Nightly the build starts from (`game`), when they find theirs
/// there: a rebuild that stayed on an older Eclipse, or mods pinning one, then move to the game
/// the player installed, and pinned mods wait for the player's Build anyway ([`mismatch`]).
fn with_hooks(
    mods: &[Bundle],
    pinned: String,
    free: bool,
    game: impl FnOnce() -> Option<String>,
    at: impl Fn(&str) -> Result<Option<u32>, String>,
    catalog: impl Fn(&str) -> Result<Option<HookCatalog>, String>,
) -> Result<String, Undecided> {
    let hooked: Vec<&Bundle> = mods.iter().filter(|b| b.hooks.is_some()).collect();
    let named = hooked.iter().any(|b| b.named_hooks().next().is_some());
    let points = |revision: &str| {
        let unread = |e: String| {
            format!(
                "Can't read the native hook points of Eclipse {}: {e}",
                short(revision)
            )
        };
        let found = at(revision).map_err(unread)?;
        let listed = match named {
            true => catalog(revision).map_err(unread)?,
            false => None,
        };
        Ok::<_, String>((found, listed))
    };
    let fits_one = |b: &Bundle, (found, listed): &(Option<u32>, Option<HookCatalog>)| {
        found.is_some()
            && b.hooks <= *found
            && b.named_hooks().all(|(name, version)| {
                listed
                    .as_ref()
                    .is_some_and(|l| l.contains(&(name.to_owned(), version)))
            })
    };
    let fits = |revision: &str| {
        let found = points(revision)?;
        Ok::<_, String>(hooked.iter().all(|b| fits_one(b, &found)))
    };
    if hooked.is_empty() || fits(&pinned)? {
        return Ok(pinned);
    }
    let tested: std::collections::BTreeSet<&String> =
        hooked.iter().filter_map(|b| b.eclipse.as_ref()).collect();
    if let (true, [revision]) = (free, Vec::from_iter(tested).as_slice()) {
        if hnmod::revision(revision) && fits(revision)? {
            return Ok((*revision).clone());
        }
    }
    if let Some(revision) = game().filter(|r| *r != pinned) {
        if fits(&revision)? {
            return Ok(revision);
        }
    }
    let found = points(&pinned)?;
    let list: Vec<String> = hooked
        .iter()
        .filter(|b| !fits_one(b, &found))
        .map(|b| format!("{} (hook points v{})", b.id, b.hooks.unwrap_or_default()))
        .collect();
    Err(Undecided::Unfit(format!(
        "Eclipse {} doesn't have the native hook points these mods use: {}. Update Eclipse or the mods.",
        short(&pinned),
        list.join(", ")
    )))
}

pub(crate) fn short(revision: &str) -> &str {
    &revision[..revision.len().min(7)]
}

/// The Eclipse commit `version` (an installed release or Nightly) was built from, from GitHub.
pub fn revision_of(version: &str) -> Result<String, String> {
    let reference = crate::nightly::commit_of(version).unwrap_or(version);
    let commit = crate::github::api(&format!("repos/{}/commits/{reference}", backend::REPO))?;
    commit["sha"]
        .as_str()
        .filter(|s| hnmod::revision(s))
        .map(str::to_owned)
        .ok_or_else(|| format!("GitHub doesn't know the commit of {version}."))
}

/// The Eclipse commit the mods that pin one (`pins`: those naming no hook points' version) give a
/// build, and which of them pin it. With `on_base`, the player chose the commit of the version the
/// build starts from (`base`) whatever the pins say; with no pin, it is `base`'s too. Mods
/// pinning different commits give the newest of them ([`newest`]).
fn pinned_revision(
    pins: &[Bundle],
    on_base: bool,
    base: impl FnOnce() -> Result<String, String>,
    date: impl Fn(&str) -> Result<String, String>,
) -> Result<(String, Vec<String>), String> {
    if on_base {
        return Ok((base()?, Vec::new()));
    }
    let revision = newest(pins, Pinned::Eclipse, base, date)?;
    let by = pins
        .iter()
        .filter(|b| b.eclipse.as_ref() == Some(&revision))
        .map(|b| b.id.clone())
        .collect();
    Ok((revision, by))
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Undecided {
    Mismatch(String),
    Unfit(String),
    Failed(String),
}

impl From<String> for Undecided {
    fn from(error: String) -> Self {
        Self::Failed(error)
    }
}

impl std::fmt::Display for Undecided {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mismatch(text) | Self::Unfit(text) | Self::Failed(text) => f.write_str(text),
        }
    }
}

pub(crate) trait Lookup {
    fn commit_of(&self, version: &str) -> Result<String, String>;
    fn date(&self, field: Pinned, revision: &str) -> Result<String, String>;
    fn hooks(&self, revision: &str) -> Result<Option<u32>, String>;
    fn catalog(&self, revision: &str) -> Result<Option<HookCatalog>, String>;
}

pub(super) struct GitHub;

impl Lookup for GitHub {
    fn commit_of(&self, version: &str) -> Result<String, String> {
        revision_of(version)
    }

    fn date(&self, field: Pinned, revision: &str) -> Result<String, String> {
        committed(field.repo(), revision)
    }

    fn hooks(&self, revision: &str) -> Result<Option<u32>, String> {
        hooks_at(revision)
    }

    fn catalog(&self, revision: &str) -> Result<Option<HookCatalog>, String> {
        catalog_at(revision)
    }
}

pub(crate) struct Base<'a> {
    pub(crate) version: &'a str,
    pub(crate) built: Option<&'a About>,
    pub(crate) on_base: bool,
    pub(crate) anyway: bool,
}

impl Base<'_> {
    pub(crate) fn origin(&self) -> &str {
        self.built.map_or(self.version, |about| about.base.as_str())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Decided {
    pub(crate) revision: String,
    pub(crate) pinned_by: Vec<String>,
    pub(crate) base_eclipse: Option<String>,
    pub(crate) on_base: bool,
    pub(crate) mismatch: Option<String>,
    pub(crate) mixins: String,
}

pub(crate) fn decide(
    mods: &[Bundle],
    base: &Base,
    lookup: &impl Lookup,
) -> Result<Decided, Undecided> {
    let pins: Vec<Bundle> = mods.iter().filter(|b| b.pin().is_some()).cloned().collect();
    let free = pins.is_empty();
    let on_base = base.on_base || base.built.is_some_and(|a| a.on_base);
    // The commit of the release or Nightly the build starts from (`origin`'s, asked once). A
    // native build's own is that of the version it started from when it recorded it; else,
    // outside `on_base`, the commit it compiled, so a rebuild stays on the Eclipse it plays.
    let asked = std::cell::OnceCell::new();
    let of_origin = || {
        asked
            .get_or_init(|| lookup.commit_of(base.origin()))
            .clone()
    };
    let base_revision = || match base.built {
        Some(about) => match &about.base_eclipse {
            Some(r) => Ok(r.clone()),
            None if !on_base && !about.off_its_nightly() => Ok(about.eclipse.clone()),
            None => of_origin(),
        },
        None => of_origin(),
    };
    let date = |r: &str| lookup.date(Pinned::Eclipse, r);
    let (pinned, mut pinned_by) = pinned_revision(&pins, on_base, base_revision, date)?;
    let base_eclipse = match pinned_by.is_empty() {
        true => Some(pinned.clone()),
        false => base_revision().ok(),
    };
    let game = || of_origin().ok();
    let revision = with_hooks(
        mods,
        pinned.clone(),
        free,
        game,
        |r| lookup.hooks(r),
        |r| lookup.catalog(r),
    )?;
    // Nothing to say when the pin is the base's own commit, or the hook points moved the build.
    if base_eclipse.as_ref() == Some(&revision) || revision != pinned {
        pinned_by.clear();
    }
    let other = mismatch(mods, &revision);
    if let (Some(list), false) = (&other, base.anyway || on_base) {
        return Err(Undecided::Mismatch(list.clone()));
    }
    let mixins = newest(
        mods,
        Pinned::Mixins,
        || Ok(MIXINS_REV.into()),
        |r| lookup.date(Pinned::Mixins, r),
    )?;
    Ok(Decided {
        revision,
        pinned_by,
        base_eclipse,
        on_base,
        mismatch: other,
        mixins,
    })
}

/// The installed version a build for an instance that plays `current` starts from: `current`
/// itself, so a rebuild stays on the Eclipse it plays (a native build's own revision, [`about`]).
/// With `update`, or when it plays nothing installed, the newest installed release or Nightly.
/// An imported build is no commit to build on: its game files' version is.
pub fn base_for(current: Option<&str>, installed: &[String], update: bool) -> Option<String> {
    let imported = current.and_then(crate::build_import::about).map(|a| a.base);
    match imported.as_deref().or(current) {
        Some(v) if !update && installed.iter().any(|t| t == v) => Some(v.to_owned()),
        _ => backend::newest_release_or_nightly(installed).cloned(),
    }
}

/// The newer Eclipse a build with `update` would move a native build started from `base` to: the
/// newest installed release or Nightly, when it is newer than `base`.
pub fn newer_than(base: &str, installed: &[String]) -> Option<String> {
    backend::newest_release_or_nightly(installed)
        .filter(|n| *n != base && backend::compare_versions(n, base).is_gt())
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::super::manifest::{bundle, MANIFEST};
    use super::*;
    use crate::native_mods::tests::{dir, A, B};

    fn known<T>(read: impl Fn(&str) -> Option<T>) -> impl Fn(&str) -> Result<Option<T>, String> {
        move |revision| Ok(read(revision))
    }

    #[test]
    fn the_mods_of_one_build_pin_one_revision() {
        let pinned = |id: &str, eclipse: Option<&str>| Bundle {
            id: id.into(),
            version: "1".into(),
            eclipse: eclipse.map(str::to_owned),
            ..Default::default()
        };
        let never = || -> Result<String, String> { panic!("a pin wins") };
        let same = [
            pinned("a", Some(A)),
            pinned("b", None),
            pinned("c", Some(A)),
        ];
        assert_eq!(agreed(&same, Pinned::Eclipse, never).unwrap(), A);
        let none = [pinned("a", None)];
        assert_eq!(agreed(&none, Pinned::Eclipse, || Ok(B.into())).unwrap(), B);
        let split = [pinned("veinminer", Some(A)), pinned("minimap", Some(B))];
        let err = agreed(&split, Pinned::Eclipse, never).unwrap_err();
        assert!(
            err.contains("veinminer → aaaaaaa") && err.contains("minimap → bbbbbbb"),
            "{err}"
        );
        let bad = [pinned("a", Some("nightly"))];
        assert!(agreed(&bad, Pinned::Eclipse, never).is_err());
    }

    /// A player's report (11-04): Right-Click Crops 1.0.0 pins Nightly 2026-10-31's commit, so
    /// every build of their instance compiled that commit, whatever Nightly or profile they
    /// picked. The pin still chooses by default, and the build now says which mod chose it;
    /// Rebuild on Eclipse (`on_base`) compiles the picked version's own commit.
    #[test]
    fn a_pin_chooses_the_commit_unless_the_player_builds_on_the_picked_version() {
        let pinned = |id: &str, eclipse: Option<&str>| Bundle {
            id: id.into(),
            version: "1".into(),
            eclipse: eclipse.map(str::to_owned),
            ..Default::default()
        };
        // A: the pin (an older Nightly); B: the Nightly the player picked.
        let picked = || -> Result<String, String> { Ok(B.into()) };
        let no_date = |_: &str| -> Result<String, String> { panic!("one pin: no date needed") };
        let mods = [pinned("right_click_crops", Some(A)), pinned("zoom", None)];
        assert_eq!(
            pinned_revision(&mods, false, picked, no_date).unwrap(),
            (A.to_owned(), vec!["right_click_crops".to_owned()]),
            "the pin wins over the picked Nightly, and the build knows whose it is"
        );
        assert_eq!(
            pinned_revision(&mods, true, picked, no_date).unwrap(),
            (B.to_owned(), Vec::new()),
            "Rebuild on Eclipse builds on the picked Nightly"
        );
        assert_eq!(
            pinned_revision(&mods[1..], false, picked, no_date).unwrap(),
            (B.to_owned(), Vec::new()),
            "no pin: the picked Nightly"
        );
        let offline = || -> Result<String, String> { Err("offline".into()) };
        assert!(pinned_revision(&mods, true, offline, no_date).is_err());
    }

    #[test]
    fn mods_made_for_different_revisions_build_on_the_newest() {
        let pinned = |id: &str, eclipse: Option<&str>, mixins: Option<&str>| Bundle {
            id: id.into(),
            version: "1".into(),
            eclipse: eclipse.map(str::to_owned),
            mixins: mixins.map(str::to_owned),
            ..Default::default()
        };
        let c = "c".repeat(40);
        // a is the oldest commit, then b, then c.
        let date = |r: &str| -> Result<String, String> {
            Ok(match &r[..1] {
                "a" => "2026-10-28T00:00:00Z",
                "b" => "2026-10-29T00:00:00Z",
                _ => "2026-10-30T00:00:00Z",
            }
            .into())
        };
        let never = || -> Result<String, String> { panic!("a pin wins") };
        let split = [
            pinned("veinminer", Some(B), Some(&c)),
            pinned("lookat", Some(A), Some(A)),
            pinned("zoom", None, None),
        ];
        assert_eq!(newest(&split, Pinned::Eclipse, never, date).unwrap(), B);
        // horizon-mixins: never older than a pin, and the launcher's own when it is newer.
        assert_eq!(newest(&split, Pinned::Mixins, never, date).unwrap(), c);
        let older = [pinned("a", None, Some(A)), pinned("b", None, Some(B))];
        assert_eq!(
            newest(&older, Pinned::Mixins, never, date).unwrap(),
            MIXINS_REV
        );
        // One pin, or the same one everywhere: that pin, and no date is asked for.
        let no_date = |_: &str| -> Result<String, String> { panic!("no date needed") };
        let same = [pinned("a", Some(A), None), pinned("b", Some(A), None)];
        assert_eq!(newest(&same, Pinned::Eclipse, never, no_date).unwrap(), A);
        let offline = |_: &str| -> Result<String, String> { Err("offline".into()) };
        let err = newest(&split, Pinned::Eclipse, never, offline).unwrap_err();
        assert!(err.contains("offline"), "{err}");
        // What the player is shown: each mod made for another revision, and the build's.
        assert_eq!(
            mismatch(&split, B).unwrap(),
            "lookat → aaaaaaa; the build uses bbbbbbb"
        );
        assert_eq!(mismatch(&same, A), None);
        let hooked = [Bundle {
            hooks: Some(1),
            ..pinned("minimap", Some(A), None)
        }];
        assert_eq!(mismatch(&hooked, B), None, "hook mods follow the build");
    }

    #[test]
    fn a_commit_date_is_asked_once_and_kept() {
        let d = dir("commit-dates");
        let file = d.join("commit-dates.json");
        let ask = |_: &str, r: &str| -> Result<String, String> { Ok(format!("date of {r}")) };
        assert_eq!(
            remembered(&file, "acme/x", A, ask).unwrap(),
            format!("date of {A}")
        );
        let offline = |_: &str, _: &str| -> Result<String, String> { Err("offline".into()) };
        assert_eq!(
            remembered(&file, "acme/x", A, offline).unwrap(),
            format!("date of {A}")
        );
        // Another repo or revision isn't known yet.
        assert!(remembered(&file, "acme/y", A, offline).is_err());
        assert!(remembered(&file, "acme/x", B, offline).is_err());
        // A broken file is asked again and rewritten.
        fs::write(&file, "not json").unwrap();
        assert_eq!(
            remembered(&file, "acme/x", B, ask).unwrap(),
            format!("date of {B}")
        );
        assert_eq!(
            remembered(&file, "acme/x", B, offline).unwrap(),
            format!("date of {B}")
        );
    }

    #[test]
    fn mods_on_the_hook_points_follow_the_game_or_the_pins() {
        assert_eq!(
            hooks_in("x\npub(crate) const NATIVE_HOOKS_API: u32 = 3;\n"),
            Some(3)
        );
        assert_eq!(hooks_in("const NATIVE_HOOKS_API: u32 = 3;"), None);
        let mod_ = |id: &str, eclipse: &str, hooks: Option<u32>| Bundle {
            id: id.into(),
            version: "1".into(),
            eclipse: Some(eclipse.into()),
            hooks,
            ..Default::default()
        };
        let c = "c".repeat(40);
        // A (the game) has hook points v1, B v2, C none.
        let at = |r: &str| match r.as_bytes()[0] {
            b'a' => Some(1),
            b'b' => Some(2),
            _ => None,
        };
        let none = || None;
        let listed = |_: &str| None;
        let zoom = [mod_("zoom", B, Some(1)), mod_("minimap", &c, Some(1))];
        assert_eq!(
            with_hooks(&zoom, A.into(), true, none, known(at), known(listed)).unwrap(),
            A,
            "the game's own"
        );
        let tested = [mod_("zoom", A, Some(1))];
        assert_eq!(
            with_hooks(&tested, c.clone(), true, none, known(at), known(listed)).unwrap(),
            A,
            "an older game"
        );
        assert_eq!(
            with_hooks(&tested, B.into(), false, none, known(at), known(listed)).unwrap(),
            B,
            "v1 mods on v2"
        );
        let newer = [mod_("recipes", B, Some(2))];
        let err = with_hooks(&newer, A.into(), false, none, known(at), known(listed))
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("aaaaaaa") && err.contains("recipes (hook points v2)"),
            "{err}"
        );
        assert_eq!(
            with_hooks(&newer, A.into(), true, none, known(at), known(listed)).unwrap(),
            B,
            "its tested commit"
        );
        let pinned_only = [mod_("veinminer", B, None)];
        assert_eq!(
            with_hooks(
                &pinned_only,
                B.into(),
                false,
                none,
                known(at),
                known(listed)
            )
            .unwrap(),
            B
        );
        // Offline is not "no hook points": the build says it couldn't read them.
        let offline = |_: &str| -> Result<Option<u32>, String> { Err("No connection".into()) };
        let err = with_hooks(&zoom, A.into(), true, none, offline, known(listed))
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("Can't read the native hook points of Eclipse aaaaaaa")
                && err.contains("No connection")
                && !err.contains("doesn't have"),
            "{err}"
        );
    }

    /// Players on launcher 0.2.31 (#5352): a rebuild that stayed on an older Eclipse (A, hook
    /// points v1), or a mod pinning it, met mods needing newer hook points, tested on different
    /// commits, and the build was refused although the installed game (B) has them all.
    #[test]
    fn mods_needing_newer_hook_points_move_the_build_to_the_installed_game() {
        let mod_ = |id: &str, eclipse: &str, hooks: Option<u32>| Bundle {
            id: id.into(),
            version: "1".into(),
            eclipse: Some(eclipse.into()),
            hooks,
            ..Default::default()
        };
        let (c, d) = ("c".repeat(40), "d".repeat(40));
        // A (the old build's revision) has hook points v1, B (the installed game) v7.
        let at = |r: &str| match r.as_bytes()[0] {
            b'a' => Some(1),
            b'b' => Some(7),
            _ => None,
        };
        let game = || Some(B.to_owned());
        let listed = |_: &str| None;
        let hooked = [
            mod_("zoom", &c, Some(1)),
            mod_("recipes", &d, Some(2)),
            mod_("xtoxray", B, Some(7)),
        ];
        assert_eq!(
            with_hooks(&hooked, A.into(), true, game, known(at), known(listed)).unwrap(),
            B,
            "a rebuild that stayed on an older Eclipse moves to the game"
        );
        let with_pin = [mod_("carryblock", A, None), mod_("recipes", &d, Some(2))];
        let revision =
            with_hooks(&with_pin, A.into(), false, game, known(at), known(listed)).unwrap();
        assert_eq!(revision, B, "a pinned mod no longer blocks the build");
        let asked = mismatch(&with_pin, &revision).expect("the pinned mod waits for Build anyway");
        assert!(
            asked.contains("carryblock") && !asked.contains("recipes"),
            "{asked}"
        );
        // A game without them still refuses, naming only the mods that don't fit.
        let err = with_hooks(
            &hooked,
            A.into(),
            true,
            || Some(c.clone()),
            known(at),
            known(listed),
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("recipes (hook points v2)"), "{err}");
        assert!(err.contains("xtoxray (hook points v7)"), "{err}");
        assert!(!err.contains("zoom"), "zoom builds on A: {err}");
    }

    /// likeluca_ on Discord (11-04, Windows, launchers 0.2.38 to 0.2.40): blockscope (hook points
    /// v2, all named) beside shulker_peek, which pins 5e391eb, a commit that had hook points v2 but
    /// no `native-hooks.json` yet (it came in d3e0774, still v2). The build stayed on the pin and
    /// horizon-mixins stopped: "E_HOOK_CATALOG: a named hook needs native-hooks.json at the
    /// workspace root". The build now moves to the game, whose catalog lists them.
    #[test]
    fn named_hook_points_need_the_revisions_catalog() {
        let d = dir("named-hooks");
        fs::write(
            d.join(MANIFEST),
            "[mod]\nid = \"blockscope\"\nversion = \"0.3.1\"\n\
             [[injection]]\nid = \"hud\"\nkind = \"HEAD\"\nhook = \"hud\"\nhook_version = 1\n\
             [[injection]]\nid = \"raw\"\nkind = \"HEAD\"\ntarget = \"eclipse_client::game::tick\"\n",
        )
        .unwrap();
        let c = "c".repeat(40);
        let blockscope = Bundle {
            eclipse: Some(c.clone()),
            hooks: Some(2),
            ..bundle(&d, None).unwrap()
        };
        let shulker_peek = Bundle {
            id: "shulker_peek".into(),
            version: "0.1.0".into(),
            eclipse: Some(A.into()),
            mixins_content: true,
            ..Default::default()
        };
        // A (the pin) has hook points v2 and no catalog; B (the game) v359 and one.
        let at = |r: &str| match r.as_bytes()[0] {
            b'a' => Some(2),
            b'b' => Some(359),
            _ => None,
        };
        let catalog = |text: &str| catalog_in(text);
        let game_catalog = r#"{"hooks": [{"name": "hud", "version": 1, "target": "x"}]}"#;
        let listed = |r: &str| match r.as_bytes()[0] {
            b'b' => catalog(game_catalog),
            _ => None,
        };
        let mods = [shulker_peek.clone(), blockscope.clone()];
        let revision = with_hooks(
            &mods,
            A.into(),
            false,
            || Some(B.into()),
            known(at),
            known(listed),
        )
        .unwrap();
        assert_eq!(
            revision, B,
            "the pin has no catalog: the game's revision has one"
        );
        let asked = mismatch(&mods, &revision).expect("the pinned mod waits for Build anyway");
        assert!(asked.contains("shulker_peek"), "{asked}");
        // A game without the catalog either refuses, naming the mod that needs it.
        let err = with_hooks(
            &mods,
            A.into(),
            false,
            || Some(c.clone()),
            known(at),
            known(listed),
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("blockscope (hook points v2)"), "{err}");
        // A catalog with only another version of the hook point doesn't do either.
        let other =
            |_: &str| catalog(r#"{"hooks": [{"name": "hud", "version": 2, "target": "x"}]}"#);
        assert!(with_hooks(
            &mods,
            A.into(),
            false,
            || Some(B.into()),
            known(at),
            known(other)
        )
        .is_err());
        // A mod with only raw targets still builds on the pin, without asking for a catalog.
        let raw = Bundle {
            injections: blockscope.injections[1..].to_vec(),
            ..blockscope
        };
        let asks = |_: &str| -> Option<HookCatalog> { panic!("no named hook point to look up") };
        assert_eq!(
            with_hooks(
                &[shulker_peek, raw],
                A.into(),
                false,
                || None,
                known(at),
                known(asks)
            )
            .unwrap(),
            A
        );
        assert_eq!(catalog_in("not json"), None);
    }

    /// levzzz5154's decision (2026-10-29): a rebuild stays on the Eclipse the instance plays;
    /// only "Rebuild on Eclipse {version}" moves it to a newer one.
    const C: &str = "cccccccccccccccccccccccccccccccccccccccc";
    const D: &str = "dddddddddddddddddddddddddddddddddddddddd";

    #[derive(Default)]
    struct Answers {
        commits: Vec<(&'static str, &'static str)>,
        dates: Vec<(&'static str, &'static str)>,
        hooks: Vec<(&'static str, u32)>,
        asked: std::cell::RefCell<Vec<String>>,
    }

    impl Lookup for Answers {
        fn commit_of(&self, version: &str) -> Result<String, String> {
            self.asked.borrow_mut().push(format!("commit of {version}"));
            let found = self.commits.iter().find(|(v, _)| *v == version);
            found
                .map(|(_, commit)| commit.to_string())
                .ok_or_else(|| "offline".to_string())
        }

        fn date(&self, _: Pinned, revision: &str) -> Result<String, String> {
            self.asked
                .borrow_mut()
                .push(format!("date of {}", short(revision)));
            let found = self.dates.iter().find(|(r, _)| *r == revision);
            found
                .map(|(_, date)| date.to_string())
                .ok_or_else(|| "offline".to_string())
        }

        fn hooks(&self, revision: &str) -> Result<Option<u32>, String> {
            let found = self.hooks.iter().find(|(r, _)| *r == revision);
            Ok(found.map(|(_, hooks)| *hooks))
        }

        fn catalog(&self, _: &str) -> Result<Option<HookCatalog>, String> {
            Ok(None)
        }
    }

    fn mod_(id: &str, eclipse: Option<&str>, hooks: Option<u32>) -> Bundle {
        Bundle {
            id: id.into(),
            version: "1".into(),
            eclipse: eclipse.map(str::to_owned),
            hooks,
            ..Default::default()
        }
    }

    fn from(version: &str) -> Base<'_> {
        Base {
            version,
            built: None,
            on_base: false,
            anyway: false,
        }
    }

    #[test]
    fn a_build_without_pins_compiles_the_commit_of_the_version_it_starts_from() {
        let github = Answers {
            commits: vec![("v2026.10.20", A)],
            ..Default::default()
        };
        let mods = [mod_("zoom", None, None)];
        assert_eq!(
            decide(&mods, &from("v2026.10.20"), &github).unwrap(),
            Decided {
                revision: A.into(),
                pinned_by: Vec::new(),
                base_eclipse: Some(A.into()),
                on_base: false,
                mismatch: None,
                mixins: MIXINS_REV.into(),
            }
        );
        assert_eq!(
            decide(&mods, &from("v2026.10.20"), &Answers::default()),
            Err(Undecided::Failed("offline".into()))
        );
    }

    #[test]
    fn a_pin_moves_the_build_to_its_commit_and_names_the_mod() {
        let nightly = "nightly-2026-11-04-bbbbbbb";
        let github = Answers {
            commits: vec![(nightly, B)],
            ..Default::default()
        };
        let mods = [
            mod_("right_click_crops", Some(A), None),
            mod_("zoom", None, None),
        ];
        let decided = decide(&mods, &from(nightly), &github).unwrap();
        assert_eq!(decided.revision, A);
        assert_eq!(decided.pinned_by, ["right_click_crops"]);
        assert_eq!(decided.base_eclipse.as_deref(), Some(B));
        let own = Answers {
            commits: vec![(nightly, A)],
            ..Default::default()
        };
        let decided = decide(&mods, &from(nightly), &own).unwrap();
        assert_eq!(decided.revision, A);
        assert!(
            decided.pinned_by.is_empty(),
            "the Nightly's own commit is nobody's pin"
        );
    }

    #[test]
    fn rebuild_on_eclipse_compiles_the_base_commit_whatever_the_mods_pin() {
        let github = Answers {
            commits: vec![("v2026.10.20", B)],
            ..Default::default()
        };
        let base = Base {
            on_base: true,
            ..from("v2026.10.20")
        };
        let decided = decide(&[mod_("crops", Some(A), None)], &base, &github).unwrap();
        assert_eq!(decided.revision, B);
        assert!(decided.on_base && decided.pinned_by.is_empty());
        assert_eq!(
            decided.mismatch.as_deref(),
            Some("crops → aaaaaaa; the build uses bbbbbbb")
        );
    }

    #[test]
    fn mods_pinning_different_commits_ask_before_building_on_the_newest() {
        let github = Answers {
            commits: vec![("v2026.10.20", A)],
            dates: vec![(A, "2026-10-28T00:00:00Z"), (B, "2026-10-29T00:00:00Z")],
            ..Default::default()
        };
        let mods = [
            mod_("veinminer", Some(B), None),
            mod_("lookat", Some(A), None),
        ];
        assert_eq!(
            decide(&mods, &from("v2026.10.20"), &github),
            Err(Undecided::Mismatch(
                "lookat → aaaaaaa; the build uses bbbbbbb".into()
            ))
        );
        let anyway = Base {
            anyway: true,
            ..from("v2026.10.20")
        };
        let decided = decide(&mods, &anyway, &github).unwrap();
        assert_eq!(decided.revision, B);
        assert_eq!(decided.pinned_by, ["veinminer"]);
        assert_eq!(decided.base_eclipse.as_deref(), Some(A));
        assert_eq!(
            decided.mismatch.as_deref(),
            Some("lookat → aaaaaaa; the build uses bbbbbbb")
        );
    }

    #[test]
    fn a_native_rebuild_stays_on_the_commit_it_compiled() {
        let about = About {
            base: "v2026.10.20".into(),
            eclipse: C.into(),
            ..Default::default()
        };
        let base = Base {
            built: Some(&about),
            ..from("native-ccccccc-0123456789ab")
        };
        let github = Answers::default();
        let decided = decide(&[mod_("zoom", None, None)], &base, &github).unwrap();
        assert_eq!(decided.revision, C);
        assert!(github.asked.borrow().is_empty(), "{:?}", github.asked);
    }

    #[test]
    fn a_rebuild_without_the_pinning_mod_goes_back_to_its_versions_own_commit() {
        let about = About {
            base: "v2026.10.20".into(),
            eclipse: A.into(),
            base_eclipse: Some(B.into()),
            pinned_by: vec!["crops".into()],
            ..Default::default()
        };
        let base = Base {
            built: Some(&about),
            ..from("native-aaaaaaa-0123456789ab")
        };
        let github = Answers::default();
        let unpinned = decide(&[mod_("zoom", None, None)], &base, &github).unwrap();
        assert_eq!(unpinned.revision, B);
        let pinned = decide(&[mod_("crops", Some(A), None)], &base, &github).unwrap();
        assert_eq!(pinned.revision, A);
        assert_eq!(pinned.pinned_by, ["crops"]);
        assert_eq!(pinned.base_eclipse.as_deref(), Some(B));
    }

    #[test]
    fn an_old_record_off_its_nightly_starts_from_the_nightlys_commit() {
        let nightly = "nightly-2026-10-29-4baa571";
        let about = About {
            base: nightly.into(),
            eclipse: A.into(),
            ..Default::default()
        };
        assert!(about.off_its_nightly());
        let base = Base {
            built: Some(&about),
            ..from("native-aaaaaaa-0123456789ab")
        };
        let github = Answers {
            commits: vec![(nightly, B)],
            ..Default::default()
        };
        let decided = decide(&[mod_("zoom", None, None)], &base, &github).unwrap();
        assert_eq!(decided.revision, B);
        assert_eq!(*github.asked.borrow(), [format!("commit of {nightly}")]);
    }

    #[test]
    fn hook_point_mods_move_a_rebuild_to_the_game_it_started_from() {
        let about = About {
            base: "v2026.11.01".into(),
            eclipse: A.into(),
            ..Default::default()
        };
        let base = Base {
            built: Some(&about),
            ..from("native-aaaaaaa-0123456789ab")
        };
        let github = Answers {
            commits: vec![("v2026.11.01", B)],
            hooks: vec![(A, 1), (B, 7)],
            ..Default::default()
        };
        let mods = [
            mod_("zoom", Some(C), Some(1)),
            mod_("recipes", Some(D), Some(2)),
        ];
        let decided = decide(&mods, &base, &github).unwrap();
        assert_eq!(decided.revision, B);
        assert_eq!(decided.mismatch, None, "hook point mods pin nothing");
        let old = Answers {
            hooks: vec![(A, 1)],
            ..github
        };
        assert!(matches!(
            decide(&mods, &base, &old),
            Err(Undecided::Unfit(e)) if e.contains("recipes (hook points v2)") && !e.contains("zoom")
        ));
    }

    #[test]
    fn horizon_mixins_is_the_newest_a_mod_names() {
        let github = Answers {
            commits: vec![("v2026.10.20", A)],
            dates: vec![
                (B, "2026-10-29T00:00:00Z"),
                (C, "2026-10-30T00:00:00Z"),
                (MIXINS_REV, "2026-10-28T00:00:00Z"),
            ],
            ..Default::default()
        };
        let naming = |id: &str, mixins: &str| Bundle {
            mixins: Some(mixins.into()),
            ..mod_(id, None, None)
        };
        let base = from("v2026.10.20");
        let decided = decide(&[naming("a", B), naming("b", C)], &base, &github).unwrap();
        assert_eq!(decided.mixins, C);
        let decided = decide(&[naming("a", B)], &base, &github).unwrap();
        assert_eq!(decided.mixins, B, "one mod's is taken as it is");
        let decided = decide(&[mod_("a", None, None)], &base, &github).unwrap();
        assert_eq!(decided.mixins, MIXINS_REV);
    }

    #[test]
    fn a_rebuild_stays_on_the_instances_eclipse_until_an_update() {
        let installed: Vec<String> = [
            "nightly-2026-10-29-4baa571",
            "v2026.10.20",
            "native-83f15fa-0123456789ab",
            "v2026.10.13",
        ]
        .map(String::from)
        .to_vec();
        let base = |current: Option<&str>, update| base_for(current, &installed, update);
        let native = "native-83f15fa-0123456789ab";
        assert_eq!(base(Some(native), false).as_deref(), Some(native));
        assert_eq!(
            base(Some("v2026.10.13"), false).as_deref(),
            Some("v2026.10.13")
        );
        let newest = Some("nightly-2026-10-29-4baa571");
        assert_eq!(base(Some(native), true).as_deref(), newest);
        assert_eq!(base(None, false).as_deref(), newest);
        assert_eq!(base(Some("native-gone"), false).as_deref(), newest);
        assert_eq!(base_for(Some(native), &[native.into()], true), None);
        // Update offers the newest release or Nightly, when newer than the build's own.
        assert_eq!(newer_than("v2026.10.20", &installed).as_deref(), newest);
        assert_eq!(newer_than("nightly-2026-10-29-4baa571", &installed), None);
        let releases = ["v2026.10.20".to_string(), "v2026.10.13".to_string()];
        assert_eq!(newer_than("v2026.10.20", &releases), None);
        assert_eq!(
            newer_than("v2026.10.13", &releases).as_deref(),
            Some("v2026.10.20")
        );
    }
}
