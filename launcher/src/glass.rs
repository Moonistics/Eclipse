use crate::glass_ui::{light, look};
use eclipse_theme::glass::{EDGE, FLOAT};
use gpui::{div, prelude::*, px, rgb, AnyElement, Div, Hsla};

pub const CONTENT_INSET: f32 = 14.;

const LIGHT_GLASS: f32 = 0.84;
const LIGHT_FLOAT: f32 = 0.94;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Kind {
    Glass,
    Float,
}

pub fn colour(kind: Kind) -> Hsla {
    if light() {
        let a = if kind == Kind::Float {
            LIGHT_FLOAT
        } else {
            LIGHT_GLASS
        };
        return gpui::white().opacity(a);
    }
    match kind {
        Kind::Glass => {
            let look = look();
            Hsla::from(rgb(look.plate)).opacity(look.plate_alpha)
        }
        Kind::Float => rgb(FLOAT).into(),
    }
}

fn edge() -> Hsla {
    if light() {
        gpui::black().opacity(0.08)
    } else {
        gpui::white().opacity(EDGE)
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
        .border_color(edge())
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
