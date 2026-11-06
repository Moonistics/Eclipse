//! The owner gate in the window (`eclipse_launcher::account::launch`): the screen that asks for a
//! Microsoft account that owns Minecraft: Java Edition. It covers the launcher once for players
//! without one (those who played offline before the gate), and again whenever Play is refused.
//! The sign-in runs on it, code and all.
use super::*;

impl Launcher {
    /// Whether the screen covers the window: no owner, and either Play was refused or the player
    /// hasn't read it yet. The data question comes first.
    pub(crate) fn asking_owner(&self) -> bool {
        (self.owner_shown() == Some(false) || sign_in_preview())
            && (self.owner_screen_open || !self.settings.owner_notice_seen || sign_in_preview())
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
        if !self.settings.owner_notice_seen {
            self.settings.owner_notice_seen = true;
            self.save_settings();
        }
    }

    pub(crate) fn owner_screen(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let configured = account::client_id().is_some();
        // The screenshot preview shows a code without asking Microsoft for one.
        let state = match &self.sign_in_state {
            _ if sign_in_preview() => Shown::Code("https://www.microsoft.com/link", "K7Q-4MXR"),
            _ if !configured => Shown::Error(tr(account::NOT_CONFIGURED)),
            Some(SignInState::Starting) => Shown::Starting("Asking Microsoft…"),
            Some(SignInState::Working(phase)) => Shown::Starting(phase.label()),
            Some(SignInState::Code(code)) => Shown::Code(&code.verification_uri, &code.user_code),
            Some(SignInState::Done(Err(e))) => Shown::Error(e),
            Some(SignInState::Done(Ok(_))) | None => Shown::Idle,
        };
        self.owner_dialog(state, configured, cx)
    }

    /// A dialog plate over the dimmed launcher, its Sign in the Account card's white Microsoft
    /// button, its code in the card's inner panel.
    fn owner_dialog(
        &self,
        state: Shown,
        configured: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        use crate::ui::{self, white};
        use eclipse_theme::glass::{ink, radius, space};
        let panel = || {
            div()
                .mt(px(space::XL))
                .p(px(space::XL))
                .rounded(px(radius::POPOVER))
                .bg(ui::inset())
        };
        let mut card = ui::dialog_plate(radius::MODAL)
            .id("owner-card")
            .occlude()
            .flex()
            .flex_col()
            // A set width: text measured at it wraps where it is drawn.
            .w(px(520.))
            .max_w_full()
            .p(px(space::XXL))
            .child(ui::label(tr("Microsoft account")))
            .child(
                ui::entity_title(
                    tr("Sign in to play"),
                    eclipse_theme::glass::pixel_text::CARD_TITLE,
                )
                .mt(px(space::SM)),
            )
            .child(
                div()
                    .mt(px(space::MD))
                    .flex()
                    .flex_col()
                    .gap(px(space::SM))
                    .child(
                        ui::text(
                            eclipse_theme::glass::step::BODY,
                            FontWeight::NORMAL,
                            ink::BODY,
                        )
                        .child(tr(
                            "Sign in with the Microsoft account that owns Minecraft: Java Edition.",
                        )),
                    ),
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
                                .child(
                                    ui::text(
                                        eclipse_theme::glass::step::NOTE,
                                        FontWeight::NORMAL,
                                        ink::MUTED,
                                    )
                                    .child(trf(
                                        "Open {link} in your browser and enter this code:",
                                        &[("link", short_link(uri))],
                                    )),
                                )
                                .child(
                                    div()
                                        .font_family(theme::MONO)
                                        .font_weight(FontWeight::BOLD)
                                        .text_size(px(30.))
                                        .line_height(px(38.))
                                        .text_color(white(ink::FULL))
                                        .child(user_code.to_string()),
                                ),
                        )
                        .child(
                            ui::button(
                                "owner-copy",
                                None,
                                tr("Copy code"),
                                ButtonKind::Secondary,
                                ui::ButtonSize::Md,
                            )
                            .on_press(move |_, _, cx| {
                                clipboard::copy_to_clipboard(
                                    cx,
                                    gpui::ClipboardItem::new_string(copy.clone()),
                                )
                            }),
                        ),
                )
            }
            Shown::Starting(what) => card.child(
                panel().child(
                    ui::text(
                        eclipse_theme::glass::step::BODY,
                        FontWeight::NORMAL,
                        ink::BODY,
                    )
                    .child(tr(what)),
                ),
            ),
            Shown::Error(e) => card.child(panel().child(ui::error_note(e.to_string()))),
            Shown::Idle => card,
        };
        let main: Option<AnyElement> = match state {
            Shown::Code(uri, _) => {
                let uri = uri.to_string();
                Some(
                    ui::button(
                        "owner-open",
                        None,
                        tr("Open link"),
                        ButtonKind::Primary,
                        ui::ButtonSize::Lg,
                    )
                    .px(px(space::XL))
                    .on_press(move |_, _, cx| cx.open_url(&uri))
                    .into_any_element(),
                )
            }
            Shown::Starting(_) => None,
            Shown::Error(_) if !configured => None,
            _ => Some(
                microsoft::white_button("owner-sign-in")
                    .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.sign_in_to_play();
                        cx.notify();
                    }))
                    .into_any_element(),
            ),
        };
        let later = ui::button(
            "owner-later",
            None,
            tr("Not now"),
            ButtonKind::Ghost,
            ui::ButtonSize::Lg,
        )
        .px(px(space::XL))
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.close_owner_screen();
            cx.notify();
        }));
        card = card.child(
            div()
                .flex()
                .items_center()
                .gap(px(space::SM))
                .mt(px(space::XL))
                .children(main)
                .child(later),
        );
        if matches!(state, Shown::Code(..)) {
            card = card.child(
                ui::text(
                    eclipse_theme::glass::step::NOTE,
                    FontWeight::NORMAL,
                    ink::MUTED,
                )
                .mt(px(space::MD))
                .child(tr("Waiting for Microsoft…")),
            );
        }
        ui::modal("owner-gate", tr("Sign in to play"), None)
            .flex()
            .items_center()
            .justify_center()
            .p(px(space::XL))
            .child(card)
    }
}

/// What the sheet shows of the sign-in.
#[derive(Clone, Copy)]
enum Shown<'a> {
    Idle,
    /// What the sign-in is doing.
    Starting(&'static str),
    /// The page and the code to enter there.
    Code(&'a str, &'a str),
    Error(&'a str),
}

/// `ECLIPSE_LAUNCHER_PAGE=sign-in` opens the sheet with a made-up code, for screenshots: nothing
/// asks Microsoft, and the gate itself is as it was (no account, no Play).
pub(super) fn sign_in_preview() -> bool {
    static PREVIEW: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *PREVIEW.get_or_init(|| std::env::var("ECLIPSE_LAUNCHER_PAGE").as_deref() == Ok("sign-in"))
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
