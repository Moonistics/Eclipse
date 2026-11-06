use super::*;
use eclipse_launcher::packs::Kind as PackKind;

#[derive(Clone, PartialEq, Debug)]
pub(super) enum Menu {
    Account,
    InstanceRow(String),
    Play,
    Version,
    Language,
    ListRow(SharedString),
    ModsTarget,
    ModsSort,
}

#[derive(Clone, PartialEq, Debug)]
pub(super) enum Asking {
    Version(String),
    OfflineName(String),
    RegistryPack(String),
    PortedPack(String),
    Mod(String),
    GamePack(PackKind, String),
    SavedServer(String),
    WorldPack(String),
    GithubMod(String),
    Screenshot,
    Collection,
    ServerRestore(String),
    ServerDelete,
    InstanceDelete(String),
    WorldDelete(SharedString),
}

const KEY_SCROLL_STEP: f32 = 40.;

const KEY_SCROLL_OVERLAP: f32 = 48.;

impl Asking {
    fn in_a_menu(&self) -> bool {
        matches!(self, Asking::InstanceDelete(_) | Asking::WorldDelete(_))
    }
}

impl Launcher {
    /// Whether `event` is the press that just closed a menu from outside it (on the menu's
    /// own button): the button then leaves the menu closed instead of opening it again.
    pub(super) fn closed_by_this_press(&mut self, event: &ClickEvent) -> bool {
        closed_by_press(self.menu_closed_at.take(), event)
    }

    pub(super) fn menu_open(&self) -> bool {
        self.open_menu.is_some() || self.mod_detail_floats()
    }

    pub(super) fn menu_is(&self, menu: &Menu) -> bool {
        self.open_menu.as_ref() == Some(menu)
    }

    /// Opens `menu`, or closes it when it is the one open, as its button's press does.
    pub(super) fn toggle_menu(&mut self, menu: Menu, window: &Window, cx: &App) {
        self.open_menu = menu_after_press(self.open_menu.take(), menu);
        self.stop_asking(Asking::in_a_menu);
        if self.open_menu.is_some() {
            self.opened_menu(window, cx);
        }
    }

    pub(super) fn mod_detail_floats(&self) -> bool {
        self.page == Page::Mods
            && self.mods.picked.is_some()
            && self.window_width < layout::MODS_DETAIL_BESIDE
    }

    pub(super) fn close_menu(&mut self) {
        self.open_menu = None;
        self.stop_asking(Asking::in_a_menu);
    }

    pub(super) fn close_menus(&mut self) -> bool {
        let open = self.menu_open();
        self.close_menu();
        if self.mod_detail_floats() {
            self.mods.picked = None;
        }
        open
    }

    pub(super) fn is_asking(&self, about: &Asking) -> bool {
        self.asking.as_ref() == Some(about)
    }

    pub(super) fn stop_asking(&mut self, about: impl Fn(&Asking) -> bool) {
        if self.asking.as_ref().is_some_and(about) {
            self.asking = None;
        }
    }

    pub(super) fn confirm_escape(&mut self) -> bool {
        self.asking.take().is_some()
    }

    pub(super) fn opened_menu(&mut self, window: &Window, cx: &App) {
        self.menu_opener = window.focused(cx);
        ui::menu_opened();
    }

    pub(super) fn return_focus_from_menu(&mut self, window: &mut Window, cx: &mut App) {
        if self.menu_open() {
            return;
        }
        let Some(opener) = self.menu_opener.take() else {
            return;
        };
        let lost = window.focused(cx).is_none()
            || ui::menu_focus().is_some_and(|m| m.contains_focused(window, cx));
        if lost {
            window.focus(&opener, cx);
        }
    }

    pub(super) fn root_key_down(
        &mut self,
        ev: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let k = &ev.keystroke;
        match k.key.as_str() {
            "tab" => {
                if k.modifiers.shift {
                    window.focus_prev(cx);
                } else {
                    window.focus_next(cx);
                }
            }
            "f6" => {
                let to_sidebar = k.modifiers.shift;
                let region = if to_sidebar {
                    &self.side_focus
                } else {
                    &self.content_focus
                };
                window.focus(region, cx);
                window.focus_next(cx);
            }
            "escape" if self.close_menus() => {
                if let Some(opener) = self.menu_opener.take() {
                    window.focus(&opener, cx);
                }
            }
            "escape" if self.confirm_escape() => {}
            "escape" if self.native_log_escape() => {}
            "escape" if self.servers_escape() => {}
            "escape" if self.instance_escape() => {}
            "escape" if self.screenshot_escape() => {}
            "left" if self.screenshot_step(false) => {}
            "right" if self.screenshot_step(true) => {}
            key if self.scroll_by_key(key, window, cx) => {}
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn scroll_by_key(&mut self, key: &str, window: &Window, cx: &App) -> bool {
        if self.fields.any_focus() || self.open_menu.is_some() {
            return false;
        }
        let on_page = window.focused(cx).is_none_or(|f| {
            f == self.root_focus || f == self.content_focus || f == self.side_focus
        });
        let pane = match self.page {
            Page::Settings => &self.settings_pane,
            Page::InstanceEditor => match &self.instances.draft {
                Some(d) => &d.scroll,
                None => &self.content_scroll.handle,
            },
            _ => &self.content_scroll.handle,
        };
        let view = f32::from(pane.bounds().size.height);
        let max = f32::from(pane.max_offset().y);
        let at = -f32::from(pane.offset().y);
        let to = match key {
            "pagedown" => at + view - KEY_SCROLL_OVERLAP,
            "space" if on_page => at + view - KEY_SCROLL_OVERLAP,
            "pageup" => at - view + KEY_SCROLL_OVERLAP,
            "down" => at + KEY_SCROLL_STEP,
            "up" => at - KEY_SCROLL_STEP,
            "home" => 0.,
            "end" => max,
            _ => return false,
        };
        let mut offset = pane.offset();
        offset.y = px(-to.clamp(0., max));
        pane.set_offset(offset);
        true
    }

    pub(super) fn palette_key_down(
        &mut self,
        ev: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let k = &ev.keystroke;
        let command = if cfg!(target_os = "macos") {
            k.modifiers.platform
        } else {
            k.modifiers.control
        };
        if command && !k.modifiers.shift && k.key == "k" {
            self.palette_toggle(window, cx);
            cx.stop_propagation();
            cx.notify();
        }
    }
}

/// Whether the click `event` began with the mouse-down that closed a menu at `closed_at`.
/// A keyboard click never did.
pub(super) fn closed_by_press(closed_at: Option<Point<Pixels>>, event: &ClickEvent) -> bool {
    matches!(event, ClickEvent::Mouse(click) if closed_at == Some(click.down.position))
}

/// The menu a press on a menu's button leaves open: that menu, or none when it was already the
/// one open (the sidebar's ⋯ and a right click on its row toggle the same way).
pub(super) fn menu_after_press(open: Option<Menu>, pressed: Menu) -> Option<Menu> {
    if open.as_ref() == Some(&pressed) {
        return None;
    }
    Some(pressed)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_menu_button_stays_closed_after_the_press_that_closed_its_menu() {
        use super::closed_by_press;
        use gpui::{point, px, ClickEvent, KeyboardClickEvent, MouseClickEvent, MouseDownEvent};
        let click = |x: f32| {
            ClickEvent::Mouse(MouseClickEvent {
                down: MouseDownEvent {
                    position: point(px(x), px(20.)),
                    ..MouseDownEvent::default()
                },
                ..MouseClickEvent::default()
            })
        };
        let closed_at = Some(point(px(10.), px(20.)));
        // The press on the button that closed the open menu: it stays closed.
        assert!(closed_by_press(closed_at, &click(10.)));
        // A later press, or one with no menu closed, toggles as before.
        assert!(!closed_by_press(closed_at, &click(11.)));
        assert!(!closed_by_press(None, &click(10.)));
        let keyboard = ClickEvent::Keyboard(KeyboardClickEvent::default());
        assert!(!closed_by_press(closed_at, &keyboard));
    }

    #[test]
    fn a_press_on_a_menu_button_opens_its_menu_or_closes_it() {
        use super::{menu_after_press, Menu};
        let row = |id: &str| Menu::InstanceRow(id.to_string());
        assert_eq!(menu_after_press(None, row("Eclipse")), Some(row("Eclipse")));
        assert_eq!(
            menu_after_press(Some(row("Other")), row("Eclipse")),
            Some(row("Eclipse"))
        );
        assert_eq!(menu_after_press(Some(row("Eclipse")), row("Eclipse")), None);
        assert_eq!(
            menu_after_press(Some(Menu::Play), Menu::Version),
            Some(Menu::Version)
        );
    }
}
