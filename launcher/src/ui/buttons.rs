use super::{
    color, edge, gap_ring, hover_is, hoverable, icon, inset, look, pixel_text, selected, tip,
    tip_on_focus, value_key, white, Hover, IconSize, Press, TipSide, LIST_INSET, PANEL_INSET,
};
use eclipse_theme::glass::{
    fill, ink,
    pixel_text::{PAGE_TITLE, PRESS},
    radius, space, step, PAPER,
};
use gpui::{
    div, prelude::*, px, AnyElement, App, ClickEvent, ClipboardItem, Div, FontWeight, Hsla, Role,
    SharedString, Stateful, Toggled, Window,
};

const BEVEL_SHINE: f32 = 0.3;

const BEVEL_SHADE: f32 = 0.22;

const BEVEL_SHINE_HEIGHT: f32 = 2.;

const BEVEL_SHADE_HEIGHT: f32 = 3.;

const SPLIT_RULE: f32 = 0.2;

const LINK_MIN_HEIGHT: f32 = 24.;

const REMOVE_ICON: &str = "delete-02";

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

    fn label_size(self) -> f32 {
        match self {
            ButtonSize::Hero => PAGE_TITLE,
            _ => PRESS,
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

pub(super) fn shine<E: ParentElement>(d: E, highlight: f32) -> E {
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
    pixel_width(&shown, size.label_size()) + icon + 2. * size.pad()
}

fn hover_of(kind: ButtonKind) -> Hover {
    match kind {
        ButtonKind::Primary => Hover::Primary,
        ButtonKind::Secondary => Hover::Row,
        ButtonKind::Ghost => Hover::Bare,
    }
}

pub fn press_width(label: &str) -> f32 {
    pixel_width(label, PRESS)
}

fn pixel_width(label: &str, size: f32) -> f32 {
    let advance = eclipse_theme::glass::MONOCRAFT_ADVANCE * super::on_pixel_grid(size)
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
    debug_assert!(
        kind != ButtonKind::Primary,
        "a Primary goes through ui::primary"
    );
    any_button(id, icon_name, label, kind, size)
}

pub(super) fn any_button(
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
    any_button(id, icon_name, label, ButtonKind::Primary, size)
}

pub fn primary_unavailable(
    id: impl Into<SharedString>,
    icon_name: Option<&'static str>,
    label: impl Into<SharedString>,
    size: ButtonSize,
    why: impl Into<SharedString>,
) -> Stateful<Div> {
    let face = unavailable_primary_face(id.into(), icon_name, label.into(), size);
    super::held_back(face, why.into())
}

pub fn primary_unavailable_said(
    id: impl Into<SharedString>,
    icon_name: Option<&'static str>,
    label: impl Into<SharedString>,
    size: ButtonSize,
    why: impl Into<SharedString>,
) -> Stateful<Div> {
    let face = unavailable_primary_face(id.into(), icon_name, label.into(), size);
    super::held_back_said(face, why.into())
}

fn unavailable_primary_face(
    id: SharedString,
    icon_name: Option<&'static str>,
    label: SharedString,
    size: ButtonSize,
) -> Stateful<Div> {
    let face = button_face(
        div().id(id),
        icon_name,
        label,
        size,
        white(ink::DIM),
        shouts(ButtonKind::Primary, size),
    );
    secondary(face)
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
    tip_on_focus(d, label, TipSide::Beside)
}

pub fn icon_button_unavailable(
    id: impl Into<SharedString>,
    name: &'static str,
    label: impl Into<SharedString>,
    size: ButtonSize,
    why: impl Into<SharedString>,
) -> Stateful<Div> {
    let button = icon_button_tipped(id, name, label, ButtonKind::Ghost, size, false, false);
    super::unavailable(button, why)
}

pub fn remove_button(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    size: ButtonSize,
) -> Stateful<Div> {
    icon_button(id, REMOVE_ICON, label, ButtonKind::Ghost, size, false)
}

pub fn remove_button_unavailable(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    size: ButtonSize,
    why: impl Into<SharedString>,
) -> Stateful<Div> {
    icon_button_unavailable(id, REMOVE_ICON, label, size, why)
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
    confirm_in(id.into(), asked, cancel, sure, false)
}

fn confirm_in(
    id: SharedString,
    asked: Confirm,
    cancel: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    sure: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    in_menu: bool,
) -> Div {
    let (question, verb) = (asked.question(), asked.verb());
    let keep = button(
        id.clone(),
        None,
        asked.keep(),
        ButtonKind::Ghost,
        ButtonSize::Sm,
    );
    let keep = if in_menu {
        super::starts_menu(keep)
    } else {
        keep
    };
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
                .child(keep.on_press(cancel))
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
    confirm_in(id.into(), asked, cancel, sure, true)
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
    super::busy_ringed(bevel(
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
    ))
}

pub fn copy_details(
    id: impl Into<SharedString>,
    details: String,
    size: ButtonSize,
) -> Stateful<Div> {
    copy_button(id, crate::tr("Copy details"), details, size)
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
    button(
        id,
        Some(crate::icons::COPY),
        label,
        ButtonKind::Secondary,
        size,
    )
    .on_press(move |_, _, cx| {
        crate::clipboard::copy_to_clipboard(cx, ClipboardItem::new_string(text.to_string()))
    })
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
            let segment = selected(
                hoverable(
                    format!("{id}-{}", value_key(value)),
                    radius::CONTROL,
                    Hover::Row,
                )
                .role(Role::RadioButton)
                .aria_toggled(Toggled::from(active))
                .flex()
                .items_center()
                .h(px(height))
                .px(px(ButtonSize::Sm.pad()))
                .text_color(white(if active { ink::FULL } else { ink::DIM }))
                .child(press_text().relative().child(label.clone())),
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
        .font_family(crate::theme::FONT)
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

pub fn shown_on_focus(el: Stateful<Div>, shown: bool) -> Stateful<Div> {
    if shown {
        return el;
    }
    el.opacity(0.)
        .focus_visible(|s| super::inset_ring(s).opacity(1.))
}
