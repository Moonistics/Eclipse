//! Blurring the screenshots on screen: the eye button beside the Screenshots tab's pictures,
//! which draws them as `blur-2xl` would. GPUI has no image filter, so each picture is made once,
//! small and blurred, in the background (`picture_cache`), and drawn stretched. For the session
//! only.
use super::*;
use crate::picture_cache::Look;
use crate::ui::{self, white};
use eclipse_theme::glass::{fill, radius};
use std::path::Path;

#[derive(Default)]
pub(super) struct ShotBlur {
    /// Whether the screenshots are blurred.
    pub(super) hidden: bool,
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
    /// (`picture_cache`): blurred while they are hidden; a plain fill until it is made.
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
}
