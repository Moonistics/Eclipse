//! The launcher's shared controls: white text and fills at the `eclipse_theme::glass` opacities
//! over [`glass`](crate::glass) plates, Geist for text and values, Monocraft for titles and
//! pressables. The look state for each frame (accent, light appearance) lives here.
use crate::{hover_fade, hover_is, keyboard, note_hover, theme::Theme, track_hover, Kind};
use eclipse_theme::glass::{fill, ink, radius, Glass};
use gpui::{
    div, prelude::*, px, rgb, rgba, svg, AnyElement, App, ClickEvent, Div, FontWeight, Hsla,
    SharedString, Stateful, Svg, Window,
};
use std::cell::Cell;

thread_local! {
    /// The look's colours for the player's accent.
    static LOOK: Cell<Glass> = Cell::new(Glass::default_look());
    /// Whether the rows being built go in a Settings card ([`flat_rows`]).
    static FLAT: Cell<bool> = const { Cell::new(false) };
    /// The light appearance: the white ink and fills turn to [`INK`].
    static LIGHT: Cell<bool> = const { Cell::new(false) };
}

/// The light appearance's ink, at the opacities the dark look gives white.
const INK: u32 = eclipse_theme::glass::ON_LIGHT;

pub const PIXEL_TEXT: f32 = 13.;

pub const NAV_TEXT: f32 = 18.;

const FIELD_EDGE: f32 = 0.1;

pub const HOVER_OPACITY: f32 = 0.92;

/// Set as each frame's pages start being built: whether the appearance is light.
pub fn set_light(light: bool) {
    LIGHT.set(light);
}

/// Whether the light appearance is drawn.
pub fn light() -> bool {
    LIGHT.get()
}

/// A solid control's fill (the sliders' knob): white, or the ink in light.
pub fn paper() -> Hsla {
    if light() {
        color(INK)
    } else {
        gpui::white()
    }
}

/// The look's green (`--live`): darker in light, to read on white plates.
pub fn live() -> Hsla {
    use eclipse_theme::glass::{LIVE, LIVE_LIGHT};
    color(if light() { LIVE_LIGHT } else { LIVE })
}

/// The look's warning red-orange (`--warn`).
pub fn warn() -> Hsla {
    use eclipse_theme::glass::{WARN, WARN_LIGHT};
    color(if light() { WARN_LIGHT } else { WARN })
}

/// The look's red (`--danger`).
pub fn danger() -> Hsla {
    use eclipse_theme::glass::{DANGER, DANGER_LIGHT};
    color(if light() { DANGER_LIGHT } else { DANGER })
}

/// Builds `f`'s rows as Settings rows, for a Settings card: `index_row` and `section_title` ask
/// [`flat`].
pub fn flat_rows<R>(f: impl FnOnce() -> R) -> R {
    let was = FLAT.replace(true);
    let built = f();
    FLAT.set(was);
    built
}

/// Whether rows are being built for a Settings card.
pub fn flat() -> bool {
    FLAT.get()
}

/// A Settings row: the name over its note, the control on the right.
pub fn setting_row(
    title: impl IntoElement,
    note: Option<gpui::AnyElement>,
    control: Option<gpui::AnyElement>,
) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(40.))
        .px(px(16.))
        .py(px(16.))
        .rounded(px(radius::ROW))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .gap(px(4.))
                .child(
                    text(15., FontWeight::SEMIBOLD, ink::FULL)
                        .line_height(px(21.))
                        .child(title),
                )
                .children(note.map(|n| {
                    text(13., FontWeight::NORMAL, ink::MUTED)
                        .line_height(px(19.5))
                        .max_w(px(560.))
                        .child(n)
                })),
        )
        .children(control.map(|c| div().flex().flex_none().items_center().gap(px(8.)).child(c)))
}

/// A stacked Settings row: the name and note, the control under them at full width.
pub fn setting_row_stacked(
    title: impl IntoElement,
    note: Option<gpui::AnyElement>,
    control: gpui::AnyElement,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(16.))
        .px(px(16.))
        .py(px(16.))
        .rounded(px(radius::ROW))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    text(15., FontWeight::SEMIBOLD, ink::FULL)
                        .line_height(px(21.))
                        .child(title),
                )
                .children(note.map(|n| {
                    text(13., FontWeight::NORMAL, ink::MUTED)
                        .line_height(px(19.5))
                        .max_w(px(580.))
                        .child(n)
                })),
        )
        .child(div().w_full().child(control))
}

/// [`field`]'s look on a field built elsewhere (a line edit with its own handlers), up to
/// 560 px wide.
pub fn wide_field(field: Stateful<Div>, focused: bool) -> Stateful<Div> {
    field
        .w_full()
        .max_w(px(560.))
        .h(px(44.))
        .px(px(16.))
        .rounded(px(radius::ROW))
        .bg(field_bg(focused))
        .border_1()
        .border_color(field_edge())
        .font_family(eclipse_theme::glass::FONT)
        .text_size(px(14.))
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

/// Set as each frame's pages start being built: the colours for the player's accent.
pub fn set_look(look: Glass) {
    LOOK.set(look);
}

/// The look's colours.
pub fn look() -> Glass {
    LOOK.get()
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
    white(FIELD_EDGE)
}

pub fn bevel(d: Div) -> Div {
    d.child(
        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .h(px(2.))
            .bg(gpui::white().opacity(0.3)),
    )
    .child(
        div()
            .absolute()
            .bottom_0()
            .left_0()
            .right_0()
            .h(px(3.))
            .bg(gpui::black().opacity(0.22)),
    )
}

pub fn label_text(size: f32) -> Div {
    div()
        .font_family(eclipse_theme::glass::LABEL_FONT)
        .text_size(px(size))
        .line_height(px((size * 1.25).round()))
}

pub fn label(text: impl Into<SharedString>) -> Div {
    let text: SharedString = text.into();
    self::text(11., FontWeight::SEMIBOLD, ink::DIM)
        .line_height(px(14.))
        .child(text.to_uppercase())
}

pub const SIDEBAR_INSET: f32 = 20.;

pub const SIDEBAR_PAD: f32 = 10.;

pub fn nav_row(
    id: impl Into<SharedString>,
    icon_name: &'static str,
    text: impl Into<SharedString>,
    current: bool,
    t: &Theme,
) -> Stateful<Div> {
    let id = id.into();
    let text: SharedString = text.into();
    let tone = if current { ink::FULL } else { ink::BODY };
    let d = div()
        .id(id.clone())
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(12.))
        .h(px(40.))
        .px(px(SIDEBAR_INSET - SIDEBAR_PAD - 1.))
        .rounded(px(radius::CONTROL))
        .cursor_pointer()
        .text_color(white(tone))
        .border_1()
        .when(current, |d| {
            d.bg(white(fill::SELECTED))
                .border_color(white(eclipse_theme::glass::EDGE))
        })
        .when(!current, |d| {
            d.border_color(gpui::transparent_black())
                .child(hover_fill(&id, radius::CONTROL, 0., fill::HOVER))
                .hover(|s| s.text_color(white(ink::FULL)))
        })
        .child(
            icon(
                icon_name,
                18.,
                if current {
                    color(look().accent)
                } else {
                    white(tone)
                },
            )
            .relative(),
        )
        .child(label_text(NAV_TEXT).relative().child(text.to_uppercase()));
    keyboard(track_hover(d, id), t)
}

fn empty_text(message: SharedString) -> Div {
    empty_style().relative().max_w(px(520.)).child(message)
}

pub fn empty_card(message: impl Into<SharedString>) -> Div {
    crate::glass::plate(radius::PANEL)
        .flex()
        .flex_col()
        .items_start()
        .gap(px(16.))
        .p(px(24.))
        .child(empty_text(message.into()))
}

pub fn card_title(title: impl Into<SharedString>) -> Div {
    text(16., FontWeight::SEMIBOLD, ink::FULL)
        .line_height(px(22.))
        .child(title.into())
}

pub fn tile(size: f32) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(size))
        .rounded(px(radius::ROW))
        .bg(white(fill::BUTTON))
        .border_1()
        .border_color(white(eclipse_theme::glass::EDGE))
}

pub struct Tip(SharedString);

impl gpui::Render for Tip {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .px(px(8.))
            .py(px(4.))
            .rounded(px(radius::CONTROL))
            .bg(color(eclipse_theme::glass::FLOAT))
            .border_1()
            .border_color(white(eclipse_theme::glass::EDGE))
            .font_family(eclipse_theme::glass::FONT)
            .text_size(px(12.))
            .text_color(white(ink::FULL))
            .child(self.0.clone())
    }
}

pub fn tip(
    text: impl Into<SharedString>,
) -> impl Fn(&mut Window, &mut App) -> gpui::AnyView + 'static {
    let text: SharedString = text.into();
    move |_, cx| cx.new(|_| Tip(text.clone())).into()
}

/// White at `a`: the dark look's ink and fills. In light, the ink at `a`, so every part keeps
/// its weight on the light plates.
pub fn white(a: f32) -> Hsla {
    if light() {
        color(INK).opacity(a)
    } else {
        gpui::white().opacity(a)
    }
}

/// A colour of the look (`0xrrggbb`).
pub fn color(c: u32) -> Hsla {
    rgb(c).into()
}

/// Text at `size` px in Geist, white at `a`, at `weight`.
pub fn text(size: f32, weight: FontWeight, a: f32) -> Div {
    div()
        .font_family(eclipse_theme::glass::FONT)
        .text_size(px(size))
        .line_height(px((size * 1.4).round()))
        .font_weight(weight)
        .text_color(white(a))
}

pub fn icon(name: &'static str, size: f32, color: Hsla) -> Svg {
    svg()
        .path(format!("icons/glass/{name}.svg"))
        .flex_none()
        .size(px(icon_grid(size)))
        .text_color(color)
}

pub fn icon_grid(size: f32) -> f32 {
    if size <= 24. {
        16.
    } else {
        (size / 16.).round().max(2.) * 16.
    }
}

/// A layer filling its parent, round at `r`, for hover fills.
fn layer(r: f32) -> impl Fn() -> Div {
    move || div().absolute().top_0().left_0().size_full().rounded(px(r))
}

/// `id`'s fill eased between `from` (white at that opacity) and `to` while hovered.
fn hover_fill(id: &SharedString, r: f32, from: f32, to: f32) -> gpui::AnyElement {
    hover_fade(id, layer(r), move |d, t| d.bg(white((to - from) * t)))
}

/// The state dot before a row's second line: live green, the warning colour for busy and for a
/// warning, the accent's soft shade (given) for an update, dim for nothing yet.
#[derive(Clone, Copy, PartialEq)]
pub enum Tone {
    Live,
    Busy,
    Warn,
    Accent(u32),
    Off,
}

pub fn dot(tone: Tone) -> Div {
    let c = match tone {
        Tone::Live => live(),
        Tone::Busy | Tone::Warn => warn(),
        Tone::Accent(soft) => color(soft),
        Tone::Off => white(ink::DIM),
    };
    div().flex_none().size(px(7.)).rounded_full().bg(c)
}

/// The progress bar: a thin round track filled in the accent's soft shade.
pub fn bar(value: f32, accent_soft: u32) -> Div {
    div()
        .h(px(4.))
        .w_full()
        .rounded_full()
        .overflow_hidden()
        .bg(white(fill::ACTIVE))
        .child(
            div()
                .h_full()
                .rounded_full()
                .w(gpui::relative(value.clamp(0., 1.)))
                .bg(color(accent_soft)),
        )
}

/// A sidebar row (an instance, a server): filled when it is the one shown, faintly under the
/// pointer. Its icon and lines go in as children; `pr` leaves room for a button laid over its
/// right end.
pub fn row(id: impl Into<SharedString>, current: bool, pr: f32, t: &Theme) -> Stateful<Div> {
    let id = id.into();
    let d = div()
        .id(id.clone())
        .relative()
        .flex()
        .items_center()
        .gap(px(12.))
        .w_full()
        .p(px(8.))
        .pr(px(pr))
        .rounded(px(radius::ROW))
        .cursor_pointer();
    let d = if current {
        d.bg(white(fill::SELECTED))
    } else {
        d.child(hover_fill(&id, radius::ROW, 0., fill::HOVER))
    };
    keyboard(track_hover(d, id), t)
}

/// A row's two lines: the name and its state after a dot.
pub fn row_text(name: impl Into<SharedString>, state: (Tone, String)) -> Div {
    div()
        .relative()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .child(
            text(14., FontWeight::SEMIBOLD, ink::FULL)
                .line_height(px(21.))
                .truncate()
                .child(name.into()),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .min_w_0()
                .child(dot(state.0))
                .child(
                    text(12.5, FontWeight::NORMAL, ink::MUTED)
                        .line_height(px(18.75))
                        .truncate()
                        .child(state.1),
                ),
        )
}

/// A square button with one icon: muted, filled under the pointer and while `active`.
pub fn icon_button(
    id: impl Into<SharedString>,
    name: &'static str,
    size: f32,
    icon_size: f32,
    r: f32,
    active: bool,
    t: &Theme,
) -> Stateful<Div> {
    let id = id.into();
    let group = SharedString::from(format!("{id}-group"));
    let d = div()
        .id(id.clone())
        .group(group.clone())
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(size))
        .rounded(px(r))
        .cursor_pointer();
    let d = if active {
        d.bg(white(fill::ACTIVE))
    } else {
        d.child(hover_fill(&id, r, 0., fill::BUTTON))
    };
    let glyph = icon(
        name,
        icon_size,
        white(if active { ink::FULL } else { ink::MUTED }),
    )
    .relative()
    .group_hover(group, |s| s.text_color(white(ink::FULL)));
    keyboard(track_hover(d.child(glyph), id), t)
}

pub fn tab(
    id: impl Into<SharedString>,
    text: impl Into<SharedString>,
    open: bool,
    t: &Theme,
) -> Stateful<Div> {
    let id = id.into();
    let d = div()
        .id(id.clone())
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.))
        .h(px(Size::Md.height()))
        .px(px(12.))
        .cursor_pointer()
        .font_family(eclipse_theme::glass::LABEL_FONT)
        .text_size(px(PIXEL_TEXT))
        .line_height(px(16.));
    let d = if open {
        d.text_color(white(ink::FULL)).child(
            div()
                .absolute()
                .left(px(12.))
                .right(px(12.))
                .bottom_0()
                .h(px(2.))
                .bg(color(look().accent)),
        )
    } else {
        d.text_color(white(ink::MUTED))
            .hover(|s| s.text_color(white(ink::FULL)))
    };
    let text: SharedString = text.into();
    let d = d.when(!text.is_empty(), |d| d.child(text.to_uppercase()));
    keyboard(track_hover(d, id), t)
}

/// A tab with an icon before its label and a count after it.
pub fn count_tab(
    id: impl Into<SharedString>,
    icon_name: Option<&'static str>,
    label: impl Into<SharedString>,
    count: Option<usize>,
    open: bool,
    t: &Theme,
) -> Stateful<Div> {
    let colour = white(if open { ink::FULL } else { ink::BODY });
    let label: SharedString = label.into();
    tab(id, "", open, t)
        .children(icon_name.map(|n| icon(n, 16., colour)))
        .child(label.to_uppercase())
        .children(count.map(|n| div().text_color(white(ink::DIM)).child(n.to_string())))
}

/// A popover's row: text after a muted icon, filled under the pointer. `danger` colours it red.
pub fn menu_row(
    id: impl Into<SharedString>,
    name: &'static str,
    label: impl Into<SharedString>,
    danger: bool,
    t: &Theme,
) -> Stateful<Div> {
    let id = id.into();
    let (fg, icon_color) = if danger {
        let c = self::danger();
        (c, c)
    } else {
        (white(ink::FULL), white(ink::MUTED))
    };
    let hover_color = if danger {
        danger_fill()
    } else {
        white(fill::FIELD)
    };
    let d = div()
        .id(id.clone())
        .relative()
        .flex()
        .items_center()
        .gap(px(12.))
        .h(px(40.))
        .w_full()
        .px(px(12.))
        .rounded(px(radius::CONTROL))
        .cursor_pointer()
        .child(hover_fade(&id, layer(radius::CONTROL), move |d, a| {
            d.bg(hover_color.opacity(hover_color.a * a))
        }))
        .child(icon(name, 17., icon_color).relative())
        .child(
            text(14., FontWeight::NORMAL, ink::FULL)
                .relative()
                .text_color(fg)
                .child(label.into()),
        );
    keyboard(track_hover(d, id), t)
}

fn danger_fill() -> Hsla {
    rgba(eclipse_theme::glass::DANGER_HOVER << 8 | 0x1f).into()
}

/// A popover: a menu's float with room around its rows and a shadow setting it above the card
/// under it.
pub fn popover(width: f32) -> Div {
    crate::glass::float(radius::POPOVER)
        .shadow_lg()
        .w(px(width))
        .p(px(6.))
        .flex()
        .flex_col()
}

/// What a dialog or the palette lays over the launcher: GPUI can't blur what it drew, so a
/// menu's float covers the page. The scrim goes over it.
pub fn scrim_backdrop() -> impl IntoElement {
    crate::glass::backdrop(crate::glass::Kind::Float, 0.)
}

/// The scrim over [`scrim_backdrop`]: lighter in the light appearance, so the dimmed launcher
/// stays day-lit.
pub fn scrim() -> Hsla {
    color(eclipse_theme::glass::SCRIM).opacity(if light() { 0.2 } else { 0.5 })
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
    crate::glass::plate(radius)
        // Light plates are already day-lit: no dimming under them.
        .child(layer(
            color(eclipse_theme::glass::SCRIM).opacity(if light() { 0. } else { 0.5 }),
        ))
        .child(layer(white(eclipse_theme::glass::TINT)))
}

pub fn solid(
    id: impl Into<SharedString>,
    text: impl Into<SharedString>,
    h: f32,
    size: f32,
    t: &Theme,
) -> Stateful<Div> {
    let id = id.into();
    let text: SharedString = text.into();
    let look = look();
    keyboard(
        bevel(
            div()
                .relative()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .h(px(h))
                .px(px(16.))
                .rounded(px(radius::CONTROL))
                .overflow_hidden()
                .bg(color(look.accent))
                .text_color(color(look.on_accent))
                .font_family(eclipse_theme::glass::LABEL_FONT)
                .text_size(px(size.min(14.)))
                .font_weight(FontWeight::BOLD),
        )
        .id(id)
        .cursor_pointer()
        .hover(|s| s.opacity(HOVER_OPACITY))
        .child(div().relative().child(text.to_uppercase())),
        t,
    )
}

/// A menu row with a note under its label, filled under the pointer, disabled while `locked`.
pub fn menu_item(
    id: impl Into<SharedString>,
    label: SharedString,
    note: String,
    locked: bool,
    t: &Theme,
) -> Stateful<Div> {
    let id = id.into();
    let d = div()
        .id(id.clone())
        .relative()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.))
        .w_full()
        .px(px(12.))
        .py(px(8.))
        .rounded(px(radius::CONTROL))
        .when(!locked, |d| {
            d.cursor_pointer()
                .child(hover_fade(&id, layer(radius::CONTROL), |d, a| {
                    d.bg(white(fill::FIELD * a))
                }))
        })
        .child(
            div()
                .relative()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .child(
                    text(14., FontWeight::MEDIUM, ink::FULL)
                        .line_height(px(21.))
                        .child(label),
                )
                .when(!note.is_empty(), |d| {
                    d.child(
                        text(12., FontWeight::NORMAL, ink::MUTED)
                            .line_height(px(18.))
                            .truncate()
                            .child(note),
                    )
                }),
        );
    if locked {
        return crate::disabled(d);
    }
    keyboard(track_hover(d, id), t)
}

/// A dropdown's choice: its name on one line, the accent's tick on the one `picked`.
pub fn pick_row(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    picked: bool,
    t: &Theme,
) -> Stateful<Div> {
    let id = id.into();
    let d = div()
        .id(id.clone())
        .relative()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.))
        .h(px(Size::Md.height()))
        .w_full()
        .px(px(12.))
        .rounded(px(radius::CONTROL))
        .cursor_pointer()
        .child(hover_fade(&id, layer(radius::CONTROL), |d, a| {
            d.bg(white(fill::FIELD * a))
        }))
        .child(
            text(14., FontWeight::NORMAL, ink::FULL)
                .relative()
                .truncate()
                .child(label.into()),
        )
        .children(picked.then(|| icon("tick-02", 17., color(look().accent_soft)).relative()));
    keyboard(track_hover(d, id), t)
}

/// A list row, faintly filled under the pointer. `on_hover` hears the pointer come and go too,
/// for rows that show their actions then. `held`: the row keeps its fill while the pointer is on
/// one of its buttons or its menu is open, as CSS's `:hover` stays on a parent.
pub fn list_row(
    id: impl Into<SharedString>,
    held: bool,
    on_hover: impl Fn(&bool, &mut Window, &mut App) + 'static,
    t: &Theme,
) -> Stateful<Div> {
    let id = id.into();
    let key = id.clone();
    let fill = if held && !hover_is(&id) {
        layer(radius::ROW)()
            .bg(white(fill::HOVER))
            .into_any_element()
    } else {
        hover_fill(&id, radius::ROW, 0., fill::HOVER)
    };
    let d = div()
        .id(id.clone())
        .relative()
        .flex()
        .items_center()
        .gap(px(12.))
        .w_full()
        .px(px(12.))
        .py(px(10.))
        .rounded(px(radius::ROW))
        .child(fill)
        .on_hover(move |hovered, window, cx| {
            note_hover(&key, *hovered, window);
            on_hover(hovered, window, cx);
        });
    keyboard(d, t)
}

/// A list row's two lines: the name and a note.
pub fn list_text(name: impl Into<SharedString>, note: impl Into<SharedString>) -> Div {
    div()
        .relative()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .child(
            text(14., FontWeight::MEDIUM, ink::FULL)
                .line_height(px(21.))
                .truncate()
                .child(name.into()),
        )
        .child(
            text(13., FontWeight::NORMAL, ink::MUTED)
                .line_height(px(19.5))
                .truncate()
                .child(note.into()),
        )
}

/// An empty state inside an area that is already a card; the message goes in as a child.
pub fn empty() -> Div {
    empty_style().p(px(24.))
}

fn empty_style() -> Div {
    text(14., FontWeight::NORMAL, ink::MUTED).line_height(px(21.))
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
            text(13., FontWeight::NORMAL, ink::MUTED)
                .line_height(px(19.5))
                .min_w_0()
                .truncate()
                .child(note.into()),
        )
}

/// A link in a line of text: the accent's soft shade, underlined, white under the pointer.
pub fn link(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Stateful<Div> {
    div()
        .id(id.into())
        .font_weight(FontWeight::MEDIUM)
        .text_color(color(look().accent_soft))
        .underline()
        .cursor_pointer()
        .hover(|s| s.text_color(white(ink::FULL)))
        .child(label.into())
}

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
        .h(px(44.))
        .px(px(16.))
        .overflow_hidden()
        .rounded(px(radius::ROW))
        .bg(bg)
        .border_1()
        .border_color(field_edge())
        .font_family(eclipse_theme::glass::FONT)
        .text_size(px(14.))
}

/// A switch: the accent with an `on_accent` knob on the right while on, a faint track with an
/// edge and a pale knob on the left while off.
pub fn toggle(
    id: impl Into<SharedString>,
    on: bool,
    accent: u32,
    on_accent: u32,
    t: &Theme,
) -> Stateful<Div> {
    let id = id.into();
    let knob = div()
        .absolute()
        .top(px(4.))
        .size(px(16.))
        .rounded_full()
        .bg(if on {
            color(on_accent)
        } else {
            white(ink::BODY)
        });
    let knob = crate::motion::knob(&id, on, knob, 4., 16.);
    keyboard(
        div()
            .id(id)
            .relative()
            .flex_none()
            .w(px(40.))
            .h(px(24.))
            .rounded_full()
            .bg(if on {
                color(accent)
            } else {
                white(fill::ACTIVE)
            })
            .border_1()
            .border_color(if on {
                gpui::transparent_black()
            } else {
                white(ink::DIM)
            })
            .cursor_pointer()
            .child(knob),
        t,
    )
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Size {
    Sm,
    Md,
    Lg,
}

impl Size {
    pub fn height(self) -> f32 {
        match self {
            Size::Sm => 32.,
            Size::Md => 36.,
            Size::Lg => 44.,
        }
    }
}

pub fn button(
    id: impl Into<SharedString>,
    icon_name: Option<&'static str>,
    label: impl Into<SharedString>,
    kind: Kind,
    size: Size,
    t: &Theme,
) -> Stateful<Div> {
    let (id, label): (SharedString, SharedString) = (id.into(), label.into());
    let look = look();
    let tone = match kind {
        Kind::Primary => color(look.on_accent),
        Kind::Secondary => white(ink::FULL),
        Kind::Ghost => white(ink::BODY),
    };
    let d = div()
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap(px(8.))
        .h(px(size.height()))
        .px(px(14.))
        .rounded(px(radius::CONTROL))
        .overflow_hidden()
        .font_family(eclipse_theme::glass::LABEL_FONT)
        .text_size(px(PIXEL_TEXT))
        .line_height(px(16.))
        .children(icon_name.map(|n| icon(n, 16., tone).relative()));
    let d = d.text_color(tone);
    let d = match kind {
        Kind::Primary => bevel(d.bg(color(look.accent)).font_weight(FontWeight::BOLD))
            .id(id.clone())
            .hover(|s| s.opacity(HOVER_OPACITY)),
        Kind::Secondary => d
            .bg(white(fill::BUTTON))
            .id(id.clone())
            .hover(|s| s.bg(white(fill::ACTIVE))),
        Kind::Ghost => d
            .id(id.clone())
            .hover(|s| s.bg(white(fill::HOVER)).text_color(white(ink::FULL))),
    };
    keyboard(
        track_hover(
            d.cursor_pointer()
                .child(div().relative().child(label.to_uppercase())),
            id,
        ),
        t,
    )
}

/// A segmented control (the server dialog's difficulty): options on a faint track, the chosen
/// one raised.
pub fn segments<T: Copy + PartialEq + 'static>(
    id: &'static str,
    options: &[(T, &'static str)],
    current: T,
    t: &Theme,
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
            let d = div()
                .id(SharedString::from(format!("{id}-{label}")))
                .flex()
                .items_center()
                .h(px(Size::Sm.height()))
                .px(px(14.))
                .rounded(px(radius::CONTROL))
                .cursor_pointer()
                .font_family(eclipse_theme::glass::LABEL_FONT)
                .text_size(px(PIXEL_TEXT))
                .line_height(px(16.))
                .when(active, |d| {
                    d.bg(white(fill::ACTIVE))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(white(ink::FULL))
                })
                .when(!active, |d| {
                    d.text_color(white(ink::MUTED))
                        .hover(|s| s.text_color(white(ink::FULL)))
                })
                .child(crate::tr(label).to_uppercase())
                .on_click(move |_: &ClickEvent, window, cx| pick(value, window, cx));
            keyboard(d, t)
        }))
}

pub fn page_head(title: impl Into<SharedString>, note: impl Into<SharedString>) -> Div {
    let note: SharedString = note.into();
    div()
        .flex()
        .flex_col()
        .pt(px(40. - crate::glass::CONTENT_INSET))
        .mb(px(28.))
        .child(
            div()
                .font_family(eclipse_theme::glass::DISPLAY_FONT)
                .font_weight(FontWeight::BOLD)
                .text_size(px(PAGE_TITLE))
                .line_height(px(PAGE_TITLE + 4.))
                .text_color(white(ink::FULL))
                .child(title.into()),
        )
        .when(!note.is_empty(), |d| {
            d.child(
                text(15., FontWeight::NORMAL, ink::BODY)
                    .mt(px(12.))
                    .max_w(px(615.))
                    .line_height(px(23.25))
                    .child(note),
            )
        })
}

pub const PAGE_TITLE: f32 = 40.;

pub fn entity_title(text: impl Into<SharedString>, size: f32) -> Div {
    let size = if size >= 32. { 32. } else { 24. };
    div()
        .font_family(eclipse_theme::glass::DISPLAY_FONT)
        .font_weight(FontWeight::BOLD)
        .text_size(px(size))
        .line_height(px(size + 4.))
        .text_color(white(ink::FULL))
        .child(text.into())
}

/// A field group's label inside a card.
pub fn card_heading(label: impl Into<SharedString>) -> Div {
    text(13., FontWeight::MEDIUM, ink::MUTED)
        .line_height(px(19.5))
        .child(label.into())
}

pub fn dropdown(
    id: impl Into<SharedString>,
    leading: Option<AnyElement>,
    label: impl IntoElement,
    open: bool,
    height: f32,
    t: &Theme,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let chevron = icon("arrow-down-01", 16., white(ink::MUTED)).with_transformation(
        gpui::Transformation::rotate(gpui::radians(if open { std::f32::consts::PI } else { 0. })),
    );
    let d = div()
        .id(id)
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.))
        .h(px(height))
        .pl(px(if leading.is_some() { 6. } else { 14. }))
        .pr(px(12.))
        .rounded(px(radius::ROW))
        .bg(field_bg(open))
        .border_1()
        .border_color(field_edge())
        .hover(|s| s.bg(field_bg(true)))
        .cursor_pointer()
        .font_family(eclipse_theme::glass::FONT)
        .text_size(px(14.))
        .text_color(white(ink::FULL))
        .children(leading)
        .child(div().flex_1().min_w_0().truncate().child(label))
        .child(chevron);
    keyboard(d, t)
}

/// A card's note.
pub fn card_note(label: impl Into<SharedString>) -> Div {
    text(13., FontWeight::NORMAL, ink::MUTED)
        .line_height(px(19.5))
        .child(label.into())
}
