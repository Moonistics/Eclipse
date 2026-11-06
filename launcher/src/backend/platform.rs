/// The release assets of each system Eclipse ships for: (OS, architecture, the game, the launcher).
/// game-build.yml builds the game's, nightly-build.yml signs and publishes them, and launcher.yml
/// builds the launcher's (`platform_assets_are_published`).
pub const PLATFORMS: [(&str, &str, &str, &str); 5] = [
    (
        "linux",
        "x86_64",
        "eclipse-linux-x86_64.tar.gz",
        "eclipse-launcher-linux-x86_64.tar.gz",
    ),
    (
        "macos",
        "aarch64",
        "eclipse-macos-arm64.zip",
        "eclipse-launcher-macos-arm64.zip",
    ),
    // Intel Macs (#1161).
    (
        "macos",
        "x86_64",
        "eclipse-macos-x64.zip",
        "eclipse-launcher-macos-x64.zip",
    ),
    (
        "windows",
        "x86_64",
        "eclipse-windows-x64.zip",
        "eclipse-launcher-windows-x64.zip",
    ),
    (
        "windows",
        "aarch64",
        "eclipse-windows-arm64.zip",
        "eclipse-launcher-windows-arm64.zip",
    ),
];

pub(super) const fn same(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// This system's entry of `PLATFORMS`: the game's asset and the launcher's.
pub(super) const fn platform_assets(os: &str, arch: &str) -> Option<(&'static str, &'static str)> {
    let mut i = 0;
    while i < PLATFORMS.len() {
        let (o, a, game, launcher) = PLATFORMS[i];
        if same(o, os) && same(a, arch) {
            return Some((game, launcher));
        }
        i += 1;
    }
    None
}

/// Both follow this launcher's own architecture. An x64 launcher on a Windows ARM64 PC (under
/// emulation) keeps updating itself, installing games and building native mods as x64, so a player
/// whose emulated setup works is never moved to an ARM64 build by an update. The ARM64 launcher is
/// what `eclipse-launcher-setup-arm64.exe` installs.
const OWN_ASSETS: Option<(&str, &str)> =
    platform_assets(std::env::consts::OS, std::env::consts::ARCH);
/// The game build this launcher installs, None where Eclipse has none.
pub const ASSET: Option<&str> = match OWN_ASSETS {
    Some((game, _)) => Some(game),
    None => None,
};
/// The launcher build this launcher updates itself from.
pub const LAUNCHER_ASSET: Option<&str> = match OWN_ASSETS {
    Some((_, launcher)) => Some(launcher),
    None => None,
};
/// The game build the ARM64 Windows launcher installs from a release that has no ARM64 one (every
/// release before it, or a Nightly whose ARM64 job failed): the x64 build, which Windows runs under
/// emulation.
pub(super) const GAME_FALLBACK: Option<&str> =
    game_fallback_for(std::env::consts::OS, std::env::consts::ARCH);

/// The asset a platform's launcher falls back to when a release has no build of its own.
pub(super) const fn game_fallback_for(os: &str, arch: &str) -> Option<&'static str> {
    if same(os, "windows") && same(arch, "aarch64") {
        match platform_assets("windows", "x86_64") {
            Some((game, _)) => Some(game),
            None => None,
        }
    } else {
        None
    }
}

/// The launcher as an AppImage (`packaging/linux/appimage.sh`): the program and its libraries in
/// one file. A launcher running from one updates by replacing that file ([`appimage_path`]), so it
/// needs this archive rather than the platform's `.tar.gz`.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub const LAUNCHER_APPIMAGE_ASSET: Option<&str> = Some("eclipse-launcher-linux-x86_64.AppImage");
#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
pub const LAUNCHER_APPIMAGE_ASSET: Option<&str> = None;

pub(super) const LAUNCHER_BINARY: &str = if cfg!(windows) {
    "eclipse-launcher.exe"
} else {
    "eclipse-launcher"
};

pub(crate) const CLIENT: &str = if cfg!(windows) {
    "eclipse-client.exe"
} else {
    "eclipse-client"
};
