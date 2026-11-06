use super::architecture::{game_assets_for, native_windows_architecture};
use super::platform::{ASSET, GAME_FALLBACK};
use super::self_update::launcher_asset;
use super::settings::Settings;
use super::REPO;

/// Launcher releases are tagged `launcher@X.Y.Z` from 0.2 on (docs/versioning.md), and Eclipse's
/// own `vX.Y.Z`. Before 0.2 they were `launcher-vX.Y.Z` and `vYYYY.MM.DD[.N]`: every 0.2 tag is
/// newer than every one of those ([`release_order`]).
pub const LAUNCHER_TAG_PREFIX: &str = "launcher@";
/// The launcher tags from before 0.2 (`launcher-v0.16.0`).
pub const OLD_LAUNCHER_TAG_PREFIX: &str = "launcher-v";

#[derive(Clone, Debug, serde::Serialize)]
pub struct Release {
    pub tag: String,
    pub name: String,
    pub body: String,
    pub date: String,
    /// When it was published, `HH:MM` UTC (empty if GitHub didn't say).
    pub time: String,
    pub asset: Option<(String, u64)>,
    /// Listed and installed by hand, never as an update: a GitHub pre-release of the old Alpha
    /// channel (v0.2.1 on), or a Nightly older than the current one.
    pub prerelease: bool,
    /// A Nightly's GitHub tag (`nightly`, or `nightly-<date>` for the last week's): its files and
    /// signature are there, while `tag` is the version it installs as (`nightly-2026-10-28-0c02067`).
    /// None for a release.
    pub nightly: Option<String>,
}

/// A Eclipse game tag: `v` then a digit (`v2026.10.13`). Launcher tags never match.
pub fn is_game_tag(tag: &str) -> bool {
    tag.strip_prefix('v')
        .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
}

/// The launcher version in a `launcher@X.Y.Z` (or `launcher-vX.Y.Z`) tag, or None for any other tag.
pub fn launcher_version(tag: &str) -> Option<&str> {
    tag.strip_prefix(LAUNCHER_TAG_PREFIX)
        .or_else(|| tag.strip_prefix(OLD_LAUNCHER_TAG_PREFIX))
        .filter(|v| v.starts_with(|c: char| c.is_ascii_digit()))
}

/// The tag this launcher was released as. launcher.yml gives it (`ECLIPSE_LAUNCHER_TAG`); a release
/// built before it did gave only the version, of a `launcher-v` tag; a build from source is
/// Cargo.toml's version on the 0.2 line.
pub fn own_tag() -> String {
    own_tag_of(
        option_env!("ECLIPSE_LAUNCHER_TAG"),
        option_env!("ECLIPSE_LAUNCHER_VERSION"),
    )
}

pub(super) fn own_tag_of(tag: Option<&str>, injected: Option<&str>) -> String {
    match (
        tag.filter(|t| !t.is_empty()),
        injected.filter(|v| !v.is_empty()),
    ) {
        (Some(tag), _) => tag.to_string(),
        (None, Some(version)) => format!("{OLD_LAUNCHER_TAG_PREFIX}{version}"),
        (None, None) => format!("{LAUNCHER_TAG_PREFIX}{}", env!("CARGO_PKG_VERSION")),
    }
}

/// Where a tag or version stands among Eclipse's releases (docs/versioning.md): the tags from
/// before 0.2 (`vYYYY.MM.DD[.N]`, `launcher-vX.Y.Z`) and bare versions first, compared number by
/// number with missing parts as zero and anything after a `-` ignored, as launchers always did;
/// then the 0.2 tags (`vX.Y.Z`, `launcher@X.Y.Z`) in semver's order, a pre-release
/// (`v0.3.0-rc.1`) before its release.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReleaseOrder {
    /// Numbers without trailing zeros, so 1.2 and 1.2.0 are equal.
    Before02(Vec<u64>),
    Semver([u64; 3], Prerelease),
    /// A Nightly (`nightly-<date>-<commit>`), by its date: newer than every release, since the
    /// Nightly is the only build Eclipse ships.
    Nightly(String),
}

/// (is a release, the pre-release's identifiers): a release follows every pre-release of its
/// version; identifiers compare one by one, numbers before words, a shorter list first.
pub type Prerelease = (bool, Vec<Identifier>);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Identifier {
    Number(u64),
    Word(String),
}

/// Semver's majors stay below this; a date tag's first number is a year.
const FIRST_YEAR: u64 = 1000;

pub fn release_order(tag: &str) -> ReleaseOrder {
    if let Some(date) = crate::nightly::date_of(tag) {
        return ReleaseOrder::Nightly(date.to_owned());
    }
    let semver = tag
        .strip_prefix(LAUNCHER_TAG_PREFIX)
        .or_else(|| tag.strip_prefix('v'))
        .and_then(semver_order);
    semver.unwrap_or_else(|| {
        let v = tag
            .strip_prefix(OLD_LAUNCHER_TAG_PREFIX)
            .or_else(|| tag.strip_prefix(LAUNCHER_TAG_PREFIX))
            .unwrap_or(tag);
        let v = v.strip_prefix('v').unwrap_or(v);
        let mut parts: Vec<u64> = v
            .split('-')
            .next()
            .unwrap_or("")
            .split('.')
            .map(|p| p.parse().unwrap_or(0))
            .collect();
        while parts.last() == Some(&0) {
            parts.pop();
        }
        ReleaseOrder::Before02(parts)
    })
}

/// `MAJOR.MINOR.PATCH[-PRE]` with a major below [`FIRST_YEAR`], or None.
fn semver_order(version: &str) -> Option<ReleaseOrder> {
    let (core, pre) = match version.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (version, None),
    };
    let numbers: Vec<u64> = core
        .split('.')
        .map(|n| n.parse().ok())
        .collect::<Option<_>>()?;
    let &[major, minor, patch] = numbers.as_slice() else {
        return None;
    };
    if major >= FIRST_YEAR {
        return None;
    }
    let pre = match pre {
        None => (true, Vec::new()),
        Some(pre) => (
            false,
            pre.split('.')
                .map(|id| {
                    (!id.is_empty()).then(|| {
                        id.parse()
                            .map_or_else(|_| Identifier::Word(id.to_string()), Identifier::Number)
                    })
                })
                .collect::<Option<_>>()?,
        ),
    };
    Some(ReleaseOrder::Semver([major, minor, patch], pre))
}

pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    release_order(a).cmp(&release_order(b))
}

/// Every published release of the repository, both kinds, as GitHub's JSON (kept on disk with its
/// ETag: [`crate::github`]).
pub(super) fn release_list() -> Result<serde_json::Value, String> {
    crate::github::json(&format!(
        "https://api.github.com/repos/{REPO}/releases?per_page=100"
    ))
}

/// The releases in `list` whose tag passes `keep`, newest first, with the asset named `asset`.
pub(crate) fn parse_releases(
    list: &serde_json::Value,
    asset: Option<&str>,
    keep: fn(&str) -> bool,
) -> Vec<Release> {
    let mut out = Vec::new();
    for r in list.as_array().into_iter().flatten() {
        let tag = r["tag_name"].as_str().unwrap_or_default();
        if r["draft"].as_bool().unwrap_or(false) || !keep(tag) {
            continue;
        }
        let found = r["assets"].as_array().into_iter().flatten().find_map(|a| {
            (Some(a["name"].as_str()?) == asset).then(|| {
                Some((
                    a["browser_download_url"].as_str()?.to_string(),
                    a["size"].as_u64()?,
                ))
            })?
        });
        out.push(Release {
            tag: tag.into(),
            name: r["name"].as_str().unwrap_or_default().into(),
            body: r["body"].as_str().unwrap_or_default().replace("\r\n", "\n"),
            date: r["published_at"]
                .as_str()
                .unwrap_or_default()
                .chars()
                .take(10)
                .collect(),
            time: r["published_at"]
                .as_str()
                .and_then(|d| d.get(11..16))
                .unwrap_or_default()
                .into(),
            asset: found,
            prerelease: r["prerelease"].as_bool().unwrap_or(false),
            nightly: None,
        });
    }
    out.sort_by(|a, b| compare_versions(&b.tag, &a.tag));
    out
}

pub(super) fn is_launcher_tag(tag: &str) -> bool {
    launcher_version(tag).is_some()
}

/// Published Eclipse builds, newest first: the current Nightly (the only build Eclipse ships since
/// 2026-10-28, docs/release-flow.md), the last week's Nightlies, then the releases from before.
/// Drafts never reach the anonymous API, and launcher releases are left out.
pub fn releases() -> Result<Vec<Release>, String> {
    Ok(game_releases(&release_list()?))
}

/// The game's builds in `list`, newest first. A Nightly is listed under the version it installs as
/// ([`crate::nightly::version_of`]); only the current one (`nightly`) is an update.
pub(crate) fn game_releases(list: &serde_json::Value) -> Vec<Release> {
    let settings = Settings::load();
    let assets = game_assets_for(
        std::env::consts::OS,
        native_windows_architecture(),
        std::env::consts::ARCH,
        settings.game_architecture,
    );
    let (asset, fallback) = assets.map_or((ASSET, GAME_FALLBACK), |(asset, fallback)| {
        (Some(asset), fallback)
    });
    game_releases_with(list, asset, fallback)
}

/// [`game_releases`] for one platform's asset, and the one it installs from a release that has
/// none of its own ([`GAME_FALLBACK`]).
pub(super) fn game_releases_with(
    list: &serde_json::Value,
    asset: Option<&str>,
    fallback: Option<&str>,
) -> Vec<Release> {
    let keep = |tag: &str| is_game_tag(tag) || eclipse_release_sign::is_nightly(tag);
    let mut out = parse_releases(list, asset, keep);
    if fallback.is_some() {
        // The same releases in the same order: only the asset looked for differs.
        for (r, other) in out.iter_mut().zip(parse_releases(list, fallback, keep)) {
            if r.asset.is_none() && r.tag == other.tag {
                r.asset = other.asset;
            }
        }
    }
    for r in &mut out {
        if eclipse_release_sign::is_nightly(&r.tag) {
            let version = crate::nightly::version_of(r);
            let tag = std::mem::replace(&mut r.tag, version);
            r.prerelease = tag != eclipse_release_sign::NIGHTLY;
            r.nightly = Some(tag);
        }
    }
    // Two Nightlies of one day: the current one first (the dated one is the day's earlier build).
    out.sort_by(|a, b| compare_versions(&b.tag, &a.tag).then(a.prerelease.cmp(&b.prerelease)));
    out
}

/// The launcher releases in `list` (the newest 100 releases). Game releases come out several times
/// a day, so the newest launcher release can be older than all of those: then the newest
/// `launcher@` and `launcher-v` tags are looked up one by one (a draft's tag is skipped: it has no
/// public release).
pub(super) fn launcher_releases_in(list: &serde_json::Value) -> Vec<Release> {
    launcher_releases_in_with(list, |url| crate::github::json(&url))
}

pub(super) fn launcher_releases_in_with(
    list: &serde_json::Value,
    json: impl Fn(String) -> Result<serde_json::Value, String>,
) -> Vec<Release> {
    let found = parse_releases(list, launcher_asset(), is_launcher_tag);
    if found.iter().any(|r| r.asset.is_some()) {
        return found;
    }
    let api = format!("https://api.github.com/repos/{REPO}");
    let mut tags = Vec::new();
    for prefix in [LAUNCHER_TAG_PREFIX, OLD_LAUNCHER_TAG_PREFIX] {
        // Every `launcher@` tag is newer than every `launcher-v` one: those only when there is none.
        if tags.is_empty() {
            if let Ok(refs) = json(format!("{api}/git/matching-refs/tags/{prefix}")) {
                tags = newest_launcher_tags(&refs);
            }
        }
    }
    for tag in tags.into_iter().take(3) {
        if let Ok(release) = json(format!("{api}/releases/tags/{tag}")) {
            let one = parse_releases(
                &serde_json::json!([release]),
                launcher_asset(),
                is_launcher_tag,
            );
            if one.iter().any(|r| r.asset.is_some()) {
                return one;
            }
        }
    }
    found
}

/// The launcher tags in GitHub's matching-refs answer, newest first.
pub(super) fn newest_launcher_tags(refs: &serde_json::Value) -> Vec<String> {
    let mut tags: Vec<String> = refs
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| r["ref"].as_str()?.strip_prefix("refs/tags/"))
        .filter(|t| is_launcher_tag(t))
        .map(String::from)
        .collect();
    tags.sort_by(|a, b| compare_versions(b, a));
    tags
}

/// The newest Eclipse release that has a build for this platform.
pub fn latest(releases: &[Release]) -> Option<&Release> {
    releases.iter().find(|r| r.asset.is_some())
}

/// The releases a launcher installs by itself: every one that is published as a release, so
/// GitHub's latest when it comes out. The pre-releases of the old Alpha channel are left out.
pub fn installable_updates(releases: &[Release]) -> Vec<Release> {
    releases.iter().filter(|r| !r.prerelease).cloned().collect()
}

/// How often a launcher left open looks for new releases: they come out several times a day.
pub const CHECK_EVERY: std::time::Duration = std::time::Duration::from_secs(30 * 60);

/// One release's notes: the tag given, or the newest.
pub fn release_notes(tag: Option<&str>) -> Result<Release, String> {
    let releases = releases()?;
    match tag {
        Some(tag) => releases
            .into_iter()
            .find(|r| r.tag == tag)
            .ok_or_else(|| format!("No Eclipse release is tagged {tag}.")),
        None => releases
            .into_iter()
            .next()
            .ok_or_else(|| "No release is published yet.".into()),
    }
}

/// Plain text from markdown, for terminals: headings on their own lines, bullets as "- ",
/// links followed by their address.
pub fn markdown_to_text(md: &str) -> String {
    use pulldown_cmark::{Event, Parser, Tag, TagEnd};
    let mut out = String::new();
    let mut links = Vec::new();
    let mut lists: Vec<Option<u64>> = Vec::new();
    for event in Parser::new(md) {
        match event {
            Event::Start(Tag::Item) => {
                let depth = lists.len().saturating_sub(1);
                out.push_str(&"  ".repeat(depth));
                match lists.last_mut() {
                    Some(Some(n)) => {
                        out.push_str(&format!("{n}. "));
                        *n += 1;
                    }
                    _ => out.push_str("- "),
                }
            }
            Event::Start(Tag::List(start)) => {
                if !out.ends_with('\n') && !out.is_empty() {
                    out.push('\n');
                }
                lists.push(start)
            }
            Event::End(TagEnd::List(_)) => {
                lists.pop();
                if lists.is_empty() {
                    out.push('\n');
                }
            }
            Event::Start(Tag::Link { dest_url, .. }) => links.push(dest_url.to_string()),
            Event::End(TagEnd::Link) => {
                if let Some(url) = links.pop() {
                    out.push_str(&format!(" ({url})"));
                }
            }
            Event::End(TagEnd::Heading(_)) => out.push_str("\n\n"),
            Event::End(TagEnd::Paragraph) => {
                out.push_str(if lists.is_empty() { "\n\n" } else { "\n" })
            }
            Event::End(TagEnd::Item) if !out.ends_with('\n') => out.push('\n'),
            Event::End(TagEnd::CodeBlock) => out.push('\n'),
            Event::Text(t) | Event::Code(t) => out.push_str(&t),
            Event::SoftBreak | Event::HardBreak => out.push('\n'),
            Event::Rule => out.push_str("---\n\n"),
            _ => {}
        }
    }
    out.trim_end().to_string() + "\n"
}
