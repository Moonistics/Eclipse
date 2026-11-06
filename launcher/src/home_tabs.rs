//! The selected instance's tabs under Home's head: its mods, worlds, servers, packs, screenshots,
//! once the game has said something its console, and last the patch notes of the Eclipse version
//! it plays. One at a time, instead of one long page. The strip, and under it one card with the
//! open tab. Patch notes shows the changes in the Eclipse version the instance plays.
use super::*;
use crate::tab_strip::StripTab;
use crate::ui;
use eclipse_theme::glass::space;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum HomeTab {
    Mods,
    #[default]
    Worlds,
    Multiplayer,
    Packs,
    Screenshots,
    Console,
    PatchNotes,
}

impl HomeTab {
    /// `ECLIPSE_LAUNCHER_PAGE=worlds` (or `instance-mods`, `multiplayer`, `packs`, `screenshots`,
    /// `console`, `patch-notes`) opens Home on that tab, for screenshots.
    pub(super) fn from_env() -> Self {
        match std::env::var("ECLIPSE_LAUNCHER_PAGE").as_deref() {
            Ok("instance-mods") => HomeTab::Mods,
            Ok("worlds") => HomeTab::Worlds,
            Ok("multiplayer") => HomeTab::Multiplayer,
            Ok("packs") => HomeTab::Packs,
            Ok("screenshots") => HomeTab::Screenshots,
            Ok("console") => HomeTab::Console,
            Ok("patch-notes") => HomeTab::PatchNotes,
            _ => HomeTab::default(),
        }
    }
}

impl Launcher {
    /// The tabs the selected instance has now, with how many things each holds (None: not known
    /// yet, or nothing to count). Console only once there is something to show, Patch notes last,
    /// only for Eclipse and once there are some.
    pub(super) fn home_tab_list(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Vec<(HomeTab, Option<usize>)> {
        let notes = self
            .has_patch_notes()
            .then_some((HomeTab::PatchNotes, None));
        let Some(eclipse) = self.selected_instance().map(|i| i.java.is_none()) else {
            return notes.into_iter().collect();
        };
        let mut tabs = Vec::new();
        if self.has_mods_tab() {
            tabs.push((HomeTab::Mods, self.instance_mods_count(cx)));
        }
        tabs.push((HomeTab::Worlds, self.worlds_count(cx)));
        tabs.push((HomeTab::Multiplayer, Some(self.servers_count())));
        tabs.push((HomeTab::Packs, Some(self.packs_count())));
        tabs.push((HomeTab::Screenshots, Some(self.screenshots_count())));
        if self.console_log().is_some() {
            tabs.push((HomeTab::Console, None));
        }
        if eclipse {
            tabs.extend(notes);
        }
        tabs
    }
}

const NOTES_WIDTH: f32 = layout::MEASURE_TEXT;

impl HomeTab {
    fn counted(self, n: usize) -> String {
        let count = n.to_string();
        let counted = [("count", count.as_str())];
        match (self, n) {
            (HomeTab::Mods, 1) => tr("1 mod").into(),
            (HomeTab::Mods, _) => trf("{count} mods", &counted),
            (HomeTab::Worlds, 1) => tr("1 world").into(),
            (HomeTab::Worlds, _) => trf("{count} worlds", &counted),
            (HomeTab::Multiplayer, 1) => tr("1 server").into(),
            (HomeTab::Multiplayer, _) => trf("{count} servers", &counted),
            (HomeTab::Packs, 1) => tr("1 pack").into(),
            (HomeTab::Packs, _) => trf("{count} packs", &counted),
            (HomeTab::Screenshots, 1) => tr("1 screenshot").into(),
            (HomeTab::Screenshots, _) => trf("{count} screenshots", &counted),
            (HomeTab::PatchNotes | HomeTab::Console, _) => count,
        }
    }
}

impl Launcher {
    pub(super) fn home_tab_card(&mut self, cx: &mut Context<Self>) -> Div {
        let tabs = self.home_tab_list(cx);
        if tabs.is_empty() {
            return div();
        }
        let open = tabs
            .iter()
            .map(|(tab, _)| *tab)
            .find(|tab| *tab == self.home_tab)
            .or_else(|| tabs.first().map(|(tab, _)| *tab))
            .unwrap_or_default();
        let tabs = tabs
            .into_iter()
            .map(|(tab, count)| StripTab {
                value: tab,
                label: match tab {
                    HomeTab::PatchNotes => tr("Patch notes"),
                    HomeTab::Mods => tr("Mods"),
                    HomeTab::Worlds => tr("Worlds"),
                    HomeTab::Multiplayer => tr("Multiplayer"),
                    HomeTab::Packs => tr("Packs"),
                    HomeTab::Screenshots => tr("Screenshots"),
                    HomeTab::Console => tr("Console"),
                }
                .into(),
                count: count.filter(|n| *n > 0).map(|n| (n, tab.counted(n).into())),
            })
            .collect();
        let strip = self.tab_strip(
            "home-tab",
            tabs,
            open,
            layout::content_width(self.window_width),
            |this, tab, _| this.home_tab = tab,
            cx,
        );
        let body: Option<AnyElement> = match open {
            HomeTab::PatchNotes => self.patch_notes(cx).map(IntoElement::into_any_element),
            HomeTab::Mods => Some(self.mods_tab(cx).into_any_element()),
            HomeTab::Worlds => self.worlds_tab(cx).map(IntoElement::into_any_element),
            HomeTab::Multiplayer => self.multiplayer_tab(cx).map(IntoElement::into_any_element),
            HomeTab::Packs => self.packs_tab(cx).map(IntoElement::into_any_element),
            HomeTab::Screenshots => self.screenshots_tab(cx).map(IntoElement::into_any_element),
            HomeTab::Console => self.console_tab(cx).map(IntoElement::into_any_element),
        };
        div().flex().flex_col().child(strip).child(
            ui::card()
                .flex()
                .flex_col()
                .p(px(ui::PANEL_INSET))
                .children(body),
        )
    }

    fn has_patch_notes(&self) -> bool {
        match &self.versions.releases {
            Load::Ready(list) => {
                patch_source(list, self.selected_version().map(String::as_str)).is_some()
            }
            Load::Loading => true,
            Load::Failed(_) => false,
        }
    }

    /// Patch notes: the notes of the Eclipse version played (the newest release's with notes
    /// when that version has none); All patch notes opens Patch notes.
    fn patch_notes(&self, cx: &mut Context<Self>) -> Option<Div> {
        let source = match &self.versions.releases {
            Load::Loading => {
                return Some(div().child(ui::loading_state(
                    "patch-notes-loading",
                    tr("Loading patch notes…"),
                )))
            }
            Load::Failed(_) => return None,
            Load::Ready(list) => patch_source(list, self.selected_version().map(String::as_str)),
        };
        let (release, notes) = source?;
        let head = div()
            .flex()
            .items_center()
            .gap(px(space::LG))
            .child(ui::release_head(
                eclipse_launcher::nightly::display_name(&release.tag),
                dates::published(&release.date, ""),
                Vec::new(),
            ))
            .child(
                ui::link("patch-notes-all", tr("All patch notes")).on_press(cx.listener(
                    |this, _: &ClickEvent, _, cx| {
                        this.go(Page::Notes);
                        cx.notify();
                    },
                )),
            );
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(space::LG))
                .p(px(space::MD))
                .child(head)
                .child(div().max_w(px(NOTES_WIDTH)).child(markdown::render(&notes))),
        )
    }
}

/// The release whose notes Home shows and those notes: the one `played` installs as when it has
/// notes, else the newest that does.
fn patch_source<'a>(
    releases: &'a [Release],
    played: Option<&str>,
) -> Option<(&'a Release, String)> {
    releases
        .iter()
        .filter(|r| Some(r.tag.as_str()) == played)
        .chain(releases)
        .map(|r| (r, notes_page::release_notes(&r.body)))
        .find(|(_, notes)| !notes.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, name: &str, date: &str, nightly: bool, body: &str) -> Release {
        Release {
            tag: tag.into(),
            name: name.into(),
            body: body.into(),
            date: date.into(),
            time: String::new(),
            asset: None,
            prerelease: false,
            nightly: nightly.then(|| "nightly".to_string()),
        }
    }

    #[test]
    fn the_patch_log_shows_the_played_version_else_the_newest_with_notes() {
        let releases = [
            release(
                "nightly-2026-11-03-2944480",
                "Nightly 2026-11-03 · 2944480",
                "2026-11-03",
                true,
                "### Fixed\n- Newest fix (#2)",
            ),
            release(
                "nightly-2026-11-01-4aa6854",
                "Nightly 2026-11-01 · 4aa6854",
                "2026-11-01",
                true,
                "### New\n- Older change (#1)",
            ),
            release(
                "nightly-2026-10-30-0c02067",
                "Nightly 2026-10-30 · 0c02067",
                "2026-10-30",
                true,
                "### Contributors\n- Thanks to @a.",
            ),
        ];
        let shown =
            |played| patch_source(&releases, played).map(|(r, notes)| (r.tag.as_str(), notes));
        assert_eq!(
            shown(Some("nightly-2026-11-01-4aa6854")),
            Some((
                "nightly-2026-11-01-4aa6854",
                "### New\n- Older change".to_string()
            ))
        );
        assert_eq!(
            shown(Some("nightly-2026-10-30-0c02067")),
            Some((
                "nightly-2026-11-03-2944480",
                "### Fixed\n- Newest fix".to_string()
            )),
            "a version without notes shows the newest notes"
        );
        assert_eq!(
            shown(None).map(|(tag, _)| tag),
            Some("nightly-2026-11-03-2944480")
        );
    }
}
