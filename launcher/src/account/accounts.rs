use super::msa::{Account, Secrets};
use super::session;
use crate::secrets::SecretStore;
use std::path::{Path, PathBuf};

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
    Offline(String),
    /// Offline as the name the game made up.
    GameName,
}

pub(super) const OFFLINE_NEEDS_OWNER: &str =
    "Sign in once with an account that owns Java Edition to pick your own player name.";

pub(super) fn secrets_in(store: &dyn SecretStore, uuid: &str) -> Option<Secrets> {
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
    pub fn find(&self, who: &str) -> Option<&Account> {
        let undashed = who.replace('-', "").to_lowercase();
        self.accounts
            .iter()
            .find(|a| a.name.eq_ignore_ascii_case(who) || a.uuid == undashed)
    }
    pub fn add(
        &mut self,
        store: &dyn SecretStore,
        account: Account,
        secrets: &Secrets,
    ) -> Result<(), String> {
        let _operation = session::lock(store)?;
        self.keep_sign_in(store, account, secrets)
    }

    pub(super) fn keep_sign_in(
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

    pub(super) fn add_offline(&mut self, name: &str) -> Result<String, String> {
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

    /// Keeps `look` for the offline name `name`; refused for a name not kept (forgotten while a
    /// file was being picked, say).
    pub fn set_look(&mut self, name: &str, look: crate::skins::Look) -> Result<(), String> {
        if !self.offline.iter().any(|n| n.eq_ignore_ascii_case(name)) {
            return Err(format!("No offline name {name}."));
        }
        self.looks.insert(name.to_ascii_lowercase(), look);
        Ok(())
    }

    pub fn look(&self, name: &str) -> crate::skins::Look {
        self.looks
            .get(&name.to_ascii_lowercase())
            .cloned()
            .unwrap_or_default()
    }

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

    pub(super) fn update(&mut self, account: Account) {
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
