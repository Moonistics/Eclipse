use super::{color, light, look, tip_body};
use crate::scrollbar::Scrollbar;
use eclipse_theme::glass::{PAGE, PAPER};
use gpui::{
    canvas, deferred, div, point, prelude::*, px, AnyElement, App, Bounds, ClickEvent, Div,
    Element, FocusHandle, KeyDownEvent, KeyUpEvent, Pixels, Role, ScrollEvent, ScrollHandle,
    SharedString, Stateful, StyleRefinement, Window,
};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

const REVEAL_MARGIN: f32 = 24.;

const RING_WIDTH: f32 = 4.;

const RING_GAP: f32 = 1.5;

const INSET_RING: f32 = 2.;

const TIP_GAP: f32 = eclipse_theme::glass::space::SM;

const SPARE_HANDLES: usize = 64;

const PAGE_STEP: usize = 8;

const TYPE_AHEAD: std::time::Duration = std::time::Duration::from_millis(900);

thread_local! {
    static SPOTS: RefCell<Vec<Rc<RefCell<Spot>>>> = const { RefCell::new(Vec::new()) };
    static LISTS: RefCell<HashMap<SharedString, Scrollbar>> = RefCell::default();
    static FOCUS_TIP: RefCell<Option<FocusTip>> = const { RefCell::new(None) };
    static MENU_OPENING: Cell<bool> = const { Cell::new(false) };
    static HANDLES: RefCell<HashMap<SharedString, FocusHandle>> = RefCell::default();
    static SPARE: RefCell<Vec<FocusHandle>> = const { RefCell::new(Vec::new()) };
    static SHORT_OF: Cell<usize> = const { Cell::new(0) };
    static FOCUSED: RefCell<Option<FocusHandle>> = const { RefCell::new(None) };
    static KEYBOARD: Cell<bool> = const { Cell::new(false) };
    static MODALS: RefCell<Modals> = RefCell::default();
    static CURSORS: RefCell<HashMap<SharedString, usize>> = RefCell::default();
    static REVEAL_SOON: RefCell<HashSet<SharedString>> = RefCell::default();
    static TYPED: RefCell<(String, Option<std::time::Instant>)> = RefCell::default();
}

#[derive(Clone, PartialEq)]
struct FocusTip {
    label: SharedString,
    bounds: Bounds<Pixels>,
    handle: FocusHandle,
    beside: bool,
}

#[derive(Default)]
struct Modals {
    shown: Vec<SharedString>,
    last: Vec<SharedString>,
    openers: HashMap<SharedString, FocusHandle>,
}

#[derive(Default)]
struct Spot {
    bounds: Bounds<Pixels>,
    panes: Vec<(ScrollHandle, Pixels)>,
}

pub(super) fn forget_spots() {
    SPOTS.with_borrow_mut(Vec::clear);
}

pub trait Press: Sized {
    fn on_press(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self;
}

impl Press for Stateful<Div> {
    fn on_press(self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        focus_stop(self).on_click(f)
    }
}

fn solid(el: &mut Stateful<Div>) -> bool {
    el.interactivity()
        .base_style
        .background
        .as_ref()
        .and_then(gpui::Fill::color)
        .and_then(|fill| fill.as_solid())
        .is_some_and(|c| c.a >= 1.)
}

pub fn focus_stop(mut el: Stateful<Div>) -> Stateful<Div> {
    let filled = solid(&mut el);
    let el = if el.a11y_role().is_none() {
        el.role(Role::Button)
    } else {
        el
    };
    let el = el.tab_index(0);
    let el = if filled {
        el.focus_visible(ring)
    } else {
        el.focus_visible(inset_ring)
    };
    reveal_on_tab(el)
}

pub(super) fn note_focus(window: &Window, cx: &mut App) {
    FOCUSED.set(window.focused(cx));
    KEYBOARD.set(window.last_input_was_keyboard());
    let wanted = SPARE_HANDLES + SHORT_OF.take();
    SPARE.with_borrow_mut(|spare| {
        while spare.len() < wanted {
            spare.push(cx.focus_handle().tab_index(0).tab_stop(true));
        }
    });
    MODALS.with_borrow_mut(|m| m.last = std::mem::take(&mut m.shown));
}

pub fn focus_for(key: impl Into<SharedString>) -> FocusHandle {
    try_focus_for(key).expect("a spare focus handle each frame")
}

pub fn try_focus_for(key: impl Into<SharedString>) -> Option<FocusHandle> {
    let key = key.into();
    HANDLES.with_borrow_mut(|handles| {
        if let Some(handle) = handles.get(&key) {
            return Some(handle.clone());
        }
        let Some(handle) = SPARE.with_borrow_mut(Vec::pop) else {
            SHORT_OF.set(SHORT_OF.get() + 1);
            return None;
        };
        handles.insert(key, handle.clone());
        Some(handle)
    })
}

pub fn has_focus(handle: &FocusHandle) -> bool {
    FOCUSED.with_borrow(|f| f.as_ref() == Some(handle))
}

pub fn keyboard_input() -> bool {
    KEYBOARD.get()
}

pub fn keyboard_focus(handle: &FocusHandle) -> bool {
    KEYBOARD.get() && has_focus(handle)
}

pub fn reveal_on_tab(el: Stateful<Div>) -> Stateful<Div> {
    let spot: Rc<RefCell<Spot>> = Rc::default();
    el.child(spot_measure(spot.clone()))
        .on_key_up(move |ev: &KeyUpEvent, window, cx| {
            if ev.keystroke.key != "tab" {
                return;
            }
            reveal_spot(&spot, window);
            cx.stop_propagation();
        })
}

fn spot_measure(spot: Rc<RefCell<Spot>>) -> impl IntoElement {
    measure(move |bounds| {
        *spot.borrow_mut() = Spot {
            bounds,
            panes: Vec::new(),
        };
        SPOTS.with_borrow_mut(|spots| spots.push(spot));
    })
}

fn reveal_spot(spot: &Rc<RefCell<Spot>>, window: &mut Window) {
    let spot = spot.borrow();
    let mut bounds = spot.bounds;
    for (pane, measured_at) in &spot.panes {
        bounds.origin.y += pane.offset().y - *measured_at;
        let before = pane.offset().y;
        reveal(pane, bounds);
        bounds.origin.y += pane.offset().y - before;
        window.refresh();
    }
}

pub fn options(
    list: Stateful<Div>,
    rows: Vec<(Stateful<Div>, SharedString, bool)>,
    follow: bool,
    on_pick: impl Fn(usize, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    options_with_heads(list, rows, Vec::new(), follow, on_pick)
}

pub fn options_with_heads(
    mut list: Stateful<Div>,
    rows: Vec<(Stateful<Div>, SharedString, bool)>,
    heads: Vec<(usize, AnyElement)>,
    follow: bool,
    on_pick: impl Fn(usize, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let id = match list.interactivity().element_id.clone() {
        Some(gpui::ElementId::Name(id)) => id,
        _ => SharedString::from("options"),
    };
    let handle = try_focus_for(format!("options-{id}"));
    let count = rows.len();
    let labels: Vec<SharedString> = rows.iter().map(|r| r.1.clone()).collect();
    let first = rows.iter().position(|r| r.2);
    let kept = CURSORS.with_borrow(|c| c.get(&id).copied());
    let cursor = if follow {
        first.or(kept)
    } else {
        kept.or(first)
    }
    .unwrap_or(0)
    .min(count.saturating_sub(1));
    CURSORS.with_borrow_mut(|c| c.insert(id.clone(), cursor));
    let ringed = handle.as_ref().is_some_and(keyboard_focus);
    let on_pick = Rc::new(on_pick);
    let mut heads = heads.into_iter().peekable();
    let mut children: Vec<AnyElement> = Vec::new();
    let rows = rows.into_iter().enumerate().map(|(i, (row, label, on))| {
        let (pick, key) = (on_pick.clone(), id.clone());
        let row = if row.a11y_role().is_none() {
            row.role(Role::ListBoxOption)
        } else {
            row
        };
        let row = row
            .aria_label(label)
            .aria_selected(on)
            .aria_position_in_set(i + 1)
            .aria_size_of_set(count)
            .cursor_pointer()
            .on_click(move |_, window, cx| {
                CURSORS.with_borrow_mut(|c| c.insert(key.clone(), i));
                pick(i, window, cx);
            });
        if i != cursor {
            return row;
        }
        let spot: Rc<RefCell<Spot>> = Rc::default();
        let (seen, list) = (spot.clone(), id.clone());
        row.child(spot_measure(spot))
            .child(
                canvas(
                    move |_, window, _| {
                        if REVEAL_SOON.with_borrow_mut(|r| r.remove(&list)) {
                            window.on_next_frame(move |window, _| reveal_spot(&seen, window));
                            window.request_animation_frame();
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute(),
            )
            .when(ringed, |r| {
                r.shadow(inset_shadow()).aria_active_descendant()
            })
    });
    for (i, row) in rows.enumerate() {
        while let Some((_, head)) = heads.next_if(|(at, _)| *at <= i) {
            children.push(head);
        }
        children.push(row.into_any_element());
    }
    children.extend(heads.map(|(_, head)| head));
    let key = id.clone();
    let soon = id.clone();
    let focus = handle.clone();
    let list = if list.a11y_role().is_none() {
        list.role(Role::ListBox)
    } else {
        list
    };
    let list = match &handle {
        Some(handle) => list.track_focus(handle),
        None => list,
    };
    list.children(children)
        .on_key_up(move |ev: &KeyUpEvent, window, cx| {
            if ev.keystroke.key != "tab" {
                return;
            }
            REVEAL_SOON.with_borrow_mut(|r| r.insert(soon.clone()));
            window.refresh();
            cx.stop_propagation();
        })
        .on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
            if let Some(focus) = &focus {
                window.focus(focus, cx)
            }
        })
        .on_key_down(move |ev: &KeyDownEvent, window, cx| {
            if count == 0 {
                return;
            }
            let k = &ev.keystroke;
            let last = count - 1;
            let cursor = CURSORS
                .with_borrow(|c| c.get(&key).copied())
                .unwrap_or(cursor)
                .min(last);
            let to = match k.key.as_str() {
                "up" | "left" => cursor.saturating_sub(1),
                "down" | "right" => (cursor + 1).min(last),
                "pageup" => cursor.saturating_sub(PAGE_STEP),
                "pagedown" => (cursor + PAGE_STEP).min(last),
                "home" => 0,
                "end" => last,
                "space" | "enter" => {
                    on_pick(cursor, window, cx);
                    cx.stop_propagation();
                    return;
                }
                _ => match typed_match(k, &labels, cursor) {
                    Some(at) => at,
                    None => return,
                },
            };
            CURSORS.with_borrow_mut(|c| c.insert(key.clone(), to));
            REVEAL_SOON.with_borrow_mut(|r| r.insert(key.clone()));
            if follow && to != cursor {
                on_pick(to, window, cx);
            }
            window.refresh();
            cx.stop_propagation();
        })
}

pub fn options_cursor(id: &str) -> usize {
    CURSORS.with_borrow(|c| c.get(id).copied().unwrap_or(0))
}

fn typed_match(k: &gpui::Keystroke, labels: &[SharedString], cursor: usize) -> Option<usize> {
    let m = &k.modifiers;
    if m.control || m.alt || m.platform {
        return None;
    }
    let typed = k.key_char.as_deref().filter(|c| c.chars().count() == 1)?;
    if typed.trim().is_empty() {
        return None;
    }
    let prefix = TYPED.with_borrow_mut(|(buffer, at)| {
        if at.is_none_or(|at| at.elapsed() > TYPE_AHEAD) {
            buffer.clear();
        }
        buffer.push_str(&typed.to_lowercase());
        *at = Some(std::time::Instant::now());
        buffer.clone()
    });
    let start = if prefix.chars().count() == 1 {
        cursor + 1
    } else {
        cursor
    };
    (0..labels.len())
        .map(|step| (start + step) % labels.len())
        .find(|&i| labels[i].to_lowercase().starts_with(&prefix))
}

fn shadow(color: gpui::Hsla, spread: f32, inset: bool) -> gpui::BoxShadow {
    gpui::BoxShadow {
        color,
        offset: point(px(0.), px(0.)),
        blur_radius: px(if inset { 0. } else { 0.5 }),
        spread_radius: px(spread),
        inset,
    }
}

fn gap_colour() -> gpui::Hsla {
    color(if light() { PAPER } else { PAGE })
}

fn ring(s: StyleRefinement) -> StyleRefinement {
    s.shadow(vec![
        shadow(color(look().accent_soft), RING_WIDTH, false),
        shadow(gap_colour(), RING_GAP, false),
    ])
}

pub(super) fn inset_ring(s: StyleRefinement) -> StyleRefinement {
    s.shadow(inset_shadow())
}

fn inset_shadow() -> Vec<gpui::BoxShadow> {
    vec![shadow(color(look().accent_soft), INSET_RING, true)]
}

pub fn gap_ring(s: StyleRefinement) -> StyleRefinement {
    s.shadow(vec![
        shadow(gap_colour(), INSET_RING + RING_GAP, true),
        shadow(color(look().accent_soft), INSET_RING, true),
    ])
}

pub fn measure(f: impl FnOnce(Bounds<Pixels>) + 'static) -> impl IntoElement {
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
            SPOTS.with_borrow_mut(|spots| {
                for spot in spots.iter() {
                    let mut spot = spot.borrow_mut();
                    let at = spot.bounds.center();
                    let inside = at.x >= view.left()
                        && at.x <= view.right()
                        && children.iter().any(|child| child.contains(&at));
                    if inside {
                        spot.panes.push((pane.clone(), offset.y));
                    }
                }
            })
        })
        .id(id.into())
        .overflow_y_scroll()
        .track_scroll(handle)
}

pub fn nested_list(id: impl Into<SharedString>) -> Stateful<Div> {
    let id: SharedString = id.into();
    let handle = LISTS.with_borrow_mut(|l| l.entry(id.clone()).or_default().handle.clone());
    hand_back_at_ends(scroll_pane(id, &handle), &handle)
}

pub fn with_scrollbar(mut list: Stateful<Div>) -> Div {
    let Some(gpui::ElementId::Name(id)) = list.interactivity().element_id.clone() else {
        return div().child(list);
    };
    let bar = LISTS.with_borrow_mut(|l| l.entry(id).or_default().clone());
    bar.wrap_nested(list)
}

pub fn hand_back_at_ends(pane: Stateful<Div>, handle: &ScrollHandle) -> Stateful<Div> {
    let at = handle.clone();
    pane.on_scroll(move |ev: &ScrollEvent, window, cx| {
        let dy = ev.delta.pixel_delta(window.line_height()).y;
        let (offset, max) = (at.offset().y, at.max_offset().y);
        let at_end = (dy > px(0.) && offset >= px(0.)) || (dy < px(0.) && -offset >= max);
        if !at_end {
            cx.stop_propagation();
        }
    })
}

pub fn scroll_list_to(id: impl Into<SharedString>, index: usize) {
    LISTS.with_borrow_mut(|l| l.entry(id.into()).or_default().handle.scroll_to_item(index));
}

fn reveal(pane: &ScrollHandle, bounds: Bounds<Pixels>) {
    let view = pane.bounds();
    let margin = px(REVEAL_MARGIN);
    let mut offset = pane.offset();
    if bounds.top() < view.top() + margin {
        offset.y += view.top() + margin - bounds.top();
    } else if bounds.bottom() > view.bottom() - margin {
        offset.y -=
            (bounds.bottom() - (view.bottom() - margin)).min(bounds.top() - view.top() - margin);
    }
    offset.y = offset.y.min(px(0.)).max(-pane.max_offset().y);
    if offset != pane.offset() {
        pane.set_offset(offset);
    }
}

pub fn tip_on_focus(el: Stateful<Div>, label: SharedString) -> Stateful<Div> {
    tip_when_focused(el, label, false)
}

pub fn tip_beside_on_focus(el: Stateful<Div>, label: SharedString) -> Stateful<Div> {
    tip_when_focused(el, label, true)
}

fn tip_when_focused(el: Stateful<Div>, label: SharedString, beside: bool) -> Stateful<Div> {
    let spot = Rc::new(Cell::new(None));
    let seen = spot.clone();
    el.child(measure(move |bounds| seen.set(Some(bounds))))
        .on_key_up(move |ev: &KeyUpEvent, window, cx| {
            if ev.keystroke.key != "tab" {
                return;
            }
            if let (Some(bounds), Some(handle)) = (spot.get(), window.focused(cx)) {
                FOCUS_TIP.set(Some(FocusTip {
                    label: label.clone(),
                    bounds,
                    handle,
                    beside,
                }));
                window.refresh();
            }
        })
}

pub(super) fn held_focus(el: Stateful<Div>, label: SharedString) -> Stateful<Div> {
    let mut el = el;
    let key = match el.interactivity().element_id.clone() {
        Some(gpui::ElementId::Name(id)) => id,
        _ => SharedString::from("unavailable"),
    };
    let filled = solid(&mut el);
    let el = if el.a11y_role().is_none() {
        el.role(Role::Button)
    } else {
        el
    };
    let Some(handle) = try_focus_for(format!("held-{key}")) else {
        return reveal_on_tab(el.tab_index(0));
    };
    let ringed = keyboard_focus(&handle);
    let el = reveal_on_tab(el.track_focus(&handle));
    if !ringed {
        return el;
    }
    let ring = div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .rounded(px(eclipse_theme::glass::radius::CONTROL))
        .shadow(if filled {
            vec![
                shadow(gap_colour(), INSET_RING + RING_GAP, true),
                shadow(color(look().accent_soft), INSET_RING, true),
            ]
        } else {
            inset_shadow()
        });
    let tip = canvas(
        move |bounds, window, _| {
            let tip = Some(FocusTip {
                label: label.clone(),
                bounds,
                handle: handle.clone(),
                beside: false,
            });
            if FOCUS_TIP.with_borrow(|t| *t != tip) {
                FOCUS_TIP.set(tip);
                window.refresh();
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full();
    el.child(deferred(ring)).child(tip)
}

pub fn focus_tip(window: &Window) -> Option<AnyElement> {
    let FocusTip {
        label,
        bounds,
        handle,
        beside,
    } = FOCUS_TIP.with_borrow(|t| t.clone())?;
    if !handle.is_focused(window) || !window.last_input_was_keyboard() {
        FOCUS_TIP.set(None);
        return None;
    }
    let at = if beside {
        point(bounds.right() + px(TIP_GAP), bounds.top())
    } else {
        point(bounds.left(), bounds.bottom() + px(TIP_GAP))
    };
    Some(
        deferred(gpui::anchored().position(at).child(tip_body(label)))
            .with_priority(2)
            .into_any_element(),
    )
}

pub fn opens_row_menu(k: &gpui::Keystroke) -> bool {
    (k.key == "f10" && k.modifiers.shift) || k.key == "menu"
}

pub fn menu_focus() -> FocusHandle {
    focus_for("menu")
}

fn menu_start() -> FocusHandle {
    focus_for("menu-start")
}

pub fn menu_opened() {
    MENU_OPENING.set(true);
}

pub fn starts_menu(row: Stateful<Div>) -> Stateful<Div> {
    row.track_focus(&menu_start())
}

pub(super) fn menu_keys(menu: Stateful<Div>) -> Stateful<Div> {
    let handle = menu_focus();
    let opening = handle.clone();
    menu.track_focus(&handle)
        .tab_group()
        .tab_stop(false)
        .child(canvas(
            move |_, window, cx| {
                if MENU_OPENING.take() {
                    window.defer(cx, move |window, cx| {
                        window.focus(&menu_start(), cx);
                        if !opening.contains_focused(window, cx) {
                            first_row(&opening, window, cx);
                        }
                    });
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

pub fn modal_focus(id: &SharedString) -> FocusHandle {
    focus_for(format!("modal-{id}"))
}

pub(super) fn modal_keys(scope: Stateful<Div>, id: SharedString) -> Stateful<Div> {
    let handle = modal_focus(&id);
    let opened = MODALS.with_borrow_mut(|m| {
        m.shown.push(id.clone());
        let opened = !m.last.contains(&id);
        if let Some(opener) = FOCUSED.with_borrow(Clone::clone).filter(|_| opened) {
            m.openers.entry(id.clone()).or_insert(opener);
        }
        opened
    });
    let watch = handle.clone();
    scope
        .track_focus(&handle)
        .tab_group()
        .tab_stop(false)
        .child(canvas(
            move |_, window, cx| {
                if opened {
                    let watch = watch.clone();
                    window.defer(cx, move |window, cx| {
                        if !watch.contains_focused(window, cx) {
                            first_row(&watch, window, cx);
                        }
                    });
                }
            },
            |_, _, _, _| {},
        ))
}

pub fn modal_opened_from(id: impl Into<SharedString>, opener: Option<FocusHandle>) {
    if let Some(opener) = opener {
        MODALS.with_borrow_mut(|m| m.openers.insert(id.into(), opener));
    }
}

pub fn modal_open() -> bool {
    MODALS.with_borrow(|m| !m.shown.is_empty())
}

pub fn modal_tab(back: bool, window: &mut Window, cx: &mut App) -> bool {
    let Some(top) = MODALS.with_borrow(|m| m.shown.last().cloned()) else {
        return false;
    };
    let scope = modal_focus(&top);
    if back {
        window.focus_prev(cx);
    } else {
        window.focus_next(cx);
    }
    if !scope.contains_focused(window, cx) || scope.is_focused(window) {
        if back {
            last_row(&scope, window, cx);
        } else {
            first_row(&scope, window, cx);
        }
    }
    true
}

pub(super) fn return_from_modals(window: &mut Window, cx: &mut App) {
    let closed: Vec<_> = MODALS.with_borrow_mut(|m| {
        let gone: Vec<_> = m
            .last
            .iter()
            .filter(|id| !m.shown.contains(id))
            .cloned()
            .collect();
        gone.into_iter()
            .filter_map(|id| Some((modal_focus(&id), m.openers.remove(&id)?)))
            .collect()
    });
    for (scope, opener) in closed {
        if scope.contains_focused(window, cx) {
            window.focus(&opener, cx);
        }
    }
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
