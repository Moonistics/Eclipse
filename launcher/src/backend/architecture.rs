use super::paths::version_dir;
use super::platform::{
    game_fallback_for, pe_machine, platform_assets, ASSET, CLIENT, GAME_FALLBACK, PE_ARM64, PE_X64,
    PLATFORMS,
};
use super::releases::Release;
use super::settings::Settings;
use std::fs;
use std::path::Path;

/// The game architecture selected in Settings. Auto preserves the launcher's existing behavior:
/// it follows the process architecture, even when an x64 launcher runs on Windows ARM.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GameArchitecture {
    #[default]
    Auto,
    X64,
    Arm64,
}

impl GameArchitecture {
    pub fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::X64 => "x64",
            Self::Arm64 => "arm64",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" | "default" => Some(Self::Auto),
            "x64" | "x86_64" | "amd64" => Some(Self::X64),
            "arm64" | "aarch64" => Some(Self::Arm64),
            _ => None,
        }
    }
}

pub(super) const GAME_BUILD_ARCHITECTURE: &str = ".eclipse-game-architecture.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct InstalledGameArchitecture {
    /// The architecture requested when this version was installed.
    pub(super) requested: GameArchitecture,
    /// The archive's actual architecture (x64 may be the safe ARM64 fallback).
    pub(super) binary: GameArchitecture,
}

fn architecture_for_platform(architecture: &str) -> Option<GameArchitecture> {
    match architecture {
        "x86_64" => Some(GameArchitecture::X64),
        "aarch64" => Some(GameArchitecture::Arm64),
        _ => None,
    }
}

pub(super) fn architecture_for_asset(asset: &str) -> Option<GameArchitecture> {
    let file = asset.rsplit('/').next()?;
    let (_, architecture, _, _) = PLATFORMS
        .iter()
        .find(|(os, _, game, _)| *os == "windows" && *game == file)?;
    architecture_for_platform(architecture)
}

/// Detect the native Windows machine even if this launcher process is running under emulation.
#[cfg(windows)]
pub(super) fn native_windows_architecture() -> Option<&'static str> {
    #[repr(C)]
    struct SystemInfo {
        processor_architecture: u16,
        _reserved: u16,
        _page_size: u32,
        _minimum_application_address: *mut std::ffi::c_void,
        _maximum_application_address: *mut std::ffi::c_void,
        _active_processor_mask: usize,
        _number_of_processors: u32,
        _processor_type: u32,
        _allocation_granularity: u32,
        _processor_level: u16,
        _processor_revision: u16,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
        fn GetModuleHandleA(module_name: *const std::ffi::c_char) -> *mut std::ffi::c_void;
        fn GetProcAddress(
            module: *mut std::ffi::c_void,
            name: *const std::ffi::c_char,
        ) -> Option<unsafe extern "system" fn()>;
        fn GetNativeSystemInfo(info: *mut SystemInfo);
    }

    type IsWow64Process2 =
        unsafe extern "system" fn(*mut std::ffi::c_void, *mut u16, *mut u16) -> i32;

    // IsWow64Process2 reports the ARM64 host for x64 emulation; GetNativeSystemInfo reports the
    // emulated processor for compatibility. Resolve it dynamically for older Windows versions.
    let kernel32 = unsafe { GetModuleHandleA(c"kernel32.dll".as_ptr()) };
    let is_wow64_process2 = if kernel32.is_null() {
        None
    } else {
        unsafe { GetProcAddress(kernel32, c"IsWow64Process2".as_ptr()) }
    };
    if let Some(function) = is_wow64_process2 {
        // SAFETY: GetProcAddress resolved this exact Kernel32 API, whose signature is declared
        // above with the documented system calling convention.
        let is_wow64_process2: IsWow64Process2 = unsafe { std::mem::transmute(function) };
        let (mut process_machine, mut native_machine) = (0u16, 0u16);
        // SAFETY: GetCurrentProcess returns a pseudo-handle accepted by IsWow64Process2, and both
        // output pointers refer to live USHORTs.
        if unsafe {
            is_wow64_process2(
                GetCurrentProcess(),
                &mut process_machine,
                &mut native_machine,
            )
        } != 0
        {
            return match native_machine {
                PE_X64 => Some("x86_64"),
                PE_ARM64 => Some("aarch64"),
                _ => None,
            };
        }
    }

    // SAFETY: GetNativeSystemInfo writes the documented SYSTEM_INFO layout into this repr(C)
    // buffer and has no failure return value. This fallback preserves existing behavior on older
    // Windows versions without IsWow64Process2.
    let mut info: SystemInfo = unsafe { std::mem::zeroed() };
    unsafe { GetNativeSystemInfo(&mut info) };
    match info.processor_architecture {
        9 => Some("x86_64"),   // PROCESSOR_ARCHITECTURE_AMD64
        12 => Some("aarch64"), // PROCESSOR_ARCHITECTURE_ARM64
        _ => None,
    }
}

#[cfg(not(windows))]
pub(super) fn native_windows_architecture() -> Option<&'static str> {
    None
}

pub(super) fn supports_game_architecture_choice(os: &str, native: Option<&str>) -> bool {
    os == "windows" && native == Some("aarch64")
}

/// Resolves the game's asset and fallback. Auto follows the process architecture, preserving
/// existing x64, Linux and macOS behavior. Manual choices are honored on native Windows ARM.
fn game_architecture_for<'a>(
    os: &str,
    native: Option<&str>,
    process: &'a str,
    choice: GameArchitecture,
) -> &'a str {
    if !supports_game_architecture_choice(os, native) {
        return process;
    }
    match choice {
        GameArchitecture::Auto => process,
        GameArchitecture::X64 => "x86_64",
        GameArchitecture::Arm64 => "aarch64",
    }
}

pub(super) fn game_assets_for(
    os: &str,
    native: Option<&str>,
    process: &str,
    choice: GameArchitecture,
) -> Option<(&'static str, Option<&'static str>)> {
    let arch = game_architecture_for(os, native, process, choice);
    let (asset, _) = platform_assets(os, arch)?;
    Some((asset, game_fallback_for(os, arch)))
}

/// The release asset the game installs from here, for the architecture `settings` pick, and the
/// one it takes from a release that has none of its own ([`GAME_FALLBACK`]).
pub(super) fn game_assets(settings: &Settings) -> (Option<&'static str>, Option<&'static str>) {
    let assets = game_assets_for(
        std::env::consts::OS,
        native_windows_architecture(),
        std::env::consts::ARCH,
        settings.game_architecture,
    );
    assets.map_or((ASSET, GAME_FALLBACK), |(asset, fallback)| {
        (Some(asset), fallback)
    })
}

pub(super) fn selected_game_architecture(settings: &Settings) -> Option<GameArchitecture> {
    if std::env::consts::OS != "windows" {
        return None;
    }
    architecture_for_platform(game_architecture_for(
        "windows",
        native_windows_architecture(),
        std::env::consts::ARCH,
        settings.game_architecture,
    ))
}

/// True for both native ARM64 launchers and x64 launchers under ARM64 emulation.
pub fn can_choose_game_architecture() -> bool {
    supports_game_architecture_choice("windows", native_windows_architecture())
}

fn chosen_game_architecture(settings: &Settings) -> Option<GameArchitecture> {
    can_choose_game_architecture()
        .then(|| selected_game_architecture(settings))
        .flatten()
}

/// A downloaded version records its requested and actual architectures. Old installs and imports
/// have no marker; their PE machine type is used for both.
fn installed_game_architecture(tag: &str) -> Option<InstalledGameArchitecture> {
    let dir = version_dir(tag);
    serde_json::from_slice(&fs::read(dir.join(GAME_BUILD_ARCHITECTURE)).ok()?)
        .ok()
        .or_else(|| {
            let binary = pe_architecture_file(&dir.join(CLIENT))?;
            Some(InstalledGameArchitecture {
                requested: binary,
                binary,
            })
        })
}

pub(super) fn pe_architecture_file(path: &Path) -> Option<GameArchitecture> {
    pe_machine(path).ok().and_then(architecture_for_pe_machine)
}

fn architecture_for_pe_machine(machine: u16) -> Option<GameArchitecture> {
    match machine {
        PE_X64 => Some(GameArchitecture::X64),
        PE_ARM64 => Some(GameArchitecture::Arm64),
        _ => None,
    }
}

pub(super) fn installed_architecture_matches(
    target: GameArchitecture,
    installed: InstalledGameArchitecture,
) -> bool {
    installed.requested == target || installed.binary == target
}

pub(super) fn game_architecture_install_needed(
    target: GameArchitecture,
    expected: GameArchitecture,
    installed: Option<InstalledGameArchitecture>,
) -> bool {
    installed.is_none_or(|installed| {
        (installed.requested != target && installed.binary != target)
            || installed.binary != expected
    })
}

/// Whether an installed version needs the selected architecture downloaded. This also refreshes
/// old installs with a same-version install when needed to record an ARM64 fallback choice.
pub fn needs_game_architecture_install(tag: &str, release: &Release, settings: &Settings) -> bool {
    let Some(target) = chosen_game_architecture(settings) else {
        return false;
    };
    let Some(expected) = release
        .asset
        .as_ref()
        .and_then(|(url, _)| architecture_for_asset(url))
    else {
        return false;
    };
    game_architecture_install_needed(target, expected, installed_game_architecture(tag))
}

pub(super) fn native_mod_architecture_message(
    process: GameArchitecture,
    selected: GameArchitecture,
) -> Option<&'static str> {
    (process != selected).then_some(if process == GameArchitecture::X64 {
        "Native mods cannot build for the selected ARM64 game yet. The available compiler builds x64 only."
    } else {
        "Native mods cannot build for the selected x64 game yet. This ARM64 launcher has no matching compiler."
    })
}

pub(crate) fn native_mod_architecture_error(settings: &Settings) -> Option<String> {
    let selected = chosen_game_architecture(settings)?;
    let process = architecture_for_platform(std::env::consts::ARCH)?;
    native_mod_architecture_message(process, selected)
        .map(|message| crate::i18n::tr(message).to_string())
}

/// Refuse to launch an explicit game architecture until this version has been installed for it.
pub(crate) fn game_architecture_launch_error(tag: &str, settings: &Settings) -> Option<String> {
    if settings.game_architecture == GameArchitecture::Auto {
        return None;
    }
    let target = chosen_game_architecture(settings)?;
    let installed = installed_game_architecture(tag);
    if installed.is_some_and(|installed| installed_architecture_matches(target, installed)) {
        return None;
    }
    let installed = installed.map(|a| a.binary);
    let label = |arch| match arch {
        GameArchitecture::X64 => "x64".to_owned(),
        GameArchitecture::Arm64 => "ARM64".to_owned(),
        GameArchitecture::Auto => crate::i18n::tr("Automatic").to_string(),
    };
    let installed = installed
        .map(label)
        .unwrap_or_else(|| crate::i18n::tr("Unknown").to_string());
    let selected = label(target);
    Some(crate::i18n::trf(
        "This version is installed for {installed}, but Settings asks for {selected}. Install the {selected} build first.",
        &[
            ("installed", &installed),
            ("selected", &selected),
        ],
    ))
}
