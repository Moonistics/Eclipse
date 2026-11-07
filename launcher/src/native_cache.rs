//! `<build_dir>/native/`, the folder native builds keep everything in ([`Dirs`]), and the
//! disposable part of it: Cargo's outputs ([`Cache`]). Installed games and player data are never
//! targets. Keep Cargo's profile directories and lock files in place: unlinking a locked inode
//! would let another Cargo acquire a replacement lock while cleanup was still deleting its outputs.
use crate::backend;
use crate::i18n::{tr, trf};
use std::fs::{self, File, Metadata};
use std::io;
use std::path::{Path, PathBuf};

/// Cargo's profile folders the builds compile into (`native_mods::Profile::cargo`).
pub(crate) const FAST: &str = "fast";
pub(crate) const RELEASE: &str = "release";
const PROFILES: [&str; 2] = [FAST, RELEASE];

/// The folders under `target/` builds compile into: the host's own builds at its top, and a
/// Windows ARM64 game's in a folder of its own, so the two never reuse each other's crates.
const PLATFORMS: [Option<&str>; 2] = [None, Some(crate::rust_toolchain::WINDOWS_ARM64_TARGET)];

/// The folder under `target/` a build for Rust target `rust_target` compiles into, if not the top.
pub(crate) fn platform_folder(rust_target: &str) -> Option<&'static str> {
    PLATFORMS
        .into_iter()
        .flatten()
        .find(|folder| *folder == rust_target)
}

/// The staging folders horizon-mixins reads the mods from: a diagnostic's file path names its mod
/// as the folder after one of these.
const MODS_INCOMING: &str = "mods.incoming";
const MIXINS_INCOMING: &str = "mixins.incoming";
pub(crate) const STAGING: [&str; 2] = [MIXINS_INCOMING, MODS_INCOMING];

/// Every path under `<build_dir>/native/`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Dirs {
    root: PathBuf,
}

impl Dirs {
    pub(crate) fn of(build_dir: &Path) -> Self {
        Self {
            root: build_dir.join("native"),
        }
    }

    pub(crate) fn current() -> Self {
        Self::of(&backend::build_dir())
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    fn at(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    pub(crate) fn log(&self) -> PathBuf {
        self.at("build.log")
    }

    pub(crate) fn records(&self) -> PathBuf {
        self.at("built.json")
    }

    pub(crate) fn dates(&self) -> PathBuf {
        self.at("commit-dates.json")
    }

    pub(crate) fn tools(&self) -> PathBuf {
        self.at("tools")
    }

    pub(crate) fn source(&self) -> PathBuf {
        self.at("source")
    }

    pub(crate) fn installed_incoming(&self) -> PathBuf {
        self.at("installed.incoming")
    }

    pub(crate) fn mods_incoming(&self) -> PathBuf {
        self.at(MODS_INCOMING)
    }

    pub(crate) fn mixins_incoming(&self) -> PathBuf {
        self.at(MIXINS_INCOMING)
    }

    pub(crate) fn stage_new(&self) -> PathBuf {
        self.at("stage.new")
    }

    pub(crate) fn stage(&self) -> PathBuf {
        self.at("stage")
    }

    pub(crate) fn patched(&self) -> PathBuf {
        self.at("patched")
    }

    pub(crate) fn programs(&self) -> PathBuf {
        self.at("programs")
    }

    pub(crate) fn post_build_cache(&self) -> PathBuf {
        self.at("cache")
    }

    pub(crate) fn server_builds(&self) -> PathBuf {
        self.at("server-builds")
    }

    pub(crate) fn github(&self) -> PathBuf {
        self.at("github")
    }

    /// Cargo's target folder for a build in `platform` ([`platform_folder`]).
    pub(crate) fn target(&self, platform: Option<&str>) -> PathBuf {
        let target = self.at("target");
        match platform {
            Some(platform) => target.join(platform),
            None => target,
        }
    }

    fn lock(&self) -> PathBuf {
        self.at(".build.lock")
    }
}

const DEFAULT_GIB: u64 = 30;

/// The cache limit in GiB (`settings native-cache-gib`); 0 turns automatic cleanup off.
pub fn limit_gib(settings: &backend::Settings) -> u64 {
    settings.native_cache_gib.unwrap_or(DEFAULT_GIB)
}

pub(crate) fn limit_bytes(settings: &backend::Settings) -> Option<u64> {
    match limit_gib(settings) {
        0 => None,
        gib => Some(gib.saturating_mul(1 << 30)),
    }
}

#[derive(Debug)]
pub(crate) enum Error {
    Busy,
    Unsafe(PathBuf),
    Io(io::Error),
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::Busy => {
                tr("A native build or cache cleanup is running. Try again when it finishes.").into()
            }
            Self::Unsafe(path) => trf(
                "Refusing native cache cleanup at an unexpected file or link: {path}",
                &[("path", &path.display().to_string())],
            ),
            Self::Io(error) => trf(
                "Could not access the native build cache: {error}",
                &[("error", &error.to_string())],
            ),
        };
        f.write_str(&text)
    }
}

impl From<Error> for String {
    fn from(error: Error) -> Self {
        error.to_string()
    }
}

/// What `native-cache` prints.
#[derive(serde::Serialize)]
pub struct Info {
    pub path: PathBuf,
    pub bytes: u64,
    pub cleared_bytes: u64,
    pub limit_gib: u64,
}

/// Inspect or clear only Cargo's native outputs, while no build is using them. This does not
/// remove installed versions, instances, mods, saves, toolchains or downloads.
pub fn info(clear: bool) -> Result<Info, String> {
    let cache = Cache::acquire(&backend::build_dir())?;
    let bytes = cache.bytes()?;
    let cleared_bytes = if clear { cache.clear()? } else { 0 };
    Ok(Info {
        path: Dirs::current().target(None),
        bytes,
        cleared_bytes,
        limit_gib: limit_gib(&backend::Settings::load()),
    })
}

pub(crate) struct Cache {
    dirs: Dirs,
    _lease: Locked,
}

/// A held lock that is released before its file closes. A child process spawned at that moment
/// (Cargo, git) holds a copy of the descriptor until its exec, and closing ours alone would leave
/// the lock taken through that copy: the next lease or cleanup would read Busy.
struct Locked(File);

impl Drop for Locked {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

/// Whether `metadata` (from `symlink_metadata`) is a link: on Windows any reparse point,
/// junctions included, not just symbolic links.
pub(crate) fn linked(metadata: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

fn checked(path: &Path, directory: bool) -> Result<Option<Metadata>, Error> {
    match fs::symlink_metadata(path) {
        Ok(meta)
            if linked(&meta)
                || (if directory {
                    !meta.is_dir()
                } else {
                    !meta.is_file()
                }) =>
        {
            Err(Error::Unsafe(path.to_owned()))
        }
        Ok(meta) => Ok(Some(meta)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn lock(path: &Path) -> Result<Locked, Error> {
    checked(path, false)?;
    let file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    file.try_lock().map_err(|error| match error {
        fs::TryLockError::WouldBlock => Error::Busy,
        fs::TryLockError::Error(error) => Error::Io(error),
    })?;
    Ok(Locked(file))
}

fn size(path: &Path) -> Result<u64, Error> {
    let meta = fs::symlink_metadata(path)?;
    if linked(&meta) {
        return Err(Error::Unsafe(path.to_owned()));
    }
    if meta.is_file() {
        return Ok(meta.len());
    }
    if !meta.is_dir() {
        return Err(Error::Unsafe(path.to_owned()));
    }
    let mut bytes = 0u64;
    for entry in fs::read_dir(path)? {
        bytes = bytes.saturating_add(size(&entry?.path())?);
    }
    Ok(bytes)
}

impl Cache {
    /// One cross-process lease covers staging, Cargo, installation/export and cleanup.
    /// OS locks release on exit, including a crash; a leftover lock file is not a busy marker.
    pub(crate) fn acquire(build_dir: &Path) -> Result<Self, Error> {
        checked(build_dir, true)?;
        fs::create_dir_all(build_dir)?;
        let dirs = Dirs::of(&build_dir.canonicalize()?);
        checked(dirs.root(), true)?;
        fs::create_dir_all(dirs.root())?;
        let dirs = Dirs {
            root: dirs.root().canonicalize()?,
        };
        let lease = lock(&dirs.lock())?;
        Ok(Self {
            dirs,
            _lease: lease,
        })
    }

    pub(crate) fn bytes(&self) -> Result<u64, Error> {
        self.process(None, false)
    }

    pub(crate) fn clear(&self) -> Result<u64, Error> {
        self.process(None, true)
    }

    /// Clears the outputs when they take more than `limit` bytes; None never does.
    pub(crate) fn trim(&self, limit: Option<u64>) -> Result<u64, Error> {
        match limit {
            Some(limit) => self.process(Some(limit), true),
            None => Ok(0),
        }
    }

    // Detect a managed directory redirected through an ancestor link after taking the lease.
    fn unchanged_path(&self, path: &Path) -> Result<(), Error> {
        if path.canonicalize()? != path {
            return Err(Error::Unsafe(path.to_owned()));
        }
        Ok(())
    }

    fn folder(&self, path: PathBuf) -> Result<Option<PathBuf>, Error> {
        if checked(&path, true)?.is_none() {
            return Ok(None);
        }
        self.unchanged_path(&path)?;
        Ok(Some(path))
    }

    fn process(&self, limit: Option<u64>, clear: bool) -> Result<u64, Error> {
        checked(self.dirs.root(), true)?;
        self.unchanged_path(self.dirs.root())?;
        if self.folder(self.dirs.target(None))?.is_none() {
            return Ok(0);
        }
        let mut locks = Vec::new();
        let mut folders = Vec::new();
        for platform in PLATFORMS {
            let Some(target) = self.folder(self.dirs.target(platform))? else {
                continue;
            };
            for profile in PROFILES {
                // Also protects an older launcher's Cargo or one started outside our lease.
                if let Some(folder) = self.folder(target.join(profile))? {
                    locks.push(lock(&folder.join(".cargo-lock"))?);
                    folders.push(folder);
                }
            }
        }
        // Validate ALL profiles and descendants before deleting ANY output. Never traverse links.
        let mut entries = Vec::new();
        let mut bytes = 0u64;
        for folder in folders {
            for entry in fs::read_dir(folder)? {
                let entry = entry?;
                if entry.file_name() == ".cargo-lock" {
                    continue;
                }
                bytes = bytes.saturating_add(size(&entry.path())?);
                entries.push(entry.path());
            }
        }
        if !clear {
            return Ok(bytes);
        }
        if limit.is_some_and(|limit| bytes <= limit) {
            return Ok(0);
        }
        let records = self.dirs.records();
        let has_records = checked(&records, false)?.is_some();
        for path in entries {
            self.unchanged_path(&path)?;
            // The launcher lease and all Cargo locks remain held through removal. These are
            // fixed children of the validated cache profiles, never caller-supplied paths.
            if fs::symlink_metadata(&path)?.is_dir() {
                fs::remove_dir_all(path)?;
            } else {
                fs::remove_file(path)?;
            }
        }
        if has_records {
            fs::remove_file(records)?;
        }
        drop(locks);
        Ok(bytes)
    }
}

#[cfg(test)]
#[path = "native_cache/tests.rs"]
mod tests;
