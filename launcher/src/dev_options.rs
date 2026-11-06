//! Developer options: a Settings section nothing in the window points to. Seven clicks on the
//! sidebar's "Launcher x.y.z" reveal it for this run; it stays shown while one of its options is
//! on. Its one option lets offline names play without an account that owns the game
//! (`backend::Settings::dev_offline`). The game still comes from the player's own install: the
//! launcher downloads Mojang's files only for an owner (`backend::download_for_owner`).
use super::*;

/// Clicks on the launcher's version that reveal the section.
const CLICKS: u8 = 7;

impl Launcher {
    /// A click on the sidebar's launcher version; the last one opens Settings on the section.
    pub(super) fn version_clicked(&mut self) {
        if self.dev_options {
            return;
        }
        self.dev_clicks += 1;
        if self.dev_clicks >= CLICKS {
            self.dev_options = true;
            self.show_settings(settings_page::SettingsSection::Developer);
        }
    }

    /// The section, when revealed or when one of its options is on.
    pub(super) fn dev_options_shown(&self) -> bool {
        self.dev_options || self.settings.dev_offline
    }

    pub(super) fn dev_offline_row(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        crate::ui::toggle_row(
            "dev-offline",
            tr("Offline names without an account"),
            Some(tr("Minecraft still comes from your own install.").into()),
            self.settings.dev_offline,
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.settings.dev_offline = !this.settings.dev_offline;
            this.save_settings();
            cx.notify();
        }))
    }
}
