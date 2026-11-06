use super::architecture::{
    architecture_for_asset, game_assets, selected_game_architecture, GameArchitecture,
    InstalledGameArchitecture, GAME_BUILD_ARCHITECTURE,
};
use super::net::{fetch_small, get};
use super::paths::{version_dir, versions_dir};
use super::platform::{CLIENT, GAME_APP};
use super::releases::{compare_versions, is_game_tag, releases, Release};
use super::resources::reuse_prepared_for;
use super::settings::Settings;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

pub(crate) const INSTALL_MARK: &str = ".installed";

/// The mod API level of the game players run: the selected version's, else the newest installed
/// one's. None with no version installed: the launcher doesn't know yet which game will run mods.
pub fn game_mod_api_level() -> Option<u32> {
    let installed = installed();
    let tag = Settings::load()
        .default_version
        .filter(|s| installed.contains(s))
        .or_else(|| installed.into_iter().next())?;
    Some(mod_api_level(&tag))
}

/// A game version's mod API level, read once from the mark its program carries
/// (`eclipse_mods::api_level::MARK`). A program from before the mark serves level 0, the
/// contract of Eclipse 0.2.67 and 0.2.68; so does one that can't be read now, asked again next
/// time.
pub fn mod_api_level(tag: &str) -> u32 {
    static LEVELS: Mutex<Vec<(String, u32)>> = Mutex::new(Vec::new());
    let mut levels = LEVELS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(&(_, level)) = levels.iter().find(|(t, _)| t == tag) {
        return level;
    }
    let Some(level) = program_mod_api_level(&version_dir(tag).join(CLIENT)) else {
        return 0;
    };
    levels.push((tag.to_owned(), level));
    level
}

/// The mod API level in the game program at `path`: its mark's, else 0; None when it can't be
/// read.
pub(super) fn program_mod_api_level(path: &Path) -> Option<u32> {
    let program = fs::read(path).ok()?;
    Some(eclipse_mods::api_level::read_mark(&program).unwrap_or(0))
}

/// Installed versions, newest tag first.
pub fn installed() -> Vec<String> {
    installed_in(&versions_dir())
}

pub(crate) fn is_installed(dir: &Path) -> bool {
    dir.join(INSTALL_MARK).is_file()
}

/// Marks the version in `dir` installed, or used again: [`installed_at`] then says now.
pub(crate) fn mark_installed(dir: &Path) -> std::io::Result<()> {
    fs::write(dir.join(INSTALL_MARK), "")
}

/// When the version in `dir` was installed or last marked used; None when it isn't installed.
pub(crate) fn installed_at(dir: &Path) -> Option<SystemTime> {
    let mark = fs::metadata(dir.join(INSTALL_MARK)).ok()?;
    Some(mark.modified().unwrap_or(SystemTime::UNIX_EPOCH))
}

/// Copies installed version `from` into `to` for another build of the game to play on its files:
/// its game program, the app bundle macOS starts that through (`game_app`) and its install mark
/// stay behind, and so do the `leave_out` names.
pub(crate) fn copy_version_except(
    from: &Path,
    to: &Path,
    leave_out: &[&str],
) -> Result<(), String> {
    let own = [INSTALL_MARK, CLIENT, GAME_APP];
    fs::create_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
    for entry in fs::read_dir(from).map_err(|e| e.to_string())?.flatten() {
        let name = entry.file_name();
        if own.iter().chain(leave_out).any(|n| name == *n) {
            continue;
        }
        let path = entry.path();
        let to = to.join(&name);
        let copied = match path.is_dir() {
            true => crate::files::copy_dir(&path, &to, &[]),
            false => crate::files::copy_file(&path, &to),
        };
        copied.map_err(|e| format!("Could not copy {}: {e}", path.display()))?;
    }
    Ok(())
}

pub(super) fn installed_in(versions: &Path) -> Vec<String> {
    let mut tags: Vec<String> = fs::read_dir(versions)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| is_installed(&e.path()))
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    tags.sort_by(|a, b| compare_versions(b, a));
    tags
}

/// The SHA-256 the release's signed checksums give for `asset`, or why it must not be installed.
pub(super) fn signed_digest(tag: &str, asset: &str) -> Result<String, String> {
    crate::signed::digest(
        tag,
        asset,
        &eclipse_release_sign::trusted_keys(),
        fetch_small,
    )
}

/// Downloads `url` into `to`; the file is kept only if its SHA-256 is `sha256`, the signed (or
/// pinned) one. A `size` of 0 takes the server's Content-Length for the progress.
pub(crate) fn download(
    url: &str,
    size: u64,
    sha256: &str,
    to: &Path,
    progress: &impl Fn(f32),
) -> Result<(), String> {
    let response = get(url)?;
    let size = match size {
        0 => response
            .header("Content-Length")
            .and_then(|n| n.parse().ok())
            .unwrap_or(0),
        n => n,
    };
    let name = url.rsplit('/').next().unwrap_or(url);
    save_checked(response.into_reader(), size, name, sha256, to, progress)
}

/// Saves `reader` as `to` once all of it has arrived and its SHA-256 is `sha256`: until then it
/// has a temporary name beside `to`, so `to` is never a partial or unchecked file.
pub(super) fn save_checked(
    reader: impl Read,
    size: u64,
    name: &str,
    sha256: &str,
    to: &Path,
    progress: &impl Fn(f32),
) -> Result<(), String> {
    let mut temp = to.as_os_str().to_owned();
    temp.push(format!(".part-{}", std::process::id()));
    let temp = PathBuf::from(temp);
    let saved = write_checked(reader, size, name, sha256, &temp, progress)
        .and_then(|()| fs::rename(&temp, to).map_err(|e| e.to_string()));
    if saved.is_err() {
        let _ = fs::remove_file(&temp);
    }
    saved
}

fn write_checked(
    mut reader: impl Read,
    size: u64,
    name: &str,
    sha256: &str,
    to: &Path,
    progress: &impl Fn(f32),
) -> Result<(), String> {
    let mut file = fs::File::create(to).map_err(|e| e.to_string())?;
    let mut hash = eclipse_release_sign::Sha256::default();
    let (mut done, mut buf) = (0u64, vec![0u8; 1 << 16]);
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("Download interrupted: {e}"))?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        done += n as u64;
        progress((done as f32 / size.max(1) as f32).min(1.0) * 0.9);
    }
    eclipse_release_sign::check(name, sha256, &hash.hex())
}

pub(super) fn unpack(archive: &Path, url: &str, to: &Path) -> Result<(), String> {
    let file = fs::File::open(archive).map_err(|e| e.to_string())?;
    if url.ends_with(".tar.gz") {
        tar::Archive::new(flate2::read::GzDecoder::new(file))
            .unpack(to)
            .map_err(|e| e.to_string())
    } else {
        zip::ZipArchive::new(file)
            .and_then(|mut z| z.extract(to))
            .map_err(|e| e.to_string())
    }
}

/// Downloads one release, checks it against its signed checksums and unpacks it; `progress` gets
/// the fraction done.
pub fn install(release: &Release, progress: impl Fn(f32)) -> Result<(), String> {
    let tag = release.tag.as_str();
    let settings = Settings::load();
    let (asset, fallback) = game_assets(&settings);
    let asset = asset_installed(release, asset, fallback).ok_or_else(|| {
        crate::i18n::trf(
            "{tag} has no build for the selected game architecture.",
            &[("tag", tag)],
        )
        .to_string()
    })?;
    let sha256 = match &release.nightly {
        // Signed for its own version (SHA256SUMS.v2.sig), which its title names and it installs as;
        // served under its own GitHub tag. A title that doesn't parse strictly is refused: nothing
        // signs GitHub's publication time.
        Some(github_tag) => {
            let version = eclipse_release_sign::nightly_title_version(&release.name)
                .filter(|v| *v == release.tag)
                .ok_or_else(|| {
                    crate::i18n::trf(
                        "This Nightly's title ({title}) doesn't name its date and commit as Eclipse \
                         signs them, so it can't be checked: it was not installed.",
                        &[("title", &format!("{:?}", release.name))],
                    )
                })?;
            crate::signed::nightly_digest(
                github_tag,
                &version,
                asset,
                &eclipse_release_sign::trusted_keys(),
                fetch_small,
            )?
        }
        None if is_game_tag(tag) => signed_digest(tag, asset)?,
        None => return Err(format!("{tag} is not an Eclipse release.")),
    };
    install_as(
        release,
        tag,
        &sha256,
        selected_game_architecture(&settings),
        progress,
    )
}

/// The asset `release` installs from: the fallback when [`game_releases_with`] took it (its file
/// name ends the download address), so the signed checksum checked is that file's.
pub(super) fn asset_installed<'a>(
    release: &Release,
    asset: Option<&'a str>,
    fallback: Option<&'a str>,
) -> Option<&'a str> {
    let file = release
        .asset
        .as_ref()
        .and_then(|(url, _)| url.rsplit('/').next());
    if fallback.is_some_and(|name| file == Some(name)) {
        fallback
    } else if asset.is_some_and(|name| file == Some(name)) {
        asset
    } else {
        None
    }
}

/// Downloads `release`'s archive for this platform, checks it against `sha256` (its signed
/// checksum), and unpacks it as the installed version `tag`.
pub(crate) fn install_as(
    release: &Release,
    tag: &str,
    sha256: &str,
    requested_architecture: Option<GameArchitecture>,
    progress: impl Fn(f32),
) -> Result<(), String> {
    let (url, size) = release
        .asset
        .clone()
        .ok_or_else(|| format!("{tag} has no build for this platform."))?;
    let root = versions_dir();
    let partial = root.join(format!(".{tag}.partial"));
    let _ = fs::remove_dir_all(&partial);
    fs::create_dir_all(&partial).map_err(|e| e.to_string())?;
    let archive = partial.join("download");
    download(&url, size, sha256, &archive, &progress)?;
    let unpacked = partial.join("unpacked");
    unpack(&archive, &url, &unpacked)?;
    progress(0.97);
    // The archives hold one top folder (eclipse-linux-x86_64/...); the version folder is its contents.
    let entries: Vec<_> = fs::read_dir(&unpacked)
        .map_err(|e| e.to_string())?
        .flatten()
        .collect();
    let top = match entries.as_slice() {
        [only] if only.path().is_dir() => only.path(),
        _ => unpacked.clone(),
    };
    if !top.join(CLIENT).exists() {
        return Err(format!("The download has no {CLIENT}"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(top.join(CLIENT), fs::Permissions::from_mode(0o755));
    }
    if let (Some(requested), Some(binary)) = (requested_architecture, architecture_for_asset(&url))
    {
        fs::write(
            top.join(GAME_BUILD_ARCHITECTURE),
            serde_json::to_vec(&InstalledGameArchitecture { requested, binary })
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    }
    let target = version_dir(tag);
    let _ = fs::remove_dir_all(&target);
    fs::rename(&top, &target).map_err(|e| e.to_string())?;
    mark_installed(&target).map_err(|e| e.to_string())?;
    let _ = fs::remove_dir_all(&partial);
    reuse_prepared_for(tag);
    progress(1.0);
    Ok(())
}

pub fn remove(tag: &str) -> Result<(), String> {
    let dir = version_dir(tag);
    remove_in(&dir)
}

pub(super) fn remove_in(dir: &Path) -> Result<(), String> {
    let installed = is_installed(dir);
    // The marker goes first: a version counted as installed until its folder is gone comes back
    // into `installed` when an overlapping removal re-reads the list mid-delete.
    let _ = fs::remove_file(dir.join(INSTALL_MARK));
    // Windows holds a file for a moment after the game, a scan or the launcher's own reader lets
    // go of it, and remove_dir_all stops at the first one. Retry for a second; a folder that is
    // already gone is removed.
    for attempt in 1.. {
        match fs::remove_dir_all(dir) {
            Ok(()) => break,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
            Err(e) if attempt == 5 => {
                // A failed removal must remain discoverable and retryable. Files may
                // already have gone, so this is recovery metadata, not a rollback.
                if installed && dir.is_dir() {
                    mark_installed(dir).map_err(|restore| format!("{e}; {restore}"))?;
                }
                return Err(e.to_string());
            }
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(250)),
        }
    }
    Ok(())
}

/// The bytes an installed version takes on disk.
pub fn disk_use(tag: &str) -> u64 {
    fn size(dir: &Path) -> u64 {
        fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| match e.metadata() {
                Ok(m) if m.is_dir() => size(&e.path()),
                Ok(m) => m.len(),
                Err(_) => 0,
            })
            .sum()
    }
    size(&version_dir(tag))
}

/// The Minecraft version a Eclipse build targets, from its research-lock.json.
pub fn game_version(tag: &str) -> String {
    game_version_at(&version_dir(tag))
}

pub(super) fn read_json(path: &Path) -> Option<serde_json::Value> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

pub(super) fn game_version_at(dir: &Path) -> String {
    read_json(&dir.join("research-lock.json"))
        .and_then(|v| v["minecraft"]["version"].as_str().map(String::from))
        .unwrap_or_else(|| eclipse_core::VANILLA_VERSION.into())
}

/// The installed version to use: the one asked for, else the selected one, else the newest.
pub fn resolve_version(requested: Option<&str>) -> Result<String, String> {
    let installed = installed();
    if let Some(tag) = requested {
        return installed
            .iter()
            .find(|t| *t == tag)
            .cloned()
            .ok_or_else(|| format!("{tag} is not installed."));
    }
    let selected = Settings::load()
        .default_version
        .filter(|s| installed.contains(s));
    selected
        .or_else(|| installed.first().cloned())
        .ok_or_else(no_version_installed)
}

/// What a command that needs an installed version says when there is none: the way to get one.
pub fn no_version_installed() -> String {
    use crate::i18n::tr;
    format!(
        "{} {}",
        tr("No version is installed yet."),
        tr("Run `eclipse-launcher install latest` to get one.")
    )
}

/// Removes an installed version, unless a program runs from it, and moves the selection to the
/// newest one left if needed.
pub fn remove_version(tag: &str) -> Result<(), String> {
    if !installed().iter().any(|t| t == tag) {
        return Err(format!("{tag} is not installed."));
    }
    if in_use(tag) {
        return Err(format!("{tag} is running. Quit it first."));
    }
    remove(tag)?;
    Settings::update(|settings| {
        if settings.default_version.as_deref() == Some(tag) {
            settings.default_version = installed().first().cloned();
        }
    })
}

/// The release versions past the `keep` newest of `installed` (newest first). Nightly builds are
/// left out: `nightly::after_update` removes the older ones.
pub(super) fn stale_versions(installed: &[String], keep: usize) -> Vec<String> {
    installed
        .iter()
        .filter(|t| is_game_tag(t))
        .skip(keep)
        .cloned()
        .collect()
}

pub(crate) fn needed_versions() -> Vec<String> {
    let mut needed: Vec<String> = Settings::load().default_version.into_iter().collect();
    needed.extend(
        crate::instances::list()
            .into_iter()
            .filter_map(|i| i.version),
    );
    needed
}

/// Removes the `stale` versions, except the ones [`needed_versions`] names and the ones a program
/// runs from. One that does not delete is logged and the rest still go. Returns the ones removed.
pub(crate) fn remove_unused(stale: Vec<String>) -> Vec<String> {
    let needed = needed_versions();
    let mut gone = Vec::new();
    for tag in stale.into_iter().filter(|t| !needed.contains(t)) {
        if in_use(&tag) {
            eprintln!("Keeping old version {tag}: the game is running from it.");
            continue;
        }
        match remove(&tag) {
            Ok(()) => {
                eprintln!("Removed old version {tag}.");
                gone.push(tag);
            }
            Err(e) => eprintln!("Could not remove old version {tag}: {e}"),
        }
    }
    gone
}

/// Whether a running program was started from the version's folder: the game and a server take
/// their program and `--resources` from it. Windows refuses to delete a running program's files,
/// so there the delete itself fails. With no answer from `ps` the version counts as running.
#[cfg(unix)]
pub(crate) fn in_use(tag: &str) -> bool {
    let dir = format!("{}/", version_dir(tag).display());
    crate::process::running().map_or(true, |running| {
        running.iter().any(|p| p.command.contains(&dir))
    })
}

#[cfg(windows)]
pub(crate) fn in_use(tag: &str) -> bool {
    crate::windows_runtime::in_use(&version_dir(tag))
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn in_use(_tag: &str) -> bool {
    false
}

/// Deletes the release versions nothing needs after an install or update, unless the player
/// turned that off: all but the newest few (`Settings::keep_newest`), as [`remove_unused`] does.
/// Returns the versions deleted. They install again from GitHub.
pub fn prune_old_versions() -> Vec<String> {
    let settings = Settings::load();
    if !settings.auto_prune() {
        return Vec::new();
    }
    remove_unused(stale_versions(&installed(), settings.keep_newest()))
}

pub fn after_install(tag: &str, as_update: bool) -> Vec<String> {
    let mut gone = Vec::new();
    if as_update && crate::nightly::date_of(tag).is_some() {
        gone = crate::nightly::after_update(tag);
    }
    gone.extend(prune_old_versions());
    gone
}

/// Whether a version installed as an update becomes the selected one: when the player had none,
/// was on a Nightly, or on the newest installed release (so a player of the release channel moves
/// to the Nightly at the first check, docs/release-flow.md). An older release picked by hand
/// stays, and so does an imported build (`build_import`); an instance pinned to a version keeps it
/// either way.
pub fn select_after_update(selected: Option<&str>, installed_before: &[String]) -> bool {
    match selected {
        None => true,
        Some(s) if crate::nightly::date_of(s).is_some() => true,
        Some(s) if crate::build_import::is_imported(s) => false,
        Some(s) => installed_before
            .iter()
            .find(|t| is_game_tag(t))
            .is_none_or(|newest| newest == s),
    }
}

/// Installs `tag` (or with "latest" the newest release, pre-releases left out), then updates the selection as
/// `select_after_update` says when `as_update`, or always selects it otherwise.
pub fn install_tag(tag: &str, as_update: bool, progress: impl Fn(f32)) -> Result<String, String> {
    let releases = releases()?;
    let release = if tag == "latest" {
        releases
            .iter()
            .find(|r| r.asset.is_some() && !r.prerelease)
            .ok_or("No release has a build for this platform yet.")?
    } else {
        releases
            .iter()
            .find(|r| r.tag == tag)
            .ok_or_else(|| format!("No Eclipse release is tagged {tag}."))?
    };
    if release.asset.is_none() {
        return Err(format!("{} has no build for this platform.", release.tag));
    }
    let before = installed();
    let downgrade = nightly_downgrade(release, &before);
    if let Some(warning) = &downgrade {
        eprintln!("{warning}");
    }
    install(release, progress)?;
    Settings::update(|settings| {
        // An older Nightly is a deliberate rollback: installed, but an update never moves the selection back.
        if !as_update
            || (downgrade.is_none()
                && select_after_update(settings.default_version.as_deref(), &before))
        {
            settings.default_version = Some(release.tag.clone());
        }
    })?;
    after_install(&release.tag, as_update);
    Ok(release.tag.clone())
}

/// A warning when `release` is a Nightly older than a Nightly already installed: a rollback, which is
/// allowed, but never what an update should do silently (an older build served as the current one
/// shows its own date, since that date is signed). None otherwise.
pub fn nightly_downgrade(release: &Release, installed: &[String]) -> Option<String> {
    release.nightly.as_ref()?;
    let newest = installed
        .iter()
        .filter(|v| crate::nightly::date_of(v).is_some())
        .max_by(|a, b| compare_versions(a, b))?;
    compare_versions(&release.tag, newest).is_lt().then(|| {
        crate::i18n::trf(
            "{tag} is older than {newest}, which is installed: installing it as a rollback. The selection \
             only moves to it when you pick it.",
            &[("tag", &release.tag), ("newest", newest)],
        )
    })
}
