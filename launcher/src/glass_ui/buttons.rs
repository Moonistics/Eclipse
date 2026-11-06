use super::{
    color, hoverable, icon, look, pixel_text, selected, setting_text, tip, tip_on_focus, white,
    Hover, IconSize, Press,
};
use crate::Kind;
use eclipse_theme::glass::{fill, ink, pixel_text::PRESS, radius, EDGE, ON_LIGHT, PAPER};
use gpui::{
    div, prelude::*, px, AnyElement, App, ClickEvent, Div, FontWeight, SharedString, Stateful,
    Window,
};

pub const SWITCH_WIDTH: f32 = 40.;

pub const SWITCH_HEIGHT: f32 = 24.;

const SWITCH_EDGE: f32 = 1.;

const SWITCH_KNOB: f32 = 18.;

const KNOB_EDGE: f32 = 0.5;

const KNOB_SHINE: f32 = 0.12;

const BEVEL_SHINE: f32 = 0.3;

const BEVEL_SHADE: f32 = 0.22;

const SPLIT_RULE: f32 = 0.2;

const BUSY_TRACK: f32 = 0.35;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Size {
    Sm,
    Md,
    Lg,
    Hero,
}

impl Size {
    pub fn height(self) -> f32 {
        match self {
            Size::Sm => 32.,
            Size::Md => 36.,
            Size::Lg => 44.,
            Size::Hero => 56.,
        }
    }

    fn pad(self) -> f32 {
        match self {
            Size::Sm => 12.,
            Size::Md => 14.,
            Size::Lg => 18.,
            Size::Hero => 24.,
        }
    }
}

pub fn bevel<E: ParentElement>(d: E) -> E {
    shine(d, BEVEL_SHINE)
}

fn shine<E: ParentElement>(d: E, highlight: f32) -> E {
    d.child(
        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .h(px(2.))
            .bg(gpui::white().opacity(highlight)),
    )
    .child(
        div()
            .absolute()
            .bottom_0()
            .left_0()
            .right_0()
            .h(px(3.))
            .bg(gpui::black().opacity(BEVEL_SHADE)),
    )
}

pub fn press_text() -> Div {
    pixel_text(PRESS)
}

pub fn button(
    id: impl Into<SharedString>,
    icon_name: Option<&'static str>,
    label: impl Into<SharedString>,
    kind: Kind,
    size: Size,
) -> Stateful<Div> {
    let label: SharedString = label.into();
    let look = look();
    let tone = match kind {
        Kind::Primary => color(look.on_accent),
        Kind::Secondary => white(ink::FULL),
        Kind::Ghost => white(ink::BODY),
    };
    let hover = match kind {
        Kind::Primary => Hover::Primary,
        Kind::Secondary => Hover::Row,
        Kind::Ghost => Hover::Bare,
    };
    let d = hoverable(id, radius::CONTROL, hover)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap(px(8.))
        .h(px(size.height()))
        .px(px(size.pad()))
        .rounded(px(radius::CONTROL))
        .overflow_hidden()
        .cursor_pointer()
        .text_color(tone)
        .children(icon_name.map(|n| icon(n, IconSize::Px16, tone).relative()))
        .when(!label.is_empty(), |d| {
            d.child(press_text().relative().child(label.to_uppercase()))
        });
    match kind {
        Kind::Primary => bevel(d.bg(color(look.accent)).font_weight(FontWeight::BOLD)),
        Kind::Secondary => d.bg(white(fill::BUTTON)),
        Kind::Ghost => d.hover(|s| s.text_color(white(ink::FULL))),
    }
}

pub fn primary(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    size: Size,
) -> Stateful<Div> {
    button(id, None, label, Kind::Primary, size)
}

pub fn more_button(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    kind: Kind,
    size: Size,
    open: bool,
) -> Stateful<Div> {
    let label: SharedString = label.into();
    let look = look();
    let (tone, hover) = match kind {
        Kind::Primary => (color(look.on_accent), Hover::Primary),
        _ => (white(ink::FULL), Hover::Row),
    };
    let d = hoverable(id, radius::CONTROL, hover)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(size.height()))
        .rounded(px(radius::CONTROL))
        .overflow_hidden()
        .cursor_pointer()
        .when(open, |d| {
            d.child(
                div()
                    .absolute()
                    .size_full()
                    .bg(gpui::black().opacity(fill::SELECTED)),
            )
        })
        .child(icon("more-horizontal", IconSize::Px16, tone).relative())
        .tooltip(tip(label.clone()));
    let d = match kind {
        Kind::Primary => bevel(d.bg(color(look.accent))),
        Kind::Secondary => d.bg(white(fill::BUTTON)),
        Kind::Ghost => d,
    };
    tip_on_focus(d, label)
}

pub fn split(main: Stateful<Div>, more: Stateful<Div>, kind: Kind) -> Div {
    let rule = match kind {
        Kind::Primary => color(look().on_accent).opacity(SPLIT_RULE),
        _ => white(EDGE),
    };
    div()
        .flex()
        .flex_none()
        .child(main.flex_1().rounded_r(px(0.)))
        .child(more.rounded_l(px(0.)).border_l_1().border_color(rule))
}

pub fn primary_busy(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    progress: Option<f32>,
    size: Size,
) -> Stateful<Div> {
    let look = look();
    let label: SharedString = label.into();
    div()
        .id(id.into())
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .h(px(size.height()))
        .px(px(size.pad()))
        .rounded(px(radius::CONTROL))
        .overflow_hidden()
        .bg(color(look.accent).opacity(BUSY_TRACK))
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .h_full()
                .w(gpui::relative(progress.unwrap_or(0.).clamp(0., 1.)))
                .bg(color(look.accent)),
        )
        .child(
            press_text()
                .relative()
                .whitespace_nowrap()
                .font_weight(FontWeight::BOLD)
                .text_color(color(look.on_accent))
                .child(
                    match progress {
                        Some(p) => format!("{label} {:.0}%", p * 100.),
                        None => format!("{label}…"),
                    }
                    .to_uppercase(),
                ),
        )
}

pub fn icon_button(
    id: impl Into<SharedString>,
    name: &'static str,
    label: impl Into<SharedString>,
    size: Size,
    active: bool,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let label: SharedString = label.into();
    let group = SharedString::from(format!("{id}-group"));
    let d = hoverable(id, radius::CONTROL, Hover::Bare)
        .group(group.clone())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(size.height()))
        .rounded(px(radius::CONTROL))
        .cursor_pointer()
        .when(active, |d| d.bg(white(fill::ACTIVE)))
        .child(
            icon(
                name,
                IconSize::Px16,
                white(if active { ink::FULL } else { ink::MUTED }),
            )
            .relative()
            .group_hover(group, |s| s.text_color(white(ink::FULL))),
        )
        .tooltip(tip(label.clone()));
    tip_on_focus(d, label)
}

pub fn toggle(id: impl Into<SharedString>, on: bool) -> Stateful<Div> {
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
            .bg(color(PAPER))
            .when(super::light(), |d| {
                d.border_1()
                    .border_color(color(ON_LIGHT).opacity(KNOB_EDGE))
            }),
        KNOB_SHINE,
    );
    let knob = crate::motion::knob(&id, on, knob, inset, travel);
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
            white(fill::ACTIVE)
        })
        .border(px(SWITCH_EDGE))
        .border_color(if on {
            gpui::transparent_black()
        } else {
            white(ink::DIM)
        })
        .cursor_pointer()
        .child(knob)
}

pub fn toggle_row(
    id: impl Into<SharedString>,
    title: impl IntoElement,
    note: Option<AnyElement>,
    on: bool,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let switch = toggle(format!("{id}-switch"), on);
    hoverable(id, radius::ROW, Hover::Row)
        .flex()
        .items_center()
        .gap(px(40.))
        .p(px(super::SETTING_ROW_PADDING))
        .rounded(px(radius::ROW))
        .cursor_pointer()
        .child(setting_text(title, note).relative())
        .child(switch)
}

pub fn toggle_label(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    on: bool,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let switch = toggle(format!("{id}-switch"), on);
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .gap(px(10.))
        .rounded(px(radius::CONTROL))
        .cursor_pointer()
        .child(switch)
        .child(
            super::text(
                eclipse_theme::glass::step::NOTE,
                FontWeight::NORMAL,
                ink::BODY,
            )
            .child(label.into()),
        )
}

/// A segmented control (the server dialog's difficulty): options on a faint track, the chosen
/// one raised.
pub fn segments<T: Copy + PartialEq + 'static>(
    id: &'static str,
    options: &[(T, &'static str)],
    current: T,
    on_pick: impl Fn(T, &mut Window, &mut App) + Clone + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .gap(px(4.))
        .p(px(4.))
        .rounded(px(radius::ROW))
        .bg(white(fill::WELL))
        .children(options.iter().map(|&(value, label)| {
            let active = current == value;
            let pick = on_pick.clone();
            let segment = selected(
                hoverable(format!("{id}-{label}"), radius::ROW, Hover::Row)
                    .flex()
                    .items_center()
                    .h(px(Size::Sm.height()))
                    .px(px(Size::Sm.pad()))
                    .cursor_pointer()
                    .text_color(white(if active { ink::FULL } else { ink::MUTED }))
                    .child(
                        press_text()
                            .relative()
                            .child(crate::tr(label).to_uppercase()),
                    ),
                active,
            );
            segment.on_press(move |_: &ClickEvent, window, cx| pick(value, window, cx))
        }))
}

pub fn tab(
    id: impl Into<SharedString>,
    text: impl Into<SharedString>,
    open: bool,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let d = div()
        .id(id)
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.))
        .h(px(Size::Md.height()))
        .px(px(TAB_PAD))
        .cursor_pointer()
        .font_family(eclipse_theme::glass::DISPLAY_FONT)
        .text_size(px(PRESS));
    let d = if open {
        d.text_color(white(ink::FULL)).child(
            div()
                .absolute()
                .left(px(TAB_PAD))
                .right(px(TAB_PAD))
                .bottom_0()
                .h(px(2.))
                .bg(color(look().mark)),
        )
    } else {
        d.text_color(white(ink::MUTED))
            .hover(|s| s.text_color(white(ink::FULL)))
    };
    let text: SharedString = text.into();
    d.when(!text.is_empty(), |d| d.child(text.to_uppercase()))
}

pub const TAB_PAD: f32 = 10.;

/// A tab with an icon before its label and a count after it.
pub fn count_tab(
    id: impl Into<SharedString>,
    icon_name: Option<&'static str>,
    label: impl Into<SharedString>,
    count: Option<usize>,
    open: bool,
) -> Stateful<Div> {
    let colour = super::icon_tone(open);
    let label: SharedString = label.into();
    tab(id, "", open)
        .children(icon_name.map(|n| icon(n, IconSize::Px16, colour)))
        .child(label.to_uppercase())
        .children(count.map(|n| div().text_color(white(ink::DIM)).child(n.to_string())))
}

pub fn nav_row(
    id: impl Into<SharedString>,
    icon_name: &'static str,
    text: impl Into<SharedString>,
    current: bool,
) -> Stateful<Div> {
    let text: SharedString = text.into();
    let tone = if current { ink::FULL } else { ink::BODY };
    selected(
        hoverable(id, radius::ROW, Hover::Row)
            .flex()
            .flex_none()
            .items_center()
            .gap(px(12.))
            .h(px(NAV_ROW))
            .px(px(super::SIDEBAR_INSET - super::SIDEBAR_PAD - 1.))
            .cursor_pointer()
            .text_color(white(tone))
            .when(!current, |d| d.hover(|s| s.text_color(white(ink::FULL))))
            .child(icon(icon_name, IconSize::Px16, super::icon_tone(current)).relative())
            .child(press_text().relative().child(text.to_uppercase())),
        current,
    )
}

pub fn link(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Stateful<Div> {
    super::text(
        eclipse_theme::glass::step::BODY,
        FontWeight::MEDIUM,
        ink::FULL,
    )
    .id(id.into())
    .flex_none()
    .text_color(color(look().accent_soft))
    .cursor_pointer()
    .hover(|s| s.text_color(white(ink::FULL)))
    .child(label.into())
}

pub const NAV_ROW: f32 = 40.;

pub const NAV_ROW_SHORT: f32 = 36.;

pub fn dropdown(
    id: impl Into<SharedString>,
    leading: Option<AnyElement>,
    label: impl IntoElement,
    open: bool,
    height: f32,
) -> Stateful<Div> {
    let chevron = icon("arrow-down-01", IconSize::Px16, white(ink::MUTED)).with_transformation(
        gpui::Transformation::rotate(gpui::radians(if open { std::f32::consts::PI } else { 0. })),
    );
    hoverable(id, radius::ROW, Hover::Row)
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.))
        .h(px(height))
        .pl(px(if leading.is_some() { 6. } else { 14. }))
        .pr(px(12.))
        .rounded(px(radius::ROW))
        .bg(super::field_bg(open))
        .border_1()
        .border_color(super::field_edge())
        .cursor_pointer()
        .font_family(eclipse_theme::glass::FONT)
        .text_size(px(eclipse_theme::glass::step::BODY.size))
        .text_color(white(ink::FULL))
        .children(leading.map(|l| div().relative().child(l)))
        .child(div().relative().flex_1().min_w_0().truncate().child(label))
        .child(chevron.relative())
}
