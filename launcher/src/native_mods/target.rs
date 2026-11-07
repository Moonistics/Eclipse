use super::Profile;
use crate::backend;
use crate::i18n::trf;
use crate::native_cache::{self, Dirs};
use crate::rust_toolchain;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The features of the release's own game (game-build.yml).
pub(super) const FEATURES: &str = "eclipse-client/mods";

impl Profile {
    /// The Cargo profile it compiles with (its folder in `native/target/`), and what it sets over
    /// it as `cargo --config` values: here rather than in `[profile.fast]`, which the workspace's
    /// own builds share. `Performance`, the default, is the workspace's `fast` (16 codegen units,
    /// incremental, thin-local LTO only), so changing a mod recompiles only `eclipse-client`'s
    /// changed parts; `Dev`, for developing a mod, is `fast` with `eclipse-client` unoptimized: the
    /// quickest to rebuild after an edit, for a game about twice as slow; `Release` is the
    /// release's own (thin LTO, one codegen unit): slowest to build, slightly faster game.
    /// `Performance` and `Dev` share `native/target/fast/`: every other crate is built alike, so
    /// switching between them recompiles only `eclipse-client`, and Cargo keeps both builds of it.
    /// Each profile has its own key, so their builds never mix.
    pub(super) fn cargo(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Self::Performance => (native_cache::FAST, &[]),
            Self::Dev => (
                native_cache::FAST,
                &["profile.fast.package.eclipse-client.opt-level=0"],
            ),
            Self::Release => (native_cache::RELEASE, &[]),
        }
    }

    /// Cargo's `--profile` and `--config` arguments for it: the game's and its post-build
    /// steps', so the steps reuse the crates the game built.
    pub(super) fn args(self, cmd: &mut Command) {
        let (profile, configs) = self.cargo();
        cmd.args(["--profile", profile]);
        cmd.args(configs.iter().flat_map(|config| ["--config", config]));
    }
}

/// What a native build compiles: the game the player plays, or the headless engine a dedicated
/// server runs (`eclipse-engine`, docs/server.md). Both build the same source with the same
/// mixins. The server uses `--no-default-features` (no audio or controllers), a separate cache,
/// and a standalone runtime folder outside the managed versions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(from = "String", into = "String")]
pub enum Target {
    /// The game: `eclipse-client` as the release ships it.
    #[default]
    Client,
    /// The headless engine: the same binary without the default features, installed and
    /// exported as `eclipse-engine`.
    Server,
}

/// A record's target by its name; any other than the server's is the game's.
impl From<String> for Target {
    fn from(name: String) -> Self {
        match name.as_str() {
            "server" => Self::Server,
            _ => Self::Client,
        }
    }
}

impl From<Target> for String {
    fn from(target: Target) -> Self {
        target.name().into()
    }
}

impl Target {
    pub fn name(self) -> &'static str {
        match self {
            Self::Client => "client",
            Self::Server => "server",
        }
    }

    /// What the build compiles with besides the profile, in its key: the game the release's own
    /// features, the engine `--no-default-features` (the `mods` feature is added to both when
    /// the source has it, so it changes neither key).
    pub(super) fn features(self) -> &'static str {
        match self {
            Self::Client => FEATURES,
            Self::Server => "--no-default-features",
        }
    }

    pub(super) fn directory(self, version: &str) -> PathBuf {
        match self {
            Self::Client => backend::version_dir(version),
            Self::Server => Dirs::current().server_builds().join(version),
        }
    }

    /// The name the built binary is installed under: the game's own, the engine's
    /// (`eclipse-engine`, docs/server.md).
    pub(super) fn binary(self) -> String {
        match self {
            Self::Client => backend::CLIENT.into(),
            Self::Server => format!("eclipse-engine{}", std::env::consts::EXE_SUFFIX),
        }
    }
}

/// The platform the game's source will be compiled for. On Windows this follows the installed
/// game's executable, not the PC's native architecture; the launcher must have a matching managed
/// host toolchain before any compilation starts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct BuildPlatform {
    pub(super) target: Option<&'static str>,
    pub(super) machine: Option<u16>,
    /// Its folder under `target/` ([`native_cache::platform_folder`]), which also keeps its
    /// builds' records and keys apart.
    pub(super) cache_dir: Option<&'static str>,
}

pub(super) fn platform_for_game(base: &str) -> Result<BuildPlatform, String> {
    if std::env::consts::OS != "windows" {
        return Ok(BuildPlatform::default());
    }
    let executable = backend::version_dir(base).join(backend::CLIENT);
    let machine =
        backend::pe_machine(&executable).map_err(|error| unreadable(&executable, &error))?;
    windows_build_platform(machine, rust_toolchain::host().map(|host| host.target))
}

fn windows_build_platform(
    machine: u16,
    host_target: Option<&str>,
) -> Result<BuildPlatform, String> {
    let target = rust_toolchain::windows_target_for_machine(machine).ok_or_else(|| {
        architecture_mismatch(&format!("PE machine 0x{machine:04x}"), "x64 or ARM64")
    })?;
    match host_target {
        Some(host) if host == target => {}
        Some(host) => {
            return Err(architecture_mismatch(
                windows_architecture(target),
                windows_architecture(host),
            ))
        }
        None => {
            return Err(architecture_mismatch(
                windows_architecture(target),
                std::env::consts::ARCH,
            ))
        }
    }
    Ok(BuildPlatform {
        target: Some(target),
        machine: Some(machine),
        cache_dir: native_cache::platform_folder(target),
    })
}

pub(super) fn unreadable(path: &Path, error: &str) -> String {
    trf(
        "Could not read {name}: {error}",
        &[("name", &path.display().to_string()), ("error", error)],
    )
}

pub(super) fn architecture_mismatch(game: &str, toolchain: &str) -> String {
    trf(
        "Native mods require supported Windows architectures that match. Game: {game_arch}; build toolchain: {toolchain_arch}.",
        &[("game_arch", game), ("toolchain_arch", toolchain)],
    )
}

fn windows_architecture(target: &str) -> &str {
    match target {
        rust_toolchain::WINDOWS_X64_TARGET => "x64",
        rust_toolchain::WINDOWS_ARM64_TARGET => "ARM64",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_mods::tests::dir;
    use std::fs;

    #[test]
    fn pe_machine_reads_the_architecture_after_validating_both_headers() {
        let dir = dir("pe-machine");
        let executable = dir.join("game.exe");
        let mut image = vec![0; 70];
        image[..2].copy_from_slice(b"MZ");
        image[0x3c..0x40].copy_from_slice(&64_u32.to_le_bytes());
        image[64..68].copy_from_slice(b"PE\0\0");
        image[68..70].copy_from_slice(&backend::PE_ARM64.to_le_bytes());
        fs::write(&executable, &image).unwrap();
        assert_eq!(backend::pe_machine(&executable).unwrap(), 0xaa64);

        image[64] = b'X';
        fs::write(&executable, image).unwrap();
        assert!(backend::pe_machine(&executable)
            .unwrap_err()
            .contains("missing PE signature"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn windows_build_platform_follows_the_selected_games_pe_machine() {
        let x64 = windows_build_platform(backend::PE_X64, Some(rust_toolchain::WINDOWS_X64_TARGET))
            .unwrap();
        assert_eq!(x64.target, Some(rust_toolchain::WINDOWS_X64_TARGET));
        assert_eq!(x64.machine, Some(backend::PE_X64));
        assert_eq!(x64.cache_dir, None, "preserve the x64 cache layout");

        let arm64 = windows_build_platform(
            backend::PE_ARM64,
            Some(rust_toolchain::WINDOWS_ARM64_TARGET),
        )
        .unwrap();
        assert_eq!(arm64.target, Some(rust_toolchain::WINDOWS_ARM64_TARGET));
        assert_eq!(arm64.machine, Some(backend::PE_ARM64));
        assert_eq!(arm64.cache_dir, Some(rust_toolchain::WINDOWS_ARM64_TARGET));

        assert!(windows_build_platform(
            backend::PE_X64,
            Some(rust_toolchain::WINDOWS_ARM64_TARGET),
        )
        .is_err());
        assert!(windows_build_platform(
            backend::PE_ARM64,
            Some(rust_toolchain::WINDOWS_X64_TARGET),
        )
        .is_err());
        assert!(
            windows_build_platform(0xa641, Some(rust_toolchain::WINDOWS_ARM64_TARGET)).is_err()
        );
        assert!(windows_build_platform(backend::PE_ARM64, None).is_err());
    }
}
