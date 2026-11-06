use super::net::get_bytes;
use super::paths::data_dir;
use super::settings::Settings;
use super::versions::read_json;
use crate::{account, mojang, sources};
use std::path::{Path, PathBuf};

/// The Minecraft folders whose asset store can hold what a prepared version lacks: the one
/// chosen in the launcher, the official launcher's (or ECLIPSE_MINECRAFT_DIR), the folder
/// other launchers' files are linked into and Eclipse's own download, then every launcher's
/// default folder.
pub(super) fn minecraft_folders() -> Vec<PathBuf> {
    let mut folders: Vec<PathBuf> = Settings::load().minecraft_dir.into_iter().collect();
    folders.extend([
        eclipse_prepare::artifacts::minecraft_dir(None),
        data_dir().join("linked-minecraft"),
        data_dir().join("minecraft"),
    ]);
    folders.extend(
        sources::Folders::current()
            .map(|f| sources::known_folders(sources::Os::current(), &f))
            .unwrap_or_default()
            .into_iter()
            .map(|(_, dir)| dir),
    );
    let mut found: Vec<PathBuf> = Vec::new();
    for folder in folders {
        if folder.is_dir() && !found.contains(&folder) {
            found.push(folder);
        }
    }
    found
}

/// The official launcher's game directory, else the first other launcher's found
/// (sources.rs); preparing looks in all of them anyway.
pub fn default_minecraft_dir() -> Option<PathBuf> {
    let folders = sources::Folders::current()?;
    sources::known_folders(sources::Os::current(), &folders)
        .into_iter()
        .map(|(_, dir)| dir)
        .find(|dir| dir.is_dir())
}

/// Downloads the pinned version from Mojang into `dir` when an account in the launcher owns
/// Minecraft (asked again now), else says what signing in would do, as the last line of the
/// not-found error.
pub(super) fn download_for_owner(
    pins: &sources::Pins,
    dir: &Path,
    fetch: sources::Fetch,
    log: &dyn Fn(String),
) -> Result<(), String> {
    let mut accounts = account::Accounts::load();
    // The strict gate, never `launch_for_game`: the developer option that lets offline names
    // play without an owner must not download the game for them.
    let pass = account::launch(
        &account::Endpoints::live(),
        account::client_id().as_deref(),
        account::store(),
        &mut accounts,
    );
    if let Err(e) = accounts.save() {
        log(format!("Eclipse: the accounts are not saved: {e}"));
    }
    download_for(pins, dir, pass, !accounts.accounts.is_empty(), fetch, log)
}

/// Downloads Minecraft `id` (any version on Mojang's list, its version file's sha1 from it) into
/// Eclipse's own game folder, in the official launcher's layout, for an account that owns the
/// game (asked again now). Returns that folder.
pub fn download_version_for_owner(
    id: &str,
    manifest_sha1: &str,
    log: &dyn Fn(String),
) -> Result<PathBuf, String> {
    let dir = data_dir().join("minecraft");
    let pins = sources::Pins::for_version(id, manifest_sha1);
    download_for_owner(&pins, &dir, &get_bytes, log)
        .map_err(|e| e.trim_start_matches("\n  • ").to_string())?;
    Ok(dir)
}

/// What [`download_for_owner`] does with the owner gate's answer: Mojang's files only for its
/// `owner`, an account the services said owns the game just now.
pub(super) fn download_for(
    pins: &sources::Pins,
    dir: &Path,
    pass: Result<account::Launch, account::NoOwner>,
    signed_in: bool,
    fetch: sources::Fetch,
    log: &dyn Fn(String),
) -> Result<(), String> {
    let v = &pins.version;
    match pass {
        Ok(account::Launch {
            owner: Some((owner, _)),
            ..
        }) => {
            let name = owner.name;
            log(format!(
                "{name} owns Minecraft: downloading {v} from Mojang"
            ));
            let os = match sources::Os::current() {
                sources::Os::Linux => "linux",
                sources::Os::MacOs => "osx",
                sources::Os::Windows => "windows",
            };
            mojang::download(pins, dir, os, 8, fetch, log).map_err(|e| {
                format!("\n  • Eclipse tried to download {v} from Mojang for {name}, but: {e}")
            })
        }
        // An owner by its last answer, who can't be asked now.
        Ok(pass) => Err(format!(
            "\n  • Downloading {v} from Mojang needs your Microsoft account, which could not be \
             checked: {}",
            pass.offline.unwrap_or_default()
        )),
        Err(_) if signed_in => Err(
            "\n  • No signed-in account owns Minecraft: Java Edition, so Eclipse can't download it."
                .into(),
        ),
        Err(_) if account::client_id().is_some() => Err(format!(
            "\n  • Or sign in with the Microsoft account that owns Minecraft: Eclipse then \
             downloads {v} from Mojang itself."
        )),
        Err(_) => Err(format!(
            "\n  • Downloading {v} from Mojang needs a Microsoft account. {}",
            account::NOT_CONFIGURED
        )),
    }
}

/// The Java home for this version: a runtime for precompiled exporters, a JDK for older
/// preparers or changed sources. Forward the selection to external preparers too.
/// Else try the official launcher's Java in `games`, then download the runtime named by
/// `manifest` into Eclipse's game folder (Mojang's runtimes are whole JDKs).
pub(super) fn java(
    major: i64,
    compiler: bool,
    games: &[PathBuf],
    manifest: &Path,
    log: &dyn Fn(String),
) -> Result<PathBuf, String> {
    let find = if compiler {
        eclipse_prepare::java::find
    } else {
        eclipse_prepare::java::find_runtime
    };
    if let Ok(jdk) = find(major, &[]) {
        return Ok(jdk.home());
    }
    let mut runtimes: Vec<PathBuf> = Vec::new();
    for home in games
        .iter()
        .flat_map(|game| eclipse_prepare::java::minecraft_runtimes(&game.join("runtime")))
    {
        if !runtimes.contains(&home) {
            runtimes.push(home);
        }
    }
    let missing = match find(major, &runtimes) {
        Ok(jdk) => return Ok(jdk.home()),
        Err(e) => e,
    };
    let component = read_json(manifest)
        .and_then(|json| json["javaVersion"]["component"].as_str().map(String::from))
        .ok_or_else(|| missing.clone())?;
    // Mojang builds its Linux Java for glibc: without glibc's loader (NixOS without nix-ld,
    // musl distributions) it cannot start, so don't download it.
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) && !Path::new(GLIBC_LOADER).exists() {
        return Err(format!(
            "{missing}\n  • Minecraft's own Java from Mojang needs glibc ({GLIBC_LOADER} is missing; on NixOS, enable programs.nix-ld), so install compatible Java instead."
        ));
    }
    let runtime = data_dir().join("minecraft").join("runtime");
    let home = mojang::java_runtime(&component, &runtime, 8, &get_bytes, log)
        .map_err(|e| {
        format!("{missing}\n  • Eclipse also tried to download Minecraft's own Java from Mojang, but: {e}")
    })?;
    find(major, &[home]).map(|jdk| jdk.home())
}

/// The dynamic loader glibc programs on x86-64 Linux start through.
pub(crate) const GLIBC_LOADER: &str = "/lib64/ld-linux-x86-64.so.2";

/// The Minecraft folder to prepare from: the one given, the one chosen in the launcher, or the
/// official launcher's default. It may be absent when imported files can be reused.
pub fn minecraft_dir(requested: Option<PathBuf>) -> Result<PathBuf, String> {
    Ok(requested
        .or(Settings::load().minecraft_dir)
        .or_else(default_minecraft_dir)
        .unwrap_or_else(|| eclipse_prepare::artifacts::minecraft_dir(None)))
}
