//! Home's preparation: the time a download has left and the steps under Play while the game
//! files are prepared.
use super::*;
use crate::glass_ui::{self as ui, Tone};
use eclipse_theme::glass::{ink, step};

const STEPS_WIDTH: f32 = 620.;

impl Launcher {
    /// How long the download under way has left, from its pace since it was first seen: none
    /// for its first seconds, or without one.
    pub(super) fn download_left(&mut self) -> Option<Duration> {
        if getting_ready_preview() {
            return Some(Duration::from_secs(70));
        }
        let Some((done, total)) = eclipse_launcher::mojang::progress() else {
            self.download_seen = None;
            return None;
        };
        let (since, from) = match self.download_seen {
            // A new download (Java's after the game's) starts counting again.
            Some(seen) if seen.1 <= done => seen,
            _ => {
                self.download_seen = Some((Instant::now(), done));
                return None;
            }
        };
        time_left(since.elapsed(), from, done, total)
    }

    /// Under Play while it prepares: a bar, what is happening and the time left, and the steps,
    /// the one under way lit, those done ticked. `version`: the Minecraft it downloads.
    pub(super) fn steps(&self, version: &str, left: Option<Duration>) -> Div {
        let now = backend::prepare_step();
        let download = download_progress();
        let doing = match (download, now) {
            (Some((done, total)), backend::PrepareStep::Files) => trf(
                "Downloading Minecraft {version} from Mojang · {done} of {total} files",
                &[
                    ("version", version),
                    ("done", &thousands(done as u64)),
                    ("total", &thousands(total as u64)),
                ],
            ),
            (Some((done, total)), _) => trf(
                "Downloading from Mojang · {done} of {total} files",
                &[
                    ("done", &thousands(done as u64)),
                    ("total", &thousands(total as u64)),
                ],
            ),
            (None, _) => self.prepare_log.back().cloned().unwrap_or_default(),
        };
        let left = left.map(|d| match d.as_secs().div_ceil(60) {
            0 | 1 => tr("about a minute left").to_string(),
            m => trf("about {n} min left", &[("n", &m.to_string())]),
        });
        // Play is the last step when it was pressed, not Prepare.
        let play = self.play_when_prepared.is_some();
        let soft = ui::look().accent_soft;
        let pill = |name: &str, state: std::cmp::Ordering| {
            let (mark, tone) = match state {
                std::cmp::Ordering::Less => (
                    ui::icon("tick-02", ui::IconSize::Px16, ui::live()).into_any_element(),
                    ink::BODY,
                ),
                std::cmp::Ordering::Equal => {
                    (ui::dot(Tone::Accent(soft)).into_any_element(), ink::FULL)
                }
                std::cmp::Ordering::Greater => (ui::dot(Tone::Off).into_any_element(), ink::MUTED),
            };
            div().flex().items_center().gap(px(6.)).child(mark).child(
                ui::text(step::CAPTION, FontWeight::MEDIUM, tone).child(tr(name).to_string()),
            )
        };
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .max_w(px(STEPS_WIDTH))
            .child(
                div()
                    .flex()
                    .child(progress_bar(download.map(|(done, total)| {
                        let f = done as f32 / total as f32;
                        (f, f)
                    }))),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .gap(px(16.))
                    .child(
                        ui::text(step::CAPTION, FontWeight::NORMAL, ink::BODY)
                            .min_w_0()
                            .truncate()
                            .child(doing),
                    )
                    .children(left.map(|l| {
                        ui::text(step::CAPTION, FontWeight::NORMAL, ink::MUTED)
                            .flex_none()
                            .child(l)
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(16.))
                    .children(
                        backend::PrepareStep::ALL
                            .iter()
                            .map(|s| pill(s.name(), s.cmp(&now))),
                    )
                    .children(play.then(|| pill("Play", std::cmp::Ordering::Greater))),
            )
    }
}

/// `ECLIPSE_LAUNCHER_PAGE=getting-ready` opens Home as if Play had been pressed on a version that
/// downloads from Mojang, a third of the way through: for screenshots, nothing runs.
pub(super) fn getting_ready_preview() -> bool {
    static PREVIEW: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *PREVIEW
        .get_or_init(|| std::env::var("ECLIPSE_LAUNCHER_PAGE").as_deref() == Ok("getting-ready"))
}

/// The download under way, files done of how many.
fn download_progress() -> Option<(usize, usize)> {
    eclipse_launcher::mojang::progress().or_else(|| getting_ready_preview().then_some((1754, 5160)))
}

/// The time `total - done` files take at the pace of `done - from` in `spent`: none for the
/// first 5 s, or before a file is done.
fn time_left(spent: Duration, from: usize, done: usize, total: usize) -> Option<Duration> {
    (spent >= Duration::from_secs(5) && done > from)
        .then(|| spent.mul_f64(total.saturating_sub(done) as f64 / (done - from) as f64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_left_follows_the_pace_since_first_seen() {
        let s = Duration::from_secs;
        assert_eq!(time_left(s(10), 100, 600, 3100), Some(s(50)));
        assert_eq!(time_left(s(4), 100, 600, 3100), None, "too early");
        assert_eq!(time_left(s(10), 600, 600, 3100), None, "nothing done yet");
        assert_eq!(time_left(s(10), 0, 50, 50), Some(s(0)));
    }

    #[test]
    fn steps_run_in_their_order() {
        use backend::PrepareStep::*;
        assert_eq!(backend::PrepareStep::ALL, [Files, Java, Sounds, Prepare]);
        for s in backend::PrepareStep::ALL {
            assert_eq!(backend::PrepareStep::ALL[s as usize], s);
        }
    }
}
