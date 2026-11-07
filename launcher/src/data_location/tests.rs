use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "eclipse-location-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(checked_path(&fs::canonicalize(path).unwrap()).unwrap()),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("create fixture: {e}"),
            }
        }
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        // std removes links themselves rather than traversing their targets.
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(windows)]
#[test]
fn canonical_windows_paths_are_validated_from_the_complete_root() {
    let path = std::fs::canonicalize(std::env::temp_dir()).unwrap();
    let plain = checked_path(&path).unwrap();
    assert_eq!(std::fs::canonicalize(&plain).unwrap(), path);
    assert_eq!(checked_path(&plain).unwrap(), plain);
}

#[test]
fn a_missing_or_unreadable_location_is_reported_before_the_window_opens() {
    let temp = Temp::new();
    let bootstrap = temp.0.join("location.json");
    let roots = Roots {
        launcher: temp.0.join("unplugged/launcher"),
        game: temp.0.join("unplugged/game"),
        build: temp.0.join("unplugged/build"),
    };
    let pointer = serde_json::to_vec(&Location {
        version: 1,
        roots: roots.clone(),
    })
    .unwrap();
    fs::write(&bootstrap, &pointer).unwrap();
    let error = resolve_at(&bootstrap).expect_err("a missing folder must reach the window");
    assert!(error.contains("is missing"), "{error}");

    // A damaged pointer is the other way the launcher finds no data folder.
    fs::write(&bootstrap, b"not json").unwrap();
    let damaged = resolve_at(&bootstrap).expect_err("a damaged pointer must reach the window");
    assert!(damaged.contains("Invalid"), "{damaged}");
}

#[cfg(unix)]
#[test]
fn a_config_folder_behind_a_link_still_opens() {
    // A dotfiles-managed ~/.config is a link: players who never moved their data must start
    // without an error, and a pointer kept there must still resolve.
    let temp = Temp::new();
    let real = temp.0.join("dotfiles/config");
    fs::create_dir_all(&real).unwrap();
    let config = temp.0.join("config");
    std::os::unix::fs::symlink(&real, &config).unwrap();
    let bootstrap = config.join("eclipse-launcher-location/location.json");

    assert_eq!(resolve_at(&bootstrap), Ok(None));

    let roots = Roots {
        launcher: temp.0.join("data/launcher"),
        game: temp.0.join("data/game"),
        build: temp.0.join("data/build"),
    };
    for root in [&roots.launcher, &roots.game, &roots.build] {
        fs::create_dir_all(root).unwrap();
    }
    let pointer = serde_json::to_vec(&Location {
        version: 1,
        roots: roots.clone(),
    })
    .unwrap();
    fs::create_dir_all(bootstrap.parent().unwrap()).unwrap();
    fs::write(&bootstrap, pointer).unwrap();
    assert_eq!(resolve_at(&bootstrap), Ok(Some(roots)));
}
