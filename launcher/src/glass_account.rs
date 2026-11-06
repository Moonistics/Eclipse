//! Settings' Account card: who plays now (a big head, the name, how), the offline names as chips
//! with a field to add one, and the Microsoft card beside them with its white Sign in with
//! Microsoft button, the accounts already signed in (Play as, Refresh, Sign out), the name the
//! game made up (a chip with New name), each offline name's skin (a pencil in its chip) and the
//! skin panel, the sign-in's code, progress and problems.

use super::*;
use crate::glass_ui::{self as ui, white, Tone};
use crate::servers_page::Field;
use account::Player;
use eclipse_theme::glass::{fill, ink, radius};

const SIDE_BY_SIDE: f32 = 1500.;

impl Launcher {
    pub(super) fn glass_account(&mut self, cx: &mut Context<Self>) -> Div {
        let left = self.glass_who_plays(cx).min_w_0().p(px(16.));
        let right = self
            .glass_microsoft_card(cx)
            .border_color(white(eclipse_theme::glass::EDGE));
        let wide = self.window_width >= SIDE_BY_SIDE;
        div()
            .flex()
            .when(!wide, |d| d.flex_col())
            .child(left.when(wide, |d| d.flex_1()))
            .child(
                right
                    .when(wide, |d| d.flex_1().min_w(px(280.)).border_l_1())
                    .when(!wide, |d| d.border_t_1()),
            )
    }

    /// The player now, the offline names and the field to add one.
    fn glass_who_plays(&mut self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let now = self.accounts.player();
        let (uuid, name, status) = match &now {
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
                None,
                name.clone(),
                (Tone::Off, tr("Offline · no online servers")),
            ),
            Player::GameName => (
                None,
                self.offline_name.clone().unwrap_or_else(|| "Player".into()),
                (Tone::Off, tr("Offline · no online servers")),
            ),
        };
        let head = div()
            .flex_none()
            .size(px(80.))
            .rounded(px(radius::PANEL))
            .overflow_hidden()
            .child(
                self.profile_heads
                    .draw(uuid.as_deref(), 80., self.scale, &t),
            );
        let header = div().flex().items_center().gap(px(20.)).child(head).child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .gap(px(10.))
                .child(ui::entity_title(name, 24.).truncate())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(ui::dot(status.0))
                        .child(
                            ui::text(14., FontWeight::NORMAL, ink::MUTED)
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child(status.1),
                        ),
                ),
        );

        // The chips: each offline name, the game's own name, then the field.
        let mut chips = div().flex().flex_wrap().gap(px(8.)).mt(px(12.));
        for (i, offline) in self.accounts.offline.clone().into_iter().enumerate() {
            let plays = now == Player::Offline(offline.clone());
            let (who, skin, forget) = (offline.clone(), offline.clone(), offline.clone());
            chips = chips.child(
                self.name_chip(
                    SharedString::from(format!("offline-chip-{i}")),
                    offline,
                    plays,
                )
                .when(!plays, |d| {
                    d.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.play_as(Player::Offline(who.clone()));
                        cx.notify();
                    }))
                })
                .child(self.chip_icon(
                    SharedString::from(format!("offline-skin-{i}")),
                    "pencil-edit-02",
                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        this.open_skin(skin.clone());
                        cx.notify();
                    }),
                ))
                .when(!plays, |d| {
                    d.child(self.chip_icon(
                        SharedString::from(format!("offline-forget-{i}")),
                        "cancel-01",
                        cx.listener(move |this, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            this.forget_offline(&forget);
                            cx.notify();
                        }),
                    ))
                }),
            );
        }
        if let Some(made_up) = self.offline_name.clone() {
            let plays = now == Player::GameName;
            chips = chips.child(
                self.name_chip("game-name-chip", made_up, plays)
                    .when(!plays, |d| {
                        d.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.play_as(Player::GameName);
                            cx.notify();
                        }))
                    })
                    .child(self.chip_icon(
                        "game-name-new".into(),
                        "refresh",
                        cx.listener(|this, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            match instances::new_player_name() {
                                Ok(name) => this.offline_name = Some(name),
                                Err(e) => this.sign_in_state = Some(SignInState::Done(Err(e))),
                            }
                            cx.notify();
                        }),
                    )),
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
                    &t,
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.offline_name_enter();
                    cx.notify();
                })),
            );
        chips = chips.child(add);
        div()
            .flex()
            .flex_col()
            .child(header)
            .child(ui::card_heading(tr("Offline names")).mt(px(32.)))
            .child(chips)
            .children(self.skin_panel(cx).map(|p| p.mt(px(16.))))
    }

    /// An offline name's chip: its head and name, a tick and a stronger fill on the one playing.
    fn name_chip(&self, id: impl Into<SharedString>, name: String, plays: bool) -> Stateful<Div> {
        let t = self.theme;
        div()
            .id(id.into())
            .flex()
            .items_center()
            .gap(px(10.))
            .h(px(44.))
            .pl(px(6.))
            .pr(px(6.))
            .rounded(px(radius::ROW))
            .when(plays, |d| d.bg(white(fill::ACTIVE)))
            .when(!plays, |d| {
                d.bg(white(fill::HOVER))
                    .cursor_pointer()
                    .hover(|s| s.bg(white(fill::BUTTON)))
            })
            .child(
                div()
                    .flex_none()
                    .size(px(30.))
                    .rounded(px(radius::ROW))
                    .overflow_hidden()
                    .child(self.profile_heads.draw(None, 30., self.scale, &t)),
            )
            .child(ui::text(14., FontWeight::MEDIUM, ink::FULL).child(name))
            .when(plays, |d| {
                d.child(ui::icon("tick-02", 16., ui::color(ui::look().accent_soft)))
            })
    }

    /// A small icon button in a chip.
    fn chip_icon(
        &self,
        id: SharedString,
        icon: &'static str,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Stateful<Div> {
        keyboard(
            div()
                .id(id)
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(28.))
                .rounded(px(radius::ROW))
                .cursor_pointer()
                .hover(|s| s.bg(white(fill::BUTTON)))
                .child(ui::icon(icon, 14., white(ink::DIM))),
            &self.theme,
        )
        .on_click(on_click)
    }

    /// The Microsoft card: the accounts signed in here, then adding one.
    fn glass_microsoft_card(&mut self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let has_accounts = !self.accounts.accounts.is_empty();
        let (rows, _) = if has_accounts {
            ui::flat_rows(|| self.microsoft_rows(cx))
        } else {
            (div(), 0)
        };
        let (title, detail, action) = self.sign_in_parts(cx);
        let configured = account::client_id().is_some();
        let idle = self.sign_in_state.is_none()
            || matches!(self.sign_in_state, Some(SignInState::Done(_)));
        let first = !has_accounts && self.sign_in_target.is_none();
        let heading = if first {
            tr("Play online with your Microsoft account").to_string()
        } else {
            title
        };
        let note: AnyElement = if first && configured && self.sign_in_state.is_none() {
            div()
                .child(tr(
                    "Sign in with the Microsoft account that owns Minecraft: Java Edition.",
                ))
                .into_any_element()
        } else {
            detail.into_any_element()
        };
        // Adding an account is Microsoft's white button; signing one in again, the code and the
        // waiting keep their own buttons.
        let button: Option<AnyElement> = if configured && idle && self.sign_in_target.is_none() {
            Some(
                keyboard(
                    div()
                        .id("glass-sign-in")
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap(px(10.))
                        .h(px(48.))
                        .w_full()
                        .mt(px(24.))
                        .rounded(px(radius::ROW))
                        .bg(gpui::white())
                        // On a light plate, Microsoft's white button keeps an edge.
                        .when(ui::light(), |d| {
                            d.border_1().border_color(ui::white(fill::ACTIVE))
                        })
                        .cursor_pointer()
                        .hover(|s| s.opacity(crate::glass_ui::HOVER_OPACITY))
                        .font_family(eclipse_theme::glass::FONT)
                        .text_size(px(15.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(ui::color(eclipse_theme::glass::ON_LIGHT))
                        .child(microsoft::logo(18.))
                        .child(tr("Sign in with Microsoft")),
                    &t,
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.start_sign_in();
                    cx.notify();
                }))
                .into_any_element(),
            )
        } else {
            action.map(|a| div().mt(px(20.)).child(a).into_any_element())
        };
        div()
            .flex()
            .flex_col()
            .p(px(16.))
            .when(has_accounts, |d| {
                d.child(rows.mx(px(-16.)).mt(px(-16.)).mb(px(16.)))
            })
            .child(microsoft::logo(36.))
            .child(ui::card_title(heading).mt(px(20.)))
            .child(
                ui::text(14., FontWeight::NORMAL, ink::MUTED)
                    .mt(px(8.))
                    .line_height(px(21.7))
                    .child(note),
            )
            .children(button)
            .when(configured, |d| {
                d.child(
                    ui::text(12., FontWeight::NORMAL, ink::DIM)
                        .mt(px(12.))
                        .line_height(px(18.))
                        .child(tr(
                            "Microsoft's own page opens in your browser. Eclipse never sees your \
                             password.",
                        )),
                )
            })
    }
}
