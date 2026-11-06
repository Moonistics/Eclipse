//! The selected instance's worlds on Home (`eclipse_launcher::worlds`): each with its picture,
//! mode, version, when it was last played and its size, and Back up, Copy, Delete (to the
//! trash, after a confirmation) and Open folder. Reading sizes and copying run in the background.
use super::*;
use eclipse_launcher::worlds::{self, World};
use std::path::PathBuf;

#[derive(Default)]
pub(super) struct Worlds {
    /// The game folder the list is of, and the list as last read: kept on screen while it is
    /// read again, so the page doesn't jump.
    pub(super) home: Option<PathBuf>,
    pub(super) list: Vec<World>,
    fresh: bool,
    reading: bool,
    /// The world being backed up, copied or deleted now.
    pub(super) busy: Option<String>,
    /// What the last action did (a backup's zip), or why it failed.
    pub(super) done: Option<Result<(String, Option<PathBuf>), String>>,
}

impl Worlds {
    /// Read again on the next frame: the game may have saved, or an action changed the list.
    pub(super) fn stale(&mut self) {
        self.fresh = false;
    }
}

pub(super) enum Action {
    Backup,
    Copy,
    Delete,
}

impl Launcher {
    /// Reads the worlds of `home` in the background when the list is missing or of another
    /// instance.
    pub(super) fn load_worlds(&mut self, home: PathBuf, cx: &mut Context<Self>) {
        if self.worlds.home.as_ref() != Some(&home) {
            self.worlds = Worlds {
                home: Some(home.clone()),
                ..Worlds::default()
            };
        }
        if self.worlds.fresh || self.worlds.reading {
            return;
        }
        self.worlds.reading = true;
        let read = home.clone();
        let task = cx
            .background_executor()
            .spawn(async move { worlds::list(&read) });
        cx.spawn(async move |this, cx| {
            let list = task.await;
            let _ = this.update(cx, |this, cx| {
                // Another instance may have been picked meanwhile.
                if this.worlds.home.as_ref() == Some(&home) {
                    this.worlds.list = list;
                    this.worlds.fresh = true;
                    this.worlds.reading = false;
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn world_action(&mut self, action: Action, world: World, cx: &mut Context<Self>) {
        let Some(home) = self.worlds.home.clone() else {
            return;
        };
        if self.worlds.busy.is_some() {
            return;
        }
        self.worlds.busy = Some(world.folder.clone());
        self.worlds.done = None;
        let task = cx.background_executor().spawn(async move {
            match action {
                Action::Backup => worlds::backup(&home, &world).map(|zip| {
                    (
                        trf("{name} backed up.", &[("name", &world.name)]),
                        Some(zip),
                    )
                }),
                Action::Copy => worlds::copy(&world)
                    .map(|c| (trf("Copied to {name}.", &[("name", &c.name)]), None)),
                Action::Delete => worlds::delete(&world).map(|()| {
                    (
                        trf("{name} moved to the trash.", &[("name", &world.name)]),
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
                this.worlds.stale();
                cx.notify();
            });
        })
        .detach();
    }

    /// How many worlds the selected instance has; None while they are read.
    pub(super) fn worlds_count(&mut self, cx: &mut Context<Self>) -> Option<usize> {
        let home = self.selected_instance()?.home.clone();
        self.load_worlds(home, cx);
        self.worlds.fresh.then_some(self.worlds.list.len())
    }
}
