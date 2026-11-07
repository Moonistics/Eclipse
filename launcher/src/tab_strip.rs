use crate::menus::Menu;
use crate::ui::{ButtonKind, Press};
use crate::{ui, Launcher};
use eclipse_launcher::i18n::{tr, trf};
use gpui::prelude::*;
use gpui::{div, ClickEvent, Context, Div, FocusHandle, KeyDownEvent, SharedString, Stateful};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Strip {
    Home,
    Mods,
    ModsProject,
    Server,
    Settings,
}

impl Strip {
    fn prefix(self) -> &'static str {
        match self {
            Strip::Home => "home-tab",
            Strip::Mods => "mods-tab",
            Strip::ModsProject => "mods-project-tab",
            Strip::Server => "server-tab",
            Strip::Settings => "settings-section",
        }
    }
}

impl From<Strip> for SharedString {
    fn from(strip: Strip) -> Self {
        format!("{}s", strip.prefix()).into()
    }
}

pub(super) struct StripTab<T> {
    pub(super) value: T,
    pub(super) label: SharedString,
    pub(super) count: Option<(usize, SharedString)>,
}

impl Launcher {
    pub(super) fn tab_strip<T: Copy + PartialEq + std::fmt::Debug + 'static>(
        &mut self,
        strip_of: Strip,
        tabs: Vec<StripTab<T>>,
        open: T,
        room: f32,
        pick: impl Fn(&mut Self, T, bool) + Clone + 'static,
        cx: &mut Context<Self>,
    ) -> (Stateful<Div>, Stateful<Div>) {
        let prefix = strip_of.prefix();
        let open_label = tabs
            .iter()
            .find(|t| t.value == open)
            .map(|t| t.label.clone())
            .unwrap_or_default();
        let panel = ui::tab_panel(format!("{prefix}-panel"), open_label);
        let widths = |pad: f32, counts: bool| -> Vec<f32> {
            tabs.iter()
                .map(|t| {
                    let count = t.count.as_ref().map(|(n, _)| *n).filter(|_| counts);
                    ui::tab_width(&t.label, count, pad)
                })
                .collect()
        };
        let fits =
            |pad: f32, counts: bool| widths(pad, counts).iter().sum::<f32>() <= room + 2. * pad;
        let counts = fits(ui::TAB_PAD, true);
        let pad = if counts || fits(ui::TAB_PAD, false) {
            ui::TAB_PAD
        } else {
            ui::TAB_PAD_TIGHT
        };
        let shown = shown_tabs(
            &widths(pad, counts),
            room + 2. * pad,
            ui::ButtonSize::Md.height(),
        );
        let more_focus = ui::try_focus_for(format!("{prefix}s-more"));
        let mut strip = Vec::new();
        let mut hidden = Vec::new();
        let mut keys = Vec::new();
        for (tab, shown) in tabs.into_iter().zip(shown) {
            if !shown {
                keys.push((tab.value, more_focus.clone()));
                hidden.push(tab);
                continue;
            }
            let (value, pick) = (tab.value, pick.clone());
            let id = format!("{prefix}-{}", ui::value_key(&value));
            let focus = ui::try_focus_for(id.clone());
            keys.push((value, focus.clone()));
            strip.push(
                ui::count_tab(
                    id,
                    tab.label,
                    tab.count.filter(|_| counts),
                    value == open,
                    pad,
                )
                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    pick(this, value, false);
                    cx.notify();
                }))
                .when_some(focus, |tab, focus| {
                    tab.track_focus(&focus.tab_stop(value == open))
                }),
            );
        }
        if !hidden.is_empty() {
            strip.push(self.more_tabs(strip_of, hidden, open, more_focus, pick.clone(), cx));
        }
        let strip = ui::tab_strip_padded(strip_of, pad, strip).on_key_down(cx.listener(
            move |this, ev: &KeyDownEvent, window, cx| {
                let at = keys.iter().position(|(v, _)| *v == open).unwrap_or(0);
                let last = keys.len().saturating_sub(1);
                let to = match ev.keystroke.key.as_str() {
                    "left" => at.checked_sub(1).unwrap_or(last),
                    "right" if at == last => 0,
                    "right" => at + 1,
                    "home" => 0,
                    "end" => last,
                    _ => return,
                };
                let Some((value, focus)) = keys.get(to).cloned() else {
                    return;
                };
                pick(this, value, true);
                if let Some(focus) = focus {
                    window.focus(&focus, cx);
                }
                cx.stop_propagation();
                cx.notify();
            },
        ));
        (strip, panel)
    }

    fn more_tabs<T: Copy + PartialEq + std::fmt::Debug + 'static>(
        &mut self,
        strip_of: Strip,
        hidden: Vec<StripTab<T>>,
        current: T,
        focus: Option<FocusHandle>,
        pick: impl Fn(&mut Self, T, bool) + Clone + 'static,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let strip = SharedString::from(strip_of);
        let menu = Menu::MoreTabs(strip_of);
        let open = self.menu_is(&menu);
        let inside = hidden.iter().find(|t| t.value == current);
        let name = match inside {
            Some(tab) => trf("More tabs, {tab} open", &[("tab", &tab.label)]),
            None => tr("More tabs").to_string(),
        };
        let more = ui::more_button(
            format!("{strip}-more"),
            name,
            ButtonKind::Ghost,
            ui::ButtonSize::Md,
            open || inside.is_some(),
        )
        .aria_expanded(open);
        let more = match focus {
            Some(focus) => ui::keep_focus(more, &focus),
            None => more,
        };
        let mut wrap = div()
            .id(SharedString::from(format!("{strip}-overflow")))
            .relative()
            .flex_none()
            .child(self.menu_opener(more, menu, cx));
        if open {
            let mut list = self.menu_popover(format!("{strip}-menu"), tr("More tabs"), cx);
            let starts_on_open = inside.is_some();
            for (i, tab) in hidden.into_iter().enumerate() {
                let (value, pick) = (tab.value, pick.clone());
                let row = ui::pick_item(
                    format!("{strip}-menu-{}", ui::value_key(&tab.value)),
                    tab.label,
                    tab.count.map(|(_, said)| said).unwrap_or_default(),
                    value == current,
                );
                let row = if i == 0 && !starts_on_open {
                    ui::starts_menu(row)
                } else {
                    row
                };
                list = list.child(
                    row.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.close_menu();
                        pick(this, value, false);
                        cx.notify();
                    })),
                );
            }
            wrap = wrap.child(ui::menu_under(
                ui::ButtonSize::Md.height(),
                ui::Align::End,
                list,
            ));
        }
        wrap
    }
}

fn shown_tabs(widths: &[f32], room: f32, more: f32) -> Vec<bool> {
    if widths.iter().sum::<f32>() <= room {
        return vec![true; widths.len()];
    }
    let mut used = more;
    widths
        .iter()
        .map(|width| {
            used += width;
            used <= room
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs_that_do_not_fit_go_under_more_in_their_order() {
        let widths = [100., 60., 80., 120., 70., 120.];
        assert_eq!(shown_tabs(&widths, 600., 36.), vec![true; 6]);
        assert_eq!(
            shown_tabs(&widths, 500., 36.),
            vec![true, true, true, true, true, false]
        );
        assert_eq!(
            shown_tabs(&widths, 400., 36.),
            vec![true, true, true, true, false, false]
        );
    }
}
