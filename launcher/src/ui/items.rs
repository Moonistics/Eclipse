use super::{
    color, danger, dot, hoverable, icon, look, row_fill, text, white, Hover, IconSize, Tone,
};
use eclipse_theme::glass::{fill, ink, radius, step, EDGE};
use gpui::{
    div, prelude::*, px, AnyElement, App, Div, FontWeight, Hsla, SharedString, Stateful, Window,
};

const SETTING_NOTE_WIDTH: f32 = 560.;

const NOTE_LINE_HEIGHT: f32 = 44.;

pub fn selected(d: Stateful<Div>, on: bool) -> Stateful<Div> {
    d.rounded(px(radius::ROW))
        .border_1()
        .when(on, |d| {
            d.bg(white(fill::SELECTED)).border_color(white(EDGE))
        })
        .when(!on, |d| d.border_color(gpui::transparent_black()))
}

pub fn row(id: impl Into<SharedString>, current: bool, pr: f32) -> Stateful<Div> {
    selected(
        hoverable(id, radius::ROW, Hover::Row)
            .flex()
            .items_center()
            .gap(px(12.))
            .w_full()
            .p(px(7.))
            .pr(px(pr))
            .cursor_pointer(),
        current,
    )
}

pub fn item_text(name: impl Into<SharedString>, note: impl IntoElement) -> Div {
    div()
        .relative()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .child(
            text(step::BODY, FontWeight::SEMIBOLD, ink::FULL)
                .truncate()
                .child(name.into()),
        )
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
        .gap(px(6.))
        .min_w_0()
        .child(dot(tone))
        .child(div().min_w_0().truncate().child(state.into()))
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
        .gap(px(12.))
        .w_full()
        .px(px(12.))
        .py(px(10.))
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
        .gap(px(12.))
        .h(px(MENU_ROW))
        .w_full()
        .px(px(12.))
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
        .gap(px(12.))
        .w_full()
        .px(px(12.))
        .py(px(8.))
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
        return crate::disabled(d);
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
        .gap(px(12.))
        .h(px(super::Size::Md.height()))
        .w_full()
        .px(px(12.))
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

pub fn setting_text(title: impl IntoElement, note: Option<AnyElement>) -> Div {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .gap(px(4.))
        .child(text(step::LEAD, FontWeight::SEMIBOLD, ink::FULL).child(title))
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
        .gap(px(40.))
        .p(px(super::SETTING_ROW_PADDING))
        .rounded(px(radius::ROW))
        .child(setting_text(title, note))
        .children(control.map(|c| div().flex().flex_none().items_center().gap(px(8.)).child(c)))
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
        .gap(px(16.))
        .p(px(super::SETTING_ROW_PADDING))
        .rounded(px(radius::ROW))
        .child(setting_text(title, note))
        .child(div().w_full().child(control))
}

/// The line over a tab's list: its note, then buttons.
pub fn list_head(note: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.))
        .px(px(12.))
        .pb(px(8.))
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
        .gap(px(12.))
        .min_h(px(NOTE_LINE_HEIGHT))
        .px(px(16.))
        .py(px(6.))
        .rounded(px(radius::ROW))
        .bg(white(fill::HOVER))
        .child(div().flex_none().size(px(7.)).rounded_full().bg(dot))
        .child(
            self::text(step::BODY, FontWeight::NORMAL, ink::FULL)
                .flex_1()
                .min_w_0()
                .child(text.into()),
        )
}

pub fn error_note(said: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .items_start()
        .gap(px(10.))
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
