//! The selected instance's resource packs and shader packs on Home (`eclipse_launcher::packs`):
//! each turned on or off with one click, Remove (to the trash, after a confirmation), and
//! Add (a file picker) or a .zip or .eshader dropped anywhere on the section.
use crate::folder_list::FolderList;
use crate::menus::Asking;
use crate::{Launcher, Pick};
use eclipse_launcher::i18n::{tr, trf};
use eclipse_launcher::packs::{self, Kind as PackKind, Pack};
use gpui::prelude::*;
use gpui::{Image, ImageFormat};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Default)]
pub(super) struct Packs {
    pub(super) list: FolderList<(Pack, Option<Arc<Image>>)>,
    /// What the last action did, or why it failed.
    pub(super) note: Option<Result<String, String>>,
}

fn read(home: &Path) -> Vec<(Pack, Option<Arc<Image>>)> {
    let resource = packs::list(home, PackKind::Resource).into_iter().map(|p| {
        let icon = p
            .icon
            .clone()
            .map(|png| Arc::new(Image::from_bytes(ImageFormat::Png, png)));
        (p, icon)
    });
    let shader = packs::list(home, PackKind::Shader)
        .into_iter()
        .map(|p| (p, None));
    resource.chain(shader).collect()
}

impl Launcher {
    pub(super) fn load_packs_list(&mut self, home: &Path, cx: &mut Context<Self>) {
        if self.game_packs.list.home() != Some(home) {
            self.game_packs.note = None;
        }
        self.read_folder(home, |this| &mut this.game_packs.list, read, |_| {}, cx);
    }

    pub(super) fn pack_action(
        &mut self,
        act: impl FnOnce(&std::path::Path) -> Result<String, String>,
    ) {
        let Some(home) = self.game_packs.list.home().map(Path::to_path_buf) else {
            return;
        };
        self.game_packs.note = Some(act(&home));
        self.stop_asking(|a| matches!(a, Asking::GamePack(..)));
        self.game_packs.list.stale();
    }

    /// Copies dropped or picked files into the instance's pack folders.
    pub(super) fn add_packs(&mut self, files: Vec<PathBuf>) {
        self.pack_action(|home| {
            let mut added = Vec::new();
            for file in &files {
                added.push(packs::add(home, file)?.title);
            }
            Ok(trf(
                "Added {names}. Turn it on to use it.",
                &[("names", &added.join(", "))],
            ))
        });
    }

    pub(super) fn choose_packs(&mut self, cx: &mut Context<Self>) {
        self.pick_paths(Pick::Files, tr("Add"), cx, |this, paths, _| {
            this.add_packs(paths)
        });
    }

    /// How many resource and shader packs the selected instance has; None while they are read.
    pub(super) fn packs_count(&mut self, cx: &mut Context<Self>) -> Option<usize> {
        let home = self.selected_instance()?.home.clone();
        self.load_packs_list(&home, cx);
        self.game_packs
            .list
            .fresh()
            .then_some(self.game_packs.list.len())
    }
}
