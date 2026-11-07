use super::{color, hover_is, ink, look, press_text, press_width, ButtonSize};
use eclipse_theme::glass::{radius, space};
use gpui::{div, prelude::*, px, Div, Role, SharedString, Stateful};

const TAB_UNDERLINE: f32 = 4.;

pub fn value_key(value: &impl std::fmt::Debug) -> String {
    let mut key = String::new();
    let mut last = '-';
    for c in format!("{value:?}").chars() {
        let starts_word = c.is_uppercase() && (last.is_lowercase() || last.is_ascii_digit());
        if (!c.is_alphanumeric() || starts_word) && !key.is_empty() && !key.ends_with('-') {
            key.push('-');
        }
        if c.is_alphanumeric() {
            key.extend(c.to_lowercase());
        }
        last = c;
    }
    key.trim_end_matches('-').to_string()
}

fn padded_tab(
    id: impl Into<SharedString>,
    text: impl Into<SharedString>,
    open: bool,
    pad: f32,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let tone = if open || hover_is(&id) {
        ink::FULL
    } else {
        ink::MUTED
    };
    let text: SharedString = text.into();
    let d = super::track_hover(div().id(id.clone()), id)
        .role(Role::Tab)
        .aria_selected(open)
        .aria_label(text.clone())
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(TAB_COUNT_GAP))
        .h(px(ButtonSize::Md.height()))
        .px(px(pad))
        .rounded(px(radius::CONTROL))
        .cursor_pointer()
        .text_color(ink(tone))
        .when(open, |d| {
            d.child(
                div()
                    .absolute()
                    .left(px(pad))
                    .right(px(pad))
                    .bottom_0()
                    .h(px(TAB_UNDERLINE))
                    .bg(color(if super::light() {
                        look().mark
                    } else {
                        look().accent
                    })),
            )
        });
    d.when(!text.is_empty(), |d| {
        d.child(press_text().child(text.to_uppercase()))
    })
}

pub const TAB_PAD: f32 = space::SM + space::XXS;

const TAB_COUNT_GAP: f32 = space::SM;

pub fn tab_width(label: &str, count: Option<usize>, pad: f32) -> f32 {
    let count = count.map_or(0., |n| TAB_COUNT_GAP + press_width(&n.to_string()));
    press_width(label) + count + 2. * pad
}

pub const TAB_PAD_TIGHT: f32 = space::SM;

pub(super) const TAB_STRIP_GAP: f32 = space::LG;

pub fn tab_strip_padded(
    id: impl Into<SharedString>,
    pad: f32,
    tabs: impl IntoIterator<Item = Stateful<Div>>,
) -> Stateful<Div> {
    div()
        .id(id.into())
        .role(Role::TabList)
        .flex()
        .items_center()
        .mx(px(-pad))
        .mb(px(TAB_STRIP_GAP))
        .children(tabs)
}

/// A tab with a count after its label.
pub fn count_tab(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    count: Option<(usize, SharedString)>,
    open: bool,
    pad: f32,
) -> Stateful<Div> {
    let label: SharedString = label.into();
    let shown = count.as_ref().map(|(n, _)| n.to_string());
    padded_tab(id, label, open, pad)
        .when_some(count.map(|(_, said)| said), |d, said| {
            d.aria_description(said)
        })
        .children(shown.map(|n| press_text().text_color(ink(ink::MUTED)).child(n)))
}
