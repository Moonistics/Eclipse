use super::{
    chosen, color, disabled, edge, gap_ring, hover_is, hoverable, icon, inset, look, pixel_text,
    selected, setting_text, tip, tip_on_focus, white, Hover, IconSize, Press, LIST_INSET,
    PANEL_INSET,
};
use eclipse_theme::glass::{
    fill, ink,
    pixel_text::{PAGE_TITLE, PRESS},
    radius, space, step, ON_LIGHT, PAPER, SWITCH_EDGE_LIGHT,
};
use gpui::{
    div, prelude::*, px, AnyElement, App, ClickEvent, ClipboardItem, Div, FontWeight, Hsla, Role,
    SharedString, Stateful, Toggled, Window,
};

pub const SWITCH_WIDTH: f32 = 40.;

pub const SWITCH_HEIGHT: f32 = 24.;

const SWITCH_EDGE: f32 = 1.;

const SWITCH_KNOB: f32 = 18.;

const KNOB_EDGE: f32 = 0.5;

const KNOB_SHINE: f32 = 0.12;

const BEVEL_SHINE: f32 = 0.3;

const BEVEL_SHADE: f32 = 0.22;

const BEVEL_SHINE_HEIGHT: f32 = 2.;

const BEVEL_SHADE_HEIGHT: f32 = 3.;

const SPLIT_RULE: f32 = 0.2;

const LINK_MIN_HEIGHT: f32 = 24.;

const TAB_UNDERLINE: f32 = 4.;

const NAV_ROW_LIST: f32 = 32.;

const NAV_ROW_LIST_SHORT: f32 = 28.;

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

    pub fn field_pad(self) -> f32 {
        match self {
            ButtonSize::Lg | ButtonSize::Hero => space::LG,
            ButtonSize::Sm | ButtonSize::Md => space::MD,
        }
    }

    pub fn field_text(self) -> eclipse_theme::glass::TextStep {
        match self {
            ButtonSize::Lg | ButtonSize::Hero => step::LEAD,
            ButtonSize::Sm | ButtonSize::Md => step::BODY,
        }
    }

    fn pad(self) -> f32 {
        match self {
            ButtonSize::Sm => space::MD,
            ButtonSize::Md => space::MD + space::XXS,
            ButtonSize::Lg => space::LG + space::XXS,
            ButtonSize::Hero => space::XL,
        }
    }

    fn label(self) -> Div {
        match self {
            ButtonSize::Hero => pixel_text(PAGE_TITLE),
            _ => press_text(),
        }
    }

    fn icon(self) -> IconSize {
        match self {
            ButtonSize::Hero => IconSize::Px24,
            _ => IconSize::Px16,
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
            .h(px(BEVEL_SHINE_HEIGHT))
            .bg(gpui::white().opacity(highlight)),
    )
    .child(
        div()
            .absolute()
            .bottom_0()
            .left_0()
            .right_0()
            .h(px(BEVEL_SHADE_HEIGHT))
            .bg(gpui::black().opacity(BEVEL_SHADE)),
    )
}

pub fn press_text() -> Div {
    pixel_text(PRESS)
}

pub fn shouts(kind: ButtonKind, size: ButtonSize) -> bool {
    kind == ButtonKind::Primary || size == ButtonSize::Hero
}

pub fn button_width(label: &str, icon: bool, kind: ButtonKind, size: ButtonSize) -> f32 {
    let shown = if shouts(kind, size) {
        label.to_uppercase()
    } else {
        label.to_string()
    };
    let icon = if icon {
        size.icon().px() + space::SM
    } else {
        0.
    };
    press_width(&shown) + icon + 2. * size.pad()
}

fn hover_of(kind: ButtonKind) -> Hover {
    match kind {
        ButtonKind::Primary => Hover::Primary,
        ButtonKind::Secondary => Hover::Row,
        ButtonKind::Ghost => Hover::Bare,
    }
}

pub fn press_width(label: &str) -> f32 {
    let advance = eclipse_theme::glass::MONOCRAFT_ADVANCE * super::on_pixel_grid(PRESS)
        / eclipse_theme::glass::MONOCRAFT_PIXEL;
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
    let look = look();
    let tone = match kind {
        ButtonKind::Primary => color(look.on_accent),
        _ if hover_is(&id) => white(ink::FULL),
        _ => white(ink::BODY),
    };
    let d = button_face(
        hoverable(id, radius::CONTROL, hover_of(kind)),
        icon_name,
        label,
        size,
        tone,
        shouts(kind, size),
    );
    match kind {
        ButtonKind::Primary => bevel(d.bg(color(look.accent))),
        ButtonKind::Secondary => secondary(d),
        ButtonKind::Ghost => d,
    }
}

fn button_face(
    d: Stateful<Div>,
    icon_name: Option<&'static str>,
    label: impl Into<SharedString>,
    size: ButtonSize,
    tone: Hsla,
    caps: bool,
) -> Stateful<Div> {
    let label: SharedString = label.into();
    let shown: SharedString = if caps {
        label.to_uppercase().into()
    } else {
        label.clone()
    };
    d.role(Role::Button)
        .when(!label.is_empty(), |d| d.aria_label(label.clone()))
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
        .children(icon_name.map(|n| icon(n, size.icon(), tone).relative()))
        .when(!label.is_empty(), |d| {
            d.child(size.label().relative().child(shown))
        })
}

pub(super) fn secondary_fill() -> Hsla {
    if super::light() {
        color(PAPER).opacity(fill::CONTROL_LIGHT)
    } else {
        white(fill::BUTTON)
    }
}

fn secondary(d: Stateful<Div>) -> Stateful<Div> {
    d.bg(secondary_fill())
        .when(super::light(), |d| d.border_1().border_color(edge()))
}

pub fn primary(
    id: impl Into<SharedString>,
    icon_name: Option<&'static str>,
    label: impl Into<SharedString>,
    size: ButtonSize,
) -> Stateful<Div> {
    button(id, icon_name, label, ButtonKind::Primary, size)
}

pub fn icon_button(
    id: impl Into<SharedString>,
    name: &'static str,
    label: impl Into<SharedString>,
    kind: ButtonKind,
    size: ButtonSize,
    active: bool,
) -> Stateful<Div> {
    icon_button_tipped(id, name, label, kind, size, active, true)
}

fn icon_button_tipped(
    id: impl Into<SharedString>,
    name: &'static str,
    label: impl Into<SharedString>,
    kind: ButtonKind,
    size: ButtonSize,
    active: bool,
    tipped: bool,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let label: SharedString = label.into();
    let look = look();
    let tone = match kind {
        ButtonKind::Primary => color(look.on_accent),
        ButtonKind::Ghost if active => super::icon_tone(true),
        _ if hover_is(&id) => white(ink::FULL),
        _ => white(ink::BODY),
    };
    let d = hoverable(id, radius::CONTROL, hover_of(kind))
        .role(Role::Button)
        .aria_label(label.clone())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(size.height()))
        .rounded(px(radius::CONTROL))
        .overflow_hidden()
        .cursor_pointer()
        .when(active && kind != ButtonKind::Ghost, |d| {
            d.child(div().absolute().size_full().bg(white(fill::ACTIVE)))
        })
        .child(icon(name, size.icon(), tone).relative())
        .when(tipped, |d| d.tooltip(tip(label.clone())));
    let d = match kind {
        ButtonKind::Primary => bevel(d.bg(color(look.accent))),
        ButtonKind::Secondary => secondary(d),
        ButtonKind::Ghost => selected(d, active).rounded(px(radius::CONTROL)),
    };
    tip_on_focus(d, label)
}

pub fn remove_button(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    size: ButtonSize,
) -> Stateful<Div> {
    icon_button(id, "delete-02", label, ButtonKind::Ghost, size, false)
}

pub fn more_button(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    kind: ButtonKind,
    size: ButtonSize,
    open: bool,
) -> Stateful<Div> {
    icon_button_tipped(id, "more-horizontal", label, kind, size, open, !open).aria_expanded(open)
}

pub fn more_busy(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    size: ButtonSize,
    open: bool,
) -> Stateful<Div> {
    more_button(id, label, ButtonKind::Primary, size, open).bg(busy_track())
}

fn busy_track() -> Hsla {
    color(look().busy_track(super::light()))
}

pub fn split(main: Stateful<Div>, more: Stateful<Div>, kind: ButtonKind) -> Div {
    let rule = match kind {
        ButtonKind::Primary => color(look().on_accent).opacity(SPLIT_RULE),
        _ => edge(),
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Confirm<'a> {
    Delete(&'a str),
    Remove(&'a str),
    Restore(&'a str),
    DeleteEverything,
    DiscardChanges,
}

impl Confirm<'_> {
    pub fn question(self) -> String {
        match self {
            Confirm::Delete(name) => crate::trf("Delete {name}?", &[("name", name)]),
            Confirm::Remove(name) => crate::trf("Remove {name}?", &[("name", name)]),
            Confirm::Restore(name) => crate::trf("Restore {name}?", &[("name", name)]),
            Confirm::DeleteEverything => crate::tr("Delete everything?").into(),
            Confirm::DiscardChanges => crate::tr("Discard your changes?").into(),
        }
    }

    pub fn verb(self) -> SharedString {
        crate::tr(match self {
            Confirm::Delete(_) | Confirm::DeleteEverything => "Delete",
            Confirm::Remove(_) => "Remove",
            Confirm::Restore(_) => "Restore",
            Confirm::DiscardChanges => "Discard",
        })
        .into()
    }

    pub fn keep(self) -> SharedString {
        match self {
            Confirm::DiscardChanges => crate::tr("Keep editing"),
            _ => crate::tr("Cancel"),
        }
        .into()
    }
}

pub fn confirm(
    id: impl Into<SharedString>,
    asked: Confirm,
    cancel: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    sure: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    let id: SharedString = id.into();
    let (question, verb) = (asked.question(), asked.verb());
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
                .child(question),
        )
        .child(
            div()
                .flex()
                .gap(px(space::SM))
                .child(
                    button(
                        id.clone(),
                        None,
                        asked.keep(),
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

pub fn chip_confirm(
    id: impl Into<SharedString>,
    asked: Confirm,
    cancel: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    sure: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    confirm(id, asked, cancel, sure)
        .h(px(super::CHIP))
        .px(px(LIST_INSET))
        .rounded(px(radius::ROW))
        .bg(white(fill::HOVER))
}

pub fn menu_confirm(
    id: impl Into<SharedString>,
    asked: Confirm,
    cancel: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    sure: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    confirm(id, asked, cancel, sure)
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
            .role(Role::ProgressIndicator)
            .aria_label(label.clone())
            .when_some(progress, |d, p| {
                d.aria_numeric_value(f64::from(p.clamp(0., 1.)))
            })
            .bg(busy_track())
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
    icon_button(
        id,
        "copy-01",
        crate::tr("Copy details"),
        ButtonKind::Ghost,
        size,
        false,
    )
    .on_press(move |_, _, cx| {
        crate::clipboard::copy_to_clipboard(cx, ClipboardItem::new_string(details.clone()))
    })
}

pub fn open_folder(dir: &std::path::Path, cx: &mut App) {
    let _ = std::fs::create_dir_all(dir);
    crate::toast::open(cx, dir);
}

pub fn open_folder_button(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    dir: std::path::PathBuf,
    size: ButtonSize,
) -> Stateful<Div> {
    button(id, Some("folder-01"), label, ButtonKind::Secondary, size)
        .on_press(move |_, _, cx| open_folder(&dir, cx))
}

pub fn copy_button(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    text: impl Into<SharedString>,
    size: ButtonSize,
) -> Stateful<Div> {
    let text: SharedString = text.into();
    button(id, Some("copy-01"), label, ButtonKind::Secondary, size).on_press(move |_, _, cx| {
        crate::clipboard::copy_to_clipboard(cx, ClipboardItem::new_string(text.to_string()))
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
                white(ink::MUTED)
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
        (false, _) => white(ink::DIM),
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
            white(fill::ACTIVE)
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
    hoverable(id, radius::ROW, Hover::Row)
        .role(Role::Switch)
        .aria_toggled(Toggled::from(on))
        .aria_label(title.clone())
        .when_some(described, |d, note| d.aria_description(note))
        .flex()
        .items_center()
        .gap(px(SETTING_ROW_GAP))
        .p(px(super::SETTING_ROW_PADDING))
        .rounded(px(radius::ROW))
        .cursor_pointer()
        .child(setting_text(title, note).relative())
        .child(switch)
}

pub const SETTING_ROW_GAP: f32 = space::XXL + space::SM;

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

/// A segmented control: options on a faint track, the chosen one raised.
pub fn segments<T: Copy + PartialEq + std::fmt::Debug + 'static>(
    id: impl Into<SharedString>,
    name: impl Into<SharedString>,
    options: &[(T, SharedString)],
    current: T,
    on_pick: impl Fn(T, &mut Window, &mut App) + Clone + 'static,
) -> Stateful<Div> {
    segments_at(id, name, options, current, ButtonSize::Sm.height(), on_pick)
}

pub fn segments_sized<T: Copy + PartialEq + std::fmt::Debug + 'static>(
    id: impl Into<SharedString>,
    name: impl Into<SharedString>,
    options: &[(T, SharedString)],
    current: T,
    size: ButtonSize,
    on_pick: impl Fn(T, &mut Window, &mut App) + Clone + 'static,
) -> Stateful<Div> {
    let height = size.height() - 2. * space::XS;
    segments_at(id, name, options, current, height, on_pick)
}

fn segments_at<T: Copy + PartialEq + std::fmt::Debug + 'static>(
    id: impl Into<SharedString>,
    name: impl Into<SharedString>,
    options: &[(T, SharedString)],
    current: T,
    height: f32,
    on_pick: impl Fn(T, &mut Window, &mut App) + Clone + 'static,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let values: Vec<T> = options.iter().map(|(value, _)| *value).collect();
    let rows = options
        .iter()
        .map(|(value, label)| {
            let active = current == *value;
            let segment = chosen(
                hoverable(
                    format!("{id}-{}", segment_key(value)),
                    radius::CONTROL,
                    Hover::Row,
                )
                .role(Role::RadioButton)
                .aria_toggled(Toggled::from(active))
                .flex()
                .items_center()
                .h(px(height))
                .px(px(ButtonSize::Sm.pad()))
                .text_color(white(if active { ink::FULL } else { ink::MUTED }))
                .child(press_text().relative().child(label.to_uppercase())),
                active,
            )
            .rounded(px(radius::CONTROL));
            (segment, label.clone(), active)
        })
        .collect();
    let group = div()
        .id(id)
        .role(Role::RadioGroup)
        .flex()
        .flex_none()
        .gap(px(space::XS))
        .p(px(space::XS))
        .rounded(px(radius::ROW))
        .bg(inset());
    super::options(group, name, rows, true, move |i, window, cx| {
        if let Some(&value) = values.get(i) {
            on_pick(value, window, cx)
        }
    })
}

pub(super) fn segment_key(value: &impl std::fmt::Debug) -> String {
    let shown = format!("{value:?}").to_lowercase();
    let words: Vec<&str> = shown
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    words.join("-")
}

fn padded_tab(
    id: impl Into<SharedString>,
    text: impl Into<SharedString>,
    open: bool,
    pad: f32,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let tone = if open || hover_is(&id) {
        ink::FULL
    } else {
        ink::MUTED
    };
    let text: SharedString = text.into();
    let d = super::track_hover(div().id(id.clone()), id)
        .role(Role::Tab)
        .aria_selected(open)
        .aria_label(text.clone())
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(TAB_COUNT_GAP))
        .h(px(ButtonSize::Md.height()))
        .px(px(pad))
        .rounded(px(radius::CONTROL))
        .cursor_pointer()
        .text_color(white(tone))
        .when(open, |d| {
            d.child(
                div()
                    .absolute()
                    .left(px(pad))
                    .right(px(pad))
                    .bottom_0()
                    .h(px(TAB_UNDERLINE))
                    .bg(color(if super::light() {
                        look().mark
                    } else {
                        look().accent
                    })),
            )
        });
    d.when(!text.is_empty(), |d| {
        d.child(press_text().child(text.to_uppercase()))
    })
}

pub const TAB_PAD: f32 = space::SM + space::XXS;

const TAB_COUNT_GAP: f32 = space::SM;

pub fn tab_width(label: &str, count: Option<usize>, pad: f32) -> f32 {
    let count = count.map_or(0., |n| TAB_COUNT_GAP + press_width(&n.to_string()));
    press_width(label) + count + 2. * pad
}

pub const TAB_PAD_TIGHT: f32 = space::SM;

pub const TAB_STRIP_GAP: f32 = space::LG;

pub fn tab_strip_padded(
    id: impl Into<SharedString>,
    pad: f32,
    tabs: impl IntoIterator<Item = Stateful<Div>>,
) -> Stateful<Div> {
    div()
        .id(id.into())
        .role(Role::TabList)
        .flex()
        .items_center()
        .mx(px(-pad))
        .mb(px(TAB_STRIP_GAP))
        .children(tabs)
}

/// A tab with a count after its label.
pub fn count_tab(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    count: Option<(usize, SharedString)>,
    open: bool,
    pad: f32,
) -> Stateful<Div> {
    let label: SharedString = label.into();
    let shown = count.as_ref().map(|(n, _)| n.to_string());
    padded_tab(id, label, open, pad)
        .when_some(count.map(|(_, said)| said), |d, said| {
            d.aria_description(said)
        })
        .children(shown.map(|n| press_text().text_color(white(ink::MUTED)).child(n)))
}

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
        .text_color(white(tone))
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

pub fn shown_on_focus(el: Stateful<Div>, shown: bool) -> Stateful<Div> {
    if shown {
        return el;
    }
    el.opacity(0.)
        .focus_visible(|s| super::inset_ring(s).opacity(1.))
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
    tip_on_focus(el.tooltip(tip(full.clone())), full)
}

pub fn link(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Stateful<Div> {
    let id: SharedString = id.into();
    let tone = if hover_is(&id) {
        white(ink::FULL)
    } else {
        color(look().accent_soft)
    };
    let label: SharedString = label.into();
    super::track_hover(
        super::text(step::BODY, FontWeight::MEDIUM, ink::FULL).id(id.clone()),
        id,
    )
    .role(Role::Link)
    .aria_label(label.clone())
    .flex()
    .flex_none()
    .items_center()
    .min_h(px(LINK_MIN_HEIGHT))
    .text_color(tone)
    .cursor_pointer()
    .child(label)
}

pub const NAV_ROW: f32 = 40.;

pub const NAV_ROW_SHORT: f32 = 32.;

pub fn chevron(open: bool) -> gpui::Svg {
    icon("arrow-down-01", IconSize::Px16, white(ink::MUTED)).with_transformation(
        gpui::Transformation::rotate(gpui::radians(if open { std::f32::consts::PI } else { 0. })),
    )
}

pub fn dropdown(
    id: impl Into<SharedString>,
    name: impl Into<SharedString>,
    leading: Option<AnyElement>,
    value: impl Into<SharedString>,
    open: bool,
    size: ButtonSize,
) -> Stateful<Div> {
    let value: SharedString = value.into();
    dropdown_face(id, name.into(), leading, value.clone(), open, size)
        .child(div().relative().flex_1().min_w_0().truncate().child(value))
        .child(chevron(open).relative())
}

pub fn labelled_dropdown(
    id: impl Into<SharedString>,
    name: impl Into<SharedString>,
    leading: Option<AnyElement>,
    value: impl Into<SharedString>,
    open: bool,
    size: ButtonSize,
) -> Stateful<Div> {
    let (name, value): (SharedString, SharedString) = (name.into(), value.into());
    let shown = div()
        .relative()
        .flex_1()
        .min_w_0()
        .flex()
        .when(size == ButtonSize::Hero, |d| d.flex_col())
        .when(size != ButtonSize::Hero, |d| {
            d.items_center().gap(px(space::SM))
        })
        .child(super::label(name.clone()).flex_none())
        .children(leading.map(|l| div().flex_none().child(l)))
        .child(
            div()
                .min_w_0()
                .truncate()
                .when(size == ButtonSize::Hero, |d| {
                    d.text_size(px(step::LEAD.size))
                        .line_height(px(step::LEAD.line))
                        .font_weight(FontWeight::MEDIUM)
                })
                .child(value.clone()),
        );
    dropdown_face(id, name, None, value, open, size)
        .child(shown)
        .child(chevron(open).relative())
}

fn dropdown_face(
    id: impl Into<SharedString>,
    name: SharedString,
    leading: Option<AnyElement>,
    value: SharedString,
    open: bool,
    size: ButtonSize,
) -> Stateful<Div> {
    let (gap, pad) = match (size, leading.is_some()) {
        (ButtonSize::Hero, _) => (space::XL, space::LG),
        (_, true) => (space::SM, space::SM),
        (_, false) => (space::SM, size.pad()),
    };
    hoverable(id, radius::CONTROL, Hover::Row)
        .role(Role::ComboBox)
        .aria_label(name)
        .aria_value(value)
        .aria_expanded(open)
        .flex()
        .flex_none()
        .items_center()
        .gap(px(gap))
        .h(px(size.height()))
        .pl(px(pad))
        .pr(px(space::MD))
        .rounded(px(radius::CONTROL))
        .bg(super::field_bg(open))
        .border_1()
        .border_color(super::field_edge())
        .cursor_pointer()
        .font_family(eclipse_theme::glass::FONT)
        .text_size(px(step::BODY.size))
        .text_color(white(ink::FULL))
        .children(leading.map(|l| div().relative().child(l)))
}

pub fn translated<T: Copy>(options: &[(T, &str)]) -> Vec<(T, SharedString)> {
    options
        .iter()
        .map(|&(value, label)| (value, SharedString::from(crate::tr(label).to_string())))
        .collect()
}

const SLIDER_HEIGHT: f32 = super::TILE_SM;

const SLIDER_KNOB: f32 = IconSize::Px16.px();

const SLIDER_PAGE: i32 = 10;

const HUE_STEPS: usize = 60;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Track {
    Fill,
    Hues,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SliderMove {
    To(f32),
    Step(i32),
    Done,
}

thread_local! {
    static SLIDERS: std::cell::RefCell<std::collections::HashMap<SharedString, gpui::Bounds<gpui::Pixels>>> =
        std::cell::RefCell::default();
    static DRAGGING: std::cell::RefCell<Option<SharedString>> = const { std::cell::RefCell::new(None) };
}

fn slider_at(id: &SharedString, x: gpui::Pixels) -> Option<f32> {
    let bounds = SLIDERS.with_borrow(|s| s.get(id).copied())?;
    let width: f32 = bounds.size.width.into();
    let across: f32 = (x - bounds.origin.x).into();
    (width > 0.).then(|| (across / width).clamp(0., 1.))
}

#[allow(clippy::too_many_arguments)]
pub fn slider(
    id: impl Into<SharedString>,
    name: impl Into<SharedString>,
    fraction: f32,
    value: f64,
    range: (f64, f64),
    track: Track,
    on_move: impl Fn(SliderMove, &mut Window, &mut App) + Clone + 'static,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let fraction = fraction.clamp(0., 1.);
    let width = crate::layout::FIELD_MD;
    let measured = id.clone();
    let bar = match track {
        Track::Fill => div()
            .h(px(super::BAR_HEIGHT))
            .w_full()
            .rounded_full()
            .overflow_hidden()
            .bg(white(fill::ACTIVE))
            .child(
                div()
                    .h_full()
                    .w(gpui::relative(fraction))
                    .rounded_full()
                    .bg(color(look().accent_soft)),
            )
            .into_any_element(),
        Track::Hues => gpui::canvas(
            |_, _, _| {},
            |bounds, _, window, _| {
                let step = f32::from(bounds.size.width) / HUE_STEPS as f32;
                for i in 0..HUE_STEPS {
                    let hue = i as f32 * 360. / HUE_STEPS as f32;
                    let colour: Hsla = gpui::rgb(eclipse_launcher::themes::hue_colour(hue)).into();
                    window.paint_quad(gpui::fill(
                        gpui::Bounds::new(
                            gpui::point(bounds.left() + px(step * i as f32), bounds.top()),
                            gpui::size(px(step), bounds.size.height),
                        ),
                        colour,
                    ));
                }
            },
        )
        .h(px(SLIDER_KNOB))
        .w_full()
        .rounded(px(radius::BLOCK))
        .into_any_element(),
    };
    let knob = div()
        .absolute()
        .left(px(fraction * (width - SLIDER_KNOB)))
        .size(px(SLIDER_KNOB))
        .rounded_full()
        .bg(super::paper())
        .border_1()
        .border_color(color(ON_LIGHT).opacity(KNOB_EDGE));
    let (down, moved, up, out, keys) = (
        on_move.clone(),
        on_move.clone(),
        on_move.clone(),
        on_move.clone(),
        on_move,
    );
    let (down_id, move_id) = (id.clone(), id.clone());
    let el = div()
        .id(id.clone())
        .role(Role::Slider)
        .aria_label(name.into())
        .aria_numeric_value(value)
        .aria_min_numeric_value(range.0)
        .aria_max_numeric_value(range.1)
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .w(px(width))
        .h(px(SLIDER_HEIGHT))
        .cursor_pointer()
        .child(
            gpui::canvas(
                move |bounds, _, _| {
                    SLIDERS.with_borrow_mut(|s| s.insert(measured.clone(), bounds));
                },
                |_, _, _, _| {},
            )
            .absolute()
            .size_full(),
        )
        .child(div().w_full().overflow_hidden().rounded_full().child(bar))
        .child(knob);
    super::focus_stop(el)
        .on_key_down(move |ev: &gpui::KeyDownEvent, window, cx| {
            let to = match ev.keystroke.key.as_str() {
                "left" | "down" => SliderMove::Step(-1),
                "right" | "up" => SliderMove::Step(1),
                "pagedown" => SliderMove::Step(-SLIDER_PAGE),
                "pageup" => SliderMove::Step(SLIDER_PAGE),
                "home" => SliderMove::To(0.),
                "end" => SliderMove::To(1.),
                _ => return,
            };
            keys(to, window, cx);
            if matches!(to, SliderMove::To(_)) {
                keys(SliderMove::Done, window, cx);
            }
            cx.stop_propagation();
        })
        .on_mouse_down(gpui::MouseButton::Left, move |ev, window, cx| {
            DRAGGING.set(Some(down_id.clone()));
            if let Some(at) = slider_at(&down_id, ev.position.x) {
                down(SliderMove::To(at), window, cx);
            }
        })
        .on_mouse_move(move |ev: &gpui::MouseMoveEvent, window, cx| {
            let dragging = DRAGGING.with_borrow(|d| d.as_ref() == Some(&move_id));
            if !dragging || ev.pressed_button != Some(gpui::MouseButton::Left) {
                return;
            }
            if let Some(at) = slider_at(&move_id, ev.position.x) {
                moved(SliderMove::To(at), window, cx);
            }
        })
        .on_mouse_up(gpui::MouseButton::Left, move |_, window, cx| {
            if DRAGGING.take().is_some() {
                up(SliderMove::Done, window, cx);
            }
        })
        .on_mouse_up_out(gpui::MouseButton::Left, move |_, window, cx| {
            if DRAGGING.take().is_some() {
                out(SliderMove::Done, window, cx);
            }
        })
}
