use super::*;
use crate::menus::Menu;
use crate::ui;

pub(super) struct StripTab<T> {
    pub(super) value: T,
    pub(super) key: String,
    pub(super) label: SharedString,
    pub(super) count: Option<usize>,
}

impl Launcher {
    pub(super) fn tab_strip<T: Copy + PartialEq + 'static>(
        &mut self,
        prefix: &str,
        tabs: Vec<StripTab<T>>,
        open: T,
        room: f32,
        pick: impl Fn(&mut Self, T, bool) + Clone + 'static,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let widths = |pad: f32, counts: bool| -> Vec<f32> {
            tabs.iter()
                .map(|t| ui::tab_width(&t.label, t.count.filter(|_| counts), pad))
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
            tabs.iter().position(|t| t.value == open).unwrap_or(0),
            room + 2. * pad,
            ui::ButtonSize::Md.height(),
        );
        let mut strip = Vec::new();
        let mut hidden = Vec::new();
        let mut keys = Vec::new();
        for (tab, shown) in tabs.into_iter().zip(shown) {
            if !shown {
                hidden.push(tab);
                continue;
            }
            let (value, pick) = (tab.value, pick.clone());
            let id = format!("{prefix}-{}", tab.key);
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
            strip.push(self.more_tabs(prefix, hidden, pick.clone(), cx));
        }
        ui::tab_strip_padded(format!("{prefix}s"), pad, strip).on_key_down(cx.listener(
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
        ))
    }

    fn more_tabs<T: Copy + 'static>(
        &mut self,
        prefix: &str,
        hidden: Vec<StripTab<T>>,
        pick: impl Fn(&mut Self, T, bool) + Clone + 'static,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let strip = format!("{prefix}s");
        let menu = Menu::MoreTabs(strip.clone().into());
        let open = self.menu_is(&menu);
        let more = ui::more_button(
            format!("{strip}-more"),
            tr("More tabs"),
            ButtonKind::Ghost,
            ui::ButtonSize::Md,
            open,
        );
        let mut wrap = div()
            .id(SharedString::from(format!("{strip}-overflow")))
            .relative()
            .flex_none()
            .child(self.menu_opener(more, menu, cx));
        if open {
            let mut list = self.menu_popover(format!("{strip}-menu"), cx);
            for (i, tab) in hidden.into_iter().enumerate() {
                let (value, pick) = (tab.value, pick.clone());
                let row = ui::menu_item(
                    format!("{strip}-menu-{}", tab.key),
                    tab.label,
                    tab.count.map(|n| n.to_string()).unwrap_or_default(),
                    false,
                )
                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.close_menu();
                    pick(this, value, false);
                    cx.notify();
                }));
                list = list.child(if i == 0 { ui::starts_menu(row) } else { row });
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

fn shown_tabs(widths: &[f32], open: usize, room: f32, more: f32) -> Vec<bool> {
    if widths.iter().sum::<f32>() <= room {
        return vec![true; widths.len()];
    }
    let mut shown = vec![false; widths.len()];
    shown[open] = true;
    let mut used = more + widths[open];
    for (i, width) in widths.iter().enumerate() {
        if i == open {
            continue;
        }
        if used + width > room {
            break;
        }
        shown[i] = true;
        used += width;
    }
    shown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs_that_do_not_fit_go_under_more_keeping_the_open_one() {
        let widths = [100., 60., 80., 120., 70., 120.];
        assert_eq!(shown_tabs(&widths, 0, 600., 36.), vec![true; 6]);
        assert_eq!(
            shown_tabs(&widths, 0, 500., 36.),
            vec![true, true, true, true, true, false]
        );
        assert_eq!(
            shown_tabs(&widths, 5, 500., 36.),
            vec![true, true, true, false, false, true]
        );
    }
}
