//! Data collection in the window: the question asked once (and again when what is collected
//! changes), and its section in Settings. Everything goes through `eclipse_launcher::telemetry`.
use super::*;
use crate::menus::Asking;
use crate::ui;
use eclipse_launcher::telemetry;
use eclipse_telemetry::{Choices, Consent, Kind as DataKind};
use eclipse_theme::glass::{ink, space, step};

/// What the window keeps about data collection.
pub(crate) struct Collection {
    pub consent: Consent,
    body_scroll: Scrollbar,
    json_scroll: Scrollbar,
    /// The question's switches, until the player answers.
    pub draft: Choices,
    /// "View what would be sent": a line on what it is, and the JSON.
    pub view: Option<(String, String)>,
    /// What "Delete my data" did, or why it could not.
    pub note: Option<String>,
}

impl Collection {
    pub fn load() -> Self {
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

fn kind_id(prefix: &str, kind: DataKind) -> &'static str {
    match (prefix, kind) {
        ("ask", DataKind::Perf) => "ask-perf",
        ("ask", DataKind::Crash) => "ask-crash",
        ("ask", DataKind::Bug) => "ask-bug",
        ("ask", DataKind::Benchmark) => "ask-benchmark",
        (_, DataKind::Perf) => "data-perf",
        (_, DataKind::Crash) => "data-crash",
        (_, DataKind::Bug) => "data-bug",
        (_, DataKind::Benchmark) => "data-benchmark",
    }
}

const JSON_HEIGHT: f32 = 260.;

/// The exact JSON, in mono, scrolling inside a well.
fn json_view(view: &(String, String), scroll: &Scrollbar) -> Div {
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
                    .font_family(theme::MONO)
                    .text_size(px(step::CAPTION.size))
                    .text_color(ui::white(ink::BODY))
                    .children(view.1.lines().map(|l| div().child(l.to_string()))),
            ),
        )
}

impl Launcher {
    /// Whether the question covers the window: there is a server to send to, and it is not
    /// answered for this consent version.
    pub(crate) fn asking_consent(&self) -> bool {
        self.collection.consent.needs_asking() && eclipse_telemetry::endpoint().is_some()
    }

    fn answer_consent(&mut self, choices: Choices) {
        match telemetry::answer(choices) {
            Ok(consent) => self.collection.consent = consent,
            Err(e) => {
                self.collection.note =
                    Some(trf("Could not save your choice: {error}", &[("error", &e)]))
            }
        }
        // The question and Settings say what the "I'm playing" ping holds, beside the switches.
        if !self.settings.ping_notice_seen {
            self.settings.ping_notice_seen = true;
            self.save_settings();
        }
        self.collection.view = None;
    }

    fn toggle_view(&mut self) {
        self.collection.view = match self.collection.view {
            Some(_) => None,
            None => Some(telemetry::preview()),
        };
    }

    /// The question, centred over the launcher. Everything starts off; "Send nothing" is one
    /// click, as easy as saying yes.
    pub(crate) fn consent_screen(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let draft = self.collection.draft;
        let rows = telemetry::SWITCHES.iter().map(|&(kind, name, what)| {
            let on = draft.allows(kind);
            ui::toggle_row(kind_id("ask", kind), tr(name), Some(tr(what).into()), on).on_press(
                cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.collection.draft.set(kind, !on);
                    cx.notify();
                }),
            )
        });
        let viewing = self.collection.view.is_some();
        // The body scrolls inside the card, so the answers stay at its foot however short the
        // window and however long what would be sent.
        let body = ui::scroll_pane("ask-body", &self.collection.body_scroll.handle)
            .flex()
            .flex_col()
            .min_h_0()
            .gap(px(space::MD))
            .child(ui::text(step::BODY, FontWeight::NORMAL, ink::BODY).child(tr(
                "Eclipse can send a few things to its developers. Each switch is off until you turn it on, and Settings can change it any time.",
            )))
            .child(div().flex().flex_col().mx(px(-ui::SETTING_ROW_PADDING)).children(rows))
            .child(ui::card_note(tr(telemetry::NEVER)))
            .child(
                div().flex().child(
                    ui::link(
                        "ask-view",
                        tr(if viewing {
                            "Hide what would be sent"
                        } else {
                            "View what would be sent"
                        }),
                    )
                    .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.toggle_view();
                        cx.notify();
                    })),
                ),
            )
            .children(
                self.collection
                    .view
                    .as_ref()
                    .map(|v| json_view(v, &self.collection.json_scroll)),
            );
        let answers = vec![
            ui::button(
                "ask-none",
                None,
                tr("Send nothing"),
                ButtonKind::Secondary,
                ui::ButtonSize::Md,
            )
            .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.answer_consent(Choices::NONE);
                cx.notify();
            }))
            .into_any_element(),
            ui::button(
                "ask-save",
                None,
                tr("Save choices"),
                ButtonKind::Primary,
                ui::ButtonSize::Md,
            )
            .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.answer_consent(this.collection.draft);
                cx.notify();
            }))
            .into_any_element(),
        ];
        let card = ui::dialog(
            "consent-card",
            Some(tr("Data collection").into()),
            tr("Help make Eclipse faster and steadier?"),
            ui::DialogWidth::Wide,
            self.collection.body_scroll.wrap(body),
            answers,
        );
        ui::modal(
            "consent",
            tr("Help make Eclipse faster and steadier?"),
            None,
        )
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .px(px(space::XXL))
        .py(px(space::XL))
        .child(card)
    }

    /// Settings' data collection: a switch per kind, then the player's data.
    pub(crate) fn collection_section(&mut self, cx: &mut Context<Self>) -> Div {
        let consent = self.collection.consent.clone();
        let mut list = div().flex().flex_col();
        for &(kind, name, what) in telemetry::SWITCHES.iter() {
            let on = !consent.needs_asking() && consent.choices.allows(kind);
            let row = ui::toggle_row(kind_id("data", kind), tr(name), Some(tr(what).into()), on)
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
                        .child(n),
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
