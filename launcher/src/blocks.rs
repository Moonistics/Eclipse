//! The icon sets served to `svg()`, the pixel icons, badges, and the slots that hold icons
//! downloaded at run time.
use crate::glass_ui::{self as ui, white};
use eclipse_theme::glass::{fill, ink, radius};
use gpui::{
    div, img, prelude::*, px, svg, Div, FontWeight, Hsla, ImageSource, ObjectFit, SharedString, Svg,
};
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

/// A small label in a tint of its colour: "Runs", "Partly", a version.
pub fn badge(label: impl Into<SharedString>, color: Hsla) -> Div {
    let label: SharedString = label.into();
    let label = SharedString::from(crate::i18n::tr(&label).to_string());
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(6.))
        .h(px(22.))
        .px(px(8.))
        .rounded(px(radius::CONTROL))
        .bg(color.opacity(fill::ACTIVE))
        .child(div().size(px(6.)).rounded_full().bg(color))
        .child(
            ui::label_text(12.)
                .font_weight(FontWeight::MEDIUM)
                .text_color(color)
                .child(label),
        )
}

/// A square slot for an icon: the picture at `source` when there is one, else the first letter of
/// `name` on a tile.
pub fn slot(source: Option<ImageSource>, name: &str, size: f32) -> Div {
    let frame = div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(size))
        .overflow_hidden()
        .rounded(px(radius::ROW))
        .bg(white(fill::FIELD));
    match source {
        Some(source) => frame.child(img(source).size(px(size)).object_fit(ObjectFit::Cover)),
        None => frame.bg(white(fill::BUTTON)).child(
            ui::text((size * 0.42).round(), FontWeight::BOLD, ink::MUTED).child(
                name.chars()
                    .find(|c| c.is_alphanumeric())
                    .unwrap_or('?')
                    .to_uppercase()
                    .to_string(),
            ),
        ),
    }
}
