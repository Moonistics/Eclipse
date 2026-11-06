use std::fs;
use std::path::{Path, PathBuf};

/// The launcher's data folder; `ECLIPSE_LAUNCHER_HOME` overrides it (for tests and scripts).
pub fn data_dir() -> PathBuf {
    root(
        "ECLIPSE_LAUNCHER_HOME",
        |roots| roots.launcher,
        "eclipse-launcher",
    )
}

fn root(
    variable: &str,
    moved: impl FnOnce(crate::data_location::Roots) -> PathBuf,
    default: &str,
) -> PathBuf {
    if let Some(home) = std::env::var_os(variable).filter(|h| !h.is_empty()) {
        return PathBuf::from(home);
    }
    // A location that does not open stands in as the default folders, as
    // `data_location::startup` makes it for the window: never a panic here.
    if let Ok(Some(roots)) = crate::data_location::configured_roots() {
        return moved(roots);
    }
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(default)
}

/// Eclipse's own Minecraft folder, in the official launcher's layout: what it downloads from
/// Mojang for an account that owns the game (versions, libraries, assets, Java runtimes).
pub fn own_minecraft_dir() -> PathBuf {
    data_dir().join("minecraft")
}

/// Where other launchers' game files are linked into the official layout for a preparation
/// (`sources::resolve`).
pub(super) fn linked_minecraft_dir() -> PathBuf {
    data_dir().join("linked-minecraft")
}

/// `path` spelled only in ASCII: what Windows' MinGW-w64 tools can open (they read paths
/// through ANSI APIs, so a Cyrillic user name in %APPDATA% breaks them, #5189).
pub(super) fn ascii(path: &Path) -> bool {
    path.as_os_str().to_str().is_some_and(str::is_ascii)
}

/// `path` as Windows' MinGW-w64 tools can take it: [`ascii`], and on Windows without a space.
/// `windres` hands its include folders to the preprocessor unquoted, so a user name with a space
/// in %APPDATA% splits there ("cc1.exe: fatal error: v\: No such file or directory").
pub(super) fn tool_safe(path: &Path) -> bool {
    ascii(path) && !(cfg!(windows) && path.as_os_str().to_str().is_some_and(|p| p.contains(' ')))
}

/// Where builds happen — the Rust toolchain, its Cargo home and `native_mods::root` — as a
/// path every build tool can read: `data_dir()` resolved, so the tools are never handed a
/// link (horizon-mixins refuses one, and macOS's /tmp is one). On Windows, when the resolved
/// path isn't [`tool_safe`], [`windows_build_dir`] names the same folder a way MinGW-w64 can open.
pub fn build_dir() -> PathBuf {
    if std::env::var_os("ECLIPSE_LAUNCHER_HOME").is_none_or(|p| p.is_empty()) {
        // A location that does not open stands in as the default folders, as
        // `data_location::startup` makes it for the window: never a panic here.
        if let Ok(Some(roots)) = crate::data_location::configured_roots() {
            return roots.build;
        }
    }
    let data = data_dir();
    let _ = fs::create_dir_all(&data);
    let real = data.canonicalize().map(crate::files::plain).unwrap_or(data);
    if tool_safe(&real) {
        return real;
    }
    #[cfg(windows)]
    return windows_build_dir(&real);
    #[cfg(not(windows))]
    real
}

/// [`build_dir`] when `real` isn't [`tool_safe`]: [`ascii_build_dir`], a real folder of
/// ProgramData's. Only when it can't be created, the data folder's short (8.3) name.
///
/// The short name came first until #6009, and wasn't enough: it is only another spelling of the
/// same folder, and rustc canonicalizes its own path to find its sysroot, which spells the long,
/// Cyrillic name again and hands it to the linker (`x86_64-w64-mingw32-gcc`: "No such file or
/// directory" on `C:/Users/\xca\xe8…`). A folder whose real name is ASCII stays ASCII however a
/// tool resolves it.
#[cfg(windows)]
fn windows_build_dir(real: &Path) -> PathBuf {
    let dir = ascii_build_dir(real, std::env::var_os("ProgramData").map(PathBuf::from));
    if fs::create_dir_all(&dir).is_ok() {
        return dir;
    }
    short_path(real).filter(|p| tool_safe(p)).unwrap_or(dir)
}

/// The build folder for a data folder `real` whose path isn't [`ascii`]: in `program_data`
/// (`%ProgramData%`, `C:\ProgramData` when unset, relative or not ASCII), which every user may
/// create folders in, named for `real` so each user's is their own.
#[cfg_attr(not(windows), allow(dead_code))]
pub(super) fn ascii_build_dir(real: &Path, program_data: Option<PathBuf>) -> PathBuf {
    use sha2::{Digest, Sha256};
    let public = program_data
        .filter(|p| p.is_absolute() && tool_safe(p))
        .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"));
    let key = Sha256::digest(real.as_os_str().as_encoded_bytes());
    let key: String = key[..4].iter().map(|b| format!("{b:02x}")).collect();
    public.join(format!("eclipse-launcher-{key}"))
}

/// `GetShortPathNameW`: `path`'s short (8.3) name, or None on error. A folder with no short
/// name (the volume may keep none) answers with its long name again, which [`ascii`] refuses.
#[cfg(windows)]
fn short_path(path: &Path) -> Option<PathBuf> {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    #[link(name = "kernel32")]
    extern "system" {
        fn GetShortPathNameW(long: *const u16, short: *mut u16, len: u32) -> u32;
    }
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    let need = unsafe { GetShortPathNameW(wide.as_ptr(), std::ptr::null_mut(), 0) } as usize;
    if need == 0 {
        return None;
    }
    let mut buf = vec![0u16; need];
    let got = unsafe { GetShortPathNameW(wide.as_ptr(), buf.as_mut_ptr(), need as u32) } as usize;
    (got != 0 && got < need).then(|| PathBuf::from(std::ffi::OsString::from_wide(&buf[..got])))
}

pub(super) fn versions_dir() -> PathBuf {
    data_dir().join("versions")
}

pub fn version_dir(tag: &str) -> PathBuf {
    versions_dir().join(tag)
}

/// Where the game keeps saves and options.txt: `ECLIPSE_HOME`, else `eclipse` in the platform's
/// data folder (~/.local/share/eclipse on Linux, as the release folders' eclipse.sh uses). It is
/// never inside a version folder, so installing, updating or removing a version keeps the saves.
pub fn game_home() -> PathBuf {
    root("ECLIPSE_HOME", |roots| roots.game, "eclipse")
}
