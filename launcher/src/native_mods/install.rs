use super::crates;
use super::is_native;
use super::record::{About, RECORD};
use super::target::Target;
use crate::backend;
use crate::i18n::trf;
use std::fs;
use std::path::{Path, PathBuf};

/// Copies the built version folder `from` into `output` (outside the managed versions) as one
/// piece: staged in a hidden folder beside `output`, then renamed into place, so an interrupted
/// export leaves nothing half-written at `output` and overwrites nothing already there.
/// Executable files keep their permissions ([`crate::files::copy_dir`], a clone where the disk can).
pub(super) fn export(from: &Path, output: &Path) -> Result<(), String> {
    let name = output.file_name().ok_or_else(|| {
        trf(
            "{path} is not a folder to build into.",
            &[("path", &output.display().to_string())],
        )
    })?;
    let parent = output.parent().unwrap_or(output);
    fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    let staging = parent.join(format!(
        ".{}.{}.exporting",
        name.to_string_lossy(),
        std::process::id()
    ));
    fs::create_dir(&staging).map_err(|e| format!("{}: {e}", staging.display()))?;
    if let Err(e) = crate::files::copy_dir(from, &staging, &[]) {
        let _ = fs::remove_dir_all(&staging);
        return Err(trf(
            "Could not copy the build: {error}",
            &[("error", &e.to_string())],
        ));
    }
    if output.symlink_metadata().is_ok() {
        let _ = fs::remove_dir_all(&staging);
        return Err(already_exists(output));
    }
    fs::rename(&staging, output).map_err(|e| {
        let _ = fs::remove_dir_all(&staging);
        trf(
            "Could not move the build to {path}: {error}",
            &[
                ("path", &output.display().to_string()),
                ("error", &e.to_string()),
            ],
        )
    })
}

pub(super) fn already_exists(path: &Path) -> String {
    trf(
        "{path} already exists. Move it away, or build into another folder.",
        &[("path", &path.display().to_string())],
    )
}

/// Copies the program `from` (in the shared target folder) to `to`, keeping it executable.
pub(super) fn copy_out(from: &Path, to: &Path) -> Result<(), String> {
    if let Some(dir) = to.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    fs::copy(from, to)
        .map(|_| ())
        .map_err(|e| format!("{}: {e}", from.display()))
}

/// Installs `binary` as version `version`: a copy of installed version `base`'s folder (its
/// resources included; cloned where the disk can) with the built binary, as `target` names it,
/// in place of `base`'s. `finish` (the mods' post-build steps) runs in its folder before it
/// counts as installed; when it fails, the folder is removed.
pub(super) fn install(
    base: &str,
    version: &str,
    target: Target,
    binary: &Path,
    about: &About,
    finish: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), String> {
    let from = backend::version_dir(base);
    let partial = target.directory(&format!(".{version}.partial"));
    let _ = fs::remove_dir_all(&partial);
    fs::create_dir_all(&partial).map_err(|e| e.to_string())?;
    copy_base(&from, &partial, target)?;
    let installed = partial.join(target.binary());
    fs::copy(binary, &installed).map_err(|e| format!("The built game: {e}"))?;
    about.write(&partial)?;
    let dir = target.directory(version);
    let _ = fs::remove_dir_all(&dir);
    fs::rename(&partial, &dir).map_err(|e| e.to_string())?;
    if let Err(e) = finish(&dir) {
        let _ = fs::remove_dir_all(&dir);
        return Err(e);
    }
    backend::mark_installed(&dir).map_err(|e| e.to_string())
}

/// Copies version folder `from` into `to` for a new build of `target`, but for what the new build
/// makes its own: its record and the mods' post-build data (`native-data/`, which the new build's
/// steps write again: a mod turned off leaves none of its data behind). A server build is the
/// headless engine: it keeps no `eclipse-engine` the base shipped, which would run unmodded beside
/// its own.
fn copy_base(from: &Path, to: &Path, target: Target) -> Result<(), String> {
    let engine = Target::Server.binary();
    let mut own = vec![RECORD, crates::DATA];
    if target == Target::Server {
        own.extend(["eclipse-engine", engine.as_str()]);
    }
    backend::copy_version_except(from, to, &own)
}

/// How many native builds nothing needs stay installed, the newest first: switching a mod
/// off and on again plays the kept build again, without compiling.
const SPARE_BUILDS: usize = 3;

/// Removes the native builds nothing needs (no instance plays them and none is selected,
/// [`backend::needed_versions`]), but `keep` and the [`SPARE_BUILDS`] newest, and any a running
/// game still runs from ([`backend::remove_unused`]). The compiled crates (`native/target`) stay,
/// so the next build only compiles what changed.
pub fn prune(keep: &str, settings: &backend::Settings) {
    let builds = made(
        backend::installed()
            .into_iter()
            .filter(|v| is_native(v))
            .map(|v| {
                let dir = backend::version_dir(&v);
                (v, dir)
            }),
    );
    backend::remove_unused(
        stale(builds, keep, &backend::needed_versions(settings)),
        settings,
    );
}

/// The installed builds among `builds` (name, folder), with when each was made or last used
/// ([`backend::installed_at`]; a build used again is marked anew).
pub(super) fn made(
    builds: impl Iterator<Item = (String, PathBuf)>,
) -> Vec<(String, std::time::SystemTime)> {
    builds
        .filter_map(|(name, dir)| Some((name, backend::installed_at(&dir)?)))
        .collect()
}

/// The native `builds` (with when each was made or last used) to remove: those not `needed`, other
/// than `keep`, past the [`SPARE_BUILDS`] newest.
pub(super) fn stale(
    mut builds: Vec<(String, std::time::SystemTime)>,
    keep: &str,
    needed: &[String],
) -> Vec<String> {
    builds.retain(|(v, _)| v != keep && !needed.contains(v));
    builds.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    builds
        .into_iter()
        .skip(SPARE_BUILDS)
        .map(|(v, _)| v)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_mods::tests::dir;

    #[test]
    fn a_rebuild_leaves_the_old_builds_mod_data_behind() {
        let base = dir("copy-base");
        let engine = Target::Server.binary();
        for file in [
            backend::CLIENT,
            backend::INSTALL_MARK,
            RECORD,
            "eclipse-ctl",
            "LICENSE",
            &engine,
        ] {
            fs::create_dir_all(base.join("from")).unwrap();
            fs::write(base.join("from").join(file), file).unwrap();
        }
        for folder in [crates::DATA, "resources"] {
            fs::create_dir_all(base.join("from").join(folder).join("inside")).unwrap();
        }
        let copied = |target: Target| {
            let to = base.join(target.name());
            copy_base(&base.join("from"), &to, target).unwrap();
            let mut names: Vec<String> = fs::read_dir(&to)
                .unwrap()
                .map(|e| e.unwrap().file_name().into_string().unwrap())
                .collect();
            names.sort();
            names
        };
        let mut client = vec!["LICENSE", "eclipse-ctl", "resources", engine.as_str()];
        client.sort();
        assert_eq!(copied(Target::Client), client);
        // A server build keeps neither the game's own binary nor a stale `eclipse-engine` the
        // base shipped, which would run unmodded beside the built one.
        assert_eq!(
            copied(Target::Server),
            ["LICENSE", "eclipse-ctl", "resources"]
        );
        fs::remove_dir_all(&base).unwrap();
    }

    /// The export ([`build_server`]): a standalone runtime folder with the engine, the mods'
    /// data and the build's record, engine still executable, and nothing of the player's
    /// overwritten.
    #[test]
    fn an_exported_server_folder_keeps_its_files_and_overwrites_nothing() {
        let built = dir("export-built");
        fs::write(built.join(Target::Server.binary()), b"engine").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                built.join(Target::Server.binary()),
                std::fs::Permissions::from_mode(0o755),
            )
            .unwrap();
        }
        fs::create_dir_all(built.join("native-data/bake")).unwrap();
        fs::write(built.join("native-data/bake/table.bin"), b"data").unwrap();
        fs::create_dir_all(built.join("resources")).unwrap();
        fs::write(built.join("resources/lang.json"), b"{}").unwrap();
        let out = dir("export-out");
        let output = out.join("server");
        export(&built, &output).unwrap();
        assert_eq!(
            fs::read(output.join(Target::Server.binary())).unwrap(),
            b"engine"
        );
        assert_eq!(
            fs::read(output.join("native-data/bake/table.bin")).unwrap(),
            b"data"
        );
        assert_eq!(fs::read(output.join("resources/lang.json")).unwrap(), b"{}");
        assert_eq!(
            fs::read_dir(&out).unwrap().flatten().count(),
            1,
            "no staging folder left beside it"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(output.join(Target::Server.binary()))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o111, 0o111, "the engine stays executable");
        }
        // A folder already there is refused, and its contents are left alone.
        let taken = dir("export-taken");
        fs::write(taken.join("keep.txt"), b"mine").unwrap();
        let err = export(&built, &taken).unwrap_err();
        assert!(err.contains("already exists"), "{err}");
        assert_eq!(fs::read(taken.join("keep.txt")).unwrap(), b"mine");
        let failed = out.join("failed");
        assert!(export(&out.join("missing-source"), &failed).is_err());
        assert!(!failed.exists());
        assert_eq!(fs::read_dir(&out).unwrap().flatten().count(), 1);
    }

    #[test]
    fn server_install_is_not_playable_and_export_survives_cache_removal() {
        let base = "native-test-server-base";
        let version = "native-test-server-cache";
        let source = backend::version_dir(base);
        fs::create_dir_all(source.join(".local/eclipse-client/26.3")).unwrap();
        fs::write(
            source.join(".local/eclipse-client/26.3/facts.json"),
            b"facts",
        )
        .unwrap();
        fs::write(source.join(Target::Server.binary()), b"unmodded").unwrap();
        let binary = dir("server-install").join("built");
        fs::write(&binary, b"modded").unwrap();
        install(
            base,
            version,
            Target::Server,
            &binary,
            &About {
                target: Target::Server,
                ..Default::default()
            },
            |d| {
                fs::create_dir_all(d.join("native-data")).unwrap();
                fs::write(d.join("native-data/test"), b"baked").unwrap();
                Ok(())
            },
        )
        .unwrap();
        assert!(!backend::installed().contains(&version.to_string()));
        let cached = Target::Server.directory(version);
        let output = dir("server-independent").join("export");
        export(&cached, &output).unwrap();
        fs::remove_dir_all(cached).unwrap();
        assert_eq!(
            fs::read(output.join(Target::Server.binary())).unwrap(),
            b"modded"
        );
        assert_eq!(fs::read(output.join("native-data/test")).unwrap(), b"baked");
        assert_eq!(
            fs::read(output.join(".local/eclipse-client/26.3/facts.json")).unwrap(),
            b"facts"
        );
        assert!(output.join("native-mods.json").is_file());
        let _ = fs::remove_dir_all(source);
    }

    #[test]
    fn the_newest_builds_no_instance_plays_are_kept() {
        let at = |s: u64| std::time::UNIX_EPOCH + std::time::Duration::from_secs(s);
        let builds: Vec<(String, std::time::SystemTime)> = [
            ("native-a", 1),
            ("native-b", 6),
            ("native-c", 3),
            ("native-d", 5),
            ("native-e", 4),
            ("native-f", 2),
            ("native-g", 7),
        ]
        .map(|(v, s)| (v.to_string(), at(s)))
        .to_vec();
        // g was just built, b another instance plays: d, e and c are the newest spare ones.
        let mut gone = stale(builds.clone(), "native-g", &["native-b".into()]);
        gone.sort();
        assert_eq!(gone, ["native-a", "native-f"]);
        assert!(stale(builds[..4].to_vec(), "native-a", &[]).is_empty());
    }
}
