//! A world's data packs, opened under its row on Home (`eclipse_launcher::datapacks`): what is
//! in `datapacks/`, Add (a file picker) or a zip dropped on the world, and Delete (to the trash,
//! after a confirmation).
use super::*;
use crate::menus::Asking;
use crate::ui::{self, white};
use eclipse_launcher::datapacks::{self, DataPack};
use eclipse_launcher::worlds::World;
use eclipse_theme::glass::{ink, space};
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
            note: Some(added.map(|names| {
                trf(
                    "Added {names}: the game turns it on when the world opens.",
                    &[("names", &names.join(", "))],
                )
            })),
            open: Some(world),
        };
    }

    fn choose_world_packs(&mut self, world: PathBuf, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(tr("Add").into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = paths.await {
                let _ = this.update(cx, |this, cx| {
                    this.add_world_packs(world, paths);
                    cx.notify();
                });
            }
        })
        .detach();
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
            None,
            tr("Add data pack…"),
            ButtonKind::Secondary,
            ui::ButtonSize::Md,
        )
        .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.choose_world_packs(path.clone(), cx);
            cx.notify();
        }));
        let folder = {
            let dir = world.path.join("datapacks");
            ui::button(
                "world-packs-folder",
                None,
                tr("Open data packs folder"),
                ButtonKind::Secondary,
                ui::ButtonSize::Md,
            )
            .on_press(move |_, _, cx| {
                let _ = std::fs::create_dir_all(&dir);
                crate::toast::open(cx, &dir);
            })
        };
        let (shown, empty_actions) = if empty {
            (None, vec![add, folder])
        } else {
            (
                Some(div().flex().gap(px(space::SM)).child(add).child(folder)),
                Vec::new(),
            )
        };
        let mut head = div().flex().flex_col().gap(px(space::SM)).child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(ui::label(tr("Data packs")))
                .children(shown),
        );
        let mut failed = None;
        match &self.world_packs.note {
            Some(Ok(m)) => head = head.child(ui::card_note(m.clone())),
            Some(Err(e)) => failed = Some(ui::error_note(e.clone())),
            None => {}
        }
        if empty {
            head = head.child(ui::empty_state(
                "world-packs-empty",
                "package-01",
                tr("No data packs yet"),
                tr("Add one from a file to change this world's rules or recipes."),
                empty_actions,
            ));
        }
        panel = panel.child(head).children(failed);
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
                            .map(|()| trf("{name} moved to the trash.", &[("name", &gone.file)]));
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
                .child(ui::icon("package-01", ui::IconSize::Px16, white(ink::MUTED)).relative())
                .child(ui::item_text(pack.file.clone(), pack.description.clone()))
                .child(actions.flex_none().relative()),
            );
        }
        Some(panel)
    }
}
