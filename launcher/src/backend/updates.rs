use super::architecture::{can_choose_game_architecture, needs_game_architecture_install};
use super::releases::{
    compare_versions, game_releases, installable_updates, is_game_tag, latest,
    launcher_releases_in, own_tag, release_list, Release,
};
use super::settings::Settings;
use super::versions::installed;

/// What a check found: a Eclipse build newer than every installed one, and a newer launcher.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct Updates {
    pub eclipse: Option<Release>,
    pub launcher: Option<Release>,
}

/// The newest Eclipse build with a build here (the current Nightly), if it isn't installed and
/// no installed version is newer. Two Nightlies of one day compare equal: the one on GitHub is
/// the update.
pub fn eclipse_update(releases: &[Release], installed: &[String]) -> Option<Release> {
    let newest = latest(releases)?;
    let newer = !installed.contains(&newest.tag)
        && installed
            .iter()
            .all(|t| compare_versions(&newest.tag, t).is_ge());
    newer.then(|| newest.clone())
}

/// The newest launcher release with an archive for this platform, if newer than this launcher
/// ([`own_tag`]: a launcher from before 0.2 takes any `launcher@` release).
pub fn launcher_update(releases: &[Release]) -> Option<Release> {
    launcher_update_from(releases, &own_tag())
}

pub(super) fn launcher_update_from(releases: &[Release], own: &str) -> Option<Release> {
    releases
        .iter()
        .find(|r| r.asset.is_some())
        .filter(|r| compare_versions(&r.tag, own).is_gt())
        .cloned()
}

/// Load the game's release list and both update kinds from the same GitHub response.
/// Publish the game releases before any fallback lookups for older launcher tags, so those
/// lookups cannot hold up the release pages or opening animation. A list error reaches both.
pub fn releases_and_updates(
    on_releases: impl FnOnce(Result<Vec<Release>, String>),
) -> Result<Updates, String> {
    releases_and_updates_with(release_list, installed, launcher_releases_in, on_releases)
}

pub(super) fn releases_and_updates_with(
    load: impl FnOnce() -> Result<serde_json::Value, String>,
    installed: impl FnOnce() -> Vec<String>,
    launchers: impl FnOnce(&serde_json::Value) -> Vec<Release>,
    on_releases: impl FnOnce(Result<Vec<Release>, String>),
) -> Result<Updates, String> {
    let list = match load() {
        Ok(list) => list,
        Err(e) => {
            on_releases(Err(e.clone()));
            return Err(e);
        }
    };
    let releases = game_releases(&list);
    let installed_versions = installed();
    let eclipse = eclipse_update(&installable_updates(&releases), &installed_versions)
        .or_else(|| architecture_update(&releases, &installed_versions));
    on_releases(Ok(releases));
    Ok(Updates {
        eclipse,
        launcher: launcher_update(&launchers(&list)),
    })
}

/// A fresh release-list request for both kinds of update (plus older-launcher fallback if needed).
pub fn check_updates() -> Result<Updates, String> {
    releases_and_updates(|_| {})
}

/// If the selected installed game has the wrong architecture, offer the same release again so
/// the preference also applies when there is no newer game version.
pub(super) fn architecture_update(releases: &[Release], installed: &[String]) -> Option<Release> {
    if !can_choose_game_architecture() {
        return None;
    }
    let settings = Settings::load();
    let tag = settings
        .default_version
        .filter(|tag| installed.contains(tag))
        .or_else(|| installed.iter().find(|tag| is_game_tag(tag)).cloned())?;
    if !is_game_tag(&tag) {
        return None;
    }
    let release = releases.iter().find(|release| release.tag == tag)?;
    needs_game_architecture_install(&tag, release).then(|| release.clone())
}
