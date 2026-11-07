use super::*;

impl Launcher {
    pub(super) fn players_tab(
        &mut self,
        dir: &Path,
        status: &Status,
        cx: &mut Context<Self>,
    ) -> Div {
        let running = status.running();
        let idle = self.servers.busy.is_none();
        let stopped = (!running).then(|| SharedString::from(tr("Start the server first")));
        let ops = servers::names(dir, "ops.json");
        let whitelist = servers::entries(dir, "whitelist.json");
        let banned = servers::entries(dir, "banned-players.json");
        let is_op = |n: &str| ops.iter().any(|o| o.eq_ignore_ascii_case(n));
        let mut tab = div().flex().flex_col().gap(px(space::XL));

        // Online now.
        let mut online = div().flex().flex_col();
        for p in status.players.clone() {
            let op = is_op(&p.name);
            let (n1, n2, n3) = (p.name.clone(), p.name.clone(), p.name.clone());
            let actions = div()
                .flex()
                .flex_none()
                .gap(px(space::SM))
                .child(self.player_action(
                    format!("online-op-{}", p.name),
                    if op {
                        tr("Remove operator")
                    } else {
                        tr("Make operator")
                    },
                    idle,
                    None,
                    cx,
                    move |this| {
                        let verb = if op { "deop" } else { "op" };
                        this.server_command(
                            ServerJob::ChangingOperators,
                            vec![verb.into(), n1.clone()],
                        );
                    },
                ))
                .child(self.player_action(
                    format!("online-kick-{}", p.name),
                    tr("Kick"),
                    true,
                    stopped.clone(),
                    cx,
                    move |this| this.api_command(format!("kick {n2}")),
                ))
                .child(self.player_action(
                    format!("online-ban-{}", p.name),
                    tr("Ban"),
                    true,
                    stopped.clone(),
                    cx,
                    move |this| this.api_command(format!("ban {n3}")),
                ));
            let ping = p
                .ping
                .map(|ms| trf("{ms} ms ping", &[("ms", &ms.to_string())]));
            let about = match (op, ping) {
                (true, Some(ping)) => Some(trf("Operator · {ping}", &[("ping", &ping)])),
                (true, None) => Some(tr("Operator").to_string()),
                (false, ping) => ping,
            };
            let face = self.player_face(&p.uuid, FACE);
            online = online.child(self.static_row(
                Some(face),
                p.name.clone(),
                about,
                Some(actions.into_any_element()),
            ));
        }
        tab = tab.child(ui::field_group(
            "server-online-title",
            trf(
                "Online now ({count})",
                &[("count", &status.players.len().to_string())],
            ),
            None,
            if status.players.is_empty() {
                ui::empty_line(
                    "server-players-none",
                    if running {
                        tr("Nobody is playing right now.")
                    } else {
                        tr("The server is stopped.")
                    },
                )
                .into_any_element()
            } else {
                online.into_any_element()
            },
        ));

        // Who can join.
        let friends_only = self
            .servers
            .setting(ServerKey::Whitelist)
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let friends = ui::toggle_row(
            "friends-only",
            tr("Friends only"),
            Some(
                if friends_only {
                    tr("On: only players on the list can join.")
                } else {
                    tr("Off: anyone who has the address can join.")
                }
                .into(),
            ),
            friends_only,
        )
        .mx(px(-ui::SETTING_ROW_PADDING))
        .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.set_setting(ServerKey::Whitelist, (!friends_only).to_string());
            cx.notify();
        }));
        let can_add = if idle { CanAdd::Yes } else { CanAdd::Busy };
        let add = self.name_adder(Field::ServerPlayer, "player", layout::FIELD_MD, can_add, cx);
        let mut people: Vec<(String, String)> = whitelist.clone();
        for op in &ops {
            if !people.iter().any(|(n, _)| n.eq_ignore_ascii_case(op)) {
                people.push((op.clone(), String::new()));
            }
        }
        let mut allowed = div().flex().flex_col();
        for (name, uuid) in people {
            let op = is_op(&name);
            let listed = whitelist.iter().any(|(n, _)| n == &name);
            let (n1, n2) = (name.clone(), name.clone());
            let mut actions =
                div()
                    .flex()
                    .flex_none()
                    .gap(px(space::SM))
                    .child(self.player_action(
                        format!("allowed-op-{name}"),
                        if op {
                            tr("Remove operator")
                        } else {
                            tr("Make operator")
                        },
                        idle,
                        None,
                        cx,
                        move |this| {
                            let verb = if op { "deop" } else { "op" };
                            this.server_command(
                                ServerJob::ChangingOperators,
                                vec![verb.into(), n1.clone()],
                            );
                        },
                    ));
            let asking = Asking::AllowedPlayer(name.clone());
            let remove_id = SharedString::from(format!("allowed-remove-{name}"));
            if listed && self.is_asking(&asking) {
                actions = div().flex_none().child(ui::confirm(
                    remove_id,
                    ui::Confirm::Remove(&name),
                    cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.asking = None;
                        cx.notify();
                    }),
                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.asking = None;
                        this.server_command(
                            ServerJob::UpdatingList,
                            vec!["whitelist".into(), "remove".into(), n2.clone()],
                        );
                        cx.notify();
                    }),
                ));
            } else if listed {
                let remove = ui::remove_button(remove_id, tr("Remove…"), ui::ButtonSize::Sm);
                actions = actions.child(match idle {
                    true => remove.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.asking = Some(asking.clone());
                        cx.notify();
                    })),
                    false => ui::disabled(remove),
                });
            }
            let face = if uuid.is_empty() {
                div().size(px(FACE)).flex_none()
            } else {
                self.player_face(&uuid, FACE)
            };
            let about = match (op, listed) {
                (true, true) => Some(tr("Operator: can run every command").to_string()),
                (true, false) => {
                    Some(tr("Operator (not on the list: can join anyway)").to_string())
                }
                _ => None,
            };
            allowed = allowed.child(self.static_row(
                Some(face),
                name,
                about,
                Some(actions.into_any_element()),
            ));
        }
        let refused = self
            .fields
            .invalid
            .and_then(|(f, e)| (f == Field::ServerPlayer).then(|| ui::error_note(tr(e))));
        tab = tab.child(ui::field_group(
            "server-join-title",
            tr("Who can join"),
            None,
            div()
                .flex()
                .flex_col()
                .gap(px(space::MD))
                .child(friends)
                .child(add)
                .children(refused)
                .child(allowed),
        ));

        // Banned.
        if !banned.is_empty() {
            let mut list = div().flex().flex_col();
            for (name, uuid) in banned {
                let n = name.clone();
                let unban = self.player_action(
                    format!("unban-{name}"),
                    tr("Unban"),
                    true,
                    stopped.clone(),
                    cx,
                    move |this| this.api_command(format!("pardon {n}")),
                );
                let face = if uuid.is_empty() {
                    div().size(px(FACE)).flex_none()
                } else {
                    self.player_face(&uuid, FACE)
                };
                list = list.child(self.static_row(
                    Some(face),
                    name,
                    None,
                    Some(unban.into_any_element()),
                ));
            }
            tab = tab.child(ui::field_group(
                "server-banned-title",
                tr("Banned"),
                None,
                div()
                    .flex()
                    .flex_col()
                    .gap(px(space::MD))
                    .when(!running, |d| {
                        d.child(ui::card_note(tr("Start the server to unban someone.")))
                    })
                    .child(list),
            ));
        }
        tab
    }

    /// A row of the settings form: a name, one line on it, and its control.
    pub(super) fn server_setting_row(
        &self,
        name: impl Into<SharedString>,
        about: String,
        control: AnyElement,
    ) -> Div {
        self.setting_row_checked(name, about, None, control)
    }

    /// A setting row whose text gives way to `error` when the value was refused.
    pub(super) fn setting_row_checked(
        &self,
        name: impl Into<SharedString>,
        about: String,
        error: Option<SharedString>,
        control: AnyElement,
    ) -> Div {
        let note = match error {
            Some(e) => ui::error_note(e),
            None => div().child(about),
        };
        let name: SharedString = name.into();
        ui::setting_row(name, Some(note.into_any_element()), Some(control))
    }

    pub(super) fn server_switch_row(
        &self,
        id: &'static str,
        name: impl Into<SharedString>,
        about: String,
        on: bool,
        press: Option<impl Fn(&ClickEvent, &mut Window, &mut App) + 'static>,
        why: Option<SharedString>,
    ) -> AnyElement {
        let Some(press) = press else {
            let switch = match why {
                Some(why) => ui::unavailable(ui::toggle(id, on), why),
                None => ui::disabled(ui::toggle(id, on)),
            };
            return self
                .server_setting_row(name, about, switch.into_any_element())
                .into_any_element();
        };
        ui::toggle_row(id, name, Some(about.into()), on)
            .on_press(press)
            .into_any_element()
    }

    /// − value + for a number setting within `range`.
    pub(super) fn server_stepper(
        &self,
        key: ServerKey,
        range: std::ops::RangeInclusive<i64>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let value: i64 = self.servers.shown(key).parse().unwrap_or(0);
        self.number_stepper(&key.dotted(), value, range, cx, move |this, to| {
            this.set_setting(key, to.to_string())
        })
    }

    pub(super) fn server_segments(
        &self,
        id: &'static str,
        name: &str,
        key: ServerKey,
        options: Vec<(&'static str, SharedString)>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let current = self.servers.shown(key);
        let current: &'static str = options
            .iter()
            .find(|(v, _)| *v == current)
            .map_or("", |(v, _)| v);
        let this = cx.entity().downgrade();
        ui::segments(
            id,
            name.to_string(),
            &options,
            current,
            move |value, _, cx| {
                let _ = this.update(cx, |this, cx| {
                    this.set_setting(key, value.to_string());
                    cx.notify();
                });
            },
        )
        .into_any_element()
    }

    pub(super) fn settings_tab(&mut self, status: &Status, cx: &mut Context<Self>) -> Div {
        let running = status.running();
        if self.servers.config.is_none() {
            return div().child(if self.older_eclipse() {
                ui::empty_line(
                    "server-settings-too-old",
                    tr(
                        "This server's Eclipse is too old to show its settings here. Update it on \
                     the Backups tab, then they appear.",
                    ),
                )
                .into_any_element()
            } else {
                ui::loading_state("server-settings-loading", tr("Reading the settings…"))
                    .into_any_element()
            });
        }
        if !self.fields.has_focus(Field::ServerMotd) && !self.fields.holds(Field::ServerMotd) {
            let motd = self.servers.shown(ServerKey::Motd);
            self.fields.text(Field::ServerMotd).set(motd);
        }
        if !self.fields.has_focus(Field::ServerPort) && !self.fields.holds(Field::ServerPort) {
            let port = self.servers.shown(ServerKey::Port);
            self.fields.text(Field::ServerPort).set(port);
        }
        let save = |id: &'static str, f: Field, cx: &mut Context<Self>| {
            ui::button(
                id,
                None,
                tr("Save"),
                ButtonKind::Secondary,
                ui::ButtonSize::Md,
            )
            .on_press(cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.field_enter(f, window, cx);
                cx.notify();
            }))
        };
        let motd = div()
            .flex()
            .items_center()
            .gap(px(space::SM))
            .child(self.field(
                Field::ServerMotd,
                tr("A Minecraft Server"),
                layout::FIELD_MD,
                ui::ButtonSize::Md,
                cx,
            ))
            .child(save("motd-save", Field::ServerMotd, cx))
            .into_any_element();
        let port = div()
            .flex()
            .items_center()
            .gap(px(space::SM))
            .child(self.field(
                Field::ServerPort,
                "25565",
                layout::FIELD_XS,
                ui::ButtonSize::Md,
                cx,
            ))
            .child(save("port-save", Field::ServerPort, cx))
            .into_any_element();
        let pvp_known = self.servers.pvp.filter(|_| running);
        let pvp = self.server_switch_row(
            "pvp",
            tr("Players can hurt each other (PvP)"),
            if running {
                tr("Applies at once.").into()
            } else {
                tr("A rule of the world: start the server to change it.").into()
            },
            pvp_known.unwrap_or(true),
            pvp_known.map(|on| {
                cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.servers.pvp = Some(!on);
                    this.api_command(format!("gamerule pvp {}", !on));
                    cx.notify();
                })
            }),
            (!running).then(|| tr("Start the server first").into()),
        );
        let accounts = self
            .servers
            .setting(ServerKey::OnlineMode)
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let live = if running {
            tr("Applies at once.")
        } else {
            tr("Used from the next start.")
        };
        let later = if running {
            tr("Saved now, used after a restart.")
        } else {
            tr("Used from the next start.")
        };
        let accounts = self.server_switch_row(
            "online-mode",
            tr("Require Microsoft sign-in"),
            if accounts && plays_offline(&self.accounts) {
                format!(
                    "{} {later} {}",
                    tr("On: players must own Minecraft and sign in with Microsoft."),
                    tr("You play offline, so you couldn't join it.")
                )
            } else if accounts {
                format!(
                    "{} {later}",
                    tr("On: players must own Minecraft and sign in with Microsoft.")
                )
            } else {
                format!(
                    "{} {later}",
                    tr("Off: anyone can play under any name, so keep Friends only on.")
                )
            },
            accounts,
            Some(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.set_setting(ServerKey::OnlineMode, (!accounts).to_string());
                cx.notify();
            })),
            None,
        );
        div()
            .flex()
            .flex_col()
            .mx(px(-ui::SETTING_ROW_PADDING))
            .child(self.server_setting_row(tr("Message in the server list"), later.into(), motd))
            .child(self.server_setting_row(
                tr("Game mode"),
                format!("{} {live}", tr("For players who join for the first time.")),
                self.server_segments(
                    "set-mode",
                    tr("Game mode"),
                    ServerKey::GameMode,
                    GameMode::options(),
                    cx,
                ),
            ))
            .child(self.server_setting_row(
                tr("Difficulty"),
                live.into(),
                self.server_segments(
                    "set-difficulty",
                    tr("Difficulty"),
                    ServerKey::Difficulty,
                    ui::translated(&DIFFICULTIES),
                    cx,
                ),
            ))
            .child(pvp)
            .child(self.server_setting_row(
                tr("Max players"),
                format!("{} {later}", tr("Operators can always join.")),
                self.server_stepper(ServerKey::MaxPlayers, 1..=100, cx),
            ))
            .child(self.server_setting_row(
                tr("View distance"),
                format!("{} {later}", tr("In chunks, 2 to 32. Lower runs lighter.")),
                self.server_stepper(ServerKey::ViewDistance, 2..=32, cx),
            ))
            .child(self.server_setting_row(
                tr("Simulation distance"),
                format!(
                    "{} {later}",
                    tr("How far from players crops grow and mobs move, in chunks.")
                ),
                self.server_stepper(ServerKey::SimulationDistance, 2..=32, cx),
            ))
            .child(
                self.setting_row_checked(
                    tr("Port"),
                    format!(
                        "{} {later}",
                        tr("1024 to 65535. Friends type it after the address unless it's 25565.")
                    ),
                    self.fields
                        .invalid
                        .and_then(|(f, e)| (f == Field::ServerPort).then(|| tr(e).into())),
                    port,
                ),
            )
            .child(accounts)
            .child(self.background_row(cx))
            .child(self.delete_server_section(status, cx))
    }

    pub(super) fn delete_server_section(&mut self, status: &Status, cx: &mut Context<Self>) -> Div {
        let name = self
            .servers
            .list
            .iter()
            .find(|entry| Some(&entry.dir) == self.servers.open.as_ref())
            .map(|entry| entry.name.clone())
            .unwrap_or_default();
        let ready = !status.running()
            && self.servers.busy.is_none()
            && !self.servers.applying
            && self.servers.queued.is_empty()
            && self.servers.service == Some(false);
        let blocked = if status.running() {
            Some(tr("Stop the server before deleting it."))
        } else if self.servers.service == Some(true) {
            Some(tr("Turn off running in the background before deleting it."))
        } else {
            None
        };
        let hint =
            blocked.or_else(|| (!ready).then(|| tr("Wait for the current operation to finish.")));
        let mut controls = div()
            .flex()
            .flex_col()
            .items_end()
            .flex_none()
            .w(px(DELETE_CONTROLS))
            .gap(px(space::MD));
        if self.is_asking(&Asking::ServerDelete) && ready {
            controls = controls.child(ui::confirm(
                "server-delete",
                ui::Confirm::Delete(&name),
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.asking = None;
                    cx.notify();
                }),
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.asking = None;
                    if let Some(dir) = this.servers.open.clone() {
                        this.servers.busy = Some(ServerJob::Trashing);
                        let tx = this.servers.tx.clone();
                        std::thread::spawn(move || {
                            let result = servers::delete(&dir);
                            let _ = tx.send(Event::Deleted { dir, result });
                        });
                    }
                    cx.notify();
                }),
            ));
        } else {
            let delete = ui::button(
                "server-delete",
                None,
                tr("Delete server…"),
                ButtonKind::Secondary,
                ui::ButtonSize::Md,
            );
            let delete = match blocked {
                Some(why) => ui::unavailable(delete, why),
                None if ready => delete.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.asking = Some(Asking::ServerDelete);
                    cx.notify();
                })),
                None => ui::disabled(delete),
            };
            controls = controls.child(delete);
        }
        if let Some(hint) = hint {
            controls = controls.child(
                ui::card_note(hint)
                    .w(px(DELETE_CONTROLS))
                    .text_align(gpui::TextAlign::Right),
            );
        }
        self.server_setting_row(
            tr("Delete server"),
            tr("Remove this server from Eclipse and move its worlds, settings and backups to the trash.").into(),
            controls.into_any_element(),
        )
        .items_start()
    }

    pub(super) fn background_row(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let on = self.servers.service.unwrap_or(false);
        let idle = self.servers.busy.is_none() && self.servers.service.is_some();
        self.server_switch_row(
            "background",
            tr("Run in the background and at login"),
            if on {
                tr("On: starts at login and keeps running when the launcher closes.").into()
            } else {
                tr("Off: the server stops when you close the launcher.").into()
            },
            on,
            idle.then(|| {
                cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.server_command(
                        ServerJob::RunInBackground(!on),
                        vec![
                            "service".into(),
                            if on { "uninstall" } else { "install" }.into(),
                        ],
                    );
                    cx.notify();
                })
            }),
            None,
        )
    }

    pub(super) fn updates_section(&mut self, status: &Status, cx: &mut Context<Self>) -> Div {
        let idle = self.servers.busy.is_none();
        let check = self.servers.update.clone();
        let release = status
            .release
            .clone()
            .or_else(|| {
                check
                    .as_ref()
                    .and_then(|u| u["installed"].as_str().map(str::to_string))
            })
            .or_else(servers::runtime_release)
            .unwrap_or_else(|| "?".into());
        let available = status.update_available.clone().or_else(|| {
            check
                .as_ref()
                .filter(|u| u["available"] == true)
                .and_then(|u| u["newest"].as_str().map(str::to_string))
        });
        let about = match (&available, &check, &status.update_last) {
            (Some(tag), _, _) => trf(
                "Update available: Eclipse {tag} (this server runs {release}). Updating backs up \
                 the world first, and goes back by itself if the new version doesn't start.",
                &[("tag", tag), ("release", &release)],
            ),
            (None, Some(_), _) => trf("Up to date: Eclipse {release}.", &[("release", &release)]),
            (None, None, Some((last, _))) => format!("Eclipse {release}. {last}"),
            (None, None, None) => format!("Eclipse {release}."),
        };
        let control = match available {
            Some(tag) => {
                let b = ui::button(
                    "update-now",
                    None,
                    tr("Update now"),
                    ButtonKind::Secondary,
                    ui::ButtonSize::Md,
                );
                if idle {
                    b.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.servers.update = None;
                        if let Some(dir) = this.servers.open.clone() {
                            let done =
                                format!("Updated to Eclipse {tag}. The world was backed up first.");
                            this.server_job(ServerJob::UpdatingTo(tag.clone()), move |log| {
                                servers::run_streamed(&dir, &["update"], log).map(|_| done)
                            });
                        }
                        cx.notify();
                    }))
                } else {
                    ui::disabled(b)
                }
            }
            None if self.servers.checking => ui::disabled(ui::button(
                "update-check",
                None,
                tr("Checking…"),
                ButtonKind::Secondary,
                ui::ButtonSize::Md,
            )),
            None if !idle => ui::disabled(ui::button(
                "update-check",
                None,
                tr("Check for updates"),
                ButtonKind::Secondary,
                ui::ButtonSize::Md,
            )),
            None => ui::button(
                "update-check",
                None,
                tr("Check for updates"),
                ButtonKind::Secondary,
                ui::ButtonSize::Md,
            )
            .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                if let Some(dir) = this.servers.open.clone() {
                    this.servers.checking = true;
                    let tx = this.servers.tx.clone();
                    std::thread::spawn(move || {
                        let result =
                            servers::run(&dir, &["update", "--check", "--json"]).and_then(|out| {
                                serde_json::from_str(out.trim())
                                    .map_err(|e| Problem::plain(e.to_string()))
                            });
                        let _ = tx.send(Event::UpdateCheck(result));
                    });
                }
                cx.notify();
            })),
        };
        let mut rows = div()
            .flex()
            .flex_col()
            .mx(px(-ui::SETTING_ROW_PADDING))
            .child(self.server_setting_row(
                tr("Eclipse version"),
                about,
                control.into_any_element(),
            ));
        if self.servers.setting(ServerKey::UpdatesKeep).is_some() {
            rows = rows.child(self.server_setting_row(
                tr("Older versions kept"),
                tr("Downloaded versions kept besides the one in use and the one an update can go back \
                 to.")
                    .into(),
                self.server_stepper(ServerKey::UpdatesKeep, 0..=5, cx),
            ));
        }
        ui::field_group("server-updates-title", tr("Updates"), None, rows)
    }

    /// The server's details came back without its settings: its eclipse-server predates
    /// `config` (and `backups --json`).
    pub(super) fn older_eclipse(&self) -> bool {
        self.servers.service.is_some() && self.servers.config.is_none()
    }

    pub(super) fn backups_tab(&mut self, status: &Status, cx: &mut Context<Self>) -> Div {
        let idle = self.servers.busy.is_none();
        let every = self.servers.shown(ServerKey::BackupsEvery);
        let auto = !every.is_empty() && every != "0";
        let keep = self.servers.shown(ServerKey::BackupsKeep);
        let now = {
            let b = ui::button(
                "backup-now",
                None,
                tr("Back up now"),
                ButtonKind::Secondary,
                ui::ButtonSize::Md,
            );
            if idle {
                b.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.server_command(ServerJob::BackingUp, vec!["backup".into()]);
                    cx.notify();
                }))
            } else {
                ui::disabled(b)
            }
        };
        let mut list = div().flex().flex_col();
        for b in self.servers.backups.clone() {
            let (when, kind) = backup_label(&b);
            let confirming = self.is_asking(&Asking::ServerRestore(b.name.clone()));
            let name = b.name.clone();
            let restored = trf(
                "The world is back as it was on {when}. The one it replaced is in the backups folder.",
                &[("when", &when)],
            );
            let restore_id = SharedString::from(format!("restore-{}", b.name));
            let right = if confirming && idle {
                ui::confirm(
                    restore_id,
                    ui::Confirm::Restore(&when),
                    cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.asking = None;
                        cx.notify();
                    }),
                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.asking = None;
                        if let Some(dir) = this.servers.open.clone() {
                            let (name, restored) = (name.clone(), restored.clone());
                            this.server_job(ServerJob::Restoring, move |log| {
                                servers::run_streamed(&dir, &["restore", &name, "--yes"], log)
                                    .map(|_| restored)
                            });
                        }
                        cx.notify();
                    }),
                )
            } else {
                let restore = ui::button(
                    restore_id,
                    None,
                    tr("Restore…"),
                    ButtonKind::Secondary,
                    ui::ButtonSize::Md,
                );
                let restore = if idle {
                    restore.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.asking = Some(Asking::ServerRestore(name.clone()));
                        cx.notify();
                    }))
                } else {
                    ui::disabled(restore)
                };
                div().flex().flex_none().items_center().child(restore)
            };
            let detail = if kind.is_empty() {
                bytes(b.bytes)
            } else {
                format!("{kind} · {}", bytes(b.bytes))
            };
            list = list.child(self.static_row(
                None,
                when,
                Some(detail),
                Some(right.into_any_element()),
            ));
        }
        let mut tab = div()
            .flex()
            .flex_col()
            .gap(px(space::XL))
            .child(div().flex().items_center().gap(px(space::LG)).child(now));
        if self.older_eclipse() {
            return tab
                .child(ui::empty_line(
                    "server-backups-too-old",
                    tr(
                        "This server's Eclipse is too old to list its backups here: they're in its \
                     backups folder. Update it below.",
                    ),
                ))
                .child(self.updates_section(status, cx));
        }
        tab = tab.child(
            ui::toggle_row(
                "auto-backup",
                tr("Back up automatically"),
                Some(
                    if auto {
                        trf(
                            "On: every {every} while the server runs, keeping the last {keep}.",
                            &[("every", &every), ("keep", &keep)],
                        )
                    } else {
                        tr("Off: only the backups you make.").to_string()
                    }
                    .into(),
                ),
                auto,
            )
            .mx(px(-ui::SETTING_ROW_PADDING))
            .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.set_setting(
                    ServerKey::BackupsEvery,
                    if auto { "0".into() } else { "6h".into() },
                );
                cx.notify();
            })),
        );
        if matches!(self.asking, Some(Asking::ServerRestore(_))) {
            tab = tab.child(ui::notice_line(
                ui::Tone::Warn,
                tr("The current world moves to the backups folder, and a running server restarts."),
            ));
        }
        tab = if self.servers.backups.is_empty() {
            tab.child(ui::empty_line("server-backups-none", tr("No backup yet.")))
        } else {
            tab.child(list)
        };
        tab.child(self.updates_section(status, cx))
    }
}
