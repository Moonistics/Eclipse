//! Who plays: the sidebar's quick switch (a menu under the account chip) and Settings' offline
//! names (`eclipse_launcher::account::Accounts::offline`).
use super::fields::Field;
use super::*;
use eclipse_launcher::account::Player;

pub(super) struct WhoPlays {
    pub(super) name: String,
    pub(super) face: String,
    pub(super) how: &'static str,
    pub(super) means: &'static str,
}

fn how_signed_in(who: &Player) -> &'static str {
    match who {
        Player::Microsoft(_) => tr("Microsoft account"),
        Player::Offline(_) | Player::GameName => tr("Offline account"),
    }
}

impl Launcher {
    pub(super) fn game_name(&self) -> String {
        self.offline_name
            .clone()
            .unwrap_or_else(|| tr(account::DEFAULT_PLAYER).into())
    }

    pub(super) fn who_plays(&self) -> WhoPlays {
        let who = self.accounts.player();
        let how = how_signed_in(&who);
        let offline = |name: String| WhoPlays {
            face: eclipse_launcher::heads::offline_key(&name),
            name,
            how,
            means: tr("Offline account · can't join online-mode servers"),
        };
        match who {
            Player::Microsoft(uuid) => WhoPlays {
                name: self
                    .accounts
                    .find(&uuid)
                    .map_or_else(String::new, |a| a.name.clone()),
                face: uuid,
                how,
                means: tr("Microsoft account · online play"),
            },
            Player::Offline(name) => offline(name),
            Player::GameName => offline(self.game_name()),
        }
    }

    pub(super) fn can_sign_in(&self) -> bool {
        !matches!(self.accounts.player(), Player::Microsoft(_)) && account::client_id().is_some()
    }

    pub(super) fn cant_sign_in_to_play(&self) -> bool {
        self.owner_shown() == Some(false) && account::client_id().is_none()
    }

    pub(super) fn why_no_sign_in(&self) -> Option<&'static str> {
        self.cant_sign_in_to_play()
            .then(|| tr("This copy of the launcher can't sign in yet."))
    }

    pub(super) fn cant_sign_in_note() -> &'static str {
        tr("Every instance needs a Microsoft account that owns Minecraft: Java Edition, and this copy of the launcher can't sign in yet.")
    }

    pub(super) fn open_sign_in(&mut self) {
        self.start_sign_in();
        self.show_settings(settings_page::SettingsSection::Account);
    }

    pub(super) fn change_accounts(
        &mut self,
        change: impl FnOnce(&mut Accounts) -> Result<(), String>,
    ) -> Result<(), String> {
        let mut accounts = Accounts::load();
        change(&mut accounts).and_then(|()| accounts.save())?;
        self.accounts = accounts;
        Ok(())
    }

    pub(super) fn play_as(&mut self, who: Player) -> bool {
        let offline = self.settings.dev_offline;
        let done = self.change_accounts(|a| a.play_as_with(account::store(), who, offline));
        if let Err(e) = &done {
            crate::toast::say_error(e.clone());
        }
        self.close_menu();
        done.is_ok()
    }

    /// Everyone the player can play as, with how each signs in.
    pub(super) fn players(&self) -> Vec<(Player, String, &'static str)> {
        let mut out: Vec<_> = (self.accounts.accounts.iter())
            .map(|a| (Player::Microsoft(a.uuid.clone()), a.name.clone()))
            .collect();
        out.extend((self.accounts.offline.iter()).map(|n| (Player::Offline(n.clone()), n.clone())));
        out.push((Player::GameName, self.game_name()));
        out.into_iter()
            .map(|(who, name)| {
                let how = how_signed_in(&who);
                (who, name, how)
            })
            .collect()
    }

    /// Enter in the offline name field.
    pub(super) fn add_offline_name(&mut self) {
        let name = self.fields.value(Field::OfflineName).trim().to_string();
        if name.is_empty() {
            return;
        }
        if self.play_as(Player::Offline(name)) {
            self.fields.text(Field::OfflineName).set(String::new());
        }
    }

    /// Forgets an offline name and its skin.
    pub(super) fn forget_offline(&mut self, name: &str) {
        let skins = eclipse_launcher::skins::dir();
        if let Err(e) = self.change_accounts(|a| a.remove_offline(name, &skins)) {
            crate::toast::say_error(e);
        }
    }
}
