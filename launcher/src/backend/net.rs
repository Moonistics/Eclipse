use super::VERSION;
use std::io::Read;
use std::time::Duration;

const GITHUB_API: &str = "https://api.github.com/";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetError {
    url: String,
    failure: Failure,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Failure {
    Status(u16),
    RateLimited,
    Transport(String),
}

impl NetError {
    pub fn new(url: &str, e: ureq::Error) -> Self {
        let failure = match e {
            ureq::Error::Status(code, r)
                if url.starts_with(GITHUB_API)
                    && rate_limited(code, |h| r.header(h).map(str::to_owned)) =>
            {
                Failure::RateLimited
            }
            ureq::Error::Status(code, _) => Failure::Status(code),
            ureq::Error::Transport(t) => Failure::Transport(t.to_string()),
        };
        Self {
            url: url.to_owned(),
            failure,
        }
    }

    pub fn status(&self) -> Option<u16> {
        match self.failure {
            Failure::Status(code) => Some(code),
            _ => None,
        }
    }

    pub fn is_not_found(&self) -> bool {
        self.status() == Some(404)
    }

    pub fn is_rate_limited(&self) -> bool {
        self.failure == Failure::RateLimited
    }
}

impl std::fmt::Display for NetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use crate::i18n::trf;
        let host = host_name(&self.url);
        let (plain, detail) = match &self.failure {
            Failure::RateLimited => {
                return f.write_str("GitHub's rate limit was reached. Try again in a few minutes.")
            }
            Failure::Status(code) => (
                trf(
                    "{host} isn't answering right now. Try again later.",
                    &[("host", host)],
                ),
                format!("{} answered {code}", self.url),
            ),
            Failure::Transport(detail) => {
                let plain = if detail.to_lowercase().contains("certificate") {
                    trf(
                        "{host}'s certificate couldn't be checked. A firewall or proxy may be in the way.",
                        &[("host", host)],
                    )
                } else {
                    trf(
                        "Couldn't reach {host}. Check your internet connection.",
                        &[("host", host)],
                    )
                };
                (plain, detail.clone())
            }
        };
        write!(f, "{plain}\n{detail}")
    }
}

impl std::error::Error for NetError {}

impl From<NetError> for String {
    fn from(e: NetError) -> Self {
        e.to_string()
    }
}

fn host_name(url: &str) -> &str {
    let host = url
        .split("://")
        .nth(1)
        .and_then(|rest| rest.split(['/', ':']).next())
        .unwrap_or(url);
    if host == "github.com"
        || host.ends_with(".github.com")
        || host.ends_with(".githubusercontent.com")
    {
        "GitHub"
    } else {
        host
    }
}

pub fn network_error(url: &str, e: ureq::Error) -> String {
    NetError::new(url, e).to_string()
}

/// The launcher's own user agent, through the player's proxy when one is set (`proxy`).
fn builder(url: &str) -> ureq::AgentBuilder {
    crate::proxy::agent(
        ureq::AgentBuilder::new().user_agent(&format!("eclipse-launcher/{VERSION}")),
        url,
    )
}

/// The agent `url` is fetched with. A connection that goes quiet fails instead of waiting for ever.
pub(crate) fn agent(url: &str) -> ureq::Agent {
    builder(url)
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(60))
        .build()
}

pub(crate) fn agent_within(url: &str, timeout: Duration) -> ureq::Agent {
    builder(url).timeout(timeout).build()
}

/// A file's body, from the network as the launcher reaches it.
pub fn get_bytes(url: &str) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    get(url)?
        .into_reader()
        .read_to_end(&mut body)
        .map_err(|e| e.to_string())?;
    Ok(body)
}

pub(crate) fn get(url: &str) -> Result<ureq::Response, NetError> {
    request(url).call().map_err(|e| NetError::new(url, e))
}

/// A GET of `url`, with `ECLIPSE_GITHUB_TOKEN` when it is GitHub's API: CI runners share addresses
/// and hit GitHub's anonymous rate limit, so they pass their own token.
pub(crate) fn request(url: &str) -> ureq::Request {
    let request = agent(url).get(url);
    match std::env::var("ECLIPSE_GITHUB_TOKEN") {
        Ok(token) if !token.is_empty() && url.starts_with(GITHUB_API) => {
            request.set("Authorization", &format!("Bearer {token}"))
        }
        _ => request,
    }
}

/// Whether a status with these headers is GitHub's rate limit: always a 429; a 403 only with
/// `x-ratelimit-remaining: 0` (the hourly limit) or `retry-after` (the secondary one), since a 403
/// can also be a refusal.
pub(crate) fn rate_limited(code: u16, header: impl Fn(&str) -> Option<String>) -> bool {
    code == 429
        || (code == 403
            && (header("x-ratelimit-remaining").as_deref() == Some("0")
                || header("retry-after").is_some()))
}

/// A small release file (its checksums or their signature), whole; None when it has none.
pub(crate) fn fetch_small(url: &str) -> Result<Option<Vec<u8>>, String> {
    let response = match get(url) {
        Ok(response) => response,
        Err(e) if e.is_not_found() => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let mut body = Vec::new();
    response
        .into_reader()
        .take(1 << 20)
        .read_to_end(&mut body)
        .map_err(|e| format!("Download interrupted: {e}"))?;
    Ok(Some(body))
}
