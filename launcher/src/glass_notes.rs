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
                return page.child(ui::empty_card(ui::empty_state(
                    "notes-loading",
                    "information-circle",
                    tr("Loading patch notes…"),
                    "",
                    vec![],
                )))
            }
            Load::Failed(e) => {
                let retry = button("retry-notes", "Try again", Kind::Secondary, &t).on_click(
                    cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.fetch_releases();
                        cx.notify();
                    }),
                );
                return page.child(ui::empty_card(ui::empty_state(
                    "notes-failed",
                    "information-circle",
                    tr("Couldn't load patch notes"),
                    e.clone(),
                    vec![retry],
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
                    .p(px(24.))
                    .child(
                        ui::text(16., FontWeight::SEMIBOLD, ink::FULL)
                            .line_height(px(24.))
                            .child(
                                gpui::StyledText::new(format!("{title} · {day}")).with_highlights(
                                    [(
                                        title.len()..title.len() + " · ".len() + day.len(),
                                        gpui::HighlightStyle {
                                            font_weight: Some(FontWeight::NORMAL),
                                            color: Some(ui::white(ink::MUTED)),
                                            ..Default::default()
                                        },
                                    )],
                                ),
                            ),
                    )
                    .child(div().child(if release.body.trim().is_empty() {
                        ui::text(14., FontWeight::NORMAL, ink::MUTED)
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
