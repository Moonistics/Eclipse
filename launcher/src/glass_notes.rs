use super::*;
use crate::glass_ui::{self as ui};
use eclipse_theme::glass::{ink, radius};

impl Launcher {
    pub(super) fn glass_notes(&mut self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
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
                let retry = ui::primary("notes-retry", tr("Try again"), ui::Size::Md).on_press(
                    cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.fetch_releases();
                        cx.notify();
                    }),
                );
                return page.child(ui::empty_card(ui::error_state(
                    "notes-failed",
                    tr("Couldn't load patch notes"),
                    e,
                    retry,
                )));
            }
            Load::Ready(r) if r.is_empty() => {
                return page.child(ui::empty_card(ui::empty_state(
                    "notes-empty",
                    "information-circle",
                    tr("No patch notes yet"),
                    "",
                    vec![],
                )));
            }
            Load::Ready(r) => r,
        };
        let list = self.long_list.clone().column(
            &self.content_scroll.handle,
            releases.len(),
            10.,
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
                let day = glass_versions::published(&release.date, "");
                crate::glass::plate(radius::PANEL)
                    .flex()
                    .flex_col()
                    .gap(px(12.))
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
                        markdown::render(&release.body, &t)
                    }))
            },
        );
        page.child(list)
    }
}
