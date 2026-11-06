use super::{
    button, color, copy_details, danger, entity_title, hand_back_at_ends, heading, icon, inset,
    light, look, menu_keys, modal_keys, nested_list, said, scroll_pane, section_title, text, tile,
    white, ButtonKind, ButtonSize, IconSize, Listener, Press, PANEL_INSET, TILE_LG,
};
use crate::scrollbar::Scrollbar;
use eclipse_theme::glass::{
    fill, ink, pixel_text, radius, space, step, EDGE, EDGE_LIGHT, FIELD_EDGE, FIELD_EDGE_LIGHT,
    FLOAT, PAPER,
};
use gpui::{
    anchored, deferred, div, point, prelude::*, px, AnyElement, App, ClickEvent, Div, FontWeight,
    Hsla, MouseDownEvent, Pixels, Point, Role, ScrollHandle, SharedString, Stateful, Window,
};

pub const POPOVER_WIDTH: f32 = 280.;

const POPOVER_MAX_HEIGHT: f32 = 360.;

pub const MENU_GAP: f32 = space::SM;

const POPOVER_PAD: f32 = space::XS + space::XXS;

const CORNER_FLOAT_WIDTH: f32 = 440.;

const MORE_BELOW_FADE: f32 = 32.;

const MORE_BELOW_FADE_SHORT: f32 = 16.;

pub const SCROLL_FADE: f32 = MORE_BELOW_FADE;

fn fade_height() -> f32 {
    if super::short_window() {
        MORE_BELOW_FADE_SHORT
    } else {
        MORE_BELOW_FADE
    }
}

const STATE_ACTIONS_GAP: f32 = space::XL;

const DIALOG_NARROW: f32 = 520.;

const DIALOG_WIDE: f32 = 680.;

const DIALOG_PADDING: f32 = space::XXL;

const SPLIT_DIALOG_MAX: (f32, f32) = (1180., 840.);

const SCRIM_SHADE: f32 = 0.5;

const SCRIM_SHADE_LIGHT: f32 = 0.2;

const LOG_HEIGHT: f32 = 320.;

const FOOT_LIFT: f32 = space::SM;

const FOOT_BLUR: f32 = space::XL;

const FOOT_SHADE: f32 = 0.45;

const FOOT_SHADE_LIGHT: f32 = 0.1;

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
        .border_color(edge())
        .into_any_element()
}

pub fn edge() -> Hsla {
    white(if light() { EDGE_LIGHT } else { EDGE })
}

pub fn card() -> Div {
    div()
        .relative()
        .rounded(px(radius::PANEL))
        .child(backdrop(Surface::Card, radius::PANEL))
}

pub fn foot_card() -> Div {
    let shade = if light() {
        color(eclipse_theme::glass::ON_LIGHT).opacity(FOOT_SHADE_LIGHT)
    } else {
        gpui::black().opacity(FOOT_SHADE)
    };
    card().shadow(vec![gpui::BoxShadow {
        color: shade,
        offset: point(px(0.), px(-FOOT_LIFT)),
        blur_radius: px(FOOT_BLUR),
        spread_radius: px(0.),
        inset: false,
    }])
}

pub fn float_card() -> Div {
    div()
        .relative()
        .rounded(px(radius::POPOVER))
        .shadow_lg()
        .child(backdrop(Surface::Float, radius::POPOVER))
}

pub fn popover(id: impl Into<SharedString>) -> Stateful<Div> {
    menu_keys(
        nested_list(id)
            .role(Role::Menu)
            .rounded(px(radius::POPOVER))
            .bg(surface_colour(Surface::Float))
            .border_1()
            .border_color(edge())
            .shadow_lg()
            .w(px(POPOVER_WIDTH))
            .max_h(px(POPOVER_MAX_HEIGHT))
            .p(px(POPOVER_PAD))
            .flex()
            .flex_col()
            .occlude(),
    )
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Align {
    Start,
    End,
}

pub fn menu_under(opener_height: f32, align: Align, menu: Stateful<Div>) -> Div {
    let (spot, anchor) = match align {
        Align::Start => (div().absolute().top_0().left_0(), gpui::Anchor::TopLeft),
        Align::End => (div().absolute().top_0().right_0(), gpui::Anchor::TopRight),
    };
    spot.child(
        deferred(
            anchored()
                .anchor(anchor)
                .offset(point(px(0.), px(opener_height + MENU_GAP)))
                .child(div().pb(px(MENU_GAP)).child(super::with_scrollbar(menu))),
        )
        .with_priority(1),
    )
}

pub fn menu_beside(at: Point<Pixels>, menu: Stateful<Div>) -> impl IntoElement {
    deferred(
        anchored()
            .position(at)
            .snap_to_window_with_margin(px(MENU_GAP))
            .child(super::with_scrollbar(menu)),
    )
    .with_priority(1)
}

pub fn modal(
    id: impl Into<SharedString>,
    title: impl Into<SharedString>,
    dismiss: Option<Listener<MouseDownEvent>>,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let shade = div().absolute().top_0().left_0().size_full().bg(scrim());
    let shade = match dismiss {
        Some(dismiss) => shade.on_mouse_down(gpui::MouseButton::Left, dismiss),
        None => shade,
    };
    modal_keys(
        div()
            .id(id.clone())
            .role(Role::Dialog)
            .aria_label(title.into())
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .occlude()
            .on_scroll(|_, _, cx| cx.stop_propagation())
            .child(scrim_backdrop())
            .child(shade),
        id,
    )
}

pub fn sheet(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Stateful<Div> {
    let id: SharedString = id.into();
    modal_keys(
        div()
            .id(id.clone())
            .role(Role::Dialog)
            .aria_label(title.into())
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(surface_colour(Surface::Float))
            .occlude()
            .on_scroll(|_, _, cx| cx.stop_propagation()),
        id,
    )
}

pub fn corner_float(content: impl IntoElement) -> Div {
    div()
        .absolute()
        .right(px(space::XL))
        .bottom(px(space::XL))
        .max_w(px(CORNER_FLOAT_WIDTH))
        .child(content)
}

/// What a dialog or the palette lays over the launcher: GPUI can't blur what it drew, so a
/// menu's float covers the page. The scrim goes over it.
pub fn scrim_backdrop() -> impl IntoElement {
    backdrop(Surface::Float, 0.)
}

/// The scrim over [`scrim_backdrop`]: lighter in the light appearance, so the dimmed launcher
/// stays day-lit.
pub fn scrim() -> Hsla {
    color(eclipse_theme::glass::SCRIM).opacity(if light() {
        SCRIM_SHADE_LIGHT
    } else {
        SCRIM_SHADE
    })
}

/// A dialog's plate: its piece of the scene under the scrim's dimming, then the plates' white.
/// The content goes after it.
pub fn dialog_plate() -> Div {
    let layer = |c: Hsla| {
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .rounded(px(radius::MODAL))
            .bg(c)
    };
    div()
        .relative()
        .rounded(px(radius::MODAL))
        .child(backdrop(Surface::Card, radius::MODAL))
        // Light plates are already day-lit: no dimming under them.
        .child(layer(
            color(eclipse_theme::glass::SCRIM).opacity(if light() { 0. } else { SCRIM_SHADE }),
        ))
        .child(layer(white(fill::HOVER)))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DialogWidth {
    Narrow,
    Wide,
}

pub fn dialog(
    id: impl Into<SharedString>,
    eyebrow: Option<SharedString>,
    title: impl Into<SharedString>,
    width: DialogWidth,
    body: impl IntoElement,
    foot: Vec<AnyElement>,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    dialog_plate()
        .id(id.clone())
        .occlude()
        .flex()
        .flex_col()
        .w(px(match width {
            DialogWidth::Narrow => DIALOG_NARROW,
            DialogWidth::Wide => DIALOG_WIDE,
        }))
        .max_w_full()
        .max_h_full()
        .children(dialog_column(id, eyebrow, title.into(), body, foot))
}

pub fn split_dialog(
    id: impl Into<SharedString>,
    title: impl Into<SharedString>,
    aside: Option<AnyElement>,
    body: impl IntoElement,
    foot: Vec<AnyElement>,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    dialog_plate()
        .id(id.clone())
        .occlude()
        .flex()
        .w_full()
        .max_w(px(SPLIT_DIALOG_MAX.0))
        .h_full()
        .max_h(px(SPLIT_DIALOG_MAX.1))
        .children(aside.map(|aside| {
            div()
                .relative()
                .flex()
                .flex_none()
                .h_full()
                .rounded_l(px(radius::MODAL))
                .bg(inset())
                .child(aside)
        }))
        .child(
            div()
                .relative()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .children(dialog_column(id, None, title.into(), body, foot)),
        )
}

fn dialog_column(
    id: SharedString,
    eyebrow: Option<SharedString>,
    title: SharedString,
    body: impl IntoElement,
    foot: Vec<AnyElement>,
) -> [AnyElement; 3] {
    let head = div()
        .relative()
        .flex()
        .flex_col()
        .flex_none()
        .gap(px(space::SM))
        .px(px(DIALOG_PADDING))
        .pt(px(DIALOG_PADDING))
        .children(eyebrow.map(super::label))
        .child(
            heading(format!("{id}-title"), title.clone(), 2)
                .child(entity_title(title, pixel_text::CARD_TITLE)),
        );
    let body = div()
        .relative()
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .px(px(DIALOG_PADDING))
        .pt(px(space::MD))
        .pb(px(if foot.is_empty() {
            DIALOG_PADDING
        } else {
            space::LG
        }))
        .child(body);
    let foot = div()
        .relative()
        .flex()
        .flex_none()
        .flex_wrap()
        .items_center()
        .justify_end()
        .gap(px(space::MD))
        .when(!foot.is_empty(), |d| {
            d.px(px(DIALOG_PADDING)).pb(px(DIALOG_PADDING))
        })
        .children(foot);
    [
        head.into_any_element(),
        body.into_any_element(),
        foot.into_any_element(),
    ]
}

pub fn dialog_colour() -> Hsla {
    let plate = surface_colour(Surface::Card);
    let plate = if light() { plate } else { plate.blend(scrim()) };
    plate.blend(white(fill::HOVER))
}

fn state(id: SharedString, glyph: Div, title: SharedString, note: SharedString) -> Stateful<Div> {
    div()
        .id(id.clone())
        .relative()
        .flex()
        .flex_col()
        .items_center()
        .p(px(PANEL_INSET))
        .text_center()
        .child(glyph)
        .child(
            heading(format!("{id}-title"), title.clone(), 2)
                .mt(px(space::LG))
                .child(entity_title(title, pixel_text::CARD_TITLE)),
        )
        .when(!note.is_empty(), |d| {
            d.child(
                empty_style()
                    .mt(px(space::SM))
                    .max_w(px(crate::layout::MEASURE_NOTE))
                    .child(said(format!("{id}-note"), note)),
            )
        })
}

fn actions(d: Stateful<Div>, actions: Vec<Stateful<Div>>) -> Stateful<Div> {
    d.when(!actions.is_empty(), |d| {
        d.child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .justify_center()
                .gap(px(space::SM))
                .mt(px(STATE_ACTIONS_GAP))
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
    let note: SharedString = note.into();
    debug_assert!(!note.is_empty(), "an empty state says what to do next");
    let glyph = tile(TILE_LG).child(icon(icon_name, IconSize::Px24, white(ink::MUTED)));
    self::actions(state(id.into(), glyph, title.into(), note), actions)
}

pub fn loading_state(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Stateful<Div> {
    let glyph = tile(TILE_LG).child(icon("clock", IconSize::Px24, white(ink::MUTED)));
    state(id.into(), glyph, title.into(), SharedString::default())
}

pub fn error_state(
    id: impl Into<SharedString>,
    title: impl Into<SharedString>,
    error: &str,
    on_retry: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    error_state_with(id, title, error, on_retry, Vec::new(), ButtonKind::Primary)
}

pub fn error_state_with(
    id: impl Into<SharedString>,
    title: impl Into<SharedString>,
    error: &str,
    on_retry: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    extra: Vec<Stateful<Div>>,
    retry: ButtonKind,
) -> Stateful<Div> {
    error_state_told(id, title, told_cause(error), error, on_retry, extra, retry)
}

pub fn error_state_told(
    id: impl Into<SharedString>,
    title: impl Into<SharedString>,
    cause: impl Into<SharedString>,
    error: &str,
    on_retry: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    extra: Vec<Stateful<Div>>,
    retry: ButtonKind,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let glyph = tile(TILE_LG).child(icon("alert-circle", IconSize::Px24, danger()));
    let retry = button(
        format!("{id}-retry"),
        Some("refresh"),
        crate::tr("Try again"),
        retry,
        ButtonSize::Md,
    )
    .on_press(on_retry);
    let copy = copy_details(format!("{id}-copy"), error.to_owned(), ButtonSize::Md);
    actions(
        state(id, glyph, title.into(), cause.into()),
        std::iter::once(retry).chain(extra).chain([copy]).collect(),
    )
}

pub fn plain_error(error: &str) -> &str {
    error.lines().next().unwrap_or_default()
}

pub fn told_cause(error: &str) -> String {
    let line = plain_error(error);
    if line.ends_with('.') {
        return line.to_string();
    }
    match line.split_once(": ") {
        Some((said, _)) if said.starts_with(char::is_uppercase) => format!("{said}."),
        _ => crate::tr("Copy the details to report it.").to_string(),
    }
}

pub fn empty_card(state: Stateful<Div>) -> Div {
    card().p(px(PANEL_INSET)).child(state)
}

/// An empty state inside an area that is already a card; the message goes in as a child.
pub fn empty() -> Div {
    empty_style().py(px(space::XL))
}

fn empty_style() -> Div {
    text(step::BODY, FontWeight::NORMAL, ink::MUTED)
}

/// A group of fields: its heading, anything `aside` at its right, then `body`.
pub fn field_group(
    id: impl Into<SharedString>,
    title: impl Into<SharedString>,
    aside: Option<AnyElement>,
    body: impl IntoElement,
) -> Div {
    div()
        .relative()
        .flex()
        .flex_col()
        .flex_none()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(space::LG))
                .h(px(ButtonSize::Md.height()))
                .mb(px(space::MD))
                .child(section_title(id, title))
                .children(aside),
        )
        .child(div().relative().child(body))
}

pub fn labelled_field(label: impl Into<SharedString>, field: impl IntoElement) -> Div {
    div()
        .relative()
        .flex()
        .flex_col()
        .gap(px(space::SM))
        .child(super::label(label))
        .child(field)
}

pub fn section_card(
    id: impl Into<SharedString>,
    title: impl Into<SharedString>,
    aside: Option<AnyElement>,
    body: impl IntoElement,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    card()
        .id(id.clone())
        .flex()
        .flex_col()
        .flex_none()
        .p(px(super::CARD_PADDING))
        .child(field_group(format!("{id}-title"), title, aside, body))
}

pub fn setting_card(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Stateful<Div> {
    let id: SharedString = id.into();
    card()
        .id(id.clone())
        .flex()
        .flex_col()
        .p(px(super::LIST_INSET))
        .child(
            section_title(format!("{id}-title"), title)
                .px(px(super::SETTING_ROW_PADDING))
                .pt(px(super::SETTING_ROW_PADDING)),
        )
}

pub fn list_box(id: impl Into<SharedString>, max: f32) -> Stateful<Div> {
    nested_list(id)
        .flex()
        .flex_col()
        .max_h(px(max))
        .p(px(POPOVER_PAD))
        .pr(px(Scrollbar::ROOM))
        .rounded(px(radius::POPOVER))
        .border_1()
        .border_color(edge())
}

pub fn scrolled(pane: &ScrollHandle) -> bool {
    pane.offset().y < px(-1.)
}

pub fn more_above_fade(pane: &ScrollHandle, under: Hsla) -> Option<Div> {
    scrolled(pane).then(|| {
        div()
            .absolute()
            .left_0()
            .right_0()
            .top_0()
            .h(px(fade_height()))
            .bg(gpui::linear_gradient(
                180.,
                gpui::linear_color_stop(under, 0.),
                gpui::linear_color_stop(under.opacity(0.), 1.),
            ))
    })
}

pub fn page_pane(pane: Stateful<Div>, bar: &Scrollbar, reach: f32) -> Div {
    div()
        .relative()
        .flex()
        .flex_col()
        .h_full()
        .min_h_0()
        .mr(px(-reach))
        .child(bar.wrap(pane.pr(px(reach))).size_full())
}

pub fn more_below_fade(pane: &ScrollHandle, under: Hsla) -> Option<Div> {
    let more = -pane.offset().y < pane.max_offset().y - px(1.);
    more.then(|| {
        div()
            .absolute()
            .left_0()
            .right_0()
            .bottom_0()
            .h(px(fade_height()))
            .bg(gpui::linear_gradient(
                180.,
                gpui::linear_color_stop(under.opacity(0.), 0.),
                gpui::linear_color_stop(under, 1.),
            ))
    })
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
    danger().opacity(fill::DANGER)
}

/// A field at `size`: its own fill and edge, a stronger fill and the focus ring with the
/// focus, a red fill and edge while what it holds is refused.
pub fn field(focused: bool, invalid: bool, size: ButtonSize) -> Div {
    let large = matches!(size, ButtonSize::Lg | ButtonSize::Hero);
    let (pad, text) = if large {
        (space::LG, step::LEAD)
    } else {
        (space::MD, step::BODY)
    };
    div()
        .relative()
        .flex()
        .items_center()
        .gap(px(space::SM))
        .h(px(size.height()))
        .px(px(pad))
        .overflow_hidden()
        .rounded(px(radius::CONTROL))
        .bg(if invalid {
            danger_fill()
        } else {
            field_bg(focused)
        })
        .border_1()
        .border_color(if invalid { danger() } else { field_edge() })
        .font_family(eclipse_theme::glass::FONT)
        .text_size(px(text.size))
        .when(focused, |d| d.child(focus_ring(radius::CONTROL)))
}

pub fn field_icon(name: &'static str) -> AnyElement {
    icon(name, IconSize::Px16, white(ink::MUTED)).into_any_element()
}

pub fn log_well(id: impl Into<SharedString>, handle: &ScrollHandle) -> Stateful<Div> {
    hand_back_at_ends(scroll_pane(id, handle), handle)
        .h(px(LOG_HEIGHT))
        .p(px(space::LG))
        .rounded(px(radius::ROW))
        .bg(inset())
        .font_family(crate::theme::MONO)
        .text_size(px(step::CAPTION.size))
        .line_height(px(step::BODY.line))
}

pub fn well() -> Div {
    div()
        .relative()
        .p(px(PANEL_INSET))
        .rounded(px(radius::ROW))
        .bg(inset())
}

pub fn code_well(code: impl Into<SharedString>) -> Div {
    well().child(super::mono(step::CAPTION, ink::BODY).child(code.into()))
}
