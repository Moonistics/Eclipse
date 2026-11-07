use super::crates;
use super::manifest::{api_target, files, Bundle};
use crate::backend;
use crate::native_cache::Dirs;
use std::fs;
use std::path::{Path, PathBuf};

/// Eclipse's source at commit `revision`: GitHub's archive of it, unpacked once, with the commit in
/// `.eclipse-revision` (bundles pin it). A Nightly's commit stays downloadable after the Nightly
/// is gone: its `nightly-source/…` tag keeps it (docs/release-flow.md). Links are left out:
/// horizon-mixins refuses them, and none is needed to build.
pub(super) fn source(revision: &str) -> Result<PathBuf, String> {
    let sources = Dirs::current().source();
    let dir = sources.join(revision);
    if dir.join(".eclipse-revision").is_file() {
        return Ok(dir);
    }
    let partial = sources.join(format!(".{revision}.partial"));
    let top = crate::github::unpack_archive(backend::REPO, revision, &partial, u64::MAX)?;
    fs::write(top.join(".eclipse-revision"), revision).map_err(|e| e.to_string())?;
    let _ = fs::remove_dir_all(&dir);
    fs::rename(&top, &dir).map_err(|e| e.to_string())?;
    let _ = fs::remove_dir_all(&partial);
    Ok(dir)
}

/// Makes `to` hold `from`'s files, writing only those that changed, so Cargo, which goes by file
/// times, recompiles only what the mods changed.
pub(super) fn sync(from: &Path, to: &Path) -> Result<(), String> {
    let new = files(from)?;
    let old = if to.is_dir() { files(to)? } else { Vec::new() };
    for (name, path) in &new {
        let target = to.join(name);
        let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        if fs::read(&target).ok() != Some(bytes) {
            fs::create_dir_all(target.parent().unwrap_or(to)).map_err(|e| e.to_string())?;
            fs::copy(path, &target).map_err(|e| format!("{}: {e}", target.display()))?;
        }
    }
    for (name, path) in old {
        if new.binary_search_by(|(n, _)| n.cmp(&name)).is_err() {
            let _ = fs::remove_file(path);
        }
    }
    Ok(())
}

/// Copies a bundle without its API package's Cargo build output.
pub(super) fn copy_bundle(bundle: &Bundle, to: &Path) -> Result<(), String> {
    let root = bundle.dir.canonicalize().map_err(|e| e.to_string())?;
    let skip = api_target(&root, bundle.api.as_deref())?;
    match skip {
        Some(skip) => crate::files::copy_dir_without(&root, to, &skip),
        None => crate::files::copy_dir(&root, to, &[]),
    }
    .map_err(|e| e.to_string())
}

/// Stage only manifests horizon-mixins can apply, after trait injections are prepared.
pub(super) fn stage_mixins(mods: &[Bundle], to: &Path) -> Result<(), String> {
    if to.exists() {
        fs::remove_dir_all(to).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for bundle in mods.iter().filter(|b| b.mixins_content) {
        copy_bundle(bundle, &to.join(&bundle.id))?;
    }
    Ok(())
}

/// The `data/` of the bundles horizon-mixins doesn't apply, staged in `stage` where it stages the
/// others' (`native-data/<mod id>/`), for `native_data!` and the mods' build steps.
pub(super) fn stage_trait_data(mods: &[Bundle], stage: &Path) -> Result<(), String> {
    for bundle in mods.iter().filter(|b| !b.mixins_content) {
        let data = bundle.dir.join("data");
        if data.is_dir() {
            let to = stage.join(crates::DATA).join(&bundle.id);
            crate::files::copy_dir(&data, &to, &[])
                .map_err(|e| format!("{}: {e}", to.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::manifest::{bundles, MANIFEST};
    use super::super::{inject, inject_cache, Profile};
    use super::*;
    use crate::native_mods::tests::dir;

    #[test]
    fn mixin_input_excludes_trait_only_bundles_but_keeps_trait_injections() {
        let root = dir("mixed-mixin-input");
        let incoming = root.join("incoming");
        for (id, injection) in [
            ("plain", ""),
            ("ordinary", "[[injection]]\nid = 'ordinary'\n"),
            ("trait_injection", "[[injection]]\nid = 'hook'\ntarget = 'eclipse_client::game::tick'\nkind = 'HEAD'\ntrait_method = 'tick'\n"),
        ] {
            let bundle = incoming.join(id);
            fs::create_dir_all(bundle.join("engine")).unwrap();
            fs::write(bundle.join("engine/Cargo.toml"), "[package]\nname = 'example'\nversion = '1.0.0'\n").unwrap();
            fs::write(bundle.join(MANIFEST), format!("[mod]\nid = '{id}'\nversion = '1'\n[api]\npath = 'engine'\n{injection}")).unwrap();
        }
        let mods = bundles(&incoming).unwrap();
        assert!(!mods[1].mixins_content);
        assert!(mods[0].mixins_content && mods[2].mixins_content);
        let key = inject_cache::key("rev", "mixins", Profile::Performance, &mods).unwrap();
        let without_plain: Vec<_> = mods.iter().filter(|b| b.id != "plain").cloned().collect();
        assert_ne!(
            key,
            inject_cache::key("rev", "mixins", Profile::Performance, &without_plain).unwrap()
        );

        let injections = inject::prepare(&mods, &root).unwrap();
        assert_eq!(injections.len(), 1);
        let input = root.join("mixins");
        stage_mixins(&mods, &input).unwrap();
        assert!(!input.join("plain").exists());
        assert!(input.join("ordinary").join(MANIFEST).is_file());
        assert!(input
            .join("trait_injection")
            .join("eclipse-trait-injection-0.rs")
            .is_file());
        let manifest = fs::read_to_string(input.join("trait_injection").join(MANIFEST)).unwrap();
        assert!(manifest.contains("eclipse-trait-injection-0.rs"));
        assert!(!manifest.contains("trait_method"));

        for id in ["plain", "ordinary"] {
            fs::create_dir_all(incoming.join(id).join("data")).unwrap();
            fs::write(incoming.join(id).join("data/table.txt"), id).unwrap();
        }
        let stage = root.join("stage");
        stage_trait_data(&mods, &stage).unwrap();
        let read =
            |id: &str| fs::read_to_string(stage.join("native-data").join(id).join("table.txt"));
        assert_eq!(read("plain").unwrap(), "plain");
        assert!(read("ordinary").is_err(), "horizon-mixins stages it");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn only_changed_files_are_written_again() {
        let (from, to) = (dir("sync-from"), dir("sync-to"));
        fs::create_dir_all(from.join("src")).unwrap();
        fs::write(from.join("src/a.rs"), "a").unwrap();
        fs::write(from.join("src/b.rs"), "b2").unwrap();
        fs::create_dir_all(to.join("src")).unwrap();
        fs::write(to.join("src/a.rs"), "a").unwrap();
        fs::write(to.join("src/b.rs"), "b").unwrap();
        fs::write(to.join("gone.rs"), "x").unwrap();
        let before = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
        fs::File::options()
            .write(true)
            .open(to.join("src/a.rs"))
            .unwrap()
            .set_modified(before)
            .unwrap();
        sync(&from, &to).unwrap();
        assert_eq!(
            fs::metadata(to.join("src/a.rs"))
                .unwrap()
                .modified()
                .unwrap(),
            before
        );
        assert_eq!(fs::read_to_string(to.join("src/b.rs")).unwrap(), "b2");
        assert!(!to.join("gone.rs").exists());
    }
}
