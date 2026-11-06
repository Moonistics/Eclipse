//! Settings' Account card: who plays now (a big head, the name, how), the offline names as chips
//! with a field to add one, and the Microsoft card beside them with its white Sign in with
//! Microsoft button, the accounts already signed in (Play as, Refresh, Sign out), the name the
//! game made up (a chip with New name), each offline name's skin (a pencil in its chip) and the
//! skin panel, the sign-in's code, progress and problems.

use super::*;
use crate::fields::Field;
use crate::ui::{self, white, Tone};
use account::Player;
use eclipse_theme::glass::{fill, ink, radius, step};

const HEAD: f32 = 80.;
const MICROSOFT_MIN: f32 = 280.;
const CHIP: f32 = 44.;
const CHIP_INSET: f32 = 5.;
const CHIP_HEAD: f32 = 30.;
const MICROSOFT_LOGO: f32 = 36.;

impl Launcher {
    pub(super) fn account_card(&mut self, cx: &mut Context<Self>) -> Div {
        let left = self
            .who_plays_card(cx)
            .min_w_0()
            .p(px(ui::SETTING_ROW_PADDING));
        let right = self
            .microsoft_card(cx)
            .border_color(white(eclipse_theme::glass::EDGE));
        let wide = self.window_width >= crate::layout::ACCOUNT_CARDS_BESIDE
            && (account::client_id().is_some() || !self.accounts.accounts.is_empty());
        div()
            .flex()
            .when(!wide, |d| d.flex_col())
            .child(left.when(wide, |d| d.flex_1()))
            .child(
                right
                    .when(wide, |d| d.flex_1().min_w(px(MICROSOFT_MIN)).border_l_1())
                    .when(!wide, |d| d.border_t_1()),
            )
    }

    /// The player now, the offline names and the field to add one.
    fn who_plays_card(&mut self, cx: &mut Context<Self>) -> Div {
        let now = self.accounts.player();
        let (face, name, status) = match &now {
            Player::Microsoft(uuid) => (
                Some(uuid.clone()),
                self.accounts
                    .accounts
                    .iter()
                    .find(|a| &a.uuid == uuid)
                    .map(|a| a.name.clone())
                    .unwrap_or_default(),
                (Tone::Live, tr("Microsoft account · online play")),
            ),
            Player::Offline(name) => (
                Some(profile_heads::offline_key(name)),
                name.clone(),
                (
                    Tone::Off,
                    tr("Offline account · can't join online-mode servers"),
                ),
            ),
            Player::GameName => {
                let name = self.offline_name.clone().unwrap_or_else(|| "Player".into());
                (
                    Some(profile_heads::offline_key(&name)),
                    name,
                    (
                        Tone::Off,
                        tr("Offline account · can't join online-mode servers"),
                    ),
                )
            }
        };
        let head = div()
            .flex_none()
            .size(px(HEAD))
            .rounded(px(radius::PANEL))
            .overflow_hidden()
            .child(self.profile_heads.draw(face.as_deref(), HEAD, self.scale));
        let header = div().flex().items_center().gap(px(20.)).child(head).child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .gap(px(10.))
                .child(
                    ui::entity_title(name, eclipse_theme::glass::pixel_text::CARD_TITLE).truncate(),
                )
                .child(
                    ui::text(step::BODY, FontWeight::NORMAL, ink::MUTED)
                        .child(ui::state_line(status.0, status.1)),
                ),
        );

        // The chips: each offline name, the game's own name, then the field.
        let mut chips = div().flex().flex_wrap().gap(px(8.)).mt(px(12.));
        for (i, offline) in self.accounts.offline.clone().into_iter().enumerate() {
            let plays = now == Player::Offline(offline.clone());
            let asked = format!("offline-name:{offline}");
            let forget_id = SharedString::from(format!("offline-forget-{i}"));
            if self.confirm_remove.as_deref() == Some(asked.as_str()) {
                let forget = offline.clone();
                chips = chips.child(
                    ui::confirm(
                        forget_id,
                        trf("Remove {name}?", &[("name", &offline)]),
                        tr("Remove"),
                        cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.confirm_remove = None;
                            cx.notify();
                        }),
                        cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.confirm_remove = None;
                            this.forget_offline(&forget);
                            cx.notify();
                        }),
                    )
                    .h(px(CHIP))
                    .px(px(ui::LIST_INSET))
                    .rounded(px(radius::ROW))
                    .bg(white(fill::HOVER)),
                );
                continue;
            }
            let (who, skin) = (offline.clone(), offline.clone());
            chips = chips.child(
                self.name_chip(
                    SharedString::from(format!("offline-chip-{i}")),
                    offline,
                    plays,
                )
                .when(!plays, |d| {
                    d.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.play_as(Player::Offline(who.clone()));
                        cx.notify();
                    }))
                })
                .child(
                    ui::icon_button(
                        format!("offline-skin-{i}"),
                        "pencil-edit-02",
                        tr("Change skin"),
                        ui::Size::Sm,
                        false,
                    )
                    .on_press(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            this.open_skin(skin.clone());
                            cx.notify();
                        },
                    )),
                )
                .when(!plays, |d| {
                    d.child(
                        ui::icon_button(forget_id, "cancel-01", tr("Remove…"), ui::Size::Sm, false)
                            .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                this.confirm_remove = Some(asked.clone());
                                cx.notify();
                            })),
                    )
                }),
            );
        }
        if let Some(made_up) = self.offline_name.clone() {
            let plays = now == Player::GameName;
            chips = chips.child(
                self.name_chip("game-name-chip", made_up, plays)
                    .when(!plays, |d| {
                        d.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.play_as(Player::GameName);
                            cx.notify();
                        }))
                    })
                    .child(
                        ui::icon_button(
                            "game-name-new",
                            "refresh",
                            tr("New name"),
                            ui::Size::Sm,
                            false,
                        )
                        .on_press(cx.listener(
                            |this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                match instances::new_player_name() {
                                    Ok(name) => this.offline_name = Some(name),
                                    Err(e) => this.sign_in_state = Some(SignInState::Done(Err(e))),
                                }
                                cx.notify();
                            },
                        )),
                    ),
            );
        }
        let add = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(self.field(Field::OfflineName, tr("Add a name"), 150., cx))
            .child(
                ui::button(
                    "offline-add",
                    None,
                    tr("Add"),
                    ButtonKind::Secondary,
                    ui::Size::Lg,
                )
                .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.offline_name_enter();
                    cx.notify();
                })),
            );
        let can_add = self.settings.dev_offline || self.owner != Some(false);
        div()
            .flex()
            .flex_col()
            .child(header)
            .child(ui::label(tr("Offline names")).mt(px(32.)))
            .child(chips.when(can_add, |d| d.child(add)))
            .when(!can_add, |d| {
                d.child(ui::card_note(tr(
                    "Sign in once with a Microsoft account that owns Minecraft: Java Edition to use offline names.",
                )))
            })
            .children(self.skin_panel(cx).map(|p| p.mt(px(16.))))
    }

    /// An offline name's chip: its head and name, a tick and a stronger fill on the one playing.
    fn name_chip(&self, id: impl Into<SharedString>, name: String, plays: bool) -> Stateful<Div> {
        let face = profile_heads::offline_key(&name);
        ui::selected(
            ui::hoverable(id, radius::ROW, ui::Hover::Row)
                .flex()
                .items_center()
                .gap(px(10.))
                .h(px(CHIP))
                .px(px(CHIP_INSET))
                .when(!plays, |d| d.bg(white(fill::HOVER)).cursor_pointer()),
            plays,
        )
        .child(
            div()
                .relative()
                .flex_none()
                .size(px(CHIP_HEAD))
                .rounded(px(radius::ROW))
                .overflow_hidden()
                .child(self.profile_heads.draw(Some(&face), CHIP_HEAD, self.scale)),
        )
        .child(
            ui::text(step::BODY, FontWeight::MEDIUM, ink::FULL)
                .relative()
                .child(name),
        )
        .when(plays, |d| d.child(ui::tick()))
    }

    /// The Microsoft card: the accounts signed in here, then adding one; one muted line when this
    /// copy of the launcher can't sign in.
    fn microsoft_card(&mut self, cx: &mut Context<Self>) -> Div {
        let has_accounts = !self.accounts.accounts.is_empty();
        let rows = has_accounts.then(|| self.microsoft_rows(cx).0);
        let configured = account::client_id().is_some();
        if !configured {
            return div().flex().flex_col().children(rows).child(
                ui::text(step::NOTE, FontWeight::NORMAL, ink::MUTED)
                    .p(px(ui::SETTING_ROW_PADDING))
                    .child(tr(account::NOT_CONFIGURED)),
            );
        }
        let (title, detail, action) = self.sign_in_parts(cx);
        let idle = self.sign_in_state.is_none()
            || matches!(self.sign_in_state, Some(SignInState::Done(_)));
        let first = !has_accounts && self.sign_in_target.is_none();
        let heading = if first {
            tr("Play online with your Microsoft account").to_string()
        } else {
            title
        };
        // Adding an account is Microsoft's white button; signing one in again, the code and the
        // waiting keep their own buttons.
        let button: Option<AnyElement> = if idle && self.sign_in_target.is_none() {
            Some(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .mt(px(24.))
                    .child(
                        microsoft::white_button("microsoft-card-sign-in")
                            .w_full()
                            .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.start_sign_in();
                                cx.notify();
                            })),
                    )
                    .child(
                        ui::text(step::CAPTION, FontWeight::NORMAL, ink::DIM)
                            .child(tr("Microsoft's own page opens in your browser.")),
                    )
                    .into_any_element(),
            )
        } else {
            action.map(|a| div().mt(px(20.)).child(a).into_any_element())
        };
        let pad = ui::SETTING_ROW_PADDING;
        div()
            .flex()
            .flex_col()
            .p(px(pad))
            .children(rows.map(|r| r.mx(px(-pad)).mt(px(-pad)).mb(px(pad))))
            .child(microsoft::logo(MICROSOFT_LOGO))
            .child(ui::card_title(heading).mt(px(20.)))
            .child(
                ui::text(step::BODY, FontWeight::NORMAL, ink::MUTED)
                    .mt(px(8.))
                    .child(detail),
            )
            .children(button)
    }
}

impl Launcher {
    /// The Microsoft accounts signed in here: each one's head, name and state, Play as,
    /// Refresh and Sign out.
    pub(super) fn microsoft_rows(&mut self, cx: &mut Context<Self>) -> (Div, usize) {
        let mut rows = div().flex().flex_col();
        let mut n = 0;
        let selected = self.accounts.selected.clone();
        for (i, a) in self.accounts.accounts.clone().into_iter().enumerate() {
            let plays = selected.as_ref() == Some(&a.uuid);
            let uuid = a.uuid.clone();
            let mut actions = div().flex().gap(px(4.));
            if plays {
                actions = actions.child(
                    button(format!("offline-{i}"), "Play offline", ButtonKind::Ghost).on_press(
                        cx.listener(|this, _: &ClickEvent, _, cx| {
                            let mut accounts = Accounts::load();
                            accounts.selected = None;
                            if accounts.save().is_ok() {
                                this.accounts = accounts;
                            }
                            cx.notify();
                        }),
                    ),
                );
            } else {
                let uuid = uuid.clone();
                actions = actions.child(
                    button(format!("select-{i}"), "Play as", ButtonKind::Secondary).on_press(
                        cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.play_as(account::Player::Microsoft(uuid.clone()));
                            cx.notify();
                        }),
                    ),
                );
            }
            let sign_in_progress = self
                .sign_in_target
                .as_ref()
                .filter(|a| a.uuid == uuid)
                .and_then(|_| match self.sign_in_state.as_ref() {
                    Some(SignInState::Starting) => Some("Asking Microsoft…"),
                    Some(SignInState::Code(_)) => Some("Waiting for browser sign-in…"),
                    Some(SignInState::Working(phase)) => Some(phase.label()),
                    _ => None,
                });
            let progress = sign_in_progress
                .or_else(|| self.auth_progress.get(&uuid).map(|phase| phase.label()));
            let busy = self.auth_busy.contains_key(&uuid) || sign_in_progress.is_some();
            let status = self.auth_status.get(&uuid);
            let needs_sign_in =
                status.is_some_and(|s| s.state == account::session::State::SignInRequired);
            let refresh_id = uuid.clone();
            let refresh = button(
                format!("refresh-{i}"),
                if busy {
                    "Checking…"
                } else if needs_sign_in {
                    "Sign in again"
                } else {
                    "Refresh"
                },
                ButtonKind::Ghost,
            );
            actions = actions.child(if busy {
                disabled(refresh)
            } else {
                refresh.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if needs_sign_in {
                        this.re_sign_in(&refresh_id);
                    } else {
                        this.refresh_account(refresh_id.clone());
                    }
                    cx.notify();
                }))
            });
            let sign_out = button(format!("sign-out-{i}"), "Sign out", ButtonKind::Ghost);
            actions = actions.child(if busy {
                disabled(sign_out)
            } else {
                sign_out.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    let Some(generation) = this.begin_auth_job(&uuid) else {
                        return;
                    };
                    let (tx, uuid) = (this.tx.clone(), uuid.clone());
                    std::thread::spawn(move || {
                        let mut accounts = Accounts::load();
                        let _ = tx.send(Msg::AccountProgress(
                            uuid.clone(),
                            generation,
                            account::session::Phase::SigningOut,
                        ));
                        let result = accounts
                            .remove(account::store(), &uuid)
                            .and_then(|()| accounts.save())
                            .map(|()| accounts.has_owner(account::store()));
                        let _ = tx.send(Msg::AccountRemoved(uuid, generation, result));
                    });
                    cx.notify();
                }))
            });
            n += 1;
            rows = rows.child(account_row(
                self.profile_heads.draw(Some(&a.uuid), 32., self.scale),
                a.name.clone(),
                Some(
                    div()
                        .child(tr(if plays {
                            "Microsoft account · in use"
                        } else {
                            "Microsoft account"
                        }))
                        .child(account_status_line(status, progress))
                        .when(!busy, |row| {
                            row.when_some(status.filter(|s| s.message != s.label()), |row, s| {
                                row.child(div().child(tr(&s.message).to_string()))
                            })
                        })
                        .into_any_element(),
                ),
                actions,
            ));
        }
        (rows, n)
    }

    /// Adding a Microsoft account, or signing one in again: what the row is called, what is
    /// happening (the code to enter, a problem), and its buttons.
    pub(super) fn sign_in_parts(
        &self,
        cx: &mut Context<Self>,
    ) -> (String, Div, Option<AnyElement>) {
        let t = self.theme;
        let configured = account::client_id().is_some();
        let busy = matches!(
            self.sign_in_state,
            Some(SignInState::Starting | SignInState::Code(_) | SignInState::Working(_))
        );
        let detail = match &self.sign_in_state {
            _ if !configured => div().child(tr(account::NOT_CONFIGURED)),
            Some(SignInState::Starting) => div().child(tr("Asking Microsoft…")),
            Some(SignInState::Working(phase)) => div().child(tr(phase.label())),
            Some(SignInState::Code(code)) => div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(trf(
                    "Open {link} in your browser and enter this code:",
                    &[("link", &code.verification_uri)],
                ))
                .child(
                    ui::text(
                        eclipse_theme::glass::step::FIGURE,
                        FontWeight::MEDIUM,
                        eclipse_theme::glass::ink::FULL,
                    )
                    .font_family(theme::MONO)
                    .child(code.user_code.clone()),
                ),
            Some(SignInState::Done(Ok(a))) => {
                div().child(trf("Signed in as {name}.", &[("name", &a.name)]))
            }
            Some(SignInState::Done(Err(e))) => div().text_color(ui::danger()).child(e.clone()),
            None if self.accounts.accounts.is_empty() => div().child(tr(
                "Sign in with the Microsoft account that owns Minecraft: Java Edition.",
            )),
            None => div(),
        };
        let detail = if let Some(target) = &self.sign_in_target {
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(trf(
                    "Use the Microsoft account for {name}.",
                    &[("name", &target.name)],
                ))
                .child(detail)
        } else {
            detail
        };
        let detail = match self.keychain.clone() {
            Some(said) if !self.accounts.accounts.is_empty() => div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(detail)
                .child(div().child(said)),
            _ => detail,
        };
        let action = if !configured {
            None
        } else if let Some(SignInState::Code(code)) = &self.sign_in_state {
            let (uri, user_code) = (code.verification_uri.clone(), code.user_code.clone());
            Some(
                div()
                    .flex()
                    .gap(px(4.))
                    .child(
                        button("sign-in-copy", "Copy code", ButtonKind::Secondary).on_press(
                            move |_, _, cx| {
                                clipboard::copy_to_clipboard(
                                    cx,
                                    gpui::ClipboardItem::new_string(user_code.clone()),
                                )
                            },
                        ),
                    )
                    .child(
                        button("sign-in-open", "Open link", ButtonKind::Secondary)
                            .on_press(move |_, _, cx| cx.open_url(&uri)),
                    )
                    .child(
                        button("sign-in-cancel", "Cancel", ButtonKind::Ghost).on_press(
                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.cancel_sign_in();
                                cx.notify();
                            }),
                        ),
                    )
                    .into_any_element(),
            )
        } else if busy {
            Some(
                button("sign-in-cancel-working", "Cancel", ButtonKind::Ghost)
                    .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.cancel_sign_in();
                        cx.notify();
                    }))
                    .into_any_element(),
            )
        } else {
            let target = self.sign_in_target.as_ref().map(|a| a.uuid.clone());
            let targeted = target.is_some();
            // Re-signing one account names it; adding one is Microsoft's own button.
            let sign_in = if targeted {
                button("sign-in", "Sign in again", ButtonKind::Secondary)
            } else {
                microsoft::sign_in_button("sign-in", &t)
            };
            let mut actions = div().flex().gap(px(4.)).child(sign_in.on_press(cx.listener(
                move |this, _: &ClickEvent, _, cx| {
                    if let Some(uuid) = &target {
                        this.re_sign_in(uuid);
                    } else {
                        this.start_sign_in();
                    }
                    cx.notify();
                },
            )));
            if targeted {
                actions = actions.child(
                    button("sign-in-cancel-target", "Cancel", ButtonKind::Ghost).on_press(
                        cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.cancel_sign_in();
                            cx.notify();
                        }),
                    ),
                );
            }
            Some(actions.into_any_element())
        };
        let title = match &self.sign_in_target {
            Some(target) => trf("Sign in again as {name}", &[("name", &target.name)]),
            None if self.accounts.accounts.is_empty() => tr("Microsoft account").to_owned(),
            None => tr("Another account").to_owned(),
        };
        (title, detail, action)
    }
}

/// Account state uses the same semantic colors as the launcher's other notices. Keep a
/// distinct icon and the full label so color is never the only way to recognize a state.
pub(super) fn account_status_line(
    status: Option<&account::session::Status>,
    progress: Option<&str>,
) -> Div {
    use account::session::State;
    use eclipse_theme::glass::ink;
    let muted = ui::white(ink::MUTED);
    let (icon, color, label) = if let Some(progress) = progress {
        ("clock", muted, progress)
    } else {
        let (icon, color) = match status.map(|s| s.state) {
            Some(State::Ready) => ("tick-02", ui::live()),
            Some(State::NeedsRefresh | State::Retry) => ("refresh", ui::warn()),
            Some(State::SignInRequired) => ("user", ui::danger()),
            Some(State::OwnershipRequired) => ("alert-circle", ui::danger()),
            None => ("information-circle", muted),
        };
        (
            icon,
            color,
            status.map_or("Sign-in not checked", |s| s.label()),
        )
    };
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .text_color(color)
        .child(ui::icon(icon, ui::IconSize::Px16, color))
        .child(tr(label).to_owned())
}

/// Microsoft account actions go below the profile in the fixed-width card.
pub(super) fn account_row(
    slot: Div,
    name: impl IntoElement,
    detail: Option<AnyElement>,
    actions: Div,
) -> Div {
    div()
        .flex()
        .flex_col()
        .min_w_0()
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(14.))
                .px(px(16.))
                .py(px(12.))
                .child(
                    slot.rounded(px(eclipse_theme::glass::radius::ROW))
                        .overflow_hidden(),
                )
                .child(ui::setting_row(name, detail, None).p_0().flex_1()),
        )
        .child(actions.flex_wrap().px(px(16.)).pb(px(12.)))
}
