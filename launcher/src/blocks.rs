//! The icon sets served to `svg()`, the pixel icons, and the slots that hold icons downloaded
//! at run time.
use crate::glass_ui::{self as ui, white};
use eclipse_theme::glass::fill;
use gpui::{img, prelude::*, px, svg, Div, Hsla, ImageSource, ObjectFit, SharedString, Svg};
use std::borrow::Cow;

/// The icons, served to `svg()` as `icons/<name>.svg`.
pub struct Assets;

impl gpui::AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        if let Some(name) = path
            .strip_prefix("icons/glass/")
            .and_then(|p| p.strip_suffix(".svg"))
        {
            return Ok(eclipse_theme::glass_icons::get(name).map(Cow::Borrowed));
        }
        let name = path
            .strip_prefix("icons/")
            .and_then(|p| p.strip_suffix(".svg"));
        Ok(name.and_then(eclipse_theme::icons::get).map(Cow::Borrowed))
    }

    fn list(&self, _path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(eclipse_theme::icons::ALL
            .iter()
            .map(|(name, _)| format!("icons/{name}.svg").into())
            .chain(
                eclipse_theme::glass_icons::ALL
                    .iter()
                    .map(|(name, _)| format!("icons/glass/{name}.svg").into()),
            )
            .collect())
    }
}

/// A pixel icon (`eclipse_theme::icons`) in `color`.
pub fn icon(name: &'static str, size: f32, color: Hsla) -> Svg {
    svg()
        .path(format!("icons/{name}.svg"))
        .flex_none()
        .size(px(ui::icon_grid(size)))
        .text_color(color)
}

/// A square slot for an icon: the picture at `source` when there is one, else the first letter of
/// `name` on a tile.
pub fn slot(source: Option<ImageSource>, name: &str, size: f32) -> Div {
    match source {
        Some(source) => {
            ui::tile(size).child(img(source).size(px(size)).object_fit(ObjectFit::Cover))
        }
        None => ui::letter_tile(white(fill::BUTTON), name, size),
    }
}
