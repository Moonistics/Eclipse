use super::{color, look, white, IconSize, KNOB_EDGE};
use eclipse_theme::glass::{fill, radius, ON_LIGHT};
use gpui::{div, prelude::*, px, App, Div, Hsla, Role, SharedString, Stateful, Window};

const SLIDER_HEIGHT: f32 = super::TILE_SM;

const SLIDER_KNOB: f32 = IconSize::Px16.px();

const SLIDER_PAGE: i32 = 10;

const HUE_STEPS: usize = 60;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Track {
    Fill,
    Hues,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SliderMove {
    To(f32),
    Step(i32),
    Done,
}

thread_local! {
    static SLIDERS: std::cell::RefCell<std::collections::HashMap<SharedString, gpui::Bounds<gpui::Pixels>>> =
        std::cell::RefCell::default();
    static DRAGGING: std::cell::RefCell<Option<SharedString>> = const { std::cell::RefCell::new(None) };
}

fn slider_at(id: &SharedString, x: gpui::Pixels) -> Option<f32> {
    let bounds = SLIDERS.with_borrow(|s| s.get(id).copied())?;
    let width: f32 = bounds.size.width.into();
    let across: f32 = (x - bounds.origin.x).into();
    (width > 0.).then(|| (across / width).clamp(0., 1.))
}

#[allow(clippy::too_many_arguments)]
pub fn slider(
    id: impl Into<SharedString>,
    name: impl Into<SharedString>,
    fraction: f32,
    value: f64,
    range: (f64, f64),
    track: Track,
    on_move: impl Fn(SliderMove, &mut Window, &mut App) + Clone + 'static,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let fraction = fraction.clamp(0., 1.);
    let width = crate::layout::FIELD_MD;
    let measured = id.clone();
    let bar = match track {
        Track::Fill => div()
            .h(px(super::BAR_HEIGHT))
            .w_full()
            .rounded_full()
            .overflow_hidden()
            .bg(white(fill::ACTIVE))
            .child(
                div()
                    .h_full()
                    .w(gpui::relative(fraction))
                    .rounded_full()
                    .bg(color(look().accent_soft)),
            )
            .into_any_element(),
        Track::Hues => gpui::canvas(
            |_, _, _| {},
            |bounds, _, window, _| {
                let step = f32::from(bounds.size.width) / HUE_STEPS as f32;
                for i in 0..HUE_STEPS {
                    let hue = i as f32 * 360. / HUE_STEPS as f32;
                    let colour: Hsla = gpui::rgb(eclipse_launcher::themes::hue_colour(hue)).into();
                    window.paint_quad(gpui::fill(
                        gpui::Bounds::new(
                            gpui::point(bounds.left() + px(step * i as f32), bounds.top()),
                            gpui::size(px(step), bounds.size.height),
                        ),
                        colour,
                    ));
                }
            },
        )
        .h(px(SLIDER_KNOB))
        .w_full()
        .rounded(px(radius::BLOCK))
        .into_any_element(),
    };
    let knob = div()
        .absolute()
        .left(px(fraction * (width - SLIDER_KNOB)))
        .size(px(SLIDER_KNOB))
        .rounded_full()
        .bg(super::paper())
        .border_1()
        .border_color(color(ON_LIGHT).opacity(KNOB_EDGE));
    let (down, moved, up, out, keys) = (
        on_move.clone(),
        on_move.clone(),
        on_move.clone(),
        on_move.clone(),
        on_move,
    );
    let (down_id, move_id) = (id.clone(), id.clone());
    let el = div()
        .id(id.clone())
        .role(Role::Slider)
        .aria_label(name.into())
        .aria_numeric_value(value)
        .aria_min_numeric_value(range.0)
        .aria_max_numeric_value(range.1)
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .w(px(width))
        .h(px(SLIDER_HEIGHT))
        .cursor_pointer()
        .child(
            gpui::canvas(
                move |bounds, _, _| {
                    SLIDERS.with_borrow_mut(|s| s.insert(measured.clone(), bounds));
                },
                |_, _, _, _| {},
            )
            .absolute()
            .size_full(),
        )
        .child(div().w_full().overflow_hidden().rounded_full().child(bar))
        .child(knob);
    super::focus_stop(el)
        .on_key_down(move |ev: &gpui::KeyDownEvent, window, cx| {
            let to = match ev.keystroke.key.as_str() {
                "left" | "down" => SliderMove::Step(-1),
                "right" | "up" => SliderMove::Step(1),
                "pagedown" => SliderMove::Step(-SLIDER_PAGE),
                "pageup" => SliderMove::Step(SLIDER_PAGE),
                "home" => SliderMove::To(0.),
                "end" => SliderMove::To(1.),
                _ => return,
            };
            keys(to, window, cx);
            if matches!(to, SliderMove::To(_)) {
                keys(SliderMove::Done, window, cx);
            }
            cx.stop_propagation();
        })
        .on_mouse_down(gpui::MouseButton::Left, move |ev, window, cx| {
            DRAGGING.set(Some(down_id.clone()));
            if let Some(at) = slider_at(&down_id, ev.position.x) {
                down(SliderMove::To(at), window, cx);
            }
        })
        .on_mouse_move(move |ev: &gpui::MouseMoveEvent, window, cx| {
            let dragging = DRAGGING.with_borrow(|d| d.as_ref() == Some(&move_id));
            if !dragging || ev.pressed_button != Some(gpui::MouseButton::Left) {
                return;
            }
            if let Some(at) = slider_at(&move_id, ev.position.x) {
                moved(SliderMove::To(at), window, cx);
            }
        })
        .on_mouse_up(gpui::MouseButton::Left, move |_, window, cx| {
            if DRAGGING.take().is_some() {
                up(SliderMove::Done, window, cx);
            }
        })
        .on_mouse_up_out(gpui::MouseButton::Left, move |_, window, cx| {
            if DRAGGING.take().is_some() {
                out(SliderMove::Done, window, cx);
            }
        })
}
