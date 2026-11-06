//! The selected instance's tabs under Home's head: the changelog of the Eclipse version it
//! plays, then its mods, worlds, servers, packs, screenshots and, once the game has said something, its
//! console. One at a time, instead of one long page.
use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum HomeTab {
    #[default]
    Changelog,
    Mods,
    Worlds,
    Multiplayer,
    Packs,
    Screenshots,
    Console,
}

impl HomeTab {
    /// `ECLIPSE_LAUNCHER_PAGE=worlds` (or `instance-mods`, `multiplayer`, `packs`, `screenshots`,
    /// `console`) opens Home on that tab, for screenshots.
    pub(super) fn from_env() -> Self {
        match std::env::var("ECLIPSE_LAUNCHER_PAGE").as_deref() {
            Ok("instance-mods") => HomeTab::Mods,
            Ok("worlds") => HomeTab::Worlds,
            Ok("multiplayer") => HomeTab::Multiplayer,
            Ok("packs") => HomeTab::Packs,
            Ok("screenshots") => HomeTab::Screenshots,
            Ok("console") => HomeTab::Console,
            _ => HomeTab::Changelog,
        }
    }
}

impl Launcher {
    /// The tabs the selected instance has now, with how many things each holds (None: not known
    /// yet, or nothing to count). The changelog only for Eclipse, Console only once there
    /// is something to show.
    pub(super) fn home_tab_list(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Vec<(HomeTab, Option<usize>)> {
        let Some(eclipse) = self.selected_instance().map(|i| i.java.is_none()) else {
            return vec![(HomeTab::Changelog, None)];
        };
        let mut tabs = Vec::new();
        if eclipse {
            tabs.push((HomeTab::Changelog, None));
        }
        if self.has_mods_tab() {
            tabs.push((HomeTab::Mods, self.instance_mods_count(cx)));
        }
        tabs.push((HomeTab::Worlds, self.worlds_count(cx)));
        tabs.push((HomeTab::Multiplayer, Some(self.servers_count())));
        tabs.push((HomeTab::Packs, Some(self.packs_count())));
        tabs.push((HomeTab::Screenshots, Some(self.screenshots_count())));
        if !self.prepare_log.is_empty() || self.client.is_some() {
            tabs.push((HomeTab::Console, None));
        }
        tabs
    }
}
