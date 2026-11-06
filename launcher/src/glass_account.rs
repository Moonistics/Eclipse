//! Settings' Account card: who plays now (a big head, the name, how), the offline names as chips
//! with a field to add one, and the Microsoft card beside them with its white Sign in with
//! Microsoft button, the accounts already signed in (Play as, Refresh, Sign out), the name the
//! game made up (a chip with New name), each offline name's skin (a pencil in its chip) and the
//! skin panel, the sign-in's code, progress and problems.

use super::*;
use crate::glass_ui::{self as ui, white, Tone};
use crate::servers_page::Field;
use account::Player;
use eclipse_theme::glass::{fill, ink, radius, step};

const HEAD: f32 = 80.;
const MICROSOFT_MIN: f32 = 280.;
const CHIP: f32 = 44.;
const CHIP_INSET: f32 = 5.;
const CHIP_HEAD: f32 = 30.;
const MICROSOFT_LOGO: f32 = 36.;

impl Launcher {
    pub(super) fn glass_account(&mut self, cx: &mut Context<Self>) -> Div {
        let left = self
            .glass_who_plays(cx)
            .min_w_0()
            .p(px(ui::SETTING_ROW_PADDING));
        let right = self
            .glass_microsoft_card(cx)
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
    fn glass_who_plays(&mut self, cx: &mut Context<Self>) -> Div {
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
                    glass_home_lists::confirm(
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
                    Kind::Secondary,
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
    fn glass_microsoft_card(&mut self, cx: &mut Context<Self>) -> Div {
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
        let button: Option<AnyElement> =
            if idle && self.sign_in_target.is_none() {
                Some(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(12.))
                        .mt(px(24.))
                        .child(microsoft::white_button("glass-sign-in").w_full().on_press(
                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.start_sign_in();
                                cx.notify();
                            }),
                        ))
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
