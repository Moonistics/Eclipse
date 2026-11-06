use super::*;

impl Launcher {
    /// Whether `event` is the press that just closed a menu from outside it (on the menu's
    /// own button): the button then leaves the menu closed instead of opening it again.
    pub(super) fn closed_by_this_press(&mut self, event: &ClickEvent) -> bool {
        closed_by_press(self.menu_closed_at.take(), event)
    }

    pub(super) fn menu_open(&self) -> bool {
        self.account_menu
            || self.row_menu.open.is_some()
            || self.home_menu != home_head::HomeMenu::None
            || self.picker_open
            || self.language_menu
            || self.list_menu.is_some()
            || self.mods.target_menu
            || self.mods.browse.sort_menu
            || self.mod_detail_floats()
    }

    pub(super) fn mod_detail_floats(&self) -> bool {
        self.page == Page::Mods
            && self.mods.picked.is_some()
            && self.window_width < layout::MODS_DETAIL_BESIDE
    }

    pub(super) fn close_menus(&mut self) -> bool {
        let open = self.menu_open();
        self.account_menu = false;
        self.row_menu.open = None;
        self.home_menu = home_head::HomeMenu::None;
        self.picker_open = false;
        self.language_menu = false;
        self.list_menu = None;
        self.mods.target_menu = false;
        self.mods.browse.sort_menu = false;
        if self.mod_detail_floats() {
            self.mods.picked = None;
        }
        open
    }

    pub(super) fn confirm_escape(&mut self) -> bool {
        let asking = self.confirm_remove.is_some()
            || self.multiplayer.confirm_remove.is_some()
            || self.game_packs.confirm_remove.is_some()
            || self.mods.confirm_remove.is_some()
            || self.github.confirm_remove.is_some()
            || self.world_packs.confirm_remove.is_some()
            || self.screenshots.confirm_delete
            || self.collection.confirm_delete
            || self.servers.confirm_restore.is_some()
            || self.servers.confirm_delete;
        self.confirm_remove = None;
        self.multiplayer.confirm_remove = None;
        self.game_packs.confirm_remove = None;
        self.mods.confirm_remove = None;
        self.github.confirm_remove = None;
        self.world_packs.confirm_remove = None;
        self.screenshots.confirm_delete = false;
        self.collection.confirm_delete = false;
        self.servers.confirm_restore = None;
        self.servers.confirm_delete = false;
        asking
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
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
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

/// The row menu a press on a sidebar row leaves open: that row's, or none when it was already
/// that row's (the ⋯ and a right click on the row toggle the same way).
pub(super) fn row_menu_after_press(
    open: Option<(String, bool)>,
    id: &str,
) -> Option<(String, bool)> {
    if open.as_ref().is_some_and(|(current, _)| current == id) {
        return None;
    }
    Some((id.to_owned(), false))
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
    fn a_press_on_a_sidebar_row_opens_its_menu_or_closes_it() {
        let open = |id: &str| Some((id.to_string(), false));
        assert_eq!(
            super::row_menu_after_press(None, "Eclipse"),
            open("Eclipse")
        );
        assert_eq!(
            super::row_menu_after_press(open("Other"), "Eclipse"),
            open("Eclipse")
        );
        assert_eq!(
            super::row_menu_after_press(open("Eclipse"), "Eclipse"),
            None
        );
        // A Delete… already pressed on another row does not carry over.
        assert_eq!(
            super::row_menu_after_press(Some(("Other".into(), true)), "Eclipse"),
            open("Eclipse")
        );
    }
}
