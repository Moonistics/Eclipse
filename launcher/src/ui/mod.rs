//! The launcher's shared controls: white text and fills at the `eclipse_theme::glass` opacities
//! over the cards ([`card`]), Geist for text and values, Monocraft for titles and
//! pressables. The look state for each frame (accent, light appearance) lives here.
mod buttons;
mod hover;
mod items;
mod nav;
mod press;
mod sliders;
mod surfaces;
mod switches;
mod tabs;
mod tiles;

pub use buttons::*;
pub use hover::*;
pub use items::*;
pub use nav::*;
pub use press::*;
pub use sliders::*;
pub use surfaces::*;
pub use switches::*;
pub use tabs::*;
pub use tiles::*;

use crate::theme::Theme;
use eclipse_theme::glass::{
    ink, light_ink, pixel_text as pixel, space, step, Glass, TextStep, MONOCRAFT_PIXEL, ON_LIGHT,
};
use gpui::{
    div, prelude::*, px, rgb, svg, AnimationExt, AnyElement, App, Div, ElementId, FontWeight, Hsla,
    Role, SharedString, Stateful, Svg, Window,
};
use std::cell::Cell;

thread_local! {
    /// The look's colours for the player's accent.
    static LOOK: Cell<Glass> = Cell::new(Glass::default_look());
    /// The light appearance: the white ink and fills turn to [`INK`].
    static LIGHT: Cell<bool> = const { Cell::new(false) };
    static SHORT: Cell<bool> = const { Cell::new(false) };
    static SCALE: Cell<f32> = const { Cell::new(1.) };
}

/// A listener, boxed so a helper can return it.
pub type Listener<E> = Box<dyn Fn(&E, &mut Window, &mut App)>;

/// The light appearance's ink, at the opacities the dark look gives white.
const INK: u32 = ON_LIGHT;

pub const CARD_PADDING: f32 = space::XL;

pub const SETTING_ROW_PADDING: f32 = space::LG;

pub const PANEL_INSET: f32 = CARD_PADDING / 2.;

pub const LIST_INSET: f32 = space::SM;

pub const SIDEBAR_INSET: f32 = space::LG + space::XS;

pub const SIDEBAR_PAD: f32 = space::SM + space::XXS;

pub const TITLE_NOTE_GAP: f32 = space::MD;

const PAGE_TITLE_TOP: f32 = space::XXL + space::SM;

const PAGE_TITLE_TOP_SHORT: f32 = space::LG + space::XS;

const PAGE_HEAD_GAP: f32 = space::XL + space::XS;

const PAGE_HEAD_GAP_SHORT: f32 = space::LG;

const TIP_PAD: f32 = space::SM;

const ODD_PIXEL: f32 = 1.;

const DOT: f32 = space::SM - ODD_PIXEL;

pub const DOT_ROOM: f32 = DOT + space::SM;

const BAR_HEIGHT: f32 = 4.;

const BAR_MIN: f32 = 0.02;

const SHUTTLE: std::time::Duration = std::time::Duration::from_millis(1600);

const SHUTTLE_SHARE: f32 = 0.3;

const STILL_SHUTTLE: f32 = 0.4;

pub const TILE_SM: f32 = 24.;

pub const TILE_MD: f32 = 36.;

pub const TILE_LG: f32 = 48.;

pub fn frame<C: std::ops::DerefMut<Target = App>, T>(
    theme: &Theme,
    short: bool,
    window: &mut Window,
    cx: &mut C,
    draw: impl FnOnce(&mut Window, &mut C) -> T,
) -> T {
    begin_frame(theme, short, window, cx);
    let drawn = draw(window, cx);
    end_frame(window, cx);
    drawn
}

fn begin_frame(theme: &Theme, short: bool, window: &Window, cx: &mut App) {
    LOOK.set(theme.look);
    LIGHT.set(!theme.dark);
    SHORT.set(short);
    SCALE.set(window.scale_factor());
    press::forget_spots();
    surfaces::roll_foot();
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
pub(super) fn paper() -> Hsla {
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
        .font_family(crate::theme::FONT)
        .text_size(px(step.size))
        .line_height(px(step.line))
        .font_weight(weight)
        .text_color(white(a))
}

pub fn mono(step: TextStep, a: f32) -> Div {
    in_mono(text(step, FontWeight::NORMAL, a), step)
}

pub fn in_mono<E: Styled>(el: E, step: TextStep) -> E {
    el.font_family(crate::theme::MONO).text_size(px(step.size))
}

pub fn sign_in_code(code: impl Into<SharedString>) -> Div {
    text(step::CODE, FontWeight::MEDIUM, ink::FULL)
        .font_family(crate::theme::MONO)
        .child(code.into())
}

pub fn said(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Stateful<Div> {
    let text: SharedString = text.into();
    div()
        .id(id)
        .role(Role::Label)
        .aria_label(text.clone())
        .child(text)
}

pub fn text_width(text: &str, step: TextStep) -> f32 {
    text.chars().count() as f32 * eclipse_theme::glass::GEIST_ADVANCE * step.size / step::BODY.size
}

pub fn on_pixel_grid(size: f32) -> f32 {
    on_device_grid(size, SCALE.get())
}

pub fn pixel_text_grows() -> bool {
    on_pixel_grid(pixel::PRESS) > pixel::PRESS
}

fn on_device_grid(size: f32, scale: f32) -> f32 {
    let font_pixel = (size / MONOCRAFT_PIXEL).floor().max(1.);
    let device = (font_pixel * scale).round().max(1.);
    device / scale * MONOCRAFT_PIXEL
}

fn icon_px(size: IconSize, cells: f32, scale: f32) -> f32 {
    let cell = (size.px() / cells * scale).round().max(1.);
    cell / scale * cells
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

pub fn section_title(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Stateful<Div> {
    let title: SharedString = title.into();
    heading(id, title.clone(), 2).child(entity_title(title, pixel::CARD_TITLE))
}

pub fn card_title(title: impl Into<SharedString>) -> Div {
    text(step::TITLE, FontWeight::SEMIBOLD, ink::FULL).child(title.into())
}

/// A card's note.
pub fn card_note(label: impl IntoElement) -> Div {
    text(step::NOTE, FontWeight::NORMAL, ink::MUTED).child(label)
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

pub fn page_head(title: impl Into<SharedString>, note: Option<SharedString>) -> Div {
    page_head_with(title, note, None)
}

pub fn page_head_with(
    title: impl Into<SharedString>,
    note: Option<SharedString>,
    aside: Option<AnyElement>,
) -> Div {
    let title = page_title(title.into());
    let head = match aside {
        Some(aside) => div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap(px(space::LG))
            .child(title)
            .child(aside),
        None => div().child(title),
    };
    div()
        .flex()
        .flex_col()
        .pt(px(page_top()))
        .mb(px(page_gap()))
        .child(head)
        .children(note.map(|note| page_note("page-note", note)))
}

pub fn page_note(id: impl Into<ElementId>, note: impl Into<SharedString>) -> Div {
    text(step::LEAD, FontWeight::NORMAL, ink::BODY)
        .mt(px(TITLE_NOTE_GAP))
        .max_w(px(crate::layout::MEASURE_NOTE))
        .child(said(id, note))
}

pub fn page_title(title: SharedString) -> Stateful<Div> {
    heading("page-title", title.clone(), 1)
        .flex()
        .items_center()
        .min_w_0()
        .min_h(px(TILE_LG))
        .child(entity_title(title, pixel::PAGE_TITLE).min_w_0().truncate())
}

pub fn entity_title(text: impl Into<SharedString>, size: f32) -> Div {
    debug_assert!(
        size == pixel::PAGE_TITLE || size == pixel::CARD_TITLE,
        "a thing inside a card is named by ui::card_title"
    );
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
    pub const fn px(self) -> f32 {
        match self {
            IconSize::Px16 => 16.,
            IconSize::Px24 => 24.,
            IconSize::Px32 => 32.,
        }
    }
}

pub fn icon(name: &'static str, size: IconSize, color: Hsla) -> Svg {
    let drawn = icon_px(size, crate::icons::cells(name, size), SCALE.get());
    svg()
        .path(crate::icons::path(name, size))
        .flex_none()
        .size(px(drawn))
        .text_color(color)
}

/// The state dot before a row's second line: green only for a running game or server, the warning
/// colour for busy and for a warning, the accent's soft shade (given) for an update, dim for the
/// rest.
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

pub(super) fn dot(tone: Tone) -> Div {
    div()
        .flex_none()
        .size(px(DOT))
        .rounded_full()
        .bg(tone_colour(tone))
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
            track.child(bar_fill().left_0().w(gpui::relative(to.clamp(BAR_MIN, 1.))))
        }
        Some((from, to)) => {
            let (from, to) = (from.clamp(BAR_MIN, 1.), to.clamp(BAR_MIN, 1.));
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

pub fn progress_line(
    label: impl Into<SharedString>,
    fraction: Option<(f32, f32)>,
    note: impl Into<SharedString>,
) -> Div {
    let (label, note): (SharedString, SharedString) = (label.into(), note.into());
    let percent = fraction.map(|(_, to)| format!("{:.0}%", to.clamp(0., 1.) * 100.));
    div()
        .flex()
        .flex_col()
        .gap(px(space::SM))
        .min_w_0()
        .when(!label.is_empty() || percent.is_some(), |d| {
            d.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(space::MD))
                    .child(self::label(label))
                    .children(percent.map(|p| {
                        text(step::NOTE, FontWeight::MEDIUM, ink::BODY)
                            .flex_none()
                            .child(p)
                    })),
            )
        })
        .child(div().flex().child(progress_bar(fraction)))
        .when(!note.is_empty(), |d| d.child(card_note(note).truncate()))
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

pub(super) struct Tip(SharedString);

impl gpui::Render for Tip {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        tip_body(self.0.clone())
    }
}

fn tip_size(text: &str) -> (f32, f32) {
    let edges = 2. * ROW_EDGE;
    (
        text_width(text, step::CAPTION) + 2. * TIP_PAD + edges,
        step::CAPTION.line + 2. * space::XS + edges,
    )
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

        let (role, n) = read(&toggle_row(
            "auto",
            "Install automatically",
            Some("Keeps the 2 newest.".into()),
            false,
        ));
        assert_eq!(role, Some(Role::Switch));
        assert_eq!(n.label(), Some("Install automatically"));
        assert_eq!(n.toggled(), Some(Toggled::False));
        assert_eq!(n.description(), Some("Keeps the 2 newest."));

        let (role, n) = read(&count_tab(
            "home-tab-worlds",
            "Worlds",
            Some((3, "3 worlds".into())),
            true,
            TAB_PAD,
        ));
        assert_eq!((role, n.is_selected()), (Some(Role::Tab), Some(true)));
        assert_eq!(n.description(), Some("3 worlds"));
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
        assert_eq!((n.label(), n.value()), (Some("Version"), Some("Nightly")));

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

        let (role, n) = read(&segments(
            "difficulty",
            "Difficulty",
            &[(1, SharedString::from("Easy"))],
            1,
            |_, _, _| {},
        ));
        assert_eq!(
            (role, n.label()),
            (Some(Role::RadioGroup), Some("Difficulty"))
        );
        assert_eq!(value_key(&Some("first_light")), "some-first-light");
        assert_eq!(value_key(&"DataCollection"), "data-collection");

        let (role, n) = read(&link("eula", "Read the EULA"));
        assert_eq!((role, n.label()), (Some(Role::Link), Some("Read the EULA")));

        let (role, n) = read(&heading("page-title", "Settings".into(), 1));
        assert_eq!((role, n.level()), (Some(Role::Heading), Some(1)));

        let (role, n) = read(&section_title("game-title", "Game"));
        assert_eq!(
            (role, n.level(), n.label()),
            (Some(Role::Heading), Some(2), Some("Game"))
        );
    }

    #[test]
    fn pixel_text_and_icons_land_on_whole_device_pixels() {
        let device = |size: f32, scale: f32| on_device_grid(size, scale) / MONOCRAFT_PIXEL * scale;
        for scale in [1., 1.25, 1.5, 1.75, 2.] {
            for size in [pixel::PRESS, pixel::CARD_TITLE, pixel::PAGE_TITLE] {
                let per_pixel = device(size, scale);
                assert!(
                    (per_pixel - per_pixel.round()).abs() < 1e-4,
                    "{size} at {scale}x: {per_pixel} device px a font pixel"
                );
            }
            for size in [IconSize::Px16, IconSize::Px24, IconSize::Px32] {
                for name in ["play", "cancel-01"] {
                    let cells = crate::icons::cells(name, size);
                    let cell = icon_px(size, cells, scale) / cells * scale;
                    assert!(
                        (cell - cell.round()).abs() < 1e-4,
                        "{name} at {size:?}, {scale}x"
                    );
                }
            }
        }
        for scale in [1., 1.5, 2.] {
            assert_eq!(on_device_grid(pixel::PRESS, scale), pixel::PRESS);
            assert_eq!(on_device_grid(pixel::PAGE_TITLE, scale), pixel::PAGE_TITLE);
        }
        assert!((on_device_grid(pixel::PRESS, 1.25) - 21.6).abs() < 1e-4);
        assert!((on_device_grid(pixel::CARD_TITLE, 1.25) - 28.8).abs() < 1e-4);
        assert!((on_device_grid(pixel::PRESS, 1.75) - 20.571_43).abs() < 1e-3);
        assert_eq!(on_device_grid(20., 1.), pixel::PRESS);
    }

    #[test]
    fn rows_and_lines_of_text_reach_assistive_tech() {
        let (_, n) = read(&side_row("side-instance-survival", "Survival World", true));
        assert_eq!(
            (n.label(), n.is_selected()),
            (Some("Survival World"), Some(true))
        );
        let (_, n) = read(&row("server-row-lobby", "Lobby", false));
        assert_eq!((n.label(), n.is_selected()), (Some("Lobby"), Some(false)));

        let (role, n) = read(&said(
            "play-failed-note",
            "GitHub's certificate couldn't be checked.",
        ));
        assert_eq!(role, Some(Role::Label));
        assert_eq!(n.label(), Some("GitHub's certificate couldn't be checked."));

        let (role, n) = read(&disabled(button(
            "check-updates",
            None,
            "Checking…",
            ButtonKind::Secondary,
            ButtonSize::Md,
        )));
        assert_eq!(role, Some(Role::Button));
        assert!(n.description().is_some());
    }

    #[test]
    fn lists_sliders_and_failures_say_what_they_are() {
        let list = options(
            div().id("side-rows"),
            "Instances",
            vec![(div().id("side-instance-a"), "Survival".into(), true)],
            false,
            |_, _, _| {},
        );
        let (role, n) = read(&list);
        assert_eq!((role, n.label()), (Some(Role::ListBox), Some("Instances")));

        let (role, _) = read(&error_state(
            "releases-failed",
            "Couldn't load versions",
            "GitHub's certificate couldn't be checked.",
            |_, _, _| {},
        ));
        assert_eq!(role, Some(Role::Alert));

        let (role, n) = read(&error_line_for(
            "check-updates-failed",
            "Couldn't check for updates",
            "offline",
            None,
            Vec::new(),
        ));
        assert_eq!((role, n.label()), (Some(Role::Alert), None));

        let (role, n) = read(&slider(
            "instance-memory",
            "Maximum memory",
            0.25,
            4096.,
            (0., 16384.),
            Track::Fill,
            |_, _, _| {},
        ));
        assert_eq!(
            (role, n.label()),
            (Some(Role::Slider), Some("Maximum memory"))
        );
        assert_eq!(n.numeric_value(), Some(4096.));
    }

    #[test]
    fn a_control_keeps_its_focus_while_it_is_busy() {
        let focusable = |n: &Node| n.supports_action(gpui::accesskit::Action::Focus);
        let (_, n) = read(&disabled(button(
            "mods-more",
            None,
            "Loading…",
            ButtonKind::Secondary,
            ButtonSize::Md,
        )));
        assert!(focusable(&n), "a busy button keeps the focus it had");
        let (role, n) = read(&primary_busy(
            "play",
            "Preparing",
            Some(0.4),
            ButtonSize::Hero,
        ));
        assert_eq!(role, Some(Role::ProgressIndicator));
        assert!(focusable(&n), "busy Play keeps the focus it had");
        let (role, n) = read(&primary_unavailable(
            "instance-save",
            None,
            "Save changes",
            ButtonSize::Lg,
            "Nothing changed yet",
        ));
        assert_eq!(role, Some(Role::Button));
        assert!(focusable(&n));
        assert_eq!(n.description(), Some("Nothing changed yet"));
    }

    #[test]
    fn panels_dialogs_and_menus_say_what_they_are() {
        let (role, n) = read(&tab_panel("home-tab-panel", "Worlds"));
        assert_eq!((role, n.label()), (Some(Role::TabPanel), Some("Worlds")));
        let (role, n) = read(&modal("owner-gate", "Sign in to play", None));
        assert_eq!(
            (role, n.label()),
            (Some(Role::Dialog), Some("Sign in to play"))
        );
        let (role, n) = read(&sheet("first-run", "Welcome"));
        assert_eq!((role, n.label()), (Some(Role::Dialog), Some("Welcome")));
        let (role, n) = read(&popover("account-menu", "Account"));
        assert_eq!((role, n.label()), (Some(Role::Menu), Some("Account")));
        let (role, n) = read(&error_state(
            "releases-failed",
            "Couldn't load versions",
            "offline",
            |_, _, _| {},
        ));
        assert_eq!(role, Some(Role::Alert));
        assert_eq!(n.label(), None);
    }

    #[test]
    fn a_cause_is_said_only_when_the_error_says_one() {
        assert_eq!(
            told_cause("GitHub's certificate couldn't be checked.\nmore"),
            Some("GitHub's certificate couldn't be checked.".into())
        );
        assert_eq!(
            told_cause("Couldn't reach GitHub: os error 11"),
            Some("Couldn't reach GitHub.".into())
        );
        assert_eq!(told_cause("java.lang.IllegalStateException: boom"), None);
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
