//! The data collection question, asked once (and again when what is collected changes), over the
//! launcher.
use crate::settings_data_collection::{json_view, kind_id, Asked};
use crate::ui::{ButtonKind, Press};
use crate::{ui, Launcher};
use eclipse_launcher::i18n::{tr, trf};
use eclipse_launcher::telemetry;
use eclipse_telemetry::Choices;
use eclipse_theme::glass::{ink, space, step};
use gpui::prelude::*;
use gpui::{div, px, ClickEvent, Div, FontWeight, Stateful};

impl Launcher {
    /// Whether the question covers the window: there is a server to send to, and it is not
    /// answered for this consent version.
    pub(super) fn asking_consent(&self) -> bool {
        self.collection.consent.needs_asking() && eclipse_telemetry::endpoint().is_some()
    }

    pub(super) fn answer_consent(&mut self, choices: Choices) {
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

    /// The question, centred over the launcher. Everything starts off; "Send nothing" is one
    /// click, as easy as saying yes.
    pub(super) fn consent_screen(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let draft = self.collection.draft;
        let rows = telemetry::SWITCHES.iter().map(|&(kind, name, what)| {
            let on = draft.allows(kind);
            ui::toggle_row(
                kind_id(Asked::Sheet, kind),
                tr(name),
                Some(tr(what).into()),
                on,
            )
            .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.collection.draft.set(kind, !on);
                cx.notify();
            }))
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
            ui::primary("ask-save", None, tr("Save choices"), ui::ButtonSize::Md)
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
}
