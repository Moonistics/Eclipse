//! The selected instance's screenshots on Home (`eclipse_launcher::screenshots`): the newest as
//! thumbnails, all of them on request, and a viewer (← and → step through) with Copy, Open,
//! Show in folder and Delete (to the trash, after a confirmation). Pictures are made at their
//! shown size (`picture_cache`), never decoded whole.
use super::*;
use crate::menus::Asking;
use crate::picture_cache::Look;
use crate::ui;
use eclipse_launcher::screenshots::{self, Shot};
use eclipse_theme::glass::{radius, space};
use gpui::{ClipboardItem, Image, ImageFormat};
use std::path::PathBuf;

/// Thumbnails shown before "Show all".
pub(super) const FIRST: usize = 8;

#[derive(Default)]
pub(super) struct Screenshots {
    home: Option<PathBuf>,
    pub(super) list: Vec<Shot>,
    fresh: bool,
    pub(super) all: bool,
    /// The picture open in the viewer.
    pub(super) viewing: Option<PathBuf>,
    /// What the last action did, or why it failed.
    pub(super) note: Option<Result<String, String>>,
}

impl Screenshots {
    /// Read again on the next frame: the game may have taken new ones.
    pub(super) fn stale(&mut self) {
        self.fresh = false;
    }
}

impl Launcher {
    pub(super) fn load_screenshots(&mut self, home: PathBuf) {
        if self.screenshots.home.as_ref() != Some(&home) {
            self.screenshots = Screenshots {
                home: Some(home.clone()),
                ..Screenshots::default()
            };
        }
        if !self.screenshots.fresh {
            let at = self.viewing_at();
            // One directory listing: cheap enough while drawing.
            self.screenshots.list = screenshots::list(&home);
            self.screenshots.fresh = true;
            if self.viewing_at().is_none() {
                let last = self.screenshots.list.len().checked_sub(1);
                self.screenshots.viewing = at
                    .zip(last)
                    .and_then(|(i, l)| self.screenshots.list.get(i.min(l)))
                    .map(|s| s.path.clone());
            }
        }
    }

    fn viewing_at(&self) -> Option<usize> {
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

    fn copy_screenshot(&mut self, shot: &Shot, cx: &mut Context<Self>) {
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

    fn delete_screenshot(&mut self, shot: &Shot) {
        self.stop_asking(|a| *a == Asking::Screenshot);
        self.screenshots.note = Some(
            screenshots::delete(shot)
                .map(|()| trf("{name} moved to the trash", &[("name", &shot.name)])),
        );
        self.screenshots.stale();
    }

    /// How many screenshots the selected instance has.
    pub(super) fn screenshots_count(&mut self) -> usize {
        let Some(home) = self.selected_instance().map(|i| i.home.clone()) else {
            return 0;
        };
        self.load_screenshots(home);
        self.screenshots.list.len()
    }

    /// The viewer over the whole window, while a picture is open.
    pub(super) fn screenshot_viewer(&mut self, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let i = self.viewing_at()?;
        let shot = self.screenshots.list.get(i)?.clone();
        let count = self.screenshots.list.len();
        // Every pixel the window can show; its tile stands in while it is made.
        let (full, thumb) = (Look::Fit(self.window_side()), Look::Fit(self.thumb_side()));
        let picture = self
            .picture(&shot.path, full, cx)
            .or_else(|| self.pictures.made(&shot.path, thumb));
        let taken: chrono::DateTime<chrono::Local> = shot.taken.into();
        let step = |id: &'static str, label: &'static str, older: bool, can: bool| {
            let b = ui::button(
                id,
                None,
                tr(label),
                ButtonKind::Secondary,
                ui::ButtonSize::Md,
            );
            if can {
                b.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.screenshot_step(older);
                    cx.notify();
                }))
            } else {
                ui::disabled(b)
            }
        };
        let mut actions = div().flex().items_center().gap(px(space::SM));
        if self.is_asking(&Asking::Screenshot) {
            let gone = shot.clone();
            actions = actions.child(ui::confirm(
                "screenshot-delete",
                ui::Confirm::Delete(&shot.name),
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.asking = None;
                    cx.notify();
                }),
                cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.delete_screenshot(&gone);
                    cx.notify();
                }),
            ));
        } else {
            let (copy, open, show) = (shot.clone(), shot.path.clone(), shot.path.clone());
            actions = actions
                .child(
                    ui::button(
                        "screenshot-copy",
                        Some("copy-01"),
                        tr("Copy"),
                        ButtonKind::Secondary,
                        ui::ButtonSize::Md,
                    )
                    .on_press(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            this.copy_screenshot(&copy, cx);
                            cx.notify();
                        },
                    )),
                )
                .child(
                    ui::button(
                        "screenshot-open",
                        None,
                        tr("Open"),
                        ButtonKind::Secondary,
                        ui::ButtonSize::Md,
                    )
                    .on_press(move |_, _, cx| crate::toast::open(cx, &open)),
                )
                .child(
                    ui::button(
                        "screenshot-show",
                        Some("folder-01"),
                        tr("Show in folder"),
                        ButtonKind::Secondary,
                        ui::ButtonSize::Md,
                    )
                    .on_press(move |_, _, cx| cx.reveal_path(&show)),
                )
                .child(
                    ui::button(
                        "screenshot-delete",
                        None,
                        tr("Delete…"),
                        ButtonKind::Secondary,
                        ui::ButtonSize::Md,
                    )
                    .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.asking = Some(Asking::Screenshot);
                        cx.notify();
                    })),
                );
        }
        let note = match &self.screenshots.note {
            Some(Ok(m)) => Some(ui::card_note(m.clone())),
            Some(Err(e)) => Some(ui::error_note(e.clone())),
            None => None,
        };
        let top = div()
            .flex()
            .items_center()
            .gap(px(space::MD))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(ui::card_title(
                        taken.format("%Y-%m-%d %H:%M:%S").to_string(),
                    ))
                    .child(
                        ui::card_note(trf(
                            "{name} · {index} of {count}",
                            &[
                                ("name", &shot.name),
                                ("index", &(i + 1).to_string()),
                                ("count", &count.to_string()),
                            ],
                        ))
                        .truncate(),
                    ),
            )
            .child(step("screenshot-newer", "← Newer", false, i > 0))
            .child(step("screenshot-older", "Older →", true, i + 1 < count))
            .child(
                ui::icon_button(
                    "screenshot-close",
                    "cancel-01",
                    tr("Close"),
                    ButtonKind::Ghost,
                    ui::ButtonSize::Md,
                    false,
                )
                .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.screenshot_escape();
                    cx.notify();
                })),
            );
        let bottom = div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(space::MD))
            .child(div().children(note))
            .child(actions);
        Some(
            ui::sheet("screenshot-viewer", shot.name.clone())
                .flex()
                .flex_col()
                .gap(px(space::LG))
                .px(px(space::XXL))
                .py(px(space::XL))
                .child(top)
                .child(
                    // The picture fills what the bars leave, whatever its own size.
                    div()
                        .relative()
                        .flex_1()
                        .min_h_0()
                        .rounded(px(radius::ROW))
                        .overflow_hidden()
                        .bg(ui::inset())
                        .children(picture.map(|image| {
                            gpui::img(image)
                                .absolute()
                                .top_0()
                                .left_0()
                                .size_full()
                                .object_fit(gpui::ObjectFit::Contain)
                        })),
                )
                .child(bottom),
        )
    }
}
