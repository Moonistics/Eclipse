//! Home's Mods tab: every mod of the selected instance, the browser's and those put in its
//! folder by hand, with its version, where it comes from, an update when the browser found one,
//! and a switch. A Java instance turns a mod off by renaming its jar (`.disabled`, as
//! `java_instance` does); a Eclipse instance's mods are the registry's, switched as the game's
//! Options > Mods does. Adding mods is the Mods page's.
//!
//! Each row shows the mod's own icon: Modrinth's for what the browser installed from it (the URLs
//! asked for in one request and kept beside the image cache, the images through the Mods page's
//! cache at the row's size), else the one its jar names, else a generic glyph.
use crate::mod_rows::ModRow;
use crate::{cull, instance_state, Launcher};
use eclipse_launcher::browser::{
    images, installed, java, java_instance, Kind as PackKind, SourceId,
};
use eclipse_launcher::native_status;
use gpui::prelude::*;
use std::path::Path;
use std::sync::Arc;

#[derive(Default)]
pub(super) struct InstanceMods {
    pub(super) fresh: bool,
    reading: bool,
    pub(super) list: Vec<ModRow>,
    made_from: u64,
    /// Only the rows near the view are built (a modpack has hundreds), measured again when the
    /// shown list changes (its length and the filter).
    pub(super) cull: cull::Cull,
    pub(super) shown: (usize, String),
    fingerprint: String,
}

impl InstanceMods {
    fn invalidate(&mut self) {
        self.fresh = false;
    }

    /// Invalidates the cached list when the native-file fingerprint changes.
    fn observe_fingerprint(&mut self, fingerprint: String) {
        if self.fingerprint != fingerprint {
            self.fingerprint = fingerprint;
            self.invalidate();
        }
    }

    /// Marks a read pending and returns the fingerprint its result must match.
    fn begin_read(&mut self) -> String {
        self.reading = true;
        self.fingerprint.clone()
    }

    fn made_from(&mut self, read: u64) {
        if self.made_from != read {
            self.made_from = read;
            self.invalidate();
        }
    }

    /// Delivers a read, marking it fresh only if it was made from the installed list and
    /// native files as they are now.
    fn complete_read(&mut self, made_from: u64, fingerprint: &str, list: Vec<ModRow>) {
        self.reading = false;
        if self.made_from == made_from {
            self.list = list;
            self.fresh = self.fingerprint == fingerprint;
        }
    }
}

/// The mods of the instance at `home`, with what the browser installed there (`entries`): a Java
/// one's jars in `mods/` (on or off), a Eclipse one's registry mods. Each jar names itself
/// (`fabric.mod.json`, `quilt.mod.json`...), else its file.
fn read(home: &Path, java_instance: bool, entries: Vec<installed::Entry>) -> Vec<ModRow> {
    let entries: Vec<installed::Entry> = entries
        .into_iter()
        .filter(|e| e.kind == PackKind::Mod)
        .collect();
    let mut rows: Vec<ModRow> = if java_instance {
        java_instance::jars(home)
            .into_iter()
            .map(|jar| {
                let entry = entries.iter().find(|e| e.file == jar.file).cloned();
                let (name, version) = match &entry {
                    Some(e) => (e.title.clone(), e.number.clone()),
                    None => eclipse_port_pack::jar::read(&jar.path)
                        .map(|j| (j.name, j.version))
                        .unwrap_or_else(|_| {
                            let stem = Path::new(&jar.file).file_stem().unwrap_or_default();
                            (stem.to_string_lossy().into_owned(), String::new())
                        }),
                };
                let icon = images::jar_icon(&jar.path, images::ROW)
                    .map(|png| Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Png, png)));
                ModRow {
                    file: jar.file,
                    name,
                    version,
                    on: jar.on,
                    entry,
                    icon,
                    native: None,
                }
            })
            .collect()
    } else {
        entries
            .into_iter()
            .map(|e| ModRow {
                file: e.file.clone(),
                name: e.title.clone(),
                version: e.number.clone(),
                on: installed::Game::Eclipse.is_on(home, &e),
                entry: Some(e),
                icon: None,
                native: None,
            })
            .collect()
    };
    if !java_instance {
        rows.extend(
            eclipse_launcher::github_mods::untracked(home)
                .into_iter()
                .map(|native| ModRow {
                    file: native.file.clone(),
                    name: native.id.clone(),
                    version: native.version.clone(),
                    on: java::is_enabled(home, &native.id),
                    entry: None,
                    icon: None,
                    native: Some(native),
                }),
        );
    }
    rows.sort_by_key(|r| r.name.to_lowercase());
    rows
}

impl Launcher {
    pub(super) fn instance_mods_changed(&mut self) {
        self.instance_mods.invalidate();
        self.instance_mods.cull.reset();
    }

    /// Whether the selected instance has mods to show: Eclipse's always, a Java one with a loader.
    pub(super) fn has_mods_tab(&self) -> bool {
        self.selected_instance()
            .is_some_and(instance_state::takes_mods)
    }

    /// Reads the selected instance's mods off the UI thread, once until something changes them:
    /// the Mods page's installed list (read again after every install, removal and switch) or the
    /// native files.
    fn load_instance_mods(&mut self, cx: &mut Context<Self>) {
        let Some(i) = self.selected_instance() else {
            return;
        };
        let java_instance = i.java.is_some();
        let home = i.home.clone();
        let (made_from, entries) = (self.mods.installed_read, self.mods.installed.clone());
        let m = &mut self.instance_mods;
        m.made_from(made_from);
        let fingerprint = if java_instance {
            String::new()
        } else {
            native_status::fingerprint(&home, None)
        };
        m.observe_fingerprint(fingerprint);
        if m.fresh || m.reading {
            return;
        }
        let read_fingerprint = m.begin_read();
        let task = cx
            .background_executor()
            .spawn(async move { read(&home, java_instance, entries) });
        cx.spawn(async move |this, cx| {
            let list = task.await;
            let _ = this.update(cx, |this, cx| {
                let m = &mut this.instance_mods;
                m.complete_read(made_from, &read_fingerprint, list);
                let ids = this
                    .instance_mods
                    .list
                    .iter()
                    .filter_map(|r| r.entry.as_ref())
                    .filter(|e| e.source == SourceId::Modrinth)
                    .map(|e| e.project.clone())
                    .collect();
                this.ask_project_icons(ids, cx);
                cx.notify();
            });
        })
        .detach();
    }

    /// How many mods the selected instance has; None while they are read.
    pub(super) fn instance_mods_count(&mut self, cx: &mut Context<Self>) -> Option<usize> {
        self.mods_load();
        self.load_instance_mods(cx);
        let m = &self.instance_mods;
        m.fresh.then_some(m.list.len())
    }

    pub(super) fn switch_mod(&mut self, row: &ModRow) {
        match &row.entry {
            Some(e) => self.mods_switch(e),
            None => {
                let home = self.mods.home();
                self.mods.failed = java_instance::set_file_on(&home, &row.file, !row.on).err();
                self.instance_mods.invalidate();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eclipse_launcher::native_mods;
    use std::path::PathBuf;

    struct TestHome(PathBuf);

    impl TestHome {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU64, Ordering};
            use std::time::{SystemTime, UNIX_EPOCH};

            static NEXT: AtomicU64 = AtomicU64::new(0);
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock is before Unix epoch")
                .as_nanos();
            loop {
                let home = std::env::temp_dir().join(format!(
                    "instance-mods-refresh-{}-{timestamp}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                match std::fs::create_dir(&home) {
                    Ok(()) => return Self(home),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => panic!("create test home {}: {error}", home.display()),
                }
            }
        }
    }

    impl Drop for TestHome {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn native_mod_changes_during_read_trigger_a_fresh_read() {
        let scratch = TestHome::new();
        let home = &scratch.0;
        let installed = home.join(native_mods::INSTALLED);
        std::fs::create_dir_all(&installed).unwrap();
        std::fs::write(installed.join("first.hnmod"), b"unreadable").unwrap();

        let mut mods = InstanceMods::default();
        mods.observe_fingerprint(native_status::fingerprint(home, None));
        assert!(!mods.fresh);
        let read_fingerprint = mods.begin_read();
        let old_list = read(home, false, Vec::new());
        assert_eq!(old_list.len(), 1);

        std::fs::write(installed.join("second.hnmod"), b"unreadable").unwrap();
        mods.observe_fingerprint(native_status::fingerprint(home, None));
        assert!(!mods.fresh);
        assert!(mods.reading, "the first read must still be pending");

        mods.complete_read(0, &read_fingerprint, old_list);
        let another_read_needed = !mods.fresh && !mods.reading;
        if another_read_needed {
            let read_fingerprint = mods.begin_read();
            mods.complete_read(0, &read_fingerprint, read(home, false, Vec::new()));
        }
        let final_files: Vec<_> = mods.list.iter().map(|row| row.file.as_str()).collect();
        let final_listing_is_fresh = mods.fresh
            && final_files.contains(&"eclipse-native-mods/first.hnmod")
            && final_files.contains(&"eclipse-native-mods/second.hnmod");
        assert!(
            another_read_needed && final_listing_is_fresh,
            "a native change during a read must leave another read needed and deliver both files; another_read_needed={another_read_needed}, fresh={}, files={final_files:?}",
            mods.fresh
        );
    }

    #[test]
    fn eclipse_mods_include_local_native_files_without_duplicating_registry_rows() {
        let home =
            std::env::temp_dir().join(format!("instance-native-mods-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(home.join(native_mods::INSTALLED)).unwrap();
        for id in ["local", "registry"] {
            std::fs::write(
                home.join(native_mods::INSTALLED)
                    .join(format!("{id}.hnmod")),
                b"unreadable",
            )
            .unwrap();
        }
        let entry: installed::Entry = serde_json::from_value(serde_json::json!({"source":"eclipse","project":"registry","slug":"registry","title":"Registry mod","kind":"mod","version":"1","number":"1","file":"eclipse-native-mods/registry.hnmod","checksum":"","page":""})).unwrap();
        installed::Installed::open(&home)
            .unwrap()
            .record(entry)
            .unwrap();
        java::set_enabled(&home, "local", false).unwrap();
        java::set_enabled(&home, "registry", true).unwrap();
        let rows = read(&home, false, installed::Installed::load(&home).entries);
        let seen: Vec<_> = rows
            .iter()
            .map(|r| (r.name.as_str(), r.on, r.native.is_some()))
            .collect();
        assert_eq!(
            seen,
            [("local", false, true), ("Registry mod", true, false)]
        );
        assert!(
            read(&home, true, Vec::new()).is_empty(),
            "native mods stay out of Java instances"
        );
        std::fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn a_java_instance_lists_its_jars_on_and_off() {
        let home = std::env::temp_dir().join(format!("instance-mods-{}", std::process::id()));
        let mods = home.join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("zeta-1.0.jar"), b"not a zip").unwrap();
        std::fs::write(mods.join("alpha-2.0.jar.disabled"), b"not a zip").unwrap();
        std::fs::write(mods.join("notes.txt"), b"").unwrap();
        let rows = read(&home, true, Vec::new());
        let seen: Vec<_> = rows.iter().map(|r| (r.name.as_str(), r.on)).collect();
        assert_eq!(seen, [("alpha-2.0", false), ("zeta-1.0", true)]);
        java_instance::set_file_on(&home, "mods/alpha-2.0.jar", true).unwrap();
        assert!(read(&home, true, Vec::new()).iter().all(|r| r.on));
        let _ = std::fs::remove_dir_all(&home);
    }
}
