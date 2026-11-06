//! The Servers page: the open server's card with its state, name, numbers, Start or Restart and
//! Stop, its console and command line; beside it the list of servers and how friends reach the
//! open one. Below, the server's Players, Settings and Backups.
use super::*;
use crate::ui::{self, white, Tone};
use eclipse_theme::glass::{ink, pixel_text, radius, space, step, TextStep};

/// The right column's width.
const SIDE: f32 = 320.;

const LIST_ROW_PAD: f32 = 10.;

const COMMAND_WIDTH: f32 = 520.;

/// The tabs under the server's card.
const LOWER_TABS: [(Tab, &str); 3] = [
    (Tab::Players, "Players"),
    (Tab::Settings, "Settings"),
    (Tab::Backups, "Backups"),
];

/// A dot's tone for a server's state.
fn tone(status: &Status) -> Tone {
    match status.phase.as_str() {
        "running" if status.crashes > 0 => Tone::Warn,
        "running" => Tone::Live,
        "stopped" => Tone::Off,
        _ => Tone::Busy,
    }
}

impl Launcher {
    pub(super) fn servers_view(&mut self, cx: &mut Context<Self>) -> Div {
        let head = ui::page_head(tr("Servers"), "");
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
        let addresses = self.server_addresses(&status, cx);
        let side = if beside {
            div()
                .flex()
                .flex_col()
                .flex_none()
                .w(px(SIDE))
                .gap(px(layout::CARD_GAP))
                .child(list)
                .child(addresses)
        } else {
            div()
                .flex()
                .items_start()
                .gap(px(layout::CARD_GAP))
                .child(list.flex_1().min_w_0())
                .child(addresses.flex_1().min_w_0())
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
            let mode = self.servers.shown("world", "game_mode");
            let mode = MODES
                .iter()
                .find(|(id, _)| *id == mode)
                .map_or(mode.clone(), |(_, label)| tr(label).to_string());
            let max = self.servers.shown("server", "max_players");
            [
                mode,
                self.servers.shown("world", "name"),
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

        let panel = ui::icon_button(
            "server-panel",
            "dashboard-square-01",
            tr("Server panel"),
            ButtonKind::Ghost,
            ui::ButtonSize::Sm,
            false,
        );
        let panel = if running {
            let d = dir.to_path_buf();
            panel.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                match servers::Api::open(&d).map(|a| a.panel_link()) {
                    Some(Ok(url)) => cx.open_url(&url),
                    Some(Err(e)) => this.servers.problem = Some(Problem::plain(e)),
                    None => {}
                }
                cx.notify();
            }))
        } else {
            ui::disabled(panel)
        };
        let folder = dir.to_path_buf();
        let folder = ui::icon_button(
            "server-folder",
            "folder-01",
            tr("Open folder"),
            ButtonKind::Ghost,
            ui::ButtonSize::Sm,
            false,
        )
        .on_press(move |_, _, cx| crate::toast::open(cx, &folder));

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
                    this.server_command("Restarting", vec!["restart".into()]);
                    cx.notify();
                }))
            } else {
                ui::disabled(restart)
            });
            let stop = ui::button(
                "server-stop",
                None,
                tr("Stop"),
                ButtonKind::Secondary,
                ui::ButtonSize::Lg,
            );
            buttons = buttons.child(if idle {
                stop.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.server_command("Stopping", vec!["stop".into()]);
                    cx.notify();
                }))
            } else {
                ui::disabled(stop)
            });
        } else if self.servers.busy.as_deref() == Some("Starting") {
            buttons = buttons.child(ui::primary_busy(
                "server-start",
                tr("Starting"),
                None,
                ui::ButtonSize::Lg,
            ));
        } else {
            let start = ui::primary("server-start", tr("Start"), ui::ButtonSize::Lg);
            buttons = buttons.child(if idle {
                start.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.start_server();
                    cx.notify();
                }))
            } else {
                ui::disabled(start)
            });
        }

        let mut card = ui::card(radius::PANEL)
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
                        div()
                            .flex()
                            .items_center()
                            .gap(px(space::SM))
                            .min_w_0()
                            .child(ui::dot(tone(&status)))
                            .child(
                                ui::text(step::NOTE, FontWeight::NORMAL, ink::BODY)
                                    .truncate()
                                    .child(state),
                            ),
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
        if let Some(label) = self.servers.busy.clone() {
            let last = self.servers.job_log.back().cloned().unwrap_or_default();
            out.push(
                div()
                    .relative()
                    .mt(px(space::LG))
                    .flex()
                    .flex_col()
                    .gap(px(space::SM))
                    .child(ui::progress_bar(None))
                    .child(
                        ui::card_note(format!("{} {last}", tr(&format!("{label}…")))).truncate(),
                    ),
            );
        }
        if let Some(p) = self.servers.problem.clone() {
            let mut actions = Vec::new();
            if let Some(fix) = p.fix {
                let label = match &fix {
                    Fix::UsePort(port) => trf("Use port {port}", &[("port", &port.to_string())]),
                    Fix::Prepare => tr("Prepare the game files").into(),
                    Fix::AgreeEula => tr("I agree to the EULA").into(),
                    Fix::ShowLog => tr("Open the logs").into(),
                };
                actions.push(
                    ui::button(
                        "problem-fix",
                        None,
                        label,
                        ButtonKind::Secondary,
                        ui::ButtonSize::Sm,
                    )
                    .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.fix(fix.clone(), cx);
                        cx.notify();
                    }))
                    .into_any_element(),
                );
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
            out.push(ui::notice_line(ui::live(), n).mt(px(space::LG)));
        }
        if running && !self.servers.restart_needed.is_empty() {
            let names: Vec<String> = self
                .servers
                .restart_needed
                .iter()
                .map(|k| tr(setting_name(k)).to_string())
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
                    ui::color(ui::look().accent_soft),
                    trf(
                        "Saved. Restart to apply: {settings}.",
                        &[("settings", &names.join(", "))],
                    ),
                )
                .mt(px(space::LG))
                .child(if idle {
                    restart.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.servers.restart_needed.clear();
                        this.server_command("Restarting", vec!["restart".into()]);
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
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::MD))
                    .p(px(ui::CARD_PADDING))
                    .rounded(px(radius::ROW))
                    .bg(ui::inset())
                    .child(ui::icon(
                        "computer-terminal-01",
                        ui::IconSize::Px16,
                        white(ink::MUTED),
                    ))
                    .child(
                        ui::text(step::BODY, FontWeight::NORMAL, ink::MUTED)
                            .child(tr("Start the server to see its console.")),
                    ),
            );
        }
        let log = ui::log_well("server-console", &self.servers.console_scroll);
        let log = if self.servers.log.is_empty() {
            log.flex()
                .items_center()
                .justify_center()
                .font_family(eclipse_theme::glass::FONT)
                .text_color(white(ink::MUTED))
                .child(tr("Waiting for the server's log…"))
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
            send.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.field_enter(Field::ServerCommand);
                cx.notify();
            }))
        } else {
            ui::disabled(send)
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
                COMMAND_WIDTH,
                ui::ButtonSize::Md,
                Some(send.into_any_element()),
                cx,
            )
            .w_full()
            .mt(px(space::SM))
            .pr(px(space::XS))
            .font_family(theme::MONO)
            .text_size(px(step::NOTE.size)),
        )
    }

    fn no_server(&mut self, cx: &mut Context<Self>) -> Div {
        let create = ui::primary(
            "server-create-empty",
            tr("Create a server"),
            ui::ButtonSize::Md,
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.open_create(cx);
            cx.notify();
        }));
        let add = ui::button(
            "server-import-empty",
            Some("folder-01"),
            tr("Import a server…"),
            ButtonKind::Secondary,
            ui::ButtonSize::Md,
        );
        let add = if self.servers.creating {
            ui::disabled(add)
        } else {
            add.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.add_server_folder(cx);
                cx.notify();
            }))
        };
        let problem = self
            .servers
            .problem
            .clone()
            .map(|p| ui::error_note(p.message).mt(px(space::LG)));
        ui::empty_card(
            ui::empty_state(
                "servers-empty",
                "computer",
                tr("No servers yet"),
                tr("Host a world your friends can join."),
                vec![create, add],
            )
            .children(problem),
        )
    }

    /// The servers, the open one lit, then Create a server and adding a server folder.
    fn server_list(&mut self, cx: &mut Context<Self>) -> Div {
        let mut list = ui::card(radius::PANEL)
            .flex()
            .flex_col()
            .p(px(ui::LIST_INSET));
        for e in self.servers.list.clone() {
            let status = self
                .servers
                .statuses
                .get(&e.dir)
                .cloned()
                .unwrap_or_else(Status::stopped);
            let current = self.servers.open.as_ref() == Some(&e.dir);
            let line = match status.phase.as_str() {
                "running" => trf(
                    "{online} of {max} online",
                    &[
                        ("online", &status.players.len().to_string()),
                        ("max", &status.max_players.unwrap_or(0).to_string()),
                    ],
                ),
                "stopped" => self.servers.stopped_line(&e.dir),
                _ => phase_label(&status),
            };
            let dir = e.dir.clone();
            list = list.child(
                ui::row(format!("server-row-{}", e.dir.display()), current)
                    .pr(px(space::MD))
                    .child(ui::item_text(e.name.clone(), line))
                    .child(ui::dot(tone(&status)).relative())
                    .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.servers.polled = None;
                        this.servers.problem = None;
                        this.servers.note = None;
                        this.open_server(dir.clone());
                        cx.notify();
                    })),
            );
        }
        let action = |id: &'static str, icon: &'static str, label: SharedString| {
            ui::row(id, false)
                .pr(px(space::MD))
                .child(ui::icon(icon, ui::IconSize::Px16, white(ink::BODY)).relative())
                .child(
                    ui::text(step::BODY, FontWeight::NORMAL, ink::BODY)
                        .relative()
                        .child(label),
                )
        };
        list = list.child(
            action("server-create", "plus-sign", tr("Create a server").into()).on_press(
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.open_create(cx);
                    cx.notify();
                }),
            ),
        );
        let add = action("server-import", "folder-01", tr("Import a server…").into());
        list.child(if self.servers.creating {
            ui::disabled(add)
        } else {
            add.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.add_server_folder(cx);
                cx.notify();
            }))
        })
    }

    /// How friends reach the open server: on this network, over the internet (Tailscale), and who
    /// is online now.
    fn server_addresses(&mut self, status: &Status, cx: &mut Context<Self>) -> Div {
        let port = port_of(self);
        let lan = match servers::lan_address() {
            Some(ip) if port == 25565 => ip.to_string(),
            Some(ip) => format!("{ip}:{port}"),
            None => format!("localhost:{port}"),
        };
        let copied = self
            .servers
            .copied
            .is_some_and(|at| at.elapsed() < Duration::from_secs(2));
        let copy = |id: &'static str, text: String, cx: &mut Context<Self>| {
            ui::icon_button(
                id,
                if copied { "tick-02" } else { "copy-01" },
                tr("Copy the address"),
                ButtonKind::Ghost,
                ui::ButtonSize::Md,
                false,
            )
            .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                clipboard::copy_to_clipboard(cx, gpui::ClipboardItem::new_string(text.clone()));
                this.servers.copied = Some(Instant::now());
                cx.notify();
            }))
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
        let here = div()
            .flex()
            .flex_col()
            .child(ui::label(tr("On this network")))
            .child(address(
                lan.clone(),
                step::FIGURE,
                copy("address-copy", lan, cx),
            ))
            .child(
                ui::card_note(tr("Friends on the same Wi-Fi can join with this address."))
                    .mt(px(space::XS)),
            );
        let internet = self.internet_card(port, cx);
        let mut card = ui::card(radius::PANEL)
            .flex()
            .flex_col()
            .gap(px(space::XL))
            .p(px(ui::CARD_PADDING))
            .child(here.relative())
            .child(internet.relative());
        if status.running() && !status.players.is_empty() {
            let ops = self
                .servers
                .open
                .as_deref()
                .map(|d| servers::names(d, "ops.json"))
                .unwrap_or_default();
            let mut people = div().mt(px(space::MD)).flex().flex_col().gap(px(space::MD));
            for p in status.players.clone() {
                let op = ops.iter().any(|o| o.eq_ignore_ascii_case(&p.name));
                people = people.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(space::MD))
                        .child(self.head(&p.uuid, 28.))
                        .child(
                            ui::text(step::BODY, FontWeight::NORMAL, ink::FULL)
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child(p.name.clone()),
                        )
                        .child(ui::card_note(if op {
                            tr("Operator")
                        } else {
                            tr("Player")
                        })),
                );
            }
            card = card.child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .child(ui::label(tr("Online now")))
                    .child(people),
            );
        }
        card
    }

    /// The server's Players, Settings and Backups, on a card under it.
    fn server_tabs(&mut self, dir: &Path, status: &Status, cx: &mut Context<Self>) -> Div {
        let strip = ui::tab_strip(
            "server-tabs",
            LOWER_TABS.iter().map(|&(tab, label)| {
                ui::tab(
                    SharedString::from(format!("server-tab-{tab:?}")),
                    tr(label),
                    self.servers.tab == tab,
                )
                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.servers.tab = tab;
                    this.asking = None;
                    cx.notify();
                }))
            }),
        );
        let players = self.servers.tab == Tab::Players;
        let body = match self.servers.tab {
            Tab::Settings => self.settings_tab(status, cx),
            Tab::Backups => self.backups_tab(status, cx),
            Tab::Players => self.players_tab(dir, status, cx),
        };
        ui::card(radius::PANEL)
            .mt(px(layout::CARD_GAP))
            .flex()
            .flex_col()
            .p(px(ui::CARD_PADDING))
            .child(strip.relative())
            .child(
                div()
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
        .setting("server", "port")
        .and_then(Value::as_u64)
        .unwrap_or(25565) as u16
}
