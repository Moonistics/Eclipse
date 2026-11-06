//! Export… on Home's Instance row: the selected instance as a `.mrpack` or a `.zip`, with the
//! worlds the player switches on, written to the launcher's `exports/` folder
//! (`eclipse_launcher::export`).
use super::*;
use crate::ui;
use eclipse_launcher::export::{self, Format};
use eclipse_theme::glass::{ink, radius, space, step};
use std::path::PathBuf;

pub(super) struct ExportPanel {
    /// The instance it is for: picking another closes it.
    instance: String,
    format: Format,
    /// The instance's worlds (folder, name) and whether each goes in.
    worlds: Vec<(String, String, bool)>,
    busy: bool,
    /// The file written, or why not.
    done: Option<Result<(String, PathBuf), String>>,
}

impl ExportPanel {}

impl Launcher {
    pub(super) fn toggle_export(&mut self) {
        if self.export.take().is_some() {
            return;
        }
        let Some(i) = self.selected_instance() else {
            return;
        };
        let worlds = eclipse_launcher::worlds::list(&i.home)
            .into_iter()
            .map(|w| (w.folder, w.name, false))
            .collect();
        self.export = Some(ExportPanel {
            instance: i.id.clone(),
            format: Format::Mrpack,
            worlds,
            busy: false,
            done: None,
        });
    }

    fn run_export(&mut self, cx: &mut Context<Self>) {
        let Some(i) = self.selected_instance().cloned() else {
            return;
        };
        let Some(panel) = self.export.as_mut().filter(|p| !p.busy) else {
            return;
        };
        panel.busy = true;
        panel.done = None;
        let format = panel.format;
        let worlds: Vec<String> = panel
            .worlds
            .iter()
            .filter(|w| w.2)
            .map(|w| w.0.clone())
            .collect();
        let task = cx.background_executor().spawn(async move {
            let linked = match format {
                Format::Mrpack => export::linkable(&i.home, &export::from_modrinth),
                Format::Zip => Vec::new(),
            };
            let out = export::free_path(&backend::data_dir().join("exports"), &i.name, format);
            export::export(&i.home, &i.name, format, &worlds, &linked, &out).map(|done| {
                let linked = match done.linked {
                    0 => String::new(),
                    n => trf(
                        ", {count} listed by their Modrinth download",
                        &[("count", &n.to_string())],
                    ),
                };
                let file = done.path.file_name().unwrap_or_default().to_string_lossy();
                (
                    trf(
                        "Exported {file}: {count} files{linked}.",
                        &[
                            ("file", &file),
                            ("count", &done.copied.to_string()),
                            ("linked", &linked),
                        ],
                    ),
                    done.path,
                )
            })
        });
        cx.spawn(async move |this, cx| {
            let done = task.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(panel) = &mut this.export {
                    panel.busy = false;
                    panel.done = Some(done);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// The Export card on Home, while it is open.
    pub(super) fn export_panel(&mut self, cx: &mut Context<Self>) -> Option<Div> {
        let selected = self.selected_instance()?.id.clone();
        if self.export.as_ref()?.instance != selected {
            self.export = None;
            return None;
        }
        let panel = self.export.as_ref()?;
        let this = cx.entity().downgrade();
        let pick = ui::segments(
            "export-format",
            &ui::translated(&[
                (Format::Mrpack, "Modrinth pack (.mrpack)"),
                (Format::Zip, "Zip file (.zip)"),
            ]),
            panel.format,
            move |format, _, cx| {
                let _ = this.update(cx, |this, cx| {
                    if let Some(p) = &mut this.export {
                        p.format = format;
                    }
                    cx.notify();
                });
            },
        );
        let mut worlds = div().flex().flex_col().gap(px(space::MD));
        if panel.worlds.is_empty() {
            worlds = worlds.child(
                ui::text(step::NOTE, FontWeight::NORMAL, ink::MUTED)
                    .child(tr("No worlds to put in.")),
            );
        }
        for (folder, name, on) in &panel.worlds {
            let key = folder.clone();
            worlds = worlds.child(
                ui::toggle_label(
                    SharedString::from(format!("export-world-{folder}")),
                    name.clone(),
                    *on,
                )
                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    let picked = this
                        .export
                        .as_mut()
                        .and_then(|p| p.worlds.iter_mut().find(|(folder, _, _)| *folder == key));
                    if let Some(w) = picked {
                        w.2 = !w.2;
                    }
                    cx.notify();
                })),
            );
        }
        let go = ui::button(
            "export-run",
            None,
            if panel.busy {
                tr("Exporting…")
            } else {
                tr("Export")
            },
            ButtonKind::Secondary,
            ui::ButtonSize::Md,
        );
        let go = match panel.busy {
            true => ui::disabled(go),
            false => go.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.run_export(cx);
                cx.notify();
            })),
        };
        let result = match &panel.done {
            Some(Ok((said, path))) => {
                let path = path.clone();
                Some(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(px(space::MD))
                        .child(
                            ui::text(step::NOTE, FontWeight::NORMAL, ink::BODY).child(said.clone()),
                        )
                        .child(
                            ui::button(
                                "export-show",
                                None,
                                tr("Show in folder"),
                                ButtonKind::Secondary,
                                ui::ButtonSize::Md,
                            )
                            .on_press(move |_, _, cx| cx.reveal_path(&path)),
                        ),
                )
            }
            Some(Err(e)) => Some(ui::error_note(e.clone())),
            None => None,
        };
        let close = ui::icon_button(
            "export-close",
            "cancel-01",
            tr("Close"),
            ButtonKind::Ghost,
            ui::ButtonSize::Sm,
            false,
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.export = None;
            cx.notify();
        }));
        Some(
            ui::card(radius::PANEL)
                .flex()
                .flex_col()
                .gap(px(space::LG))
                .p(px(ui::CARD_PADDING))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(ui::section_title(tr("Export to share")))
                        .child(close),
                )
                .child(ui::labelled_field(
                    tr("File type"),
                    div().flex().child(pick),
                ))
                .child(ui::labelled_field(tr("Worlds"), worlds))
                .child(div().flex().child(go))
                .children(result),
        )
    }
}
