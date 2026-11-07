use super::net::get;
use super::REPO;

/// The community, sponsor and website links and the official server's address, from
/// `launcher/links.json` on main, so they can change without a launcher release. An empty address
/// hides its button (an empty `official_server`: no official server offered); all are empty until
/// links.json names them.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct Links {
    pub discord: String,
    pub sponsor: String,
    pub website: String,
    /// The official server's address, as servers.dat lists it (`server_list::add_official`).
    pub official_server: String,
    /// The addresses it had before: a list holding one moves to `official_server`. A move appends
    /// the old address here.
    pub official_server_previous: Vec<String>,
}

impl Links {
    /// Reads links.json. A key that is missing keeps its default, an empty one hides the link,
    /// and text that is not a JSON object gives the defaults.
    pub fn parse(text: &str) -> Self {
        let mut links = Self::default();
        if let Ok(serde_json::Value::Object(map)) = serde_json::from_str(text) {
            for (key, slot) in [
                ("discord", &mut links.discord),
                ("sponsor", &mut links.sponsor),
                ("website", &mut links.website),
                ("official_server", &mut links.official_server),
            ] {
                if let Some(url) = map.get(key).and_then(|v| v.as_str()) {
                    *slot = url.trim().to_string();
                }
            }
            // A list of addresses; anything else keeps the built-in one.
            if let Some(serde_json::Value::Array(previous)) = map.get("official_server_previous") {
                if previous.iter().all(serde_json::Value::is_string) {
                    links.official_server_previous = previous
                        .iter()
                        .filter_map(|a| a.as_str())
                        .map(|a| a.trim().to_string())
                        .filter(|a| !a.is_empty())
                        .collect();
                }
            }
        }
        links
    }
}

/// The links from main, or the built-in ones when offline.
pub fn links() -> Links {
    let url = format!("https://raw.githubusercontent.com/{REPO}/main/launcher/links.json");
    get(&url)
        .ok()
        .and_then(|r| r.into_string().ok())
        .map_or_else(Links::default, |text| Links::parse(&text))
}
