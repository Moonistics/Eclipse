use super::install::copy_out;
use super::manifest::Bundle;
use super::run::run;
use super::step::Step;
use super::target::{Target, FEATURES};
use super::{crates, inject, Profile};
use crate::rust_toolchain::{self, Toolchain};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Whether Eclipse's source at `source` has the `mods` feature (the WebAssembly mods' runtime,
/// gone from later revisions).
fn has_mods_feature(source: &Path) -> bool {
    let path = source.join("crates/eclipse-client/Cargo.toml");
    let text = fs::read_to_string(path).unwrap_or_default();
    let value: toml::Table = text.parse().unwrap_or_default();
    value.get("features").and_then(|f| f.get("mods")).is_some()
}

/// The profile the source at `source` builds with: `chosen`, except that one built on
/// `[profile.fast]` becomes `Release` (the release's own) for a revision whose Cargo.toml has none.
pub(super) fn profile_of(source: &Path, chosen: Profile) -> Profile {
    if chosen == Profile::Release {
        return chosen;
    }
    let text = fs::read_to_string(source.join("Cargo.toml")).unwrap_or_default();
    let value: toml::Table = text.parse().unwrap_or_default();
    match value.get("profile").and_then(|p| p.get("fast")) {
        Some(_) => chosen,
        None => Profile::Release,
    }
}

/// The build of the game in `stage` with `profile` (game-build.yml's, for `Release`) and
/// `target`'s features, into `target`. It starts outside any Cargo configuration of the
/// player's ([`rust_toolchain::outside_configs`]), so only the workspace's own
/// (`stage/.cargo/config.toml`, when it has one) applies. A server build compiles headless:
/// `--no-default-features` (the server build, docs/server.md), keeping the `mods` feature when
/// the source has one.
pub(super) fn compile(
    rust: &Toolchain,
    stage: &Path,
    target: Target,
    target_dir: &Path,
    profile: Profile,
    package: &str,
) -> Command {
    let mut cmd = cargo(rust, stage, "build");
    // Cargo's progress bar (units done of the total) is read from its output. Incremental
    // compilation, the `fast` profile's, is not turned off by the player's environment: the
    // toolchain's command drops `CARGO_INCREMENTAL` and `CARGO_BUILD_*`.
    cmd.env("CARGO_TARGET_DIR", target_dir)
        .env("CARGO_TERM_PROGRESS_WHEN", "always")
        .env("CARGO_TERM_PROGRESS_WIDTH", "100");
    cmd.args(["--locked", "--message-format=json"]);
    profile.args(&mut cmd);
    if target == Target::Server {
        cmd.arg("--no-default-features");
    }
    if has_mods_feature(stage) {
        cmd.args(["--features", FEATURES]);
    }
    cmd.args(["-p", package, "--bin", GAME_BIN]);
    cmd
}

/// The game's binary in Eclipse's workspace, which the trait mods' generated package also builds.
pub(super) const GAME_BIN: &str = "eclipse-client";

/// The launcher's cargo running `subcommand` on the workspace `dir`, at low priority, with only
/// the workspace's own Cargo configuration ([`compile`]).
pub(super) fn cargo(rust: &Toolchain, dir: &Path, subcommand: &str) -> Command {
    let mut cmd = rust.cargo_command();
    cmd.arg(subcommand);
    let config = dir.join(".cargo").join("config.toml");
    if config.is_file() {
        cmd.arg("--config").arg(config);
    }
    cmd.arg("--manifest-path").arg(dir.join("Cargo.toml"));
    // Only this stage's declarations apply, never a path inherited from the launcher's shell.
    let injections = dir.join(inject::INPUT);
    if injections.is_file() {
        cmd.env(inject::ENV, injections);
    } else {
        cmd.env_remove(inject::ENV);
    }
    cmd
}

/// Checks the mods' crates (`native-crates/`) against `source`'s `Cargo.lock`, Eclipse's own.
/// horizon-mixins already wrote them into `stage`'s lock (and refused a crate the lock lacks,
/// `E_CRATE_DEPENDENCY`), leaving every other package as it was: this checks that again, a crate
/// they name must be in Eclipse's lock and every package downloaded exactly as there. Then the
/// crates are fetched, so a feature turning on a dependency the lock lacks fails here, readably.
pub(super) fn lock_crates(
    rust: &Toolchain,
    source: &Path,
    stage: &Path,
    mods: &[Bundle],
) -> Result<(), String> {
    let read = |dir: &Path| {
        let path = dir.join("Cargo.lock");
        fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))
    };
    let eclipse = crates::downloaded(&read(source)?)?;
    let staged = crates::downloaded(&read(stage)?)?;
    // `wanted` reads native-crates, which API-only stages do not have.
    let wanted = if crates::any(stage) {
        crates::wanted(stage)?
    } else {
        Vec::new()
    };
    let api_ids: Vec<&str> = mods
        .iter()
        .filter(|m| m.api.is_some())
        .map(|m| m.id.as_str())
        .collect();
    if let Some(why) = crates::refused(&eclipse, Some(&staged), &wanted, &api_ids) {
        return Err(why);
    }
    let mut fetch = cargo(rust, stage, "fetch");
    fetch.arg("--locked");
    if let Some(host) = rust_toolchain::host() {
        fetch.args(["--target", host.target]);
    }
    run(fetch, "Downloading the crates", |_| {}).map_err(crates::locked)
}

/// Builds the mods' post-build programs with the game's profile in `target`; each step and its
/// program, in mod order. A bin's file name doesn't include its package, so two mods' bins of the
/// same name (or one named like the game) are the same file in `target`: each program is copied
/// to `programs/<step>/` as soon as it is built. horizon-mixins refuses such names too
/// (`E_POST_BUILD`, horizon-mixins#8); this keeps a bundle built by an older one safe.
pub(super) fn post_build_programs(
    rust: &Toolchain,
    stage: &Path,
    target: &Path,
    programs: &Path,
    profile: Profile,
    progress: &impl Fn(Step),
) -> Result<Vec<(crates::PostBuild, PathBuf)>, String> {
    let steps = crates::post_builds(stage)?;
    let _ = fs::remove_dir_all(programs);
    let mut out = Vec::new();
    for (i, step) in steps.into_iter().enumerate() {
        progress(Step::PostBuildCompile(step.mod_id.clone()));
        let mut cmd = cargo(rust, stage, "build");
        cmd.env("CARGO_TARGET_DIR", target);
        cmd.arg("--locked");
        profile.args(&mut cmd);
        cmd.args(["-p", &step.package, "--bin", &step.bin]);
        run(
            cmd,
            &format!("Building {}'s post-build step", step.mod_id),
            |_| {},
        )?;
        let file = format!("{}{}", step.bin, std::env::consts::EXE_SUFFIX);
        let program = programs.join(i.to_string()).join(&file);
        copy_out(&target.join(profile.cargo().0).join(&file), &program)?;
        out.push((step, program));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::super::key::version_for;
    use super::super::manifest::bundles;
    use super::super::stage::sync;
    use super::*;
    use crate::native_mods::tests::{
        args_after_host, dir, host_args, toolchains, write_bundle, A, B,
    };

    #[test]
    fn staged_cargo_uses_only_its_own_injections_including_cached_output() {
        let d = dir("injection-env");
        let fresh = d.join("fresh");
        let snapshot = d.join("snapshot");
        let stage = d.join("stage");
        fs::create_dir_all(fresh.join("crates/eclipse-client")).unwrap();
        fs::write(
            fresh.join("crates/eclipse-client/Cargo.toml"),
            "[package]\nname = 'eclipse-client'\nversion = '0.1.0'\n[build-dependencies]\neclipse-mod-api-gen = { path = '../eclipse-mod-api-gen' }\n",
        )
        .unwrap();
        let rusts = toolchains(d.join("rust"));
        // The same for every host.
        let env = |dir: &Path| {
            let each: Vec<_> = rusts
                .iter()
                .map(|rust| {
                    cargo(rust, dir, "build")
                        .get_envs()
                        .find(|(key, _)| *key == inject::ENV)
                        .map(|(_, value)| value.map(std::ffi::OsStr::to_owned))
                })
                .collect();
            assert!(each.windows(2).all(|w| w[0] == w[1]), "{each:?}");
            each[0].clone()
        };
        assert_eq!(env(&stage), Some(None));
        inject::stage(&fresh, &[]).unwrap();
        sync(&fresh, &snapshot).unwrap();
        sync(&snapshot, &stage).unwrap();
        assert_eq!(fs::read(stage.join(inject::INPUT)).unwrap(), b"[]");
        assert_eq!(
            env(&stage),
            Some(Some(stage.join(inject::INPUT).into_os_string()))
        );
        fs::remove_file(fresh.join(inject::INPUT)).unwrap();
        sync(&fresh, &stage).unwrap();
        assert_eq!(env(&stage), Some(None));
        fs::remove_dir_all(d).unwrap();
    }

    /// The server build (docs/server.md): compiled headless off the same source, with its own
    /// key, and the game build's flags unchanged.
    #[test]
    fn a_server_build_compiles_headless_with_its_own_key() {
        let d = dir("server-key");
        write_bundle(&d, "hello", "fn hook() {}");
        let found = bundles(&d).unwrap();
        let key =
            |target: Target| version_for(A, B, Profile::Performance, target, None, &found).unwrap();
        assert_ne!(
            key(Target::Client),
            key(Target::Server),
            "the two targets never stand in for each other"
        );
        assert_eq!(key(Target::Server), key(Target::Server));
        assert_ne!(
            key(Target::Server),
            version_for(A, B, Profile::Release, Target::Server, None, &found).unwrap(),
            "the profile is still in the server key"
        );
        let rusts = toolchains("/rust/1.99.1-x");
        // The same after each host's own arguments.
        let flags = |stage: &Path, target: Target| {
            let each: Vec<Vec<String>> = rusts
                .iter()
                .map(|rust| {
                    let cmd = compile(
                        rust,
                        stage,
                        target,
                        Path::new("/native/target"),
                        Profile::Performance,
                        "eclipse-client",
                    );
                    args_after_host(rust, &cmd)
                })
                .collect();
            assert!(each.windows(2).all(|w| w[0] == w[1]), "{each:?}");
            each[0].clone()
        };
        let server = flags(&d, Target::Server);
        assert!(server.contains(&"--no-default-features".into()));
        assert!(
            !server.iter().any(|a| a.starts_with("--features")),
            "no feature the source doesn't have"
        );
        let client = flags(&d, Target::Client);
        assert!(!client.contains(&"--no-default-features".into()));
        // A source with the `mods` feature keeps it in both builds.
        fs::create_dir_all(d.join("crates/eclipse-client")).unwrap();
        fs::write(
            d.join("crates/eclipse-client/Cargo.toml"),
            "[features]\nmods = []\n",
        )
        .unwrap();
        for target in [Target::Client, Target::Server] {
            let args = flags(&d, target);
            assert!(
                args.windows(2)
                    .any(|w| w == ["--features", FEATURES].map(String::from)),
                "{:?}: the mods feature stays on",
                target
            );
        }
    }

    #[test]
    fn the_game_compiles_outside_the_players_cargo_configuration() {
        for rust in toolchains("/rust/1.99.1-x") {
            assert_eq!(
                host_args(&rust).is_empty(),
                rust.host.target != rust_toolchain::WINDOWS_ARM64_TARGET,
                "only the ARM64 host adds its target's rustflags"
            );
            compiles_outside_the_players_cargo_configuration(&rust);
        }
    }

    fn compiles_outside_the_players_cargo_configuration(rust: &Toolchain) {
        let stage = dir(&format!("stage-config-{}", rust.host.target));
        let cmd = compile(
            rust,
            &stage,
            Target::Client,
            Path::new("/native/target"),
            Profile::Performance,
            "eclipse-client",
        );
        let args = args_after_host(rust, &cmd);
        assert_eq!(
            args[..3],
            [
                "build",
                "--manifest-path",
                &*stage.join("Cargo.toml").to_string_lossy()
            ]
        );
        assert!(
            args.ends_with(&["-p", "eclipse-client", "--bin", "eclipse-client"].map(String::from))
        );
        let dir = cmd.get_current_dir().expect("a folder of its own");
        assert!(
            dir.has_root() && dir.parent().is_none(),
            "{}",
            dir.display()
        );
        // The workspace's own configuration still applies.
        fs::create_dir_all(stage.join(".cargo")).unwrap();
        fs::write(stage.join(".cargo/config.toml"), "").unwrap();
        let cmd = compile(
            rust,
            &stage,
            Target::Client,
            Path::new("/native/target"),
            Profile::Performance,
            "eclipse-client",
        );
        let args = args_after_host(rust, &cmd);
        assert_eq!(args[1], "--config");
        assert_eq!(PathBuf::from(&args[2]), stage.join(".cargo/config.toml"));
        let _ = fs::remove_dir_all(stage);
    }

    #[test]
    fn builds_fall_back_to_release_where_the_source_has_no_fast_profile() {
        let d = dir("profile");
        assert_eq!(profile_of(&d, Profile::Performance), Profile::Release);
        fs::write(d.join("Cargo.toml"), "[profile.release]\nlto = \"thin\"\n").unwrap();
        assert_eq!(profile_of(&d, Profile::Performance), Profile::Release);
        fs::write(
            d.join("Cargo.toml"),
            "[profile.release]\nlto = \"thin\"\n[profile.fast]\ninherits = \"release\"\n",
        )
        .unwrap();
        assert_eq!(profile_of(&d, Profile::Performance), Profile::Performance);
        assert_eq!(
            profile_of(&d, Profile::Release),
            Profile::Release,
            "the player's choice of release stays"
        );
        let rusts = toolchains("/rust/1.99.1-x");
        // The same after each host's own arguments.
        let args = |profile| -> Vec<String> {
            let each: Vec<Vec<String>> = rusts
                .iter()
                .map(|rust| {
                    let cmd = compile(
                        rust,
                        &d,
                        Target::Client,
                        Path::new("/native/target"),
                        profile,
                        "eclipse-client",
                    );
                    args_after_host(rust, &cmd)
                })
                .collect();
            assert!(each.windows(2).all(|w| w[0] == w[1]), "{each:?}");
            each[0].clone()
        };
        let performance = args(Profile::Performance);
        assert!(performance.windows(2).any(|w| w == ["--profile", "fast"]));
        assert!(!performance.contains(&"--config".to_string()));
        let dev = args(Profile::Dev);
        assert!(dev.windows(2).any(|w| w == ["--profile", "fast"]));
        assert!(dev.windows(2).any(|w| w
            == [
                "--config",
                "profile.fast.package.eclipse-client.opt-level=0"
            ]));
        let release = args(Profile::Release);
        assert!(release.windows(2).any(|w| w == ["--profile", "release"]));
        assert!(
            !release.contains(&"--config".to_string()),
            "the release's own stays"
        );
    }
}
