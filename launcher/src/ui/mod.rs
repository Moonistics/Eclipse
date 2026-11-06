//! The launcher's shared controls: white text and fills at the `eclipse_theme::glass` opacities
//! over the cards ([`card`]), Geist for text and values, Monocraft for titles and
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
    ink, light_ink, pixel_text as pixel, space, step, Glass, TextStep, MONOCRAFT_PIXEL, ON_LIGHT,
};
use gpui::{
    div, prelude::*, px, rgb, svg, AnimationExt, App, Div, FontWeight, Hsla, Role, SharedString,
    Stateful, Svg, Window,
};
use std::cell::Cell;

thread_local! {
    /// The look's colours for the player's accent.
    static LOOK: Cell<Glass> = Cell::new(Glass::default_look());
    /// The light appearance: the white ink and fills turn to [`INK`].
    static LIGHT: Cell<bool> = const { Cell::new(false) };
    static SHORT: Cell<bool> = const { Cell::new(false) };
}

/// A listener, boxed so a helper can return it.
pub type Listener<E> = Box<dyn Fn(&E, &mut Window, &mut App)>;

/// The light appearance's ink, at the opacities the dark look gives white.
const INK: u32 = ON_LIGHT;

pub const CARD_PADDING: f32 = 24.;

pub const SETTING_ROW_PADDING: f32 = 16.;

pub const PANEL_INSET: f32 = CARD_PADDING / 2.;

pub const LIST_INSET: f32 = space::SM;

pub const SIDEBAR_INSET: f32 = space::LG + space::XS;

pub const SIDEBAR_PAD: f32 = space::SM + space::XXS;

pub const TITLE_NOTE_GAP: f32 = space::MD;

const PAGE_TITLE_TOP: f32 = space::XXL + space::SM;

const PAGE_TITLE_TOP_SHORT: f32 = space::LG + space::XS;

const PAGE_HEAD_GAP: f32 = space::XL + space::XS;

const PAGE_HEAD_GAP_SHORT: f32 = space::LG;

const PAGE_NOTE_WIDTH: f32 = 615.;

const TIP_PAD: f32 = space::SM;

const DOT: f32 = 7.;

pub const DOT_ROOM: f32 = DOT + space::SM;

const BAR_HEIGHT: f32 = 4.;

const SHUTTLE: std::time::Duration = std::time::Duration::from_millis(1600);

const SHUTTLE_SHARE: f32 = 0.3;

const STILL_SHUTTLE: f32 = 0.4;

pub const TILE_SM: f32 = 24.;

pub const TILE_MD: f32 = 36.;

pub const TILE_LG: f32 = 48.;

pub fn begin_frame(theme: &Theme, short: bool, window: &Window, cx: &mut App) {
    LOOK.set(theme.look);
    LIGHT.set(!theme.dark);
    SHORT.set(short);
    press::forget_spots();
    press::note_focus(window, cx);
    hover::forget_disabled();
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

pub fn inset() -> Hsla {
    use eclipse_theme::glass::fill;
    if light() {
        white(fill::FIELD)
    } else {
        gpui::black().opacity(fill::WELL_SHADE)
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

pub fn text_width(text: &str, step: TextStep) -> f32 {
    text.chars().count() as f32 * eclipse_theme::glass::GEIST_ADVANCE * step.size / step::BODY.size
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

pub fn section_title(title: impl Into<SharedString>) -> Div {
    pixel_text(pixel::PRESS)
        .text_color(white(ink::FULL))
        .child(title.into())
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
    let (title, note): (SharedString, SharedString) = (title.into(), note.into());
    div()
        .flex()
        .flex_col()
        .pt(px(page_top()))
        .mb(px(page_gap()))
        .child(
            heading("page-title", title.clone(), 1).child(entity_title(title, pixel::PAGE_TITLE)),
        )
        .when(!note.is_empty(), |d| {
            d.child(
                text(step::LEAD, FontWeight::NORMAL, ink::BODY)
                    .mt(px(TITLE_NOTE_GAP))
                    .max_w(px(PAGE_NOTE_WIDTH))
                    .child(note),
            )
        })
}

pub fn entity_title(text: impl Into<SharedString>, size: f32) -> Div {
    pixel_text(size)
        .text_color(white(ink::FULL))
        .child(text.into())
}

pub fn heading(id: impl Into<SharedString>, title: SharedString, level: usize) -> Stateful<Div> {
    div()
        .id(id.into())
        .role(Role::Heading)
        .aria_level(level)
        .aria_label(title)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IconSize {
    Px16,
    Px24,
    Px32,
}

impl IconSize {
    pub fn px(self) -> f32 {
        match self {
            IconSize::Px16 => 16.,
            IconSize::Px24 => 24.,
            IconSize::Px32 => 32.,
        }
    }
}

pub fn icon(name: &'static str, size: IconSize, color: Hsla) -> Svg {
    svg()
        .path(format!("icons/{name}.svg"))
        .flex_none()
        .size(px(size.px()))
        .text_color(color)
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

pub fn tone_colour(tone: Tone) -> Hsla {
    match tone {
        Tone::Live => live(),
        Tone::Busy | Tone::Warn => warn(),
        Tone::Accent(soft) => color(soft),
        Tone::Off => white(ink::DIM),
    }
}

pub fn dot(tone: Tone) -> Div {
    div()
        .flex_none()
        .size(px(DOT))
        .rounded_full()
        .bg(tone_colour(tone))
}

/// The progress bar: a thin round track filled in the accent's soft shade.
pub fn bar(value: f32) -> Stateful<Div> {
    bar_track()
        .aria_numeric_value(f64::from(value.clamp(0., 1.)))
        .w_full()
        .child(bar_fill().left_0().w(gpui::relative(value.clamp(0., 1.))))
}

/// A thin bar. A known fraction eases from its previous value; an unknown one shuttles slowly.
pub fn progress_bar(fraction: Option<(f32, f32)>) -> Stateful<Div> {
    let track = bar_track().flex_1();
    let track = match fraction {
        Some((_, to)) => track.aria_numeric_value(f64::from(to.clamp(0., 1.))),
        None => track,
    };
    let still = crate::motion::still();
    match fraction {
        Some((_, to)) if still => {
            track.child(bar_fill().left_0().w(gpui::relative(to.clamp(0.02, 1.))))
        }
        Some((from, to)) => {
            let (from, to) = (from.clamp(0.02, 1.), to.clamp(0.02, 1.));
            track.child(bar_fill().left_0().with_animation(
                gpui::ElementId::named_usize("bar", (to * 1000.) as usize),
                crate::motion::eased(eclipse_theme::MOVE),
                move |d, p| d.w(gpui::relative(from + (to - from) * p)),
            ))
        }
        None if still => track.child(bar_fill().left_0().w_full().opacity(STILL_SHUTTLE)),
        None => track.child(
            bar_fill().w(gpui::relative(SHUTTLE_SHARE)).with_animation(
                "shuttle",
                gpui::Animation::new(SHUTTLE)
                    .repeat()
                    .with_easing(gpui::ease_in_out),
                |d, p| d.left(gpui::relative(-SHUTTLE_SHARE + (1. + SHUTTLE_SHARE) * p)),
            ),
        ),
    }
}

fn bar_track() -> Stateful<Div> {
    div()
        .id("progress")
        .role(Role::ProgressIndicator)
        .aria_min_numeric_value(0.)
        .aria_max_numeric_value(1.)
        .relative()
        .h(px(BAR_HEIGHT))
        .rounded_full()
        .overflow_hidden()
        .bg(white(eclipse_theme::glass::fill::ACTIVE))
}

fn bar_fill() -> Div {
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .rounded_full()
        .bg(color(look().accent_soft))
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
        .py(px(space::XS))
        .rounded(px(eclipse_theme::glass::radius::CONTROL))
        .bg(surface_colour(Surface::Float))
        .border_1()
        .border_color(edge())
        .shadow_md()
        .child(self::text(step::CAPTION, FontWeight::NORMAL, ink::FULL).child(text))
}

pub fn tip(
    text: impl Into<SharedString>,
) -> impl Fn(&mut Window, &mut App) -> gpui::AnyView + 'static {
    let text: SharedString = text.into();
    move |_, cx| cx.new(|_| Tip(text.clone())).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::accesskit::{Node, Toggled};
    use gpui::Element;

    fn read(el: &impl Element) -> (Option<Role>, Node) {
        let mut node = Node::new(Role::Unknown);
        el.write_a11y_info(&mut node);
        (el.a11y_role(), node)
    }

    #[test]
    fn controls_say_what_they_are_and_what_state_they_are_in() {
        let (role, n) = read(&button(
            "save",
            None,
            "Save",
            ButtonKind::Secondary,
            ButtonSize::Md,
        ));
        assert_eq!((role, n.label()), (Some(Role::Button), Some("Save")));

        let (role, n) = read(&more_button(
            "side-more",
            "Survival options",
            ButtonKind::Ghost,
            ButtonSize::Sm,
            true,
        ));
        assert_eq!(role, Some(Role::Button));
        assert_eq!(n.label(), Some("Survival options"));
        assert_eq!(n.is_expanded(), Some(true));

        let (role, n) = read(&toggle("motion", true));
        assert_eq!(
            (role, n.toggled()),
            (Some(Role::Switch), Some(Toggled::True))
        );

        let (role, n) = read(&toggle_row("auto", "Install automatically", None, false));
        assert_eq!(role, Some(Role::Switch));
        assert_eq!(n.label(), Some("Install automatically"));
        assert_eq!(n.toggled(), Some(Toggled::False));

        let (role, n) = read(&count_tab("home-tab-worlds", "Worlds", None, true, TAB_PAD));
        assert_eq!((role, n.is_selected()), (Some(Role::Tab), Some(true)));
        assert_eq!(
            read(&tab_strip_padded("home-tabs", TAB_PAD, [])).0,
            Some(Role::TabList)
        );

        let (role, n) = read(&nav_row(
            "nav-home",
            "home",
            "Home",
            true,
            NavDensity::Sidebar,
        ));
        assert_eq!((role, n.is_selected()), (Some(Role::Link), Some(true)));
        assert_eq!(n.label(), Some("Home"));

        let (role, n) = read(&dropdown(
            "picker",
            "Version",
            None,
            "Nightly",
            false,
            ButtonSize::Md,
        ));
        assert_eq!((role, n.is_expanded()), (Some(Role::ComboBox), Some(false)));
        assert_eq!(n.label(), Some("Version"));

        let (role, n) = read(&menu_row(
            "row-menu-edit",
            "pencil-edit-02",
            "Edit instance",
            false,
        ));
        assert_eq!(
            (role, n.label()),
            (Some(Role::MenuItem), Some("Edit instance"))
        );

        let (role, n) = read(&pick_row("language-pick-de", "Deutsch", false));
        assert_eq!(role, Some(Role::MenuItemRadio));
        assert_eq!(n.toggled(), Some(Toggled::False));

        let (role, n) = read(&progress_bar(Some((0., 0.5))));
        assert_eq!(role, Some(Role::ProgressIndicator));
        assert_eq!(n.numeric_value(), Some(0.5));

        let (role, n) = read(&stepper("players", 10, 1..=100, |_, _, _| {}));
        assert_eq!(
            (role, n.numeric_value()),
            (Some(Role::SpinButton), Some(10.))
        );

        let (role, _) = read(&segments(
            "difficulty",
            &[(1, SharedString::from("Easy"))],
            1,
            |_, _, _| {},
        ));
        assert_eq!(role, Some(Role::RadioGroup));

        let (role, n) = read(&link("eula", "Read the EULA"));
        assert_eq!((role, n.label()), (Some(Role::Link), Some("Read the EULA")));

        let (role, n) = read(&heading("page-title", "Settings".into(), 1));
        assert_eq!((role, n.level()), (Some(Role::Heading), Some(1)));
    }

    #[test]
    fn a_control_that_cant_be_pressed_says_why() {
        let blocked = unavailable(
            button("join", None, "Join", ButtonKind::Secondary, ButtonSize::Md),
            "Paste a friend's invite code first",
        );
        let (role, n) = read(&blocked);
        assert_eq!(role, Some(Role::Button));
        assert_eq!(n.description(), Some("Paste a friend's invite code first"));
    }
}
