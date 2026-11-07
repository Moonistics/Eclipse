//! The selected instance's worlds on Home (`eclipse_launcher::worlds`). Reading sizes and
//! copying run in the background.
use crate::folder_list::FolderList;
use crate::Launcher;
use eclipse_launcher::i18n::trf;
use eclipse_launcher::worlds::{self, World};
use gpui::prelude::*;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub(super) struct Worlds {
    pub(super) list: FolderList<World>,
    /// The world being backed up, duplicated or deleted now.
    pub(super) busy: Option<String>,
    /// What the last action did (a backup's zip), or why it failed.
    pub(super) done: Option<Result<(String, Option<PathBuf>), String>>,
}

pub(super) enum Action {
    Backup,
    Duplicate,
    Delete,
}

impl Launcher {
    /// Reads the worlds of `home` in the background when the list is missing, old or of another
    /// instance.
    pub(super) fn load_worlds(&mut self, home: &Path, cx: &mut Context<Self>) {
        if self.worlds.list.home() != Some(home) {
            self.worlds.busy = None;
            self.worlds.done = None;
        }
        self.read_folder(home, |this| &mut this.worlds.list, worlds::list, |_| {}, cx);
    }

    pub(super) fn world_action(&mut self, action: Action, world: World, cx: &mut Context<Self>) {
        let Some(home) = self.worlds.list.home().map(Path::to_path_buf) else {
            return;
        };
        if self.worlds.busy.is_some() {
            return;
        }
        self.worlds.busy = Some(world.folder.clone());
        self.worlds.done = None;
        let task = cx.background_executor().spawn(async move {
            match action {
                Action::Backup => worlds::backup(&home, &world)
                    .map(|zip| (trf("{name} backed up", &[("name", &world.name)]), Some(zip))),
                Action::Duplicate => worlds::copy(&world)
                    .map(|c| (trf("Duplicated as {name}", &[("name", &c.name)]), None)),
                Action::Delete => worlds::delete(&world).map(|()| {
                    (
                        trf("{name} moved to the trash", &[("name", &world.name)]),
                        None,
                    )
                }),
            }
        });
        cx.spawn(async move |this, cx| {
            let done = task.await;
            let _ = this.update(cx, |this, cx| {
                this.worlds.busy = None;
                this.worlds.done = Some(done);
                this.worlds.list.stale();
                cx.notify();
            });
        })
        .detach();
    }

    /// How many worlds the selected instance has; None while they are read.
    pub(super) fn worlds_count(&mut self, cx: &mut Context<Self>) -> Option<usize> {
        let home = self.selected_instance()?.home.clone();
        self.load_worlds(&home, cx);
        self.worlds.list.fresh().then_some(self.worlds.list.len())
    }

    pub(super) fn datapack_world(&self) -> Option<&World> {
        let list = self.worlds.list.of(&self.mods.home())?;
        (list.iter())
            .find(|w| Some(&w.path) == self.mods.world.as_ref())
            .or(list.first())
    }
}
