//! The launcher's shared controls: white text and fills at the `eclipse_theme::glass` opacities
//! over [`glass`](crate::glass) plates, Geist for text and values, Monocraft for titles and
//! pressables. The look state for each frame (accent, light appearance) lives here.
mod buttons;
mod hover;
mod items;
mod press;
mod surfaces;
mod tiles;

pub use buttons::*;
pub use hover::*;
pub use items::*;
pub use press::*;
pub use surfaces::*;
pub use tiles::*;

use crate::theme::Theme;
use eclipse_theme::glass::{
    ink, light_ink, pixel_text as pixel, step, Glass, TextStep, MONOCRAFT_PIXEL, ON_LIGHT,
};
use gpui::{div, prelude::*, px, rgb, svg, App, Div, FontWeight, Hsla, SharedString, Svg, Window};
use std::cell::Cell;

thread_local! {
    /// The look's colours for the player's accent.
    static LOOK: Cell<Glass> = Cell::new(Glass::default_look());
    /// The light appearance: the white ink and fills turn to [`INK`].
    static LIGHT: Cell<bool> = const { Cell::new(false) };
    static SHORT: Cell<bool> = const { Cell::new(false) };
}

/// The light appearance's ink, at the opacities the dark look gives white.
const INK: u32 = ON_LIGHT;

pub const CARD_PADDING: f32 = 24.;

pub const SETTING_ROW_PADDING: f32 = 16.;

pub const PANEL_INSET: f32 = CARD_PADDING / 2.;

pub const LIST_INSET: f32 = 8.;

pub const SIDEBAR_INSET: f32 = 20.;

pub const SIDEBAR_PAD: f32 = 10.;

const PAGE_TITLE_TOP: f32 = 40.;

const PAGE_TITLE_TOP_SHORT: f32 = 20.;

const PAGE_HEAD_GAP: f32 = 28.;

const PAGE_HEAD_GAP_SHORT: f32 = 16.;

const PAGE_NOTE_WIDTH: f32 = 615.;

const TIP_PAD: f32 = 8.;

pub fn begin_frame(theme: &Theme, short: bool) {
    LOOK.set(theme.look);
    LIGHT.set(!theme.dark);
    SHORT.set(short);
    press::forget_spots();
}

/// Whether the light appearance is drawn.
pub fn light() -> bool {
    LIGHT.get()
}

/// The look's colours.
pub fn look() -> Glass {
    LOOK.get()
}

pub fn short_window() -> bool {
    SHORT.get()
}

pub fn white(a: f32) -> Hsla {
    if light() {
        color(INK).opacity(light_ink(a))
    } else {
        gpui::white().opacity(a)
    }
}

/// A colour of the look (`0xrrggbb`).
pub fn color(c: u32) -> Hsla {
    rgb(c).into()
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

pub fn well() -> Hsla {
    if light() {
        white(eclipse_theme::glass::fill::FIELD)
    } else {
        gpui::black().opacity(0.25)
    }
}

pub fn icon_tone(selected: bool) -> Hsla {
    if selected {
        color(look().mark)
    } else {
        white(ink::BODY)
    }
}

pub fn text(step: TextStep, weight: FontWeight, a: f32) -> Div {
    div()
        .font_family(eclipse_theme::glass::FONT)
        .text_size(px(step.size))
        .line_height(px(step.line))
        .font_weight(weight)
        .text_color(white(a))
}

pub fn on_pixel_grid(size: f32) -> f32 {
    ((size / MONOCRAFT_PIXEL).floor() * MONOCRAFT_PIXEL).max(MONOCRAFT_PIXEL)
}

pub fn pixel_text(size: f32) -> Div {
    let size = on_pixel_grid(size);
    div()
        .font_family(eclipse_theme::glass::DISPLAY_FONT)
        .text_size(px(size))
        .line_height(px(size + MONOCRAFT_PIXEL / 2.))
}

pub fn label(text: impl Into<SharedString>) -> Div {
    let text: SharedString = text.into();
    self::text(step::LABEL, FontWeight::SEMIBOLD, ink::DIM).child(text.to_uppercase())
}

pub fn card_title(title: impl Into<SharedString>) -> Div {
    text(step::TITLE, FontWeight::SEMIBOLD, ink::FULL).child(title.into())
}

/// A card's note.
pub fn card_note(label: impl Into<SharedString>) -> Div {
    text(step::NOTE, FontWeight::NORMAL, ink::MUTED).child(label.into())
}

pub fn page_top() -> f32 {
    let top = if short_window() {
        PAGE_TITLE_TOP_SHORT
    } else {
        PAGE_TITLE_TOP
    };
    top - crate::layout::CONTENT_INSET
}

pub fn page_gap() -> f32 {
    if short_window() {
        PAGE_HEAD_GAP_SHORT
    } else {
        PAGE_HEAD_GAP
    }
}

pub fn page_head(title: impl Into<SharedString>, note: impl Into<SharedString>) -> Div {
    let note: SharedString = note.into();
    div()
        .flex()
        .flex_col()
        .pt(px(page_top()))
        .mb(px(page_gap()))
        .child(entity_title(title, pixel::PAGE_TITLE))
        .when(!note.is_empty(), |d| {
            d.child(
                text(step::LEAD, FontWeight::NORMAL, ink::BODY)
                    .mt(px(12.))
                    .max_w(px(PAGE_NOTE_WIDTH))
                    .child(note),
            )
        })
}

pub fn entity_title(text: impl Into<SharedString>, size: f32) -> Div {
    pixel_text(size)
        .font_weight(FontWeight::BOLD)
        .text_color(white(ink::FULL))
        .child(text.into())
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IconSize {
    Px16,
    Px32,
}

impl IconSize {
    pub fn px(self) -> f32 {
        match self {
            IconSize::Px16 => 16.,
            IconSize::Px32 => 32.,
        }
    }
}

pub fn icon(name: &'static str, size: IconSize, color: Hsla) -> Svg {
    svg()
        .path(format!("icons/glass/{name}.svg"))
        .flex_none()
        .size(px(size.px()))
        .text_color(color)
}

pub fn icon_grid(size: f32) -> f32 {
    if size <= 24. {
        16.
    } else {
        (size / 16.).round().max(2.) * 16.
    }
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
        .bg(white(eclipse_theme::glass::fill::ACTIVE))
        .child(
            div()
                .h_full()
                .rounded_full()
                .w(gpui::relative(value.clamp(0., 1.)))
                .bg(color(accent_soft)),
        )
}

pub struct Tip(SharedString);

impl gpui::Render for Tip {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        tip_body(self.0.clone())
    }
}

fn tip_body(text: SharedString) -> Div {
    div()
        .px(px(TIP_PAD))
        .py(px(4.))
        .rounded(px(eclipse_theme::glass::radius::CONTROL))
        .bg(crate::glass::colour(crate::glass::Kind::Float))
        .border_1()
        .border_color(white(eclipse_theme::glass::EDGE))
        .shadow_md()
        .child(self::text(step::CAPTION, FontWeight::NORMAL, ink::FULL).child(text))
}

pub fn tip(
    text: impl Into<SharedString>,
) -> impl Fn(&mut Window, &mut App) -> gpui::AnyView + 'static {
    let text: SharedString = text.into();
    move |_, cx| cx.new(|_| Tip(text.clone())).into()
}
