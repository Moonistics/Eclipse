//! Home's Console tab: the game's output of this run as it comes, with a search, Follow (keep the
//! newest line in view), Copy, and Share log (`eclipse_launcher::game_log`): an upload to mclo.gs
//! that happens only after the player confirms, without their home folder or names.
use super::servers_page::Field;
use super::*;
use eclipse_launcher::game_log::{self, Shared};
use gpui::{ClipboardItem, ScrollHandle};

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
    /// What Share log sends: this run's lines, else the selected instance's latest.log.
    fn log_to_share(&self) -> Result<(String, Vec<std::path::PathBuf>), String> {
        let home = self
            .client_home
            .clone()
            .or_else(|| self.selected_instance().map(|i| i.home.clone()))
            .unwrap_or_else(backend::game_home);
        let text = match self.prepare_log.is_empty() {
            false => {
                self.prepare_log
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n")
                    + "\n"
            }
            true => {
                let path = home.join("logs/latest.log");
                std::fs::read_to_string(&path)
                    .map_err(|_| tr("There is no log to share yet.").to_string())?
            }
        };
        Ok((text, vec![home, backend::game_home()]))
    }

    fn share_log(&mut self, cx: &mut Context<Self>) {
        let shared = match self.log_to_share() {
            Ok(log) => log,
            Err(e) => {
                self.console.shared = Some(Err(e));
                return;
            }
        };
        self.console.confirm_share = false;
        self.console.sharing = true;
        let task = cx.background_executor().spawn(async move {
            let (text, homes) = shared;
            let homes: Vec<&std::path::Path> = homes.iter().map(|h| h.as_path()).collect();
            let names = game_log::names(&homes);
            game_log::share(&game_log::private(
                &text,
                dirs::home_dir().as_deref(),
                &names,
            ))
        });
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
    /// Home's Console tab in the glass look: the count and the tools over the log, which is the
    /// mockup's `Logs` (round at 12, black at 25 %, Geist Mono 12 px on 1.75, white at 75 %).
    /// Not in the mockup (its Overview shows a game log only while starting): kept, in its
    /// patterns. Dark appearance only.
    pub(super) fn glass_console(&mut self, cx: &mut Context<Self>) -> Option<Div> {
        use crate::glass_ui::{self as ui, white};
        use eclipse_theme::glass::{fill, ink, radius};
        let t = self.theme;
        if self.prepare_log.is_empty() && self.client.is_none() {
            return None;
        }
        let count = self.prepare_log.len();
        let note = match self.client {
            Some(_) => trf(
                "{count} lines, the game is running",
                &[("count", &count.to_string())],
            ),
            None => trf(
                "{count} lines from the last run",
                &[("count", &count.to_string())],
            ),
        };
        let search = self.servers.value(Field::ConsoleSearch).trim().to_string();
        let lines: Vec<String> = self
            .prepare_log
            .iter()
            .filter(|l| game_log::matches(l, &search))
            .cloned()
            .collect();
        let now = (count, search);
        if self.console.follow && now != self.console.seen {
            self.console.scroll.scroll_to_bottom();
        }
        self.console.seen = now;
        let follow = div()
            .id("console-follow")
            .flex()
            .items_center()
            .gap(px(8.))
            .cursor_pointer()
            .child(ui::toggle(
                "console-follow-switch",
                self.console.follow,
                self.look.accent,
                self.look.on_accent,
                &t,
            ))
            .child(ui::text(14., FontWeight::NORMAL, ink::BODY).child(tr("Auto-scroll")))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.console.follow = !this.console.follow;
                this.console.seen.0 = usize::MAX;
                cx.notify();
            }));
        let copied = lines.join("\n");
        let logs = self
            .client_home
            .clone()
            .or_else(|| self.selected_instance().map(|i| i.home.clone()))
            .unwrap_or_else(backend::game_home)
            .join("logs");
        let share = ui::button(
            "console-share",
            None,
            tr("Share log…"),
            Kind::Secondary,
            ui::Size::Md,
            &t,
        );
        let share = match self.console.sharing {
            true => crate::disabled(share),
            false => share.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.console.confirm_share = true;
                cx.notify();
            })),
        };
        let tools = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.))
            .px(px(12.))
            .pb(px(12.))
            .child(self.field(Field::ConsoleSearch, tr("Search the log"), 240., cx))
            .child(div().ml(px(8.)).child(follow))
            .child(div().flex_1())
            .child(
                ui::button(
                    "console-copy",
                    None,
                    tr("Copy"),
                    Kind::Secondary,
                    ui::Size::Md,
                    &t,
                )
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    copy_to_clipboard(cx, ClipboardItem::new_string(copied.clone()));
                    this.console.shared = None;
                    cx.notify();
                })),
            )
            .child(share)
            .child(
                ui::icon_button(
                    "console-folder",
                    "folder-01",
                    36.,
                    17.,
                    radius::CONTROL,
                    false,
                    &t,
                )
                .on_click(move |_, _, cx| {
                    let _ = std::fs::create_dir_all(&logs);
                    crate::glass_toast::open(cx, &logs);
                }),
            );
        let mut body = div()
            .flex()
            .flex_col()
            .child(ui::list_head(note))
            .child(tools);
        // Share log asks first, on a card as the patch log's (white at 4 %, round at 14).
        if self.console.confirm_share {
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .mx(px(12.))
                    .mb(px(12.))
                    .p(px(16.))
                    .rounded(px(radius::ROW))
                    .bg(white(fill::WELL))
                    .child(div().flex_1().min_w_0().child(
                        ui::text(13., FontWeight::NORMAL, ink::BODY).child(tr(
                            "Upload this log to mclo.gs? Anyone with the link can read it.",
                        )),
                    ))
                    .child(
                        ui::button(
                            "console-share-no",
                            None,
                            tr("Cancel"),
                            Kind::Secondary,
                            ui::Size::Md,
                            &t,
                        )
                        .on_click(cx.listener(
                            |this, _: &ClickEvent, _, cx| {
                                this.console.confirm_share = false;
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        ui::solid("console-share-yes", tr("Upload"), 36., 14., &t).on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.share_log(cx);
                                cx.notify();
                            }),
                        ),
                    ),
            );
        }
        let status = match &self.console.shared {
            _ if self.console.sharing => Some(ui::list_head(tr("Sending…"))),
            Some(Ok(shared)) => {
                let (url, open_url) = (shared.url.clone(), shared.url.clone());
                Some(
                    ui::list_head(trf("Shared: {url}", &[("url", &shared.url)])).child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .child(
                                ui::button(
                                    "console-link-copy",
                                    None,
                                    tr("Copy link"),
                                    Kind::Secondary,
                                    ui::Size::Md,
                                    &t,
                                )
                                .on_click(move |_, _, cx| {
                                    copy_to_clipboard(cx, ClipboardItem::new_string(url.clone()))
                                }),
                            )
                            .child(
                                ui::button(
                                    "console-link-open",
                                    None,
                                    tr("Open"),
                                    Kind::Secondary,
                                    ui::Size::Md,
                                    &t,
                                )
                                .on_click(move |_, _, cx| cx.open_url(&open_url)),
                            )
                            .child(
                                ui::button(
                                    "console-unshare",
                                    None,
                                    tr("Delete upload"),
                                    Kind::Secondary,
                                    ui::Size::Md,
                                    &t,
                                )
                                .on_click(cx.listener(
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
                div().px(px(12.)).pb(px(8.)).child(
                    ui::text(13., FontWeight::NORMAL, ink::FULL)
                        .text_color(ui::danger())
                        .child(e.clone()),
                ),
            ),
            None => None,
        };
        let shown = lines.len().saturating_sub(LOG_LINES);
        let log = div()
            .id("console-lines")
            .track_scroll(&self.console.scroll)
            .h(px(320.))
            .overflow_y_scroll()
            .p(px(16.))
            .rounded(px(radius::ROW))
            .bg(ui::well())
            .font_family(theme::MONO)
            .text_size(px(12.))
            .line_height(px(21.))
            .children(lines.into_iter().skip(shown).map(|line| {
                let color = if line.contains("/ERROR]") || line.contains("ERROR:") {
                    ui::danger()
                } else if line.contains("/WARN]") {
                    ui::warn()
                } else {
                    white(ink::BODY)
                };
                div().text_color(color).child(line)
            }));
        Some(body.children(status).child(log))
    }
}
