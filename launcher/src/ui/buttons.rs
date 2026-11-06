use super::{
    color, disabled, gap_ring, hover_is, hoverable, icon, inset, look, pixel_text, selected,
    setting_text, tip, tip_on_focus, white, Hover, IconSize, Press, LIST_INSET, PANEL_INSET,
};
use eclipse_theme::glass::{
    fill, ink, pixel_text::PRESS, radius, space, step, EDGE, ON_LIGHT, PAPER,
};
use gpui::{
    div, prelude::*, px, AnyElement, App, ClickEvent, ClipboardItem, Div, FontWeight, SharedString,
    Stateful, Window,
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

const LINK_MIN_HEIGHT: f32 = 24.;

const STEPPER_VALUE_WIDTH: f32 = 48.;

#[derive(Clone, Copy, PartialEq)]
pub enum ButtonKind {
    Primary,
    Secondary,
    Ghost,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonSize {
    Sm,
    Md,
    Lg,
    Hero,
}

impl ButtonSize {
    pub fn height(self) -> f32 {
        match self {
            ButtonSize::Sm => 32.,
            ButtonSize::Md => 36.,
            ButtonSize::Lg => 44.,
            ButtonSize::Hero => 56.,
        }
    }

    fn pad(self) -> f32 {
        match self {
            ButtonSize::Sm => space::MD,
            ButtonSize::Md => 14.,
            ButtonSize::Lg => 18.,
            ButtonSize::Hero => space::XL,
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

pub fn press_width(label: &str) -> f32 {
    let advance =
        eclipse_theme::glass::MONOCRAFT_ADVANCE * PRESS / eclipse_theme::glass::MONOCRAFT_PIXEL;
    label.chars().count() as f32 * advance
}

pub fn button(
    id: impl Into<SharedString>,
    icon_name: Option<&'static str>,
    label: impl Into<SharedString>,
    kind: ButtonKind,
    size: ButtonSize,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let label: SharedString = label.into();
    let look = look();
    let tone = match kind {
        ButtonKind::Primary => color(look.on_accent),
        ButtonKind::Secondary => white(ink::FULL),
        ButtonKind::Ghost if hover_is(&id) => white(ink::FULL),
        ButtonKind::Ghost => white(ink::BODY),
    };
    let hover = match kind {
        ButtonKind::Primary => Hover::Primary,
        ButtonKind::Secondary => Hover::Row,
        ButtonKind::Ghost => Hover::Bare,
    };
    let d = hoverable(id, radius::CONTROL, hover)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap(px(space::SM))
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
        ButtonKind::Primary => bevel(d.bg(color(look.accent)).font_weight(FontWeight::BOLD)),
        ButtonKind::Secondary => d.bg(white(fill::BUTTON)),
        ButtonKind::Ghost => d,
    }
}

pub fn primary(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    size: ButtonSize,
) -> Stateful<Div> {
    button(id, None, label, ButtonKind::Primary, size)
}

pub fn icon_button(
    id: impl Into<SharedString>,
    name: &'static str,
    label: impl Into<SharedString>,
    kind: ButtonKind,
    size: ButtonSize,
    active: bool,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let label: SharedString = label.into();
    let look = look();
    let tone = match kind {
        ButtonKind::Primary => color(look.on_accent),
        ButtonKind::Secondary => white(ink::FULL),
        ButtonKind::Ghost if active || hover_is(&id) => white(ink::FULL),
        ButtonKind::Ghost => white(ink::MUTED),
    };
    let hover = match kind {
        ButtonKind::Primary => Hover::Primary,
        ButtonKind::Secondary => Hover::Row,
        ButtonKind::Ghost => Hover::Bare,
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
        .when(active, |d| {
            d.child(div().absolute().size_full().bg(white(fill::ACTIVE)))
        })
        .child(icon(name, IconSize::Px16, tone).relative())
        .tooltip(tip(label.clone()));
    let d = match kind {
        ButtonKind::Primary => bevel(d.bg(color(look.accent))),
        ButtonKind::Secondary => d.bg(white(fill::BUTTON)),
        ButtonKind::Ghost => d,
    };
    tip_on_focus(d, label)
}

pub fn more_button(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    kind: ButtonKind,
    size: ButtonSize,
    open: bool,
) -> Stateful<Div> {
    icon_button(id, "more-horizontal", label, kind, size, open)
}

pub fn split(main: Stateful<Div>, more: Stateful<Div>, kind: ButtonKind) -> Div {
    let rule = match kind {
        ButtonKind::Primary => color(look().on_accent).opacity(SPLIT_RULE),
        _ => white(EDGE),
    };
    div()
        .flex()
        .flex_none()
        .child(main.flex_1().rounded_r(px(0.)).focus_visible(gap_ring))
        .child(
            more.rounded_l(px(0.))
                .border_l_1()
                .border_color(rule)
                .focus_visible(gap_ring),
        )
}

pub fn confirm(
    id: impl Into<SharedString>,
    question: impl Into<SharedString>,
    verb: impl Into<SharedString>,
    cancel: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    sure: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    let id: SharedString = id.into();
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .justify_end()
        .gap(px(space::SM))
        .child(
            super::text(step::NOTE, FontWeight::MEDIUM, ink::FULL)
                .flex_grow_1()
                .text_color(super::danger())
                .child(question.into()),
        )
        .child(
            div()
                .flex()
                .gap(px(space::SM))
                .child(
                    button(
                        id.clone(),
                        None,
                        crate::tr("Cancel"),
                        ButtonKind::Ghost,
                        ButtonSize::Sm,
                    )
                    .on_press(cancel),
                )
                .child(
                    button(
                        format!("{id}-confirm"),
                        None,
                        verb,
                        ButtonKind::Secondary,
                        ButtonSize::Sm,
                    )
                    .text_color(super::danger())
                    .on_press(sure),
                ),
        )
}

pub fn menu_confirm(
    id: impl Into<SharedString>,
    question: impl Into<SharedString>,
    verb: impl Into<SharedString>,
    cancel: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    sure: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    confirm(id, question, verb, cancel, sure)
        .mt(px(space::XS))
        .px(px(PANEL_INSET))
        .py(px(LIST_INSET))
}

pub fn primary_busy(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    progress: Option<f32>,
    size: ButtonSize,
) -> Stateful<Div> {
    let look = look();
    let label: SharedString = label.into();
    bevel(
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
            ),
    )
}

pub fn copy_details(
    id: impl Into<SharedString>,
    details: String,
    size: ButtonSize,
) -> Stateful<Div> {
    button(
        id,
        Some("copy-01"),
        crate::tr("Copy details"),
        ButtonKind::Secondary,
        size,
    )
    .on_press(move |_, _, cx| {
        crate::clipboard::copy_to_clipboard(cx, ClipboardItem::new_string(details.clone()))
    })
}

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
        side_button("more", "plus-sign", value + 1),
    );
    div()
        .id(id.clone())
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
    let look = look();
    let track = if super::light() {
        look.mark
    } else {
        look.accent
    };
    div()
        .id(id)
        .relative()
        .flex_none()
        .w(px(SWITCH_WIDTH))
        .h(px(SWITCH_HEIGHT))
        .rounded(px(radius::BLOCK))
        .bg(if on {
            color(track)
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
        .gap(px(SETTING_ROW_GAP))
        .p(px(super::SETTING_ROW_PADDING))
        .rounded(px(radius::ROW))
        .cursor_pointer()
        .child(setting_text(title, note).relative())
        .child(switch)
}

pub const SETTING_ROW_GAP: f32 = 40.;

pub fn toggle_label(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    on: bool,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let switch = toggle(format!("{id}-switch"), on);
    let tone = if hover_is(&id) { ink::FULL } else { ink::BODY };
    super::track_hover(div().id(id.clone()), id)
        .flex()
        .flex_none()
        .items_center()
        .gap(px(space::MD))
        .rounded(px(radius::CONTROL))
        .cursor_pointer()
        .child(switch)
        .child(super::text(step::NOTE, FontWeight::NORMAL, tone).child(label.into()))
}

/// A segmented control (the server dialog's difficulty): options on a faint track, the chosen
/// one raised.
pub fn segments<T: Copy + PartialEq + std::fmt::Debug + 'static>(
    id: impl Into<SharedString>,
    options: &[(T, SharedString)],
    current: T,
    on_pick: impl Fn(T, &mut Window, &mut App) + Clone + 'static,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    div()
        .id(id.clone())
        .flex()
        .flex_none()
        .gap(px(space::XS))
        .p(px(space::XS))
        .rounded(px(radius::ROW))
        .bg(inset())
        .children(options.iter().map(|(value, label)| {
            let (value, active) = (*value, current == *value);
            let pick = on_pick.clone();
            let segment = selected(
                hoverable(format!("{id}-{value:?}"), radius::CONTROL, Hover::Row)
                    .flex()
                    .items_center()
                    .h(px(ButtonSize::Sm.height()))
                    .px(px(ButtonSize::Sm.pad()))
                    .cursor_pointer()
                    .text_color(white(if active { ink::FULL } else { ink::MUTED }))
                    .child(press_text().relative().child(label.to_uppercase())),
                active,
            )
            .rounded(px(radius::CONTROL));
            segment.on_press(move |_: &ClickEvent, window, cx| pick(value, window, cx))
        }))
}

pub fn tab(
    id: impl Into<SharedString>,
    text: impl Into<SharedString>,
    open: bool,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let tone = if open || hover_is(&id) {
        ink::FULL
    } else {
        ink::MUTED
    };
    let d = super::track_hover(div().id(id.clone()), id)
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(space::SM))
        .h(px(ButtonSize::Md.height()))
        .px(px(TAB_PAD))
        .rounded(px(radius::CONTROL))
        .cursor_pointer()
        .text_color(white(tone))
        .when(open, |d| {
            d.child(
                div()
                    .absolute()
                    .left(px(TAB_PAD))
                    .right(px(TAB_PAD))
                    .bottom_0()
                    .h(px(2.))
                    .bg(color(look().mark)),
            )
        });
    let text: SharedString = text.into();
    d.when(!text.is_empty(), |d| {
        d.child(press_text().child(text.to_uppercase()))
    })
}

pub const TAB_PAD: f32 = 10.;

pub const TAB_STRIP_GAP: f32 = space::LG;

pub fn tab_strip(
    id: impl Into<SharedString>,
    tabs: impl IntoIterator<Item = Stateful<Div>>,
) -> Stateful<Div> {
    div()
        .id(id.into())
        .flex()
        .flex_wrap()
        .items_center()
        .mx(px(-TAB_PAD))
        .mb(px(TAB_STRIP_GAP))
        .children(tabs)
}

/// A tab with a count after its label.
pub fn count_tab(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    count: Option<usize>,
    open: bool,
) -> Stateful<Div> {
    let label: SharedString = label.into();
    tab(id, label, open).children(count.map(|n| {
        press_text()
            .text_color(white(ink::DIM))
            .child(n.to_string())
    }))
}

pub fn nav_row(
    id: impl Into<SharedString>,
    icon_name: &'static str,
    text: impl Into<SharedString>,
    current: bool,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let text: SharedString = text.into();
    let tone = if current || hover_is(&id) {
        ink::FULL
    } else {
        ink::BODY
    };
    selected(
        hoverable(id, radius::ROW, Hover::Row)
            .flex()
            .flex_none()
            .items_center()
            .gap(px(space::MD))
            .h(px(NAV_ROW))
            .px(px(space::MD))
            .cursor_pointer()
            .text_color(white(tone))
            .child(icon(icon_name, IconSize::Px16, super::icon_tone(current)).relative())
            .child(
                press_text()
                    .relative()
                    .min_w_0()
                    .truncate()
                    .child(text.to_uppercase()),
            ),
        current,
    )
}

pub fn link(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Stateful<Div> {
    let id: SharedString = id.into();
    let tone = if hover_is(&id) {
        white(ink::FULL)
    } else {
        color(look().accent_soft)
    };
    super::track_hover(
        super::text(step::BODY, FontWeight::MEDIUM, ink::FULL).id(id.clone()),
        id,
    )
    .flex()
    .flex_none()
    .items_center()
    .min_h(px(LINK_MIN_HEIGHT))
    .text_color(tone)
    .cursor_pointer()
    .child(label.into())
}

pub const NAV_ROW: f32 = 40.;

pub const NAV_ROW_SHORT: f32 = 36.;

pub fn dropdown(
    id: impl Into<SharedString>,
    leading: Option<AnyElement>,
    label: impl IntoElement,
    open: bool,
    size: ButtonSize,
) -> Stateful<Div> {
    let chevron = icon("arrow-down-01", IconSize::Px16, white(ink::MUTED)).with_transformation(
        gpui::Transformation::rotate(gpui::radians(if open { std::f32::consts::PI } else { 0. })),
    );
    hoverable(id, radius::ROW, Hover::Row)
        .flex()
        .flex_none()
        .items_center()
        .gap(px(space::SM))
        .h(px(size.height()))
        .pl(px(if leading.is_some() { space::SM } else { 14. }))
        .pr(px(space::MD))
        .rounded(px(radius::ROW))
        .bg(super::field_bg(open))
        .border_1()
        .border_color(super::field_edge())
        .cursor_pointer()
        .font_family(eclipse_theme::glass::FONT)
        .text_size(px(step::BODY.size))
        .text_color(white(ink::FULL))
        .children(leading.map(|l| div().relative().child(l)))
        .child(div().relative().flex_1().min_w_0().truncate().child(label))
        .child(chevron.relative())
}

pub fn translated<T: Copy>(options: &[(T, &str)]) -> Vec<(T, SharedString)> {
    options
        .iter()
        .map(|&(value, label)| (value, SharedString::from(crate::tr(label).to_string())))
        .collect()
}
