//! The Servers page: the open server's card with its state, name, numbers, Start or Restart and
//! Stop, its console and command line; beside it the list of servers and how friends reach the
//! open one. Below, the server's Players, Settings and Backups.
use super::*;
use crate::tab_strip::{Strip, StripTab};
use crate::ui::{self, white, Tone};
use eclipse_theme::glass::{ink, pixel_text, space, step, TextStep};

/// The right column's width.
const SIDE: f32 = 380.;

const LIST_ROW_PAD: f32 = space::SM + space::XXS;

/// The tabs under the server's card.
const LOWER_TABS: [(Tab, &str); 3] = [
    (Tab::Players, "Players"),
    (Tab::Settings, "Settings"),
    (Tab::Backups, "Backups"),
];

/// A dot's tone for a server's state.
fn tone(status: &Status) -> Tone {
    match () {
        _ if status.serving() && status.crashes > 0 => Tone::Warn,
        _ if status.serving() => Tone::Live,
        _ if !status.running() => Tone::Off,
        _ => Tone::Busy,
    }
}

impl Launcher {
    pub(super) fn servers_view(&mut self, cx: &mut Context<Self>) -> Div {
        let head = ui::page_head(Page::Servers.title(), None);
        // A server is always shown: the open one, else the first.
        let listed = |d: &PathBuf| self.servers.list.iter().any(|e| &e.dir == d);
        if !self.servers.open.as_ref().is_some_and(listed) {
            if let Some(first) = self.servers.list.first() {
                let dir = first.dir.clone();
                self.open_server(dir);
            }
        }
        let open = self
            .servers
            .open
            .clone()
            .filter(|d| self.servers.list.iter().any(|e| &e.dir == d));
        let main = match &open {
            Some(dir) => self.server_card(dir, cx),
            None => {
                return div()
                    .flex()
                    .flex_col()
                    .child(head)
                    .child(self.no_server(cx))
            }
        };
        let status = self.servers.status();
        let content = layout::content_width(self.window_width);
        let beside = content >= layout::SERVER_CARD_MIN + layout::CARD_GAP + SIDE;
        let list = self.server_list(cx);
        let addresses = self.server_addresses(cx);
        let side = if beside {
            div()
                .flex()
                .flex_col()
                .flex_none()
                .w(px(SIDE))
                .gap(px(layout::CARD_GAP))
                .child(list)
                .child(addresses)
        } else if content >= 2. * SIDE + layout::CARD_GAP {
            div()
                .flex()
                .items_start()
                .gap(px(layout::CARD_GAP))
                .child(list.flex_1().min_w_0())
                .child(addresses.flex_1().min_w_0())
        } else {
            div()
                .flex()
                .flex_col()
                .gap(px(layout::CARD_GAP))
                .child(list)
                .child(addresses)
        };
        let columns = if beside {
            div()
                .flex()
                .items_start()
                .gap(px(layout::CARD_GAP))
                .child(main.flex_1().min_w_0())
                .child(side)
        } else {
            div()
                .flex()
                .flex_col()
                .gap(px(layout::CARD_GAP))
                .child(main)
                .child(side)
        };
        let tabs = open.map(|dir| self.server_tabs(&dir, &status, cx));
        div()
            .flex()
            .flex_col()
            .child(head)
            .child(columns)
            .children(tabs)
    }

    /// The open server's card: its state, name and numbers, its buttons, then its console.
    fn server_card(&mut self, dir: &Path, cx: &mut Context<Self>) -> Div {
        let status = self.servers.status();
        let running = status.running();
        let idle = self.servers.busy.is_none();
        let name = self
            .servers
            .list
            .iter()
            .find(|e| e.dir == dir)
            .map(|e| e.name.clone())
            .unwrap_or_default();
        let mut state = phase_label(&status);
        if let Some(up) = status.uptime.filter(|_| running) {
            state = format!("{state} · {}", trf("up {time}", &[("time", &uptime(up))]));
        }
        let numbers = if running {
            let mut parts = vec![trf(
                "{online} of {max} players",
                &[
                    ("online", &status.players.len().to_string()),
                    ("max", &status.max_players.unwrap_or(0).to_string()),
                ],
            )];
            if let Some(tps) = status.tps {
                parts.push(trf("{tps} TPS", &[("tps", &format!("{tps:.1}"))]));
            }
            if let Some(mspt) = status.mspt {
                parts.push(trf("{ms} ms a tick", &[("ms", &format!("{mspt:.1}"))]));
            }
            if let Some(ram) = status.ram_bytes {
                parts.push(format!("{:.2} GB", ram as f64 / 1e9));
            }
            parts.join(" · ")
        } else {
            let mode = GameMode::label_of(&self.servers.shown(ServerKey::GameMode));
            let max = self.servers.shown(ServerKey::MaxPlayers);
            [
                mode,
                self.servers.shown(ServerKey::WorldName),
                if max.is_empty() {
                    max
                } else {
                    trf("up to {max} players", &[("max", &max)])
                },
            ]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" · ")
        };

        let (panel_icon, panel_label) = (crate::icons::SERVER_PANEL, tr("Open the web panel"));
        let panel = if running {
            let d = dir.to_path_buf();
            ui::icon_button(
                "server-panel",
                panel_icon,
                panel_label,
                ButtonKind::Ghost,
                ui::ButtonSize::Sm,
                false,
            )
            .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                match servers::Api::open(&d).map(|a| a.panel_link()) {
                    Some(Ok(url)) => cx.open_url(&url),
                    Some(Err(e)) => this.servers.problem = Some(Problem::plain(e)),
                    None => {}
                }
                cx.notify();
            }))
        } else {
            ui::icon_button_unavailable(
                "server-panel",
                panel_icon,
                panel_label,
                ui::ButtonSize::Sm,
                tr("Start the server first"),
            )
        };
        let folder = dir.to_path_buf();
        let folder = ui::icon_button(
            "server-folder",
            "folder-01",
            tr("Open server folder"),
            ButtonKind::Ghost,
            ui::ButtonSize::Sm,
            false,
        )
        .on_press(move |_, _, cx| ui::open_folder(&folder, cx));

        let mut buttons = div().flex().flex_none().items_center().gap(px(space::SM));
        if running {
            let restart = ui::button(
                "server-restart",
                None,
                tr("Restart"),
                ButtonKind::Secondary,
                ui::ButtonSize::Lg,
            );
            buttons = buttons.child(if idle {
                restart.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.servers.restart_needed.clear();
                    this.server_command(ServerJob::Restarting, vec!["restart".into()]);
                    cx.notify();
                }))
            } else {
                ui::disabled(restart)
            });
            let stop = ui::button(
                "server-stop",
                Some("stop"),
                tr("Stop"),
                ButtonKind::Secondary,
                ui::ButtonSize::Lg,
            );
            buttons = buttons.child(if idle {
                stop.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.server_command(ServerJob::Stopping, vec!["stop".into()]);
                    cx.notify();
                }))
            } else {
                ui::disabled(stop)
            });
        } else if self.servers.busy == Some(ServerJob::Starting) {
            buttons = buttons.child(ui::primary_busy(
                "server-start",
                tr("Starting"),
                None,
                ui::ButtonSize::Lg,
            ));
        } else {
            let start = ui::primary(
                "server-start",
                Some("play"),
                tr("Start"),
                ui::ButtonSize::Lg,
            );
            buttons = buttons.child(if idle {
                start.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.start_server();
                    cx.notify();
                }))
            } else {
                ui::disabled(start)
            });
        }

        let mut card = ui::card()
            .flex()
            .flex_col()
            .p(px(ui::CARD_PADDING))
            .child(
                div()
                    .relative()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(space::MD))
                    .child(
                        ui::text(step::NOTE, FontWeight::NORMAL, ink::BODY)
                            .min_w_0()
                            .child(ui::state_line(tone(&status), state)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .gap(px(space::XS))
                            .child(panel)
                            .child(folder),
                    ),
            )
            .child(
                div()
                    .relative()
                    .mt(px(space::MD))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(space::XL))
                    .child(
                        ui::entity_title(name, pixel_text::CARD_TITLE)
                            .min_w_0()
                            .truncate(),
                    )
                    .child(buttons),
            )
            .child(
                ui::text(step::BODY, FontWeight::NORMAL, ink::BODY)
                    .relative()
                    .mt(px(space::SM))
                    .truncate()
                    .child(numbers),
            );
        // What the launcher says besides: a crash count, the action under way, a problem, a
        // note, settings that wait for a restart.
        if status.crashes > 0 {
            card = card.child(
                ui::card_note(trf(
                    "The server crashed {count} times and restarted itself.",
                    &[("count", &status.crashes.to_string())],
                ))
                .relative()
                .mt(px(space::SM)),
            );
        }
        card = card.children(self.server_notes(running, idle, cx));
        card.child(self.server_console(running, cx))
    }

    /// The action under way, a problem, a note and settings waiting for a restart.
    fn server_notes(&mut self, running: bool, idle: bool, cx: &mut Context<Self>) -> Vec<Div> {
        let mut out = Vec::new();
        if let Some(job) = self.servers.busy.clone() {
            let last = self.servers.job_log.back().cloned().unwrap_or_default();
            out.push(div().relative().mt(px(space::LG)).child(ui::progress_line(
                job.label(),
                None,
                last,
            )));
        }
        if let Some(bar) = self.servers_preparing() {
            out.push(div().relative().mt(px(space::LG)).child(bar));
        }
        if let Some(p) = self.servers.problem.clone() {
            let mut actions = Vec::new();
            if let Some(fix) = p.fix {
                let label = match &fix {
                    Fix::UsePort(port) => trf("Use port {port}", &[("port", &port.to_string())]),
                    Fix::Prepare => set_up_label(&self.servers_minecraft()),
                    Fix::AgreeEula => tr("I agree to the EULA").into(),
                    Fix::ShowLog => tr("Open the logs").into(),
                };
                let button = ui::button(
                    "problem-fix",
                    None,
                    label,
                    ButtonKind::Secondary,
                    ui::ButtonSize::Sm,
                );
                actions.push(self.fix_button(button, fix, cx).into_any_element());
            }
            actions.push(
                ui::button(
                    "problem-dismiss",
                    None,
                    tr("Dismiss"),
                    ButtonKind::Ghost,
                    ui::ButtonSize::Sm,
                )
                .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.servers.problem = None;
                    cx.notify();
                }))
                .into_any_element(),
            );
            out.push(ui::error_line(p.message.clone(), actions).mt(px(space::LG)));
        }
        if let Some(n) = self.servers.note.clone() {
            out.push(ui::notice_line(ui::Tone::Off, n).mt(px(space::LG)));
        }
        if running && !self.servers.restart_needed.is_empty() {
            let names: Vec<String> = self
                .servers
                .restart_needed
                .iter()
                .map(|k| setting_name(k))
                .collect();
            let restart = ui::button(
                "restart-apply",
                None,
                tr("Restart now"),
                ButtonKind::Secondary,
                ui::ButtonSize::Sm,
            );
            out.push(
                ui::notice_line(
                    ui::Tone::Accent(ui::look().accent_soft),
                    trf(
                        "Saved. Restart to apply: {settings}.",
                        &[("settings", &names.join(", "))],
                    ),
                )
                .mt(px(space::LG))
                .child(if idle {
                    restart.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.servers.restart_needed.clear();
                        this.server_command(ServerJob::Restarting, vec!["restart".into()]);
                        cx.notify();
                    }))
                } else {
                    ui::disabled(restart)
                }),
            );
        }
        out
    }

    /// The console and the command line under it, or what a stopped server shows instead.
    fn server_console(&mut self, running: bool, cx: &mut Context<Self>) -> Div {
        let console = div().relative().mt(px(space::XL)).flex().flex_col();
        if !running {
            return console.child(
                ui::well()
                    .flex()
                    .items_center()
                    .gap(px(space::MD))
                    .child(ui::icon(
                        crate::icons::CONSOLE,
                        ui::IconSize::Px16,
                        white(ink::MUTED),
                    ))
                    .child(ui::empty_line(
                        "server-console-stopped",
                        tr("Start the server to see its console."),
                    )),
            );
        }
        let log = ui::log_well("server-console", &self.servers.console_scroll);
        let log = if self.servers.log.is_empty() {
            log.flex()
                .items_center()
                .justify_center()
                .child(ui::empty_line(
                    "server-console-waiting",
                    tr("Waiting for the server's log…"),
                ))
        } else {
            let accent = ui::color(ui::look().accent_soft);
            log.text_color(white(ink::BODY))
                .children(self.servers.log.iter().map(|l| {
                    div()
                        .when(l.starts_with("> "), |d| d.text_color(accent))
                        .child(l.clone())
                }))
        };
        let send = ui::button(
            "server-send",
            None,
            tr("Send"),
            ButtonKind::Secondary,
            ui::ButtonSize::Sm,
        );
        let typed = !self.fields.value(Field::ServerCommand).trim().is_empty();
        let send = if typed {
            send.on_press(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.field_enter(Field::ServerCommand, window, cx);
                cx.notify();
            }))
        } else {
            ui::unavailable(send, tr("Type a command first"))
        };
        let slash = div()
            .text_color(white(ink::DIM))
            .child("/")
            .into_any_element();
        console.child(log).child(
            self.field_with(
                Field::ServerCommand,
                Some(slash),
                tr("Type a command, like list or time set day"),
                layout::FIELD_XL,
                ui::ButtonSize::Md,
                Some(send.into_any_element()),
                cx,
            )
            .w_full()
            .mt(px(space::SM))
            .pr(px(space::XS)),
        )
    }

    fn no_server(&mut self, cx: &mut Context<Self>) -> Div {
        let needs = self.server_needs();
        if let Some(state) = self
            .eclipse_install_state("servers-installing")
            .filter(|_| needs == Some(Needs::Eclipse))
        {
            return ui::empty_card(state);
        }
        let game_files = matches!(needs, Some(Needs::GameFiles { .. }));
        if let Some(bar) = self.servers_preparing().filter(|_| game_files) {
            let title = trf(
                "Setting up Minecraft {version}",
                &[("version", &self.servers_minecraft())],
            );
            return ui::empty_card(
                ui::loading_state("servers-preparing", ui::page_only(title))
                    .child(bar.mt(px(space::LG))),
            );
        }
        if let Some(e) = self.shown_prepare_error().cloned().filter(|_| game_files) {
            return ui::empty_card(self.servers_prepare_failed(&e, cx));
        }
        let first = match needs.clone() {
            Some(need) => self.server_need_button("servers-empty", need, ButtonKind::Primary, cx),
            None => ui::primary(
                "server-create-empty",
                Some("plus-sign"),
                tr("Create a server"),
                ui::ButtonSize::Md,
            )
            .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.open_create(cx);
                cx.notify();
            })),
        };
        let add = self.server_import_button("server-import-empty", cx);
        let problem = self
            .servers
            .problem
            .clone()
            .map(|p| ui::error_note(p.message).mt(px(space::LG)));
        let note = needs.map_or_else(
            || tr("Host a world your friends can join.").to_string(),
            |need| need.line(),
        );
        ui::empty_card(
            ui::empty_state(
                "servers-empty",
                Page::Servers.icon(),
                ui::page_only(tr("No servers yet")),
                note,
                vec![first, add],
            )
            .children(problem),
        )
    }

    pub(super) fn server_need_button(
        &mut self,
        prefix: &str,
        need: Needs,
        kind: ButtonKind,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let size = ui::ButtonSize::Md;
        let button = |id: String, icon, label: SharedString| match kind {
            ButtonKind::Primary => ui::primary(id, icon, label, size),
            _ => ui::button(id, icon, label, kind, size),
        };
        match need {
            Needs::Eclipse if self.updates.eclipse.is_some() => button(
                format!("{prefix}-install-eclipse"),
                Some("download-04"),
                tr("Install Eclipse").into(),
            )
            .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.install_update();
                cx.notify();
            })),
            Needs::Eclipse => button(
                format!("{prefix}-choose-version"),
                None,
                tr("Choose a version").into(),
            )
            .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.servers.create = None;
                this.servers.problem = None;
                this.go(Page::Versions);
                cx.notify();
            })),
            Needs::GameFiles { minecraft } => {
                let label = set_up_label(&minecraft).into();
                self.fix_button(
                    button(format!("{prefix}-prepare"), None, label),
                    Fix::Prepare,
                    cx,
                )
            }
            Needs::ServerProgram { .. } => button(
                format!("{prefix}-versions"),
                None,
                tr("Manage versions").into(),
            )
            .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.servers.create = None;
                this.servers.problem = None;
                this.go(Page::Versions);
                cx.notify();
            })),
        }
    }

    fn server_import_button(&mut self, id: &'static str, cx: &mut Context<Self>) -> Stateful<Div> {
        let add = ui::button(
            id,
            Some("folder-01"),
            tr("Import a server…"),
            ButtonKind::Secondary,
            ui::ButtonSize::Md,
        );
        let needs = self.server_needs().filter(|_| servers::program().is_none());
        if self.servers.creating {
            ui::disabled(add)
        } else if let Some(need) = needs {
            ui::unavailable(add, need.line())
        } else {
            add.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.add_server_folder(cx);
                cx.notify();
            }))
        }
    }

    pub(super) fn servers_minecraft(&self) -> String {
        self.selected_version().map_or_else(
            || servers::GAME_VERSION.to_string(),
            |tag| backend::game_version(tag),
        )
    }

    pub(super) fn fix_button(
        &self,
        button: Stateful<Div>,
        fix: Fix,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        match self.busy_reason().filter(|_| fix == Fix::Prepare) {
            Some(why) => ui::unavailable(button, why),
            None => button.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.fix(fix.clone(), cx);
                cx.notify();
            })),
        }
    }

    pub(super) fn servers_preparing(&self) -> Option<Div> {
        let PlayState::Preparing(done) = self.selected_play_state() else {
            return None;
        };
        let last = self.game.log.back().cloned().unwrap_or_default();
        Some(
            div()
                .w_full()
                .max_w(px(layout::FIELD_MD))
                .child(ui::progress_line(
                    tr("Preparing"),
                    done.map(|d| (d, d)),
                    last,
                )),
        )
    }

    fn servers_prepare_failed(&self, error: &str, cx: &mut Context<Self>) -> Stateful<Div> {
        let minecraft = self
            .selected_version()
            .map(|tag| backend::game_version(tag))
            .unwrap_or_default();
        let again = cx.listener(|this, _: &ClickEvent, _, cx| {
            this.fix(Fix::Prepare, cx);
            cx.notify();
        });
        let args = [("version", minecraft.as_str())];
        if error.starts_with(&eclipse_launcher::sources::not_found_line(&minecraft)) {
            return ui::error_state_told(
                "servers-prepare-failed",
                ui::page_only(trf("Minecraft {version} not found", &args)),
                trf(
                    "Play {version} once in your Minecraft launcher, then try again.",
                    &args,
                ),
                error,
                again,
                Vec::new(),
                ButtonKind::Primary,
            );
        }
        let versions = ui::button(
            "servers-prepare-failed-versions",
            None,
            tr("Manage versions"),
            ButtonKind::Secondary,
            ui::ButtonSize::Md,
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.go(Page::Versions);
            cx.notify();
        }));
        ui::error_state_told(
            "servers-prepare-failed",
            ui::page_only(tr("Couldn't set up the game")),
            ui::told_cause(error)
                .unwrap_or_else(|| trf("Eclipse couldn't get Minecraft {version} ready.", &args)),
            error,
            again,
            vec![versions],
            ButtonKind::Primary,
        )
    }

    /// The servers, the open one lit, then Create a server and adding a server folder.
    fn server_list(&mut self, cx: &mut Context<Self>) -> Div {
        let mut rows = Vec::new();
        for e in self.servers.list.clone() {
            let status = self
                .servers
                .statuses
                .get(&e.dir)
                .cloned()
                .unwrap_or_else(Status::stopped);
            let current = self.servers.open.as_ref() == Some(&e.dir);
            let line = match () {
                _ if status.serving() => trf(
                    "{online} of {max} online",
                    &[
                        ("online", &status.players.len().to_string()),
                        ("max", &status.max_players.unwrap_or(0).to_string()),
                    ],
                ),
                _ if !status.running() => self.servers.stopped_line(&e.dir),
                _ => phase_label(&status),
            };
            let row = ui::row(
                format!("server-row-{}", e.dir.display()),
                e.name.clone(),
                current,
            )
            .pr(px(space::MD))
            .child(ui::item_text(
                e.name.clone(),
                ui::state_line(tone(&status), line),
            ));
            rows.push((row, SharedString::from(e.name.clone()), current));
        }
        let dirs: Vec<PathBuf> = self.servers.list.iter().map(|e| e.dir.clone()).collect();
        let this = cx.entity().downgrade();
        let servers = ui::options(
            div().id("server-rows").flex().flex_col(),
            tr("Servers"),
            rows,
            true,
            move |i, _, cx| {
                let Some(dir) = dirs.get(i).cloned() else {
                    return;
                };
                let _ = this.update(cx, |this, cx| {
                    this.servers.polled = None;
                    this.servers.problem = None;
                    this.servers.note = None;
                    this.open_server(dir);
                    cx.notify();
                });
            },
        );
        let list = ui::card()
            .flex()
            .flex_col()
            .p(px(ui::LIST_INSET))
            .child(servers);
        let create = ui::button(
            "server-create",
            Some("plus-sign"),
            tr("Create a server"),
            ButtonKind::Secondary,
            ui::ButtonSize::Md,
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.open_create(cx);
            cx.notify();
        }));
        let add = self.server_import_button("server-import", cx);
        list.child(
            div()
                .flex()
                .flex_wrap()
                .gap(px(space::SM))
                .p(px(space::SM))
                .child(create)
                .child(add),
        )
    }

    /// How friends reach the open server: on this network and over the internet (Tailscale).
    fn server_addresses(&mut self, cx: &mut Context<Self>) -> Div {
        let port = port_of(self);
        let lan = match servers::lan_address() {
            Some(ip) if port == 25565 => ip.to_string(),
            Some(ip) => format!("{ip}:{port}"),
            None => format!("localhost:{port}"),
        };
        let copy = |id: &'static str, text: String| {
            ui::icon_button(
                id,
                "copy-01",
                tr("Copy the address"),
                ButtonKind::Ghost,
                ui::ButtonSize::Md,
                false,
            )
            .on_press(move |_, _, cx| {
                clipboard::copy_to_clipboard(cx, gpui::ClipboardItem::new_string(text.clone()))
            })
        };
        let address = |value: String, size: TextStep, button: Stateful<Div>| {
            div()
                .mt(px(space::XS))
                .flex()
                .items_center()
                .justify_between()
                .gap(px(space::MD))
                .child(
                    ui::text(size, FontWeight::MEDIUM, ink::FULL)
                        .min_w_0()
                        .truncate()
                        .child(value),
                )
                .child(button)
        };
        let here = ui::field_group(
            "server-lan-title",
            tr("On this network"),
            None,
            div()
                .flex()
                .flex_col()
                .child(address(
                    lan.clone(),
                    step::FIGURE,
                    copy("address-copy", lan),
                ))
                .child(
                    ui::card_note(tr("Friends on the same Wi-Fi can join with this address."))
                        .mt(px(space::XS)),
                ),
        );
        let internet = ui::field_group(
            "server-internet-title",
            tr("Over the internet"),
            None,
            self.internet_card(port, cx),
        );
        ui::card()
            .flex()
            .flex_col()
            .gap(px(space::XL))
            .p(px(ui::CARD_PADDING))
            .child(here.relative())
            .child(internet.relative())
    }

    /// The server's Players, Settings and Backups, on a card under it.
    fn server_tabs(&mut self, dir: &Path, status: &Status, cx: &mut Context<Self>) -> Div {
        let tabs = LOWER_TABS
            .iter()
            .map(|&(tab, label)| StripTab {
                value: tab,
                label: tr(label).into(),
                count: None,
            })
            .collect();
        let (strip, panel) = self.tab_strip(
            Strip::Server,
            tabs,
            self.servers.tab,
            layout::content_width(self.window_width) - 2. * ui::CARD_PADDING,
            |this, tab, _| {
                this.servers.tab = tab;
                this.asking = None;
            },
            cx,
        );
        let players = self.servers.tab == Tab::Players;
        let body = match self.servers.tab {
            Tab::Settings => self.settings_tab(status, cx),
            Tab::Backups => self.backups_tab(status, cx),
            Tab::Players => self.players_tab(dir, status, cx),
        };
        ui::card()
            .mt(px(layout::CARD_GAP))
            .flex()
            .flex_col()
            .p(px(ui::CARD_PADDING))
            .child(strip.relative())
            .child(
                panel
                    .relative()
                    .flex()
                    .flex_col()
                    .gap(px(space::XL))
                    .child(body)
                    .when(players, |d| {
                        d.child(self.internet_invites(port_of(self), cx))
                    }),
            )
    }

    /// A row of a tab's list (players, backups): what leads it, the name over its detail, then
    /// what is on its right.
    pub(super) fn static_row(
        &self,
        lead: Option<Div>,
        name: impl Into<SharedString>,
        detail: Option<String>,
        right: Option<AnyElement>,
    ) -> Div {
        let text = match detail {
            Some(detail) => ui::item_text(name, detail),
            None => div().flex_1().min_w_0().child(
                ui::text(step::BODY, FontWeight::SEMIBOLD, ink::FULL)
                    .truncate()
                    .child(name.into()),
            ),
        };
        div()
            .flex()
            .items_center()
            .gap(px(space::MD))
            .py(px(LIST_ROW_PAD))
            .children(lead)
            .child(text)
            .children(right)
    }

    /// − value + for a number within `range`; `set` takes the new value.
    pub(super) fn number_stepper(
        &self,
        id: &str,
        value: i64,
        range: std::ops::RangeInclusive<i64>,
        cx: &mut Context<Self>,
        set: impl Fn(&mut Self, i64) + Clone + 'static,
    ) -> AnyElement {
        let this = cx.entity().downgrade();
        ui::stepper(
            SharedString::from(id.to_string()),
            value,
            range,
            move |to, _, cx| {
                let set = set.clone();
                let _ = this.update(cx, |this, cx| {
                    set(this, to);
                    cx.notify();
                });
            },
        )
        .into_any_element()
    }
}

/// The open server's port.
fn port_of(this: &Launcher) -> u16 {
    this.servers
        .setting(ServerKey::Port)
        .and_then(Value::as_u64)
        .unwrap_or(25565) as u16
}
