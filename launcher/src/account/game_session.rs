use super::accounts::{secrets_in, Accounts, Player};
use super::msa::{
    check, client_id, now, Account, Check, Endpoints, Secrets, DOES_NOT_OWN, NEEDS_OWNER,
    RENEW_BEFORE,
};
use super::session;
use crate::secrets::{store, SecretStore};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

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
                    let said = crate::i18n::trf(
                        "{name}'s renewed sign-in couldn't be saved in the keychain ({error}): \
                         you may have to sign in again next time.",
                        &[("name", &account.name), ("error", &e.to_string())],
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
pub(super) fn launch_for_game(
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
pub(super) fn write_session_file(
    dir: &Path,
    account: &Account,
    secrets: &Secrets,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
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
                .is_ok_and(|m| m.elapsed().is_ok_and(|age| age > SESSION_FILE_LIFE));
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
pub(super) const SESSION_FILE_LIFE: Duration = Duration::from_secs(120);

/// The session files this launcher wrote for games it starts, until they are gone: a game
/// deletes each as it reads it; the rest go when the game would refuse them anyway
/// ([`SESSION_FILE_LIFE`], a game that never started or crashed first) or when the launch failed
/// ([`discard_unread_sessions`]). Nothing with a token stays on disk.
static UNREAD: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

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

pub(super) fn reap(unread: &mut Vec<PathBuf>, life: Duration) {
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

#[derive(Debug, Default)]
pub struct GameSession {
    /// The playing account's session (`--session-file`); none: offline.
    pub file: Option<PathBuf>,
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

pub(super) fn launch_as(
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

fn session_file(game_home: &Path, pass: Launch, env: Vec<(&'static str, String)>) -> GameSession {
    let mut out = GameSession {
        env,
        offline: pass.offline,
        ..GameSession::default()
    };
    if let Some((account, secrets)) = pass.session {
        match write_launch_session_file(
            &game_home.join(crate::game_folder::SESSIONS),
            &account,
            &secrets,
        ) {
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
        let selected = std::mem::replace(&mut accounts.selected, playing.map(str::to_owned));
        let env = account_sessions(
            &Endpoints::live(),
            client_id().as_deref(),
            store(),
            &game_home.join(crate::game_folder::SESSIONS),
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
pub(super) fn splitscreen_on(game_home: &Path) -> bool {
    std::fs::read_to_string(game_home.join(crate::game_folder::EXTRAS))
        .is_ok_and(|text| crate::game_folder::setting(&text, "splitscreen") == Some("true"))
}

/// How long Play spends renewing the other accounts' sessions, in all: past it, the rest are
/// offered only with a token that still holds (no request starts; one under way finishes).
const OTHERS_WAIT: Duration = Duration::from_secs(10);

/// The signed-in accounts but the one that plays, each its session in a file of its own, for
/// the game's splitscreen players to sign in with (`ECLIPSE_ACCOUNT_SESSIONS`: name, uuid and
/// file each). A token known to own the game and not due goes as it is; otherwise it is renewed
/// and asked about (`check`). An account that can't be renewed, doesn't own the game or whose
/// token names another profile goes with why it was `skipped` instead of a file: the game starts
/// all the same. Nothing with one account.
pub(super) fn account_sessions(
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

pub(super) fn account_sessions_until(
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

pub(super) fn java_player_with(
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
