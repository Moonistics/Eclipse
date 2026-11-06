//! Home: the page (its head, the steps under Play while the game files are prepared, its panels
//! and tabs, the crash notice) and the time a download has left.
use super::*;
use crate::ui::{self, Tone};
use eclipse_theme::glass::{ink, space, step};

const STEPS_WIDTH: f32 = 620.;
const INSTALL_BAR_WIDTH: f32 = 320.;

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
            div()
                .flex()
                .items_center()
                .gap(px(space::SM))
                .child(mark)
                .child(
                    ui::text(step::CAPTION, FontWeight::MEDIUM, tone).child(tr(name).to_string()),
                )
        };
        div()
            .flex()
            .flex_col()
            .gap(px(space::MD))
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
                    .gap(px(space::LG))
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
                    .gap(px(space::LG))
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
            let state = self.first_version_state(cx);
            return div()
                .flex()
                .flex_col()
                .child(
                    self.home_title(cx)
                        .pt(px(ui::page_top()))
                        .mb(px(ui::page_gap())),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(layout::CARD_GAP))
                        .child(ui::empty_card(state)),
                );
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
            .gap(px(space::XL))
            .child(self.home_head(&tag, prepared, cx))
            .children(steps);
        let mut page = div().flex().flex_col().gap(px(space::XL)).child(banner);
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

    fn first_version_state(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        if self.installing.is_some() {
            return ui::loading_state("play-installing", tr("Installing Eclipse")).child(
                div()
                    .flex()
                    .w_full()
                    .max_w(px(INSTALL_BAR_WIDTH))
                    .mt(px(space::LG))
                    .child(ui::progress_bar(Some(self.bar))),
            );
        }
        let update = self.updates.eclipse.as_ref().map(|r| r.tag.clone());
        let install_failed = self
            .install_error
            .as_ref()
            .filter(|(tag, _)| update.as_ref() == Some(tag))
            .map(|(_, e)| e.clone());
        if let Some(e) = install_failed {
            return ui::error_state(
                "play-download-failed",
                tr("Couldn't download Eclipse"),
                &e,
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.install_update();
                    cx.notify();
                }),
            );
        }
        if let Some(e) = self.update_error.clone() {
            return ui::error_state_with(
                "play-download-failed",
                tr("Couldn't download Eclipse"),
                &e,
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.update_error = None;
                    this.releases = Load::Loading;
                    this.fetch_startup_releases();
                    cx.notify();
                }),
                Some(Self::proxy_settings_button("play-download-failed", cx)),
                ButtonKind::Primary,
            );
        }
        let choose = |kind| {
            ui::button(
                "go-versions",
                None,
                tr("Choose a version"),
                kind,
                ui::ButtonSize::Md,
            )
            .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.go(Page::Versions);
                cx.notify();
            }))
        };
        if matches!(self.releases, Load::Loading) && update.is_none() {
            return ui::loading_state("play-checking", tr("Looking for the newest Eclipse…"));
        }
        if let Some(own) = self.selected_instance().and_then(|i| i.version.clone()) {
            let name = eclipse_launcher::nightly::label(&own)
                .or_else(|| build_import::label(&own))
                .unwrap_or(own);
            return ui::empty_state(
                "play-not-installed",
                "package-01",
                trf("{version} isn't installed", &[("version", &name)]),
                tr("Choose a version to play."),
                vec![choose(ButtonKind::Primary)],
            );
        }
        let no_build =
            matches!(&self.releases, Load::Ready(list) if !list.iter().any(|r| r.asset.is_some()));
        if no_build {
            return ui::empty_state(
                "play-not-installed",
                "package-01",
                tr("Eclipse isn't installed yet"),
                tr("There's no Eclipse build for this computer yet."),
                vec![choose(ButtonKind::Secondary)],
            );
        }
        let install = ui::button(
            "play-install",
            Some("download-04"),
            tr("Install Eclipse"),
            ButtonKind::Primary,
            ui::ButtonSize::Md,
        );
        let install = match update {
            Some(_) => install.track_focus(&self.play_focus).on_press(cx.listener(
                |this, _: &ClickEvent, _, cx| {
                    this.install_update();
                    cx.notify();
                },
            )),
            None => ui::disabled(install),
        };
        ui::empty_state(
            "play-not-installed",
            "package-01",
            tr("Eclipse isn't installed yet"),
            tr("Install the newest version to start playing."),
            vec![install, choose(ButtonKind::Secondary)],
        )
    }

    /// "Eclipse crashed": the report and the logs, and where to report it.
    pub(super) fn crash_notice(&self, crash: Crash, cx: &mut Context<Self>) -> Div {
        use eclipse_theme::glass::{ink, pixel_text, step};
        let mut actions = div()
            .flex()
            .flex_wrap()
            .gap(px(space::SM))
            .mt(px(space::MD));
        if let Some(report) = crash.report.clone() {
            let open = report.clone();
            actions = actions
                .child(
                    ui::button(
                        "crash-open",
                        None,
                        tr("Open report"),
                        ButtonKind::Secondary,
                        ui::ButtonSize::Md,
                    )
                    .on_press(move |_, _, cx| crate::toast::open(cx, &open)),
                )
                .child(
                    ui::button(
                        "crash-copy",
                        None,
                        tr(if crash.copied {
                            "Copied"
                        } else {
                            "Copy report"
                        }),
                        ButtonKind::Secondary,
                        ui::ButtonSize::Md,
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
            ui::button(
                "crash-logs",
                None,
                tr("Open logs folder"),
                ButtonKind::Secondary,
                ui::ButtonSize::Md,
            )
            .on_press(|_, _, cx| {
                let dir = backend::logs_dir();
                let _ = std::fs::create_dir_all(&dir);
                crate::toast::open(cx, &dir);
            }),
        );
        actions = actions.child(
            ui::button(
                "crash-dismiss",
                None,
                tr("Dismiss"),
                ButtonKind::Ghost,
                ui::ButtonSize::Md,
            )
            .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.crash = None;
                cx.notify();
            })),
        );
        let help = crash
            .report
            .is_none()
            .then(|| tr("The game ended without a crash report."));
        let mut body = div()
            .id("crash")
            .flex()
            .flex_col()
            .gap(px(space::XS))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::MD))
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
