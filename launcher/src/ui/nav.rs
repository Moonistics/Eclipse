use super::{
    hover_is, hoverable, icon, ink, press_text, tip, tip_on_focus, Hover, IconSize, TipSide,
};
use eclipse_theme::glass::{radius, space};
use gpui::{prelude::*, px, Div, Role, SharedString, Stateful};

const NAV_ROW_LIST: f32 = 32.;

const NAV_ROW_LIST_SHORT: f32 = 28.;

pub(super) const NAV_ROW: f32 = 40.;

pub(super) const NAV_ROW_SHORT: f32 = 32.;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NavDensity {
    Sidebar,
    List,
}

impl NavDensity {
    fn height(self) -> f32 {
        match self {
            NavDensity::Sidebar if super::short_window() => NAV_ROW_SHORT,
            NavDensity::Sidebar => NAV_ROW,
            NavDensity::List if super::short_window() => NAV_ROW_LIST_SHORT,
            NavDensity::List => NAV_ROW_LIST,
        }
    }

    fn gap(self) -> f32 {
        match self {
            NavDensity::Sidebar => space::MD,
            NavDensity::List => space::SM,
        }
    }
}

pub fn nav_row(
    id: impl Into<SharedString>,
    icon_name: &'static str,
    text: impl Into<SharedString>,
    current: bool,
    density: NavDensity,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let text: SharedString = text.into();
    let tone = if current || hover_is(&id) {
        ink::FULL
    } else {
        ink::BODY
    };
    let sidebar = density == NavDensity::Sidebar;
    let row = hoverable(id, radius::ROW, Hover::Row)
        .when(sidebar, |d| d.role(Role::Link))
        .aria_selected(current)
        .aria_label(text.clone())
        .when(sidebar && current, |d| {
            d.aria_description(crate::tr("Current page"))
        })
        .flex()
        .flex_none()
        .items_center()
        .gap(px(density.gap()))
        .h(px(density.height()))
        .px(px(density.gap()))
        .cursor_pointer()
        .text_color(ink(tone))
        .child(icon(icon_name, IconSize::Px16, super::icon_tone(current)).relative())
        .child(
            press_text()
                .relative()
                .min_w_0()
                .truncate()
                .child(text.to_uppercase()),
        );
    super::side_selected(row, current)
}

pub fn nav_text_room(row_width: f32, pad: f32, density: NavDensity) -> f32 {
    row_width - 2. * (pad + super::ROW_EDGE) - IconSize::Px16.px() - density.gap()
}

pub fn tip_when_clipped(
    el: Stateful<Div>,
    full: impl Into<SharedString>,
    width: f32,
    room: f32,
) -> Stateful<Div> {
    if width < room {
        return el;
    }
    let full: SharedString = full.into();
    tip_on_focus(el.tooltip(tip(full.clone())), full, TipSide::Below)
}
