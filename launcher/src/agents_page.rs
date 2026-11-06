//! Settings' AI rows: the Show AI features switch (`eclipse_core::no_ai`, shared with the game)
//! and "Connect an AI agent", the ready-made MCP config of each common client with Copy buttons,
//! a one-click add where the client has a CLI (run only on that click, its command shown first),
//! and a test that starts the MCP server and lists its tools. With AI features hidden, only the
//! switch shows.
use super::*;
use crate::glass_ui as ui;
use eclipse_launcher::agents;
use eclipse_theme::glass::{ink, radius, step};

#[derive(Default)]
pub(crate) struct AgentsUi {
    /// The client shown, an index into `agents::clients`.
    client: usize,
    /// Which Copy button just copied.
    copied: Option<&'static str>,
    /// A command or test running.
    busy: bool,
    /// What the last add or test said, and whether it went well.
    status: Option<(bool, String)>,
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
        let pick = self.agents.client.min(clients.len() - 1);
        let client = &clients[pick];
        let this = cx.entity().downgrade();
        let options: Vec<(usize, &'static str)> = clients
            .iter()
            .enumerate()
            .map(|(i, c)| (i, c.name))
            .collect();
        let picker = ui::segments("agent-client", &options, pick, move |i, _, cx| {
            let _ = this.update(cx, |this, cx| {
                this.agents.client = i;
                this.agents.copied = None;
                this.agents.status = None;
                cx.notify();
            });
        });
        let copy = |id: &'static str, text: String, cx: &mut Context<Self>| {
            let copied = self.agents.copied == Some(id);
            ui::button(
                id,
                Some("copy-01"),
                tr(if copied { "Copied" } else { "Copy" }),
                Kind::Secondary,
                ui::Size::Md,
            )
            .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                copy_to_clipboard(cx, gpui::ClipboardItem::new_string(text.clone()));
                this.agents.copied = Some(id);
                cx.notify();
            }))
        };
        let say = |s: String| ui::card_note(s);
        let code = |s: &str| {
            div()
                .p(px(12.))
                .rounded(px(radius::ROW))
                .bg(ui::well())
                .font_family(theme::MONO)
                .text_size(px(step::CAPTION.size))
                .line_height(px(step::CAPTION.line))
                .text_color(ui::white(ink::FULL))
                .children(s.lines().map(|l| div().child(l.to_owned())))
        };
        let mut steps = div()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(12.))
            .child(picker);
        if let Some((program, args)) = client.add.clone() {
            let line = client.add_line().unwrap_or_default();
            let label = trf("Add to {client}", &[("client", client.name)]);
            let busy = self.agents.busy;
            let add = ui::primary(
                "agent-add",
                if busy {
                    tr("Running…").to_owned()
                } else {
                    label
                },
                ui::Size::Md,
            )
            .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                if this.agents.busy {
                    return;
                }
                this.agents.busy = true;
                this.agents.status = None;
                let (program, args) = (program, args.clone());
                let task = cx
                    .background_executor()
                    .spawn(async move { run_add(program, &args) });
                cx.spawn(async move |this, cx| {
                    let result = task.await;
                    let _ = this.update(cx, |this, cx| {
                        this.agents.busy = false;
                        this.agents.status = Some(result);
                        cx.notify();
                    });
                })
                .detach();
                cx.notify();
            }));
            steps = steps
                .child(say(trf(
                    "{client} adds it with this command:",
                    &[("client", client.name)],
                )))
                .child(code(&line).w_full())
                .child(div().flex().gap(px(8.)).child(add).child(copy(
                    "agent-copy-command",
                    line.clone(),
                    cx,
                )));
        }
        let test = ui::button(
            "agent-test",
            None,
            tr("Test the connection"),
            Kind::Secondary,
            ui::Size::Md,
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            if this.agents.busy {
                return;
            }
            this.agents.busy = true;
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
                    this.agents.busy = false;
                    this.agents.status = Some(result);
                    cx.notify();
                });
            })
            .detach();
            cx.notify();
        }));
        steps = steps
            .child(say(trf(
                if client.add.is_some() {
                    "Or put this in {file}:"
                } else {
                    "Put this in {file}:"
                },
                &[("file", tr(client.file))],
            )))
            .child(code(&client.config).w_full())
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(copy("agent-copy-config", client.config.clone(), cx))
                    .child(test),
            );
        if let Some((ok, message)) = &self.agents.status {
            steps = steps.child(
                ui::text(step::NOTE, FontWeight::NORMAL, ink::FULL)
                    .when(!*ok, |d| d.text_color(ui::danger()))
                    .child(message.clone()),
            );
        }
        steps = steps.child(
            ui::link("agent-docs", tr("Agent commands"))
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
