use super::minecraft::{download_for_owner, minecraft_folders};
use super::net::get_bytes;
use super::paths::{own_minecraft_dir, version_dir, versions_dir};
use super::versions::{game_version_at, installed_in, read_json};
use crate::sources;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub fn resources(tag: &str) -> PathBuf {
    resources_at(&version_dir(tag))
}

pub(super) fn resources_at(dir: &Path) -> PathBuf {
    resources_in(dir, &game_version_at(dir))
}

/// The resources of the version folder `dir` for Minecraft `game_version`.
pub(crate) fn resources_in(dir: &Path, game_version: &str) -> PathBuf {
    dir.join(".local/eclipse-client").join(game_version)
}

pub(crate) const RESOURCES_MANIFEST: &str = "eclipse-resources.json";

pub fn is_prepared(tag: &str) -> bool {
    resources(tag).join(RESOURCES_MANIFEST).exists()
}

/// Whether a version's resources hold Minecraft's sounds, which `sounds` copies from the player's own
/// install beside the preparation. A version prepared while that copy failed (Mojang's republished 26.3
/// asset index, #1076), or one that reused such a version's resources, plays no sound at all (#1035).
pub fn has_sounds(tag: &str) -> bool {
    sounds_in(&resources(tag))
}

pub(super) fn sounds_in(resources: &Path) -> bool {
    // What the game's sound registry reads (eclipse-client sound_events.rs).
    resources.join("assets/minecraft/sounds.json").is_file()
}

/// Whether the resources `from` prepared are the ones preparing `to` would make: both pin the same
/// Minecraft jars (research-lock.json) and their preparers make the same output. A preparer
/// declares what it makes with `OUTPUT_VERSION` (crates/eclipse-prepare/src/lib.rs, whose sources
/// ship in the release folder), bumped by hand when the output changes and checked by CI
/// (tools/prepare_output.py), so a release that changes the preparer's code but not its output
/// keeps the resources. Preparers from before the number are known by their sources
/// (`LEGACY_OUTPUT_VERSIONS`); older ones still are compared file by file (`same_preparer`).
fn resources_compatible(from: &Path, to: &Path, manifest: &serde_json::Value) -> bool {
    let pins = |dir: &Path| {
        read_json(&dir.join("research-lock.json")).map(|lock| {
            ["version", "client_sha1", "server_sha1"].map(|k| lock["minecraft"][k].clone())
        })
    };
    let (Some(a), Some(b)) = (pins(from), pins(to)) else {
        return false;
    };
    if a != b || a.iter().any(|v| !v.is_string()) {
        return false;
    }
    match (output_version_at(from), output_version_at(to)) {
        (Some(a), Some(b)) => a == b,
        (None, None) => same_preparer(from, to, manifest),
        _ => false,
    }
}

/// The `OUTPUT_VERSION` of a version folder's preparer: the constant in its sources, or for a
/// preparer from before it, the number `LEGACY_OUTPUT_VERSIONS` gives its fingerprint. None for
/// an unknown older preparer.
fn output_version_at(dir: &Path) -> Option<u32> {
    output_version_in(dir, &LEGACY_OUTPUT_VERSIONS)
}

pub(super) fn output_version_in(dir: &Path, legacy: &[(&str, u32)]) -> Option<u32> {
    let crate_dir = dir.join(PREPARER_CRATE);
    let lib = fs::read_to_string(crate_dir.join("src/lib.rs")).ok()?;
    let declared = lib.lines().find_map(|line| {
        line.strip_prefix("pub const OUTPUT_VERSION: u32 = ")?
            .strip_suffix(';')?
            .parse()
            .ok()
    });
    if declared.is_some() {
        return declared;
    }
    let fingerprint = legacy_fingerprint(dir)?;
    legacy
        .iter()
        .find(|(known, _)| *known == fingerprint)
        .map(|&(_, version)| version)
}

/// Preparers from before `OUTPUT_VERSION`, by `legacy_fingerprint`, whose output
/// tools/prepare_output.py measured: what they prepare is version 1's.
const LEGACY_OUTPUT_VERSIONS: [(&str, u32); 2] = [
    // v0.2.52 to v0.2.64
    (
        "54218ade1c058b1f15ea4094bf8c1a7893adb37b1ed5cc803ddae8e950c31704",
        1,
    ),
    // v0.2.49 to v0.2.51
    (
        "da9ffe7558129c38b44abf43289e81946fe9f2512e5e07272eeae0cf0b4f1254",
        1,
    ),
];

/// The vanilla exporters those preparers ran (the `*_exporters_sha256` of their manifests), in
/// mods/eclipse/src/test/java/dev/eclipselab/parity.
pub(super) const LEGACY_EXPORTERS: [&str; 18] = [
    "ParityData.java",
    "VanillaBlockBehaviourExporter.java",
    "VanillaBlockLandingOracle.java",
    "VanillaBlockStateBindingOracle.java",
    "VanillaCollisionOracle.java",
    "VanillaCommandParseOracle.java",
    "VanillaCreativeTabsExporter.java",
    "VanillaCuboidModelOracle.java",
    "VanillaDamageSourceOracle.java",
    "VanillaEntityAttributeOracle.java",
    "VanillaEntityModelExporter.java",
    "VanillaEntityTypesExporter.java",
    "VanillaFaceVisibilityOracle.java",
    "VanillaGameplayFactsExporter.java",
    "VanillaHeightmapOracle.java",
    "VanillaMobDataExporter.java",
    "VanillaMovementRegistryOracle.java",
    "VanillaPlayerHealthOracle.java",
];

/// Where a version's parity exporters are: the Java the resources were made with.
const PARITY: &str = "mods/eclipse/src/test/java/dev/eclipselab/parity";

/// A version folder's preparer as the launchers before `OUTPUT_VERSION` compared it: the digest
/// of its sources (`eclipse_prepare::sources_sha256`) and of the exporters it ran, line endings
/// as checked out ignored.
pub(super) fn legacy_fingerprint(dir: &Path) -> Option<String> {
    use sha2::Digest;
    let sources = eclipse_prepare::sources_sha256(&dir.join(PREPARER_CRATE))?;
    let mut hash = sha2::Sha256::new();
    hash.update(sources.as_bytes());
    for name in LEGACY_EXPORTERS {
        let path = format!("{PARITY}/{name}");
        let data: Vec<u8> = fs::read(dir.join(&path))
            .ok()?
            .into_iter()
            .filter(|b| *b != b'\r')
            .collect();
        hash.update(format!("{path}\0{}\0", data.len()));
        hash.update(&data);
    }
    Some(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// How launchers before `OUTPUT_VERSION` compared two preparers, for versions older than the
/// ones `LEGACY_OUTPUT_VERSIONS` knows. The manifest records the hashes of the vanilla exporters
/// the resources were made with (mods/eclipse/src/test/java/.../parity), and the preparer writes
/// every file and the manifest's `schema`, so both need the same preparer and the same exporter
/// sources. The preparer is crates/eclipse-prepare and tools/prepare_native_client.py (the
/// Python preparer of older versions, a wrapper of the binary since); a file only one of the two
/// versions has is a different preparer. The helpers the Python preparer imported (lab.py and
/// the test_*_parity.py modules) change between releases in how they find and check the
/// player's files, which the output does not depend on; they are not compared.
fn same_preparer(from: &Path, to: &Path, manifest: &serde_json::Value) -> bool {
    let Some(fields) = manifest.as_object() else {
        return false;
    };
    let mut preparer = vec![PathBuf::from("tools/prepare_native_client.py")];
    for dir in [from, to] {
        preparer.extend(files_under(dir, Path::new(PREPARER_CRATE)));
    }
    let same_or_absent = |path: &Path| match (fs::read(from.join(path)), fs::read(to.join(path))) {
        (Ok(a), Ok(b)) => a == b,
        (Err(a), Err(b)) => {
            a.kind() == std::io::ErrorKind::NotFound && b.kind() == std::io::ErrorKind::NotFound
        }
        _ => false,
    };
    if !preparer.iter().all(|p| same_or_absent(p)) {
        return false;
    }
    let mut sources = vec![format!("{PARITY}/VanillaHeightmapOracle.java")];
    for (_, value) in fields
        .iter()
        .filter(|(key, _)| key.ends_with("_exporters_sha256"))
    {
        let Some(files) = value.as_object() else {
            return false;
        };
        sources.extend(files.keys().cloned());
    }
    let same = |source: &str| {
        let path = Path::new(source);
        path.is_relative()
            && path
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_)))
            && matches!((fs::read(from.join(path)), fs::read(to.join(path))), (Ok(a), Ok(b)) if a == b)
    };
    sources.iter().all(|s| same(s))
}

/// Where a version folder keeps the preparer's sources (the release package copies them).
pub(super) const PREPARER_CRATE: &str = "crates/eclipse-prepare";

/// The files under `dir/relative`, as paths relative to `dir`.
fn files_under(dir: &Path, relative: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir.join(relative)) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    for entry in entries.flatten() {
        let path = relative.join(entry.file_name());
        match entry.file_type() {
            Ok(t) if t.is_dir() => files.extend(files_under(dir, &path)),
            Ok(_) => files.push(path),
            Err(_) => {}
        }
    }
    files
}

/// After an install: when `tag` is not prepared but another installed version made the same
/// resources, copies them (not hard links: preparing again rewrites files in place, which would
/// change both). The player then needs neither their Minecraft folder nor a new preparation to
/// play a release that only changed Eclipse. Returns the version they came from.
pub(super) fn reuse_prepared(versions: &Path, tag: &str) -> Option<String> {
    let target = versions.join(tag);
    let to = resources_at(&target);
    if to.join(RESOURCES_MANIFEST).exists() {
        return None;
    }
    let version = game_version_at(&target);
    for other in installed_in(versions).into_iter().filter(|t| t != tag) {
        let source = versions.join(&other);
        if game_version_at(&source) != version {
            continue;
        }
        let from = resources_at(&source);
        let Some(manifest) = read_json(&from.join(RESOURCES_MANIFEST)) else {
            continue;
        };
        if !resources_compatible(&source, &target, &manifest) {
            continue;
        }
        // Copied beside it and renamed when complete: an interrupted copy is never "prepared".
        // The name is this call's own, as the window's start and an install can both copy.
        static COPIES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = COPIES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let partial = to.with_file_name(format!(".{version}.partial-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&partial);
        let copied = crate::files::copy_dir(&from, &partial, &[]).and_then(|_| {
            if to.join(RESOURCES_MANIFEST).exists() {
                return Ok(false); // Prepared meanwhile, by another copy or the player.
            }
            let _ = fs::remove_dir_all(&to);
            fs::rename(&partial, &to).map(|_| true)
        });
        let _ = fs::remove_dir_all(&partial);
        match copied {
            Ok(true) => return Some(other),
            Ok(false) => return None,
            Err(_) => {}
        }
    }
    None
}

/// What an install does once its folder is in place (an imported build's too, `build_import`):
/// another installed version's resources if it has none, and whatever its resources lack.
pub(crate) fn reuse_prepared_for(tag: &str) {
    reuse_prepared(&versions_dir(), tag);
    complete_assets(tag);
}

/// `reuse_prepared` for every installed version that is not prepared, oldest first, so a
/// version prepared this way can serve the next. Launchers before 0.1.4 installed new versions
/// without it; this catches those up. Returns each (version, where its resources came from).
/// Prepared versions also get the run-time reports (`complete_reports`) and the built-in packs,
/// panorama and sounds (`complete_assets`) they lack.
pub fn reuse_prepared_everywhere() -> Vec<(String, String)> {
    let versions = versions_dir();
    let reused = reuse_prepared_in(&versions);
    for tag in installed_in(&versions) {
        // Here, on the start-up thread, and not in `complete_assets`, which Play also runs: a
        // download of thousands of files is not for the moment the game is waited for.
        refill_own_store(&tag);
        complete_assets(&tag);
    }
    reused
}

pub(super) fn reuse_prepared_in(versions: &Path) -> Vec<(String, String)> {
    let mut tags = installed_in(versions);
    tags.reverse();
    let reused = tags
        .iter()
        .filter_map(|tag| reuse_prepared(versions, tag).map(|from| (tag.clone(), from)))
        .collect();
    for tag in &tags {
        complete_reports(versions, tag);
    }
    reused
}

/// The data generator reports the game reads at run time: the built-in registries' network ids
/// (without them no server can be joined or hosted) and the command tree.
pub(super) const RUNTIME_REPORTS: [&str; 2] = ["registries.json", "commands.json"];

/// Puts the data generator reports the game reads at run time (`RUNTIME_REPORTS`) into a prepared version's
/// resources when they are missing. Preparers before they shipped there left them only in the
/// scratch output of the version that ran the data generator (`.local/datagen/out/reports`),
/// and resources copied from another version (`reuse_prepared`) came without them. They depend
/// only on the server jar, so the version's own scratch output or any installed version pinning
/// the same jar serves, and no new preparation is needed. Returns whether all are there.
pub(super) fn complete_reports(versions: &Path, tag: &str) -> bool {
    let target = versions.join(tag);
    let to = resources_at(&target);
    if !to.join(RESOURCES_MANIFEST).exists() {
        return false;
    }
    let missing: Vec<_> = RUNTIME_REPORTS
        .iter()
        .filter(|name| !to.join(name).is_file())
        .collect();
    if missing.is_empty() {
        return true;
    }
    let server = |dir: &Path| {
        read_json(&dir.join("research-lock.json")).and_then(|lock| {
            let pins = ["version", "server_sha1"].map(|k| lock["minecraft"][k].clone());
            pins.iter().all(|v| v.is_string()).then_some(pins)
        })
    };
    let Some(pins) = server(&target) else {
        return false;
    };
    let reports = |dir: &Path| dir.join(".local/datagen/out/reports");
    let mut sources = vec![reports(&target)];
    for other in installed_in(versions).into_iter().filter(|t| t != tag) {
        let dir = versions.join(&other);
        if server(&dir).as_ref() == Some(&pins) {
            sources.push(resources_at(&dir));
            sources.push(reports(&dir));
        }
    }
    let mut complete = true;
    for name in missing {
        let Some(from) = sources.iter().map(|s| s.join(name)).find(|p| p.is_file()) else {
            complete = false;
            continue;
        };
        // Copied beside it and renamed: an interrupted copy never looks complete.
        let partial = to.join(format!(".{name}.partial-{}", std::process::id()));
        let copied = fs::copy(&from, &partial).and_then(|_| fs::rename(&partial, to.join(name)));
        if copied.is_err() {
            let _ = fs::remove_file(&partial);
            complete = false;
        }
    }
    complete
}

/// What a prepared version may lack that the launcher adds without preparing again, each copied
/// from the player's own Minecraft asset store as `eclipse-prepare <kind>` does: what the
/// player misses without it, its kind (`eclipse_prepare::missing_extras`, the same check as
/// `eclipse-prepare --check`), and the copy.
type Completion = (
    &'static str,
    &'static str,
    fn(&Path, Option<&Path>, eclipse_prepare::Log) -> Result<PathBuf, String>,
);

const COMPLETIONS: [Completion; 5] = [
    // Preparers before #1538 left the built-in packs out.
    (
        "Programmer Art and High Contrast",
        "packs",
        eclipse_prepare::packs,
    ),
    // A preparation that could not read the asset store keeps the client jar's 1x1 grey
    // placeholders, its English alone, and no sounds (#1867: Prism Launcher installs prepared
    // since 0.2.48).
    ("The title panorama", "panorama", eclipse_prepare::panorama),
    ("The Unifont glyphs", "fonts", eclipse_prepare::fonts),
    ("The languages", "languages", eclipse_prepare::languages),
    ("The sounds", "sounds", eclipse_prepare::sounds),
];

/// Adds what a prepared version lacks of `COMPLETIONS`: a new version reuses the resources of the
/// one before (`reuse_prepared`), which may come from a preparer that left them out. Without them
/// in any Minecraft folder, one line each in the log, once per launcher run, and the version plays
/// as before. Nothing is prepared again.
pub(crate) fn complete_assets(tag: &str) {
    complete_assets_in(&versions_dir(), tag, minecraft_folders, &|line| {
        eprintln!("{line}")
    });
}

/// Eclipse's own download (an owner's) may have stopped before the sounds and the languages,
/// and the copies of `complete_assets` read them from its store: asked again, once per run, through
/// the same owner gate as any download from Mojang (`download_for_owner`), which keeps the files
/// already whole. Called where a download is expected (`reuse_prepared_everywhere`), never on Play.
pub(super) fn refill_own_store(tag: &str) {
    static ASKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let own = own_minecraft_dir();
    let resources = resources_at(&versions_dir().join(tag));
    if !own.join("assets/indexes").is_dir()
        || !eclipse_prepare::missing_extras(&resources)
            .iter()
            .any(|(kind, _)| matches!(*kind, "languages" | "sounds"))
        || ASKED.swap(true, std::sync::atomic::Ordering::Relaxed)
    {
        return;
    }
    let Some(pins) = read_json(&version_dir(tag).join("research-lock.json"))
        .and_then(|l| sources::Pins::read(&l))
    else {
        return;
    };
    if let Err(e) = download_for_owner(&pins, &own, &get_bytes, &|line| eprintln!("{line}")) {
        eprintln!("{tag}: the missing assets were not downloaded: {e}");
    }
}

/// `folders`: where to look, asked only when the version lacks something.
pub(super) fn complete_assets_in(
    versions: &Path,
    tag: &str,
    folders: impl Fn() -> Vec<PathBuf>,
    log: &dyn Fn(String),
) {
    // The versions tried this run. Held while copying: the start, an install and Play can
    // complete the same version at once, and the copies' temporary names are per process.
    static TRIED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
    let root = versions.join(tag);
    let resources = resources_at(&root);
    let lacking = || -> Vec<&Completion> {
        if !resources.join(RESOURCES_MANIFEST).exists() {
            return Vec::new();
        }
        let missing = eclipse_prepare::missing_extras(&resources);
        COMPLETIONS
            .iter()
            .filter(|(_, kind, _)| missing.iter().any(|(m, _)| m == kind))
            .collect()
    };
    if lacking().is_empty() {
        return;
    }
    let mut tried = TRIED.lock().unwrap_or_else(|e| e.into_inner());
    let lacking = lacking();
    if tried.contains(&root) || lacking.is_empty() {
        return;
    }
    tried.push(root.clone());
    let folders = folders();
    for (what, _, copy) in lacking {
        let mut first_error = None;
        let added = folders.iter().any(|folder| {
            let copied = super::prepare::caught("the copy stopped unexpectedly", || {
                copy(&root, Some(folder), &|line| log(line.to_string()))
            });
            copied.map_err(|e| first_error.get_or_insert(e)).is_ok()
        });
        if !added {
            log(format!(
                "{tag}: {what} not added: {}",
                first_error.unwrap_or_else(|| "no Minecraft folder found".into())
            ));
        }
    }
}
