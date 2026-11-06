//! Advice for a launcher that cannot replace its own executable.
use std::path::Path;

/// Package-owned files must be replaced by their package manager, not a downloaded archive.
/// `/usr/local` is where an administrator copies programs by hand: no package manager owns it.
pub fn package_managed(target: &Path) -> bool {
    ["/usr", "/nix/store", "/app"]
        .iter()
        .any(|root| target.starts_with(root))
        && !target.starts_with("/usr/local")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_roots_are_matched_as_path_components() {
        for path in [
            "/usr/bin/eclipse-launcher",
            "/nix/store/hash-eclipse/bin/eclipse-launcher",
            "/app/bin/eclipse-launcher",
        ] {
            assert!(package_managed(Path::new(path)), "{path}");
        }
        for path in [
            "/home/player/Downloads/Eclipse.AppImage",
            "/usr/local/bin/eclipse-launcher",
            "/usr-backup/eclipse-launcher",
            "/nix/storehouse/eclipse-launcher",
            "/application/eclipse-launcher",
            "/tmp/.mount_Eclipse/usr/bin/eclipse-launcher",
        ] {
            assert!(!package_managed(Path::new(path)), "{path}");
        }
    }
}
