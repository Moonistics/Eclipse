use super::{
    button, color, copy_details, danger, entity_title, hand_back_at_ends, heading, icon, inset,
    light, look, menu_keys, modal_keys, nested_list, said, scroll_pane, section_title, text, tile,
    white, ButtonKind, ButtonSize, IconSize, Listener, Press, PANEL_INSET, TILE_LG, TILE_MD,
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

pub(super) const POPOVER_WIDTH: f32 = 280.;

const POPOVER_MAX_HEIGHT: f32 = 360.;

pub const MENU_GAP: f32 = space::SM;

const POPOVER_PAD: f32 = space::XS + space::XXS;

const CORNER_FLOAT_WIDTH: f32 = 440.;

const MORE_BELOW_FADE: f32 = 32.;

const MORE_BELOW_FADE_SHORT: f32 = 16.;

pub fn fade_height() -> f32 {
    if super::short_window() {
        MORE_BELOW_FADE_SHORT
    } else {
        MORE_BELOW_FADE
    }
}

const STATE_ACTIONS_GAP: f32 = space::XL;

const GROUP_HEAD: f32 = space::XXL + space::SM;

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

pub(super) fn backdrop(kind: Surface, radius: f32) -> AnyElement {
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

thread_local! {
    static FOOT: std::cell::Cell<f32> = const { std::cell::Cell::new(0.) };
    static LAST_FOOT: std::cell::Cell<f32> = const { std::cell::Cell::new(0.) };
}

pub(super) fn roll_foot() {
    LAST_FOOT.set(FOOT.take());
}

pub fn rule() -> Div {
    div().flex_none().h(px(super::ROW_EDGE)).bg(edge())
}

pub fn foot_card() -> Div {
    let lift = gpui::canvas(
        |bounds, window, _| {
            let under = window.viewport_size().height - bounds.top();
            FOOT.set(FOOT.get().max(f32::from(under)));
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full();
    let shade = if light() {
        color(eclipse_theme::glass::ON_LIGHT).opacity(FOOT_SHADE_LIGHT)
    } else {
        gpui::black().opacity(FOOT_SHADE)
    };
    card()
        .shadow(vec![gpui::BoxShadow {
            color: shade,
            offset: point(px(0.), px(-FOOT_LIFT)),
            blur_radius: px(FOOT_BLUR),
            spread_radius: px(0.),
            inset: false,
        }])
        .child(lift)
}

pub fn float_card() -> Div {
    div()
        .relative()
        .rounded(px(radius::POPOVER))
        .shadow_lg()
        .child(backdrop(Surface::Float, radius::POPOVER))
}

pub fn popover(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Stateful<Div> {
    menu_keys(
        nested_list(id)
            .role(Role::Menu)
            .aria_label(label.into())
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
        .bottom(px(space::XL.max(LAST_FOOT.get() + space::SM)))
        .max_w(px(CORNER_FLOAT_WIDTH))
        .child(content)
}

/// What a dialog or the palette lays over the launcher: GPUI can't blur what it drew, so a
/// menu's float covers the page. The scrim goes over it.
pub(super) fn scrim_backdrop() -> impl IntoElement {
    backdrop(Surface::Float, 0.)
}

/// The scrim over [`scrim_backdrop`]: lighter in the light appearance, so the dimmed launcher
/// stays day-lit.
pub(super) fn scrim() -> Hsla {
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

pub const DIALOG_BUTTON: ButtonSize = ButtonSize::Lg;

pub fn dialog_dismiss(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
) -> Stateful<Div> {
    button(id, None, label, ButtonKind::Ghost, DIALOG_BUTTON)
}

pub fn dialog_foot(dismiss: Stateful<Div>, main: Option<AnyElement>) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(space::MD))
        .child(dismiss)
        .children(main)
}

pub fn tab_panel(id: impl Into<SharedString>, tab: impl Into<SharedString>) -> Stateful<Div> {
    div()
        .id(id.into())
        .role(Role::TabPanel)
        .aria_label(tab.into())
}

pub fn dialog_colour() -> Hsla {
    let plate = surface_colour(Surface::Card);
    let plate = if light() { plate } else { plate.blend(scrim()) };
    plate.blend(white(fill::HOVER))
}

pub struct StateTitle {
    text: SharedString,
    lone: bool,
}

impl From<&'static str> for StateTitle {
    fn from(text: &'static str) -> Self {
        Self {
            text: text.into(),
            lone: false,
        }
    }
}

impl From<String> for StateTitle {
    fn from(text: String) -> Self {
        Self {
            text: text.into(),
            lone: false,
        }
    }
}

impl From<SharedString> for StateTitle {
    fn from(text: SharedString) -> Self {
        Self { text, lone: false }
    }
}

pub fn page_only(title: impl Into<SharedString>) -> StateTitle {
    StateTitle {
        text: title.into(),
        lone: true,
    }
}

impl StateTitle {
    fn glyph(&self, name: &'static str, tone: Hsla) -> Div {
        if self.lone {
            tile(TILE_LG).child(icon(name, IconSize::Px24, tone))
        } else {
            tile(TILE_MD).child(icon(name, IconSize::Px16, tone))
        }
    }
}

fn state(
    id: SharedString,
    glyph: &'static str,
    tone: Hsla,
    title: StateTitle,
    note: SharedString,
) -> Stateful<Div> {
    let (size, gap) = if title.lone {
        (pixel_text::CARD_TITLE, space::LG)
    } else {
        (pixel_text::PRESS, space::MD)
    };
    div()
        .id(id.clone())
        .relative()
        .flex()
        .flex_col()
        .items_center()
        .p(px(PANEL_INSET))
        .text_center()
        .child(title.glyph(glyph, tone))
        .child(
            heading(format!("{id}-title"), title.text.clone(), 2)
                .mt(px(gap))
                .child(
                    super::pixel_text(size)
                        .text_color(white(ink::FULL))
                        .child(title.text),
                ),
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
    title: impl Into<StateTitle>,
    note: impl Into<SharedString>,
    actions: Vec<Stateful<Div>>,
) -> Stateful<Div> {
    let note: SharedString = note.into();
    debug_assert!(!note.is_empty(), "an empty state says what to do next");
    let state = state(id.into(), icon_name, white(ink::MUTED), title.into(), note);
    self::actions(state, actions)
}

pub fn loading_state(id: impl Into<SharedString>, title: impl Into<StateTitle>) -> Stateful<Div> {
    super::keeps_retry_focus(state(
        id.into(),
        "clock",
        white(ink::MUTED),
        title.into(),
        SharedString::default(),
    ))
}

pub fn error_state(
    id: impl Into<SharedString>,
    title: impl Into<StateTitle>,
    error: &str,
    on_retry: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    error_state_with(id, title, error, on_retry, Vec::new(), ButtonKind::Primary)
}

pub fn error_state_with(
    id: impl Into<SharedString>,
    title: impl Into<StateTitle>,
    error: &str,
    on_retry: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    extra: Vec<Stateful<Div>>,
    retry: ButtonKind,
) -> Stateful<Div> {
    let cause = told_cause(error).unwrap_or_default();
    error_state_told(id, title, cause, error, on_retry, extra, retry)
}

pub fn error_state_told(
    id: impl Into<SharedString>,
    title: impl Into<StateTitle>,
    cause: impl Into<SharedString>,
    error: &str,
    on_retry: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    extra: Vec<Stateful<Div>>,
    retry: ButtonKind,
) -> Stateful<Div> {
    let again = crate::tr("Try again");
    error_state_again(id, title, cause, error, again, on_retry, extra, retry)
}

#[allow(clippy::too_many_arguments)]
pub fn error_state_again(
    id: impl Into<SharedString>,
    title: impl Into<StateTitle>,
    cause: impl Into<SharedString>,
    error: &str,
    again: impl Into<SharedString>,
    on_retry: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    extra: Vec<Stateful<Div>>,
    retry: ButtonKind,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let retry_id = format!("{id}-retry");
    super::retry_back(&retry_id);
    let retry = super::any_button(
        retry_id.clone(),
        Some(crate::icons::RETRY),
        again,
        retry,
        ButtonSize::Md,
    )
    .on_press(move |event, window, cx| {
        super::retrying(&retry_id);
        on_retry(event, window, cx);
    });
    let copy = copy_details(format!("{id}-copy"), error.to_owned(), ButtonSize::Md);
    let state = state(id, "alert-circle", danger(), title.into(), cause.into()).role(Role::Alert);
    let buttons = std::iter::once(retry).chain(extra).chain([copy]);
    actions(state, buttons.collect())
}

pub fn plain_error(error: &str) -> &str {
    error.lines().next().unwrap_or_default()
}

pub fn told_cause(error: &str) -> Option<String> {
    let line = plain_error(error);
    if line.ends_with('.') {
        return Some(line.to_string());
    }
    match line.split_once(": ") {
        Some((said, _)) if said.starts_with(char::is_uppercase) => Some(format!("{said}.")),
        _ => None,
    }
}

pub fn empty_card(state: Stateful<Div>) -> Div {
    card().p(px(PANEL_INSET)).child(state)
}

pub fn empty(id: impl Into<gpui::ElementId>, text: impl Into<SharedString>) -> Div {
    empty_line(id, text).py(px(space::XL))
}

pub fn empty_line(id: impl Into<gpui::ElementId>, text: impl Into<SharedString>) -> Div {
    empty_style().child(said(id, text))
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
        .child(group_head(id, title, aside).mb(px(space::MD)))
        .child(div().relative().child(body))
}

pub fn group_head(
    id: impl Into<SharedString>,
    title: impl Into<SharedString>,
    aside: Option<AnyElement>,
) -> Div {
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap_x(px(space::LG))
        .gap_y(px(space::SM))
        .min_h(px(GROUP_HEAD))
        .child(section_title(id, title))
        .children(aside)
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

pub fn setting_card(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Stateful<Div> {
    card()
        .id(id.into())
        .role(Role::Group)
        .aria_label(name.into())
        .flex()
        .flex_col()
        .p(px(super::LIST_INSET))
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

const SCROLLED_SLACK: f32 = 1.;

pub fn scrolled(pane: &ScrollHandle) -> bool {
    pane.offset().y < px(-SCROLLED_SLACK)
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
    let more = -pane.offset().y < pane.max_offset().y - px(SCROLLED_SLACK);
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
pub(super) fn focus_ring(radius: f32) -> Div {
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .rounded(px(radius))
        .border_2()
        .border_color(color(look().accent_soft))
}

pub(super) fn field_bg(focused: bool) -> Hsla {
    if light() {
        color(PAPER).opacity(if focused { 1. } else { fill::CONTROL_LIGHT })
    } else {
        color(if focused {
            eclipse_theme::glass::FIELD_BG_FOCUS
        } else {
            eclipse_theme::glass::FIELD_BG
        })
    }
}

pub(super) fn field_edge() -> Hsla {
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
    let (pad, text) = (size.field_pad(), size.field_text());
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
        .font_family(crate::theme::FONT)
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
