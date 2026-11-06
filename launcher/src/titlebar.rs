//! Window chrome: Windows and Linux client-side decorations are custom drawn; macOS and Linux
//! server-side decorations use the platform's native chrome.
use crate::theme::Theme;
use crate::ui::{self, white};
use eclipse_launcher::backend::Appearance;
use eclipse_theme::glass::{ink, space};
use gpui::{Div, ParentElement, Styled, TitlebarOptions, Window};

fn wordmark() -> Div {
    ui::press_text()
        .text_color(white(ink::FULL))
        .child("Eclipse")
}

const BAR_HEIGHT: f32 = 32.;
const BUTTON_WIDTH: f32 = 46.;
const GLYPH: f32 = 14.;
const GLYPH_SQUARE: f32 = 10.;
const GLYPH_STROKE: f32 = 1.;
const GLYPH_INSET: f32 = (GLYPH - GLYPH_SQUARE) / 2.;
const MARK: f32 = ui::IconSize::Px16.px();

#[derive(Clone, Copy, PartialEq)]
enum Control {
    Minimize,
    Maximize,
    Close,
}

impl Control {
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    fn name(self) -> &'static str {
        match self {
            Self::Minimize => "window-minimize",
            Self::Maximize => "window-maximize",
            Self::Close => "window-close",
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn control_icon(control: Control, maximized: bool, color: gpui::Hsla) -> Div {
    use gpui::{div, prelude::*, px};
    let square = || {
        div()
            .absolute()
            .size(px(GLYPH_SQUARE))
            .border_1()
            .border_color(color)
    };
    let icon = div().relative().size(px(GLYPH));
    match control {
        Control::Minimize => icon.child(
            div()
                .absolute()
                .left(px(GLYPH_INSET))
                .top(px((GLYPH - GLYPH_STROKE) / 2.))
                .w(px(GLYPH_SQUARE))
                .h(px(GLYPH_STROKE))
                .bg(color),
        ),
        Control::Maximize if maximized => icon
            .child(
                div()
                    .absolute()
                    .left(px(2. * GLYPH_INSET))
                    .top_0()
                    .size(px(GLYPH_SQUARE))
                    .border_t_1()
                    .border_r_1()
                    .border_color(color),
            )
            .child(
                square()
                    .left(px(GLYPH_STROKE))
                    .top(px(GLYPH - GLYPH_SQUARE - GLYPH_STROKE)),
            ),
        Control::Maximize => icon.child(square().left(px(GLYPH_INSET)).top(px(GLYPH_INSET))),
        Control::Close => icon.flex().items_center().justify_center().child(ui::icon(
            "cancel-01",
            ui::IconSize::Px16,
            color,
        )),
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn control_face(control: Control, maximized: bool) -> gpui::Stateful<Div> {
    use gpui::{prelude::*, px};
    let id: gpui::SharedString = control.name().into();
    let hovered = ui::hover_is(&id);
    let (hover, color) = match control {
        Control::Close if hovered => (ui::Hover::Danger, ui::danger()),
        Control::Close => (ui::Hover::Danger, white(ink::BODY)),
        _ if hovered => (ui::Hover::Bare, white(ink::FULL)),
        _ => (ui::Hover::Bare, white(ink::BODY)),
    };
    ui::hoverable(id, 0., hover)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .w(px(BUTTON_WIDTH))
        .h_full()
        .child(control_icon(control, maximized, color).relative())
}

/// Whether Windows uses a transparent native title bar for the launcher's custom chrome.
pub const DRAWN: bool = cfg!(target_os = "windows");

pub fn options() -> TitlebarOptions {
    TitlebarOptions {
        title: Some("Eclipse".into()),
        appears_transparent: DRAWN,
        ..Default::default()
    }
}

#[cfg(all(not(target_os = "windows"), not(target_os = "linux")))]
pub fn render(_theme: &Theme, _window: &Window) -> Option<Div> {
    None
}

/// Draw the launcher's Linux title bar when the platform uses client-side decorations.
#[cfg(target_os = "linux")]
pub fn render(theme: &Theme, window: &Window) -> Option<Div> {
    use crate::ui::Press;
    use gpui::{div, prelude::*, px, Decorations, MouseButton};

    if matches!(window.window_decorations(), Decorations::Server) {
        return None;
    }

    let controls = window.window_controls();
    let drag = div()
        .id("window-drag")
        .flex()
        .flex_1()
        .h_full()
        .items_center()
        .gap(px(space::SM))
        .px(px(space::MD))
        .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
        .child(mark(MARK, window.scale_factor()))
        .child(wordmark());
    let maximized = window.is_maximized();
    let shown = [
        (Control::Minimize, controls.minimize),
        (Control::Maximize, controls.maximize),
        (Control::Close, true),
    ];
    Some(
        div()
            .flex()
            .flex_none()
            .items_center()
            .h(px(BAR_HEIGHT))
            .bg(theme.side)
            .text_color(white(ink::MUTED))
            .border_b_1()
            .border_color(gpui::transparent_black())
            .child(drag)
            .children(shown.into_iter().filter(|(_, on)| *on).map(|(control, _)| {
                control_face(control, maximized)
                    .aria_label(control_label(control))
                    .on_press(move |_, window, _| match control {
                        Control::Minimize => window.minimize_window(),
                        Control::Maximize => window.zoom_window(),
                        Control::Close => window.remove_window(),
                    })
            })),
    )
}

#[cfg(target_os = "linux")]
fn control_label(control: Control) -> &'static str {
    eclipse_launcher::i18n::tr(match control {
        Control::Minimize => "Minimize",
        Control::Maximize => "Maximize",
        Control::Close => "Close",
    })
}

#[cfg(target_os = "linux")]
pub fn resize_handles(window: &Window) -> Option<Div> {
    use gpui::{div, prelude::*, px, CursorStyle, Decorations, MouseButton, ResizeEdge};

    if matches!(window.window_decorations(), Decorations::Server) || window.is_maximized() {
        return None;
    }

    let handle = |id, edge, cursor| {
        div()
            .id(id)
            .absolute()
            .cursor(cursor)
            .on_mouse_down(MouseButton::Left, move |_, window, _| {
                window.start_window_resize(edge)
            })
    };

    Some(
        div()
            .absolute()
            .size_full()
            .child(
                handle("resize-left", ResizeEdge::Left, CursorStyle::ResizeLeft)
                    .left_0()
                    .top(px(8.))
                    .bottom(px(8.))
                    .w(px(5.)),
            )
            .child(
                handle("resize-right", ResizeEdge::Right, CursorStyle::ResizeRight)
                    .right_0()
                    .top(px(8.))
                    .bottom(px(8.))
                    .w(px(5.)),
            )
            .child(
                handle("resize-bottom", ResizeEdge::Bottom, CursorStyle::ResizeDown)
                    .bottom_0()
                    .left(px(8.))
                    .right(px(8.))
                    .h(px(5.)),
            )
            .child(
                handle(
                    "resize-top-left",
                    ResizeEdge::TopLeft,
                    CursorStyle::ResizeUpLeftDownRight,
                )
                .left_0()
                .top_0()
                .size(px(8.)),
            )
            .child(
                handle(
                    "resize-top-right",
                    ResizeEdge::TopRight,
                    CursorStyle::ResizeUpRightDownLeft,
                )
                .right_0()
                .top_0()
                .size(px(8.)),
            )
            .child(
                handle(
                    "resize-bottom-left",
                    ResizeEdge::BottomLeft,
                    CursorStyle::ResizeUpRightDownLeft,
                )
                .left_0()
                .bottom_0()
                .size(px(8.)),
            )
            .child(
                handle(
                    "resize-bottom-right",
                    ResizeEdge::BottomRight,
                    CursorStyle::ResizeUpLeftDownRight,
                )
                .right_0()
                .bottom_0()
                .size(px(8.)),
            ),
    )
}

#[cfg(not(target_os = "linux"))]
pub fn resize_handles(_: &Window) -> Option<Div> {
    None
}

/// macOS keeps the window's appearance on the system setting, so a launcher set to Dark still
/// draws a light title bar on a light desktop. Pin the NSWindow's appearance to the choice
/// instead (`theme::pinned_dark`: `System` clears the pin, handing the chrome back to the
/// system). Windows uses custom chrome; Linux draws it only when the compositor requests it.
#[cfg(target_os = "macos")]
pub fn apply_appearance(window: &Window, choice: Appearance) {
    use objc2_app_kit::{
        NSAppearance, NSAppearanceCustomization, NSAppearanceName, NSAppearanceNameAqua,
        NSAppearanceNameDarkAqua, NSView,
    };
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    // AppKit's name constants are extern statics: reading them is `unsafe` to rustc only.
    let name: Option<&'static NSAppearanceName> = match crate::theme::pinned_dark(choice) {
        Some(true) => Some(unsafe { NSAppearanceNameDarkAqua }),
        Some(false) => Some(unsafe { NSAppearanceNameAqua }),
        None => None,
    };
    // The trait, not `Window::window_handle` (which is gpui's own `AnyWindowHandle`).
    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return;
    };
    // The handle is the window's content view; its `window` is the NSWindow itself. Both outlive
    // this call.
    let view = unsafe { appkit.ns_view.cast::<NSView>().as_ref() };
    let Some(native) = view.window() else {
        return;
    };
    let appearance = name.and_then(NSAppearance::appearanceNamed);
    native.setAppearance(appearance.as_deref());
}

#[cfg(not(target_os = "macos"))]
pub fn apply_appearance(_: &Window, _: Appearance) {}

#[cfg(target_os = "windows")]
mod windows {
    use super::*;
    use gpui::{div, prelude::*, px, Stateful, WindowControlArea};

    const RESIZE_INSET: f32 = 5.;

    impl Control {
        fn area(self) -> WindowControlArea {
            match self {
                Self::Minimize => WindowControlArea::Min,
                Self::Maximize => WindowControlArea::Max,
                Self::Close => WindowControlArea::Close,
            }
        }
    }

    fn control_button(control: Control, maximized: bool) -> Stateful<Div> {
        // GPUI maps these to native non-client hit tests, retaining Snap Layouts,
        // Alt+Space and close handling. Click callbacks would run the action twice.
        control_face(control, maximized).window_control_area(control.area())
    }

    pub fn render(theme: &Theme, window: &Window) -> Option<Div> {
        let drag = div()
            .id("window-drag")
            .flex()
            .flex_1()
            .h_full()
            .items_center()
            .gap(px(space::SM))
            .px(px(space::MD))
            .window_control_area(WindowControlArea::Drag)
            .child(mark(MARK, window.scale_factor()))
            .child(wordmark());
        Some(
            div()
                .flex()
                .flex_none()
                .items_center()
                .h(px(BAR_HEIGHT))
                // Control hit tests run before native edge resizing in GPUI 0.2.2.
                .when(!window.is_maximized(), |bar| {
                    bar.pt(px(RESIZE_INSET)).px(px(RESIZE_INSET))
                })
                .bg(theme.side)
                .text_color(white(ink::MUTED))
                .border_b_1()
                .border_color(gpui::transparent_black())
                .child(drag)
                .children(
                    [Control::Minimize, Control::Maximize, Control::Close]
                        .map(|control| control_button(control, window.is_maximized())),
                ),
        )
    }
}

#[cfg(target_os = "windows")]
pub use windows::render;

pub(super) fn mark(size: f32, scale: f32) -> Div {
    thread_local! {
        static MARKS: std::cell::RefCell<std::collections::HashMap<(u32, u32), std::sync::Arc<gpui::RenderImage>>> =
            Default::default();
    }
    let image = MARKS.with_borrow_mut(|marks| {
        marks
            .entry((size.to_bits(), scale.to_bits()))
            .or_insert_with(|| {
                crate::art::bake(&eclipse_theme::mark::texels(), 16, 16, size / 16., scale)
            })
            .clone()
    });
    gpui::div()
        .flex_none()
        .size(gpui::px(size))
        .child(gpui::img(image).size(gpui::px(size)))
}
