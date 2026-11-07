use super::net::get_bytes;
use super::paths::{linked_minecraft_dir, own_minecraft_dir};
use super::settings::Settings;
use crate::{account, mojang, sources};
use std::path::{Path, PathBuf};

/// The Minecraft folders whose asset store can hold what a prepared version lacks: the one
/// chosen in the launcher, the official launcher's (or ECLIPSE_MINECRAFT_DIR), the folder
/// other launchers' files are linked into and Eclipse's own download, then every launcher's
/// default folder.
pub(super) fn minecraft_folders(chosen: Option<PathBuf>) -> Vec<PathBuf> {
    let mut folders: Vec<PathBuf> = chosen.into_iter().collect();
    folders.extend([
        eclipse_prepare::artifacts::minecraft_dir(None),
        linked_minecraft_dir(),
        own_minecraft_dir(),
    ]);
    folders.extend(
        sources::known_folders_here()
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
    sources::known_folders_here()
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
    progress: &dyn Fn(Option<(usize, usize)>),
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
    download_for(
        pins,
        dir,
        pass,
        !accounts.accounts.is_empty(),
        fetch,
        log,
        progress,
    )
}

/// Downloads Minecraft `id` (any version on Mojang's list, its version file's sha1 from it) into
/// Eclipse's own game folder, in the official launcher's layout, for an account that owns the
/// game (asked again now). Returns that folder.
pub(crate) fn download_version_for_owner(
    id: &str,
    manifest_sha1: &str,
    log: &dyn Fn(String),
) -> Result<PathBuf, String> {
    let dir = own_minecraft_dir();
    let pins = sources::Pins::for_version(id, manifest_sha1);
    download_for_owner(&pins, &dir, &get_bytes, log, &|_| {})?;
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
    progress: &dyn Fn(Option<(usize, usize)>),
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
            let os = crate::java_launch::os_name();
            mojang::download(pins, dir, os, mojang::FILES_AT_A_TIME, fetch, log, progress).map_err(
                |e| format!("Eclipse tried to download {v} from Mojang for {name}, but: {e}"),
            )
        }
        // An owner by its last answer, who can't be asked now.
        Ok(pass) => Err(format!(
            "Downloading {v} from Mojang needs your Microsoft account, which could not be \
             checked: {}",
            pass.offline.unwrap_or_default()
        )),
        Err(_) if signed_in => Err(
            "No signed-in account owns Minecraft: Java Edition, so Eclipse can't download it."
                .into(),
        ),
        Err(_) if account::client_id().is_some() => Err(format!(
            "Or sign in with the Microsoft account that owns Minecraft: Eclipse then \
             downloads {v} from Mojang itself."
        )),
        Err(_) => Err(format!(
            "Downloading {v} from Mojang needs a Microsoft account. {}",
            account::NOT_CONFIGURED
        )),
    }
}

/// The Minecraft folder to prepare from: the one given, the one chosen in the launcher, or the
/// official launcher's default. It may be absent when imported files can be reused.
pub fn minecraft_dir(requested: Option<PathBuf>, settings: &Settings) -> Result<PathBuf, String> {
    Ok(requested
        .or_else(|| settings.minecraft_dir.clone())
        .or_else(default_minecraft_dir)
        .unwrap_or_else(|| eclipse_prepare::artifacts::minecraft_dir(None)))
}
