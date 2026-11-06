use super::*;
use crate::ui;
use eclipse_theme::glass::{ink, space};

impl Launcher {
    pub(super) fn notes_page(&mut self, cx: &mut Context<Self>) -> Div {
        let page = div()
            .flex()
            .flex_col()
            .child(ui::page_head(Page::Notes.title(), ""));
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
                let title = eclipse_launcher::nightly::display_name(&release.tag);
                let day = dates::published(&release.date, "");
                ui::card()
                    .flex()
                    .flex_col()
                    .gap(px(space::MD))
                    .p(px(ui::CARD_PADDING))
                    .child(
                        ui::text(
                            eclipse_theme::glass::step::TITLE,
                            FontWeight::SEMIBOLD,
                            ink::FULL,
                        )
                        .child(
                            gpui::StyledText::new(format!("{title} · {day}")).with_highlights([(
                                title.len()..title.len() + " · ".len() + day.len(),
                                gpui::HighlightStyle {
                                    font_weight: Some(FontWeight::NORMAL),
                                    color: Some(ui::white(ink::MUTED)),
                                    ..Default::default()
                                },
                            )]),
                        ),
                    )
                    .child(div().child(if release.body.trim().is_empty() {
                        ui::empty_line(
                            SharedString::from(format!("notes-none-{}", release.tag)),
                            tr("No notes for this release."),
                        )
                        .into_any_element()
                    } else {
                        markdown::render(&release.body)
                    }))
            },
        );
        page.child(list)
    }
}
