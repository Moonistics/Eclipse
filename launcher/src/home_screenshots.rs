use super::*;
use crate::menus::Asking;
use crate::ui::{self, white};
use eclipse_theme::glass::{fill, ink, radius, space};

impl Launcher {
    /// Home's Screenshots tab: how many, then the pictures in a grid, 4 across on wide windows,
    /// else 2, each 16:9, its file name under it. A click opens the viewer.
    pub(super) fn screenshots_tab(&mut self, cx: &mut Context<Self>) -> Option<Div> {
        let home = self.selected_instance()?.home.clone();
        self.load_screenshots(home.clone());
        let list = self.screenshots.list.clone();
        if list.is_empty() {
            return Some(div().child(ui::empty_state(
                "screenshots-empty",
                "image-01",
                tr("No screenshots yet"),
                tr("Press F2 in the game."),
                vec![],
            )));
        }
        let count = match list.len() {
            1 => tr("1 screenshot").to_string(),
            n => trf("{count} screenshots", &[("count", &n.to_string())]),
        };
        let folder = eclipse_launcher::screenshots::folder(&home);
        let privacy = self.shots_privacy(cx);
        let head = div()
            .flex()
            .items_center()
            .justify_between()
            .h(px(ui::ButtonSize::Sm.height()))
            .child(
                ui::text(
                    eclipse_theme::glass::step::NOTE,
                    FontWeight::NORMAL,
                    ink::MUTED,
                )
                .child(count),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::XS))
                    .child(privacy)
                    .child(
                        ui::icon_button(
                            "screenshots-folder",
                            "folder-01",
                            tr("Open screenshots folder"),
                            ButtonKind::Ghost,
                            ui::ButtonSize::Sm,
                            false,
                        )
                        .on_press(move |_, _, cx| {
                            let _ = std::fs::create_dir_all(&folder);
                            crate::toast::open(cx, &folder);
                        }),
                    ),
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
            .grid_cols(if self.window_width >= crate::layout::PACK_GRID_WIDE {
                4
            } else {
                2
            })
            .gap_x(px(space::MD))
            .gap_y(px(space::LG))
            .mt(px(space::MD))
            .children(list.into_iter().take(shown).map(|shot| {
                let picture = pictures.next();
                let path = shot.path.clone();
                div()
                    .id(SharedString::from(format!("screenshot-{}", shot.name)))
                    .min_w_0()
                    .cursor_pointer()
                    .child(
                        div()
                            .w_full()
                            .aspect_ratio(16. / 9.)
                            .rounded(px(radius::ROW))
                            .overflow_hidden()
                            .bg(white(fill::HOVER))
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
                    )
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
            div()
                .px(px(space::MD))
                .child(head)
                .children(error)
                .child(grid)
                .children(toggle_all),
        )
    }
}
