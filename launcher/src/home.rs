//! Home: the page (its head, the steps under Play while the game files are prepared, the crash
//! notice, its panels and tabs) and the time a download has left.
use super::*;
use crate::game_run::GameProblem;
use crate::ui::{self, Tone};
use eclipse_theme::glass::{ink, space, step};

const STEPS_WIDTH: f32 = 620.;

impl Launcher {
    /// How long the download under way has left, from its pace since it was first seen: none
    /// for its first seconds, or without one.
    pub(super) fn download_left(&mut self) -> Option<Duration> {
        if preview::shows(preview::Preview::GettingReady) {
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
            (None, _) => self.game.log.back().cloned().unwrap_or_default(),
        };
        let left = left.map(|d| match d.as_secs().div_ceil(60) {
            0 | 1 => tr("about a minute left").to_string(),
            m => trf("about {n} min left", &[("n", &m.to_string())]),
        });
        // Play is the last step when it was pressed, not Prepare.
        let play = self.game.plays_when_prepared();
        let soft = ui::look().accent_soft;
        let step_line = |name: &str, state: std::cmp::Ordering| {
            let (tone, ink) = match state {
                std::cmp::Ordering::Less => (Tone::Off, ink::BODY),
                std::cmp::Ordering::Equal => (Tone::Accent(soft), ink::FULL),
                std::cmp::Ordering::Greater => (Tone::Off, ink::MUTED),
            };
            ui::text(step::CAPTION, FontWeight::MEDIUM, ink)
                .child(ui::state_line(tone, name.to_string()))
        };
        let fraction = download.map(|(done, total)| {
            let f = done as f32 / total as f32;
            (f, f)
        });
        let note = match left {
            Some(left) => format!("{doing} · {left}"),
            None => doing,
        };
        div()
            .flex()
            .flex_col()
            .gap(px(space::MD))
            .max_w(px(STEPS_WIDTH))
            .child(ui::progress_line("", fraction, note))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(space::LG))
                    .children(
                        backend::PrepareStep::ALL
                            .iter()
                            .map(|s| step_line(tr(s.name()), s.cmp(&now))),
                    )
                    .children(play.then(|| step_line(tr("Play"), std::cmp::Ordering::Greater))),
            )
    }
}

/// The download under way, files done of how many.
fn download_progress() -> Option<(usize, usize)> {
    eclipse_launcher::mojang::progress()
        .or_else(|| preview::shows(preview::Preview::GettingReady).then_some((1754, 5160)))
}

/// The time `total - done` files take at the pace of `done - from` in `spent`: none for the
/// first 5 s, or before a file is done.
fn time_left(spent: Duration, from: usize, done: usize, total: usize) -> Option<Duration> {
    (spent >= Duration::from_secs(5) && done > from)
        .then(|| spent.mul_f64(total.saturating_sub(done) as f64 / (done - from) as f64))
}

pub(super) fn minecraft_missing(error: &str, version: &str) -> bool {
    error.starts_with(&eclipse_launcher::sources::not_found_line(version))
}

impl Launcher {
    pub(super) fn home_page(&mut self, cx: &mut Context<Self>) -> Div {
        // A Java instance plays Mojang's game: no Eclipse version, nothing of Eclipse's to prepare.
        let java = self.selected_instance().and_then(|i| i.java.clone());
        let Some(tag) = java
            .is_some()
            .then(String::new)
            .or_else(|| self.selected_version().cloned())
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
        let preparing = matches!(self.selected_play_state(), PlayState::Preparing(_));
        let steps = preparing.then(|| self.steps(&version, left));
        let banner = div()
            .flex()
            .flex_col()
            .gap(px(space::XL))
            .child(self.home_head(&tag, prepared, cx))
            .children(steps);
        let mut page = div().flex().flex_col().gap(px(space::XL)).child(banner);
        if let Some(e) = self.shown_prepare_error().cloned() {
            page = page.child(ui::empty_card(self.prepare_failed(&e, &version, cx)));
        }
        if let Some(problem) = self.shown_game_problem().cloned() {
            page = page.child(self.game_problem(problem, cx));
        }
        if let Some(crash) = self.shown_crash().cloned().or_else(preview::crash) {
            page = page.child(self.crash_notice(crash, cx));
        }
        page.children(self.export_panel(cx))
            .child(self.home_tab_card(cx))
    }

    fn prepare_failed(&self, error: &str, version: &str, cx: &mut Context<Self>) -> Stateful<Div> {
        let again = cx.listener(|this, _: &ClickEvent, _, cx| {
            match this.cant_sign_in_to_play() {
                true => this.prepare(backend::MinecraftFrom::Install, false),
                false => this.press_play(),
            }
            cx.notify();
        });
        if !minecraft_missing(error, version) {
            let tag = self.selected_version().cloned().unwrap_or_default();
            let eclipse = eclipse_launcher::nightly::display_name(&tag);
            let known = self.versions.release(&tag).is_some();
            let versions = ui::button(
                "prepare-failed-versions",
                None,
                tr("Manage versions"),
                ButtonKind::Secondary,
                ui::ButtonSize::Md,
            )
            .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.go(Page::Versions);
                cx.notify();
            }));
            let reinstall = known.then(|| {
                let tag = tag.clone();
                ui::button(
                    "prepare-failed-reinstall",
                    Some("download-04"),
                    trf("Reinstall {eclipse}", &[("eclipse", &eclipse)]),
                    ButtonKind::Secondary,
                    ui::ButtonSize::Md,
                )
                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.reinstall(&tag);
                    cx.notify();
                }))
            });
            let home = self
                .selected_instance()
                .map_or_else(backend::game_home, |i| i.home.clone());
            let logs = ui::open_folder_button(
                "prepare-failed-logs",
                tr("Open logs folder"),
                backend::logs_dir_in(&home),
                ui::ButtonSize::Md,
            );
            let cause = ui::told_cause(error).unwrap_or_else(|| {
                trf(
                    "Eclipse couldn't get Minecraft {version} ready.",
                    &[("version", version)],
                )
            });
            return ui::error_state_told(
                "prepare-failed",
                tr("Couldn't set up the game"),
                cause,
                error,
                again,
                reinstall.into_iter().chain([versions, logs]).collect(),
                ButtonKind::Secondary,
            );
        }
        let folder = ui::button(
            "prepare-failed-folder",
            Some("folder-01"),
            tr("Choose Minecraft folder…"),
            ButtonKind::Secondary,
            ui::ButtonSize::Md,
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.choose_folder(cx);
            cx.notify();
        }));
        let sign_in = self.can_sign_in().then(|| {
            ui::button(
                "prepare-failed-sign-in",
                None,
                tr("Sign in with Microsoft"),
                ButtonKind::Secondary,
                ui::ButtonSize::Md,
            )
            .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.open_sign_in();
                cx.notify();
            }))
        });
        ui::error_state_told(
            "prepare-failed",
            trf("Minecraft {version} not found", &[("version", version)]),
            trf(
                "Play {version} once in your Minecraft launcher, then try again.",
                &[("version", version)],
            ),
            error,
            again,
            std::iter::once(folder).chain(sign_in).collect(),
            ButtonKind::Secondary,
        )
    }

    fn game_problem(&self, problem: GameProblem, cx: &mut Context<Self>) -> AnyElement {
        let again = cx.listener(|this, _: &ClickEvent, _, cx| {
            this.press_play();
            cx.notify();
        });
        let notice = match problem {
            GameProblem::NeedsOwner => {
                return ui::empty_card(self.sign_in_to_play_state(cx)).into_any_element()
            }
            GameProblem::Failed(error) => {
                return ui::empty_card(ui::error_state_with(
                    "game-failed",
                    tr("Couldn't start the game"),
                    &error,
                    again,
                    Vec::new(),
                    ButtonKind::Secondary,
                ))
                .into_any_element()
            }
            GameProblem::Notice(notice) => notice,
        };
        let dismiss = ui::button(
            "game-warning-dismiss",
            None,
            tr("Dismiss"),
            ButtonKind::Ghost,
            ui::ButtonSize::Md,
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.game.problem = None;
            cx.notify();
        }));
        ui::notice(
            ui::Notice::Info,
            crate::icons::INFO,
            ui::said("game-warning", notice),
            vec![dismiss.into_any_element()],
        )
        .into_any_element()
    }

    fn sign_in_to_play_state(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let (title, cause) = match account::client_id() {
            Some(_) => (
                tr("Sign in to play"),
                format!("{}.", tr(account::NEEDS_OWNER)),
            ),
            None => (
                tr("Microsoft sign-in needed"),
                Self::cant_sign_in_note().to_string(),
            ),
        };
        let sign_in = self.can_sign_in().then(|| {
            microsoft::white_button("play-sign-in-microsoft").on_press(cx.listener(
                |this, _: &ClickEvent, _, cx| {
                    this.sign_in_to_play();
                    cx.notify();
                },
            ))
        });
        let dismiss = ui::button(
            "play-sign-in-dismiss",
            None,
            tr("Dismiss"),
            ButtonKind::Ghost,
            ui::ButtonSize::Md,
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.game.problem = None;
            cx.notify();
        }));
        ui::empty_state(
            "play-sign-in",
            crate::icons::ACCOUNT,
            title,
            cause,
            sign_in.into_iter().chain([dismiss]).collect(),
        )
    }

    fn first_version_state(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        if let Some(state) = self.eclipse_install_state("play-installing") {
            return state;
        }
        let update = self.updates.eclipse.as_ref().map(|r| r.tag.clone());
        let install_failed = self
            .versions
            .install_error
            .as_ref()
            .filter(|(tag, _)| update.as_ref() == Some(tag))
            .map(|(_, e)| e.clone());
        if let Some(e) = install_failed {
            return ui::error_state(
                "play-download-failed",
                ui::page_only(tr("Couldn't download Eclipse")),
                &e,
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.install_update();
                    cx.notify();
                }),
            );
        }
        if let Some(e) = self.update_check.error().map(str::to_owned) {
            return ui::error_state_with(
                "play-check-failed",
                ui::page_only(tr("Couldn't load versions")),
                &e,
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.versions.releases = Load::Loading;
                    this.fetch_startup_releases();
                    cx.notify();
                }),
                vec![
                    Self::proxy_settings_button("play-check-failed", cx),
                    self.import_build_button("play-check-failed-import", cx),
                ],
                ButtonKind::Primary,
            );
        }
        let choose = |kind| {
            let label = tr("Choose a version");
            match kind {
                ButtonKind::Primary => ui::primary("go-versions", None, label, ui::ButtonSize::Md),
                _ => ui::button("go-versions", None, label, kind, ui::ButtonSize::Md),
            }
            .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.go(Page::Versions);
                cx.notify();
            }))
        };
        if matches!(self.versions.releases, Load::Loading) && update.is_none() {
            return ui::loading_state(
                "play-checking",
                ui::page_only(tr("Looking for the newest Eclipse…")),
            );
        }
        let kept = self
            .selected_instance()
            .and_then(|i| self.plays(i))
            .filter(|plays| !plays.follows)
            .and_then(|plays| plays.tag.cloned());
        if let Some(own) = kept {
            let name = eclipse_launcher::nightly::display_name(&own);
            return ui::empty_state(
                "play-not-installed",
                crate::icons::VERSION,
                ui::page_only(trf("{version} isn't installed", &[("version", &name)])),
                tr("Choose a version to play."),
                vec![choose(ButtonKind::Primary)],
            );
        }
        let no_build = matches!(&self.versions.releases, Load::Ready(list) if !list.iter().any(|r| r.asset.is_some()));
        if no_build {
            return ui::empty_state(
                "play-not-installed",
                crate::icons::VERSION,
                ui::page_only(tr("Eclipse isn't installed yet")),
                tr("There's no Eclipse build for this computer yet."),
                vec![choose(ButtonKind::Secondary)],
            );
        }
        let install = ui::primary(
            "play-install",
            Some("download-04"),
            tr("Install Eclipse"),
            ui::ButtonSize::Md,
        );
        let install = match update {
            Some(_) => ui::keep_focus(install, &self.play_focus).on_press(cx.listener(
                |this, _: &ClickEvent, _, cx| {
                    this.install_update();
                    cx.notify();
                },
            )),
            None => ui::disabled(install),
        };
        ui::empty_state(
            "play-not-installed",
            crate::icons::VERSION,
            ui::page_only(tr("Eclipse isn't installed yet")),
            tr("Install the newest version to start playing."),
            vec![install, choose(ButtonKind::Secondary)],
        )
    }

    /// "Eclipse crashed": play again, the report and the logs, and the details to report it.
    pub(super) fn crash_notice(&self, crash: Crash, cx: &mut Context<Self>) -> Div {
        let title = tr(
            if self.selected_instance().is_some_and(|i| i.java.is_some()) {
                "Minecraft crashed"
            } else {
                "Eclipse crashed"
            },
        );
        let cause = match &crash.report {
            Some(_) => ui::told_cause(&crash.summary)
                .unwrap_or_else(|| tr("The game closed unexpectedly.").to_string()),
            None => tr("The game ended without a crash report.").to_string(),
        };
        let details = [crash.summary.clone()]
            .into_iter()
            .chain(crash.report.as_deref().map(short))
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        let again = cx.listener(|this, _: &ClickEvent, _, cx| {
            this.game.crash = None;
            this.press_play();
            cx.notify();
        });
        let open = crash.report.clone().map(|report| {
            ui::button(
                "crash-open",
                None,
                tr("Open crash report"),
                ButtonKind::Secondary,
                ui::ButtonSize::Md,
            )
            .on_press(move |_, _, cx| crate::toast::open(cx, &report))
        });
        let logs = ui::open_folder_button(
            "crash-logs",
            tr("Open logs folder"),
            backend::logs_dir_in(&crash.home),
            ui::ButtonSize::Md,
        );
        let dismiss = ui::button(
            "crash-dismiss",
            None,
            tr("Dismiss"),
            ButtonKind::Ghost,
            ui::ButtonSize::Md,
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.game.crash = None;
            cx.notify();
        }));
        ui::empty_card(ui::error_state_again(
            "crash",
            title,
            cause,
            &details,
            tr("Play again"),
            again,
            open.into_iter().chain([logs, dismiss]).collect(),
            ButtonKind::Secondary,
        ))
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
