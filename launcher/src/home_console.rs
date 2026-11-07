//! Home's Console tab: the game's output of this run as it comes, with a search, Auto-scroll,
//! Copy, and Share log (`eclipse_launcher::game_log`): an upload to mclo.gs that happens only
//! after the player confirms, without their home folder or names.
use super::fields::Field;
use crate::play_state::PlayState;
use crate::ui::{self, ink, ButtonKind, Press};
use crate::{clipboard, layout, Launcher, LOG_LINES};
use eclipse_launcher::backend;
use eclipse_launcher::game_log::{self, Shared};
use eclipse_launcher::i18n::{tr, trf};
use eclipse_theme::glass::{ink, space, step};
use gpui::prelude::*;
use gpui::{div, px, ClickEvent, ClipboardItem, Div, FontWeight, ScrollHandle};

pub(super) struct Console {
    follow: bool,
    scroll: ScrollHandle,
    /// Lines and search at the last draw: a change means scroll down when following.
    seen: (usize, String),
    confirm_share: bool,
    sharing: bool,
    shared: Option<Result<Shared, String>>,
}

impl Default for Console {
    fn default() -> Self {
        Console {
            follow: true,
            scroll: ScrollHandle::new(),
            seen: (0, String::new()),
            confirm_share: false,
            sharing: false,
            shared: None,
        }
    }
}

impl Launcher {
    fn console_home(&self) -> std::path::PathBuf {
        self.game
            .home
            .clone()
            .or_else(|| self.selected_instance().map(|i| i.home.clone()))
            .unwrap_or_else(backend::game_home)
    }

    /// What Share log sends: this run's lines, else the selected instance's latest.log.
    fn log_to_share(&self) -> Result<String, String> {
        match self.console_log().filter(|log| !log.is_empty()) {
            Some(log) => Ok(log.iter().cloned().collect::<Vec<_>>().join("\n") + "\n"),
            None => std::fs::read_to_string(backend::latest_log_in(&self.console_home()))
                .map_err(|_| tr("There is no log to share yet.").to_string()),
        }
    }

    fn share_log(&mut self, cx: &mut Context<Self>) {
        let text = match self.log_to_share() {
            Ok(text) => text,
            Err(e) => {
                self.console.shared = Some(Err(e));
                return;
            }
        };
        self.console.confirm_share = false;
        self.console.sharing = true;
        let home = self.console_home();
        let task = cx
            .background_executor()
            .spawn(async move { game_log::share_private(&text, &home) });
        cx.spawn(async move |this, cx| {
            let done = task.await;
            let _ = this.update(cx, |this, cx| {
                this.console.sharing = false;
                this.console.shared = Some(done);
                cx.notify();
            });
        })
        .detach();
    }

    fn unshare_log(&mut self, cx: &mut Context<Self>) {
        let Some(Ok(shared)) = self.console.shared.clone() else {
            return;
        };
        self.console.sharing = true;
        let task = cx
            .background_executor()
            .spawn(async move { game_log::unshare(&shared) });
        cx.spawn(async move |this, cx| {
            let done = task.await;
            let _ = this.update(cx, |this, cx| {
                this.console.sharing = false;
                this.console.shared = match done {
                    Ok(()) => None,
                    Err(e) => Some(Err(e)),
                };
                cx.notify();
            });
        })
        .detach();
    }
}

impl Launcher {
    pub(super) fn console_tab(&mut self, cx: &mut Context<Self>) -> Option<Div> {
        let log = self.console_log()?;
        let count = log.len();
        let running = self.selected_play_state() == PlayState::Running;
        let shown = count.to_string();
        let counted = [("count", shown.as_str())];
        let note = match (running, count) {
            (true, 1) => tr("1 line, the game is running").to_string(),
            (true, _) => trf("{count} lines, the game is running", &counted),
            (false, 1) => tr("1 line from the last run").to_string(),
            (false, _) => trf("{count} lines from the last run", &counted),
        };
        let search = self.fields.value(Field::ConsoleSearch).trim().to_string();
        let lines: Vec<String> = log
            .iter()
            .filter(|l| game_log::matches(l, &search))
            .cloned()
            .collect();
        let now = (count, search);
        if self.console.follow && now != self.console.seen {
            self.console.scroll.scroll_to_bottom();
        }
        self.console.seen = now;
        let follow = ui::toggle_label("console-follow", tr("Auto-scroll"), self.console.follow)
            .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.console.follow = !this.console.follow;
                this.console.seen.0 = usize::MAX;
                cx.notify();
            }));
        let copied = lines.join("\n");
        let logs = backend::logs_dir_in(&self.console_home());
        let share = ui::button(
            "console-share",
            None,
            tr("Share log…"),
            ButtonKind::Secondary,
            ui::ButtonSize::Md,
        );
        let share = match self.console.sharing {
            true => ui::disabled(share),
            false => share.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.console.confirm_share = true;
                cx.notify();
            })),
        };
        let tools = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(space::SM))
            .px(px(space::MD))
            .pb(px(space::MD))
            .child(self.field(
                Field::ConsoleSearch,
                tr("Search the log"),
                layout::FIELD_MD,
                ui::ButtonSize::Md,
                cx,
            ))
            .child(div().ml(px(space::SM)).child(follow))
            .child(div().flex_1())
            .child(
                ui::button(
                    "console-copy",
                    Some(crate::icons::COPY),
                    tr("Copy log"),
                    ButtonKind::Secondary,
                    ui::ButtonSize::Md,
                )
                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    clipboard::copy_to_clipboard(cx, ClipboardItem::new_string(copied.clone()));
                    this.console.shared = None;
                    cx.notify();
                })),
            )
            .child(share)
            .child(ui::open_folder_icon(
                "console-folder",
                tr("Open logs folder"),
                logs,
                ButtonKind::Ghost,
                ui::ButtonSize::Md,
            ));
        let mut body = div()
            .flex()
            .flex_col()
            .child(ui::list_head(note))
            .child(tools);
        // Share log asks first.
        if self.console.confirm_share {
            body = body.child(
                ui::well()
                    .flex()
                    .items_center()
                    .gap(px(space::MD))
                    .mx(px(space::MD))
                    .mb(px(space::MD))
                    .child(div().flex_1().min_w_0().child(
                        ui::text(step::NOTE, FontWeight::NORMAL, ink::BODY).child(tr(
                            "Upload this log to mclo.gs? Anyone with the link can read it.",
                        )),
                    ))
                    .child(
                        ui::button(
                            "console-share-no",
                            None,
                            tr("Cancel"),
                            ButtonKind::Ghost,
                            ui::ButtonSize::Md,
                        )
                        .on_press(cx.listener(
                            |this, _: &ClickEvent, _, cx| {
                                this.console.confirm_share = false;
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        ui::button(
                            "console-share-yes",
                            None,
                            tr("Upload"),
                            ButtonKind::Secondary,
                            ui::ButtonSize::Md,
                        )
                        .on_press(cx.listener(
                            |this, _: &ClickEvent, _, cx| {
                                this.share_log(cx);
                                cx.notify();
                            },
                        )),
                    ),
            );
        }
        let status = match &self.console.shared {
            _ if self.console.sharing => Some(ui::list_head(tr("Uploading…"))),
            Some(Ok(shared)) => {
                let (url, open_url) = (shared.url.clone(), shared.url.clone());
                Some(
                    ui::list_head(trf("Shared: {url}", &[("url", &shared.url)])).child(
                        div()
                            .flex()
                            .gap(px(space::SM))
                            .child(ui::copy_button(
                                "console-link-copy",
                                tr("Copy link"),
                                url.clone(),
                                ui::ButtonSize::Md,
                            ))
                            .child(
                                ui::button(
                                    "console-link-open",
                                    None,
                                    tr("Open link"),
                                    ButtonKind::Secondary,
                                    ui::ButtonSize::Md,
                                )
                                .on_press(move |_, _, cx| cx.open_url(&open_url)),
                            )
                            .child(
                                ui::button(
                                    "console-unshare",
                                    None,
                                    tr("Delete upload"),
                                    ButtonKind::Secondary,
                                    ui::ButtonSize::Md,
                                )
                                .on_press(cx.listener(
                                    |this, _: &ClickEvent, _, cx| {
                                        this.unshare_log(cx);
                                        cx.notify();
                                    },
                                )),
                            ),
                    ),
                )
            }
            Some(Err(e)) => Some(
                ui::error_note(ui::plain_error(e).to_string())
                    .mx(px(space::MD))
                    .mb(px(space::MD)),
            ),
            None => None,
        };
        let shown = lines.len().saturating_sub(LOG_LINES);
        let log = ui::log_well("console-lines", &self.console.scroll).children(
            lines.into_iter().skip(shown).map(|line| {
                let color = if line.contains("/ERROR]") || line.contains("ERROR:") {
                    ui::danger()
                } else if line.contains("/WARN]") {
                    ui::warn()
                } else {
                    ink(ink::BODY)
                };
                div().text_color(color).child(line)
            }),
        );
        Some(body.children(status).child(log))
    }
}
