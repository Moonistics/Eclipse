use super::mixins::DEFAULT_PRIORITY;
use super::{api, dependencies};
use eclipse_mods::index::native as hnmod;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) const MANIFEST: &str = "eclipse-mixin.toml";
/// A registry download's manifest (`.hnmod`, `eclipse_mods::index::native`), beside `mixins/<id>/`.
pub(super) const NATIVE_MANIFEST: &str = hnmod::MANIFEST;

/// A mixin bundle: a folder with a `eclipse-mixin.toml`, and the revisions it pins.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bundle {
    pub id: String,
    pub version: String,
    pub dir: PathBuf,
    /// The Eclipse commit it builds on (`[compatibility] eclipse_revision`, or its `.hnmod`'s).
    pub eclipse: Option<String>,
    /// The horizon-mixins commit it was tested with (its `.hnmod`'s).
    pub mixins: Option<String>,
    /// The native hook points' version it needs (its `.hnmod`'s `hooks`): then `eclipse` is the
    /// revision it was tested on, and it builds on any with these hook points or later ones.
    pub hooks: Option<u32>,
    /// The bundle-relative trait package folder (`[api] path`).
    pub api: Option<String>,
    /// True when the manifest has content for horizon-mixins.
    pub mixins_content: bool,
    /// Its `[[injection]]` entries, for the build's warnings. Read leniently:
    /// an entry the launcher cannot make sense of is left out, horizon-mixins
    /// refuses it when it applies.
    pub injections: Vec<Injection>,
    pub(super) dependencies: Vec<dependencies::Dependency>,
    pub(super) modules: Vec<(String, String)>,
    pub(super) manifest: toml::Table,
}

/// One `[[injection]]` of a bundle's manifest, read for the build's
/// warnings: where it sits and whether it can cancel.
#[derive(Clone, Debug, PartialEq)]
pub struct Injection {
    pub id: String,
    /// Where it injects: a native hook point by name, or a raw function path.
    pub site: InjectionSite,
    /// A named hook point's `hook_version`: with the name, what the revision's
    /// `native-hooks.json` must list for horizon-mixins to find it.
    pub hook_version: Option<u32>,
    /// Its `kind` (`HEAD`, `RETURN`, …) as written.
    pub kind: Option<String>,
    /// Its `selector` and `ordinal`, which with the kind tell two interior
    /// sites of one function apart.
    pub selector: Option<String>,
    pub ordinal: Option<i64>,
    /// The `[mod]` `priority` unless the entry overrides it. horizon-mixins
    /// defaults it to 0 (`#[serde(default)] priority: i32`).
    pub priority: i64,
    /// `cancellable = true` (a `HEAD`, `BEFORE_CALL`, `AFTER_CALL` or
    /// `AFTER_ASSIGN` may carry it): a `Break` skips the later injections
    /// at the site.
    pub cancellable: bool,
}

/// An injection's site: `hook = "frame"` names a native hook point from the
/// catalog, `target = "game::tick"` a raw function path.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum InjectionSite {
    Hook(String),
    Target(String),
}

impl InjectionSite {
    pub(super) fn as_text(&self) -> String {
        match self {
            Self::Hook(name) => format!("hook `{name}`"),
            Self::Target(path) => format!("`{path}`"),
        }
    }
}

/// The Eclipse commit a mod pins: the one it names (`eclipse`), unless it names a hook points'
/// version (`hooks`) instead: then it follows the others, else the game, and pins nothing.
pub(crate) fn pin(hooks: Option<u32>, eclipse: Option<&str>) -> Option<&str> {
    eclipse.filter(|_| hooks.is_none())
}

impl Bundle {
    pub(crate) fn listed(id: &str, hooks: Option<u32>, eclipse: &str) -> Bundle {
        Bundle {
            id: id.into(),
            eclipse: Some(eclipse.into()),
            hooks,
            ..Default::default()
        }
    }

    pub(super) fn pin(&self) -> Option<&str> {
        pin(self.hooks, self.eclipse.as_deref())
    }

    /// The named hook points it injects at, with their versions: each must be in the
    /// `native-hooks.json` of the revision it builds on.
    pub(super) fn named_hooks(&self) -> impl Iterator<Item = (&str, u32)> {
        self.injections
            .iter()
            .filter_map(|i| match (&i.site, i.hook_version) {
                (InjectionSite::Hook(name), Some(version)) => Some((name.as_str(), version)),
                _ => None,
            })
    }
}

impl Injection {
    /// The place it sits at, as the manifest tells it: the function, the
    /// kind, and the selector and ordinal of interior hooks. horizon-mixins
    /// resolves the same place against the source (`plan.rs` sorts by file,
    /// node, call site, constant site, then priority descending, mod id
    /// ascending, injection id ascending): same manifest place, and its
    /// tie-break tail is the run order.
    pub(super) fn place(&self) -> (&InjectionSite, Option<&str>, Option<&str>, Option<i64>) {
        (
            &self.site,
            self.kind.as_deref(),
            self.selector.as_deref(),
            self.ordinal,
        )
    }
    /// The place as a message names it.
    pub(super) fn place_as_text(&self) -> String {
        let mut text = self.site.as_text();
        if let Some(kind) = &self.kind {
            text.push(' ');
            text.push_str(kind);
        }
        if let Some(selector) = &self.selector {
            text.push_str(&format!(" selector `{selector}`"));
        }
        if let Some(ordinal) = self.ordinal {
            text.push_str(&format!(" ordinal {ordinal}"));
        }
        text
    }
}

/// The bundles in `folder`: the folder itself when it is one (a `eclipse-mixin.toml`, or an
/// unpacked `.hnmod`: `eclipse-native.json` and `mixins/<id>/`), else each of its subfolders that is.
pub fn bundles(folder: &Path) -> Result<Vec<Bundle>, String> {
    if let Some(found) = bundle_in(folder)? {
        return Ok(vec![found]);
    }
    let mut dirs: Vec<PathBuf> = fs::read_dir(folder)
        .map_err(|e| format!("Could not read {}: {e}", folder.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    let mut found = Vec::new();
    for dir in dirs {
        found.extend(bundle_in(&dir)?);
    }
    if found.is_empty() {
        return Err(format!(
            "{} holds no native mod ({MANIFEST} or {NATIVE_MANIFEST}).",
            folder.display()
        ));
    }
    Ok(found)
}

pub(crate) fn bundle_in(dir: &Path) -> Result<Option<Bundle>, String> {
    if dir.join(MANIFEST).is_file() {
        return bundle(dir, None).map(Some);
    }
    let path = dir.join(NATIVE_MANIFEST);
    if !path.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let native: hnmod::Manifest =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let id = native.id;
    if !valid_id(&id) {
        return Err(format!("{}: invalid mod id {id:?}", path.display()));
    }
    let found = bundle(&dir.join("mixins").join(&id), Some(native.mixins))?;
    match &found.eclipse {
        Some(pinned) if *pinned != native.eclipse => Err(format!(
            "{id}: {NATIVE_MANIFEST} pins Eclipse {}, its {MANIFEST} {pinned}.",
            native.eclipse
        )),
        _ => Ok(Some(Bundle {
            eclipse: found.eclipse.clone().or(Some(native.eclipse)),
            hooks: native.hooks,
            ..found
        })),
    }
}

/// A native mod's id, a folder name in every staged workspace: what horizon-mixins takes.
pub(super) fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 100
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

pub(super) fn bundle(dir: &Path, mixins: Option<String>) -> Result<Bundle, String> {
    let path = dir.join(MANIFEST);
    let value = api::read(&path)?;
    let field = |table: &str, name: &str| {
        value
            .get(table)
            .and_then(|m| m.get(name))
            .and_then(|v| v.as_str())
            .map(str::to_owned)
    };
    let need = |name: &str| {
        field("mod", name).ok_or_else(|| format!("{} has no [mod] {name}.", path.display()))
    };
    let mod_priority = value
        .get("mod")
        .and_then(|m| m.get("priority"))
        .and_then(|v| v.as_integer())
        .unwrap_or(DEFAULT_PRIORITY);
    // Read leniently: an entry without an id and a site (`hook` or `target`)
    // is left out — horizon-mixins refuses it when it applies.
    let injections = value
        .get("injection")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let text = |name: &str| entry.get(name).and_then(|v| v.as_str()).map(str::to_owned);
            let id = text("id")?;
            let site = match (text("hook"), text("target")) {
                (Some(hook), _) => InjectionSite::Hook(hook),
                (None, Some(target)) => InjectionSite::Target(target),
                (None, None) => return None,
            };
            Some(Injection {
                id,
                site,
                hook_version: entry
                    .get("hook_version")
                    .and_then(|v| v.as_integer())
                    .and_then(|v| u32::try_from(v).ok()),
                kind: text("kind"),
                selector: text("selector"),
                ordinal: entry.get("ordinal").and_then(|v| v.as_integer()),
                priority: entry
                    .get("priority")
                    .and_then(|v| v.as_integer())
                    .unwrap_or(mod_priority),
                cancellable: entry.get("cancellable").and_then(|v| v.as_bool()) == Some(true),
            })
        })
        .collect();
    let api = value
        .get("api")
        .map(|table| api::folder(dir, table))
        .transpose()?;
    let mixins_content = value.keys().any(|key| {
        !["mod", "compatibility"].contains(&key.as_str())
            && !api::LAUNCHER_ONLY.contains(&key.as_str())
    });
    // Reject links before copying the bundle into the incoming folder.
    files_with_api(dir, api.as_deref())?;
    let id = need("id")?;
    if !valid_id(&id) {
        return Err(format!("{}: invalid mod id {id:?}", path.display()));
    }
    let dependencies =
        dependencies::parse(&id, value.get("dependencies")).map_err(|e| e.to_string())?;
    let modules = value
        .get("module")
        .and_then(|m| m.as_array())
        .into_iter()
        .flatten()
        .filter_map(|m| {
            Some((
                m.get("file")?.as_str()?.into(),
                m.get("name")?.as_str()?.into(),
            ))
        })
        .collect();
    let version = need("version")?;
    let eclipse = field("compatibility", "eclipse_revision");
    Ok(Bundle {
        id,
        version,
        dir: dir.to_owned(),
        eclipse,
        mixins,
        hooks: None,
        injections,
        api,
        mixins_content,
        dependencies,
        modules,
        manifest: value,
    })
}

/// A bundle that names a hook points' version builds on another revision than the one it was
/// tested on: its `eclipse_revision` (which horizon-mixins checks) becomes the build's.
pub(super) fn repin(bundle: &mut Bundle, revision: &str) -> Result<(), String> {
    if let Some(toml::Value::Table(compat)) = bundle.manifest.get_mut("compatibility") {
        compat.insert("eclipse_revision".into(), revision.into());
    }
    api::write(&bundle.dir.join(MANIFEST), &bundle.manifest)
}

/// Every file under `dir`, by its path relative to `dir` (with `/`), sorted. A symlink is refused:
/// horizon-mixins refuses them too.
pub(super) fn files(dir: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    files_with_api(dir, None)
}

/// Bundle files omit only the API package's Cargo build output.
pub(super) fn bundle_files(bundle: &Bundle) -> Result<Vec<(String, PathBuf)>, String> {
    files_with_api(&bundle.dir, bundle.api.as_deref())
}

/// The API package's target folder in a canonical bundle root.
pub(super) fn api_target(root: &Path, api: Option<&str>) -> Result<Option<PathBuf>, String> {
    api.map(|path| root.join(path).canonicalize().map(|p| p.join("target")))
        .transpose()
        .map_err(|e| e.to_string())
}

fn files_with_api(dir: &Path, api: Option<&str>) -> Result<Vec<(String, PathBuf)>, String> {
    fn walk(
        root: &Path,
        dir: &Path,
        skip: Option<&Path>,
        out: &mut Vec<(String, PathBuf)>,
    ) -> Result<(), String> {
        for entry in fs::read_dir(dir).map_err(|e| e.to_string())?.flatten() {
            let (path, kind) = (entry.path(), entry.file_type().map_err(|e| e.to_string())?);
            if kind.is_symlink() {
                return Err(format!(
                    "{} is a link: copy the file instead.",
                    path.display()
                ));
            } else if kind.is_dir() {
                if Some(path.as_path()) != skip {
                    walk(root, &path, skip, out)?;
                }
            } else {
                let relative = path.strip_prefix(root).unwrap_or(&path);
                out.push((relative.to_string_lossy().replace('\\', "/"), path));
            }
        }
        Ok(())
    }
    let root = dir
        .canonicalize()
        .map_err(|e| format!("{}: {e}", dir.display()))?;
    let skip = api_target(&root, api)?;
    let mut out = Vec::new();
    walk(&root, &root, skip.as_deref(), &mut out)?;
    out.sort();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::super::conflicts::warn_conflicts;
    use super::*;
    use crate::native_mods::tests::{bundle_of, dir, write_bundle, write_native, A, B};

    #[test]
    fn a_manifest_without_injections_builds_an_empty_list() {
        let mods = [bundle_of("alpha", "")];
        assert!(mods[0].injections.is_empty());
        assert!(warn_conflicts(&mods).is_empty());
    }

    #[test]
    fn an_entry_without_a_site_is_left_out() {
        let mods = [bundle_of(
            "alpha",
            "[[injection]]\nid = \"broken\"\nkind = \"HEAD\"\n",
        )];
        assert!(mods[0].injections.is_empty());
    }

    #[test]
    fn a_folder_is_one_bundle_or_a_folder_of_them() {
        let one = dir("one");
        write_bundle(&one, "hello", "fn hook() {}");
        let found = bundles(&one).unwrap();
        assert_eq!(
            (found[0].id.as_str(), found[0].version.as_str()),
            ("hello", "0.1.0")
        );
        let many = dir("many");
        write_bundle(&many.join("b"), "beta", "fn hook() {}");
        write_bundle(&many.join("a"), "alpha", "fn hook() {}");
        fs::create_dir_all(many.join("notes")).unwrap();
        let ids: Vec<String> = bundles(&many).unwrap().into_iter().map(|b| b.id).collect();
        assert_eq!(ids, ["alpha", "beta"]);
        assert!(bundles(&many.join("notes")).is_err());
    }

    #[test]
    fn a_registry_download_is_a_bundle_with_its_pins() {
        let d = dir("hnmod");
        write_native(&d.join("veinminer"), "veinminer", A, B);
        write_bundle(&d.join("local"), "hello", "fn hook() {}");
        let found = bundles(&d).unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found[1].id, "veinminer");
        assert_eq!(found[1].dir, d.join("veinminer/mixins/veinminer"));
        assert_eq!(found[1].eclipse.as_deref(), Some(A));
        assert_eq!(found[1].mixins.as_deref(), Some(B));
        assert_eq!(
            (found[0].eclipse.as_ref(), found[0].mixins.as_ref()),
            (None, None)
        );
    }

    #[test]
    fn a_hooks_bundle_is_read_and_repinned_to_the_build() {
        let d = dir("hooks");
        write_native(&d, "zoom", A, B);
        let json = fs::read_to_string(d.join(NATIVE_MANIFEST)).unwrap();
        let json = json.replacen('}', ",\"hooks\":1}", 1);
        fs::write(d.join(NATIVE_MANIFEST), json).unwrap();
        let mut found = bundles(&d).unwrap();
        assert_eq!(
            (found[0].hooks, found[0].eclipse.as_deref()),
            (Some(1), Some(A))
        );
        repin(&mut found[0], B).unwrap();
        let again = bundle(&found[0].dir, None).unwrap();
        assert_eq!(
            (again.id.as_str(), again.eclipse.as_deref()),
            ("zoom", Some(B))
        );
    }
}
