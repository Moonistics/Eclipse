//! Nightly (docs/release-flow.md): the only build Eclipse ships since 2026-10-28, main plus the pull
//! requests the trusted reviewers picked, published as the `nightly` pre-release a few times a day.
//! Every launcher installs it as its update (`backend::eclipse_update`), as its own version
//! `nightly-<date>-<commit>`, beside the player's worlds, which it never touches. Its checksums must
//! be signed for the nightly channel (`signed::nightly_digest`).
//!
//! Before that it was opt-in and played in its own "Nightly" instance (`instances/nightly/`):
//! that instance, and its worlds, stay, and keep following the Nightly.
use crate::backend;
use crate::instances;

/// The id of the instance (its folder in `instances/`) the opt-in Nightly played in.
pub const INSTANCE: &str = "nightly";

/// The date of a nightly's version (`nightly-2026-10-28-0c02067`), or None for any other version.
pub use eclipse_release_sign::nightly_date as date_of;

/// The commit a nightly's version names (`0c02067` in `nightly-2026-10-28-0c02067`), or None for
/// any other version.
pub fn commit_of(version: &str) -> Option<&str> {
    date_of(version)?;
    version
        .get("nightly-2026-10-28-".len()..)
        .filter(|c| !c.is_empty())
}

/// A Nightly's version as a player reads it, the way its release is titled:
/// `Nightly 2026-10-29 · 4baa571` for `nightly-2026-10-29-4baa571`, or None for anything that is
/// not one.
pub fn label(version: &str) -> Option<String> {
    let date = date_of(version)?;
    Some(match commit_of(version) {
        Some(commit) => format!("Nightly {date} · {commit}"),
        None => format!("Nightly {date}"),
    })
}

pub fn display_name(version: &str) -> String {
    label(version)
        .or_else(|| crate::build_import::label(version))
        .unwrap_or_else(|| version.trim_start_matches('v').to_string())
}

/// After `version`, a Nightly, installed as an update: the old Nightly instance follows it, and the
/// older Nightlies nothing needs go (`backend::remove_unused`). Worlds stay. Returns the versions
/// removed.
pub fn after_update(version: &str) -> Vec<String> {
    if instances::list()
        .iter()
        .any(|i| i.id == INSTANCE && i.version.as_deref().is_some_and(|v| date_of(v).is_some()))
    {
        let _ = instances::edit(INSTANCE, |meta| meta.version = Some(version.to_owned()));
    }
    backend::remove_unused(
        backend::installed()
            .into_iter()
            .filter(|v| date_of(v).is_some() && v != version)
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_nightly_names_the_commit_it_was_built_from() {
        assert_eq!(commit_of("nightly-2026-10-28-0c02067"), Some("0c02067"));
        assert_eq!(commit_of("nightly-2026-10-28"), None);
        assert_eq!(commit_of("v0.2.95"), None);
        assert_eq!(commit_of("nightly"), None);
    }

    #[test]
    fn a_nightly_reads_as_its_release_is_titled() {
        assert_eq!(
            label("nightly-2026-10-29-4baa571").as_deref(),
            Some("Nightly 2026-10-29 · 4baa571")
        );
        assert_eq!(
            label("nightly-2026-10-29").as_deref(),
            Some("Nightly 2026-10-29")
        );
        assert_eq!(label("v0.2.97"), None);
    }

    #[cfg(unix)]
    #[test]
    fn an_update_never_removes_a_nightly_that_is_running() {
        let [current, running, old] = [
            "nightly-2026-01-03-ccccccc",
            "nightly-2026-01-02-bbbbbbb",
            "nightly-2026-01-01-aaaaaaa",
        ];
        for version in [current, running, old] {
            let dir = backend::version_dir(version);
            std::fs::create_dir_all(&dir).unwrap();
            backend::mark_installed(&dir).unwrap();
        }
        let program = backend::version_dir(running).join("eclipse-client");
        let mut game = std::process::Command::new("sh")
            .args(["-c", "sleep 30"])
            .arg(&program)
            .spawn()
            .unwrap();
        let gone = after_update(current);
        let _ = game.kill();
        let _ = game.wait();
        let left = backend::installed();
        for version in [current, running] {
            let _ = std::fs::remove_dir_all(backend::version_dir(version));
        }
        assert_eq!(gone, [old]);
        assert!(left.iter().any(|v| v == running), "{left:?}");
        assert!(left.iter().any(|v| v == current), "{left:?}");
    }
}
