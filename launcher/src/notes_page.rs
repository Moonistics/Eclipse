use super::*;
use crate::ui;
use eclipse_theme::glass::{ink, radius, space};

impl Launcher {
    pub(super) fn notes_page(&mut self, cx: &mut Context<Self>) -> Div {
        let page = div()
            .flex()
            .flex_col()
            .child(ui::page_head(tr("Patch notes"), ""));
        let releases = match &self.releases {
            Load::Loading => {
                return page.child(ui::empty_card(ui::loading_state(
                    "notes-loading",
                    tr("Loading patch notes…"),
                )))
            }
            Load::Failed(e) => {
                return page.child(ui::empty_card(ui::error_state_with(
                    "notes-failed",
                    tr("Couldn't load patch notes"),
                    e,
                    cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.fetch_releases();
                        cx.notify();
                    }),
                    Some(Self::proxy_settings_button("notes-failed", cx)),
                    ButtonKind::Primary,
                )));
            }
            Load::Ready(r) if r.is_empty() => {
                return page.child(ui::empty_card(ui::empty_state(
                    "notes-empty",
                    "information-circle",
                    tr("No patch notes yet"),
                    tr("Patch notes come with each Eclipse release."),
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
                let title = release
                    .name
                    .split_once(": ")
                    .map(|(_, rest)| rest.to_string())
                    .unwrap_or_else(|| release.name.clone());
                let title = if title.is_empty() {
                    release.tag.clone()
                } else {
                    title
                };
                let day = dates::published(&release.date, "");
                ui::card(radius::PANEL)
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
                        ui::text(
                            eclipse_theme::glass::step::BODY,
                            FontWeight::NORMAL,
                            ink::MUTED,
                        )
                        .child(tr("No notes for this release."))
                        .into_any_element()
                    } else {
                        markdown::render(&release.body)
                    }))
            },
        );
        page.child(list)
    }
}
