use crate::menus::Asking;
use crate::pictures::Look;
use crate::ui::{white, ButtonKind, Press};
use crate::{home_screenshots_state, ui, Launcher};
use eclipse_launcher::i18n::{tr, trf};
use eclipse_theme::glass::{fill, ink, radius, space};
use gpui::prelude::*;
use gpui::{div, px, AnyElement, ClickEvent, Div, FontWeight, SharedString, Stateful};
use std::path::Path;

impl Launcher {
    /// Home's Screenshots tab: the pictures in a grid, 4 across on wide windows, else 2, each
    /// 16:9, its file name under it. A click opens the viewer.
    pub(super) fn screenshots_tab(&mut self, cx: &mut Context<Self>) -> Option<Div> {
        let home = self.selected_instance()?.home.clone();
        self.load_screenshots(&home, cx);
        let list = self.screenshots.list.to_vec();
        if list.is_empty() {
            return Some(div().child(ui::empty_state(
                "screenshots-empty",
                "image-01",
                tr("No screenshots yet"),
                tr("Press F2 in the game."),
                vec![],
            )));
        }
        let folder = eclipse_launcher::screenshots::folder(&home);
        let privacy = self.shots_privacy(cx);
        let head = ui::list_head(SharedString::default()).child(
            div()
                .flex()
                .items_center()
                .gap(px(space::XS))
                .child(privacy)
                .child(ui::open_folder_icon(
                    "screenshots-folder",
                    tr("Open screenshots folder"),
                    folder,
                    ButtonKind::Ghost,
                    ui::ButtonSize::Md,
                )),
        );
        let shown = if self.screenshots.all {
            list.len()
        } else {
            home_screenshots_state::FIRST
        };
        let more = list.len() > home_screenshots_state::FIRST;
        let pictures: Vec<AnyElement> = list
            .iter()
            .take(shown)
            .map(|s| self.shot_picture(&s.path, cx))
            .collect();
        let mut pictures = pictures.into_iter();
        let grid = div()
            .grid()
            .grid_cols(
                if self.window_width >= crate::layout::SCREENSHOT_GRID_WIDE {
                    4
                } else {
                    2
                },
            )
            .gap_x(px(space::MD))
            .gap_y(px(space::LG))
            .mt(px(space::XS))
            .children(list.into_iter().take(shown).map(|shot| {
                let picture = pictures.next();
                let path = shot.path.clone();
                let tile = div()
                    .min_w_0()
                    .child(
                        ui::picture_slot()
                            .w_full()
                            .aspect_ratio(16. / 9.)
                            .children(picture),
                    )
                    .child(
                        ui::text(
                            eclipse_theme::glass::step::CAPTION,
                            FontWeight::NORMAL,
                            ink::MUTED,
                        )
                        .pt(px(space::SM))
                        .truncate()
                        .child(shot.name.clone()),
                    );
                ui::picture_button(
                    SharedString::from(format!("screenshot-{}", shot.name)),
                    tile,
                    shot.name.clone(),
                )
                .min_w_0()
                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.screenshots.viewing = Some(path.clone());
                    this.stop_asking(|a| *a == Asking::Screenshot);
                    this.screenshots.note = None;
                    cx.notify();
                }))
            }));
        let toggle_all = more.then(|| {
            let label = match self.screenshots.all {
                true => tr("Show fewer").to_string(),
                false => trf(
                    "Show all {count}",
                    &[("count", &self.screenshots.list.len().to_string())],
                ),
            };
            div().mt(px(space::LG)).child(
                ui::button(
                    "screenshots-all",
                    None,
                    label,
                    ButtonKind::Secondary,
                    ui::ButtonSize::Md,
                )
                .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.screenshots.all = !this.screenshots.all;
                    cx.notify();
                })),
            )
        });
        let error = match (&self.screenshots.note, &self.screenshots.viewing) {
            (Some(Err(e)), None) => Some(div().mt(px(space::SM)).child(ui::error_note(e.clone()))),
            _ => None,
        };
        Some(
            div().child(head).child(
                div()
                    .px(px(space::MD))
                    .children(error)
                    .child(grid)
                    .children(toggle_all),
            ),
        )
    }
}

impl Launcher {
    /// The eye button: blurs the screenshots, or shows them again.
    pub(super) fn shots_privacy(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let hidden = self.shot_blur.hidden;
        ui::icon_button(
            "shots-privacy",
            if hidden { "view-off-slash" } else { "view" },
            tr(if hidden {
                "Show screenshots"
            } else {
                "Hide screenshots"
            }),
            ButtonKind::Ghost,
            ui::ButtonSize::Md,
            hidden,
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.shot_blur.hidden = !this.shot_blur.hidden;
            cx.notify();
        }))
    }

    /// Screenshot `path` filling its tile, round at `radius::ROW`, made at the tile's size
    /// (`pictures`): blurred while they are hidden; a plain fill until it is made.
    pub(super) fn shot_picture(&mut self, path: &Path, cx: &mut Context<Self>) -> AnyElement {
        let look = match self.shot_blur.hidden {
            true => Look::Blurred,
            false => Look::Fit(self.thumb_side()),
        };
        match self.picture(path, look, cx) {
            Some(image) => gpui::img(image)
                .size_full()
                .rounded(px(radius::ROW))
                .object_fit(gpui::ObjectFit::Cover)
                .into_any_element(),
            None => div()
                .size_full()
                .rounded(px(radius::ROW))
                .bg(white(fill::HOVER))
                .into_any_element(),
        }
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
                        Some(crate::icons::COPY),
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
                        Some(crate::icons::OPEN_FOLDER),
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
