//! The icons served to `svg()`, and the slots that hold icons downloaded at run time.
use crate::ui::{self, white};
use eclipse_theme::glass::fill;
use gpui::{img, prelude::*, px, Div, ImageSource, ObjectFit, SharedString};
use std::borrow::Cow;

/// The icons, served to `svg()` as `icons/<name>.svg`.
pub struct Assets;

impl gpui::AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        let name = path
            .strip_prefix("icons/")
            .and_then(|p| p.strip_suffix(".svg"));
        Ok(name
            .and_then(eclipse_theme::glass_icons::get)
            .map(Cow::Borrowed))
    }

    fn list(&self, _path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(eclipse_theme::glass_icons::ALL
            .iter()
            .map(|(name, _)| format!("icons/{name}.svg").into())
            .collect())
    }
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
