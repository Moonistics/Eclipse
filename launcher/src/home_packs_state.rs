//! The selected instance's resource packs and shader packs on Home (`eclipse_launcher::packs`):
//! each turned on or off with one click, Remove (to the trash, after a confirmation), and
//! Add (a file picker) or a .zip or .eshader dropped anywhere on the section.
use super::*;
use crate::menus::Asking;
use eclipse_launcher::packs::{self, Kind as PackKind, Pack};
use gpui::{Image, ImageFormat};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Default)]
pub(super) struct Packs {
    home: Option<PathBuf>,
    pub(super) resource: Vec<(Pack, Option<Arc<Image>>)>,
    pub(super) shader: Vec<Pack>,
    fresh: bool,
    /// What the last action did, or why it failed.
    pub(super) note: Option<Result<String, String>>,
}

impl Packs {
    pub(super) fn stale(&mut self) {
        self.fresh = false;
    }
}

impl Launcher {
    pub(super) fn load_packs_list(&mut self, home: PathBuf) {
        if self.game_packs.home.as_ref() != Some(&home) {
            self.game_packs = Packs {
                home: Some(home.clone()),
                ..Packs::default()
            };
        }
        if self.game_packs.fresh {
            return;
        }
        // A few small reads per pack (pack.mcmeta, pack.png): fine while drawing.
        self.game_packs.resource = packs::list(&home, PackKind::Resource)
            .into_iter()
            .map(|p| {
                let icon = p
                    .icon
                    .clone()
                    .map(|png| Arc::new(Image::from_bytes(ImageFormat::Png, png)));
                (p, icon)
            })
            .collect();
        self.game_packs.shader = packs::list(&home, PackKind::Shader);
        self.game_packs.fresh = true;
    }

    pub(super) fn pack_action(
        &mut self,
        act: impl FnOnce(&std::path::Path) -> Result<String, String>,
    ) {
        let Some(home) = self.game_packs.home.clone() else {
            return;
        };
        self.game_packs.note = Some(act(&home));
        self.stop_asking(|a| matches!(a, Asking::GamePack(..)));
        self.game_packs.stale();
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
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(tr("Add").into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = paths.await {
                let _ = this.update(cx, |this, cx| {
                    this.add_packs(paths);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// How many resource and shader packs the selected instance has.
    pub(super) fn packs_count(&mut self) -> usize {
        let Some(home) = self.selected_instance().map(|i| i.home.clone()) else {
            return 0;
        };
        self.load_packs_list(home);
        self.game_packs.resource.len() + self.game_packs.shader.len()
    }
}
