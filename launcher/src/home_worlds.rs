//! Home's Worlds tab: one row per world, which plays it.
use crate::game_run::QuickPlay;
use crate::home_worlds_state::Action;
use crate::menus::{Asking, Menu};
use crate::ui::{self, ink, ButtonKind, Press};
use crate::{dates, megabytes, Launcher};
use eclipse_launcher::i18n::{tr, trf};
use eclipse_launcher::worlds::{self, World};
use eclipse_theme::glass::{ink, space};
use gpui::prelude::*;
use gpui::{div, px, ClickEvent, Div, ExternalPaths, FontWeight, SharedString, Stateful};
use std::path::Path;

impl Launcher {
    /// Home's Worlds tab: the selected instance's worlds, the last played first.
    pub(super) fn worlds_tab(&mut self, cx: &mut Context<Self>) -> Option<Div> {
        let home = self.selected_instance()?.home.clone();
        self.load_worlds(&home, cx);
        let list = self.worlds.list.to_vec();
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
        if list.is_empty() {
            let open = ui::open_folder_button(
                "worlds-folder",
                tr("Open saves folder"),
                worlds::saves_dir(&home),
                ui::ButtonSize::Md,
            );
            return Some(body.children(done).child(ui::empty_state(
                "worlds-empty",
                crate::icons::WORLD,
                tr("No worlds yet"),
                tr("Create one in the game."),
                vec![open],
            )));
        }
        body = body.children(done);
        for w in list {
            let row = self.world_row(&w, cx);
            body = body.child(row).children(self.world_packs_panel(&w, cx));
        }
        Some(body)
    }

    pub(super) fn worlds_folder(&self) -> Option<(Stateful<Div>, f32)> {
        let saves = worlds::saves_dir(&self.selected_instance()?.home);
        let size = ui::ButtonSize::Md;
        let open = ui::open_folder_icon(
            "worlds-folder",
            tr("Open saves folder"),
            saves,
            ButtonKind::Ghost,
            size,
        );
        (!self.worlds.list.is_empty()).then_some((open, size.height()))
    }

    fn world_row(&mut self, w: &World, cx: &mut Context<Self>) -> Div {
        let key = SharedString::from(format!("world-{}", w.folder));
        let hovered = ui::hover_within(&key);
        let menu_open = self.menu_is(&Menu::WorldRow(w.folder.clone()));
        let busy = self.worlds.busy.is_some();
        let this_busy = self.worlds.busy.as_ref() == Some(&w.folder);
        let refused = self.start_refusal(&QuickPlay::World(w.folder.clone()));
        let can_play = refused.is_none() && !busy;
        let why_not = refused.filter(|_| !busy);
        let mut facts = vec![tr(w.mode).to_string()];
        facts.extend(w.version.clone());
        facts.push(dates::ago(w.last_played));
        let focus = ui::try_focus_for(key.clone());
        let focused = focus.as_ref().is_some_and(ui::keyboard_focus);
        let menu_key = w.folder.clone();
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
                        this.open_menu_from(Menu::WorldRow(menu_key.clone()), window, cx);
                        cx.stop_propagation();
                        cx.notify();
                    }
                }),
            );
        if can_play {
            let folder = w.folder.clone();
            if let Some(focus) = &focus {
                row = ui::keep_focus(row, focus);
            }
            row = row
                .cursor_pointer()
                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.play(Some(QuickPlay::World(folder.clone())));
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
                    ink(ink::FULL),
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
        let more = self.menu_opener(more, Menu::WorldRow(w.folder.clone()), cx);
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

    fn world_menu(&mut self, w: &World, cx: &mut Context<Self>) -> Stateful<Div> {
        let confirming = self.is_asking(&Asking::WorldDelete(w.folder.clone()));
        let act = |this: &mut Self, action: Action, w: &World, cx: &mut Context<Self>| {
            this.close_menu();
            this.world_action(action, w.clone(), cx);
            cx.notify();
        };
        let (backup, dup, packs, gone) = (w.clone(), w.clone(), w.clone(), w.clone());
        let folder_asked = w.folder.clone();
        let folder = w.path.clone();
        let packs_label = if self.world_packs_open(w) {
            tr("Hide data packs")
        } else {
            tr("Show data packs")
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
                    this.asking = Some(Asking::WorldDelete(folder_asked.clone()));
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
            ui::menu_row(
                "world-menu-backup",
                crate::icons::DOWNLOAD,
                tr("Back up world"),
                false,
            )
            .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                act(this, Action::Backup, &backup, cx)
            })),
        )
        .child(
            ui::menu_row(
                "world-menu-duplicate",
                crate::icons::COPY,
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
                crate::icons::OPEN_FOLDER,
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
}
