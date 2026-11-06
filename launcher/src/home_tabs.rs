//! The selected instance's tabs under Home's head: the patch notes of the Eclipse version it
//! plays, then its mods, worlds, servers, packs, screenshots and, once the game has said something, its
//! console. One at a time, instead of one long page. The strip, and under it one card with the
//! open tab. Patch notes shows the changes in the Eclipse version the instance plays.
use super::*;
use crate::ui::{self, white};
use eclipse_theme::glass::{ink, radius, space, step};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum HomeTab {
    #[default]
    PatchNotes,
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
            _ => HomeTab::PatchNotes,
        }
    }
}

impl Launcher {
    /// The tabs the selected instance has now, with how many things each holds (None: not known
    /// yet, or nothing to count). Patch notes only for Eclipse, Console only once there
    /// is something to show.
    pub(super) fn home_tab_list(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Vec<(HomeTab, Option<usize>)> {
        let Some(eclipse) = self.selected_instance().map(|i| i.java.is_none()) else {
            return vec![(HomeTab::PatchNotes, None)];
        };
        let mut tabs = Vec::new();
        if eclipse {
            tabs.push((HomeTab::PatchNotes, None));
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

const NOTES_WIDTH: f32 = 720.;

const BULLET: f32 = 5.;

const COUNT_GAP: f32 = 8.;

fn tab_width(label: &str, count: Option<usize>) -> f32 {
    2. * ui::TAB_PAD
        + ui::press_width(label)
        + count.map_or(0., |n| COUNT_GAP + ui::press_width(&n.to_string()))
}

impl Launcher {
    pub(super) fn home_tab_card(&mut self, cx: &mut Context<Self>) -> Div {
        let tabs = self.home_tab_list(cx);
        let open = tabs
            .iter()
            .map(|(tab, _)| *tab)
            .find(|tab| *tab == self.home_tab)
            .or_else(|| tabs.first().map(|(tab, _)| *tab))
            .unwrap_or_default();
        let named: Vec<_> = tabs
            .into_iter()
            .map(|(tab, count)| {
                let name = match tab {
                    HomeTab::PatchNotes => tr("Patch notes"),
                    HomeTab::Mods => tr("Mods"),
                    HomeTab::Worlds => tr("Worlds"),
                    HomeTab::Multiplayer => tr("Multiplayer"),
                    HomeTab::Packs => tr("Packs"),
                    HomeTab::Screenshots => tr("Screenshots"),
                    HomeTab::Console => tr("Console"),
                };
                (tab, name, count.filter(|n| *n > 0))
            })
            .collect();
        let room = layout::content_width(self.window_width) + 2. * ui::TAB_PAD;
        let counted: f32 = named.iter().map(|(_, name, n)| tab_width(name, *n)).sum();
        let counts = counted <= room;
        let strip = ui::tab_strip(
            "home-tabs",
            named.into_iter().map(|(tab, name, count)| {
                ui::count_tab(
                    format!("home-tab-{tab:?}"),
                    name,
                    count.filter(|_| counts),
                    tab == open,
                )
                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.home_tab = tab;
                    cx.notify();
                }))
            }),
        );
        let body: Option<AnyElement> = match open {
            HomeTab::PatchNotes => Some(self.patch_notes(cx).into_any_element()),
            HomeTab::Mods => Some(self.mods_tab(cx).into_any_element()),
            HomeTab::Worlds => self.worlds_tab(cx).map(IntoElement::into_any_element),
            HomeTab::Multiplayer => self.multiplayer_tab(cx).map(IntoElement::into_any_element),
            HomeTab::Packs => self.packs_tab(cx).map(IntoElement::into_any_element),
            HomeTab::Screenshots => self.screenshots_tab(cx).map(IntoElement::into_any_element),
            HomeTab::Console => self.console_tab(cx).map(IntoElement::into_any_element),
        };
        div().flex().flex_col().child(strip).child(
            ui::card(radius::PANEL)
                .flex()
                .flex_col()
                .p(px(ui::PANEL_INSET))
                .children(body),
        )
    }

    /// Patch notes: the changes in the Eclipse version played, by section (the newest Nightly's
    /// when that version lists none), else the newest launcher release's; All patch notes opens
    /// Patch notes.
    fn patch_notes(&self, cx: &mut Context<Self>) -> Div {
        let source = match &self.releases {
            Load::Loading => {
                return div().child(ui::loading_state(
                    "patch-notes-loading",
                    tr("Loading patch notes…"),
                ))
            }
            Load::Failed(e) => {
                return div().child(ui::error_state_with(
                    "patch-notes-failed",
                    tr("Couldn't load patch notes"),
                    e,
                    cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.fetch_releases();
                        cx.notify();
                    }),
                    Some(Self::proxy_settings_button("patch-notes-failed", cx)),
                ));
            }
            Load::Ready(list) => patch_source(list, self.selected().map(String::as_str)),
        };
        let Some((note, sections)) = source.filter(|(_, sections)| !sections.is_empty()) else {
            return div().child(ui::empty_state(
                "patch-notes-empty",
                "information-circle",
                tr("No patch notes yet"),
                tr("Patch notes come with each Eclipse release."),
                vec![],
            ));
        };
        let many = sections.len() > 1;
        let bullet = |item: String| {
            div()
                .flex()
                .gap(px(space::MD))
                .child(
                    div()
                        .flex_none()
                        .mt(px((step::BODY.line - BULLET) / 2.))
                        .size(px(BULLET))
                        .rounded_full()
                        .bg(white(ink::DIM)),
                )
                .child(
                    ui::text(step::BODY, FontWeight::NORMAL, ink::STRONG)
                        .min_w_0()
                        .child(item),
                )
        };
        let list = div()
            .flex()
            .flex_col()
            .gap(px(space::LG))
            .px(px(space::MD))
            .max_w(px(NOTES_WIDTH))
            .children(sections.into_iter().map(|(section, items)| {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(space::SM))
                    .children(many.then(|| ui::label(section)))
                    .children(items.into_iter().map(bullet))
            }));
        div()
            .flex()
            .flex_col()
            .child(ui::list_head(note).child(
                ui::link("patch-notes-all", tr("All patch notes")).on_press(cx.listener(
                    |this, _: &ClickEvent, _, cx| {
                        this.go(Page::Notes);
                        cx.notify();
                    },
                )),
            ))
            .child(list.pb(px(space::MD)))
    }
}

/// A release's changes: each section's name and its lines.
type Sections = Vec<(String, Vec<String>)>;

/// The patch notes from `releases` and the line over it: the release `played` installs as when it
/// lists changes, else the newest Nightly that does ("Nightly 2026-11-01 · 4aa6854");
/// without a version played, the newest launcher release ("Launcher 0.2.37 · Nov 2").
fn patch_source(releases: &[Release], played: Option<&str>) -> Option<(String, Sections)> {
    let day = |r: &Release| dates::published(&r.date, "");
    if let Some(played) = played {
        let nightly = releases
            .iter()
            .filter(|r| r.tag == played)
            .chain(releases.iter().filter(|r| r.nightly.is_some()))
            .map(|r| (r, patch_sections(&r.body)))
            .find(|(_, sections)| !sections.is_empty());
        if let Some((r, sections)) = nightly {
            return Some((r.name.clone(), sections));
        }
    }
    let r = releases
        .iter()
        .find(|r| r.nightly.is_none() && !r.prerelease)?;
    let version = r
        .tag
        .rsplit('@')
        .next()
        .unwrap_or(&r.tag)
        .trim_start_matches('v');
    let note = format!(
        "{} · {}",
        trf("Launcher {version}", &[("version", version)]),
        day(r)
    );
    Some((note, patch_sections(&r.body)))
}

/// A release's changes by section ("### New", "### Fixed"), each line without its pull request
/// numbers; the contributors' thanks and what follows the notes are left out.
fn patch_sections(body: &str) -> Sections {
    let mut out: Sections = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        if let Some(name) = line.strip_prefix("### ") {
            out.push((name.trim().to_string(), Vec::new()));
        } else if line == "---" || line.starts_with("Your launcher updates itself") {
            break;
        } else if let (Some(item), Some((section, items))) =
            (line.strip_prefix("- "), out.last_mut())
        {
            if section.as_str() != "Contributors" {
                items.push(strip_refs(item));
            }
        }
    }
    out.retain(|(name, items)| name != "Contributors" && !items.is_empty());
    out
}

/// "Fix the picker (#6389, fixes #6342)" → "Fix the picker".
fn strip_refs(item: &str) -> String {
    let mut s = item.trim();
    while let Some(open) = s.rfind(" (") {
        let tail = &s[open..];
        if tail.ends_with(')') && tail.contains('#') {
            s = s[..open].trim_end();
        } else {
            break;
        }
    }
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_release_body_becomes_its_sections_without_refs_or_thanks() {
        let body = "### New\n- Native mods: export a headless engine (#6462)\n\n### Fixed\n- Play with friends: any port (#6363, fixes #6342)\n- Keep (this) aside\n\n### Contributors\nThanks to @a.\n\nYour launcher updates itself. New here?\n\n---\nNot an official Minecraft product.";
        assert_eq!(
            patch_sections(body),
            vec![
                (
                    "New".to_string(),
                    vec!["Native mods: export a headless engine".to_string()]
                ),
                (
                    "Fixed".to_string(),
                    vec![
                        "Play with friends: any port".to_string(),
                        "Keep (this) aside".to_string()
                    ]
                ),
            ]
        );
    }

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
    fn the_patch_log_shows_the_played_nightly_else_the_newest_else_the_launcher() {
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
                "No list.",
            ),
            release(
                "launcher@0.2.37",
                "Eclipse launcher 0.2.37",
                "2026-11-02",
                false,
                "### Fixed\n- Launcher fix",
            ),
        ];
        let (note, sections) = patch_source(&releases, Some("nightly-2026-11-01-4aa6854")).unwrap();
        assert_eq!(note, "Nightly 2026-11-01 · 4aa6854");
        assert_eq!(
            sections,
            vec![("New".to_string(), vec!["Older change".to_string()])]
        );
        // A version that lists no changes: the newest Nightly's.
        let (note, _) = patch_source(&releases, Some("nightly-2026-10-30-0c02067")).unwrap();
        assert_eq!(note, "Nightly 2026-11-03 · 2944480");
        // No version played: the launcher's.
        let (note, sections) = patch_source(&releases, None).unwrap();
        assert_eq!(note, "Launcher 0.2.37 · Nov 2");
        assert_eq!(
            sections,
            vec![("Fixed".to_string(), vec!["Launcher fix".to_string()])]
        );
    }
}
