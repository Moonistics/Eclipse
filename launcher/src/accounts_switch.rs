//! Who plays: the sidebar's quick switch (a menu under the account chip) and Settings' offline
//! names (`eclipse_launcher::account::Accounts::offline`).
use super::fields::Field;
use super::*;
use eclipse_launcher::account::Player;

impl Launcher {
    /// The chip's two lines: the name that plays, and how.
    pub(super) fn who_plays(&self) -> (String, &'static str) {
        match self.accounts.player() {
            Player::Microsoft(_) => (
                self.accounts
                    .selected()
                    .map_or_else(String::new, |a| a.name.clone()),
                tr("Microsoft account"),
            ),
            Player::Offline(name) => (name, tr("Offline account")),
            Player::GameName => (
                self.offline_name.clone().unwrap_or_else(|| "Player".into()),
                tr("Offline account"),
            ),
        }
    }

    pub(super) fn play_as(&mut self, who: Player) {
        let mut accounts = Accounts::load();
        let done = accounts
            .play_as_with(account::store(), who, self.settings.dev_offline)
            .and_then(|()| accounts.save());
        match done {
            Ok(()) => self.accounts = accounts,
            Err(e) => self.sign_in_state = Some(SignInState::Done(Err(e))),
        }
        self.close_menu();
    }

    /// Everyone the player can play as, with how each signs in.
    pub(super) fn players(&self) -> Vec<(Player, String, &'static str)> {
        let mut out: Vec<_> = self
            .accounts
            .accounts
            .iter()
            .map(|a| {
                (
                    Player::Microsoft(a.uuid.clone()),
                    a.name.clone(),
                    tr("Microsoft account"),
                )
            })
            .collect();
        out.extend(
            self.accounts
                .offline
                .iter()
                .map(|n| (Player::Offline(n.clone()), n.clone(), tr("Offline account"))),
        );
        out.push((
            Player::GameName,
            self.offline_name.clone().unwrap_or_else(|| "Player".into()),
            tr("Offline account"),
        ));
        out
    }

    fn add_offline_name(&mut self) {
        let name = self.fields.value(Field::OfflineName).trim().to_string();
        if name.is_empty() {
            return;
        }
        self.play_as(Player::Offline(name));
        if !matches!(self.sign_in_state, Some(SignInState::Done(Err(_)))) {
            self.fields.text(Field::OfflineName).set(String::new());
        }
    }

    /// Forgets an offline name and its skin.
    pub(super) fn forget_offline(&mut self, name: &str) {
        let mut accounts = Accounts::load();
        if accounts
            .remove_offline(name, &eclipse_launcher::skins::dir())
            .is_ok()
            && accounts.save().is_ok()
        {
            self.accounts = accounts;
        }
    }

    /// Enter in the offline name field.
    pub(super) fn offline_name_enter(&mut self) {
        self.add_offline_name();
    }
}
