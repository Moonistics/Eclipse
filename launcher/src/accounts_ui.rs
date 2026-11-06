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
