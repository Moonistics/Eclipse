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
mod accounts;
mod game_session;
mod msa;
pub mod session;

pub use crate::secrets::{no_keychain, store, SecretStore};
pub use accounts::*;
pub use game_session::*;
pub use msa::*;

#[cfg(test)]
mod tests;
