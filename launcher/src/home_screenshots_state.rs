//! The selected instance's screenshots on Home (`eclipse_launcher::screenshots`): the newest as
//! thumbnails, all of them on request, and a viewer (← and → step through) with Copy, Open,
//! Show in folder and Delete (to the trash, after a confirmation). Pictures are made at their
//! shown size (`pictures`), never decoded whole.
use crate::folder_list::FolderList;
use crate::menus::Asking;
use crate::{clipboard, Launcher};
use eclipse_launcher::i18n::{tr, trf};
use eclipse_launcher::screenshots::{self, Shot};
use gpui::prelude::*;
use gpui::{ClipboardItem, Image, ImageFormat};
use std::path::{Path, PathBuf};

/// Thumbnails shown before "Show all".
pub(super) const FIRST: usize = 8;

#[derive(Default)]
pub(super) struct Screenshots {
    pub(super) list: FolderList<Shot>,
    pub(super) all: bool,
    /// The picture open in the viewer.
    pub(super) viewing: Option<PathBuf>,
    /// What the last action did, or why it failed.
    pub(super) note: Option<Result<String, String>>,
}

#[derive(Default)]
pub(super) struct ShotBlur {
    /// Whether the screenshots are blurred.
    pub(super) hidden: bool,
}

impl Launcher {
    pub(super) fn load_screenshots(&mut self, home: &Path, cx: &mut Context<Self>) {
        if self.screenshots.list.home() != Some(home) {
            self.screenshots = Screenshots::default();
        }
        let at = self.viewing_at();
        let then = move |this: &mut Self| {
            if this.viewing_at().is_none() {
                let list = &this.screenshots.list;
                let last = list.len().checked_sub(1);
                let next = at.zip(last).and_then(|(i, l)| list.get(i.min(l)));
                this.screenshots.viewing = next.map(|s| s.path.clone());
            }
        };
        self.read_folder(
            home,
            |this| &mut this.screenshots.list,
            screenshots::list,
            then,
            cx,
        );
    }

    pub(super) fn viewing_at(&self) -> Option<usize> {
        let path = self.screenshots.viewing.as_ref()?;
        self.screenshots.list.iter().position(|s| &s.path == path)
    }

    /// Escape closes the viewer. Whether it did.
    pub(super) fn screenshot_escape(&mut self) -> bool {
        let open = self.screenshots.viewing.take().is_some();
        self.stop_asking(|a| *a == Asking::Screenshot);
        open
    }

    /// ← and → in the viewer: the newer and the older picture. Whether it moved.
    pub(super) fn screenshot_step(&mut self, older: bool) -> bool {
        let Some(i) = self.viewing_at() else {
            return false;
        };
        let next = match older {
            true => (i + 1 < self.screenshots.list.len()).then_some(i + 1),
            false => i.checked_sub(1),
        };
        if let Some(n) = next {
            self.screenshots.viewing = self.screenshots.list.get(n).map(|s| s.path.clone());
            self.stop_asking(|a| *a == Asking::Screenshot);
            self.screenshots.note = None;
        }
        true
    }

    pub(super) fn copy_screenshot(&mut self, shot: &Shot, cx: &mut Context<Self>) {
        let format = match shot.path.extension().and_then(|x| x.to_str()) {
            Some(x) if x.eq_ignore_ascii_case("png") => ImageFormat::Png,
            _ => ImageFormat::Jpeg,
        };
        self.screenshots.note = Some(match std::fs::read(&shot.path) {
            Ok(bytes) => {
                clipboard::copy_to_clipboard(
                    cx,
                    ClipboardItem::new_image(&Image::from_bytes(format, bytes)),
                );
                Ok(tr("Copied to the clipboard").into())
            }
            Err(e) => Err(trf(
                "Could not read {name}: {error}",
                &[("name", &shot.name), ("error", &e.to_string())],
            )),
        });
    }

    pub(super) fn delete_screenshot(&mut self, shot: &Shot) {
        self.stop_asking(|a| *a == Asking::Screenshot);
        self.screenshots.note = Some(
            screenshots::delete(shot)
                .map(|()| trf("{name} moved to the trash", &[("name", &shot.name)])),
        );
        self.screenshots.list.stale();
    }

    /// How many screenshots the selected instance has; None while they are read.
    pub(super) fn screenshots_count(&mut self, cx: &mut Context<Self>) -> Option<usize> {
        let home = self.selected_instance()?.home.clone();
        self.load_screenshots(&home, cx);
        self.screenshots
            .list
            .fresh()
            .then_some(self.screenshots.list.len())
    }
}
