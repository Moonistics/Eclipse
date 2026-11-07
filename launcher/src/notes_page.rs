use crate::page::Page;
use crate::ui::ButtonKind;
use crate::{dates, layout, ui, Launcher, Load};
use eclipse_launcher::backend::Release;
use eclipse_launcher::i18n::tr;
use eclipse_theme::glass::space;
use gpui::prelude::*;
use gpui::{div, px, ClickEvent, Div, SharedString, Stateful};

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
                return page.child(ui::empty_card(Self::releases_failed(
                    "notes-failed",
                    ui::page_only(tr("Couldn't load patch notes")),
                    e,
                    ButtonKind::Primary,
                    cx,
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
                let notes = release.notes();
                ui::card()
                    .flex()
                    .flex_col()
                    .gap(px(space::LG))
                    .p(px(ui::CARD_PADDING))
                    .child(notes_head(release))
                    .child(div().child(if notes.is_empty() {
                        ui::empty_line(
                            SharedString::from(format!("notes-none-{}", release.tag)),
                            tr("No notes for this release."),
                        )
                        .into_any_element()
                    } else {
                        ui::markdown::render(&notes)
                    }))
            },
        );
        page.child(list)
    }
}

pub(super) fn notes_head(release: &Release) -> Div {
    ui::release_head(
        eclipse_launcher::nightly::display_name(&release.tag),
        dates::published(&release.date, ""),
        Vec::new(),
    )
}

impl Launcher {
    pub(super) fn releases_failed(
        id: &'static str,
        title: impl Into<ui::StateTitle>,
        error: &str,
        retry: ButtonKind,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        ui::error_state_with(
            id,
            title,
            error,
            cx.listener(|this, _: &ClickEvent, _, cx| {
                this.fetch_releases();
                cx.notify();
            }),
            vec![Self::proxy_settings_button(id, cx)],
            retry,
        )
    }
}
