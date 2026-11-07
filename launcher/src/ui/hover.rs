use super::white;
use eclipse_theme::glass::fill;
use gpui::{
    div, prelude::*, px, AnimationExt, AnyElement, Div, ElementId, Hsla, SharedString, Stateful,
    Window,
};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::sync::Mutex;
use std::time::Instant;

thread_local! {
    static OFF: RefCell<HashSet<SharedString>> = RefCell::default();
    static INSIDE: RefCell<HashSet<SharedString>> = RefCell::default();
    static LEFT_DISABLED: Cell<bool> = const { Cell::new(false) };
}

/// The element under the pointer and the one it just left, with when that happened. Hover
/// backgrounds ease in and out from these rather than snapping.
struct HoverState {
    on: Option<(SharedString, Instant)>,
    off: Option<(SharedString, Instant)>,
}

static HOVER: Mutex<HoverState> = Mutex::new(HoverState {
    on: None,
    off: None,
});

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hover {
    Row,
    Bare,
    Primary,
    Danger,
    Shade,
}

impl Hover {
    fn fill(self) -> Hsla {
        match self {
            Hover::Row => white(fill::HOVER),
            Hover::Bare => white(fill::BUTTON),
            Hover::Primary => gpui::white().opacity(fill::PRIMARY_HOVER),
            Hover::Danger => super::danger_fill(),
            Hover::Shade => gpui::black().opacity(fill::BUTTON),
        }
    }
}

pub fn hoverable(id: impl Into<SharedString>, r: f32, kind: Hover) -> Stateful<Div> {
    let id: SharedString = id.into();
    let fill = kind.fill();
    let layer = hover_fade(&id, layer(r), move |d, t| d.bg(fill.opacity(fill.a * t)));
    track_hover(div().id(id.clone()).relative().child(layer), id)
}

pub fn hover_over(id: impl Into<SharedString>, r: f32, content: impl IntoElement) -> Stateful<Div> {
    let id: SharedString = id.into();
    let fill = Hover::Bare.fill();
    let layer = hover_fade(&id, layer(r), move |d, t| d.bg(fill.opacity(fill.a * t)));
    track_hover(
        div().id(id.clone()).relative().child(content).child(layer),
        id,
    )
}

pub(crate) fn track_hover(el: Stateful<Div>, id: SharedString) -> Stateful<Div> {
    el.on_hover(move |hovered, window, _| note_hover(&id, *hovered, window))
}

/// Whether `id` is the element [`hover_fade`] shows hovered now.
pub fn hover_is(id: &SharedString) -> bool {
    HOVER
        .lock()
        .unwrap()
        .on
        .as_ref()
        .is_some_and(|(h, _)| h == id)
}

pub fn hover_within(id: &SharedString) -> bool {
    INSIDE.with_borrow(|inside| inside.contains(id))
}

/// Records that `id` is now hovered or not, for [`hover_fade`]: an element whose own hover
/// listener does more calls it.
pub(super) fn note_hover(id: &SharedString, hovered: bool, window: &mut Window) {
    let hovered = hovered && !OFF.with_borrow(|off| off.contains(id));
    INSIDE.with_borrow_mut(|inside| {
        if hovered {
            inside.insert(id.clone());
        } else {
            inside.remove(id);
        }
    });
    let mut state = HOVER.lock().unwrap();
    let is_on = state.on.as_ref().is_some_and(|(h, _)| h == id);
    if hovered && !is_on {
        state.off = state.on.take();
        state.on = Some((id.clone(), Instant::now()));
    } else if !hovered && is_on {
        state.off = Some((id.clone(), Instant::now()));
        state.on = None;
    }
    window.refresh();
}

pub fn disabled(el: Stateful<Div>) -> Stateful<Div> {
    super::busy_focus(
        unhovered(el)
            .opacity(crate::theme::DISABLED)
            .cursor_default()
            .aria_description(crate::tr("Unavailable")),
    )
}

pub fn unavailable(el: Stateful<Div>, why: impl Into<SharedString>) -> Stateful<Div> {
    held_back(el.opacity(crate::theme::DISABLED), why.into())
}

pub(super) fn held_back(el: Stateful<Div>, why: SharedString) -> Stateful<Div> {
    let el = unhovered(el)
        .cursor_default()
        .aria_description(why.clone())
        .tooltip(super::tip(why.clone()));
    super::held_focus(el, Some(why))
}

pub(super) fn held_back_said(el: Stateful<Div>, why: SharedString) -> Stateful<Div> {
    let el = unhovered(el).cursor_default().aria_description(why);
    super::held_focus(el, None)
}

pub(super) fn unhovered(mut el: Stateful<Div>) -> Stateful<Div> {
    if let Some(ElementId::Name(id)) = el.interactivity().element_id.clone() {
        let mut state = HOVER.lock().unwrap();
        if state.on.as_ref().is_some_and(|(h, _)| *h == id) {
            state.on = None;
            LEFT_DISABLED.set(true);
        }
        OFF.with_borrow_mut(|off| off.insert(id));
    }
    el
}

pub(super) fn forget_disabled() {
    OFF.with_borrow_mut(HashSet::clear);
}

pub(super) fn end_frame(window: &mut Window, cx: &mut gpui::App) {
    super::return_from_modals(window, cx);
    if LEFT_DISABLED.take() {
        window.request_animation_frame();
    }
}

/// A layer filling its parent, round at `r`, for hover fills.
pub(crate) fn layer(r: f32) -> impl Fn() -> Div {
    move || div().absolute().top_0().left_0().size_full().rounded(px(r))
}

pub(crate) fn row_fill(id: &SharedString, r: f32, held: bool) -> AnyElement {
    if held && !hover_is(id) {
        return layer(r)().bg(white(fill::HOVER)).into_any_element();
    }
    let fill = Hover::Row.fill();
    hover_fade(id, layer(r), move |d, t| d.bg(fill.opacity(fill.a * t)))
}

fn hover_fade(
    id: &SharedString,
    layer: impl FnOnce() -> Div,
    paint: impl Fn(Div, f32) -> Div + 'static,
) -> AnyElement {
    let state = HOVER.lock().unwrap();
    // Each transition gets its own animation id, from when it started, so it plays once.
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    let start = *START.get_or_init(Instant::now);
    let key = |dir: &str, at: Instant| {
        ElementId::named_usize(format!("{id}-{dir}"), (at - start).as_micros() as usize)
    };
    let ease = || crate::motion::eased(eclipse_theme::QUICK);
    if let Some((_, at)) = state.on.as_ref().filter(|(h, _)| h == id) {
        return layer()
            .with_animation(key("in", *at), ease(), paint)
            .into_any_element();
    }
    if let Some((_, at)) = state
        .off
        .as_ref()
        .filter(|(h, at)| h == id && at.elapsed() < crate::theme::QUICK)
    {
        return layer()
            .with_animation(key("out", *at), ease(), move |d, t| paint(d, 1. - t))
            .into_any_element();
    }
    // Not hovered: nothing to draw (out of the flow, so it takes no gap either).
    div().absolute().into_any_element()
}
