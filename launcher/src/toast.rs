//! What the launcher just did, said in a toast at the window's bottom right: for 6 s (longer
//! while the pointer is on it), or until dismissed when something went wrong.
//!
//! Any handler can say something, even one with only `&mut App`: the line waits here and the
//! launcher's drain (every 80 ms) draws it, then lets it go.

use super::*;
use crate::ui::{self, white};
use eclipse_theme::glass::{ink, space};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// How long a line stays.
const SHOWN: Duration = Duration::from_millis(6000);

/// The line, when it was said, its number (a new one restarts the 6 s), and whether it is an
/// error, which stays until dismissed.
static LINE: Mutex<Option<(String, Instant, u64, bool)>> = Mutex::new(None);

static HELD: AtomicBool = AtomicBool::new(false);

/// Says `text` now, in place of whatever was said before.
pub(super) fn say(text: impl Into<String>) {
    put(text.into(), false);
}

pub(super) fn say_error(text: impl Into<String>) {
    put(text.into(), true);
}

fn put(text: String, error: bool) {
    let mut line = LINE.lock().unwrap_or_else(|e| e.into_inner());
    let n = line.as_ref().map_or(1, |l| l.2 + 1);
    *line = Some((text, Instant::now(), n, error));
}

fn dismiss() {
    *LINE.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// The line still showing, its number, and whether it is an error.
fn current() -> Option<(String, u64, bool)> {
    let mut line = LINE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(l) = line.as_mut() {
        if HELD.load(Ordering::Relaxed) {
            l.1 = Instant::now();
        }
    }
    if line
        .as_ref()
        .is_some_and(|l| !l.3 && l.1.elapsed() >= SHOWN)
    {
        *line = None;
    }
    line.as_ref().map(|l| (l.0.clone(), l.2, l.3))
}

/// `/Users/me/x` as `~/x`.
pub(super) fn tilde(path: &Path) -> String {
    let shown = path.display().to_string();
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && shown.starts_with(&home) => {
            format!("~{}", &shown[home.len()..])
        }
        _ => shown,
    }
}

/// Opens `path` in the system's file manager (or its app), and says so.
pub(super) fn open(cx: &mut App, path: &Path) {
    cx.open_with_system(path);
    say(trf("Opened {path}", &[("path", &tilde(path))]));
}

impl Launcher {
    /// From the drain: whether the line changed (a new one, or the last one gone).
    pub(super) fn toast_tick(&mut self) -> bool {
        let now = current().map(|l| l.1);
        let changed = now != self.toast_seen;
        self.toast_seen = now;
        changed
    }

    pub(super) fn toast_float(&self) -> Option<Div> {
        let (text, n, error) = current()?;
        let back = self.content_focus.clone();
        let close = error.then(|| {
            ui::icon_button(
                "toast-dismiss",
                "cancel-01",
                tr("Dismiss"),
                ButtonKind::Ghost,
                ui::ButtonSize::Sm,
                false,
            )
            .on_press(move |_, window, cx| {
                dismiss();
                window.focus(&back, cx);
                window.refresh();
            })
        });
        let said = ui::text(
            eclipse_theme::glass::step::NOTE,
            FontWeight::NORMAL,
            ink::FULL,
        )
        .relative()
        .flex_1()
        .min_w_0()
        .text_color(white(ink::FULL))
        .child(text.clone());
        let said = if error {
            ui::error_note(text.clone()).relative().flex_1().min_w_0()
        } else {
            said
        };
        Some(ui::corner_float(
            ui::float_card()
                .id("toast")
                .role(if error {
                    gpui::Role::Alert
                } else {
                    gpui::Role::Status
                })
                .aria_label(text)
                .on_hover(|hovered, _, _| HELD.store(*hovered, Ordering::Relaxed))
                .flex()
                .items_center()
                .gap(px(space::SM))
                .pl(px(space::LG))
                .pr(px(if error { space::SM } else { space::LG }))
                .py(px(if error { space::SM } else { space::MD }))
                .child(said)
                .children(close)
                .with_animation(
                    ElementId::named_usize("toast", n as usize),
                    crate::motion::eased(crate::motion::ARRIVE),
                    |d, p| d.opacity(p).top(px(crate::motion::ARRIVE_RISE * (1. - p))),
                ),
        ))
    }
}
