//! Native mods (docs/native-mods.md): mixin bundles that
//! levzzz5154's horizon-mixins (MIT, <https://github.com/horizon-rust/horizon-mixins>) applies to
//! Eclipse's source, which the launcher then compiles into a game of its own with its portable Rust
//! ([`rust_toolchain`]). Unlike `.emod` mods they are not sandboxed: their code runs with the
//! player's rights.
//!
//! A native mod pins the Eclipse revision it was made for (a Nightly's or a release's commit).
//! Mods pinning different ones build on the newest of them, once the player agreed to try (they
//! often still apply and compile). A mod that hooks only Eclipse's native hook points
//! (`game/native_hooks.rs`) names the version it needs instead (`hooks`): it builds on any Eclipse
//! whose hook points have that version or a later one (they are only ever added to), the player's own
//! by default, so such mods always build together. The build compiles that revision's source, from
//! GitHub, never a checkout on this computer, and installs as its own version,
//! `native-<revision>-<key>` (the key hashes the revision, the mods' files, the tool's commit and
//! the profile, so the same set builds once). The instance the mods are installed in plays it, and
//! its next build stays on the same revision ([`base_for`]) until the player asks to update
//! ([`newer_than`]), so a rebuild compiles only what the mods changed.
mod api;
mod build;
mod compile;
mod conflicts;
mod crates;
mod dependencies;
mod diagnostics;
mod inject;
mod inject_cache;
mod install;
mod installed;
mod isolate;
mod key;
mod manifest;
mod mixins;
mod record;
mod revision;
mod run;
mod stage;
mod step;
mod target;

pub use crate::backend::NativeProfile as Profile;
pub use build::{build, build_server, BuildError};
pub use install::prune;
pub(crate) use installed::installed_files;
pub use installed::{installed_mods, Unpacked, INSTALLED};
pub use isolate::LeftOut;
pub use key::digest;
pub(crate) use manifest::{bundle_in, pin};
pub use manifest::{bundles, Bundle, Injection, InjectionSite};
pub use mixins::MIXINS_REV;
pub use record::{about, classify, About, BuiltMod, Plan, Record};
pub use revision::{base_for, newer_than, revision_of};
pub(crate) use revision::{decide, hooks_at, short, Base, HookCatalog, Lookup, Pinned, Undecided};
pub use run::{append_log, clear_log, log_path, Cargo};
pub use step::{duration_label, Step};
pub use target::Target;

const PREFIX: &str = "native-";

pub fn is_native(version: &str) -> bool {
    version.starts_with(PREFIX)
}

#[cfg(test)]
mod tests {
    pub(super) static LOG_TEST: std::sync::Mutex<()> = std::sync::Mutex::new(());

    use super::manifest::{bundles, Bundle, MANIFEST, NATIVE_MANIFEST};
    use crate::rust_toolchain::{self, Toolchain};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    pub(super) fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("eclipse-native-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    /// The launcher's toolchain in `dir` for every host it ships one for: the build commands are
    /// checked for each host on whichever machine runs the tests, not only for its own.
    pub(super) fn toolchains(dir: impl Into<PathBuf>) -> Vec<Toolchain> {
        let dir = dir.into();
        rust_toolchain::hosts()
            .iter()
            .map(|host| Toolchain {
                dir: dir.clone(),
                host,
            })
            .collect()
    }

    /// What `rust`'s host puts before cargo's subcommand ([`Toolchain::configure_cargo`]): the
    /// ARM64 target's rustflags, nothing on the other hosts.
    pub(super) fn host_args(rust: &Toolchain) -> Vec<String> {
        let mut cmd = Command::new("cargo");
        rust.configure_cargo(&mut cmd);
        cmd.get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    /// `cmd`'s arguments after its host's own ([`host_args`]), which must come first.
    pub(super) fn args_after_host(rust: &Toolchain, cmd: &Command) -> Vec<String> {
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        let host = host_args(rust);
        assert_eq!(args[..host.len()], host, "{}", rust.host.target);
        args[host.len()..].to_vec()
    }

    pub(super) fn write_bundle(at: &Path, id: &str, helper: &str) {
        fs::create_dir_all(at).unwrap();
        let manifest = format!("[mod]\nid = \"{id}\"\nversion = \"0.1.0\"\n");
        fs::write(at.join(MANIFEST), manifest).unwrap();
        fs::write(at.join("hook.rs"), helper).unwrap();
    }

    /// A bundle whose manifest only needs to parse, with the injections of
    /// `toml`'s `[[injection]]` list read as the build's warnings read them.
    pub(super) fn bundle_of(id: &str, toml: &str) -> Bundle {
        // Each call its own folder: the tests run in parallel.
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let at = dir(&format!("warn-{id}-{n}"));
        write_bundle(&at, id, "fn hook() {}");
        // A fragment may carry its own `[mod]` (a priority, say).
        let manifest = if toml.starts_with("[mod]") {
            toml.to_owned()
        } else {
            format!("[mod]\nid = \"{id}\"\nversion = \"0.1.0\"\n{toml}")
        };
        fs::write(at.join(MANIFEST), manifest).unwrap();
        bundles(&at).unwrap().remove(0)
    }

    pub(super) const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    pub(super) const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    /// An unpacked `.hnmod`, as the registry ships one (eclipse_mods::index::native).
    pub(super) fn write_native(at: &Path, id: &str, eclipse: &str, mixins: &str) {
        let manifest = serde_json::json!({"schema": 1, "id": id, "version": "0.3.0", "game": "26.3",
            "eclipse": eclipse, "mixins": mixins});
        fs::create_dir_all(at.join("mixins").join(id)).unwrap();
        fs::write(at.join(NATIVE_MANIFEST), manifest.to_string()).unwrap();
        let toml = format!(
            "[mod]\nid = \"{id}\"\nversion = \"0.3.0\"\n[compatibility]\neclipse_revision = \"{eclipse}\"\n"
        );
        fs::write(at.join("mixins").join(id).join(MANIFEST), toml).unwrap();
    }
}
