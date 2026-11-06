//! What the launcher just did, said for 4.2 s in a toast at the window's bottom right.
//!
//! Any handler can say something, even one with only `&mut App`: the line waits here and the
//! launcher's drain (every 80 ms) draws it, then lets it go.

use super::*;
use crate::ui::{self, white};
use eclipse_theme::glass::{ink, space};
use std::path::Path;
use std::sync::Mutex;

/// How long a line stays.
const SHOWN: Duration = Duration::from_millis(4200);

/// The line, when it was said, and its number (a new one restarts the 4.2 s).
static LINE: Mutex<Option<(String, Instant, u64)>> = Mutex::new(None);

/// Says `text` now, in place of whatever was said before.
pub(super) fn say(text: impl Into<String>) {
    let mut line = LINE.lock().unwrap_or_else(|e| e.into_inner());
    let n = line.as_ref().map_or(1, |l| l.2 + 1);
    *line = Some((text.into(), Instant::now(), n));
}

/// The line still showing, and its number.
fn current() -> Option<(String, u64)> {
    let mut line = LINE.lock().unwrap_or_else(|e| e.into_inner());
    if line.as_ref().is_some_and(|l| l.1.elapsed() >= SHOWN) {
        *line = None;
    }
    line.as_ref().map(|l| (l.0.clone(), l.2))
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
        let (text, n) = current()?;
        Some(
            div()
                .absolute()
                .right(px(20.))
                .bottom(px(20.))
                .max_w(px(420.))
                .child(
                    ui::float_card(eclipse_theme::glass::radius::POPOVER)
                        .shadow_lg()
                        .px(px(space::LG))
                        .py(px(space::MD))
                        .child(
                            ui::text(
                                eclipse_theme::glass::step::NOTE,
                                FontWeight::NORMAL,
                                ink::FULL,
                            )
                            .relative()
                            .text_color(white(ink::FULL))
                            .child(text),
                        )
                        .with_animation(
                            ElementId::named_usize("toast", n as usize),
                            crate::motion::eased(crate::motion::POP),
                            |d, p| d.opacity(p).top(px(crate::motion::POP_RISE * (1. - p))),
                        ),
                ),
        )
    }
}
