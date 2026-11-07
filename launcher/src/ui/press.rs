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

const LIST_ROW_GAP: f32 = eclipse_theme::glass::space::XS;

const RING_WIDTH: f32 = 4.;

const RING_GAP: f32 = 1.5;

const SPLIT_RING_GAP: f32 = 2.5;

const INSET_RING: f32 = 2.;

const TIP_GAP: f32 = eclipse_theme::glass::space::SM;

const SPARE_HANDLES: usize = 64;

const SPARE_PRESS_HANDLES: usize = 128;

const PAGE_STEP: usize = 8;

const TYPE_AHEAD: std::time::Duration = std::time::Duration::from_millis(900);

thread_local! {
    static SPOTS: RefCell<Vec<Rc<RefCell<Spot>>>> = const { RefCell::new(Vec::new()) };
    static LISTS: RefCell<HashMap<SharedString, Scrollbar>> = RefCell::default();
    static FOCUS_TIP: RefCell<Option<FocusTip>> = const { RefCell::new(None) };
    static MENU_OPENING: Cell<bool> = const { Cell::new(false) };
    static MENU_START: RefCell<Option<SharedString>> = const { RefCell::new(None) };
    static HANDLES: RefCell<HashMap<SharedString, FocusHandle>> = RefCell::default();
    static SPARE: RefCell<Vec<FocusHandle>> = const { RefCell::new(Vec::new()) };
    static SHORT_OF: Cell<usize> = const { Cell::new(0) };
    static PRESS_SPARE: RefCell<Vec<FocusHandle>> = const { RefCell::new(Vec::new()) };
    static PRESS_SHORT_OF: Cell<usize> = const { Cell::new(0) };
    static PRESSED: RefCell<HashSet<SharedString>> = RefCell::default();
    static KEPT: RefCell<HashMap<SharedString, FocusHandle>> = RefCell::default();
    static FOCUSED: RefCell<Option<FocusHandle>> = const { RefCell::new(None) };
    static RETRYING: RefCell<Option<(SharedString, FocusHandle)>> = const { RefCell::new(None) };
    static KEYBOARD: Cell<bool> = const { Cell::new(false) };
    static MODALS: RefCell<Modals> = RefCell::default();
    static CURSORS: RefCell<HashMap<SharedString, Cursor>> = RefCell::default();
    static REVEAL_SOON: RefCell<HashSet<SharedString>> = RefCell::default();
    static FOCUS_WHEN_SHOWN: RefCell<HashMap<SharedString, usize>> = RefCell::default();
    static TYPED: RefCell<(String, Option<std::time::Instant>)> = RefCell::default();
}

#[derive(Clone, PartialEq)]
struct FocusTip {
    label: SharedString,
    bounds: Bounds<Pixels>,
    handle: FocusHandle,
    side: TipSide,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum TipSide {
    Below,
    Beside,
    Above,
}

#[derive(Default)]
struct Modals {
    shown: Vec<SharedString>,
    last: Vec<SharedString>,
    openers: HashMap<SharedString, FocusHandle>,
}

#[derive(Clone, Default)]
struct Cursor {
    at: usize,
    row: Option<SharedString>,
}

#[derive(Default)]
struct Spot {
    bounds: Bounds<Pixels>,
    panes: Vec<(ScrollHandle, Pixels)>,
    row: bool,
}

pub(super) fn forget_spots() {
    SPOTS.with_borrow_mut(Vec::clear);
    PRESSED.with_borrow_mut(HashSet::clear);
    KEPT.with_borrow_mut(HashMap::clear);
    MENU_START.set(None);
}

fn element_name(el: &mut Stateful<Div>) -> Option<SharedString> {
    match el.interactivity().element_id.clone() {
        Some(gpui::ElementId::Name(id)) => Some(id),
        _ => None,
    }
}

pub fn skipped_by_tab(mut el: Stateful<Div>) -> Stateful<Div> {
    match press_handle(&mut el, true) {
        Some(handle) => el.track_focus(&handle.tab_stop(false)),
        None => el.tab_stop(false),
    }
}

pub fn keep_focus(mut el: Stateful<Div>, handle: &FocusHandle) -> Stateful<Div> {
    if let Some(id) = element_name(&mut el) {
        keep_focus_for(id, handle);
    }
    el.track_focus(handle)
}

pub fn keep_focus_for(id: impl Into<SharedString>, handle: &FocusHandle) {
    KEPT.with_borrow_mut(|kept| kept.insert(id.into(), handle.clone()));
}

fn press_handle(el: &mut Stateful<Div>, again: bool) -> Option<FocusHandle> {
    let id = element_name(el)?;
    if let Some(kept) = KEPT.with_borrow(|kept| kept.get(&id).cloned()) {
        return Some(kept);
    }
    let first = PRESSED.with_borrow_mut(|pressed| pressed.insert(id.clone()));
    if !first && !again {
        return None;
    }
    let key: SharedString = format!("press-{id}").into();
    HANDLES.with_borrow_mut(|handles| {
        if let Some(handle) = handles.get(&key) {
            return Some(handle.clone());
        }
        let Some(handle) = PRESS_SPARE.with_borrow_mut(Vec::pop) else {
            PRESS_SHORT_OF.set(PRESS_SHORT_OF.get() + 1);
            return None;
        };
        handles.insert(key, handle.clone());
        Some(handle)
    })
}

fn ring_while_focused(el: Stateful<Div>, handle: &FocusHandle, filled: bool) -> Stateful<Div> {
    if !keyboard_focus(handle) {
        return el;
    }
    let ring = div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .rounded(px(eclipse_theme::glass::radius::CONTROL))
        .shadow(if filled {
            gap_ring_shadows(RING_GAP)
        } else {
            inset_shadow()
        });
    el.child(deferred(ring))
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
    let held = press_handle(&mut el, false);
    let el = if el.a11y_role().is_none() {
        el.role(Role::Button)
    } else {
        el
    };
    let el = el.tab_index(0);
    let el = match held {
        Some(handle) => el.track_focus(&handle.tab_stop(true)),
        None => el,
    };
    let el = if filled {
        el.focus_visible(ring)
    } else {
        el.focus_visible(inset_ring)
    };
    reveal_on_tab(el)
}

pub(super) fn note_focus(window: &Window, cx: &mut App) {
    let focused = window.focused(cx);
    if RETRYING.with_borrow(|r| r.as_ref().is_some_and(|(_, h)| Some(h) != focused.as_ref())) {
        RETRYING.set(None);
    }
    FOCUSED.set(focused);
    KEYBOARD.set(window.last_input_was_keyboard());
    for (pool, short, size) in [
        (&SPARE, &SHORT_OF, SPARE_HANDLES),
        (&PRESS_SPARE, &PRESS_SHORT_OF, SPARE_PRESS_HANDLES),
    ] {
        let wanted = size + short.take();
        pool.with_borrow_mut(|spare| {
            while spare.len() < wanted {
                spare.push(cx.focus_handle().tab_index(0).tab_stop(true));
            }
        });
    }
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

pub(super) fn retrying(id: &str) {
    let key = SharedString::from(format!("press-{id}"));
    let handle = HANDLES.with_borrow(|handles| handles.get(&key).cloned());
    RETRYING.set(handle.map(|h| (id.to_string().into(), h)));
}

pub(super) fn retry_back(id: &str) {
    if RETRYING.with_borrow(|r| r.as_ref().is_some_and(|(at, _)| at == id)) {
        RETRYING.set(None);
    }
}

pub(super) fn keeps_retry_focus(el: Stateful<Div>) -> Stateful<Div> {
    match RETRYING.with_borrow(|r| r.as_ref().map(|(_, h)| h.clone())) {
        Some(handle) => el.track_focus(&handle.tab_stop(false)),
        None => el,
    }
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
    el.child(spot_measure(spot.clone(), false))
        .on_key_up(move |ev: &KeyUpEvent, window, cx| {
            if ev.keystroke.key != "tab" {
                return;
            }
            reveal_spot(&spot, window);
            cx.stop_propagation();
        })
}

fn spot_measure(spot: Rc<RefCell<Spot>>, row: bool) -> impl IntoElement {
    measure(move |bounds| {
        *spot.borrow_mut() = Spot {
            bounds,
            panes: Vec::new(),
            row,
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
        let margin = if spot.row {
            let room = (pane.bounds().size.height - bounds.size.height) / 2.;
            (bounds.size.height + px(LIST_ROW_GAP)).min(room.max(px(0.)))
        } else {
            px(REVEAL_MARGIN)
        };
        reveal(pane, bounds, margin);
        bounds.origin.y += pane.offset().y - before;
        window.refresh();
    }
}

pub fn options(
    list: Stateful<Div>,
    name: impl Into<SharedString>,
    rows: Vec<(Stateful<Div>, SharedString, bool)>,
    follow: bool,
    on_pick: impl Fn(usize, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    options_with_heads(list, name, rows, Vec::new(), follow, on_pick)
}

fn options_key(list: &str) -> String {
    format!("options-{list}")
}

pub fn options_has_keyboard(list: &str) -> bool {
    try_focus_for(options_key(list)).is_some_and(|h| keyboard_focus(&h))
}

pub fn options_focus(list: &str) -> Option<FocusHandle> {
    try_focus_for(options_key(list))
}

pub fn options_with_heads(
    mut list: Stateful<Div>,
    name: impl Into<SharedString>,
    mut rows: Vec<(Stateful<Div>, SharedString, bool)>,
    heads: Vec<(usize, AnyElement)>,
    follow: bool,
    on_pick: impl Fn(usize, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let id = match list.interactivity().element_id.clone() {
        Some(gpui::ElementId::Name(id)) => id,
        _ => SharedString::from("options"),
    };
    let handle = try_focus_for(options_key(&id));
    let count = rows.len();
    let labels: Vec<SharedString> = rows.iter().map(|r| r.1.clone()).collect();
    let keys: Rc<Vec<Option<SharedString>>> =
        Rc::new(rows.iter_mut().map(|r| element_name(&mut r.0)).collect());
    let first = rows.iter().position(|r| r.2);
    let kept = CURSORS.with_borrow(|c| c.get(&id).cloned()).map(|kept| {
        kept.row
            .and_then(|row| keys.iter().position(|k| k.as_ref() == Some(&row)))
            .unwrap_or(kept.at)
    });
    let shown_now = FOCUS_WHEN_SHOWN.with_borrow_mut(|w| {
        let at = w.get(&id).copied().filter(|&at| at < count)?;
        w.remove(&id);
        Some(at)
    });
    let cursor = if follow {
        first.or(kept)
    } else {
        kept.or(first)
    };
    let cursor = shown_now
        .or(cursor)
        .unwrap_or(0)
        .min(count.saturating_sub(1));
    if shown_now.is_some() {
        REVEAL_SOON.with_borrow_mut(|r| r.insert(id.clone()));
    }
    let cursor_at = {
        let keys = keys.clone();
        move |at: usize| Cursor {
            at,
            row: keys.get(at).cloned().flatten(),
        }
    };
    CURSORS.with_borrow_mut(|c| c.insert(id.clone(), cursor_at(cursor)));
    let ringed = handle.as_ref().is_some_and(keyboard_focus);
    let on_pick = Rc::new(on_pick);
    let mut heads = heads.into_iter().peekable();
    let mut children: Vec<AnyElement> = Vec::new();
    let rows = rows.into_iter().enumerate().map(|(i, (row, label, on))| {
        let (pick, key, at) = (on_pick.clone(), id.clone(), cursor_at.clone());
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
                CURSORS.with_borrow_mut(|c| c.insert(key.clone(), at(i)));
                pick(i, window, cx);
            });
        if i != cursor {
            return row;
        }
        let spot: Rc<RefCell<Spot>> = Rc::default();
        let (seen, list) = (spot.clone(), id.clone());
        row.child(spot_measure(spot, true))
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
    let name: SharedString = name.into();
    let list = if name.is_empty() {
        list
    } else {
        list.aria_label(name)
    };
    let list = match &handle {
        Some(handle) => list.track_focus(handle),
        None => list,
    };
    let list = match handle.clone().filter(|_| shown_now.is_some()) {
        Some(handle) => list.child(
            canvas(
                move |_, window, _| {
                    let handle = handle.clone();
                    window.on_next_frame(move |window, cx| window.focus(&handle, cx));
                },
                |_, _, _, _| {},
            )
            .absolute(),
        ),
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
                .with_borrow(|c| c.get(&key).map(|c| c.at))
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
            CURSORS.with_borrow_mut(|c| c.insert(key.clone(), cursor_at(to)));
            REVEAL_SOON.with_borrow_mut(|r| r.insert(key.clone()));
            if follow && to != cursor {
                on_pick(to, window, cx);
            }
            window.refresh();
            cx.stop_propagation();
        })
}

pub fn options_cursor(id: &str) -> usize {
    CURSORS.with_borrow(|c| c.get(id).map_or(0, |c| c.at))
}

pub fn options_step(id: &str, by: isize, count: usize) {
    let to = options_cursor(id)
        .saturating_add_signed(by)
        .min(count.saturating_sub(1));
    set_options_cursor(id, to);
}

pub fn set_options_cursor(id: &str, at: usize) {
    start_options_cursor(id, at);
    REVEAL_SOON.with_borrow_mut(|r| r.insert(SharedString::from(id.to_string())));
}

pub fn start_options_cursor(id: &str, at: usize) {
    let id = SharedString::from(id.to_string());
    CURSORS.with_borrow_mut(|c| c.insert(id, Cursor { at, row: None }));
}

pub fn focus_option(id: &str, at: usize) {
    FOCUS_WHEN_SHOWN.with_borrow_mut(|w| w.insert(SharedString::from(id.to_string()), at));
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

fn gap_ring_shadows(gap: f32) -> Vec<gpui::BoxShadow> {
    vec![
        shadow(gap_colour(), INSET_RING + gap, true),
        shadow(color(look().accent_soft), INSET_RING, true),
    ]
}

pub(super) fn gap_ring(s: StyleRefinement) -> StyleRefinement {
    s.shadow(gap_ring_shadows(SPLIT_RING_GAP))
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
    LISTS.with_borrow_mut(|l| {
        l.entry(id.into())
            .or_default()
            .handle
            .scroll_to_top_of_item(index)
    });
}

fn reveal(pane: &ScrollHandle, bounds: Bounds<Pixels>, margin: Pixels) {
    let view = pane.bounds();
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

pub(super) fn tip_on_focus(el: Stateful<Div>, label: SharedString, side: TipSide) -> Stateful<Div> {
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
                    side,
                }));
                window.refresh();
            }
        })
}

pub(super) fn held_focus(el: Stateful<Div>, tip: Option<SharedString>) -> Stateful<Div> {
    let mut el = el;
    let filled = solid(&mut el);
    let handle = press_handle(&mut el, true);
    let el = if el.a11y_role().is_none() {
        el.role(Role::Button)
    } else {
        el
    };
    let Some(handle) = handle else {
        return reveal_on_tab(el.tab_index(0));
    };
    let el = reveal_on_tab(el.track_focus(&handle.clone().tab_stop(true)));
    if !keyboard_focus(&handle) {
        return el;
    }
    let el = ring_while_focused(el, &handle, filled);
    let Some(label) = tip else {
        return el;
    };
    let tip = canvas(
        move |bounds, window, _| {
            let tip = Some(FocusTip {
                label: label.clone(),
                bounds,
                handle: handle.clone(),
                side: TipSide::Below,
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
    el.child(tip)
}

pub(super) fn busy_focus(mut el: Stateful<Div>) -> Stateful<Div> {
    let filled = solid(&mut el);
    match press_handle(&mut el, true) {
        Some(handle) => {
            let el = el.track_focus(&handle.clone().tab_stop(false));
            ring_while_focused(el, &handle, filled)
        }
        None => el.tab_index(0).tab_stop(false),
    }
}

pub(super) fn busy_ringed(mut el: Stateful<Div>) -> Stateful<Div> {
    let el = match press_handle(&mut el, true) {
        Some(handle) => el.track_focus(&handle.tab_stop(false)),
        None => el.tab_index(0).tab_stop(false),
    };
    el.focus_visible(ring)
}

pub fn focus_tip(window: &Window) -> Option<AnyElement> {
    let FocusTip {
        label,
        bounds,
        handle,
        side,
    } = FOCUS_TIP.with_borrow(|t| t.clone())?;
    if !handle.is_focused(window) || !window.last_input_was_keyboard() {
        FOCUS_TIP.set(None);
        return None;
    }
    let (width, height) = super::tip_size(&label);
    let gap = px(TIP_GAP);
    let beside = point(bounds.right() + gap, bounds.center().y - px(height / 2.));
    let above = point(bounds.left(), bounds.top() - gap - px(height));
    let at = match side {
        TipSide::Beside if beside.x + px(width) + gap <= window.viewport_size().width => beside,
        TipSide::Above if above.y >= px(0.) => above,
        _ => point(bounds.left(), bounds.bottom() + gap),
    };
    Some(
        deferred(
            gpui::anchored()
                .position(at)
                .snap_to_window_with_margin(px(TIP_GAP))
                .child(tip_body(label)),
        )
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

pub fn menu_opened() {
    MENU_OPENING.set(true);
}

pub fn starts_menu(mut row: Stateful<Div>) -> Stateful<Div> {
    MENU_START.set(element_name(&mut row));
    row
}

fn menu_start() -> Option<FocusHandle> {
    let id = MENU_START.with_borrow(Clone::clone)?;
    HANDLES.with_borrow(|handles| {
        handles
            .get(&SharedString::from(format!("press-{id}")))
            .cloned()
    })
}

pub(super) fn menu_keys(menu: Stateful<Div>) -> Stateful<Div> {
    let Some(handle) = try_focus_for("menu") else {
        return menu;
    };
    let opening = handle.clone();
    menu.track_focus(&handle)
        .tab_group()
        .tab_stop(false)
        .child(canvas(
            move |_, window, cx| {
                if MENU_OPENING.take() {
                    window.defer(cx, move |window, cx| {
                        if let Some(start) = menu_start() {
                            window.focus(&start, cx);
                        }
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

fn modal_focus(id: &SharedString) -> FocusHandle {
    focus_for(format!("modal-{id}"))
}

pub(super) fn modal_keys(scope: Stateful<Div>, id: SharedString) -> Stateful<Div> {
    let Some(handle) = try_focus_for(format!("modal-{id}")) else {
        return scope;
    };
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
