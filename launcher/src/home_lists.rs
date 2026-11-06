//! Home's Worlds and Multiplayer tabs: one row per world or server. A world's row plays it, its
//! size turning into "Play →" under the pointer; its ⋯ menu holds what the launcher does besides
//! (back up, copy, data packs, folder, delete). A server's row has Join, and Remove under the
//! pointer. Deleting or removing asks first, in place.
use super::*;
use crate::menus::{Asking, Menu};
use crate::ui::{self, white};
use eclipse_launcher::worlds::World;
use eclipse_theme::glass::{ink, space};
use gpui::{deferred, ExternalPaths};
use home_worlds_state::Action;
use std::path::Path;

impl Launcher {
    /// Records whether the pointer is over the row `key`; the row then shows its actions.
    pub(super) fn list_hover_listener(
        &self,
        key: SharedString,
        cx: &mut Context<Self>,
    ) -> ui::Listener<bool> {
        Box::new(cx.listener(move |this, hovered: &bool, _, cx| {
            if *hovered {
                this.list_hover = Some(key.clone());
            } else if this.list_hover.as_ref() == Some(&key) {
                this.list_hover = None;
            }
            cx.notify();
        }))
    }

    /// Home's Worlds tab: the selected instance's worlds, the last played first.
    pub(super) fn worlds_tab(&mut self, cx: &mut Context<Self>) -> Option<Div> {
        let home = self.selected_instance()?.home.clone();
        self.load_worlds(home.clone(), cx);
        let list = self.worlds.list.clone();
        let mut body = div().flex().flex_col();
        // What the last back up, copy or delete did.
        match self.worlds.done.clone() {
            Some(Ok((message, zip))) => {
                let open = zip.map(|zip| {
                    let dir = zip.parent().map(Path::to_path_buf).unwrap_or(zip);
                    ui::button(
                        "world-backups",
                        None,
                        tr("Open backups"),
                        ButtonKind::Secondary,
                        ui::ButtonSize::Md,
                    )
                    .on_press(move |_, _, cx| crate::toast::open(cx, &dir))
                });
                body = body.child(ui::list_head(message).children(open));
            }
            Some(Err(e)) => {
                body = body.child(ui::error_note(e).px(px(space::MD)).pb(px(space::SM)))
            }
            None => {}
        }
        if list.is_empty() {
            let saves = home.join("saves");
            let open = ui::button(
                "worlds-folder",
                Some("folder-01"),
                tr("Open folder"),
                ButtonKind::Secondary,
                ui::ButtonSize::Md,
            )
            .on_press(move |_, _, cx| {
                let _ = std::fs::create_dir_all(&saves);
                crate::toast::open(cx, &saves);
            });
            return Some(body.child(ui::empty_state(
                "worlds-empty",
                "earth",
                tr("No worlds yet"),
                tr("Create one in the game."),
                vec![open],
            )));
        }
        // Pictures line up: a world without one gets a plain tile when another has one.
        let pictures = list.iter().any(|w| w.icon.is_some());
        for w in list {
            let row = self.world_row(&w, pictures, cx);
            body = body.child(row).children(self.world_packs_panel(&w, cx));
        }
        Some(body)
    }

    fn world_row(&mut self, w: &World, pictures: bool, cx: &mut Context<Self>) -> Div {
        let key = SharedString::from(format!("world-{}", w.folder));
        let hovered = self.list_hover.as_ref() == Some(&key);
        let menu_open = self.menu_is(&Menu::ListRow(key.clone()));
        let busy = self.worlds.busy.is_some();
        let this_busy = self.worlds.busy.as_ref() == Some(&w.folder);
        let can_play = self.can_start_selected() && !busy;
        let mut facts = vec![tr(w.mode).to_string()];
        facts.extend(w.version.clone());
        facts.push(home_head::last_played(w.last_played));
        let hover = self.list_hover_listener(key.clone(), cx);
        let mut row = ui::list_row(key.clone(), hovered || menu_open, hover);
        if can_play {
            let folder = w.folder.clone();
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
        if pictures {
            row = row.child(ui::row_picture(
                w.icon.clone().map(gpui::ImageSource::from),
                "earth",
            ));
        }
        let size = if this_busy {
            ui::text(
                eclipse_theme::glass::step::NOTE,
                FontWeight::NORMAL,
                ink::MUTED,
            )
            .child(tr("Working…"))
        } else if hovered && can_play {
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
            ui::text(
                eclipse_theme::glass::step::NOTE,
                FontWeight::NORMAL,
                ink::MUTED,
            )
            .child(megabytes(w.size))
        };
        let more = ui::more_button(
            SharedString::from(format!("{key}-more")),
            trf("{name} options", &[("name", &w.name)]),
            ButtonKind::Ghost,
            ui::ButtonSize::Md,
            menu_open,
        )
        .on_press(cx.listener({
            let key = key.clone();
            move |this, event: &ClickEvent, window, cx| {
                if !this.closed_by_this_press(event) {
                    this.toggle_menu(Menu::ListRow(key.clone()), window, cx);
                }
                cx.stop_propagation();
                cx.notify();
            }
        }));
        let mut more = div().relative().child(more);
        if menu_open {
            more = more.child(
                deferred(
                    div()
                        .absolute()
                        .top(px(ui::ButtonSize::Md.height() + ui::MENU_GAP))
                        .right_0()
                        .child(self.world_menu(w, cx)),
                )
                .with_priority(1),
            );
        }
        div().child(
            row.child(ui::item_text(w.name.clone(), facts.join(" · ")))
                .child(size.relative())
                .child(more),
        )
    }

    /// A world's ⋯ menu: Back up, Copy, its data packs, Open folder, then Delete… (then asked to
    /// confirm).
    fn world_menu(&mut self, w: &World, cx: &mut Context<Self>) -> Stateful<Div> {
        let key = SharedString::from(format!("world-{}", w.folder));
        let confirming = self.is_asking(&Asking::WorldDelete(key.clone()));
        let act = |this: &mut Self, action: Action, w: &World, cx: &mut Context<Self>| {
            this.close_menu();
            this.world_action(action, w.clone(), cx);
            cx.notify();
        };
        let (backup, copy, packs, gone) = (w.clone(), w.clone(), w.clone(), w.clone());
        let folder = w.path.clone();
        let packs_label = if self.world_packs_open(w) {
            tr("Hide data packs")
        } else {
            tr("Data packs")
        };
        let delete = if confirming {
            ui::menu_confirm(
                "world-menu-delete",
                trf("Delete {name}?", &[("name", &w.name)]),
                tr("Delete"),
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
                .mt(px(space::XS))
                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.asking = Some(Asking::WorldDelete(key.clone()));
                    cx.notify();
                }))
                .into_any_element()
        };
        ui::popover("world-menu")
            .on_mouse_down_out(cx.listener(|this, event: &MouseDownEvent, _, cx| {
                this.close_menu();
                this.menu_closed_at = Some(event.position);
                cx.notify();
            }))
            .child(
                ui::menu_row("world-menu-backup", "download-04", tr("Back up"), false).on_press(
                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                        act(this, Action::Backup, &backup, cx)
                    }),
                ),
            )
            .child(
                ui::menu_row("world-menu-copy", "copy-01", tr("Copy"), false).on_press(
                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                        act(this, Action::Copy, &copy, cx)
                    }),
                ),
            )
            .child(
                ui::menu_row("world-menu-packs", "package-01", packs_label, false).on_press(
                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.close_menu();
                        this.toggle_world_packs(&packs);
                        cx.notify();
                    }),
                ),
            )
            .child(
                ui::menu_row("world-menu-folder", "folder-01", tr("Open folder"), false).on_press(
                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.close_menu();
                        crate::toast::open(cx, &folder);
                        cx.notify();
                    }),
                ),
            )
            .child(delete)
    }

    /// Home's Multiplayer tab: the selected instance's saved servers (the game's own list), and
    /// a line to add one.
    pub(super) fn multiplayer_tab(&mut self, cx: &mut Context<Self>) -> Option<Div> {
        let instance = self.selected_instance()?;
        let (home, name) = (instance.home.clone(), instance.name.clone());
        self.load_servers(home);
        self.ping_servers(cx);
        let list = self.multiplayer.list.clone();
        let adding = self.multiplayer.adding;
        let mut body = div().flex().flex_col();
        if list.is_empty() && !adding {
            let add = ui::button(
                "server-add",
                Some("plus-sign"),
                tr("Add server"),
                ButtonKind::Secondary,
                ui::ButtonSize::Md,
            )
            .on_press(toggle_add(cx));
            return Some(body.child(ui::empty_state(
                "multiplayer-empty",
                "globe-02",
                tr("No servers saved"),
                tr("Add a server by its address to join it with one click."),
                vec![add],
            )));
        }
        let count = list.len().to_string();
        let note = match list.len() {
            0 => tr("No servers saved").into(),
            1 => trf("1 server in {name}", &[("name", &name)]),
            _ => trf(
                "{count} servers in {name}",
                &[("count", &count), ("name", &name)],
            ),
        };
        body = body.child(
            ui::list_head(note).child(
                ui::button(
                    "server-add",
                    Some("plus-sign"),
                    tr("Add server"),
                    ButtonKind::Secondary,
                    ui::ButtonSize::Md,
                )
                .on_press(toggle_add(cx)),
            ),
        );
        if let Some(e) = self.multiplayer.error.clone() {
            body = body.child(ui::error_note(e).px(px(space::MD)).pb(px(space::SM)));
        }
        if adding {
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::SM))
                    .px(px(space::MD))
                    .pb(px(space::SM))
                    .child(self.field(
                        fields::Field::MultiplayerName,
                        tr("Minecraft Server"),
                        220.,
                        ui::ButtonSize::Md,
                        cx,
                    ))
                    .child(self.field(
                        fields::Field::MultiplayerAddress,
                        "play.example.com",
                        300.,
                        ui::ButtonSize::Md,
                        cx,
                    ))
                    .child(
                        ui::button(
                            "server-save",
                            None,
                            tr("Add"),
                            ButtonKind::Secondary,
                            ui::ButtonSize::Lg,
                        )
                        .on_press(cx.listener(
                            |this, _: &ClickEvent, _, cx| {
                                this.add_server();
                                cx.notify();
                            },
                        )),
                    ),
            );
        }
        let pictures = list.iter().any(|(_, icon)| icon.is_some());
        for (i, (server, icon)) in list.into_iter().enumerate() {
            body = body.child(self.server_row(i, server, icon, pictures, cx));
        }
        Some(body)
    }

    fn server_row(
        &mut self,
        i: usize,
        server: eclipse_launcher::server_list::Server,
        icon: Option<std::sync::Arc<gpui::Image>>,
        pictures: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let key = SharedString::from(format!("server-{i}"));
        let hovered = self.list_hover.as_ref() == Some(&key);
        let confirming = self.is_asking(&Asking::SavedServer(i));
        let hover = self.list_hover_listener(key.clone(), cx);
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
        let join = if self.can_join_selected() && !offline {
            join.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.quick_play("--server", ip.clone());
                cx.notify();
            }))
        } else {
            ui::disabled(join)
        };
        let remove = SharedString::from(format!("{key}-remove"));
        let end = if confirming {
            ui::confirm(
                remove,
                trf("Remove {name}?", &[("name", &server.name)]),
                tr("Remove"),
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.asking = None;
                    cx.notify();
                }),
                cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.edit_servers(|list| list.remove(i).map(|_| ()));
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
                    ui::icon_button(
                        remove,
                        "delete-02",
                        tr("Remove…"),
                        ButtonKind::Ghost,
                        ui::ButtonSize::Md,
                        false,
                    )
                    .on_press(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            this.asking = Some(Asking::SavedServer(i));
                            cx.notify();
                        },
                    )),
                )
        };
        ui::list_row(key, hovered || confirming, hover)
            .child(div().relative().flex_none().child(ui::dot(tone)))
            .when(pictures, |r| {
                r.child(ui::row_picture(
                    icon.map(gpui::ImageSource::from),
                    "globe-02",
                ))
            })
            .child(ui::item_text(server.name, line))
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
