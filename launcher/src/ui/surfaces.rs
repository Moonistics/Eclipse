use super::{
    button, card_title, color, danger, icon, light, look, menu_keys, scroll_pane, text, tile, well,
    white, ButtonKind, IconSize, Press, Size, PANEL_INSET,
};
use eclipse_theme::glass::{
    fill, ink, radius, step, EDGE, FIELD_EDGE, FIELD_EDGE_LIGHT, FLOAT, PAPER,
};
use gpui::{
    div, prelude::*, px, AnyElement, ClipboardItem, Div, FontWeight, Hsla, ScrollEvent,
    ScrollHandle, SharedString, Stateful,
};

pub const POPOVER_WIDTH: f32 = 280.;

pub const MENU_GAP: f32 = 8.;

const EMPTY_TILE: f32 = 48.;

const EMPTY_NOTE_WIDTH: f32 = 520.;

const FIELD_HEIGHT: f32 = 44.;

const WIDE_FIELD_MAX: f32 = 560.;

const MORE_BELOW_FADE: f32 = 32.;

const SCRIM: f32 = 0.5;

const SCRIM_LIGHT: f32 = 0.2;

const LOG_HEIGHT: f32 = 320.;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Surface {
    Card,
    Float,
}

pub fn surface_colour(kind: Surface) -> Hsla {
    if light() {
        return color(PAPER);
    }
    match kind {
        Surface::Card => color(look().plate),
        Surface::Float => color(FLOAT),
    }
}

pub fn backdrop(kind: Surface, radius: f32) -> AnyElement {
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .rounded(px(radius))
        .bg(surface_colour(kind))
        .border_1()
        .border_color(white(EDGE))
        .into_any_element()
}

pub fn card(radius: f32) -> Div {
    div()
        .relative()
        .rounded(px(radius))
        .child(backdrop(Surface::Card, radius))
}

pub fn float_card(radius: f32) -> Div {
    div()
        .relative()
        .rounded(px(radius))
        .child(backdrop(Surface::Float, radius))
}

pub fn popover(id: impl Into<SharedString>) -> Stateful<Div> {
    menu_keys(
        float_card(radius::POPOVER)
            .shadow_lg()
            .w(px(POPOVER_WIDTH))
            .p(px(6.))
            .flex()
            .flex_col()
            .id(id.into())
            .occlude(),
    )
}

/// What a dialog or the palette lays over the launcher: GPUI can't blur what it drew, so a
/// menu's float covers the page. The scrim goes over it.
pub fn scrim_backdrop() -> impl IntoElement {
    backdrop(Surface::Float, 0.)
}

/// The scrim over [`scrim_backdrop`]: lighter in the light appearance, so the dimmed launcher
/// stays day-lit.
pub fn scrim() -> Hsla {
    color(eclipse_theme::glass::SCRIM).opacity(if light() { SCRIM_LIGHT } else { SCRIM })
}

/// A dialog's plate, round at `radius`: its piece of the scene under the scrim's dimming, then
/// the plates' white. The content goes after it.
pub fn dialog_plate(radius: f32) -> Div {
    let layer = |c: Hsla| {
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .rounded(px(radius))
            .bg(c)
    };
    card(radius)
        // Light plates are already day-lit: no dimming under them.
        .child(layer(
            color(eclipse_theme::glass::SCRIM).opacity(if light() { 0. } else { SCRIM }),
        ))
        .child(layer(white(fill::HOVER)))
}

pub fn dialog_colour() -> Hsla {
    let plate = surface_colour(Surface::Card);
    let plate = if light() { plate } else { plate.blend(scrim()) };
    plate.blend(white(fill::HOVER))
}

fn state(id: SharedString, glyph: Div, title: SharedString, note: SharedString) -> Stateful<Div> {
    div()
        .id(id)
        .relative()
        .flex()
        .flex_col()
        .items_center()
        .p(px(PANEL_INSET))
        .text_center()
        .child(glyph)
        .child(card_title(title).mt(px(16.)))
        .when(!note.is_empty(), |d| {
            d.child(
                empty_style()
                    .mt(px(4.))
                    .max_w(px(EMPTY_NOTE_WIDTH))
                    .child(note),
            )
        })
}

fn actions(d: Stateful<Div>, actions: Vec<Stateful<Div>>) -> Stateful<Div> {
    d.when(!actions.is_empty(), |d| {
        d.child(
            div()
                .flex()
                .flex_wrap()
                .justify_center()
                .gap(px(8.))
                .mt(px(20.))
                .children(actions),
        )
    })
}

pub fn empty_state(
    id: impl Into<SharedString>,
    icon_name: &'static str,
    title: impl Into<SharedString>,
    note: impl Into<SharedString>,
    actions: Vec<Stateful<Div>>,
) -> Stateful<Div> {
    let glyph = tile(EMPTY_TILE).child(icon(icon_name, IconSize::Px32, white(ink::MUTED)));
    self::actions(state(id.into(), glyph, title.into(), note.into()), actions)
}

pub fn loading_state(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Stateful<Div> {
    let glyph = tile(EMPTY_TILE).child(icon("clock", IconSize::Px32, white(ink::MUTED)));
    state(id.into(), glyph, title.into(), SharedString::default())
}

pub fn error_state(
    id: impl Into<SharedString>,
    title: impl Into<SharedString>,
    error: &str,
    retry: Stateful<Div>,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let glyph = tile(EMPTY_TILE).child(icon("alert-circle", IconSize::Px32, danger()));
    let details = error.to_owned();
    let copy = button(
        format!("{id}-copy"),
        Some("copy-01"),
        crate::tr("Copy details"),
        ButtonKind::Ghost,
        Size::Md,
    )
    .on_press(move |_, _, cx| {
        crate::clipboard::copy_to_clipboard(cx, ClipboardItem::new_string(details.clone()))
    });
    actions(
        state(id, glyph, title.into(), plain_error(error).into()),
        vec![retry, copy],
    )
}

pub fn plain_error(error: &str) -> &str {
    error.lines().next().unwrap_or_default()
}

pub fn error_card(said: impl Into<SharedString>) -> Div {
    card(radius::PANEL)
        .p(px(PANEL_INSET))
        .child(super::error_note(said))
}

pub fn empty_card(state: Stateful<Div>) -> Div {
    card(radius::PANEL).p(px(PANEL_INSET)).child(state)
}

/// An empty state inside an area that is already a card; the message goes in as a child.
pub fn empty() -> Div {
    empty_style().p(px(24.))
}

fn empty_style() -> Div {
    text(step::BODY, FontWeight::NORMAL, ink::MUTED)
}

pub fn more_below_fade(pane: &ScrollHandle, under: Hsla) -> Option<Div> {
    let more = -pane.offset().y < pane.max_offset().y - px(1.);
    more.then(|| {
        div()
            .absolute()
            .left_0()
            .right_0()
            .bottom_0()
            .h(px(MORE_BELOW_FADE))
            .bg(gpui::linear_gradient(
                180.,
                gpui::linear_color_stop(under.opacity(0.), 0.),
                gpui::linear_color_stop(under, 1.),
            ))
    })
}

/// [`field`]'s look on a field built elsewhere (a line edit with its own handlers), up to
/// 560 px wide.
pub fn wide_field(field: Stateful<Div>, focused: bool) -> Stateful<Div> {
    field
        .w_full()
        .max_w(px(WIDE_FIELD_MAX))
        .h(px(FIELD_HEIGHT))
        .px(px(16.))
        .rounded(px(radius::ROW))
        .bg(field_bg(focused))
        .border_1()
        .border_color(field_edge())
        .font_family(eclipse_theme::glass::FONT)
        .text_size(px(step::BODY.size))
        .relative()
        .when(focused, |d| d.child(focus_ring(radius::ROW)))
}

/// The focus ring (2 px of the accent's soft shade) over a focused field, inside its edge with
/// the field's corners: GPUI has no outline, and a field clips what lies outside it. Put it in a
/// `relative` field round at `radius`.
pub fn focus_ring(radius: f32) -> Div {
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .rounded(px(radius))
        .border_2()
        .border_color(color(look().accent_soft))
}

pub fn field_bg(focused: bool) -> Hsla {
    if light() {
        white(if focused { fill::SELECTED } else { fill::FIELD })
    } else {
        color(if focused {
            eclipse_theme::glass::FIELD_BG_FOCUS
        } else {
            eclipse_theme::glass::FIELD_BG
        })
    }
}

pub fn field_edge() -> Hsla {
    if light() {
        color(eclipse_theme::glass::ON_LIGHT).opacity(FIELD_EDGE_LIGHT)
    } else {
        gpui::white().opacity(FIELD_EDGE)
    }
}

pub(crate) fn danger_fill() -> Hsla {
    danger().opacity(DANGER_FILL)
}

const DANGER_FILL: f32 = 0.12;

/// A field: its own fill and edge, a stronger fill with the focus, red while what it holds is
/// refused.
pub fn field(focused: bool, invalid: bool) -> Div {
    let bg = if invalid {
        danger_fill()
    } else {
        field_bg(focused)
    };
    div()
        .relative()
        .flex()
        .items_center()
        .h(px(FIELD_HEIGHT))
        .px(px(16.))
        .overflow_hidden()
        .rounded(px(radius::ROW))
        .bg(bg)
        .border_1()
        .border_color(field_edge())
        .font_family(eclipse_theme::glass::FONT)
        .text_size(px(step::BODY.size))
}

pub fn log_well(id: &'static str, handle: &ScrollHandle) -> Stateful<Div> {
    let at = handle.clone();
    scroll_pane(id, handle)
        .on_scroll(move |ev: &ScrollEvent, window, cx| {
            let dy = ev.delta.pixel_delta(window.line_height()).y;
            let (offset, max) = (at.offset().y, at.max_offset().y);
            let at_end = (dy > px(0.) && offset >= px(0.)) || (dy < px(0.) && -offset >= max);
            if !at_end {
                cx.stop_propagation();
            }
        })
        .h(px(LOG_HEIGHT))
        .p(px(16.))
        .rounded(px(radius::ROW))
        .bg(well())
        .font_family(crate::theme::MONO)
        .text_size(px(step::CAPTION.size))
        .line_height(px(step::BODY.line))
}
