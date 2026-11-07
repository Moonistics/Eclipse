use super::{
    color, copy_details, danger, disabled, dot, edge, hoverable, icon, light, look, row_fill,
    starts_menu, text, warn, white, ButtonSize, Hover, IconSize, Tone, SETTING_ROW_GAP, TILE_MD,
    TILE_SM,
};
use eclipse_theme::glass::{
    fill, ink, radius, space, step, ON_LIGHT, PAPER, SELECTED_EDGE, SELECTED_EDGE_LIGHT,
};
use gpui::{
    div, point, prelude::*, px, AnyElement, BoxShadow, Div, ExternalPaths, FontWeight, ImageSource,
    Role, SharedString, Stateful, Toggled,
};

const NOTE_LINE_HEIGHT: f32 = 44.;

const NOTICE_PAD: (f32, f32) = (space::LG, space::MD);

const TICK_SLOT: f32 = 20.;

const RAISE_SHADE: f32 = 0.14;

const RAISE_DROP: f32 = 2.;

pub const ROW_EDGE: f32 = 1.;

pub fn selected_edge() -> gpui::Hsla {
    if light() {
        color(ON_LIGHT).opacity(SELECTED_EDGE_LIGHT)
    } else {
        gpui::white().opacity(SELECTED_EDGE)
    }
}

pub fn selected(d: Stateful<Div>, on: bool) -> Stateful<Div> {
    d.rounded(px(radius::ROW))
        .border(px(ROW_EDGE))
        .when(on, |d| {
            d.bg(white(fill::SELECTED)).border_color(selected_edge())
        })
        .when(!on, |d| d.border_color(gpui::transparent_black()))
}

pub fn side_selected(d: Stateful<Div>, on: bool) -> Stateful<Div> {
    if !(on && light()) {
        return selected(d, on);
    }
    d.rounded(px(radius::ROW))
        .border(px(ROW_EDGE))
        .bg(color(PAPER))
        .border_color(selected_edge())
        .shadow(vec![BoxShadow {
            color: color(ON_LIGHT).opacity(RAISE_SHADE),
            offset: point(px(0.), px(RAISE_DROP)),
            blur_radius: px(0.),
            spread_radius: px(0.),
            inset: false,
        }])
}

pub fn row(
    id: impl Into<SharedString>,
    name: impl Into<SharedString>,
    current: bool,
) -> Stateful<Div> {
    selected(row_body(id, name.into(), current), current)
}

pub fn side_row(
    id: impl Into<SharedString>,
    name: impl Into<SharedString>,
    current: bool,
) -> Stateful<Div> {
    let row =
        row_body(id, name.into(), current).when(super::short_window(), |d| d.py(px(space::XS)));
    side_selected(row, current)
}

pub fn side_row_text_room(row_width: f32, pad_start: f32, pad_end: f32) -> f32 {
    row_width - 2. * ROW_EDGE - pad_start - pad_end - TILE_MD - space::MD
}

fn row_body(id: impl Into<SharedString>, name: SharedString, current: bool) -> Stateful<Div> {
    hoverable(id, radius::ROW, Hover::Row)
        .aria_label(name)
        .aria_selected(current)
        .flex()
        .items_center()
        .gap(px(space::MD))
        .w_full()
        .p(px(space::SM))
        .cursor_pointer()
}

pub fn choice(id: impl Into<SharedString>, on: bool) -> Stateful<Div> {
    selected(
        hoverable(id, radius::ROW, Hover::Row)
            .flex()
            .cursor_pointer(),
        on,
    )
}

pub fn choice_card(id: impl Into<SharedString>, on: bool) -> Stateful<Div> {
    choice(id, on)
        .when(!on, |d| d.border_color(edge()))
        .when(on, |d| {
            d.child(
                div()
                    .absolute()
                    .top(px(space::MD))
                    .right(px(space::MD))
                    .child(tick()),
            )
        })
}

pub fn list_choice(id: impl Into<SharedString>, on: bool) -> Stateful<Div> {
    choice(id, on)
        .flex_none()
        .items_center()
        .gap(px(space::LG))
        .px(px(space::LG))
}

pub fn choice_card_row(id: impl Into<SharedString>, on: bool) -> Stateful<Div> {
    list_choice(id, on).when(!on, |d| d.border_color(edge()))
}

pub fn item_text(name: impl Into<SharedString>, note: impl IntoElement) -> Div {
    item_lines(name_line(name.into()), note)
}

pub fn item_text_by(
    name: impl Into<SharedString>,
    by: impl Into<SharedString>,
    note: impl IntoElement,
) -> Div {
    let line = div()
        .flex()
        .items_baseline()
        .gap(px(space::SM))
        .min_w_0()
        .child(name_line(name.into()))
        .child(
            text(step::NOTE, FontWeight::NORMAL, ink::DIM)
                .truncate()
                .child(by.into()),
        );
    item_lines(line, note)
}

pub fn item_text_wrapped(
    name: impl Into<SharedString>,
    note: impl Into<SharedString>,
    lines: usize,
) -> Div {
    div()
        .relative()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .child(name_line(name.into()))
        .child(
            text(step::NOTE, FontWeight::NORMAL, ink::MUTED)
                .line_clamp(lines)
                .child(note.into()),
        )
}

fn name_line(name: SharedString) -> Div {
    text(step::BODY, FontWeight::SEMIBOLD, ink::FULL)
        .truncate()
        .child(name)
}

fn item_lines(name: Div, note: impl IntoElement) -> Div {
    div()
        .relative()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .child(name)
        .child(
            text(step::NOTE, FontWeight::NORMAL, ink::MUTED)
                .truncate()
                .child(note),
        )
}

pub fn state_line(tone: Tone, state: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(space::SM))
        .min_w_0()
        .child(dot(tone))
        .child(div().min_w_0().truncate().child(state.into()))
}

fn state_lines(tone: Tone, state: SharedString, line: f32) -> Div {
    div()
        .flex()
        .gap(px(space::SM))
        .min_w_0()
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .h(px(line))
                .child(dot(tone)),
        )
        .child(div().min_w_0().child(state))
}

pub fn state_note(tone: Tone, state: impl Into<SharedString>) -> Div {
    text(step::NOTE, FontWeight::NORMAL, ink::BODY).child(state_lines(
        tone,
        state.into(),
        step::NOTE.line,
    ))
}

pub fn state_title(tone: Tone, state: impl Into<SharedString>) -> Div {
    let tint = match tone {
        Tone::Live | Tone::Warn => super::tone_colour(tone),
        _ => white(ink::DIM),
    };
    text(step::BODY, FontWeight::MEDIUM, ink::FULL)
        .text_color(tint)
        .child(state_lines(tone, state.into(), step::BODY.line))
}

pub fn row_picture(image: Option<ImageSource>, fallback: &'static str) -> AnyElement {
    match image {
        Some(image) => super::image_tile(image, TILE_MD),
        None => super::tile(TILE_MD)
            .relative()
            .child(icon(fallback, IconSize::Px16, white(ink::MUTED)))
            .into_any_element(),
    }
}

pub fn chip(
    id: impl Into<SharedString>,
    face: impl IntoElement,
    name: impl Into<SharedString>,
    on: bool,
) -> Stateful<Div> {
    selected(
        chip_body(hoverable(id, radius::ROW, Hover::Row), face, name)
            .when(!on, |d| d.bg(white(fill::HOVER)).cursor_pointer()),
        on,
    )
}

pub fn filter_chip(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    on: bool,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let label: SharedString = label.into();
    let tone = if on || super::hover_is(&id) {
        ink::FULL
    } else {
        ink::MUTED
    };
    selected(
        hoverable(id, radius::CONTROL, Hover::Row)
            .flex()
            .flex_none()
            .items_center()
            .gap(px(space::XS))
            .h(px(ButtonSize::Sm.height()))
            .px(px(space::MD))
            .bg(super::secondary_fill())
            .cursor_pointer()
            .text_color(white(tone))
            .children(on.then(tick))
            .child(super::press_text().relative().child(label)),
        on,
    )
    .when(light() && !on, |d| d.border_color(edge()))
    .rounded(px(radius::CONTROL))
}

pub fn person_chip(face: impl IntoElement, name: impl Into<SharedString>) -> Div {
    chip_body(div(), face, name)
        .rounded(px(radius::ROW))
        .bg(white(fill::HOVER))
}

fn chip_body<E: Styled + ParentElement>(
    el: E,
    face: impl IntoElement,
    name: impl Into<SharedString>,
) -> E {
    el.flex()
        .items_center()
        .gap(px(space::MD))
        .h(px(CHIP))
        .pl(px(space::SM))
        .pr(px(space::MD))
        .child(super::face(face, CHIP_FACE))
        .child(
            text(step::BODY, FontWeight::MEDIUM, ink::FULL)
                .relative()
                .child(name.into()),
        )
}

pub const CHIP: f32 = 44.;

pub const CHIP_FACE: f32 = TILE_SM;

pub fn drop_target(el: Stateful<Div>) -> Stateful<Div> {
    el.drag_over::<ExternalPaths>(|s, _, _, _| s.bg(white(fill::SELECTED)))
}

/// A list row, faintly filled under the pointer (`hover_within` tells a row's own code, for
/// rows that show their actions then), and while the pointer is on one of its buttons, as CSS's
/// `:hover` stays on a parent. `held`: it keeps its fill while its menu is open.
pub fn list_row(id: impl Into<SharedString>, held: bool) -> Stateful<Div> {
    let id: SharedString = id.into();
    let held = held || super::hover_within(&id);
    super::track_hover(div().id(id.clone()), id.clone())
        .relative()
        .flex()
        .items_center()
        .gap(px(space::MD))
        .w_full()
        .px(px(space::MD))
        .py(px(space::MD))
        .rounded(px(radius::ROW))
        .child(row_fill(&id, radius::ROW, held))
}

/// A popover's row: text after a muted icon, filled under the pointer. `danger` colours it red.
pub fn menu_row(
    id: impl Into<SharedString>,
    name: &'static str,
    label: impl Into<SharedString>,
    danger: bool,
) -> Stateful<Div> {
    menu_row_in(id, name, label.into(), danger, IconSize::Px16.px())
}

pub fn face_menu_row(
    id: impl Into<SharedString>,
    name: &'static str,
    label: impl Into<SharedString>,
) -> Stateful<Div> {
    menu_row_in(id, name, label.into(), false, CHIP_FACE)
}

fn menu_row_in(
    id: impl Into<SharedString>,
    name: &'static str,
    label: SharedString,
    danger: bool,
    slot: f32,
) -> Stateful<Div> {
    let (fg, icon_color, hover) = if danger {
        let c = self::danger();
        (c, c, Hover::Danger)
    } else {
        (white(ink::FULL), white(ink::MUTED), Hover::Row)
    };
    hoverable(id, radius::CONTROL, hover)
        .role(Role::MenuItem)
        .aria_label(label.clone())
        .when(danger, |d| d.mt(px(space::XS)))
        .flex()
        .items_center()
        .gap(px(space::MD))
        .h(px(MENU_ROW))
        .w_full()
        .px(px(space::MD))
        .rounded(px(radius::CONTROL))
        .cursor_pointer()
        .child(
            div()
                .relative()
                .flex()
                .flex_none()
                .justify_center()
                .w(px(slot))
                .child(icon(name, IconSize::Px16, icon_color)),
        )
        .child(
            text(step::BODY, FontWeight::NORMAL, ink::FULL)
                .relative()
                .text_color(fg)
                .child(label),
        )
}

const MENU_ROW: f32 = 40.;

/// A menu row with a note under its label, filled under the pointer, disabled while `locked`.
pub fn menu_item(
    id: impl Into<SharedString>,
    label: SharedString,
    note: String,
    locked: bool,
) -> Stateful<Div> {
    let d = hoverable(id, radius::CONTROL, Hover::Row)
        .role(Role::MenuItem)
        .aria_label(label.clone())
        .when(!note.is_empty(), |d| d.aria_description(note.clone()))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(space::MD))
        .w_full()
        .px(px(space::MD))
        .py(px(space::SM))
        .rounded(px(radius::CONTROL))
        .when(!locked, |d| d.cursor_pointer())
        .child(if note.is_empty() {
            div()
                .relative()
                .flex_1()
                .min_w_0()
                .child(text(step::BODY, FontWeight::SEMIBOLD, ink::FULL).child(label))
        } else {
            item_text(label, note)
        });
    if locked {
        return disabled(d);
    }
    d
}

/// A dropdown's choice: its name on one line, the accent's tick on the one `picked`.
pub fn pick_row(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    picked: bool,
) -> Stateful<Div> {
    let label: SharedString = label.into();
    let row = hoverable(id, radius::CONTROL, Hover::Row)
        .role(Role::MenuItemRadio)
        .aria_toggled(Toggled::from(picked))
        .aria_label(label.clone())
        .flex()
        .items_center()
        .justify_between()
        .gap(px(space::MD))
        .h(px(ButtonSize::Md.height()))
        .w_full()
        .px(px(space::MD))
        .rounded(px(radius::CONTROL))
        .cursor_pointer()
        .child(
            text(step::BODY, FontWeight::NORMAL, ink::FULL)
                .relative()
                .truncate()
                .child(label),
        )
        .children(picked.then(tick));
    if picked {
        starts_menu(row)
    } else {
        row
    }
}

pub fn tick() -> gpui::Svg {
    icon("tick-02", IconSize::Px16, color(look().mark)).relative()
}

pub fn tick_slot(on: bool) -> Div {
    div()
        .flex()
        .flex_none()
        .justify_center()
        .w(px(TICK_SLOT))
        .children(on.then(tick))
}

pub fn setting_text(title: impl IntoElement, note: Option<AnyElement>) -> Div {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .gap(px(space::XS))
        .child(text(step::BODY, FontWeight::SEMIBOLD, ink::FULL).child(title))
        .children(note.map(|n| {
            text(step::NOTE, FontWeight::NORMAL, ink::MUTED)
                .max_w(px(crate::layout::MEASURE_NOTE))
                .child(n)
        }))
}

/// A Settings row: the name over its note, the control on the right.
pub fn setting_row(
    title: impl IntoElement,
    note: Option<AnyElement>,
    control: Option<AnyElement>,
) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(SETTING_ROW_GAP))
        .p(px(super::SETTING_ROW_PADDING))
        .rounded(px(radius::ROW))
        .child(setting_text(title, note))
        .children(control.map(|c| {
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(px(space::SM))
                .child(c)
        }))
}

/// A stacked Settings row: the name and note, the control under them at full width.
pub fn setting_row_stacked(
    title: impl IntoElement,
    note: Option<AnyElement>,
    control: AnyElement,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(space::LG))
        .p(px(super::SETTING_ROW_PADDING))
        .rounded(px(radius::ROW))
        .child(setting_text(title, note))
        .child(div().w_full().child(control))
}

pub fn setting_row_fit(
    beside: bool,
    title: impl IntoElement,
    note: Option<AnyElement>,
    control: AnyElement,
) -> Div {
    if beside {
        setting_row(title, note, Some(control))
    } else {
        setting_row_stacked(title, note, div().flex().child(control).into_any_element())
    }
}

/// The line over a tab's list: its note, then buttons.
pub fn list_head(note: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(space::MD))
        .px(px(space::MD))
        .pb(px(space::SM))
        .child(
            text(step::NOTE, FontWeight::NORMAL, ink::MUTED)
                .min_w_0()
                .truncate()
                .child(note.into()),
        )
}

pub fn notice_line(tone: Tone, text: impl Into<SharedString>) -> Div {
    div()
        .relative()
        .flex()
        .items_center()
        .gap(px(space::MD))
        .min_h(px(NOTE_LINE_HEIGHT))
        .px(px(NOTICE_PAD.0))
        .py(px(NOTICE_PAD.1))
        .rounded(px(radius::ROW))
        .bg(white(fill::HOVER))
        .child(dot(tone))
        .child(
            self::text(step::BODY, FontWeight::NORMAL, ink::FULL)
                .flex_1()
                .min_w_0()
                .child(text.into()),
        )
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Notice {
    Info,
    Warn,
    InCard,
}

pub fn notice(
    kind: Notice,
    icon_name: &'static str,
    said: impl IntoElement,
    actions: Vec<AnyElement>,
) -> Div {
    let (plate, mark) = match kind {
        Notice::Warn => (warn().opacity(fill::DANGER), warn()),
        Notice::Info => (white(fill::HOVER), white(ink::BODY)),
        Notice::InCard => (gpui::transparent_black(), white(ink::BODY)),
    };
    div()
        .relative()
        .flex()
        .flex_col()
        .gap(px(space::MD))
        .when(kind != Notice::InCard, |d| {
            d.px(px(NOTICE_PAD.0)).py(px(NOTICE_PAD.1))
        })
        .rounded(px(radius::ROW))
        .bg(plate)
        .child(
            div()
                .flex()
                .items_start()
                .gap(px(space::MD))
                .child(notice_icon(icon_name, mark))
                .child(
                    text(step::BODY, FontWeight::NORMAL, ink::FULL)
                        .flex_1()
                        .min_w_0()
                        .child(said),
                ),
        )
        .when(!actions.is_empty(), |d| {
            d.child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(space::SM))
                    .children(actions),
            )
        })
}

fn notice_icon(name: &'static str, colour: gpui::Hsla) -> Div {
    div()
        .flex()
        .items_center()
        .h(px(step::BODY.line))
        .child(icon(name, IconSize::Px16, colour))
}

pub fn error_note(said: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .items_start()
        .gap(px(space::MD))
        .child(notice_icon("alert-circle", danger()))
        .child(
            text(step::BODY, FontWeight::NORMAL, ink::FULL)
                .flex_1()
                .min_w_0()
                .child(said_once(said.into())),
        )
}

fn said_once(text: SharedString) -> Stateful<Div> {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hash);
    super::said(
        gpui::ElementId::NamedInteger("said".into(), hash.finish()),
        text,
    )
    .role(Role::Alert)
}

pub fn error_line(said: impl Into<SharedString>, actions: Vec<AnyElement>) -> Div {
    div()
        .relative()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(space::MD))
        .min_h(px(NOTE_LINE_HEIGHT))
        .px(px(NOTICE_PAD.0))
        .py(px(NOTICE_PAD.1))
        .rounded(px(radius::ROW))
        .bg(white(fill::HOVER))
        .child(error_note(said).flex_1().min_w_0())
        .when(!actions.is_empty(), |d| {
            d.child(
                div()
                    .flex()
                    .flex_none()
                    .gap(px(space::SM))
                    .children(actions),
            )
        })
}

pub fn error_line_for(
    id: impl Into<SharedString>,
    title: impl Into<SharedString>,
    error: &str,
    retry: Option<Stateful<Div>>,
    extra: Vec<Stateful<Div>>,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let copy = copy_details(format!("{id}-copy"), error.to_owned(), ButtonSize::Md);
    let actions = retry.into_iter().chain(extra).chain([copy]);
    let cause = super::told_cause(error);
    div()
        .id(id.clone())
        .role(Role::Alert)
        .relative()
        .flex()
        .flex_col()
        .gap(px(space::MD))
        .px(px(NOTICE_PAD.0))
        .py(px(NOTICE_PAD.1))
        .rounded(px(radius::ROW))
        .bg(white(fill::HOVER))
        .child(
            div()
                .flex()
                .items_start()
                .gap(px(space::MD))
                .child(notice_icon("alert-circle", danger()))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .child(
                            text(step::BODY, FontWeight::SEMIBOLD, ink::FULL)
                                .child(super::said(format!("{id}-title"), title.into())),
                        )
                        .children(cause.map(|cause| {
                            text(step::NOTE, FontWeight::NORMAL, ink::BODY)
                                .child(super::said(format!("{id}-cause"), cause))
                        })),
                ),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(space::SM))
                .children(actions),
        )
}

pub fn release_head(
    title: impl Into<SharedString>,
    note: impl Into<SharedString>,
    states: Vec<Div>,
) -> Div {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .gap(px(space::XXS))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(space::MD))
                .child(super::card_title(title).truncate())
                .children(states),
        )
        .child(super::card_note(note.into()).truncate())
}

pub fn pick_item(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    note: impl Into<SharedString>,
    picked: bool,
) -> Stateful<Div> {
    let (label, note): (SharedString, SharedString) = (label.into(), note.into());
    let row = hoverable(id, radius::CONTROL, Hover::Row)
        .role(Role::MenuItemRadio)
        .aria_toggled(Toggled::from(picked))
        .aria_label(label.clone())
        .when(!note.is_empty(), |d| d.aria_description(note.clone()))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(space::MD))
        .w_full()
        .px(px(space::MD))
        .py(px(space::SM))
        .rounded(px(radius::CONTROL))
        .cursor_pointer()
        .child(item_text(label, note))
        .child(tick_slot(picked).relative());
    if picked {
        starts_menu(row)
    } else {
        row
    }
}
