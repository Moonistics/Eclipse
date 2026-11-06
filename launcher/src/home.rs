//! Home: the page (its head, the steps under Play while the game files are prepared, its panels
//! and tabs, the crash notice) and the time a download has left.
use super::*;
use crate::ui::{self, Tone};
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
                    .child(ui::progress_bar(download.map(|(done, total)| {
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

impl Launcher {
    pub(super) fn home_page(&mut self, cx: &mut Context<Self>) -> Div {
        // A Java instance plays Mojang's game: no Eclipse version, nothing of Eclipse's to prepare.
        let java = self.selected_instance().and_then(|i| i.java.clone());
        let Some(tag) = self
            .selected()
            .cloned()
            .or_else(|| java.is_some().then(String::new))
        else {
            // No hero without a version: the page starts below the top bar like the others.
            let memory = self.memory_row(cx);
            // The first version, downloading since an instance was made to play it.
            let waiting = match &self.installing {
                Some((_, p)) => format!("{} {:.0}%", tr("Installing"), p * 100.),
                None => tr("No version installed").to_string(),
            };
            let choose = button("go-versions", "Choose a version", ButtonKind::Primary).on_press(
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.go(Page::Versions);
                    cx.notify();
                }),
            );
            return div()
                .flex()
                .flex_col()
                .gap(px(24.))
                .pt(px(28.))
                .child(page_header("Play", ""))
                .child(ui::empty_card(ui::empty_state(
                    "play-no-version",
                    "package-01",
                    waiting,
                    "",
                    vec![choose],
                )))
                .children(memory);
        };
        let prepared = backend::is_prepared(&tag);
        let version = java
            .as_ref()
            .map_or_else(|| backend::game_version(&tag), |j| j.minecraft.clone());
        let left = self.download_left();
        let steps = self.preparing.then(|| self.steps(&version, left));
        let banner = div()
            .flex()
            .flex_col()
            .gap(px(24.))
            .child(self.home_head(&tag, prepared, cx))
            .children(steps);
        let mut page = div().flex().flex_col().gap(px(28.)).child(banner);
        if let Some(e) = self.prepare_error.clone() {
            page = page.child(ui::error_card(e));
        }
        page = page
            .children(self.export_panel(cx))
            .children(self.launch_panel(cx))
            .child(self.home_tab_card(cx));
        if let Some(e) = self.client_error.clone() {
            page = page.child(ui::error_card(e));
        }
        if let Some(crash) = self.crash.clone() {
            page = page.child(self.crash_notice(crash, cx));
        }
        page
    }

    /// "Eclipse crashed": the report and the logs, and where to report it.
    pub(super) fn crash_notice(&self, crash: Crash, cx: &mut Context<Self>) -> Div {
        use eclipse_theme::glass::{ink, pixel_text, step};
        let mut actions = div().flex().flex_wrap().gap(px(8.)).mt(px(12.));
        if let Some(report) = crash.report.clone() {
            let open = report.clone();
            actions = actions
                .child(
                    button("crash-open", "Open report", ButtonKind::Secondary)
                        .on_press(move |_, _, cx| crate::toast::open(cx, &open)),
                )
                .child(
                    button(
                        "crash-copy",
                        if crash.copied {
                            "Copied"
                        } else {
                            "Copy report"
                        },
                        ButtonKind::Secondary,
                    )
                    .on_press(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            if let Ok(text) = std::fs::read_to_string(&report) {
                                clipboard::copy_to_clipboard(
                                    cx,
                                    gpui::ClipboardItem::new_string(text),
                                );
                                if let Some(c) = &mut this.crash {
                                    c.copied = true;
                                }
                            }
                            cx.notify();
                        },
                    )),
                );
        }
        actions = actions.child(
            button("crash-logs", "Open logs folder", ButtonKind::Secondary).on_press(|_, _, cx| {
                let dir = backend::logs_dir();
                let _ = std::fs::create_dir_all(&dir);
                crate::toast::open(cx, &dir);
            }),
        );
        actions = actions.child(
            button("crash-dismiss", "Dismiss", ButtonKind::Ghost).on_press(cx.listener(
                |this, _: &ClickEvent, _, cx| {
                    this.crash = None;
                    cx.notify();
                },
            )),
        );
        let help = crash
            .report
            .is_none()
            .then(|| tr("The game ended without a crash report."));
        let mut body = div().id("crash").flex().flex_col().gap(px(4.)).child(
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .child(ui::icon("alert-circle", ui::IconSize::Px16, ui::danger()))
                .child(ui::entity_title(
                    tr("Eclipse crashed"),
                    pixel_text::CARD_TITLE,
                )),
        );
        if !crash.summary.is_empty() {
            body = body
                .child(ui::text(step::BODY, FontWeight::NORMAL, ink::BODY).child(crash.summary));
        }
        if let Some(report) = &crash.report {
            body = body.child(
                ui::text(step::CAPTION, FontWeight::NORMAL, ink::MUTED)
                    .font_family(theme::MONO)
                    .truncate()
                    .child(short(report)),
            );
        }
        body = body.child(actions).children(help.map(ui::card_note));
        ui::card(eclipse_theme::glass::radius::PANEL)
            .p(px(ui::CARD_PADDING))
            .child(body)
    }
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
