use crate::glass_ui::{light, look, white};
use eclipse_theme::glass::{EDGE, FLOAT, PAPER};
use gpui::{div, prelude::*, px, rgb, AnyElement, Div, Hsla};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Kind {
    Glass,
    Float,
}

pub fn colour(kind: Kind) -> Hsla {
    if light() {
        return rgb(PAPER).into();
    }
    match kind {
        Kind::Glass => rgb(look().plate).into(),
        Kind::Float => rgb(FLOAT).into(),
    }
}

pub fn backdrop(kind: Kind, radius: f32) -> AnyElement {
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .rounded(px(radius))
        .bg(colour(kind))
        .border_1()
        .border_color(white(EDGE))
        .into_any_element()
}

pub fn plate(radius: f32) -> Div {
    div()
        .relative()
        .rounded(px(radius))
        .child(backdrop(Kind::Glass, radius))
}

pub fn float(radius: f32) -> Div {
    div()
        .relative()
        .rounded(px(radius))
        .child(backdrop(Kind::Float, radius))
}
