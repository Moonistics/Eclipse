use super::{edge, hover_over, on_pixel_grid, tip, tip_beside_on_focus, white, TILE_LG};
use eclipse_theme::glass::{fill, ink, radius, step};
use gpui::{
    div, img, prelude::*, px, AnyElement, Div, Hsla, ImageSource, Role, SharedString, Stateful,
};

const LETTER_SHARE: f32 = 0.5;

pub const MARK: f32 = 32.;

pub const FACE_XL: f32 = 80.;

const BULLET: f32 = 5.;

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
        .border_color(edge())
}

pub fn mark_tile(mark: impl IntoElement) -> Div {
    tile(TILE_LG).relative().child(mark)
}

pub fn face(picture: impl IntoElement, size: f32) -> Div {
    div()
        .relative()
        .flex_none()
        .size(px(size))
        .rounded(px(radius::ROW))
        .overflow_hidden()
        .child(picture)
}

pub fn picture_button(
    id: impl Into<SharedString>,
    picture: impl IntoElement,
    label: impl Into<SharedString>,
) -> Stateful<Div> {
    let label: SharedString = label.into();
    let el = hover_over(id, radius::ROW, picture)
        .role(Role::Button)
        .aria_label(label.clone())
        .flex_none()
        .rounded(px(radius::ROW))
        .cursor_pointer()
        .tooltip(tip(label.clone()));
    tip_beside_on_focus(el, label)
}

pub fn bullet() -> Div {
    div()
        .flex_none()
        .mt(px((step::BODY.line - BULLET) / 2.))
        .size(px(BULLET))
        .bg(white(ink::DIM))
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
        .text_size(px(letter))
        .line_height(px(letter))
        .text_color(tone)
        .child(initial(name))
}

pub fn initial(name: &str) -> String {
    name.chars()
        .find(|c| c.is_alphanumeric())
        .unwrap_or('?')
        .to_uppercase()
        .to_string()
}
