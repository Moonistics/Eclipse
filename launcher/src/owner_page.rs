//! The owner gate in the window (`eclipse_launcher::account::launch`): the screen that asks for a
//! Microsoft account that owns Minecraft: Java Edition. It covers the launcher once for players
//! without one (those who played offline before the gate), and again whenever Play is refused.
//! The sign-in runs on it, code and all.
use super::*;

impl Launcher {
    /// Whether the screen covers the window: no owner, and either Play was refused or the player
    /// hasn't read it yet. The data question comes first.
    pub(crate) fn asking_owner(&self) -> bool {
        (self.owner_shown() == Some(false) || preview::shows(preview::Preview::SignIn))
            && (self.owner_screen_open
                || !self.settings.owner_notice_seen
                || preview::shows(preview::Preview::SignIn))
            && !self.asking_consent()
    }

    /// `owner` as the window shows it: the developer option that lets offline names play without
    /// an owner (`dev_options`) hides the gate, as if the keychain hadn't answered yet.
    pub(crate) fn owner_shown(&self) -> Option<bool> {
        match self.owner {
            Some(false) if self.settings.dev_offline => None,
            owner => owner,
        }
    }

    /// Opens the screen and starts the sign-in on it.
    pub(crate) fn sign_in_to_play(&mut self) {
        self.owner_screen_open = true;
        self.start_sign_in();
    }

    /// Closes it; it comes back with the next refused Play.
    fn close_owner_screen(&mut self) {
        self.cancel_sign_in();
        self.owner_screen_open = false;
        if self.game.problem == Some(game_run::GameProblem::NeedsOwner) {
            self.game.problem = None;
        }
        if !self.settings.owner_notice_seen {
            self.settings.owner_notice_seen = true;
            self.save_settings();
        }
    }

    pub(crate) fn owner_screen(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let configured = account::client_id().is_some();
        // The screenshot preview shows a code without asking Microsoft for one.
        let state = match &self.accounts_ui.sign_in_state {
            _ if preview::shows(preview::Preview::SignIn) => {
                Shown::Code("https://www.microsoft.com/link", "K7Q-4MXR")
            }
            _ if !configured => Shown::Unavailable,
            Some(SignInState::Starting) => Shown::Starting(tr("Asking Microsoft…")),
            Some(SignInState::Working(phase)) => Shown::Starting(tr(phase.label())),
            Some(SignInState::Code(code)) => Shown::Code(&code.verification_uri, &code.user_code),
            Some(SignInState::Done(Err(e))) => Shown::Error(e),
            Some(SignInState::Done(Ok(_))) | None => Shown::Idle,
        };
        self.owner_dialog(state, cx)
    }

    /// A dialog plate over the dimmed launcher, its Sign in the Account card's white Microsoft
    /// button, its code in the card's inner panel.
    fn owner_dialog(&self, state: Shown, cx: &mut Context<Self>) -> Stateful<Div> {
        use crate::ui::{self};
        use eclipse_theme::glass::{ink, space};
        let panel = || ui::well().mt(px(space::XL));
        let (title, ask) = match state {
            Shown::Unavailable => (tr("Microsoft sign-in needed"), Self::cant_sign_in_note()),
            _ => (
                tr("Sign in to play"),
                tr("Sign in with the Microsoft account that owns Minecraft: Java Edition."),
            ),
        };
        let mut card = div().flex().flex_col().child(
            ui::text(
                eclipse_theme::glass::step::BODY,
                FontWeight::NORMAL,
                ink::BODY,
            )
            .child(ask),
        );
        card = match state {
            Shown::Code(uri, user_code) => {
                let copy = user_code.to_string();
                card.child(
                    panel()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(space::LG))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(space::XS))
                                .child(ui::card_note(trf(
                                    "Open {link} in your browser and enter this code:",
                                    &[("link", short_link(uri))],
                                )))
                                .child(ui::sign_in_code(user_code.to_string())),
                        )
                        .child(ui::copy_button(
                            "owner-copy",
                            tr("Copy code"),
                            copy.clone(),
                            ui::ButtonSize::Md,
                        )),
                )
            }
            Shown::Starting(what) => card.child(
                panel().child(
                    ui::text(
                        eclipse_theme::glass::step::BODY,
                        FontWeight::NORMAL,
                        ink::BODY,
                    )
                    .child(what.to_string()),
                ),
            ),
            Shown::Error(e) => card.child(panel().child(ui::error_note(e.to_string()))),
            Shown::Idle | Shown::Unavailable => card,
        };
        let main: Option<AnyElement> = match state {
            Shown::Code(uri, _) => {
                let uri = uri.to_string();
                Some(
                    ui::primary("owner-open", None, tr("Open link"), ui::DIALOG_BUTTON)
                        .on_press(move |_, _, cx| cx.open_url(&uri))
                        .into_any_element(),
                )
            }
            Shown::Starting(_) | Shown::Unavailable => None,
            _ => Some(
                microsoft::white_button("owner-sign-in")
                    .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.sign_in_to_play();
                        cx.notify();
                    }))
                    .into_any_element(),
            ),
        };
        let close = match state {
            Shown::Unavailable => tr("Close"),
            _ => tr("Not now"),
        };
        let later = ui::dialog_dismiss("owner-later", close).on_press(cx.listener(
            |this, _: &ClickEvent, _, cx| {
                this.close_owner_screen();
                cx.notify();
            },
        ));
        if matches!(state, Shown::Code(..)) {
            card = card.child(ui::card_note(tr("Waiting for Microsoft…")).mt(px(space::MD)));
        }
        let foot = vec![ui::dialog_foot(later, main).into_any_element()];
        ui::modal("owner-gate", title, None)
            .flex()
            .items_center()
            .justify_center()
            .p(px(space::XL))
            .child(ui::dialog(
                "owner-card",
                Some(tr("Microsoft account").into()),
                title,
                ui::DialogWidth::Narrow,
                card,
                foot,
            ))
    }
}

/// What the sheet shows of the sign-in.
#[derive(Clone, Copy)]
enum Shown<'a> {
    Idle,
    Unavailable,
    /// What the sign-in is doing.
    Starting(&'a str),
    /// The page and the code to enter there.
    Code(&'a str, &'a str),
    Error(&'a str),
}

/// `https://www.microsoft.com/link` as `microsoft.com/link`.
fn short_link(uri: &str) -> &str {
    let uri = uri
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    uri.strip_prefix("www.").unwrap_or(uri)
}

#[cfg(test)]
mod tests {
    #[test]
    fn links_are_shown_as_typed() {
        assert_eq!(
            super::short_link("https://www.microsoft.com/link"),
            "microsoft.com/link"
        );
        assert_eq!(
            super::short_link("https://login.live.com/oauth20_remoteconnect.srf"),
            "login.live.com/oauth20_remoteconnect.srf"
        );
    }
}
