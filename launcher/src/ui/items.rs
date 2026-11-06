use super::{
    color, danger, disabled, dot, hoverable, icon, look, row_fill, text, white, ButtonSize, Hover,
    IconSize, Tone, PANEL_INSET, SETTING_ROW_GAP, TILE_MD,
};
use eclipse_theme::glass::{fill, ink, radius, space, step, EDGE};
use gpui::{
    div, prelude::*, px, AnyElement, App, Div, ExternalPaths, FontWeight, Hsla, ImageSource,
    SharedString, Stateful, Window,
};

const SETTING_NOTE_WIDTH: f32 = 560.;

const NOTE_LINE_HEIGHT: f32 = 44.;

const TICK_SLOT: f32 = 20.;

pub fn selected(d: Stateful<Div>, on: bool) -> Stateful<Div> {
    d.rounded(px(radius::ROW))
        .border_1()
        .when(on, |d| {
            d.bg(white(fill::SELECTED)).border_color(white(EDGE))
        })
        .when(!on, |d| d.border_color(gpui::transparent_black()))
}

pub fn row(id: impl Into<SharedString>, current: bool) -> Stateful<Div> {
    selected(
        hoverable(id, radius::ROW, Hover::Row)
            .flex()
            .items_center()
            .gap(px(space::MD))
            .w_full()
            .p(px(space::SM))
            .cursor_pointer(),
        current,
    )
}

pub fn choice(id: impl Into<SharedString>, on: bool) -> Stateful<Div> {
    selected(
        hoverable(id, radius::ROW, Hover::Row)
            .flex()
            .cursor_pointer(),
        on,
    )
}

pub fn list_choice(id: impl Into<SharedString>, on: bool) -> Stateful<Div> {
    choice(id, on)
        .flex_none()
        .items_center()
        .gap(px(space::LG))
        .px(px(space::LG))
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
        .child(
            div()
                .relative()
                .flex_none()
                .size(px(CHIP_FACE))
                .rounded(px(radius::ROW))
                .overflow_hidden()
                .child(face),
        )
        .child(
            text(step::BODY, FontWeight::MEDIUM, ink::FULL)
                .relative()
                .child(name.into()),
        )
}

pub const CHIP: f32 = 44.;

pub const CHIP_FACE: f32 = 30.;

pub fn drop_target(el: Stateful<Div>) -> Stateful<Div> {
    el.drag_over::<ExternalPaths>(|s, _, _, _| s.bg(white(fill::SELECTED)))
}

/// A list row, faintly filled under the pointer. `on_hover` hears the pointer come and go too,
/// for rows that show their actions then. `held`: the row keeps its fill while the pointer is on
/// one of its buttons or its menu is open, as CSS's `:hover` stays on a parent.
pub fn list_row(
    id: impl Into<SharedString>,
    held: bool,
    on_hover: impl Fn(&bool, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let id = id.into();
    let key = id.clone();
    div()
        .id(id.clone())
        .relative()
        .flex()
        .items_center()
        .gap(px(space::MD))
        .w_full()
        .px(px(space::MD))
        .py(px(space::MD))
        .rounded(px(radius::ROW))
        .child(row_fill(&id, radius::ROW, held))
        .on_hover(move |hovered, window, cx| {
            super::note_hover(&key, *hovered, window);
            on_hover(hovered, window, cx);
        })
}

/// A popover's row: text after a muted icon, filled under the pointer. `danger` colours it red.
pub fn menu_row(
    id: impl Into<SharedString>,
    name: &'static str,
    label: impl Into<SharedString>,
    danger: bool,
) -> Stateful<Div> {
    let (fg, icon_color, hover) = if danger {
        let c = self::danger();
        (c, c, Hover::Danger)
    } else {
        (white(ink::FULL), white(ink::MUTED), Hover::Row)
    };
    hoverable(id, radius::CONTROL, hover)
        .flex()
        .items_center()
        .gap(px(space::MD))
        .h(px(MENU_ROW))
        .w_full()
        .px(px(space::MD))
        .rounded(px(radius::CONTROL))
        .cursor_pointer()
        .child(icon(name, IconSize::Px16, icon_color).relative())
        .child(
            text(step::BODY, FontWeight::NORMAL, ink::FULL)
                .relative()
                .text_color(fg)
                .child(label.into()),
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
    hoverable(id, radius::CONTROL, Hover::Row)
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
                .child(label.into()),
        )
        .children(picked.then(tick))
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
                .max_w(px(SETTING_NOTE_WIDTH))
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

pub fn notice_line(dot: Hsla, text: impl Into<SharedString>) -> Div {
    div()
        .relative()
        .flex()
        .items_center()
        .gap(px(space::MD))
        .min_h(px(NOTE_LINE_HEIGHT))
        .px(px(space::LG))
        .py(px(space::SM))
        .rounded(px(radius::ROW))
        .bg(white(fill::HOVER))
        .child(
            div()
                .flex_none()
                .size(px(super::DOT))
                .rounded_full()
                .bg(dot),
        )
        .child(
            self::text(step::BODY, FontWeight::NORMAL, ink::FULL)
                .flex_1()
                .min_w_0()
                .child(text.into()),
        )
}

pub fn notice(icon_name: &'static str, said: impl IntoElement, actions: Vec<AnyElement>) -> Div {
    div()
        .relative()
        .flex()
        .flex_col()
        .gap(px(space::MD))
        .p(px(PANEL_INSET))
        .rounded(px(radius::ROW))
        .bg(white(fill::HOVER))
        .child(
            div()
                .flex()
                .items_start()
                .gap(px(space::MD))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .h(px(step::BODY.line))
                        .child(icon(icon_name, IconSize::Px16, white(ink::BODY))),
                )
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

pub fn error_note(said: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .items_start()
        .gap(px(space::MD))
        .child(
            div()
                .flex()
                .items_center()
                .h(px(step::BODY.line))
                .child(icon("alert-circle", IconSize::Px16, danger())),
        )
        .child(
            text(step::BODY, FontWeight::NORMAL, ink::FULL)
                .flex_1()
                .min_w_0()
                .child(said.into()),
        )
}

pub fn error_line(said: impl Into<SharedString>, actions: Vec<AnyElement>) -> Div {
    div()
        .relative()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(space::MD))
        .min_h(px(NOTE_LINE_HEIGHT))
        .px(px(space::LG))
        .py(px(space::SM))
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
