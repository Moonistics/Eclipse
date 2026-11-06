//! GitHub's REST API, as the launcher reads it: release lists, releases by tag, commits.
//!
//! Without a token GitHub allows 60 API calls an hour per address, and players behind a shared one
//! (a school, a dorm, a carrier's NAT) spend them together. So:
//! - every answer is kept on disk (`<data>/cache/github/`) and read again without asking for
//!   [`FRESH_SECS`]: a launcher started a few times in a row, or several pages asking for the same
//!   list, spend one call. After that the read is a conditional request with the saved ETag. An
//!   anonymous 304 still counts against the hour (measured 2026-11-01), so that only saves the
//!   download; with `ECLIPSE_GITHUB_TOKEN` (CI) a 304 is free;
//! - when GitHub answers with its rate limit (or not at all), Eclipse's own repository is read
//!   through the mirror `ECLIPSE_GITHUB_MIRROR` names, if any, which caches GitHub's answers for
//!   everyone;
//! - when both fail, the last answer kept on disk is used, else the error says what happened.
//!
//! Release files are not read here: their `browser_download_url` (github.com, then
//! release-assets.githubusercontent.com) is not part of the API and has no such limit. Nor do a
//! commit's files ([`raw`]), kept on disk for good since they never change.
use crate::backend;
use sha2::Digest;
use std::fs;
use std::path::{Path, PathBuf};

const API: &str = "https://api.github.com";

/// How long a saved answer is used without asking again: the launcher looks for releases every
/// [`backend::CHECK_EVERY`], so a few minutes late at worst.
pub const FRESH_SECS: u64 = 5 * 60;

/// What one source answered.
#[derive(Debug)]
pub(crate) enum Answer {
    /// 200, with the ETag GitHub gave (none from the mirror).
    Fresh(serde_json::Value, Option<String>),
    /// 304: the copy on disk is still current.
    NotModified,
    NotFound,
    /// 429, or 403 with GitHub's rate-limit headers.
    RateLimited,
    /// Any other status, or no connection: the error to show.
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    NotFound(String),
    RateLimited(String),
    Failed(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(url) => write!(f, "{url} answered 404"),
            Self::RateLimited(why) | Self::Failed(why) => f.write_str(why),
        }
    }
}

impl From<Error> for String {
    fn from(error: Error) -> Self {
        error.to_string()
    }
}

/// GitHub's JSON for API path `path` (`repos/<owner>/<repo>/...`).
pub fn api(path: &str) -> Result<serde_json::Value, Error> {
    get(&format!("{API}/{path}"))
}

/// GitHub's JSON for `url` (`https://api.github.com/...`). A 404 is the error `<url> answered 404`.
pub fn json(url: &str) -> Result<serde_json::Value, String> {
    get(url).map_err(String::from)
}

fn get(url: &str) -> Result<serde_json::Value, Error> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let mirror_base = std::env::var("ECLIPSE_GITHUB_MIRROR").unwrap_or_default();
    json_with(url, &cache_dir(), now, &mirror_base, github, mirror)
}

/// GitHub's archive (`.tar.gz`) of `repo` at `revision`, unpacked into the fresh folder
/// `partial`: the archive's one top folder (`<repo>-<commit>/`), which is returned. Links are left
/// out; at most `limit` bytes are unpacked.
pub fn unpack_archive(
    repo: &str,
    revision: &str,
    partial: &Path,
    limit: u64,
) -> Result<PathBuf, String> {
    use std::io::Read;
    let url = format!("https://codeload.github.com/{repo}/tar.gz/{revision}");
    let _ = fs::remove_dir_all(partial);
    fs::create_dir_all(partial).map_err(|e| e.to_string())?;
    let reader = flate2::read::GzDecoder::new(backend::get(&url)?.into_reader()).take(limit);
    let mut tar = tar::Archive::new(reader);
    for entry in tar.entries().map_err(|e| e.to_string())? {
        let mut entry = entry.map_err(|e| format!("Download interrupted: {e}"))?;
        let kind = entry.header().entry_type();
        if !(kind.is_file() || kind.is_dir()) {
            continue;
        }
        entry
            .unpack_in(partial)
            .map_err(|e| format!("Could not unpack the source: {e}"))?;
    }
    fs::read_dir(partial)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.is_dir())
        .ok_or_else(|| "The source archive is empty.".into())
}

/// The text of `file` in `repo` at commit `revision` (40 hex digits), or None when it has no such
/// file. Kept on disk once read: a commit's files never change, so it is read once, offline after.
pub fn raw(repo: &str, revision: &str, file: &str) -> Result<Option<String>, Error> {
    raw_with(
        repo,
        revision,
        file,
        &cache_dir().join("raw"),
        |url| match backend::get(url) {
            Ok(r) => r
                .into_string()
                .map(Some)
                .map_err(|e| Error::Failed(e.to_string())),
            Err(e) if e.is_not_found() => Ok(None),
            Err(e) => Err(Error::Failed(e.to_string())),
        },
    )
}

fn raw_with(
    repo: &str,
    revision: &str,
    file: &str,
    cache: &Path,
    fetch: impl Fn(&str) -> Result<Option<String>, Error>,
) -> Result<Option<String>, Error> {
    let url = format!("https://raw.githubusercontent.com/{repo}/{revision}/{file}");
    let kept = cache.join(cache_name(&url)).with_extension("txt");
    if let Ok(text) = fs::read_to_string(&kept) {
        return Ok(Some(text));
    }
    let text = fetch(&url)?;
    if let Some(text) = &text {
        if eclipse_mods::index::native::revision(revision) {
            let _ = crate::files::write(&kept, text.as_bytes());
        }
    }
    Ok(text)
}

/// [`json`] with its cache folder, the time (Unix seconds), the mirror and its two sources given
/// (tests).
pub(crate) fn json_with(
    url: &str,
    cache: &Path,
    now: u64,
    mirror_base: &str,
    github: impl Fn(&str, Option<&str>) -> Answer,
    mirror: impl Fn(&str) -> Answer,
) -> Result<serde_json::Value, Error> {
    let file = cache.join(cache_name(url));
    let kept = read_kept(&file, url);
    if let Some(k) = kept.as_ref() {
        // A copy saved "later" than now (the clock was set back) is not trusted as fresh.
        if now >= k.saved && now - k.saved < FRESH_SECS {
            return Ok(k.body.clone());
        }
    }
    let etag = kept
        .as_ref()
        .map(|k| k.etag.as_str())
        .filter(|e| !e.is_empty());
    let (limited, why) = match github(url, etag) {
        Answer::Fresh(body, etag) => {
            write_kept(&file, url, etag.as_deref().unwrap_or(""), &body, now);
            return Ok(body);
        }
        Answer::NotModified => match kept {
            Some(k) => {
                write_kept(&file, url, &k.etag, &k.body, now);
                return Ok(k.body);
            }
            // A 304 to a request that had no ETag: can't happen, but don't trust it.
            None => (false, "GitHub answered 304 without a saved copy".to_owned()),
        },
        Answer::NotFound => return Err(Error::NotFound(url.to_owned())),
        Answer::RateLimited => (true, RATE_LIMITED.to_owned()),
        Answer::Failed(e) => (false, e),
    };
    let from_mirror = match mirror_url_with(url, mirror_base) {
        Some(m) => match mirror(&m) {
            Answer::Fresh(body, _) => {
                write_kept(&file, url, "", &body, now);
                return Ok(body);
            }
            Answer::NotFound => return Err(Error::NotFound(url.to_owned())),
            Answer::RateLimited => Some("too many requests from this network".to_owned()),
            Answer::Failed(e) => Some(e),
            Answer::NotModified => Some("an unexpected 304".to_owned()),
        },
        None => None,
    };
    if let Some(k) = kept {
        return Ok(k.body);
    }
    let why = match from_mirror {
        Some(m) => format!(
            "{why}, and Eclipse's mirror of GitHub couldn't help either ({m}). Try again in a few minutes."
        ),
        None => why,
    };
    Err(match limited {
        true => Error::RateLimited(why),
        false => Error::Failed(why),
    })
}

const RATE_LIMITED: &str = "GitHub's rate limit was reached for your network";

/// The mirror's URL for an API URL it serves (Eclipse's own repository), or None; an empty `base`
/// is no mirror.
fn mirror_url_with(url: &str, base: &str) -> Option<String> {
    let path = url.strip_prefix(API)?;
    let repo = format!("/repos/{}/", backend::REPO);
    (!base.is_empty() && path.starts_with(&repo))
        .then(|| format!("{}{path}", base.trim_end_matches('/')))
}

/// GitHub itself, with the saved ETag when there is one.
fn github(url: &str, etag: Option<&str>) -> Answer {
    let mut request = backend::request(url);
    if let Some(etag) = etag {
        request = request.set("If-None-Match", etag);
    }
    match request.call() {
        Ok(r) if r.status() == 304 => Answer::NotModified,
        Ok(r) => {
            let etag = r.header("ETag").map(str::to_owned);
            match r.into_json() {
                Ok(body) => Answer::Fresh(body, etag),
                Err(e) => Answer::Failed(e.to_string()),
            }
        }
        Err(e) => failure(url, e),
    }
}

fn mirror(url: &str) -> Answer {
    match backend::agent(url).get(url).call() {
        Ok(r) => match r.into_json() {
            Ok(body) => Answer::Fresh(body, None),
            Err(e) => Answer::Failed(e.to_string()),
        },
        Err(e) => failure(url, e),
    }
}

fn failure(url: &str, e: ureq::Error) -> Answer {
    match e {
        ureq::Error::Status(404, _) => Answer::NotFound,
        ureq::Error::Status(code, r)
            if backend::rate_limited(code, |h| r.header(h).map(str::to_owned)) =>
        {
            Answer::RateLimited
        }
        other => Answer::Failed(backend::network_error(url, other)),
    }
}

fn cache_dir() -> PathBuf {
    backend::data_dir().join("cache").join("github")
}

fn cache_name(url: &str) -> String {
    let hash = sha2::Sha256::digest(url.as_bytes());
    let hex: String = hash[..12].iter().map(|b| format!("{b:02x}")).collect();
    format!("{hex}.json")
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Kept {
    url: String,
    /// GitHub's ETag, or empty (the mirror's answers, or GitHub gave none).
    etag: String,
    /// When it was saved or last confirmed (Unix seconds).
    saved: u64,
    body: serde_json::Value,
}

fn read_kept(file: &Path, url: &str) -> Option<Kept> {
    let kept: Kept = serde_json::from_slice(&fs::read(file).ok()?).ok()?;
    (kept.url == url).then_some(kept)
}

/// Saves an answer for the next reads; a failure only costs that saving.
fn write_kept(file: &Path, url: &str, etag: &str, body: &serde_json::Value, saved: u64) {
    let kept = Kept {
        url: url.to_owned(),
        etag: etag.to_owned(),
        saved,
        body: body.clone(),
    };
    if let Ok(text) = serde_json::to_vec(&kept) {
        let _ = crate::files::write(file, &text);
    }
}

#[cfg(test)]
#[path = "github/tests.rs"]
mod tests;
