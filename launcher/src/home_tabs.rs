//! The selected instance's tabs under Home's head, one at a time instead of one long page.
use crate::page::Page;
use crate::tab_strip::{Strip, StripTab};
use crate::ui::Press;
use crate::{layout, notes_page, ui, Launcher, Load};
use eclipse_launcher::backend::Release;
use eclipse_launcher::i18n::{tr, trf};
use eclipse_theme::glass::space;
use gpui::prelude::*;
use gpui::{div, px, AnyElement, ClickEvent, Div};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HomeTab {
    Mods,
    Worlds,
    Multiplayer,
    Packs,
    Screenshots,
    Console,
    PatchNotes,
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
        tabs.push((HomeTab::Multiplayer, self.servers_count(cx)));
        tabs.push((HomeTab::Packs, self.packs_count(cx)));
        tabs.push((HomeTab::Screenshots, self.screenshots_count(cx)));
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
    fn label(self) -> &'static str {
        match self {
            HomeTab::PatchNotes => tr("Patch notes"),
            HomeTab::Mods => tr("Mods"),
            HomeTab::Worlds => tr("Worlds"),
            HomeTab::Multiplayer => tr("Multiplayer"),
            HomeTab::Packs => tr("Packs"),
            HomeTab::Screenshots => tr("Screenshots"),
            HomeTab::Console => tr("Console"),
        }
    }

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

    fn comes_and_goes(self) -> bool {
        matches!(self, HomeTab::Console | HomeTab::PatchNotes)
    }
}

impl Launcher {
    pub(super) fn home_tab_card(&mut self, cx: &mut Context<Self>) -> Div {
        let tabs = self.home_tab_list(cx);
        let kept: Vec<HomeTab> = tabs
            .iter()
            .map(|(tab, _)| *tab)
            .filter(|tab| !tab.comes_and_goes())
            .collect();
        let changed = self
            .home_tabs_drawn
            .as_ref()
            .is_some_and(|drawn| *drawn != kept);
        if changed || self.home_tab.is_none() {
            self.home_tab = tabs.first().map(|(tab, _)| *tab);
        }
        self.home_tabs_drawn = Some(kept);
        let Some(first) = tabs.first().map(|(tab, _)| *tab) else {
            return div();
        };
        let open = tabs
            .iter()
            .map(|(tab, _)| *tab)
            .find(|tab| Some(*tab) == self.home_tab)
            .unwrap_or(first);
        let tabs = tabs
            .into_iter()
            .map(|(tab, count)| StripTab {
                value: tab,
                label: tab.label().into(),
                count: count.filter(|n| *n > 0).map(|n| (n, tab.counted(n).into())),
            })
            .collect();
        let aside = match open {
            HomeTab::Worlds => self.worlds_folder(),
            HomeTab::Multiplayer => self.add_server_button(cx),
            _ => None,
        };
        let room = layout::content_width(self.window_width)
            - aside.as_ref().map_or(0., |(_, width)| width + space::MD);
        let (strip, panel) = self.tab_strip(
            Strip::Home,
            tabs,
            open,
            room,
            |this, tab, _| this.home_tab = Some(tab),
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
        let head = div()
            .flex()
            .items_start()
            .justify_between()
            .gap(px(space::MD))
            .child(strip)
            .children(aside.map(|(button, _)| button));
        div().flex().flex_col().child(head).child(
            panel.child(
                ui::card()
                    .flex()
                    .flex_col()
                    .p(px(ui::PANEL_INSET))
                    .children(body),
            ),
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
            .child(notes_page::notes_head(release))
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
                .child(
                    div()
                        .max_w(px(NOTES_WIDTH))
                        .child(ui::markdown::render(&notes)),
                ),
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
        .map(|r| (r, r.notes()))
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
