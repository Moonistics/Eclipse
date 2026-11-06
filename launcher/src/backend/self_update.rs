use super::paths::{data_dir, plain};
use super::platform::{LAUNCHER_APPIMAGE_ASSET, LAUNCHER_ASSET, LAUNCHER_BINARY};
use super::releases::{compare_versions, launcher_version, Release};
use super::versions::{download, signed_digest, unpack};
use super::{REPO, VERSION};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The launcher's own path, read once at start: after a self-update the running file is gone.
pub fn exe_path() -> &'static Path {
    static EXE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    EXE.get_or_init(|| std::env::current_exe().unwrap_or_else(|_| PathBuf::from(LAUNCHER_BINARY)))
}

/// The AppImage this launcher runs from, when it does: the runtime sets `$APPIMAGE` to the file it
/// mounted and `$APPDIR` to the mount, while [`exe_path`] then names a copy inside that read-only
/// mount, gone with it. Updating and restarting work on this file, and the menu entry, shortcuts and
/// agent configurations start it.
pub fn appimage_path() -> Option<PathBuf> {
    appimage_path_in(
        std::env::var_os("APPIMAGE").as_deref(),
        std::env::var_os("APPDIR").as_deref(),
        exe_path(),
    )
}

/// `$APPIMAGE`, if it names a file still there and this binary runs from inside `$APPDIR`. Both
/// variables reach every child of an AppImage: a launcher a different AppImage's program started
/// must never take that program's file for its own (an update would overwrite it).
pub(super) fn appimage_path_in(
    appimage: Option<&std::ffi::OsStr>,
    appdir: Option<&std::ffi::OsStr>,
    exe: &Path,
) -> Option<PathBuf> {
    let appdir = Path::new(appdir?);
    if !appdir.is_absolute() || !exe.starts_with(appdir) {
        return None;
    }
    appimage
        .map(PathBuf::from)
        .filter(|path| path.is_absolute() && path.is_file())
}

/// The file a self-update replaces and a restart starts: the AppImage when the launcher runs from
/// one, the binary this process started from otherwise.
pub fn update_target() -> PathBuf {
    appimage_path().unwrap_or_else(|| exe_path().to_path_buf())
}

/// The launcher archive this build installs from: the AppImage's when it runs from one (a `.tar.gz`
/// binary would be written inside the read-only mount), this platform's otherwise.
pub fn launcher_asset() -> Option<&'static str> {
    launcher_asset_running_from(appimage_path().is_some())
}

pub(super) fn launcher_asset_running_from(appimage: bool) -> Option<&'static str> {
    if appimage {
        LAUNCHER_APPIMAGE_ASSET
    } else {
        LAUNCHER_ASSET
    }
}

/// Development builds and `ECLIPSE_LAUNCHER_NO_SELF_UPDATE` never replace themselves.
pub fn self_update_allowed() -> bool {
    !cfg!(debug_assertions) && std::env::var_os("ECLIPSE_LAUNCHER_NO_SELF_UPDATE").is_none()
}

/// The version in `eclipse-launcher --version`'s output (`eclipse-launcher 0.1.5`).
pub(super) fn parse_version_output(out: &str) -> Option<&str> {
    out.trim()
        .strip_prefix("eclipse-launcher ")
        .map(str::trim)
        .filter(|v| !v.is_empty())
}

/// The version of the launcher binary now at `path`, which is not this process's once another
/// update (the command line, another window) has replaced the file.
pub(crate) fn installed_launcher_version(path: &Path) -> Option<String> {
    let mut cmd = crate::process::command(path);
    cmd.arg("--version")
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    let out = cmd.output().ok()?;
    parse_version_output(&String::from_utf8_lossy(&out.stdout)).map(String::from)
}

/// Puts `new` at `exe`, the path this launcher started from. The running process keeps its
/// file (unlinked) and works on. Unix only: `self_replace` asks the system where the running
/// binary is, and Linux answers "<path> (deleted)" once the file has been replaced, so a second
/// update in the same run failed with "No such file or directory".
#[cfg(unix)]
pub(super) fn replace_binary(new: &Path, exe: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let dir = exe.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "the launcher has no folder")
    })?;
    let name = exe.file_name().unwrap_or_default().to_string_lossy();
    let temp = dir.join(format!(".{name}.update-{}", std::process::id()));
    let mode = fs::metadata(exe).map_or(0o755, |m| m.permissions().mode() | 0o111);
    let result = fs::copy(new, &temp)
        .and_then(|_| fs::set_permissions(&temp, fs::Permissions::from_mode(mode)))
        .and_then(|_| fs::rename(&temp, exe));
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// Windows keeps a running binary from being written or deleted but lets it be renamed: the
/// running one steps aside to `<name>.old-<pid>-<n>` in its folder, the new one takes its path,
/// and a later start removes the old file (`remove_replaced_binaries`). The `self_replace` crate
/// did this by copying the launcher under a random hidden name and starting that copy to delete
/// the original through a hidden `cmd.exe`, which Microsoft Defender quarantines as
/// Trojan:Script/Wacatac (#3175).
#[cfg(not(unix))]
pub(super) fn replace_binary(new: &Path, exe: &Path) -> std::io::Result<()> {
    use std::sync::atomic::{AtomicU32, Ordering};
    // A second update in the same run moves the first one's binary aside too.
    static MOVED: AtomicU32 = AtomicU32::new(0);
    let dir = exe.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "the launcher has no folder")
    })?;
    let name = exe.file_name().unwrap_or_default().to_string_lossy();
    let pid = std::process::id();
    let temp = dir.join(format!("{name}.update-{pid}"));
    let old = dir.join(format!(
        "{name}.old-{pid}-{}",
        MOVED.fetch_add(1, Ordering::Relaxed)
    ));
    if let Err(e) = fs::copy(new, &temp) {
        let _ = fs::remove_file(&temp);
        return Err(e);
    }
    match fs::rename(exe, &old) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            let _ = fs::remove_file(&temp);
            return Err(e);
        }
        _ => {}
    }
    if let Err(e) = fs::rename(&temp, exe) {
        let _ = fs::rename(&old, exe);
        let _ = fs::remove_file(&temp);
        return Err(e);
    }
    Ok(())
}

/// Deletes the binaries earlier self-updates moved aside (`replace_binary`), and the copies of an
/// update that stopped before its new binary took its place (`<name>.update-<pid>`). The one the
/// launcher just restarted from is still exiting, so this keeps trying for a few seconds in the
/// background; one that stays in use (another launcher window) goes on a later start.
#[cfg(windows)]
pub fn remove_replaced_binaries() {
    let exe = exe_path();
    let (Some(dir), Some(name)) = (exe.parent(), exe.file_name()) else {
        return;
    };
    let dir = dir.to_path_buf();
    let name = name.to_string_lossy();
    let old = format!("{name}.old-");
    let update = format!("{name}.update-");
    let own = format!("{update}{}", std::process::id());
    std::thread::spawn(move || {
        for _ in 0..10 {
            let Ok(entries) = fs::read_dir(&dir) else {
                return;
            };
            let mut left = false;
            for entry in entries.flatten() {
                let file = entry.file_name();
                let file = file.to_string_lossy();
                if file.starts_with(&old) || (file.starts_with(&update) && file != own) {
                    left |= fs::remove_file(entry.path()).is_err();
                }
            }
            if !left {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    });
}

/// The app bundle the launcher runs from (`…/Eclipse.app` for `…/Eclipse.app/Contents/MacOS/
/// eclipse-launcher`), or None for a bare binary, as launchers before the .app shipped.
pub fn app_bundle(exe: &Path) -> Option<&Path> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let app = contents.parent()?;
    (macos.file_name()? == "MacOS"
        && contents.file_name()? == "Contents"
        && app.extension()? == "app")
        .then_some(app)
}

pub(super) fn find_app(dir: &Path) -> Option<PathBuf> {
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.extension().is_some_and(|e| e == "app") {
                return Some(path);
            }
            if let Some(found) = find_app(&path) {
                return Some(found);
            }
        }
    }
    None
}

/// Puts a verified app bundle in place, retaining old bundles until no process uses them.
#[cfg(target_os = "macos")]
pub(super) fn replace_bundle(new: &Path, app: &Path) -> Result<(), String> {
    crate::mac_update::replace_bundle(new, app)
}

/// Retired macOS launchers are removed only once their executable is no longer in use.
/// Run for every entry point (GUI, CLI and MCP), before a command can return or exec.
pub fn cleanup_launcher_updates() {
    #[cfg(target_os = "macos")]
    if let Some(app) = app_bundle(exe_path()) {
        crate::mac_update::cleanup(app);
    }
}

#[cfg(not(target_os = "macos"))]
pub(super) fn replace_bundle(_new: &Path, _app: &Path) -> Result<(), String> {
    Err("App bundles are a macOS thing.".into())
}

/// A launcher release downloaded and checked against its signed SHA256SUMS, waiting to take this
/// launcher's place: the new AppImage, `.app` bundle or binary, in a stage folder of its own inside
/// [`update_work_dir`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Staged {
    pub tag: String,
    pub payload: PathBuf,
    /// The signed SHA-256 of the archive beside it, checked again just before it goes in
    /// ([`place_checked`]).
    pub sha256: String,
}

/// Where launcher updates are downloaded and unpacked before one takes this launcher's place. Each
/// has its own folder in here, `<pid>-<nonce>`, named after the process that made it, because a
/// stage waits there until its window closes and other launchers stage updates meanwhile.
pub fn update_work_dir() -> PathBuf {
    data_dir().join(".launcher-update")
}

/// How long a stage whose process has gone is kept: the installer may still be working from it.
/// The window marks it when it hands it over ([`mark_handed_over`]), so this counts from there.
pub(super) const STAGE_GRACE: std::time::Duration = std::time::Duration::from_secs(60 * 60);
pub(super) const HANDED_OVER: &str = "handed-over";
const UNPACKED: &str = "unpacked";
/// The archive's name in a stage says how to open it, in the installer too.
static ARCHIVE_ENDINGS: [&str; 3] = [".tar.gz", ".AppImage", ".zip"];

fn archive_name(url: &str) -> String {
    let ending = ARCHIVE_ENDINGS
        .iter()
        .find(|ending| url.ends_with(**ending))
        .unwrap_or(&".zip");
    format!("download{ending}")
}

/// The process that made the stage folder called `name`; None for any other name.
fn stage_owner(name: &str) -> Option<u32> {
    let (pid, nonce) = name.split_once('-')?;
    if nonce.len() != 32 || !nonce.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    pid.parse().ok()
}

pub(crate) fn new_stage_folder(work: &Path) -> Result<PathBuf, String> {
    let mut random = [0u8; 16];
    getrandom::fill(&mut random).map_err(|e| e.to_string())?;
    let nonce: String = random.iter().map(|b| format!("{b:02x}")).collect();
    let stage = work.join(format!("{}-{nonce}", std::process::id()));
    fs::create_dir_all(work).map_err(|e| e.to_string())?;
    fs::create_dir(&stage).map_err(|e| e.to_string())?;
    Ok(stage)
}

/// The stage folder directly under `work` that holds `payload`. None for a path anywhere else,
/// which is not an update a launcher staged, and which nothing may delete or install.
pub fn stage_folder(work: &Path, payload: &Path) -> Option<PathBuf> {
    use std::path::Component;
    let mut inside = payload.strip_prefix(work).ok()?.components();
    let Component::Normal(name) = inside.next()? else {
        return None;
    };
    stage_owner(name.to_str()?)?;
    let rest: Vec<_> = inside.collect();
    (!rest.is_empty() && rest.iter().all(|c| matches!(c, Component::Normal(_))))
        .then(|| work.join(name))
}

/// Removes the stage holding `payload`, and only that one.
pub fn remove_stage(work: &Path, payload: &Path) {
    if let Some(stage) = stage_folder(work, payload) {
        let _ = fs::remove_dir_all(stage);
    }
}

/// The window is about to start the installer for `staged`: from here its folder is in use by a
/// process no other launcher can tell from its name.
pub fn mark_handed_over(staged: &Staged) {
    if let Some(stage) = stage_folder(&update_work_dir(), &staged.payload) {
        let _ = fs::write(stage.join(HANDED_OVER), b"");
    }
}

/// Removes what a crash or a killed window left behind: stages whose process has gone and that
/// nothing touched for [`STAGE_GRACE`], and the files launchers before stage folders kept directly
/// in `work`. A stage whose process is alive stays however old it is; when its number has been
/// reused, it only stays longer.
pub(super) fn remove_abandoned_stages(
    work: &Path,
    now: std::time::SystemTime,
    alive: impl Fn(u32) -> bool,
) {
    let Ok(entries) = fs::read_dir(work) else {
        return;
    };
    for entry in entries.flatten() {
        if entry
            .file_name()
            .to_str()
            .and_then(stage_owner)
            .is_some_and(&alive)
        {
            continue;
        }
        let path = entry.path();
        let touched = [path.clone(), path.join(HANDED_OVER)]
            .iter()
            .filter_map(|p| fs::metadata(p).and_then(|m| m.modified()).ok())
            .max();
        let abandoned =
            touched.is_some_and(|t| now.duration_since(t).is_ok_and(|age| age >= STAGE_GRACE));
        if abandoned {
            let _ = match entry.file_type() {
                Ok(kind) if kind.is_dir() => fs::remove_dir_all(&path),
                _ => fs::remove_file(&path),
            };
        }
    }
}

/// Whether process `pid` is running. One this user may not look at counts as running.
#[cfg(unix)]
pub(super) fn process_alive(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    // SAFETY: signal 0 sends nothing, it only asks whether the process is there.
    pid > 0
        && (unsafe { libc::kill(pid, 0) } == 0
            || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM))
}

/// Whether process `pid` is running. One this user may not look at counts as running.
#[cfg(windows)]
pub(super) fn process_alive(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, ERROR_ACCESS_DENIED, STILL_ACTIVE,
    };
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    // SAFETY: the handle is checked before use and closed once; `code` outlives the call.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return GetLastError() == ERROR_ACCESS_DENIED;
        }
        let mut code = 0u32;
        // An exited process stays openable while something holds a handle to it.
        let running = GetExitCodeProcess(handle, &mut code) != 0 && code as i32 == STILL_ACTIVE;
        CloseHandle(handle);
        running
    }
}

fn file_sha256(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hash = eclipse_release_sign::Sha256::default();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            return Ok(hash.hex());
        }
        hash.update(&buf[..n]);
    }
}

/// Checks the stage's archive against the signed SHA-256 again, then hands `place` the launcher
/// from those same bytes: an AppImage is the archive, anything else is unpacked afresh, whatever
/// has happened to the stage since it was downloaded. Nothing is placed when the bytes differ.
pub fn place_checked(
    work: &Path,
    staged: &Staged,
    place: impl FnOnce(&Staged) -> Result<(), String>,
) -> Result<(), String> {
    let not_staged = || {
        format!(
            "{} is not a launcher update this launcher downloaded.",
            staged.payload.display()
        )
    };
    let stage = stage_folder(work, &staged.payload).ok_or_else(not_staged)?;
    let archive = ARCHIVE_ENDINGS
        .iter()
        .map(|ending| stage.join(format!("download{ending}")))
        .find(|path| path.is_file())
        .ok_or_else(not_staged)?;
    eclipse_release_sign::check(&staged.tag, &staged.sha256, &file_sha256(&archive)?)?;
    let payload = if staged.payload == archive {
        archive
    } else {
        let inside = staged
            .payload
            .strip_prefix(stage.join(UNPACKED))
            .map_err(|_| not_staged())?;
        let fresh = stage.join("checked");
        let _ = fs::remove_dir_all(&fresh);
        unpack(&archive, &archive.to_string_lossy(), &fresh)?;
        let payload = fresh.join(inside);
        if !payload.exists() {
            return Err("The download has no launcher binary.".into());
        }
        payload
    };
    place(&Staged {
        payload,
        ..staged.clone()
    })
}

/// Whether `installed`, the launcher in place, is already the release `tag` or a newer one.
pub(crate) fn already_in_place(installed: Option<&str>, tag: &str) -> bool {
    match (installed, launcher_version(tag)) {
        (Some(installed), Some(target)) => compare_versions(installed, target).is_ge(),
        _ => false,
    }
}

/// Downloads a launcher release and puts it in place of this one. The running process keeps
/// working; `restart` starts the new one. When the file there already is that release or
/// a newer one (updated from the command line or another window), there is nothing to do.
pub fn self_update(release: &Release, progress: impl Fn(f32)) -> Result<(), String> {
    if let Some(staged) = stage_update(release, &progress)? {
        let work = update_work_dir();
        let placed = place_checked(&work, &staged, place_staged);
        remove_stage(&work, &staged.payload);
        placed?;
    }
    progress(1.0);
    Ok(())
}

/// Downloads and unpacks a launcher release next to this one without putting it in place yet
/// ([`place_checked`] does, in this process or in the installer `update_handoff` starts), in a new
/// stage folder that no other launcher writes to. None when the file there already is that release
/// or a newer one.
pub fn stage_update(release: &Release, progress: impl Fn(f32)) -> Result<Option<Staged>, String> {
    let (url, size) = release
        .asset
        .clone()
        .ok_or("This launcher release has no build for this platform.")?;
    let target = launcher_version(&release.tag).unwrap_or("0");
    if installed_launcher_version(&update_target())
        .is_some_and(|v| v != VERSION && compare_versions(&v, target).is_ge())
    {
        return Ok(None);
    }
    let sha256 = signed_digest(&release.tag, launcher_asset().unwrap_or_default())?;
    let work = update_work_dir();
    remove_abandoned_stages(&work, std::time::SystemTime::now(), process_alive);
    let stage = new_stage_folder(&work)?;
    let staged = fill_stage(&stage, &release.tag, &url, size, &sha256, &progress);
    if staged.is_err() {
        let _ = fs::remove_dir_all(&stage);
    }
    staged.map(Some)
}

fn fill_stage(
    stage: &Path,
    tag: &str,
    url: &str,
    size: u64,
    sha256: &str,
    progress: &impl Fn(f32),
) -> Result<Staged, String> {
    let archive = stage.join(archive_name(url));
    download(url, size, sha256, &archive, progress)?;
    let staged = |payload| {
        Ok(Staged {
            tag: tag.to_owned(),
            payload,
            sha256: sha256.to_owned(),
        })
    };
    // An AppImage is the whole program: the new one takes the place of the file `$APPIMAGE` names
    // (the running mount keeps its copy until `restart` starts it). Its archive is not a `.tar.gz`
    // to unpack, so this comes before the unpacking below.
    if appimage_path().is_some() {
        if !url.ends_with(".AppImage") {
            return Err(format!(
                "{tag} is not an AppImage, and this launcher runs from one. Install it from its release page instead."
            ));
        }
        return staged(archive);
    }
    let unpacked = stage.join(UNPACKED);
    unpack(&archive, url, &unpacked)?;
    // A launcher inside Eclipse.app replaces the whole bundle (its Info.plist, icon and
    // signature belong with the binary); a bare binary, as older launchers are, only the binary,
    // which the macOS archive also holds (Eclipse.app/Contents/MacOS/eclipse-launcher).
    if app_bundle(exe_path()).is_some() {
        if let Some(new) = find_app(&unpacked) {
            return staged(new);
        }
    }
    let binary =
        find_file(&unpacked, LAUNCHER_BINARY).ok_or("The download has no launcher binary.")?;
    staged(binary)
}

/// Puts a [`stage_update`]d launcher where this one runs from: the AppImage, the `.app` bundle
/// or the binary. [`place_checked`] calls this with the bytes it has just checked.
pub fn place_staged(staged: &Staged) -> Result<(), String> {
    let new = &staged.payload;
    if let Some(appimage) = appimage_path() {
        replace_binary(new, &appimage).map_err(|e| {
            format!(
                "Could not replace the AppImage at {}: {e}",
                appimage.display()
            )
        })?;
        return Ok(());
    }
    match app_bundle(exe_path()) {
        Some(app) if new.extension().is_some_and(|e| e == "app") => replace_bundle(new, app)?,
        _ => replace_binary(new, exe_path()).map_err(|e| {
            format!(
                "Could not replace the launcher at {}: {e}",
                exe_path().display()
            )
        })?,
    }
    // An installed launcher's Uninstall entry names the new version (installer.rs).
    #[cfg(windows)]
    crate::installer::after_update(launcher_version(&staged.tag).unwrap_or(VERSION));
    Ok(())
}

/// The website's install page, where a launcher that can't update itself sends the player; the
/// launcher releases without a website.
pub fn download_page(website: &str) -> String {
    match website.trim_end_matches('/') {
        "" => format!("https://github.com/{REPO}/releases?q=launcher"),
        site => format!("{site}/docs/install"),
    }
}

/// Whether this copy of the launcher can't put a new one in its place: its folder is read-only (a
/// disk image, or the read-only copy macOS runs a downloaded app from until it is moved, its "App
/// Translocation"), or on Windows it runs from the temporary folder, where Explorer and archive
/// tools run a program opened inside a `.zip` (and delete it later). Such a copy is told to download
/// the launcher again (#9287). Elsewhere a launcher unpacked in /tmp replaces itself fine.
pub fn cannot_replace_itself() -> bool {
    let target = update_target();
    let place = app_bundle(&target).unwrap_or(&target);
    place.parent().is_none_or(|dir| {
        !writable(dir) || (cfg!(windows) && in_temp_folder(dir, &std::env::temp_dir()))
    })
}

/// Whether a package manager owns this launcher: the update target is classified, rather than
/// the executable inside an AppImage's mount. Asked while the sidebar is drawn, and the launcher
/// doesn't move while it runs, so the path is resolved once.
pub fn package_managed_launcher() -> bool {
    static MANAGED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *MANAGED.get_or_init(|| {
        let target = update_target();
        let target = target.canonicalize().unwrap_or(target);
        package_managed(&target)
    })
}

/// Package-owned files must be replaced by their package manager, not a downloaded archive.
/// `/usr/local` is where an administrator copies programs by hand: no package manager owns it.
pub(super) fn package_managed(target: &Path) -> bool {
    ["/usr", "/nix/store", "/app"]
        .iter()
        .any(|root| target.starts_with(root))
        && !target.starts_with("/usr/local")
}

pub(super) fn writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".eclipse-write-check-{}", std::process::id()));
    let made = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .is_ok();
    let _ = fs::remove_file(&probe);
    made
}

/// `dir` is the temporary folder or inside it, however either is spelled: %TEMP% can use an
/// account's short name (`C:\Users\USERNA~1\...`) while the launcher's own path has the long one.
pub(super) fn in_temp_folder(dir: &Path, temp: &Path) -> bool {
    let resolved = |path: &Path| plain(path.canonicalize().unwrap_or_else(|_| path.to_path_buf()));
    resolved(dir).starts_with(resolved(temp))
}

fn find_file(dir: &Path, name: &str) -> Option<PathBuf> {
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file(&path, name) {
                return Some(found);
            }
        } else if entry.file_name() == name {
            return Some(path);
        }
    }
    None
}

/// Starts the launcher again (the updated binary, or the replaced AppImage) with the same
/// arguments in place of this one: on Unix the process becomes the new binary (exec), on Windows
/// the new one starts and this one exits. Returns only when the new one could not start.
pub fn restart() -> String {
    let target = update_target();
    let mut cmd = Command::new(&target);
    cmd.args(std::env::args_os().skip(1));
    #[cfg(unix)]
    let error = {
        use std::os::unix::process::CommandExt;
        cmd.exec()
    };
    #[cfg(not(unix))]
    let error = match cmd.spawn() {
        Ok(_) => std::process::exit(0),
        Err(e) => e,
    };
    format!(
        "Could not start {}: {error}. Close the launcher and open it again.",
        target.display()
    )
}
