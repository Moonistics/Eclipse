use super::{on_pixel_grid, white};
use eclipse_theme::glass::{fill, ink, radius, EDGE};
use gpui::{div, img, prelude::*, px, AnyElement, Div, FontWeight, Hsla, ImageSource};

const LETTER_SHARE: f32 = 0.5;

pub fn tile(size: f32) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(size))
        .rounded(px(radius::ROW))
        .overflow_hidden()
        .bg(white(fill::BUTTON))
        .border_1()
        .border_color(white(EDGE))
}

pub fn image_tile(image: impl Into<ImageSource>, size: f32) -> AnyElement {
    img(image)
        .flex_none()
        .size(px(size))
        .rounded(px(radius::ROW))
        .into_any_element()
}

pub fn letter_tile(colour: Hsla, name: &str, size: f32) -> Div {
    let letter = on_pixel_grid(size * LETTER_SHARE);
    let tone = if colour.a < 1. {
        white(ink::BODY)
    } else {
        gpui::white()
    };
    tile(size)
        .bg(colour)
        .font_family(eclipse_theme::glass::DISPLAY_FONT)
        .font_weight(FontWeight::BOLD)
        .text_size(px(letter))
        .line_height(px(letter))
        .text_color(tone)
        .child(crate::instance_state::initial(name))
}
