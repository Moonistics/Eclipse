//! Settings' AI rows: the Show AI features switch (`eclipse_core::no_ai`, shared with the game)
//! and "Connect an AI agent": a one-click add where the client has a CLI (run only on that click),
//! else its ready-made MCP config with a Copy button, and a test that starts the MCP server and
//! lists its tools. With AI features hidden, only the switch shows.
use super::*;
use crate::menus::Menu;
use crate::ui;
use eclipse_launcher::agents;
use eclipse_theme::glass::{ink, space, step};

#[derive(Default)]
pub(crate) struct AgentsUi {
    client: Option<&'static str>,
    /// Which Copy button just copied.
    copied: Option<&'static str>,
    /// The add or test under way, by its button.
    running: Option<&'static str>,
    /// What the last add or test said, and whether it went well.
    status: Option<(bool, String)>,
    add_failed: bool,
}

impl Launcher {
    /// The Show AI features row: always shown, so they can be shown again.
    pub(crate) fn ai_features_row(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let shown = !backend::no_ai();
        ui::toggle_row("ai-features", tr("Show AI features"), None, shown).on_press(cx.listener(
            move |this, _: &ClickEvent, _, cx| {
                this.agents.status = backend::set_no_ai(shown).err().map(|e| (false, e));
                cx.notify();
            },
        ))
    }

    /// "Connect an AI agent"; None while AI features are hidden.
    pub(crate) fn agents_section(&mut self, cx: &mut Context<Self>) -> Option<Div> {
        if backend::no_ai() {
            return None;
        }
        let exe = &backend::update_target();
        let clients = agents::clients(exe);
        let client = clients
            .iter()
            .find(|c| Some(c.id) == self.agents.client)
            .unwrap_or(&clients[0]);
        let running = self.agents.running;
        let open = self.menu_is(&Menu::AgentClient);
        let picker = ui::dropdown(
            "agent-client",
            tr("Connect an AI agent"),
            None,
            tr(client.name),
            open,
            ui::ButtonSize::Md,
        );
        let mut picker = div()
            .relative()
            .child(self.menu_opener(picker, Menu::AgentClient, cx));
        if open {
            let mut menu = self.menu_popover("agent-client-menu", cx);
            for c in &clients {
                let id = c.id;
                menu = menu.child(
                    ui::pick_row(format!("agent-client-{id}"), tr(c.name), id == client.id)
                        .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.agents.client = Some(id);
                            this.agents.copied = None;
                            this.agents.status = None;
                            this.agents.add_failed = false;
                            this.close_menu();
                            cx.notify();
                        })),
                );
            }
            picker = picker.child(ui::menu_under(
                ui::ButtonSize::Md.height(),
                ui::Align::Start,
                menu,
            ));
        }
        let copy = |id: &'static str, label: &'static str, text: String, cx: &mut Context<Self>| {
            let copied = self.agents.copied == Some(id);
            ui::button(
                id,
                Some("copy-01"),
                tr(if copied { "Copied" } else { label }),
                ButtonKind::Secondary,
                ui::ButtonSize::Md,
            )
            .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                clipboard::copy_to_clipboard(cx, gpui::ClipboardItem::new_string(text.clone()));
                this.agents.copied = Some(id);
                cx.notify();
            }))
        };
        let mut steps = div()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(space::MD))
            .child(picker);
        let mut actions = div().flex().flex_wrap().gap(px(space::SM));
        if let Some((program, args)) = client.add.clone() {
            let label = trf("Add to {client}", &[("client", client.name)]);
            let add = ui::button(
                "agent-add",
                None,
                if running == Some("agent-add") {
                    tr("Running…").to_owned()
                } else {
                    label
                },
                ButtonKind::Secondary,
                ui::ButtonSize::Md,
            );
            let add = if running.is_some() {
                ui::disabled(add)
            } else {
                add.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.agents.running = Some("agent-add");
                    this.agents.status = None;
                    let (program, args) = (program, args.clone());
                    let task = cx
                        .background_executor()
                        .spawn(async move { run_add(program, &args) });
                    cx.spawn(async move |this, cx| {
                        let result = task.await;
                        let _ = this.update(cx, |this, cx| {
                            this.agents.running = None;
                            this.agents.add_failed = !result.0;
                            this.agents.status = Some(result);
                            cx.notify();
                        });
                    })
                    .detach();
                    cx.notify();
                }))
            };
            actions = actions.child(add);
        }
        if client.add.is_none() || self.agents.add_failed {
            steps = steps
                .child(ui::card_note(trf(
                    "Put this in {file}:",
                    &[("file", tr(client.file))],
                )))
                .child(ui::code_well(client.config.clone()).w_full());
            actions = actions.child(copy(
                "agent-copy-config",
                "Copy config",
                client.config.clone(),
                cx,
            ));
        }
        let test = ui::button(
            "agent-test",
            None,
            tr(if running == Some("agent-test") {
                "Testing…"
            } else {
                "Test the connection"
            }),
            ButtonKind::Secondary,
            ui::ButtonSize::Md,
        );
        let test = if running.is_some() {
            ui::disabled(test)
        } else {
            test.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.agents.running = Some("agent-test");
                this.agents.status = None;
                let exe = backend::update_target();
                let task = cx.background_executor().spawn(async move {
                    match agents::self_test(&exe) {
                        Ok(n) => (true, trf("The MCP server answered with {count} tools. Now ask your agent: \"Say hello from Eclipse\".", &[("count", &n.to_string())])),
                        Err(e) => (false, trf("The MCP server did not answer: {error}", &[("error", &e)])),
                    }
                });
                cx.spawn(async move |this, cx| {
                    let result = task.await;
                    let _ = this.update(cx, |this, cx| {
                        this.agents.running = None;
                        this.agents.status = Some(result);
                        cx.notify();
                    });
                })
                .detach();
                cx.notify();
            }))
        };
        steps = steps.child(actions.child(test));
        if let Some((ok, message)) = &self.agents.status {
            steps = steps.child(if *ok {
                ui::text(step::NOTE, FontWeight::NORMAL, ink::FULL).child(message.clone())
            } else {
                ui::error_note(message.clone())
            });
        }
        steps = steps.child(
            ui::link("agent-docs", tr("What agents can do"))
                .on_press(|_, _, cx| cx.open_url(backend::AGENT_DOCS_URL)),
        );
        Some(ui::setting_row_stacked(
            tr("Connect an AI agent"),
            Some(
                div()
                    .child(tr("Lets your AI agent control the launcher and the game."))
                    .into_any_element(),
            ),
            steps.into_any_element(),
        ))
    }
}

/// Runs a client's add command; (went well, what it said).
fn run_add(program: &str, args: &[String]) -> (bool, String) {
    match eclipse_launcher::process::command(agents::locate(program))
        .args(args)
        .output()
    {
        Ok(out) if out.status.success() => (
            true,
            trf(
                "Added. Restart {program} if it is open, then ask it: \"Say hello from Eclipse\".",
                &[("program", program)],
            ),
        ),
        Ok(out) => (
            false,
            trf(
                "{program} said: {output}",
                &[
                    ("program", program),
                    (
                        "output",
                        String::from_utf8_lossy(if out.stderr.is_empty() {
                            &out.stdout
                        } else {
                            &out.stderr
                        })
                        .trim(),
                    ),
                ],
            ),
        ),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (
            false,
            trf(
                "{program} isn't installed. Copy the config instead.",
                &[("program", program)],
            ),
        ),
        Err(e) => (
            false,
            trf(
                "Could not run {program}: {error}",
                &[("program", program), ("error", &e.to_string())],
            ),
        ),
    }
}
