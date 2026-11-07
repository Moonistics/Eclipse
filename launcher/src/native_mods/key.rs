use super::manifest::{bundle_files, files, Bundle, NATIVE_MANIFEST};
use super::revision::short;
use super::target::Target;
use super::{Profile, PREFIX};
use crate::rust_toolchain;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

fn hash_file(hash: &mut Sha256, name: &str, bytes: &[u8]) {
    hash.update(name.as_bytes());
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}

pub(super) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub(super) fn mod_hash(b: &Bundle) -> Result<String, String> {
    let mut hash = Sha256::new();
    for (name, path) in bundle_files(b)? {
        let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        hash_file(&mut hash, &name, &bytes);
    }
    Ok(hex(&hash.finalize()[..8]))
}

/// The installed version `bundles` built on Eclipse `revision` with horizon-mixins `mixins`
/// become: `native-<7 digits of the revision>-<12 hex digits>`. The two targets' keys never mix:
/// a server build salts its hash, so it is another version than the client build of the same
/// mods, and the client key is unchanged.
pub(super) fn version_for(
    revision: &str,
    mixins: &str,
    profile: Profile,
    target: Target,
    platform: Option<&str>,
    bundles: &[Bundle],
) -> Result<String, String> {
    let mut hash = Sha256::new();
    let (cargo_profile, configs) = profile.cargo();
    for part in [mixins, cargo_profile]
        .into_iter()
        .chain(configs.iter().copied())
        .chain([target.features(), rust_toolchain::VERSION, revision])
        .chain(platform)
    {
        hash.update(part.as_bytes());
        hash.update([0]);
    }
    if target == Target::Server {
        hash.update(target.name().as_bytes());
        hash.update([0]);
    }
    // bundles() reads folders in path order, which with their manifests decides trait dispatch
    // order: hashed in that order, a different order is another build.
    for b in bundles {
        hash.update(b.id.as_bytes());
        hash.update([0]);
        for (name, path) in bundle_files(b)? {
            let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            hash_file(&mut hash, &name, &bytes);
        }
    }
    let key = hex(&hash.finalize()[..6]);
    Ok(format!("{PREFIX}{}-{key}", short(revision)))
}

/// A digest of a mod's files (name, length, bytes, sorted by name), kept in `native-mods.json` so
/// the Mods page sees a mod edited or reinstalled without a version bump.
pub fn digest<'a>(files: impl Iterator<Item = (&'a str, &'a [u8])>) -> String {
    let mut hash = Sha256::new();
    for (name, bytes) in files {
        hash_file(&mut hash, name, bytes);
    }
    hex(&hash.finalize())
}

/// The digest of a bundle's files as the Mods page reads it (`native_status::enabled`): an
/// unpacked `.hnmod`'s whole folder, its `eclipse-native.json` included, else the bundle's own
/// folder. A build records it, so a build of the mods turned on reads as up to date.
pub(super) fn source_digest(bundle: &Bundle) -> Option<String> {
    let root = bundle
        .dir
        .parent()
        .and_then(Path::parent)
        .filter(|root| {
            root.join(NATIVE_MANIFEST).is_file()
                && bundle.dir == root.join("mixins").join(&bundle.id)
        })
        .unwrap_or(&bundle.dir);
    let paths = if root == bundle.dir {
        bundle_files(bundle)
    } else {
        files(root)
    };
    let read: Vec<_> = paths
        .ok()?
        .into_iter()
        .map(|(name, path)| Some((name, fs::read(path).ok()?)))
        .collect::<Option<_>>()?;
    Some(digest(read.iter().map(|(n, b)| (n.as_str(), b.as_slice()))))
}

#[cfg(test)]
mod tests {
    use super::super::is_native;
    use super::super::manifest::{bundle, bundles, MANIFEST};
    use super::super::mixins::MIXINS_REV;
    use super::super::stage::copy_bundle;
    use super::*;
    use crate::backend;
    use crate::native_mods::tests::{dir, write_bundle, A, B};

    #[test]
    fn the_version_changes_with_the_revision_and_any_file_of_the_mods() {
        let d = dir("key");
        write_bundle(&d, "hello", "fn hook() {}");
        let key = |revision: &str| {
            version_for(
                revision,
                MIXINS_REV,
                Profile::Performance,
                Target::Client,
                None,
                &bundles(&d).unwrap(),
            )
            .unwrap()
        };
        let first = key(A);
        assert!(
            first.starts_with("native-aaaaaaa-") && first.len() == "native-aaaaaaa-".len() + 12
        );
        assert!(is_native(&first) && !backend::is_game_tag(&first));
        assert_eq!(first, key(A));
        assert_ne!(
            first,
            version_for(
                A,
                MIXINS_REV,
                Profile::Performance,
                Target::Client,
                Some(rust_toolchain::WINDOWS_ARM64_TARGET),
                &bundles(&d).unwrap(),
            )
            .unwrap(),
            "the ARM64 toolchain has a separate installed build"
        );
        assert_ne!(first, key(B));
        assert_ne!(
            first,
            version_for(
                A,
                B,
                Profile::Performance,
                Target::Client,
                None,
                &bundles(&d).unwrap()
            )
            .unwrap()
        );
        fs::write(d.join("hook.rs"), "fn hook() { let _ = 1; }").unwrap();
        assert_ne!(first, key(A));
    }

    #[test]
    fn api_target_is_neither_copied_nor_hashed() {
        let d = dir("api-target");
        fs::create_dir_all(d.join("engine/target/debug")).unwrap();
        fs::create_dir_all(d.join("other/target")).unwrap();
        fs::create_dir_all(d.join("engine/empty")).unwrap();
        fs::write(
            d.join(MANIFEST),
            "[mod]\nid = 'example'\nversion = '1'\n[api]\npath = 'engine'\n",
        )
        .unwrap();
        fs::write(
            d.join("engine/Cargo.toml"),
            "[package]\nname = 'example'\nversion = '0.1.0'\n",
        )
        .unwrap();
        fs::write(d.join("engine/target/debug/output"), "one").unwrap();
        fs::write(d.join("other/target/keep"), "keep").unwrap();
        let bundle = bundle(&d, None).unwrap();
        let key = || {
            version_for(
                A,
                B,
                Profile::Performance,
                Target::Client,
                None,
                std::slice::from_ref(&bundle),
            )
            .unwrap()
        };
        let first = key();
        let hash = mod_hash(&bundle).unwrap();
        let digest = source_digest(&bundle);
        fs::write(d.join("engine/target/debug/output"), "two").unwrap();
        assert_eq!(first, key());
        assert_eq!(hash, mod_hash(&bundle).unwrap());
        assert_eq!(digest, source_digest(&bundle));
        let dest = dir("api-target-copy");
        copy_bundle(&bundle, &dest).unwrap();
        assert!(!dest.join("engine/target").exists());
        assert!(dest.join("engine/empty").is_dir());
        assert!(dest.join("other/target/keep").is_file());
        fs::write(d.join("engine/Cargo.toml"), "changed").unwrap();
        assert_ne!(first, key());
    }

    /// The test bundle (`tests/fixtures/native-crate-demo`): its crate's sources and data, and its
    /// own data folder, are all in the build's key.
    #[test]
    fn the_key_covers_a_bundles_crate_and_data_files() {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/native-crate-demo");
        let d = dir("crate-key");
        crate::files::copy_dir(&fixture, &d, &[]).unwrap();
        let found = bundles(&d).unwrap();
        assert_eq!(found[0].id, "crate_demo");
        let key = || {
            version_for(
                A,
                B,
                Profile::Performance,
                Target::Client,
                None,
                &bundles(&d).unwrap(),
            )
            .unwrap()
        };
        let mut seen = vec![key()];
        for file in [
            "engine/src/lib.rs",
            "engine/data/salt.txt",
            "engine/Cargo.toml",
            "data/greeting.txt",
        ] {
            let mut bytes = fs::read(d.join(file)).unwrap();
            bytes.extend_from_slice(b"\n");
            fs::write(d.join(file), bytes).unwrap();
            let now = key();
            assert!(!seen.contains(&now), "{file} is in the key");
            seen.push(now);
        }
    }
}
