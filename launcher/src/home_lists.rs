//! Home's Worlds and Multiplayer tabs: one row per world or server. A world's row plays it, its
//! size turning into "Play →" under the pointer (or why it can't); its ⋯ menu holds what the
//! launcher does besides (back up, duplicate, data packs, folder, delete). A server's row has
//! Join, and Remove under the pointer. Deleting or removing asks first, in place.
use super::*;
use crate::menus::{Asking, Menu};
use crate::ui::{self, white};
use eclipse_launcher::worlds::World;
use eclipse_theme::glass::{ink, space};
use gpui::ExternalPaths;
use home_worlds_state::Action;
use std::path::Path;

impl Launcher {
    /// Home's Worlds tab: the selected instance's worlds, the last played first.
    pub(super) fn worlds_tab(&mut self, cx: &mut Context<Self>) -> Option<Div> {
        let home = self.selected_instance()?.home.clone();
        self.load_worlds(home.clone(), cx);
        let list = self.worlds.list.clone();
        // What the last back up, duplicate or delete did.
        let done = match self.worlds.done.clone() {
            Some(Ok((message, zip))) => {
                let open = zip.map(|zip| {
                    let dir = zip.parent().map(Path::to_path_buf).unwrap_or(zip);
                    ui::open_folder_button(
                        "world-backups",
                        tr("Open backups folder"),
                        dir,
                        ui::ButtonSize::Md,
                    )
                });
                Some(ui::list_head(message).children(open))
            }
            Some(Err(e)) => Some(div().child(ui::error_line_for(
                "worlds-failed",
                tr("Couldn't change the world"),
                &e,
                None,
                Vec::new(),
            ))),
            None => None,
        };
        let mut body = div().flex().flex_col();
        let open = ui::open_folder_button(
            "worlds-folder",
            tr("Open saves folder"),
            home.join("saves"),
            ui::ButtonSize::Md,
        );
        if list.is_empty() {
            return Some(body.children(done).child(ui::empty_state(
                "worlds-empty",
                crate::icons::WORLD,
                tr("No worlds yet"),
                tr("Create one in the game."),
                vec![open],
            )));
        }
        body = body
            .child(ui::list_head(SharedString::default()).child(open))
            .children(done);
        for w in list {
            let row = self.world_row(&w, cx);
            body = body.child(row).children(self.world_packs_panel(&w, cx));
        }
        Some(body)
    }

    fn world_row(&mut self, w: &World, cx: &mut Context<Self>) -> Div {
        let key = SharedString::from(format!("world-{}", w.folder));
        let hovered = ui::hover_within(&key);
        let menu_open = self.menu_is(&Menu::ListRow(key.clone()));
        let busy = self.worlds.busy.is_some();
        let this_busy = self.worlds.busy.as_ref() == Some(&w.folder);
        let can_play = self.can_start_selected() && !busy;
        let why_not = (!can_play && !busy).then(|| {
            if self.selected_instance().is_some_and(|i| i.java.is_some()) {
                tr("Start it from the game's menu").to_string()
            } else {
                self.why_not_start()
            }
        });
        let mut facts = vec![tr(w.mode).to_string()];
        facts.extend(w.version.clone());
        facts.push(dates::ago(w.last_played));
        let focus = ui::try_focus_for(key.clone());
        let focused = focus.as_ref().is_some_and(ui::keyboard_focus);
        let menu_key = key.clone();
        let description = facts
            .iter()
            .cloned()
            .chain(why_not.clone())
            .collect::<Vec<_>>();
        let mut row = ui::list_row(key.clone(), hovered || menu_open)
            .aria_label(w.name.clone())
            .aria_description(description.join(" · "))
            .on_key_down(
                cx.listener(move |this, ev: &gpui::KeyDownEvent, window, cx| {
                    if ui::opens_row_menu(&ev.keystroke) {
                        this.open_menu_from(Menu::ListRow(menu_key.clone()), window, cx);
                        cx.stop_propagation();
                        cx.notify();
                    }
                }),
            );
        if can_play {
            let folder = w.folder.clone();
            if let Some(focus) = &focus {
                row = row.track_focus(focus);
            }
            row = row
                .cursor_pointer()
                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.quick_play("--world", folder.clone());
                    cx.notify();
                }));
        }
        // Data packs dropped on a world go into it.
        let path = w.path.clone();
        row =
            ui::drop_target(row).on_drop(cx.listener(move |this, paths: &ExternalPaths, _, cx| {
                this.add_world_packs(path.clone(), paths.paths().to_vec());
                cx.stop_propagation();
                cx.notify();
            }));
        row = row.child(ui::row_picture(
            w.icon.clone().map(gpui::ImageSource::from),
            crate::icons::WORLD,
        ));
        let size = if this_busy {
            ui::card_note(tr("Working…"))
        } else if let Some(why) = why_not.filter(|_| hovered) {
            ui::card_note(why).truncate()
        } else if (hovered || focused) && can_play {
            div()
                .flex()
                .items_center()
                .gap(px(space::XS))
                .child(
                    ui::text(
                        eclipse_theme::glass::step::NOTE,
                        FontWeight::MEDIUM,
                        ink::FULL,
                    )
                    .child(tr("Play")),
                )
                .child(ui::icon(
                    "arrow-right-01",
                    ui::IconSize::Px16,
                    white(ink::FULL),
                ))
        } else {
            ui::card_note(megabytes(w.size))
        };
        let more = ui::more_button(
            SharedString::from(format!("{key}-more")),
            trf("{name} options", &[("name", &w.name)]),
            ButtonKind::Ghost,
            ui::ButtonSize::Md,
            menu_open,
        );
        let more = self.menu_opener(more, Menu::ListRow(key.clone()), cx);
        let mut more = div().relative().child(more);
        if menu_open {
            more = more.child(ui::menu_under(
                ui::ButtonSize::Md.height(),
                ui::Align::End,
                self.world_menu(w, cx),
            ));
        }
        div().child(
            row.child(ui::item_text(w.name.clone(), facts.join(" · ")))
                .child(size.relative())
                .child(more),
        )
    }

    /// A world's ⋯ menu: Back up, Duplicate world, its data packs, Open world folder, then
    /// Delete… (then asked to confirm).
    fn world_menu(&mut self, w: &World, cx: &mut Context<Self>) -> Stateful<Div> {
        let key = SharedString::from(format!("world-{}", w.folder));
        let confirming = self.is_asking(&Asking::WorldDelete(key.clone()));
        let act = |this: &mut Self, action: Action, w: &World, cx: &mut Context<Self>| {
            this.close_menu();
            this.world_action(action, w.clone(), cx);
            cx.notify();
        };
        let (backup, dup, packs, gone) = (w.clone(), w.clone(), w.clone(), w.clone());
        let folder = w.path.clone();
        let packs_label = if self.world_packs_open(w) {
            tr("Hide data packs")
        } else {
            tr("Data packs")
        };
        let delete = if confirming {
            ui::menu_confirm(
                "world-menu-delete",
                ui::Confirm::Delete(&w.name),
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.stop_asking(|a| matches!(a, Asking::WorldDelete(_)));
                    cx.notify();
                }),
                cx.listener(move |this, _: &ClickEvent, _, cx| {
                    act(this, Action::Delete, &gone, cx)
                }),
            )
            .into_any_element()
        } else {
            ui::menu_row("world-menu-delete", "delete-02", tr("Delete…"), true)
                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.asking = Some(Asking::WorldDelete(key.clone()));
                    cx.notify();
                }))
                .into_any_element()
        };
        self.menu_popover(
            "world-menu",
            trf("{name} options", &[("name", &w.name)]),
            cx,
        )
        .child(
            ui::menu_row("world-menu-backup", "download-04", tr("Back up"), false).on_press(
                cx.listener(move |this, _: &ClickEvent, _, cx| {
                    act(this, Action::Backup, &backup, cx)
                }),
            ),
        )
        .child(
            ui::menu_row(
                "world-menu-duplicate",
                "copy-01",
                tr("Duplicate world"),
                false,
            )
            .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                act(this, Action::Duplicate, &dup, cx)
            })),
        )
        .child(
            ui::menu_row("world-menu-packs", crate::icons::PACK, packs_label, false).on_press(
                cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.close_menu();
                    this.toggle_world_packs(&packs);
                    cx.notify();
                }),
            ),
        )
        .child(
            ui::menu_row(
                "world-menu-folder",
                "folder-01",
                tr("Open world folder"),
                false,
            )
            .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.close_menu();
                ui::open_folder(&folder, cx);
                cx.notify();
            })),
        )
        .child(delete)
    }

    /// Home's Multiplayer tab: the selected instance's saved servers (the game's own list), and
    /// a line to add one.
    pub(super) fn multiplayer_tab(&mut self, cx: &mut Context<Self>) -> Option<Div> {
        let home = self.selected_instance()?.home.clone();
        self.load_servers(home);
        self.ping_servers(cx);
        let list = self.multiplayer.list.clone();
        let adding = self.multiplayer.adding;
        let mut body = div().flex().flex_col();
        if list.is_empty() && !adding {
            let add = ui::button(
                "multiplayer-empty-add",
                Some("plus-sign"),
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
        body = body.child(if adding {
            div()
                .flex()
                .items_center()
                .min_h(px(ui::ButtonSize::Md.height()))
                .px(px(space::MD))
                .pb(px(space::SM))
                .child(ui::section_title("server-form-title", tr("Add a server")))
        } else {
            ui::list_head(SharedString::default()).child(
                ui::button(
                    "server-add",
                    Some("plus-sign"),
                    tr("Add server"),
                    ButtonKind::Secondary,
                    ui::ButtonSize::Md,
                )
                .on_press(toggle_add(cx)),
            )
        });
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
        let join = if offline {
            ui::unavailable(join, tr("The server is offline"))
        } else if self.can_join_selected() {
            join.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.quick_play("--server", ip.clone());
                cx.notify();
            }))
        } else {
            ui::unavailable(join, self.why_not_start())
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

/// Add server: opens or closes the line to add one, its address field focused.
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
