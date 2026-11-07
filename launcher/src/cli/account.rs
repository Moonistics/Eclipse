use super::json;
use clap::Subcommand;
use eclipse_launcher::account::{self, Accounts};
use eclipse_launcher::backend;
use eclipse_launcher::i18n::tr;
use std::path::PathBuf;

#[derive(Subcommand)]
pub(super) enum AccountCmd {
    /// The signed-in accounts and which one plays
    List,
    /// Sign in with a Microsoft account in your browser (device code). Play needs one that owns
    /// Minecraft: Java Edition
    SignIn {
        /// Sign this account (name or UUID) in again: another Microsoft account is refused and
        /// who plays stays the same
        #[arg(long = "for")]
        target: Option<String>,
    },
    /// Renew an account's session now, even when its access token is still fresh
    Refresh { who: String },
    /// Renew an account's session for a game already running: prints the read-once session file
    /// it hands over (`--json`: {"session_file"} or {"authentication"} when it can't)
    Session { who: String },
    /// Sign out: the account's tokens leave the keychain (and the secrets file, where it kept them)
    SignOut {
        /// Its name or UUID
        who: String,
    },
    /// Play as this account, or as one of your offline names
    Select { who: String },
    /// Play offline (LAN, offline-mode servers) beside a signed-in account that owns the game:
    /// as NAME (kept to switch back to; 3 to 16 letters, digits or _), or as the name the game
    /// made up
    Offline { name: Option<String> },
    /// Forget one of your offline names; its skin and cape go with it
    ForgetOffline { name: String },
    /// Show or set an offline name's skin and cape: a 64x64 (or 64x32) skin PNG, a 64x32 cape
    /// PNG, the model. Kept by the launcher and given to the game when that name plays
    Skin {
        name: String,
        /// The skin's PNG
        file: Option<PathBuf>,
        /// classic or slim arms
        #[arg(long)]
        model: Option<String>,
        /// The cape's PNG
        #[arg(long)]
        cape: Option<PathBuf>,
        /// Back to the default skin
        #[arg(long)]
        no_skin: bool,
        /// Take the cape off
        #[arg(long)]
        no_cape: bool,
    },
}

/// `account …`: names and UUIDs only; tokens are never printed.
pub(super) fn account(action: AccountCmd, as_json: bool) -> Result<(), String> {
    let mut accounts = Accounts::load();
    let find = |accounts: &Accounts, who: &str| {
        accounts
            .find(who)
            .cloned()
            .ok_or_else(|| format!("No signed-in account {who}. See `eclipse-launcher account`."))
    };
    match action {
        AccountCmd::List => {
            if as_json {
                json(serde_json::json!({
                    "accounts": accounts.accounts.iter().map(|a| serde_json::json!({
                        "name": a.name, "uuid": a.uuid,
                        "selected": accounts.selected.as_ref() == Some(&a.uuid),
                        "authentication": account::session::status(account::store(), &a.uuid),
                    })).collect::<Vec<_>>(),
                    "offline": accounts.offline.iter().map(|n| serde_json::json!({
                        "name": n,
                        "selected": accounts.player() == account::Player::Offline(n.clone()),
                    })).collect::<Vec<_>>(),
                    "sign_in_configured": account::client_id().is_some(),
                    "owner": accounts.has_owner(account::store()),
                }));
            } else if accounts.accounts.is_empty() && accounts.offline.is_empty() {
                println!(
                    "No Microsoft account signed in. Play needs one that owns Minecraft: Java \
                     Edition: eclipse-launcher account sign-in"
                );
            } else {
                for a in &accounts.accounts {
                    let mark = if accounts.selected.as_ref() == Some(&a.uuid) {
                        "*"
                    } else {
                        " "
                    };
                    let status = account::session::status(account::store(), &a.uuid);
                    println!("{mark} {}  {}  {}", a.name, a.uuid, status.label());
                }
                for n in &accounts.offline {
                    let on = accounts.player() == account::Player::Offline(n.clone());
                    println!("{} {n}  offline", if on { "*" } else { " " });
                }
                if accounts.player() == account::Player::GameName {
                    println!(
                        "Playing offline as the game's own name (`account select NAME` to switch)."
                    );
                }
                if !accounts.has_owner(account::store()) {
                    println!("{} {}", account::NoOwner, tr(backend::SIGN_IN_HINT));
                }
            }
        }
        AccountCmd::Refresh { who } => {
            let status = account::session::refresh(&who)?;
            if as_json {
                json(serde_json::to_value(&status).map_err(|e| e.to_string())?);
            } else if status.state != account::session::State::Ready {
                return Err(format!("{}: {}", status.label(), status.message));
            } else {
                println!("{}: {}", status.label(), status.message);
            }
        }
        AccountCmd::Session { who } => {
            let dir = backend::game_home().join(eclipse_launcher::game_folder::SESSIONS);
            match (account::session::renew_for_game(&dir, &who), as_json) {
                (Ok(file), true) => json(serde_json::json!({ "session_file": file })),
                (Ok(file), false) => println!("{}", file.display()),
                (Err(status), true) => json(serde_json::json!({ "authentication": status })),
                (Err(status), false) => return Err(status.message),
            }
        }
        AccountCmd::SignIn { target: Some(who) } => {
            let a = find(&accounts, &who)?;
            let sign_in = account::SignIn::start_for(a.clone());
            let mut shown = false;
            let signed_in = loop {
                match sign_in.state() {
                    account::SignInState::Code(code) if !shown => {
                        shown = true;
                        if as_json {
                            json(serde_json::json!({
                                "link": code.verification_uri,
                                "code": code.user_code,
                            }));
                        } else {
                            eprintln!(
                                "To sign {} in again, open {} in a browser and enter the code {}",
                                a.name, code.verification_uri, code.user_code
                            );
                        }
                    }
                    account::SignInState::Done(result) => break result?,
                    _ => {}
                }
                std::thread::sleep(std::time::Duration::from_millis(200));
            };
            if as_json {
                json(serde_json::json!({ "signed_in": signed_in.uuid }));
            } else {
                println!("Signed in as {} again.", signed_in.name);
            }
        }
        AccountCmd::SignIn { target: None } => {
            let client_id = account::client_id().ok_or(account::NOT_CONFIGURED)?;
            let ep = account::Endpoints::live();
            let code = account::device_code(&ep, &client_id)?;
            eprintln!(
                "To sign in, open {} in a browser and enter the code {}",
                code.verification_uri, code.user_code
            );
            let ms = account::wait_for_sign_in(&ep, &client_id, &code, &|| false)?;
            let (signed_in, secrets) = account::finish_sign_in(&ep, ms)?;
            accounts.add(account::store(), signed_in.clone(), &secrets)?;
            accounts.save()?;
            println!("Signed in as {}.", signed_in.name);
            if let Some(said) = account::no_keychain() {
                eprintln!("{said}");
            }
        }
        AccountCmd::SignOut { who } => {
            let a = find(&accounts, &who)?;
            accounts.remove(account::store(), &a.uuid)?;
            accounts.save()?;
            println!("Signed out {}.", a.name);
        }
        AccountCmd::Select { who } => {
            let offline = accounts
                .offline
                .iter()
                .find(|n| n.eq_ignore_ascii_case(&who))
                .cloned();
            let (player, name) = match (accounts.find(&who).cloned(), offline) {
                (Some(a), _) => (account::Player::Microsoft(a.uuid), a.name),
                (None, Some(n)) => (
                    account::Player::Offline(n.clone()),
                    format!("{n} (offline)"),
                ),
                (None, None) => return Err(find(&accounts, &who).unwrap_err()),
            };
            accounts.play_as(account::store(), player)?;
            accounts.save()?;
            println!("The game plays as {name}.");
        }
        AccountCmd::Offline { name } => {
            let player = match name {
                Some(n) => account::Player::Offline(n),
                None => account::Player::GameName,
            };
            accounts.play_as(account::store(), player)?;
            accounts.save()?;
            match accounts.name_arg() {
                Some(n) => println!("The game plays offline as {n}."),
                None => println!("The game plays offline, as the name it made up."),
            }
        }
        AccountCmd::Skin {
            name,
            file,
            model,
            cape,
            no_skin,
            no_cape,
        } => {
            use eclipse_launcher::skins::{self, Model};
            let name = accounts
                .offline
                .iter()
                .find(|n| n.eq_ignore_ascii_case(&name))
                .cloned()
                .ok_or_else(|| {
                    format!("No offline name {name}: add it with `account offline {name}`.")
                })?;
            let model = match model.as_deref() {
                None => None,
                Some("classic") => Some(Model::Classic),
                Some("slim") => Some(Model::Slim),
                Some(m) => return Err(format!("{m}: say classic or slim")),
            };
            let mut look = accounts.look(&name);
            if let Some(file) = &file {
                skins::keep(&skins::dir(), &name, file, false)?;
                look.skin = true;
            }
            if let Some(file) = &cape {
                skins::keep(&skins::dir(), &name, file, true)?;
                look.cape = true;
            }
            if let Some(m) = model {
                look.model = m;
            }
            look.skin &= !no_skin;
            look.cape &= !no_cape;
            accounts.set_look(&name, look.clone())?;
            accounts.save()?;
            if as_json {
                json(serde_json::json!({
                    "name": name, "skin": look.skin, "cape": look.cape, "model": look.model.name(),
                }));
            } else {
                let say = |on: bool, what: &str| if on { what.to_string() } else { "none".into() };
                println!(
                    "{name}: skin {} ({} arms), cape {}",
                    say(look.skin, "kept"),
                    look.model.name(),
                    say(look.cape, "kept")
                );
            }
        }
        AccountCmd::ForgetOffline { name } => {
            accounts.remove_offline(&name, &eclipse_launcher::skins::dir())?;
            accounts.save()?;
            println!("Forgot the offline name {name}.");
        }
    }
    Ok(())
}
