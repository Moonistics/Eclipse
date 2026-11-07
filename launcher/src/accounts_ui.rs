use super::*;

#[derive(Default)]
pub(super) struct AccountsUi {
    pub(super) auth_status: HashMap<String, account::session::Status>,
    pub(super) auth_progress: HashMap<String, account::session::Phase>,
    pub(super) auth_checked: Option<Instant>,
    pub(super) sign_in_target: Option<account::Account>,
    pub(super) auth_busy: HashMap<String, u64>,
    pub(super) auth_next: u64,
    pub(super) sign_in: Option<SignIn>,
    pub(super) sign_in_state: Option<SignInState>,
    pub(super) keychain: Option<String>,
}

/// Polling must handle each sign-in transition once. Replaying a completed sign-in
/// restarts every account's status worker and leaves the UI perpetually checking.
fn sign_in_change(shown: &mut Option<SignInState>, next: SignInState) -> Option<SignInState> {
    if shown.as_ref() == Some(&next) {
        return None;
    }
    *shown = Some(next.clone());
    Some(next)
}

impl Launcher {
    pub(super) fn reload_accounts(&mut self) -> bool {
        let saved = Accounts::load();
        let changed = saved != self.accounts;
        self.accounts = saved;
        changed
    }

    pub(super) fn load_profile_heads(&mut self) {
        let resources = self
            .selected_version()
            .or_else(|| self.versions.installed.first())
            .map(|tag| backend::resources(tag))
            .unwrap_or_default();
        // Offline names wear their own skin's face, or the default skin of their offline UUID.
        let offline: Vec<String> = self
            .players()
            .into_iter()
            .filter(|(who, _, _)| !matches!(who, account::Player::Microsoft(_)))
            .map(|(_, name, _)| name)
            .collect();
        self.profile_heads
            .request(&self.accounts.accounts, &offline, resources, &self.tx);
    }

    pub(super) fn begin_auth_job(&mut self, uuid: &str) -> Option<u64> {
        if self.accounts_ui.auth_busy.contains_key(uuid) {
            return None;
        }
        self.accounts_ui.auth_next += 1;
        self.accounts_ui
            .auth_busy
            .insert(uuid.into(), self.accounts_ui.auth_next);
        self.accounts_ui
            .auth_progress
            .insert(uuid.into(), account::session::Phase::Queued);
        Some(self.accounts_ui.auth_next)
    }

    /// Only workers read the keychain or contact authentication services. Cached public status
    /// keeps drawing independent of keychain prompts, network timeouts and refresh operations.
    pub(super) fn load_auth_statuses(&mut self, renew_due: bool) {
        self.accounts_ui.auth_checked = Some(Instant::now());
        for account in self.accounts.accounts.clone() {
            let Some(generation) = self.begin_auth_job(&account.uuid) else {
                continue;
            };
            self.in_background(move |tell| {
                let progress = |phase| {
                    tell.send(Msg::AccountProgress(
                        account.uuid.clone(),
                        generation,
                        phase,
                    ))
                };
                progress(account::session::Phase::Keychain);
                let store = account::store();
                let mut status =
                    account::session::status_with_progress(store, &account.uuid, &progress);
                if renew_due && status.state == account::session::State::NeedsRefresh {
                    if let Ok(renewed) =
                        account::session::refresh_with_progress(&account.uuid, &progress)
                    {
                        status = renewed;
                    }
                }
                tell.send(Msg::AccountStatus(account.uuid, generation, status));
            });
        }
    }

    pub(super) fn refresh_account(&mut self, uuid: String) {
        let Some(generation) = self.begin_auth_job(&uuid) else {
            return;
        };
        self.in_background(move |tell| {
            let progress = |phase| tell.send(Msg::AccountProgress(uuid.clone(), generation, phase));
            progress(account::session::Phase::Keychain);
            if let Ok(status) = account::session::refresh_with_progress(&uuid, &progress) {
                tell.send(Msg::AccountStatus(uuid, generation, status));
            } else {
                // It may have been removed through the CLI while this window was open.
                tell.send(Msg::AccountRemoved(
                    uuid,
                    generation,
                    Ok(Accounts::load().has_owner(account::store())),
                ));
            }
        });
    }

    /// Generic Add account and targeted re-sign-in share the launcher-owned device-code flow.
    pub(super) fn start_sign_in(&mut self) {
        self.begin_sign_in(None);
    }

    pub(super) fn re_sign_in(&mut self, uuid: &str) {
        if let Some(account) = self.accounts.find(uuid).cloned() {
            self.begin_sign_in(Some(account));
        }
    }

    fn begin_sign_in(&mut self, target: Option<account::Account>) {
        self.cancel_sign_in();
        self.accounts_ui.sign_in = Some(match &target {
            Some(account) => SignIn::start_for(account.clone()),
            None => SignIn::start(),
        });
        self.accounts_ui.sign_in_target = target;
        self.accounts_ui.sign_in_state = Some(SignInState::Starting);
    }

    pub(super) fn cancel_sign_in(&mut self) {
        if let Some(old) = self.accounts_ui.sign_in.take() {
            old.cancel();
        }
        self.accounts_ui.sign_in_state = None;
        self.accounts_ui.sign_in_target = None;
    }

    pub(super) fn auth_moved(
        &mut self,
        uuid: String,
        generation: u64,
        phase: account::session::Phase,
    ) {
        if self.accounts_ui.auth_busy.get(&uuid) == Some(&generation) {
            self.accounts_ui.auth_progress.insert(uuid, phase);
        }
    }

    fn end_auth_job(&mut self, uuid: &str, generation: u64) -> bool {
        if self.accounts_ui.auth_busy.get(uuid) != Some(&generation) {
            return false;
        }
        self.accounts_ui.auth_busy.remove(uuid);
        self.accounts_ui.auth_progress.remove(uuid);
        true
    }

    pub(super) fn auth_checked(
        &mut self,
        uuid: String,
        generation: u64,
        status: account::session::Status,
    ) {
        if !self.end_auth_job(&uuid, generation) || self.accounts.find(&uuid).is_none() {
            return;
        }
        if status.state == account::session::State::Ready {
            self.owner = Some(true);
        }
        self.accounts_ui.auth_status.insert(uuid, status);
        // A refresh may have saved a new name or skin: show it now.
        if self.reload_accounts() {
            self.load_profile_heads();
        }
    }

    pub(super) fn account_removed(
        &mut self,
        uuid: String,
        generation: u64,
        result: Result<bool, String>,
    ) {
        if !self.end_auth_job(&uuid, generation) {
            return;
        }
        match result {
            Ok(owns) => {
                self.reload_accounts();
                self.accounts_ui.auth_status.remove(&uuid);
                self.owner = Some(owns);
            }
            Err(e) => self.accounts_ui.sign_in_state = Some(SignInState::Done(Err(e))),
        }
    }

    pub(super) fn sign_in_moved(&mut self) -> bool {
        let Some(sign_in) = &self.accounts_ui.sign_in else {
            return false;
        };
        let Some(state) = sign_in_change(&mut self.accounts_ui.sign_in_state, sign_in.state())
        else {
            return false;
        };
        if let SignInState::Done(Ok(ref account)) = state {
            self.accounts_ui.auth_busy.remove(&account.uuid);
            self.accounts_ui.sign_in_target = None;
            self.reload_accounts();
            self.load_profile_heads();
            self.load_auth_statuses(false);
            // Sign-in refuses accounts that don't own the game.
            self.owner = Some(true);
            self.owner_screen_open = false;
            self.game.problem = None;
            if !self.settings.owner_notice_seen {
                self.settings.owner_notice_seen = true;
                self.save_settings();
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_completed_sign_in_is_handled_once_across_ui_polls() {
        use super::{sign_in_change, SignInState};
        let mut shown = None;
        let completed = SignInState::Done(Ok(eclipse_launcher::account::Account {
            uuid: "11111111111111111111111111111111".into(),
            name: "TestAccount".into(),
            skin: None,
            cape: None,
        }));
        assert!(sign_in_change(&mut shown, SignInState::Starting).is_some());
        assert!(sign_in_change(&mut shown, SignInState::Starting).is_none());
        let mut account_reloads = 0;
        for _ in 0..100 {
            if matches!(
                sign_in_change(&mut shown, completed.clone()),
                Some(SignInState::Done(Ok(_)))
            ) {
                account_reloads += 1;
            }
        }
        assert_eq!(
            account_reloads, 1,
            "completed sign-in must not restart account checks every frame"
        );
        let failed = SignInState::Done(Err("Sign-in cancelled".into()));
        assert!(sign_in_change(&mut shown, failed.clone()).is_some());
        assert!(sign_in_change(&mut shown, failed).is_none());
    }
}
