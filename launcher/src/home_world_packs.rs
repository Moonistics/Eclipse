//! A world's data packs, opened under its row on Home (`eclipse_launcher::datapacks`): what is
//! in `datapacks/`, Add (a file picker) or a zip dropped on the world, and Delete (to the trash,
//! after a confirmation).
use crate::menus::Asking;
use crate::ui::{self, white, ButtonKind, Press};
use crate::{Launcher, Pick};
use eclipse_launcher::datapacks::{self, DataPack};
use eclipse_launcher::i18n::{tr, trf};
use eclipse_launcher::worlds::World;
use eclipse_theme::glass::{ink, space};
use gpui::prelude::*;
use gpui::{div, px, ClickEvent, Div, SharedString};
use std::path::PathBuf;

const WORLD_PACKS_INDENT: f32 = space::MD + ui::TILE_MD + space::MD;

#[derive(Default)]
pub(super) struct WorldPacks {
    /// The world whose packs are open, and its packs as last read.
    open: Option<PathBuf>,
    list: Vec<DataPack>,
    note: Option<Result<String, String>>,
}

impl WorldPacks {
    /// Reads the open world's packs again, after something else changed them.
    pub(super) fn reread(&mut self) {
        if let Some(world) = &self.open {
            self.list = datapacks::list(world);
        }
    }
}

impl Launcher {
    /// Opens or closes the packs of `world` under its row.
    pub(super) fn toggle_world_packs(&mut self, world: &World) {
        if self.world_packs.open.as_ref() == Some(&world.path) {
            self.world_packs = WorldPacks::default();
        } else {
            self.world_packs = WorldPacks {
                open: Some(world.path.clone()),
                list: datapacks::list(&world.path),
                ..WorldPacks::default()
            };
        }
    }

    pub(super) fn world_packs_open(&self, world: &World) -> bool {
        self.world_packs.open.as_ref() == Some(&world.path)
    }

    /// Copies dropped or picked zips into `world`, and opens its packs to show them.
    pub(super) fn add_world_packs(&mut self, world: PathBuf, files: Vec<PathBuf>) {
        let added: Result<Vec<String>, String> = files
            .iter()
            .map(|f| datapacks::add(&world, f).map(|p| p.file))
            .collect();
        self.world_packs = WorldPacks {
            list: datapacks::list(&world),
            note: Some(added.map(|names| trf("Added {names}", &[("names", &names.join(", "))]))),
            open: Some(world),
        };
    }

    fn choose_world_packs(&mut self, world: PathBuf, cx: &mut Context<Self>) {
        self.pick_paths(Pick::Files, tr("Add"), cx, |this, paths, _| {
            this.add_world_packs(world, paths)
        });
    }

    /// The open world's packs, under its row.
    pub(super) fn world_packs_panel(
        &mut self,
        world: &World,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        if !self.world_packs_open(world) {
            return None;
        }
        let path = world.path.clone();
        let empty = self.world_packs.list.is_empty();
        let mut panel = ui::well()
            .flex()
            .flex_col()
            .gap(px(space::SM))
            .ml(px(WORLD_PACKS_INDENT))
            .mb(px(space::SM));
        let add = ui::button(
            "world-packs-add",
            Some(crate::icons::ADD),
            tr("Add data pack…"),
            ButtonKind::Secondary,
            ui::ButtonSize::Md,
        )
        .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.choose_world_packs(path.clone(), cx);
            cx.notify();
        }));
        let folder = {
            ui::open_folder_button(
                "world-packs-folder",
                tr("Open data packs folder"),
                datapacks::folder(&world.path),
                ui::ButtonSize::Md,
            )
        };
        let (shown, empty_actions) = if empty {
            (None, vec![add, folder])
        } else {
            (
                Some(div().flex().gap(px(space::SM)).child(add).child(folder)),
                Vec::new(),
            )
        };
        let mut head = div().flex().flex_col().gap(px(space::SM));
        let mut failed = None;
        match &self.world_packs.note {
            Some(Ok(m)) => head = head.child(ui::card_note(m.clone())),
            Some(Err(e)) => {
                failed = Some(ui::error_line_for(
                    "world-packs-failed",
                    tr("Couldn't change the data packs"),
                    e,
                    None,
                    Vec::new(),
                ))
            }
            None => {}
        }
        if empty {
            head = head.child(ui::empty_state(
                "world-packs-empty",
                crate::icons::PACK,
                tr("No data packs yet"),
                tr("Add one from a file to change this world's rules or recipes."),
                empty_actions,
            ));
        }
        panel = panel
            .child(ui::field_group(
                "world-packs-title",
                tr("Data packs"),
                shown.map(IntoElement::into_any_element),
                head,
            ))
            .children(failed);
        for pack in self.world_packs.list.clone() {
            let confirming = self.is_asking(&Asking::WorldPack(pack.file.clone()));
            let remove_id = SharedString::from(format!("world-pack-remove-{}", pack.file));
            let mut actions = div().flex().items_center().gap(px(space::SM));
            if confirming {
                let (world, gone) = (world.path.clone(), pack.clone());
                actions = actions.child(ui::confirm(
                    remove_id,
                    ui::Confirm::Delete(&pack.file),
                    cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.asking = None;
                        cx.notify();
                    }),
                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                        let note = datapacks::remove(&world, &gone)
                            .map(|()| trf("{name} moved to the trash", &[("name", &gone.file)]));
                        this.world_packs.list = datapacks::list(&world);
                        this.world_packs.note = Some(note);
                        this.asking = None;
                        cx.notify();
                    }),
                ));
            } else {
                let file = pack.file.clone();
                if !pack.enabled {
                    actions = actions.child(ui::card_note(tr("Off in the game")));
                }
                actions = actions.child(
                    ui::remove_button(remove_id, tr("Delete…"), ui::ButtonSize::Md).on_press(
                        cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.asking = Some(Asking::WorldPack(file.clone()));
                            cx.notify();
                        }),
                    ),
                );
            }
            panel = panel.child(
                ui::list_row(
                    SharedString::from(format!("world-pack-{}", pack.file)),
                    confirming,
                )
                .child(
                    ui::icon(crate::icons::PACK, ui::IconSize::Px16, white(ink::MUTED)).relative(),
                )
                .child(ui::item_text(pack.file.clone(), pack.description.clone()))
                .child(actions.flex_none().relative()),
            );
        }
        Some(panel)
    }
}
