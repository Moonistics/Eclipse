use super::{
    color, disabled, hover_is, hoverable, icon_button, ink, inset, look, setting_row_frame,
    setting_text, shine, ButtonKind, ButtonSize, Hover, Press,
};
use eclipse_theme::glass::{fill, radius, space, step, ON_LIGHT, PAPER, SWITCH_EDGE_LIGHT};
use gpui::{
    div, prelude::*, px, App, Div, FontWeight, Role, SharedString, Stateful, Toggled, Window,
};

const SWITCH_WIDTH: f32 = 40.;

const SWITCH_HEIGHT: f32 = 24.;

const SWITCH_EDGE: f32 = 1.;

const SWITCH_KNOB: f32 = 18.;

pub(super) const KNOB_EDGE: f32 = 0.5;

const KNOB_SHINE: f32 = 0.12;

const STEPPER_VALUE_WIDTH: f32 = 48.;

pub fn stepper(
    id: impl Into<SharedString>,
    value: i64,
    range: std::ops::RangeInclusive<i64>,
    on_step: impl Fn(i64, &mut Window, &mut App) + Clone + 'static,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let side_button = |side: &str, icon_name: &'static str, to: i64| {
        let label = crate::tr(if to < value { "Decrease" } else { "Increase" });
        let b = icon_button(
            format!("{id}-{side}"),
            icon_name,
            label,
            ButtonKind::Ghost,
            ButtonSize::Md,
            false,
        );
        if range.contains(&to) && to != value {
            let on_step = on_step.clone();
            b.on_press(move |_, window, cx| on_step(to, window, cx))
        } else {
            disabled(b)
        }
    };
    let (less, more) = (
        side_button("less", "minus-sign", value - 1),
        side_button("more", crate::icons::ADD, value + 1),
    );
    div()
        .id(id.clone())
        .role(Role::SpinButton)
        .aria_numeric_value(value as f64)
        .aria_min_numeric_value(*range.start() as f64)
        .aria_max_numeric_value(*range.end() as f64)
        .flex()
        .items_center()
        .gap(px(space::XS))
        .p(px(space::XS))
        .rounded(px(radius::ROW))
        .bg(inset())
        .child(less)
        .child(
            super::text(step::LEAD, FontWeight::SEMIBOLD, ink::FULL)
                .w(px(STEPPER_VALUE_WIDTH))
                .flex()
                .justify_center()
                .child(value.to_string()),
        )
        .child(more)
}

pub fn toggle(id: impl Into<SharedString>, on: bool) -> Stateful<Div> {
    switch_face(id, on)
        .role(Role::Switch)
        .aria_toggled(Toggled::from(on))
}

fn switch_face(id: impl Into<SharedString>, on: bool) -> Stateful<Div> {
    let id: SharedString = id.into();
    let inset = (SWITCH_HEIGHT - 2. * SWITCH_EDGE - SWITCH_KNOB) / 2.;
    let travel = SWITCH_WIDTH - 2. * (SWITCH_EDGE + inset) - SWITCH_KNOB;
    let knob = shine(
        div()
            .absolute()
            .top(px(inset))
            .size(px(SWITCH_KNOB))
            .rounded(px(radius::BLOCK / 2.))
            .overflow_hidden()
            .bg(if on || super::light() {
                color(PAPER)
            } else {
                ink(ink::MUTED)
            })
            .when(super::light(), |d| {
                d.border_1()
                    .border_color(color(ON_LIGHT).opacity(KNOB_EDGE))
            }),
        KNOB_SHINE,
    );
    let knob = crate::motion::knob(&id, on, knob, inset, travel);
    let rim = match (on, super::light()) {
        (true, true) => color(ON_LIGHT).opacity(SWITCH_EDGE_LIGHT),
        (true, false) => gpui::transparent_black(),
        (false, _) => ink(ink::DIM),
    };
    div()
        .id(id)
        .relative()
        .flex_none()
        .w(px(SWITCH_WIDTH))
        .h(px(SWITCH_HEIGHT))
        .rounded(px(radius::BLOCK))
        .bg(if on {
            color(look().accent)
        } else {
            ink(fill::ACTIVE)
        })
        .border(px(SWITCH_EDGE))
        .border_color(rim)
        .cursor_pointer()
        .child(knob)
}

pub fn toggle_row(
    id: impl Into<SharedString>,
    title: impl Into<SharedString>,
    note: Option<SharedString>,
    on: bool,
) -> Stateful<Div> {
    let (id, title): (SharedString, SharedString) = (id.into(), title.into());
    let switch = switch_face(format!("{id}-switch"), on);
    let described = note.clone();
    let note = note.map(|n| div().child(n).into_any_element());
    setting_row_frame(hoverable(id, radius::ROW, Hover::Row))
        .role(Role::Switch)
        .aria_toggled(Toggled::from(on))
        .aria_label(title.clone())
        .when_some(described, |d, note| d.aria_description(note))
        .cursor_pointer()
        .child(setting_text(title, note).relative())
        .child(switch)
}

pub fn toggle_row_inset(
    id: impl Into<SharedString>,
    title: impl Into<SharedString>,
    note: Option<SharedString>,
    on: bool,
) -> Stateful<Div> {
    toggle_row(id, title, note, on).bg(inset())
}

pub fn toggle_label(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    on: bool,
) -> Stateful<Div> {
    let (id, label): (SharedString, SharedString) = (id.into(), label.into());
    let switch = switch_face(format!("{id}-switch"), on);
    let tone = if hover_is(&id) { ink::FULL } else { ink::BODY };
    super::track_hover(div().id(id.clone()), id)
        .role(Role::Switch)
        .aria_toggled(Toggled::from(on))
        .aria_label(label.clone())
        .flex()
        .flex_none()
        .items_center()
        .gap(px(space::MD))
        .rounded(px(radius::CONTROL))
        .cursor_pointer()
        .child(switch)
        .child(super::text(step::NOTE, FontWeight::NORMAL, tone).child(label))
}
