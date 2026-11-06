//! Microsoft account sign-in (docs/microsoft-account.md): the OAuth device-code flow, then Xbox
//! Live, XSTS and the Minecraft services, the game ownership check and the profile. Secrets (the
//! Microsoft refresh token, the Minecraft access token, the last ownership answer) live in the OS
//! keychain, or in a file only the player can read where there is none; accounts.json keeps only
//! names and UUIDs. The game gets the session in a file it
//! reads once (`--session-file`), never on its command line. Tokens never reach logs, errors or
//! output.
//!
//! The owner gate ([`launch`]): the launcher starts the game only while an account in it owns
//! Minecraft: Java Edition, as other launchers do. Offline names play behind such an account.
//! The game binary itself, its harnesses and the servers don't go through the launcher's gate.
//!
//! The Azure app (client ID) is Eclipse's own and is configuration: `ECLIPSE_MSA_CLIENT_ID` at
//! build time, or at run time to override it. Without one, sign-in says it is not configured.
pub mod session;

pub use crate::secrets::{no_keychain, store, SecretStore};

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const SCOPE: &str = "XboxLive.signin offline_access";
pub const NOT_CONFIGURED: &str =
    "Microsoft sign-in isn't available in this copy of the launcher yet.";
/// What every Play says while no account in the launcher owns the game.
pub const NEEDS_OWNER: &str =
    "Sign in with a Microsoft account that owns Minecraft: Java Edition to play";
pub const DEFAULT_PLAYER: &str = "Player";
const DOES_NOT_OWN: &str = "This account doesn't own Minecraft: Java Edition.";
const NO_PROFILE: &str =
    "This account has no Minecraft profile yet. Create one at minecraft.net, then sign in again.";
const OTHER_PROFILE: &str =
    "The Microsoft account now has another Minecraft profile. Sign in again.";
const SIGN_IN_AGAIN: &str = "Your Microsoft sign-in expired. Sign in again.";
/// The Minecraft token is renewed when it has less than this left.
const RENEW_BEFORE: u64 = 3600;

/// The configured Azure app, if any: the run-time variable wins over the build-time one, and an
/// empty value means none.
pub fn client_id() -> Option<String> {
    std::env::var("ECLIPSE_MSA_CLIENT_ID")
        .ok()
        .or_else(|| option_env!("ECLIPSE_MSA_CLIENT_ID").map(str::to_owned))
        .map(|id| id.trim().to_owned())
        .filter(|id| !id.is_empty())
}

/// Where the requests go: the real services, or a mock in tests.
#[derive(Clone, Debug)]
pub struct Endpoints {
    /// The OAuth 2.0 v2 endpoints for personal accounts (`…/devicecode`, `…/token`).
    pub microsoft: String,
    pub xbox_user: String,
    pub xsts: String,
    /// api.minecraftservices.com.
    pub minecraft: String,
}

impl Endpoints {
    pub fn live() -> Self {
        // Development builds only: `ECLIPSE_MSA_MICROSOFT_ENDPOINT` points the device-code and
        // token requests at a local mock, to see and screenshot the sign-in panel.
        let microsoft = std::env::var("ECLIPSE_MSA_MICROSOFT_ENDPOINT")
            .ok()
            .filter(|_| cfg!(debug_assertions))
            .unwrap_or_else(|| "https://login.microsoftonline.com/consumers/oauth2/v2.0".into());
        Self {
            microsoft,
            xbox_user: "https://user.auth.xboxlive.com/user/authenticate".into(),
            xsts: "https://xsts.auth.xboxlive.com/xsts/authorize".into(),
            minecraft: "https://api.minecraftservices.com".into(),
        }
    }
}

/// A request to `url`: the launcher's user agent, a 20 s timeout, and the player's proxy when one
/// is set (`proxy`), since a sign-in has to get out the way downloads do.
fn request_to(method: &str, url: &str) -> ureq::Request {
    crate::backend::agent_within(url, Duration::from_secs(20)).request(method, url)
}

/// One request's result: the status and the JSON body (empty when there is none). Transport
/// errors name the service, never the URL's query or the request.
fn send(request: ureq::Request, body: Option<Body>, service: &str) -> Result<(u16, Value), String> {
    let result = match body {
        None => request.call(),
        Some(Body::Form(fields)) => request.send_form(&fields),
        Some(Body::Json(value)) => request.set("Accept", "application/json").send_json(value),
    };
    let response = match result {
        Ok(r) => r,
        Err(ureq::Error::Status(_, r)) => r,
        Err(ureq::Error::Transport(_)) => {
            return Err(format!(
                "Could not reach {service}. Check your internet connection."
            ));
        }
    };
    let status = response.status();
    let text = response.into_string().unwrap_or_default();
    Ok((status, serde_json::from_str(&text).unwrap_or(Value::Null)))
}

enum Body<'a> {
    Form(Vec<(&'a str, &'a str)>),
    Json(Value),
}

fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

// ----- The device-code flow ----------------------------------------------------------------------

/// What the player types at the verification page.
#[derive(Clone, PartialEq)]
pub struct DeviceCode {
    pub user_code: String,
    pub verification_uri: String,
    pub interval: u64,
    pub expires_at: u64,
    device_code: String,
}

impl std::fmt::Debug for DeviceCode {
    // The device code can be traded for tokens once the player approves: never shown.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceCode")
            .field("user_code", &self.user_code)
            .field("verification_uri", &self.verification_uri)
            .finish_non_exhaustive()
    }
}

/// The Microsoft tokens; the access token only lasts for the Xbox step.
pub struct MsTokens {
    access: String,
    refresh: String,
}

impl std::fmt::Debug for MsTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MsTokens(<hidden>)")
    }
}

pub enum Poll {
    Pending,
    /// The server asked to poll less often: wait 5 s more.
    SlowDown,
    Done(MsTokens),
}

pub fn device_code(ep: &Endpoints, client_id: &str) -> Result<DeviceCode, String> {
    let (status, v) = send(
        request_to("POST", &format!("{}/devicecode", ep.microsoft)),
        Some(Body::Form(vec![("client_id", client_id), ("scope", SCOPE)])),
        "Microsoft",
    )?;
    match (
        status,
        str_field(&v, "user_code"),
        str_field(&v, "device_code"),
    ) {
        (200, Some(user_code), Some(device_code)) => Ok(DeviceCode {
            user_code,
            verification_uri: str_field(&v, "verification_uri")
                .unwrap_or_else(|| "https://www.microsoft.com/link".into()),
            interval: v
                .get("interval")
                .and_then(Value::as_u64)
                .unwrap_or(5)
                .max(1),
            expires_at: now() + v.get("expires_in").and_then(Value::as_u64).unwrap_or(900),
            device_code,
        }),
        _ => Err(oauth_error(&v)),
    }
}

/// The OAuth error codes as messages for the player.
fn oauth_error(v: &Value) -> String {
    match v.get("error").and_then(Value::as_str) {
        Some("authorization_declined") => "Sign-in was declined in the browser.".into(),
        Some("expired_token") => "The sign-in code expired. Start again.".into(),
        Some("bad_verification_code") => "The sign-in code was not recognised. Start again.".into(),
        Some("invalid_grant") => SIGN_IN_AGAIN.into(),
        Some("invalid_client" | "unauthorized_client") => {
            "Microsoft refused this copy of the launcher's sign-in.".into()
        }
        Some(other) => format!("Microsoft sign-in failed ({other})."),
        None => "Microsoft sign-in failed.".into(),
    }
}

/// One poll of the token endpoint while the player signs in.
pub fn poll(ep: &Endpoints, client_id: &str, code: &DeviceCode) -> Result<Poll, String> {
    let (status, v) = send(
        request_to("POST", &format!("{}/token", ep.microsoft)),
        Some(Body::Form(vec![
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("client_id", client_id),
            ("device_code", &code.device_code),
        ])),
        "Microsoft",
    )?;
    match v.get("error").and_then(Value::as_str) {
        Some("authorization_pending") => return Ok(Poll::Pending),
        Some("slow_down") => return Ok(Poll::SlowDown),
        _ => {}
    }
    ms_tokens(status, &v).map(Poll::Done)
}

fn ms_tokens(status: u16, v: &Value) -> Result<MsTokens, String> {
    match (
        status,
        str_field(v, "access_token"),
        str_field(v, "refresh_token"),
    ) {
        (200, Some(access), Some(refresh)) => Ok(MsTokens { access, refresh }),
        _ => Err(oauth_error(v)),
    }
}

/// Wait for the player to finish in the browser, polling as asked, until the code expires or
/// `cancelled` says stop.
pub fn wait_for_sign_in(
    ep: &Endpoints,
    client_id: &str,
    code: &DeviceCode,
    cancelled: &dyn Fn() -> bool,
) -> Result<MsTokens, String> {
    let mut interval = code.interval;
    loop {
        let until = std::time::Instant::now() + Duration::from_secs(interval);
        while std::time::Instant::now() < until {
            if cancelled() {
                return Err("Sign-in cancelled.".into());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        if now() > code.expires_at {
            return Err("The sign-in code expired. Start again.".into());
        }
        match poll(ep, client_id, code)? {
            Poll::Pending => {}
            Poll::SlowDown => interval += 5,
            Poll::Done(tokens) => return Ok(tokens),
        }
    }
}

fn refresh_ms(ep: &Endpoints, client_id: &str, refresh: &str) -> Result<MsTokens, String> {
    let (status, v) = send(
        request_to("POST", &format!("{}/token", ep.microsoft)),
        Some(Body::Form(vec![
            ("grant_type", "refresh_token"),
            ("client_id", client_id),
            ("refresh_token", refresh),
            ("scope", SCOPE),
        ])),
        "Microsoft",
    )?;
    ms_tokens(status, &v)
}

// ----- Xbox Live, XSTS, Minecraft ----------------------------------------------------------------

/// The XSTS `XErr` codes, as the official launcher explains them.
fn xsts_error(code: u64) -> String {
    match code {
        2148916233 => "This Microsoft account has no Xbox profile yet. Sign in once at xbox.com to create one, then try again.".into(),
        2148916235 => "Xbox Live is not available in your country or region.".into(),
        2148916236 | 2148916237 => "This account needs adult verification on xbox.com before it can play.".into(),
        2148916238 => "This is a child account: an adult has to add it to a Microsoft family first.".into(),
        other => format!("Xbox Live refused the sign-in (error {other})."),
    }
}

/// The Minecraft access token from the Microsoft one, through Xbox Live and XSTS.
struct Minecraft {
    token: String,
    expires_at: u64,
    xuid: Option<String>,
}

fn minecraft_login(
    ep: &Endpoints,
    ms_access: &str,
    progress: &dyn Fn(session::Phase),
) -> Result<Minecraft, String> {
    progress(session::Phase::Xbox);
    let (status, xbl) = send(
        request_to("POST", &ep.xbox_user),
        Some(Body::Json(json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={ms_access}"),
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT",
        }))),
        "Xbox Live",
    )?;
    let xbl_token = str_field(&xbl, "Token")
        .filter(|_| status == 200)
        .ok_or("Xbox Live refused the sign-in.")?;
    let (status, xsts) = send(
        request_to("POST", &ep.xsts),
        Some(Body::Json(json!({
            "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl_token] },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT",
        }))),
        "Xbox Live",
    )?;
    if let Some(code) = xsts.get("XErr").and_then(Value::as_u64) {
        return Err(xsts_error(code));
    }
    let claims = xsts.pointer("/DisplayClaims/xui/0");
    let (Some(xsts_token), Some(uhs)) = (
        str_field(&xsts, "Token").filter(|_| status == 200),
        claims.and_then(|c| str_field(c, "uhs")),
    ) else {
        return Err("Xbox Live refused the sign-in.".into());
    };
    let xuid = claims.and_then(|c| str_field(c, "xid"));
    progress(session::Phase::Minecraft);
    let (token, expires_in) = minecraft_token(ep, &format!("XBL3.0 x={uhs};{xsts_token}"))?;
    Ok(Minecraft {
        token,
        expires_at: now() + expires_in,
        xuid,
    })
}

/// The Minecraft access token and its lifetime from the XSTS one (`XBL3.0 x=<uhs>;<token>`).
/// `launcher/login` first: a new account's token from `login_with_xbox` names no profile, so
/// joining an online-mode server answers "Profile not found" (found and diagnosed by
/// martipops). `login_with_xbox` stays the fallback, so a sign-in that worked before still does
/// when the other refuses.
fn minecraft_token(ep: &Endpoints, xtoken: &str) -> Result<(String, u64), String> {
    let lifetime = |v: &Value| v.get("expires_in").and_then(Value::as_u64).unwrap_or(86400);
    let launcher = send(
        request_to("POST", &format!("{}/launcher/login", ep.minecraft)),
        Some(Body::Json(
            json!({ "xtoken": xtoken, "platform": "PC_LAUNCHER" }),
        )),
        "the Minecraft services",
    );
    if let Ok((200, v)) = &launcher {
        if let Some(token) = str_field(v, "access_token") {
            return Ok((token, lifetime(v)));
        }
    }
    let (status, mc) = send(
        request_to(
            "POST",
            &format!("{}/authentication/login_with_xbox", ep.minecraft),
        ),
        Some(Body::Json(json!({ "identityToken": xtoken }))),
        "the Minecraft services",
    )?;
    match (status, str_field(&mc, "access_token")) {
        (200, Some(token)) => Ok((token, lifetime(&mc))),
        // New Azure apps are refused until Mojang approves them (Minecraft API access).
        (403, _) => {
            Err("Mojang hasn't approved this launcher for sign-in yet. Try again later.".into())
        }
        (429, _) => Err("Too many sign-ins in a short time. Wait a minute and try again.".into()),
        _ => Err("The Minecraft services refused the sign-in.".into()),
    }
}

fn bearer(request: ureq::Request, token: &str) -> ureq::Request {
    request.set("Authorization", &format!("Bearer {token}"))
}

/// Whether the account owns Minecraft: Java Edition (Game Pass included).
fn owns_game(ep: &Endpoints, token: &str) -> Result<bool, String> {
    let (status, v) = send(
        bearer(
            request_to("GET", &format!("{}/entitlements/mcstore", ep.minecraft)),
            token,
        ),
        None,
        "the Minecraft services",
    )?;
    if status != 200 {
        return Err("Could not check which games this account owns.".into());
    }
    Ok(v.get("items")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|i| {
                matches!(
                    i.get("name").and_then(Value::as_str),
                    Some("game_minecraft" | "product_minecraft")
                )
            })
        }))
}

/// A Minecraft profile, as the launcher shows and remembers it (no secrets).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Account {
    /// The profile UUID without dashes.
    pub uuid: String,
    pub name: String,
    /// The active skin's URL (textures.minecraft.net) and model ("classic" or "slim").
    #[serde(default)]
    pub skin: Option<(String, String)>,
    #[serde(default)]
    pub cape: Option<String>,
}

/// The profile; none when the account has none (it doesn't own the game, or never made one).
fn profile(ep: &Endpoints, token: &str) -> Result<Option<Account>, String> {
    let (status, v) = send(
        bearer(
            request_to("GET", &format!("{}/minecraft/profile", ep.minecraft)),
            token,
        ),
        None,
        "the Minecraft services",
    )?;
    if status == 404 {
        return Ok(None);
    }
    let (Some(uuid), Some(name)) = (str_field(&v, "id"), str_field(&v, "name")) else {
        return Err("Could not read the Minecraft profile.".into());
    };
    let active = |key: &str| {
        v.get(key)
            .and_then(Value::as_array)
            .and_then(|list| {
                list.iter()
                    .find(|s| s.get("state").and_then(Value::as_str) == Some("ACTIVE"))
            })
            .cloned()
    };
    Ok(Some(Account {
        uuid: uuid.replace('-', "").to_lowercase(),
        name,
        skin: active("skins").and_then(|s| {
            Some((
                str_field(&s, "url")?,
                str_field(&s, "variant")
                    .unwrap_or_else(|| "CLASSIC".into())
                    .to_lowercase(),
            ))
        }),
        cape: active("capes").and_then(|c| str_field(&c, "url")),
    }))
}

/// What the keychain keeps for one account.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Secrets {
    refresh_token: String,
    #[serde(default)]
    sign_in_required: bool,
    minecraft_token: String,
    minecraft_expires_at: u64,
    #[serde(default)]
    xuid: Option<String>,
    /// The last answer the Minecraft services gave: whether the account owns the game. It is
    /// kept here, beside the tokens, so that no plain file can say yes. Accounts saved before it
    /// existed passed the same check at sign-in, which refuses everyone else.
    #[serde(default = "owned_at_sign_in")]
    owns: bool,
}
fn owned_at_sign_in() -> bool {
    true
}
impl std::fmt::Debug for Secrets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secrets(<hidden>)")
    }
}

/// From Microsoft tokens to a ready account: Xbox, XSTS, Minecraft, ownership, profile.
pub fn finish_sign_in(ep: &Endpoints, ms: MsTokens) -> Result<(Account, Secrets), String> {
    finish_sign_in_with_progress(ep, ms, &|_| {})
}

pub fn finish_sign_in_with_progress(
    ep: &Endpoints,
    ms: MsTokens,
    progress: &dyn Fn(session::Phase),
) -> Result<(Account, Secrets), String> {
    let mc = minecraft_login(ep, &ms.access, progress)?;
    progress(session::Phase::Ownership);
    if !owns_game(ep, &mc.token)? {
        return Err(DOES_NOT_OWN.into());
    }
    progress(session::Phase::Profile);
    let account = profile(ep, &mc.token)?.ok_or(NO_PROFILE)?;
    Ok((account, Secrets::new(ms.refresh, mc)))
}

impl Secrets {
    fn new(refresh_token: String, mc: Minecraft) -> Self {
        Self {
            refresh_token,
            sign_in_required: false,
            minecraft_token: mc.token,
            minecraft_expires_at: mc.expires_at,
            xuid: mc.xuid,
            owns: true,
        }
    }
}

/// What asking the Minecraft services again found for one account.
enum Check {
    /// It owns the game: its profile as it is now (the name may have changed) and its tokens.
    Owns(Account, Secrets),
    /// The services said it doesn't own the game (or has no profile any more).
    DoesNot(Secrets),
    /// No answer (no network, a sign-in that can't be renewed): the last answer stands.
    NoAnswer(String, Secrets),
}

/// Asks whether `account` owns the game, with a Minecraft token renewed through the refresh token
/// first when it has less than an hour left. The secrets come back renewed whatever the answer.
fn check(ep: &Endpoints, client_id: Option<&str>, account: &Account, secrets: Secrets) -> Check {
    check_with(ep, client_id, account, secrets, false, &|_| {})
}

fn check_with(
    ep: &Endpoints,
    client_id: Option<&str>,
    account: &Account,
    mut secrets: Secrets,
    force: bool,
    progress: &dyn Fn(session::Phase),
) -> Check {
    if secrets.sign_in_required {
        return Check::NoAnswer(SIGN_IN_AGAIN.into(), secrets);
    }
    if force || secrets.minecraft_expires_at <= now() + RENEW_BEFORE {
        progress(session::Phase::Microsoft);
        let renewed = client_id
            .ok_or_else(|| NOT_CONFIGURED.to_string())
            .and_then(|id| refresh_ms(ep, id, &secrets.refresh_token));
        let ms = match renewed {
            Ok(ms) => ms,
            Err(e) => {
                secrets.sign_in_required = e == SIGN_IN_AGAIN;
                return Check::NoAnswer(e, secrets);
            }
        };
        // Microsoft can rotate its refresh credential even if a later service fails.
        secrets.refresh_token = ms.refresh;
        match minecraft_login(ep, &ms.access, progress) {
            Ok(mc) => {
                secrets = Secrets {
                    owns: secrets.owns,
                    ..Secrets::new(secrets.refresh_token, mc)
                }
            }
            Err(e) => return Check::NoAnswer(e, secrets),
        }
    }
    let token = &secrets.minecraft_token;
    progress(session::Phase::Ownership);
    match owns_game(ep, token).and_then(|owns| {
        if !owns {
            return Ok(None);
        }
        progress(session::Phase::Profile);
        profile(ep, token)
    }) {
        Err(e) => Check::NoAnswer(e, secrets),
        Ok(None) => Check::DoesNot(Secrets {
            owns: false,
            ..secrets
        }),
        Ok(Some(p)) if p.uuid != account.uuid => Check::NoAnswer(
            OTHER_PROFILE.into(),
            Secrets {
                sign_in_required: true,
                ..secrets
            },
        ),
        Ok(Some(p)) => Check::Owns(
            p,
            Secrets {
                owns: true,
                ..secrets
            },
        ),
    }
}

// ----- Accounts ------------------------------------------------------------------------------------

/// accounts.json: the signed-in accounts and the one that plays. Never a token.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Accounts {
    pub accounts: Vec<Account>,
    /// The UUID of the account the game starts with; none plays offline.
    pub selected: Option<String>,
    /// Offline names the player keeps, to switch between (`--name`): no account, no skin.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub offline: Vec<String>,
    /// The offline name that plays when no account is selected; none: the name the game made up
    /// (`eclipse-player.txt`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offline_selected: Option<String>,
    /// Each offline name's skin and cape (`skins`), by the name in lowercase.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub looks: std::collections::BTreeMap<String, crate::skins::Look>,
}

/// Who plays, as the quick switch lists them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Player {
    /// A Microsoft account, by UUID.
    Microsoft(String),
    /// An offline name.
    Offline(String),
    /// Offline as the name the game made up.
    GameName,
}

const OFFLINE_NEEDS_OWNER: &str =
    "Sign in once with an account that owns Java Edition to pick your own player name.";

/// The secrets an account keeps in `store`, if it still has them there.
fn secrets_in(store: &dyn SecretStore, uuid: &str) -> Option<Secrets> {
    store
        .get(uuid)
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
}

/// Whether `name` can be an offline name: 3 to 16 letters, digits or `_`, as account names are.
pub fn valid_offline_name(name: &str) -> bool {
    (3..=16).contains(&name.chars().count())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

impl Accounts {
    pub(crate) fn path() -> PathBuf {
        crate::backend::data_dir().join("accounts.json")
    }
    pub fn load() -> Self {
        Self::load_from(&Self::path())
    }
    pub fn save(&self) -> Result<(), String> {
        self.save_to(&Self::path())
    }
    pub(crate) fn load_from(path: &Path) -> Self {
        crate::files::read_json(path)
            .ok()
            .flatten()
            .unwrap_or_default()
    }
    pub(crate) fn save_to(&self, path: &Path) -> Result<(), String> {
        crate::files::read_json::<Self>(path)?;
        crate::files::write_json(path, self)
    }
    pub fn selected(&self) -> Option<&Account> {
        let uuid = self.selected.as_ref()?;
        self.accounts.iter().find(|a| &a.uuid == uuid)
    }
    /// Find an account by name (any case) or UUID (with or without dashes).
    pub fn find(&self, who: &str) -> Option<&Account> {
        let undashed = who.replace('-', "").to_lowercase();
        self.accounts
            .iter()
            .find(|a| a.name.eq_ignore_ascii_case(who) || a.uuid == undashed)
    }
    /// Keep a new or renewed account, its secrets in the store, and select it.
    pub fn add(
        &mut self,
        store: &dyn SecretStore,
        account: Account,
        secrets: &Secrets,
    ) -> Result<(), String> {
        let _operation = session::lock(store)?;
        self.keep_sign_in(store, account, secrets)
    }

    fn keep_sign_in(
        &mut self,
        store: &dyn SecretStore,
        account: Account,
        secrets: &Secrets,
    ) -> Result<(), String> {
        store.set(
            &account.uuid,
            &serde_json::to_string(secrets).map_err(|e| e.to_string())?,
        )?;
        self.selected = Some(account.uuid.clone());
        match self.accounts.iter_mut().find(|a| a.uuid == account.uuid) {
            Some(a) => *a = account,
            None => self.accounts.push(account),
        }
        Ok(())
    }
    /// Whether an account here owned the game at its last check (the keychain's answer, no
    /// request): what Play and offline names need.
    pub fn has_owner(&self, store: &dyn SecretStore) -> bool {
        // The selected account first: it owning the game ends the search after one account's
        // item — every account's item read can ask for the keychain (macOS, #4912).
        self.selected()
            .into_iter()
            .chain(self.accounts.iter().filter(|a| Some(*a) != self.selected()))
            .any(|a| secrets_in(store, &a.uuid).is_some_and(|s| s.owns))
    }

    pub fn player(&self) -> Player {
        match (self.selected(), &self.offline_selected) {
            (Some(a), _) => Player::Microsoft(a.uuid.clone()),
            (None, Some(name)) => Player::Offline(name.clone()),
            (None, None) => Player::GameName,
        }
    }

    /// Plays as `who`: an account, an offline name (kept, only beside an account that owns the
    /// game), or the game's own name.
    pub fn play_as(&mut self, store: &dyn SecretStore, who: Player) -> Result<(), String> {
        self.play_as_with(store, who, false)
    }

    /// [`Self::play_as`], where `offline_without_owner` (the window's hidden developer option,
    /// `backend::Settings::dev_offline`) lets an offline name play without an account that owns
    /// the game.
    pub fn play_as_with(
        &mut self,
        store: &dyn SecretStore,
        who: Player,
        offline_without_owner: bool,
    ) -> Result<(), String> {
        match who {
            Player::Microsoft(uuid) => {
                if !self.accounts.iter().any(|a| a.uuid == uuid) {
                    return Err("That account isn't signed in.".into());
                }
                self.selected = Some(uuid);
            }
            Player::Offline(name) => {
                if !offline_without_owner && !self.has_owner(store) {
                    return Err(OFFLINE_NEEDS_OWNER.into());
                }
                let name = self.add_offline(&name)?;
                self.selected = None;
                self.offline_selected = Some(name);
            }
            Player::GameName => {
                self.selected = None;
                self.offline_selected = None;
            }
        }
        Ok(())
    }

    /// Keeps an offline name (once, in any case); returns it as kept.
    fn add_offline(&mut self, name: &str) -> Result<String, String> {
        let name = name.trim();
        if !valid_offline_name(name) {
            return Err(format!(
                "{name} can't be used: offline names are 3 to 16 letters, numbers or underscores."
            ));
        }
        if let Some(kept) = self.offline.iter().find(|n| n.eq_ignore_ascii_case(name)) {
            return Ok(kept.clone());
        }
        self.offline.push(name.to_string());
        Ok(name.to_string())
    }

    /// Forgets an offline name: its look and its skin and cape files (`skins`) go with it; if
    /// it was playing, the game's own name plays.
    pub fn remove_offline(&mut self, name: &str, skins_dir: &Path) -> Result<(), String> {
        let before = self.offline.len();
        self.offline.retain(|n| !n.eq_ignore_ascii_case(name));
        self.looks.remove(&name.to_ascii_lowercase());
        if self.offline.len() == before {
            return Err(format!("No offline name {name}."));
        }
        crate::skins::forget(skins_dir, name);
        if self
            .offline_selected
            .as_ref()
            .is_some_and(|n| n.eq_ignore_ascii_case(name))
        {
            self.offline_selected = None;
        }
        Ok(())
    }

    /// An offline name's look (the default: no skin of its own).
    pub fn look(&self, name: &str) -> crate::skins::Look {
        self.looks
            .get(&name.to_ascii_lowercase())
            .cloned()
            .unwrap_or_default()
    }

    /// The game's variables for the offline name that plays: its skin and cape.
    pub fn look_env(&self) -> Vec<(&'static str, String)> {
        match self.name_arg() {
            Some(name) => crate::skins::env(&crate::skins::dir(), name, &self.look(name)),
            None => Vec::new(),
        }
    }

    /// The game's `--name` for this run: the offline name, when one plays.
    pub fn name_arg(&self) -> Option<&str> {
        match self.selected() {
            Some(_) => None,
            None => self.offline_selected.as_deref(),
        }
    }

    /// A renewed profile (its name or skin changed) in place of the old one; who plays stays.
    fn update(&mut self, account: Account) {
        if let Some(a) = self.accounts.iter_mut().find(|a| a.uuid == account.uuid) {
            *a = account;
        }
    }

    /// Sign out: the secrets leave the keychain, the account the list. Offline names stay, but
    /// play only once an account that owns the game is back.
    pub fn remove(&mut self, store: &dyn SecretStore, uuid: &str) -> Result<(), String> {
        let _operation = session::lock(store)?;
        store.delete(uuid)?;
        self.accounts.retain(|a| a.uuid != uuid);
        if self.selected.as_deref() == Some(uuid) {
            self.selected = None;
        }
        Ok(())
    }
}

/// No account in the launcher owns Minecraft: Java Edition: the game must not start.
#[derive(Debug, PartialEq)]
pub struct NoOwner;
impl std::fmt::Display for NoOwner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{NEEDS_OWNER}.")
    }
}

impl NoOwner {
    /// The same refusal for the window, which has Settings rather than a command to name.
    pub fn in_window() -> String {
        use crate::i18n::tr;
        format!(
            "{}. {}",
            tr(NEEDS_OWNER),
            tr("Open Settings and sign in under Account.")
        )
    }
}

/// The owner gate's pass for one launch.
#[derive(Debug, Default)]
pub struct Launch {
    /// The selected account and its renewed session; none when an offline name plays, or when
    /// its sign-in couldn't be renewed now (`offline` says why).
    pub session: Option<(Account, Secrets)>,
    /// Why the selected account plays without its session this time.
    pub offline: Option<String>,
    /// An account the services said owns the game just now, the selected one first: who the
    /// launcher may download the game for.
    pub owner: Option<(Account, Secrets)>,
    /// A renewed sign-in the keychain refused to keep: it plays now, and may have to sign in
    /// again next time.
    pub unsaved: Option<String>,
}

/// The last [`Launch::unsaved`], for the window or the terminal to show once.
static UNSAVED: Mutex<Option<String>> = Mutex::new(None);

/// What the last launch couldn't save in the keychain, once.
pub fn take_unsaved() -> Option<String> {
    UNSAVED.lock().ok()?.take()
}

/// The owner gate: the game starts only while an account in the launcher owns Minecraft: Java
/// Edition. The accounts are asked again, the selected one first, until one owns it; each keeps
/// the answer in the keychain. An account that gets no answer (no network) keeps its last one, so
/// being offline never blocks a player who passed before, while "doesn't own" blocks at once.
/// The caller saves `accounts` (renewed names and skins).
pub fn launch(
    ep: &Endpoints,
    client_id: Option<&str>,
    store: &dyn SecretStore,
    accounts: &mut Accounts,
) -> Result<Launch, NoOwner> {
    let selected = accounts.selected.clone();
    let mut order = accounts.accounts.clone();
    order.sort_by_key(|a| Some(&a.uuid) != selected.as_ref());
    let mut pass = Launch::default();
    let mut owned = false;
    for account in order {
        if owned {
            break;
        }
        let plays = Some(&account.uuid) == selected.as_ref();
        let operation = session::lock(store);
        let Some(secrets) = secrets_in(store, &account.uuid) else {
            if plays {
                pass.offline = Some(format!(
                    "{} is no longer signed in on this computer. Sign in again.",
                    account.name
                ));
            }
            continue;
        };
        let before = serde_json::to_string(&secrets).ok();
        let checked = match &operation {
            Ok(_) => check(ep, client_id, &account, secrets),
            Err(e) => Check::NoAnswer(e.clone(), secrets),
        };
        let (said, secrets) = match checked {
            Check::Owns(renewed, secrets) => {
                accounts.update(renewed.clone());
                if plays {
                    pass.session = Some((renewed.clone(), secrets.clone()));
                }
                pass.owner = Some((renewed, secrets.clone()));
                (None, secrets)
            }
            Check::DoesNot(secrets) => (Some(DOES_NOT_OWN.to_string()), secrets),
            Check::NoAnswer(e, secrets) => (Some(e), secrets),
        };
        owned = secrets.owns;
        if plays {
            pass.offline = said;
        }
        let after = serde_json::to_string(&secrets).ok();
        if after != before {
            if let Some(text) = after {
                // The game still starts; the player hears why the next launch may ask again.
                if let Err(e) = store.set(&account.uuid, &text) {
                    let said = format!(
                        "{}'s renewed sign-in couldn't be saved in the keychain ({e}): you may \
                         have to sign in again next time.",
                        account.name
                    );
                    if let Ok(mut last) = UNSAVED.lock() {
                        *last = Some(said.clone());
                    }
                    pass.unsaved = Some(said);
                }
            }
        }
    }
    if owned {
        Ok(pass)
    } else {
        Err(NoOwner)
    }
}

/// The owner gate before the game starts. With the window's hidden developer option
/// (`backend::Settings::dev_offline`, `offline_without_owner`) no owner is no refusal: the game
/// starts offline, without a session and without an `owner`. Downloads never come through here:
/// `backend::download_for_owner` asks [`launch`] itself, so the option never downloads the game.
pub fn launch_for_game(
    ep: &Endpoints,
    client_id: Option<&str>,
    store: &dyn SecretStore,
    accounts: &mut Accounts,
    offline_without_owner: bool,
) -> Result<Launch, NoOwner> {
    match launch(ep, client_id, store, accounts) {
        Err(NoOwner) if offline_without_owner => Ok(Launch::default()),
        pass => pass,
    }
}

/// Write the session for the game, readable by this user only, in a fresh file it deletes
/// when it reads it.
pub fn write_session_file(
    dir: &Path,
    account: &Account,
    secrets: &Secrets,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    // The folder too is this user's only (the files already are).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
    // Leftovers of games that never started (refused by the game after two minutes anyway).
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let old = e
                .metadata()
                .and_then(|m| m.modified())
                .is_ok_and(|m| m.elapsed().is_ok_and(|age| age > Duration::from_secs(120)));
            if old {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    let mut name = [0u8; 12];
    getrandom::fill(&mut name).map_err(|e| e.to_string())?;
    let path = dir.join(format!(
        "session-{}.json",
        name.iter().map(|b| format!("{b:02x}")).collect::<String>()
    ));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(&path).map_err(|e| e.to_string())?;
    let body = json!({
        "name": account.name,
        "uuid": account.uuid,
        "access_token": secrets.minecraft_token,
        "user_type": "msa",
        "xuid": secrets.xuid,
    });
    std::io::Write::write_all(&mut file, body.to_string().as_bytes()).map_err(|e| e.to_string())?;
    Ok(path)
}

/// How long a game takes a session file: it refuses one older than this (the game's
/// `MAX_SESSION_FILE_AGE`), so the launcher deletes it then if it is still there.
const SESSION_FILE_LIFE: Duration = Duration::from_secs(120);

/// The session files this launcher wrote for games it starts, until they are gone: a game
/// deletes each as it reads it; the rest go when the game would refuse them anyway
/// ([`SESSION_FILE_LIFE`], a game that never started or crashed first) or when the launch failed
/// ([`discard_unread_sessions`]). Nothing with a token stays on disk.
static UNREAD: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// [`write_session_file`] for a game this launcher is about to start (see [`UNREAD`]).
fn write_launch_session_file(
    dir: &Path,
    account: &Account,
    secrets: &Secrets,
) -> Result<PathBuf, String> {
    let path = write_session_file(dir, account, secrets)?;
    let Ok(mut unread) = UNREAD.lock() else {
        return Ok(path);
    };
    if unread.is_empty() {
        // One reaper while files wait; it ends with the list (under its lock, so a file added
        // as it ends starts the next).
        let _ = std::thread::Builder::new()
            .name("Session files".into())
            .spawn(|| loop {
                std::thread::sleep(Duration::from_secs(5));
                let Ok(mut unread) = UNREAD.lock() else {
                    return;
                };
                reap(&mut unread, SESSION_FILE_LIFE);
                if unread.is_empty() {
                    return;
                }
            });
    }
    unread.push(path.clone());
    Ok(path)
}

/// Deletes the files of `unread` older than `life` (the game would refuse them) and forgets
/// those gone (read).
fn reap(unread: &mut Vec<PathBuf>, life: Duration) {
    unread.retain(|path| {
        let waiting = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .is_ok_and(|m| m.elapsed().is_ok_and(|age| age <= life));
        if !waiting {
            let _ = std::fs::remove_file(path);
        }
        waiting
    });
}

/// Deletes the session files written for a launch whose game didn't start, or has ended: any it
/// didn't read is left over (a game deletes those it reads).
pub fn discard_unread_sessions() {
    let files = UNREAD
        .lock()
        .map(|mut unread| std::mem::take(&mut *unread))
        .unwrap_or_default();
    for path in files {
        let _ = std::fs::remove_file(path);
    }
}

/// Called under the account-operation lock, after the final profile is known. Rejected or
/// cancelled sign-ins must not save a credential, add another identity, or change selection.
fn accept_sign_in(
    accounts: &mut Accounts,
    store: &dyn SecretStore,
    expected: Option<&Account>,
    account: Account,
    secrets: &Secrets,
    cancelled: &dyn Fn() -> bool,
    progress: &dyn Fn(session::Phase),
) -> Result<(), String> {
    if cancelled() {
        return Err("Sign-in cancelled.".into());
    }
    if let Some(expected) = expected {
        if expected.uuid != account.uuid {
            return Err(format!(
                "Signed in as {} instead of {}. Use the Microsoft account for {} and try again.",
                account.name, expected.name, expected.name
            ));
        }
        if accounts.find(&expected.uuid).is_none() {
            return Err("That account was signed out while sign-in was in progress. Add it again to continue.".into());
        }
    }
    let selected = accounts.selected.clone();
    progress(session::Phase::Saving);
    accounts.keep_sign_in(store, account, secrets)?;
    if expected.is_some() {
        accounts.selected = selected;
    }
    Ok(())
}

/// Where a background sign-in is (the window and the MCP server show it).
#[derive(Clone, Debug, PartialEq)]
pub enum SignInState {
    Starting,
    /// Waiting for the player at `verification_uri` with `user_code`.
    Code(DeviceCode),
    Working(session::Phase),
    Done(Result<Account, String>),
}

/// A sign-in on its own thread, from the code to the saved account.
pub struct SignIn {
    state: std::sync::Arc<Mutex<SignInState>>,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl SignIn {
    pub fn start() -> Self {
        Self::start_expected(None)
    }

    /// Restore this exact Minecraft identity without changing who currently plays.
    pub fn start_for(account: Account) -> Self {
        Self::start_expected(Some(account))
    }

    fn start_expected(expected: Option<Account>) -> Self {
        use std::sync::atomic::Ordering;
        let state = std::sync::Arc::new(Mutex::new(SignInState::Starting));
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (thread_state, thread_cancel) = (state.clone(), cancel.clone());
        std::thread::spawn(move || {
            let set = |s| *thread_state.lock().unwrap() = s;
            let result = (|| {
                let ep = Endpoints::live();
                let client_id = client_id().ok_or(NOT_CONFIGURED)?;
                let code = device_code(&ep, &client_id)?;
                set(SignInState::Code(code.clone()));
                let ms = wait_for_sign_in(&ep, &client_id, &code, &|| {
                    thread_cancel.load(Ordering::Relaxed)
                })?;
                let progress = |phase| set(SignInState::Working(phase));
                let (account, secrets) = finish_sign_in_with_progress(&ep, ms, &progress)?;
                progress(session::Phase::Queued);
                let store = store();
                let _operation = session::lock(store)?;
                let mut accounts = Accounts::load();
                accept_sign_in(
                    &mut accounts,
                    store,
                    expected.as_ref(),
                    account.clone(),
                    &secrets,
                    &|| thread_cancel.load(Ordering::Relaxed),
                    &progress,
                )?;
                progress(session::Phase::Saving);
                accounts.save()?;
                Ok(account)
            })();
            set(SignInState::Done(result));
        });
        Self { state, cancel }
    }
    pub fn state(&self) -> SignInState {
        self.state.lock().unwrap().clone()
    }
    pub fn cancel(&self) {
        self.cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// What the game about to start gets from the accounts.
#[derive(Debug, Default)]
pub struct GameSession {
    /// The playing account's session (`--session-file`); none: offline.
    pub file: Option<PathBuf>,
    /// Why the account plays offline this time, if it does.
    pub offline: Option<String>,
    /// The other accounts' sessions, for splitscreen players (`account_sessions`).
    pub env: Vec<(&'static str, String)>,
}

/// The owner gate for the game about to start, then the selected account's session in a file
/// for it, and the other accounts' for its splitscreen players. `offline_without_owner`: see
/// [`launch_for_game`].
pub fn session_for_game(
    game_home: &Path,
    offline_without_owner: bool,
) -> Result<GameSession, NoOwner> {
    let mut accounts = Accounts::load();
    let pass = launch_for_game(
        &Endpoints::live(),
        client_id().as_deref(),
        store(),
        &mut accounts,
        offline_without_owner,
    )?;
    let env = split_sessions(
        game_home,
        &mut accounts,
        pass.session.as_ref().map(|(a, _)| a.uuid.as_str()),
    );
    if let Err(e) = accounts.save() {
        eprintln!("Eclipse: the accounts are not saved: {e}");
    }
    Ok(session_file(game_home, pass, env))
}

/// [`session_for_game`] for one launch as `who` (`play --account`, a signed-in account): who
/// plays changes for this launch only, so several games can play as several accounts at once
/// (#4952). What the services renewed (tokens, names, skins) is still kept; which account
/// `account select` picked is not touched.
pub fn session_for_game_as(game_home: &Path, who: &Player) -> Result<GameSession, String> {
    let mut accounts = Accounts::load();
    let pass = launch_as(
        &Endpoints::live(),
        client_id().as_deref(),
        store(),
        &mut accounts,
        who,
    );
    if let Err(e) = accounts.save() {
        eprintln!("Eclipse: the accounts are not saved: {e}");
    }
    let pass = pass?;
    let env = split_sessions(
        game_home,
        &mut accounts,
        pass.session.as_ref().map(|(a, _)| a.uuid.as_str()),
    );
    if let Err(e) = accounts.save() {
        eprintln!("Eclipse: the accounts are not saved: {e}");
    }
    Ok(session_file(game_home, pass, env))
}

/// The owner gate as `who`, on `accounts`, which keeps its choice of player.
fn launch_as(
    ep: &Endpoints,
    client_id: Option<&str>,
    store: &dyn SecretStore,
    accounts: &mut Accounts,
    who: &Player,
) -> Result<Launch, String> {
    let chosen = (
        accounts.selected.clone(),
        accounts.offline_selected.clone(),
        accounts.offline.clone(),
    );
    let pass = accounts
        .play_as(store, who.clone())
        .and_then(|()| launch(ep, client_id, store, accounts).map_err(|e| e.to_string()));
    (
        accounts.selected,
        accounts.offline_selected,
        accounts.offline,
    ) = chosen;
    pass
}

/// The game's session files for a launch that passed the gate.
fn session_file(game_home: &Path, pass: Launch, env: Vec<(&'static str, String)>) -> GameSession {
    let mut out = GameSession {
        env,
        offline: pass.offline,
        ..GameSession::default()
    };
    if let Some((account, secrets)) = pass.session {
        match write_launch_session_file(&game_home.join("sessions"), &account, &secrets) {
            Ok(file) => out.file = Some(file),
            Err(e) => out.offline = Some(e),
        }
    }
    out
}

fn split_sessions(
    game_home: &Path,
    accounts: &mut Accounts,
    playing: Option<&str>,
) -> Vec<(&'static str, String)> {
    if splitscreen_on(game_home) {
        // The launch may override the saved player, or play with an offline name.
        let selected = std::mem::replace(&mut accounts.selected, playing.map(str::to_owned));
        let env = account_sessions(
            &Endpoints::live(),
            client_id().as_deref(),
            store(),
            &game_home.join("sessions"),
            accounts,
        );
        accounts.selected = selected;
        env
    } else {
        Vec::new()
    }
}

/// Whether the game's Splitscreen setting is on (`eclipse-extras.txt`, `splitscreen:true`): only
/// then do the other accounts' sessions go to it.
fn splitscreen_on(game_home: &Path) -> bool {
    std::fs::read_to_string(game_home.join("eclipse-extras.txt"))
        .is_ok_and(|text| text.lines().any(|l| l.trim() == "splitscreen:true"))
}

/// The signed-in accounts but the one that plays, each its session in a file of its own, for
/// the game's splitscreen players to sign in with (`ECLIPSE_ACCOUNT_SESSIONS`: name, uuid and
/// file each). A token known to own the game and not due goes as it is; otherwise it is renewed
/// and asked about (`check`), and an account that can't be renewed, doesn't own the game or
/// whose token names another profile is left out: the game starts all the same. Nothing with one
/// account.
/// How long Play spends renewing the other accounts' sessions, in all: past it, the rest are
/// offered only with a token that still holds (no request starts; one under way finishes).
const OTHERS_WAIT: Duration = Duration::from_secs(10);

fn account_sessions(
    ep: &Endpoints,
    client_id: Option<&str>,
    store: &dyn SecretStore,
    dir: &Path,
    accounts: &mut Accounts,
) -> Vec<(&'static str, String)> {
    account_sessions_until(
        ep,
        client_id,
        store,
        dir,
        accounts,
        Instant::now() + OTHERS_WAIT,
    )
}

fn account_sessions_until(
    ep: &Endpoints,
    client_id: Option<&str>,
    store: &dyn SecretStore,
    dir: &Path,
    accounts: &mut Accounts,
    deadline: Instant,
) -> Vec<(&'static str, String)> {
    let others: Vec<Account> = accounts
        .accounts
        .iter()
        .filter(|a| Some(&a.uuid) != accounts.selected.as_ref())
        .cloned()
        .collect();
    let mut list = Vec::new();
    for account in others {
        let skipped =
            |reason: &str| json!({ "name": account.name, "uuid": account.uuid, "skipped": reason });
        // Under the account lock, as every renewal is: a refresh token Microsoft rotates here is
        // not lost to another launcher renewing the same account at once. Still busy at the
        // deadline: no request, the token while it holds.
        let operation = session::lock_within(
            store,
            Some(deadline.saturating_duration_since(Instant::now())),
        );
        let Some(secrets) = secrets_in(store, &account.uuid) else {
            list.push(skipped("Sign in again in the launcher."));
            continue;
        };
        let fallback = skipped("Sign in again in the launcher.");
        let before = serde_json::to_string(&secrets).ok();
        let uuid = account.uuid.clone();
        // An account the services sent back to sign-in (a refused refresh, a token now for
        // another profile) is never handed over, whatever its token's time says.
        let holds = |s: &Secrets, margin: u64| {
            s.owns && !s.sign_in_required && s.minecraft_expires_at > now() + margin
        };
        let known = holds(&secrets, RENEW_BEFORE);
        let late = operation.is_err() || Instant::now() >= deadline;
        let (account, secrets) = match known {
            true => (Some(account), secrets),
            // Out of time: its token while it holds, else Sign in again.
            false if late => (holds(&secrets, 60).then_some(account), secrets),
            false => match check(ep, client_id, &account, secrets) {
                Check::Owns(renewed, secrets) => {
                    accounts.update(renewed.clone());
                    (Some(renewed), secrets)
                }
                Check::DoesNot(secrets) => (None, secrets),
                Check::NoAnswer(e, secrets) => {
                    eprintln!("{}'s session for splitscreen: {e}", account.name);
                    // Its token may still hold (no network): offered only while it does. A token
                    // for another profile is marked sign-in required, so it never is.
                    (holds(&secrets, 60).then_some(account), secrets)
                }
            },
        };
        // A renewed token, or the services' answer, kept for the next launch.
        let after = serde_json::to_string(&secrets).ok();
        if let (true, Some(text)) = (after != before, after) {
            let _ = store.set(&uuid, &text);
        }
        drop(operation);
        let Some(account) = account else {
            list.push(if secrets.owns { fallback } else {
                json!({ "name": fallback["name"], "uuid": uuid, "skipped": "This account needs Minecraft: Java Edition ownership confirmed in the launcher." })
            });
            continue;
        };
        match write_launch_session_file(dir, &account, &secrets) {
            Ok(file) => {
                list.push(json!({ "name": account.name, "uuid": account.uuid, "file": file }))
            }
            Err(_) => list.push(fallback),
        }
    }
    if list.is_empty() {
        return Vec::new();
    }
    vec![(
        "ECLIPSE_ACCOUNT_SESSIONS",
        serde_json::Value::from(list).to_string(),
    )]
}

/// Who plays the real Minecraft: Java Edition (a Java instance), past the owner gate itself
/// ([`launch`]): the selected account when it owns the game, else one that does. With no answer
/// from the services, an account that owned it plays with its last token. Mojang's game takes the
/// token on its command line, as Mojang's launcher gives it; that command is never logged.
pub fn java_player() -> Result<crate::java_launch::Player, NoOwner> {
    let mut accounts = Accounts::load();
    let player = java_player_with(
        &Endpoints::live(),
        client_id().as_deref(),
        store(),
        &mut accounts,
    );
    if let Err(e) = accounts.save() {
        eprintln!("Eclipse: the accounts are not saved: {e}");
    }
    player
}

fn java_player_with(
    ep: &Endpoints,
    client_id: Option<&str>,
    store: &dyn SecretStore,
    accounts: &mut Accounts,
) -> Result<crate::java_launch::Player, NoOwner> {
    let pass = launch(ep, client_id, store, accounts)?;
    let kept = || {
        let order = accounts.selected().into_iter().chain(&accounts.accounts);
        order
            .into_iter()
            .find_map(|a| Some((a.clone(), secrets_in(store, &a.uuid).filter(|s| s.owns)?)))
    };
    let (account, secrets) = pass.session.or(pass.owner).or_else(kept).ok_or(NoOwner)?;
    Ok(crate::java_launch::Player {
        name: account.name,
        uuid: account.uuid,
        token: secrets.minecraft_token,
        xuid: secrets.xuid.unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests;
