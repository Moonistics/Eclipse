//! Settings' Data collection section, and what the window keeps about data collection (the
//! question itself is `consent_sheet.rs`). Everything goes through `eclipse_launcher::telemetry`.
use crate::menus::Asking;
use crate::scrollbar::Scrollbar;
use crate::ui::{ButtonKind, Press};
use crate::{ui, Launcher};
use eclipse_launcher::i18n::{tr, trf};
use eclipse_launcher::telemetry;
use eclipse_telemetry::{Choices, Consent, Kind as DataKind};
use eclipse_theme::glass::{ink, space, step};
use gpui::prelude::*;
use gpui::{div, px, ClickEvent, Div, FontWeight};

pub(super) struct Collection {
    pub(super) consent: Consent,
    pub(super) body_scroll: Scrollbar,
    pub(super) json_scroll: Scrollbar,
    /// The question's switches, until the player answers.
    pub(super) draft: Choices,
    /// "View what would be sent": a line on what it is, and the JSON.
    pub(super) view: Option<(String, String)>,
    /// What "Delete my data" did, or why it could not.
    pub(super) note: Option<String>,
}

impl Collection {
    pub(super) fn load() -> Self {
        Self {
            consent: telemetry::consent(),
            body_scroll: Scrollbar::default(),
            json_scroll: Scrollbar::default(),
            draft: Choices::NONE,
            view: None,
            note: None,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum Asked {
    Sheet,
    Settings,
}

pub(super) fn kind_id(asked: Asked, kind: DataKind) -> &'static str {
    match (asked, kind) {
        (Asked::Sheet, DataKind::Perf) => "ask-perf",
        (Asked::Sheet, DataKind::Crash) => "ask-crash",
        (Asked::Sheet, DataKind::Bug) => "ask-bug",
        (Asked::Sheet, DataKind::Benchmark) => "ask-benchmark",
        (Asked::Settings, DataKind::Perf) => "data-perf",
        (Asked::Settings, DataKind::Crash) => "data-crash",
        (Asked::Settings, DataKind::Bug) => "data-bug",
        (Asked::Settings, DataKind::Benchmark) => "data-benchmark",
    }
}

const JSON_HEIGHT: f32 = 260.;

/// The exact JSON, in mono, scrolling inside a well.
pub(super) fn json_view(view: &(String, String), scroll: &Scrollbar) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(space::SM))
        .child(ui::card_note(view.0.clone()))
        .child(
            scroll.wrap(
                ui::well()
                    .id("data-json")
                    .max_h(px(JSON_HEIGHT))
                    .overflow_y_scroll()
                    .child(
                        ui::mono(step::CAPTION, ink::BODY)
                            .children(view.1.lines().map(|l| div().child(l.to_string()))),
                    ),
            ),
        )
}

impl Launcher {
    pub(super) fn toggle_view(&mut self) {
        self.collection.view = match self.collection.view {
            Some(_) => None,
            None => Some(telemetry::preview()),
        };
    }

    pub(super) fn collection_section(&mut self, cx: &mut Context<Self>) -> Div {
        let consent = self.collection.consent.clone();
        let mut list = div().flex().flex_col();
        for &(kind, name, what) in telemetry::SWITCHES.iter() {
            let on = !consent.needs_asking() && consent.choices.allows(kind);
            let row = ui::toggle_row(
                kind_id(Asked::Settings, kind),
                tr(name),
                Some(tr(what).into()),
                on,
            )
            .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                let mut choices = if this.collection.consent.needs_asking() {
                    Choices::NONE
                } else {
                    this.collection.consent.choices
                };
                choices.set(kind, !on);
                this.answer_consent(choices);
                cx.notify();
            }));
            list = list.child(row);
        }
        let waiting = eclipse_telemetry::outbox::pending(&telemetry::home()).len();
        let id = match consent.install_id.as_str() {
            "" => tr("No install ID: nothing has been collected").to_string(),
            id => trf(
                "Install ID {prefix}…{suffix}",
                &[("prefix", &id[..4]), ("suffix", &id[id.len() - 4..])],
            ),
        };
        let server = if eclipse_telemetry::endpoint().is_some() {
            trf(
                "{count} waiting to be sent",
                &[("count", &waiting.to_string())],
            )
        } else {
            tr("Nothing is sent").into()
        };
        let actions = if self.is_asking(&Asking::Collection) {
            ui::confirm(
                "data-delete",
                ui::Confirm::DeleteEverything,
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.asking = None;
                    cx.notify();
                }),
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.collection.note = Some(match telemetry::delete_my_data() {
                        Ok(done) => done,
                        Err(e) => e,
                    });
                    this.collection.consent = telemetry::consent();
                    this.collection.view = None;
                    this.asking = None;
                    cx.notify();
                }),
            )
        } else {
            div()
                .flex()
                .flex_wrap()
                .gap(px(space::SM))
                .child(
                    ui::button(
                        "data-view",
                        None,
                        tr(if self.collection.view.is_some() {
                            "Hide"
                        } else {
                            "View what would be sent"
                        }),
                        ButtonKind::Secondary,
                        ui::ButtonSize::Md,
                    )
                    .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.toggle_view();
                        cx.notify();
                    })),
                )
                .child(
                    ui::button(
                        "data-delete",
                        None,
                        tr("Delete my data…"),
                        ButtonKind::Secondary,
                        ui::ButtonSize::Md,
                    )
                    .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.asking = Some(Asking::Collection);
                        cx.notify();
                    })),
                )
        };
        list = list.child(ui::setting_row_stacked(
            tr("Your data"),
            Some(
                ui::mono(step::NOTE, ink::MUTED)
                    .child(format!("{id}  ·  {server}"))
                    .into_any_element(),
            ),
            actions.into_any_element(),
        ));
        let inset = |d: Div| d.px(px(ui::SETTING_ROW_PADDING));
        list.child(inset(ui::card_note(tr(telemetry::NEVER))))
            .children(self.collection.note.clone().map(|n| {
                inset(
                    ui::text(step::BODY, FontWeight::NORMAL, ink::FULL)
                        .pt(px(space::MD))
                        .child(ui::said("data-note", n)),
                )
            }))
            .children(
                self.collection
                    .view
                    .as_ref()
                    .map(|v| inset(json_view(v, &self.collection.json_scroll).pt(px(space::MD)))),
            )
    }
}
