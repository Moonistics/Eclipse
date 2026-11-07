use super::*;
use crate::ui;
use eclipse_theme::glass::space;

impl Launcher {
    pub(super) fn notes_page(&mut self, cx: &mut Context<Self>) -> Div {
        let page = div()
            .flex()
            .flex_col()
            .child(ui::page_head(Page::Notes.title(), None));
        let releases = match &self.versions.releases {
            Load::Loading => {
                return page.child(ui::empty_card(ui::loading_state(
                    "notes-loading",
                    ui::page_only(tr("Loading patch notes…")),
                )))
            }
            Load::Failed(e) => {
                return page.child(ui::empty_card(ui::error_state_with(
                    "notes-failed",
                    ui::page_only(tr("Couldn't load patch notes")),
                    e,
                    cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.fetch_releases();
                        cx.notify();
                    }),
                    vec![Self::proxy_settings_button("notes-failed", cx)],
                    ButtonKind::Primary,
                )));
            }
            Load::Ready(r) if r.is_empty() => {
                return page.child(ui::empty_card(ui::empty_state(
                    "notes-empty",
                    crate::icons::NOTES,
                    ui::page_only(tr("No patch notes yet")),
                    tr("Eclipse hasn't published a version yet."),
                    vec![],
                )));
            }
            Load::Ready(r) => r,
        };
        let list = self.long_list.clone().column(
            &self.content_scroll.handle,
            releases.len(),
            layout::CARD_GAP,
            |_| false,
            |i| {
                let release = &releases[i];
                let notes = release_notes(&release.body);
                ui::card()
                    .flex()
                    .flex_col()
                    .gap(px(space::LG))
                    .p(px(ui::CARD_PADDING))
                    .child(ui::release_head(
                        eclipse_launcher::nightly::display_name(&release.tag),
                        dates::published(&release.date, ""),
                        Vec::new(),
                    ))
                    .child(div().child(if notes.is_empty() {
                        ui::empty_line(
                            SharedString::from(format!("notes-none-{}", release.tag)),
                            tr("No notes for this release."),
                        )
                        .into_any_element()
                    } else {
                        markdown::render(&notes)
                    }))
            },
        );
        page.child(list)
    }
}

/// A release's notes as players read them: its changes without their pull request numbers; the
/// contributors' thanks, what follows the notes and the raw HTML every view skips are left out.
pub(super) fn release_notes(body: &str) -> String {
    let mut out = Vec::new();
    let mut thanks = false;
    for line in body.lines() {
        let said = line.trim();
        if said == "---" || said.starts_with("Your launcher updates itself") {
            break;
        }
        if let Some(name) = said.strip_prefix("### ") {
            thanks = name.trim() == "Contributors";
        }
        if thanks || said.starts_with('<') {
            continue;
        }
        out.push(match said.strip_prefix("- ") {
            Some(item) => format!(
                "{}- {}",
                &line[..line.len() - line.trim_start().len()],
                strip_refs(item)
            ),
            None => line.to_string(),
        });
    }
    out.join("\n").trim().to_string()
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
    fn a_release_body_becomes_its_notes_without_refs_thanks_or_footer() {
        let body = "<h1>Want to play?</h1>\n\n### New\n- Native mods: export a headless engine (#6462)\n\n### Fixed\n- Play with friends: any port (#6363, fixes #6342)\n- Keep (this) aside\n\n### Contributors\nThanks to @a.\n\nYour launcher updates itself. New here?\n\n---\nNot an official Minecraft product.";
        assert_eq!(
            release_notes(body),
            "### New\n- Native mods: export a headless engine\n\n### Fixed\n- Play with friends: any port\n- Keep (this) aside"
        );
        assert_eq!(
            release_notes("Rebuilt from the latest main.\n\n---\nNot official."),
            "Rebuilt from the latest main."
        );
        assert_eq!(release_notes("### Contributors\nThanks to @a."), "");
    }
}
