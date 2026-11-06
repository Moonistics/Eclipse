use super::{color, light, look, tip_body};
use eclipse_theme::glass::{PAGE, PAPER};
use gpui::{
    canvas, deferred, div, point, prelude::*, px, AnyElement, App, Bounds, ClickEvent, Div,
    FocusHandle, KeyDownEvent, KeyUpEvent, Pixels, ScrollEvent, ScrollHandle, SharedString,
    Stateful, StyleRefinement, Window,
};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

const REVEAL_MARGIN: f32 = 24.;

const RING_WIDTH: f32 = 4.;

const RING_GAP: f32 = 1.5;

const TIP_GAP: f32 = 6.;

thread_local! {
    static UNCLAIMED: RefCell<Vec<Rc<RefCell<Spot>>>> = const { RefCell::new(Vec::new()) };
    static LISTS: RefCell<HashMap<SharedString, ScrollHandle>> = RefCell::default();
    static FOCUS_TIP: RefCell<Option<(SharedString, Bounds<Pixels>, FocusHandle)>> =
        const { RefCell::new(None) };
    static MENU: RefCell<Option<FocusHandle>> = const { RefCell::new(None) };
    static MENU_OPENING: Cell<bool> = const { Cell::new(false) };
}

#[derive(Default)]
struct Spot {
    bounds: Bounds<Pixels>,
    pane: Option<ScrollHandle>,
}

pub(super) fn forget_spots() {
    UNCLAIMED.with_borrow_mut(Vec::clear);
}

pub trait Press: Sized {
    fn on_press(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self;
}

impl Press for Stateful<Div> {
    fn on_press(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        let spot: Rc<RefCell<Spot>> = Rc::default();
        let seen = spot.clone();
        self.tab_index(0)
            .focus_visible(ring)
            .on_click(f)
            .child(measure(move |bounds| {
                *seen.borrow_mut() = Spot { bounds, pane: None };
                UNCLAIMED.with_borrow_mut(|u| u.push(seen));
            }))
            .on_key_up(move |ev: &KeyUpEvent, window, cx| {
                if ev.keystroke.key != "tab" {
                    return;
                }
                let spot = spot.borrow();
                if let Some(pane) = &spot.pane {
                    reveal(pane, spot.bounds);
                    window.refresh();
                }
                cx.stop_propagation();
            })
    }
}

fn ring(s: StyleRefinement) -> StyleRefinement {
    let gap = color(if light() { PAPER } else { PAGE });
    let shadow = |color, spread: f32| gpui::BoxShadow {
        color,
        offset: point(px(0.), px(0.)),
        blur_radius: px(0.5),
        spread_radius: px(spread),
        inset: false,
    };
    s.shadow(vec![
        shadow(color(look().accent_soft), RING_WIDTH),
        shadow(gap, RING_GAP),
    ])
}

fn measure(f: impl FnOnce(Bounds<Pixels>) + 'static) -> impl IntoElement {
    canvas(move |bounds, _, _| f(bounds), |_, _, _, _| {})
        .absolute()
        .top_0()
        .left_0()
        .size_full()
}

pub fn scroll_pane(id: impl Into<SharedString>, handle: &ScrollHandle) -> Stateful<Div> {
    let pane = handle.clone();
    div()
        .on_children_prepainted(move |_, _, _| {
            let (view, offset) = (pane.bounds(), pane.offset());
            let children: Vec<_> = (0..)
                .map_while(|i| pane.bounds_for_item(i))
                .map(|child| Bounds::new(child.origin + offset, child.size))
                .collect();
            UNCLAIMED.with_borrow_mut(|unclaimed| {
                unclaimed.retain(|spot| {
                    let mut spot = spot.borrow_mut();
                    let at = spot.bounds.center();
                    let inside = at.x >= view.left()
                        && at.x <= view.right()
                        && children.iter().any(|child| child.contains(&at));
                    if inside {
                        spot.pane = Some(pane.clone());
                    }
                    !inside
                })
            })
        })
        .id(id.into())
        .overflow_y_scroll()
        .track_scroll(handle)
}

pub fn nested_list(id: impl Into<SharedString>) -> Stateful<Div> {
    let id: SharedString = id.into();
    let handle = LISTS.with_borrow_mut(|l| l.entry(id.clone()).or_default().clone());
    let at = handle.clone();
    scroll_pane(id, &handle).on_scroll(move |ev: &ScrollEvent, window, cx| {
        let dy = ev.delta.pixel_delta(window.line_height()).y;
        let (offset, max) = (at.offset().y, at.max_offset().y);
        let at_end = (dy > px(0.) && offset >= px(0.)) || (dy < px(0.) && -offset >= max);
        if !at_end {
            cx.stop_propagation();
        }
    })
}

pub fn scroll_list_to(id: impl Into<SharedString>, index: usize) {
    LISTS.with_borrow_mut(|l| l.entry(id.into()).or_default().scroll_to_item(index));
}

fn reveal(pane: &ScrollHandle, bounds: Bounds<Pixels>) {
    let view = pane.bounds();
    let margin = px(REVEAL_MARGIN);
    let mut offset = pane.offset();
    if bounds.top() < view.top() + margin {
        offset.y += view.top() + margin - bounds.top();
    } else if bounds.bottom() > view.bottom() - margin {
        offset.y -= bounds.bottom() - (view.bottom() - margin);
    }
    offset.y = offset.y.min(px(0.)).max(-pane.max_offset().y);
    if offset != pane.offset() {
        pane.set_offset(offset);
    }
}

pub fn tip_on_focus(el: Stateful<Div>, label: SharedString) -> Stateful<Div> {
    let spot = Rc::new(Cell::new(None));
    let seen = spot.clone();
    el.child(measure(move |bounds| seen.set(Some(bounds))))
        .on_key_up(move |ev: &KeyUpEvent, window, cx| {
            if ev.keystroke.key != "tab" {
                return;
            }
            if let (Some(bounds), Some(focused)) = (spot.get(), window.focused(cx)) {
                FOCUS_TIP.set(Some((label.clone(), bounds, focused)));
                window.refresh();
            }
        })
}

pub fn focus_tip(window: &Window) -> Option<AnyElement> {
    let (label, bounds, handle) = FOCUS_TIP.with_borrow(|t| t.clone())?;
    if !handle.is_focused(window) || !window.last_input_was_keyboard() {
        FOCUS_TIP.set(None);
        return None;
    }
    let at = point(bounds.left(), bounds.bottom() + px(TIP_GAP));
    Some(
        deferred(gpui::anchored().position(at).child(tip_body(label)))
            .with_priority(2)
            .into_any_element(),
    )
}

pub fn set_menu_focus(handle: FocusHandle) {
    MENU.set(Some(handle));
}

pub fn menu_focus() -> Option<FocusHandle> {
    MENU.with_borrow(|m| m.clone())
}

pub fn menu_opened() {
    MENU_OPENING.set(true);
}

pub(super) fn menu_keys(menu: Stateful<Div>) -> Stateful<Div> {
    let Some(handle) = menu_focus() else {
        return menu;
    };
    let opening = handle.clone();
    menu.track_focus(&handle)
        .tab_group()
        .tab_stop(false)
        .child(canvas(
            move |_, window, cx| {
                if MENU_OPENING.take() {
                    window.defer(cx, move |window, cx| first_row(&opening, window, cx));
                }
            },
            |_, _, _, _| {},
        ))
        .on_key_down(move |ev: &KeyDownEvent, window, cx| {
            match ev.keystroke.key.as_str() {
                "down" => {
                    window.focus_next(cx);
                    if !handle.contains_focused(window, cx) {
                        first_row(&handle, window, cx);
                    }
                }
                "up" => {
                    window.focus_prev(cx);
                    if !handle.contains_focused(window, cx) || handle.is_focused(window) {
                        last_row(&handle, window, cx);
                    }
                }
                "home" => first_row(&handle, window, cx),
                "end" => last_row(&handle, window, cx),
                _ => return,
            }
            cx.stop_propagation();
        })
}

fn first_row(menu: &FocusHandle, window: &mut Window, cx: &mut App) {
    window.focus(menu, cx);
    window.focus_next(cx);
}

fn last_row(menu: &FocusHandle, window: &mut Window, cx: &mut App) {
    first_row(menu, window, cx);
    for _ in 0..64 {
        let at = window.focused(cx);
        window.focus_next(cx);
        if !menu.contains_focused(window, cx) || window.focused(cx) == at {
            if let Some(at) = at {
                window.focus(&at, cx);
            }
            return;
        }
    }
}
