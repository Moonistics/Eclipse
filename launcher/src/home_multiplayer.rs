//! Home's Multiplayer tab: the selected instance's saved servers.
use crate::game_run::QuickPlay;
use crate::menus::Asking;
use crate::ui::{self, ButtonKind, Press};
use crate::{fields, home_multiplayer_state, layout, Launcher};
use eclipse_launcher::i18n::{tr, trf};
use eclipse_theme::glass::space;
use gpui::prelude::*;
use gpui::{div, px, ClickEvent, Div, SharedString, Stateful};

impl Launcher {
    pub(super) fn multiplayer_tab(&mut self, cx: &mut Context<Self>) -> Option<Div> {
        let home = self.selected_instance()?.home.clone();
        self.load_servers(&home, cx);
        self.ping_servers(cx);
        let list = self.multiplayer.list.to_vec();
        let adding = self.multiplayer.adding;
        let mut body = div().flex().flex_col();
        if list.is_empty() && !adding {
            let add = ui::button(
                "multiplayer-empty-add",
                Some(crate::icons::ADD),
                tr("Add server"),
                ButtonKind::Secondary,
                ui::ButtonSize::Md,
            )
            .on_press(toggle_add(cx));
            return Some(body.child(ui::empty_state(
                "multiplayer-empty",
                crate::icons::MULTIPLAYER,
                tr("No servers saved"),
                tr("Add one by its address."),
                vec![add],
            )));
        }
        body = body.children(adding.then(|| {
            div()
                .flex()
                .items_center()
                .min_h(px(ui::ButtonSize::Md.height()))
                .px(px(space::MD))
                .pb(px(space::SM))
                .child(ui::section_title("server-form-title", tr("Add a server")))
        }));
        if let Some(e) = self.multiplayer.error.clone() {
            body = body.child(ui::error_line_for(
                "multiplayer-failed",
                tr("Couldn't save the server list"),
                &e,
                None,
                Vec::new(),
            ));
        }
        if adding {
            let address_field = self
                .field(
                    fields::Field::MultiplayerAddress,
                    tr("Server address"),
                    layout::FIELD_LG,
                    ui::ButtonSize::Md,
                    cx,
                )
                .w_full();
            let name_field = self
                .field(
                    fields::Field::MultiplayerName,
                    tr("Server name"),
                    layout::FIELD_MD,
                    ui::ButtonSize::Md,
                    cx,
                )
                .w_full();
            let room = || div().flex_1().min_w(px(layout::FIELD_SM));
            body = body.child(
                div()
                    .id("server-form")
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(space::SM))
                    .px(px(space::MD))
                    .pb(px(space::SM))
                    .child(room().child(address_field))
                    .child(room().max_w(px(layout::FIELD_MD)).child(name_field))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(space::SM))
                            .child(
                                ui::button(
                                    "server-save",
                                    None,
                                    tr("Save server"),
                                    ButtonKind::Secondary,
                                    ui::ButtonSize::Md,
                                )
                                .on_press(cx.listener(
                                    |this, _: &ClickEvent, _, cx| {
                                        this.add_server();
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                ui::button(
                                    "server-add-cancel",
                                    None,
                                    tr("Cancel"),
                                    ButtonKind::Ghost,
                                    ui::ButtonSize::Md,
                                )
                                .on_press(toggle_add(cx)),
                            ),
                    ),
            );
        }
        for (server, icon) in list {
            body = body.child(self.server_row(server, icon, cx));
        }
        Some(body)
    }

    pub(super) fn add_server_button(&self, cx: &mut Context<Self>) -> Option<(Stateful<Div>, f32)> {
        if self.multiplayer.list.is_empty() || self.multiplayer.adding {
            return None;
        }
        let (label, size) = (tr("Add server"), ui::ButtonSize::Md);
        let add = ui::button(
            "server-add",
            Some(crate::icons::ADD),
            label,
            ButtonKind::Secondary,
            size,
        )
        .on_press(toggle_add(cx));
        let width = ui::button_width(label, true, ButtonKind::Secondary, size);
        Some((add, width))
    }

    fn server_row(
        &mut self,
        server: eclipse_launcher::server_list::Server,
        icon: Option<std::sync::Arc<gpui::Image>>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let key = SharedString::from(format!("server-{}", server.ip));
        let hovered = ui::hover_within(&key);
        let confirming = self.is_asking(&Asking::SavedServer(server.ip.clone()));
        let ip = server.ip.clone();
        // The dot and "address · 3 of 10", or "Offline" with Join off.
        let reach = self
            .multiplayer
            .reach
            .get(&server.ip)
            .copied()
            .unwrap_or(home_multiplayer_state::Reach::Asking);
        let (tone, line) = match reach {
            home_multiplayer_state::Reach::Online { players, max } => (
                ui::Tone::Live,
                format!(
                    "{} · {}",
                    server.ip,
                    trf(
                        "{online} of {max}",
                        &[("online", &players.to_string()), ("max", &max.to_string())]
                    )
                ),
            ),
            home_multiplayer_state::Reach::Offline => {
                (ui::Tone::Off, format!("{} · {}", server.ip, tr("Offline")))
            }
            home_multiplayer_state::Reach::Asking => (ui::Tone::Off, server.ip.clone()),
        };
        let offline = reach == home_multiplayer_state::Reach::Offline;
        let join = ui::button(
            SharedString::from(format!("{key}-join")),
            None,
            tr("Join"),
            ButtonKind::Secondary,
            ui::ButtonSize::Md,
        );
        let to = QuickPlay::Server(ip);
        let join = if offline {
            ui::unavailable(join, tr("The server is offline"))
        } else if let Some(why) = self.start_refusal(&to) {
            ui::unavailable(join, why)
        } else {
            join.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.play(Some(to.clone()));
                cx.notify();
            }))
        };
        let remove = SharedString::from(format!("{key}-remove"));
        let gone = server.ip.clone();
        let end = if confirming {
            ui::confirm(
                remove,
                ui::Confirm::Remove(&server.name),
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.asking = None;
                    cx.notify();
                }),
                cx.listener(move |this, _: &ClickEvent, _, cx| {
                    let gone = gone.clone();
                    this.edit_servers(|list| {
                        match list.servers().iter().position(|s| s.ip == gone) {
                            Some(at) => list.remove(at).map(|_| ()),
                            None => Ok(()),
                        }
                    });
                    cx.notify();
                }),
            )
        } else {
            div()
                .flex()
                .items_center()
                .gap(px(space::SM))
                .child(join)
                .child(
                    ui::remove_button(remove, tr("Remove…"), ui::ButtonSize::Md).on_press(
                        cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.asking = Some(Asking::SavedServer(gone.clone()));
                            cx.notify();
                        }),
                    ),
                )
        };
        ui::list_row(key, hovered || confirming)
            .child(ui::row_picture(
                icon.map(gpui::ImageSource::from),
                crate::icons::SERVER,
            ))
            .child(ui::item_text(server.name, ui::state_line(tone, line)))
            .child(end.relative().flex_none())
    }
}

fn toggle_add(cx: &mut Context<Launcher>) -> ui::Listener<ClickEvent> {
    Box::new(cx.listener(|this, _: &ClickEvent, _, cx| {
        this.multiplayer.adding = !this.multiplayer.adding;
        this.multiplayer.error = None;
        if this.multiplayer.adding {
            this.fields.focus_soon = Some(fields::Field::MultiplayerAddress);
        }
        cx.notify();
    }))
}
