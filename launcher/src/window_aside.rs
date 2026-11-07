//! What the window does while the game runs (`backend::WhilePlaying`): stay, minimise, or hide
//! (macOS), and come back to the front when the game closes, with how the run ended. Settings'
//! Game section chooses (`settings_game.rs`).
use crate::Launcher;
use eclipse_launcher::backend::WhilePlaying;
use gpui::prelude::*;
use gpui::AnyWindowHandle;

pub(super) struct Aside {
    window: AnyWindowHandle,
    /// What the window did when the game started, to undo when it closes.
    stepped: Option<WhilePlaying>,
}

impl Aside {
    pub(super) fn new(window: AnyWindowHandle) -> Self {
        Aside {
            window,
            stepped: None,
        }
    }
}

impl Launcher {
    /// The game started: get out of its way as the player asked. Settings saved elsewhere since
    /// (`eclipse-launcher settings while-playing …`) count at once.
    pub(super) fn step_aside(&mut self, cx: &mut Context<Self>) {
        self.pull_settings();
        let how = self.settings.while_playing.here();
        if how == WhilePlaying::Stay {
            return;
        }
        let window = self.aside.window;
        self.aside.stepped = Some(how);
        cx.defer(move |cx| match how {
            WhilePlaying::Hide => cx.hide(),
            _ => {
                let _ = window.update(cx, |_, window, _| window.minimize_window());
            }
        });
    }

    /// The game closed: back to the front, where its play time, errors and log are.
    pub(super) fn come_back(&mut self, cx: &mut Context<Self>) {
        if self.aside.stepped.take().is_none() {
            return;
        }
        let window = self.aside.window;
        cx.defer(move |cx| {
            cx.activate(true);
            let _ = window.update(cx, |_, window, _| window.activate_window());
        });
    }
}
