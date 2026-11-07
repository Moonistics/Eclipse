//! Where the launcher's data lives: the default folders, or those `location.json` names (players
//! who moved their data with an earlier launcher). The pointer is never resolved through Settings.
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};

type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Roots {
    pub launcher: PathBuf,
    pub game: PathBuf,
    pub build: PathBuf,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Location {
    version: u32,
    roots: Roots,
}

fn bootstrap_path() -> Result<PathBuf> {
    // Portable installs and tests can keep their location pointer separate from the user profile.
    if let Some(path) = std::env::var_os("ECLIPSE_LAUNCHER_LOCATION_FILE").filter(|p| !p.is_empty())
    {
        return pointer_path(Path::new(&path));
    }
    dirs::config_dir()
        .map(|p| p.join("eclipse-launcher-location").join("location.json"))
        .ok_or_else(|| "Cannot locate the platform configuration folder for location.json".into())
}

fn overridden(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|v| !v.is_empty())
}

/// Roots resolved once per process.
static LOCATION: Mutex<Option<Result<Option<Roots>>>> = Mutex::new(None);
/// A startup failure falls back to the default folders, so a window can still open.
static FORCE_DEFAULT: AtomicBool = AtomicBool::new(false);
static STARTUP_ERROR: Mutex<Option<String>> = Mutex::new(None);

/// Stable for the lifetime of this process, including errors. Environment overrides are
/// applied per root by the caller; fully isolated callers need not read the real bootstrap.
pub fn configured_roots() -> Result<Option<Roots>> {
    if FORCE_DEFAULT.load(Ordering::Acquire) {
        return Ok(None);
    }
    let mut slot = LOCATION
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if slot.is_none() {
        let resolved = if overridden("ECLIPSE_LAUNCHER_HOME") && overridden("ECLIPSE_HOME") {
            Ok(None)
        } else {
            match bootstrap_path() {
                Ok(bootstrap) => resolve_at(&bootstrap),
                Err(error) => Err(error),
            }
        };
        *slot = Some(resolved);
    }
    slot.as_ref().expect("roots are resolved above").clone()
}

/// Run before any launcher path is used. A broken location never ends the process: it is
/// recorded ([`startup_error`]), and the default folders stand in.
pub fn startup() {
    let error = configured_roots().err();
    if error.is_some() {
        FORCE_DEFAULT.store(true, Ordering::Release);
    }
    *STARTUP_ERROR
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = error;
}

pub fn startup_error() -> Option<String> {
    STARTUP_ERROR
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

fn resolve_at(bootstrap: &Path) -> Result<Option<Roots>> {
    pointer_path(bootstrap)?;
    let bytes = match fs::read(bootstrap) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(problem(bootstrap, e)),
    };
    let location: Location = serde_json::from_slice(&bytes).map_err(|e| {
        format!(
            "Invalid {}: {e}; restore this file or the previous data folder",
            bootstrap.display()
        )
    })?;
    if location.version != 1 {
        return Err(format!(
            "Unsupported data location version {} in {}",
            location.version,
            bootstrap.display()
        ));
    }
    let canonical = validate_roots(&location.roots)?;
    if cfg!(windows) && !crate::backend::tool_safe(&canonical.build) {
        return Err(format!(
            "Build folder {} has a space or a non-ASCII character, which Windows build tools can't \
             read; restore location.json",
            canonical.build.display()
        ));
    }
    for root in [
        &location.roots.launcher,
        &location.roots.game,
        &location.roots.build,
    ] {
        if !root.is_dir() {
            return Err(format!(
                "Data folder {} is missing; reconnect its drive or restore location.json",
                root.display()
            ));
        }
    }
    Ok(Some(canonical))
}

fn validate_roots(roots: &Roots) -> Result<Roots> {
    let roots = Roots {
        launcher: checked_path(&roots.launcher)?,
        game: checked_path(&roots.game)?,
        build: checked_path(&roots.build)?,
    };
    for root in [&roots.launcher, &roots.game, &roots.build] {
        match fs::metadata(root) {
            Ok(meta) if !meta.is_dir() => {
                return Err(format!("Data root must be a folder: {}", root.display()))
            }
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(problem(root, e)),
            _ => {}
        }
    }
    if overlaps(&roots.launcher, &roots.game)
        || overlaps(&roots.build, &roots.game)
        || (overlaps(&roots.launcher, &roots.build) && !within(&roots.build, &roots.launcher))
    {
        return Err(
            "Source data roots overlap; only a build folder inside/equal to launcher is supported"
                .into(),
        );
    }
    Ok(roots)
}

fn within(path: &Path, parent: &Path) -> bool {
    #[cfg(windows)]
    {
        Path::new(&path.to_string_lossy().to_lowercase())
            .starts_with(parent.to_string_lossy().to_lowercase())
    }
    #[cfg(not(windows))]
    {
        path.starts_with(parent)
    }
}
fn overlaps(a: &Path, b: &Path) -> bool {
    within(a, b) || within(b, a)
}

/// Canonicalize existing ancestors, reject links/junctions and ambiguous relative components.
fn checked_path(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(format!(
            "Use an absolute path without '..': {}",
            path.display()
        ));
    }
    let mut current = PathBuf::new();
    for part in path.components() {
        current.push(part);
        // A Windows drive/UNC prefix alone is not a filesystem root. Inspect it
        // only after RootDir completes it, including canonical verbatim paths.
        if matches!(part, Component::Prefix(_)) {
            continue;
        }
        match fs::symlink_metadata(&current) {
            Ok(meta) => {
                reject_link(&current, &meta)?;
                current = fs::canonicalize(&current).map_err(|e| problem(&current, e))?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(problem(&current, e)),
        }
    }
    Ok(crate::files::plain(current))
}

/// The pointer sits in the platform config folder, which may be a link (a dotfiles-managed
/// `~/.config`, Silverblue's `/home`): unlike the data roots ([`checked_path`]), a link on the
/// way is followed, not refused. Refusing it would report a broken data folder at every start
/// to players who never moved their data.
fn pointer_path(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(format!(
            "Use an absolute path without '..': {}",
            path.display()
        ));
    }
    let mut existing = path;
    let mut rest = Vec::new();
    loop {
        match fs::canonicalize(existing) {
            Ok(real) => {
                let real = crate::files::plain(real);
                return Ok(rest.iter().rev().fold(real, |path, part| path.join(part)));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(problem(existing, e)),
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name.to_os_string());
                existing = parent;
            }
            _ => return Ok(path.to_path_buf()),
        }
    }
}

fn reject_link(path: &Path, meta: &fs::Metadata) -> Result<()> {
    #[cfg(windows)]
    let reparse = {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    };
    #[cfg(not(windows))]
    let reparse = false;
    if meta.file_type().is_symlink() || reparse {
        return Err(format!(
            "Symbolic links and reparse points cannot be relocated: {}",
            path.display()
        ));
    }
    Ok(())
}

fn problem(path: &Path, error: impl std::fmt::Display) -> String {
    format!("{}: {error}", path.display())
}

#[cfg(test)]
mod tests;
