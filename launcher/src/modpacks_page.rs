//! Imported modpacks, on the Mods page's Modpacks tab: import a pack, read its coverage report,
//! port what this computer can and play it as its own profile (`eclipse_launcher::modpacks`).
use super::*;
use crate::menus::Asking;
use crate::ui::{self, white};
use eclipse_launcher::browser::eclipse::pack as eclipse_pack;
use eclipse_launcher::modpacks::{self, Imported};
use eclipse_port_pack::report::Status;
use eclipse_theme::glass::{ink, space, step};
use std::path::PathBuf;

/// The report's filter: every row, or one status.
pub(super) const FILTERS: [(Option<Status>, &str); 5] = [
    (None, "All"),
    (Some(Status::Ported), "Ported"),
    (Some(Status::Partial), "Partial"),
    (Some(Status::NotYet), "Not yet"),
    (Some(Status::NotNeeded), "Not needed"),
];

/// Lines of the pipeline's log the page shows while it works.
const LOG_SHOWN: usize = 6;

const COUNT_MIN: f32 = 120.;

#[derive(Default)]
pub(super) struct Modpacks {
    pub(super) imported: Vec<Imported>,
    pub(super) registry: Vec<eclipse_pack::Installed>,
    pub(super) loaded: Option<Instant>,
    pub(super) open: Option<String>,
    pub(super) filter: Option<Status>,
    pub(super) busy: bool,
    pub(super) log: VecDeque<String>,
    pub(super) error: Option<String>,
}

impl Launcher {
    fn open_pack(&self) -> Option<&Imported> {
        let slug = self.modpacks.open.as_ref()?;
        self.modpacks.imported.iter().find(|p| &p.slug == slug)
    }

    /// Runs a pipeline step on a thread; its lines and its end arrive as messages.
    pub(super) fn pack_job(
        &mut self,
        job: impl FnOnce(&dyn Fn(String)) -> Result<Imported, String> + Send + 'static,
    ) {
        if self.modpacks.busy {
            return;
        }
        self.modpacks.busy = true;
        self.modpacks.log.clear();
        self.modpacks.error = None;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let lines = tx.clone();
            let log = move |line: String| {
                let _ = lines.send(Msg::PackLine(line));
            };
            let result = job(&log);
            let _ = tx.send(Msg::PackDone(result.map(Box::new)));
        });
    }

    /// A finished step: the pack's new state, opened.
    pub(super) fn pack_done(&mut self, result: Result<Box<Imported>, String>) {
        self.modpacks.busy = false;
        match result {
            Ok(pack) => {
                self.mods.report_list.reset();
                self.modpacks.open = Some(pack.slug.clone());
                self.modpacks.imported.retain(|p| p.slug != pack.slug);
                self.modpacks.imported.push(*pack);
                self.modpacks
                    .imported
                    .sort_by_key(|p| p.title().to_ascii_lowercase());
            }
            Err(e) => self.modpacks.error = Some(e),
        }
    }

    pub(super) fn choose_pack(&mut self, folder: bool, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: !folder,
            directories: folder,
            multiple: false,
            prompt: Some(tr("Import").into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(mut paths))) = paths.await {
                if let Some(path) = paths.pop() {
                    let _ = this.update(cx, |this, cx| {
                        this.pack_job(move |log| modpacks::import(&path, log));
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    /// Ports the open pack; `from` is a folder of the player's jars, chosen when `ask`.
    fn port_pack(&mut self, ask: bool, cx: &mut Context<Self>) {
        let Some(work) = self.open_pack().map(|p| p.work.clone()) else {
            return;
        };
        let resources = self
            .selected_version()
            .filter(|tag| backend::is_prepared(tag))
            .map(|tag| backend::resources(tag));
        if !ask {
            self.pack_job(move |log| modpacks::port(&work, resources.as_deref(), None, log));
            return;
        }
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(tr("Use these jars").into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(mut paths))) = paths.await {
                if let Some(from) = paths.pop() {
                    let _ = this.update(cx, |this, cx| {
                        this.pack_job(move |log| {
                            modpacks::port(&work, resources.as_deref(), Some(&from), log)
                        });
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn play_pack(&mut self, work: PathBuf) {
        // The pack's instance plays its own version, if it has one.
        let pack = self.instances.list.iter().find(|i| i.work == work);
        let Some(tag) = pack.and_then(|i| self.version_of(i)).cloned() else {
            return;
        };
        if !self.can_start(&tag) {
            return;
        }
        self.game.pack = Some(work);
        self.start_client(tag);
    }

    /// Reads the imported packs, once (the sidebar and the Modpacks tab both show them).
    pub(super) fn load_packs(&mut self) {
        if self.modpacks.loaded.is_none() {
            self.mods.report_list.reset();
            self.modpacks.imported = modpacks::list();
            self.modpacks.registry = eclipse_pack::list();
            self.reload_instances();
            if self.modpacks.open.is_none() {
                self.modpacks.open = self.modpacks.imported.first().map(|p| p.slug.clone());
            }
            self.modpacks.loaded = Some(Instant::now());
        }
    }

    /// The imported packs, above the Modpacks tab's search.
    pub(super) fn imported_packs(&mut self, cx: &mut Context<Self>) -> Div {
        self.load_packs();
        let busy = self.modpacks.busy;
        let pick = |id: &'static str, label: &'static str, folder: bool| {
            let b = ui::button(
                id,
                Some("folder-01"),
                if busy { tr("Importing…") } else { label },
                ButtonKind::Secondary,
                ui::ButtonSize::Md,
            );
            if busy {
                ui::disabled(b)
            } else {
                b.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.choose_pack(folder, cx);
                    cx.notify();
                }))
            }
        };
        let actions = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(space::SM))
            .child(pick("import-file", tr("Import a modpack…"), false))
            .child(pick(
                "import-folder",
                tr("Import an instance folder…"),
                true,
            ));
        let none = self.modpacks.imported.is_empty() && self.modpacks.registry.is_empty();
        let mut page = div()
            .flex()
            .flex_col()
            .gap(px(space::LG))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(ui::TITLE_NOTE_GAP))
                    .child(ui::section_title("modpacks-title", tr("Your modpacks")))
                    .when(none, |d| {
                        d.child(ui::empty_line("modpacks-none", tr("No modpacks yet.")))
                    }),
            )
            .child(actions);
        if busy || !self.modpacks.log.is_empty() {
            page = page.child(self.pack_log_plate());
        }
        if let Some(e) = &self.modpacks.error {
            page = page.child(ui::error_note(e.clone()));
        }
        if !self.modpacks.registry.is_empty() {
            page = page.child(self.registry_pack_rows(cx));
        }
        if self.modpacks.imported.is_empty() {
            return page;
        }
        if self.modpacks.imported.len() > 1 {
            let rows = self
                .modpacks
                .imported
                .iter()
                .map(|p| {
                    let open = self.modpacks.open.as_ref() == Some(&p.slug);
                    let row = ui::row(format!("open-{}", p.slug), p.title().to_string(), open)
                        .pr(px(space::MD))
                        .child(ui::item_text(p.title().to_string(), p.summary()));
                    (row, SharedString::from(p.title().to_string()), open)
                })
                .collect();
            let slugs: Vec<String> = self
                .modpacks
                .imported
                .iter()
                .map(|p| p.slug.clone())
                .collect();
            let this = cx.entity().downgrade();
            page = page.child(ui::options(
                div().id("modpacks-imported").flex().flex_col(),
                tr("Your modpacks"),
                rows,
                true,
                move |i, _, cx| {
                    let _ = this.update(cx, |this, cx| {
                        this.modpacks.open = slugs.get(i).cloned();
                        this.mods.report_list.reset();
                        cx.notify();
                    });
                },
            ));
        }
        match self.pack_detail(cx) {
            Some(detail) => page.child(detail),
            None => page,
        }
    }

    /// The modpacks installed from Eclipse's registry: each plays its own profile.
    fn registry_pack_rows(&self, cx: &mut Context<Self>) -> Div {
        let mut list = div().flex().flex_col();
        for p in self.modpacks.registry.iter() {
            let play = p.work.clone();
            let id = p.id.clone();
            let key = Asking::RegistryPack(id.clone());
            let confirming = self.is_asking(&key);
            let remove_id = SharedString::from(format!("remove-pack-{id}"));
            let right = if confirming {
                let gone = format!("pack:{id}");
                div().flex_none().child(ui::confirm(
                    remove_id,
                    ui::Confirm::Delete(&p.name),
                    cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.asking = None;
                        cx.notify();
                    }),
                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.delete_pack(&gone);
                        cx.notify();
                    }),
                ))
            } else {
                div()
                    .flex()
                    .flex_none()
                    .gap(px(space::SM))
                    .child(
                        ui::button(
                            format!("play-pack-{id}"),
                            None,
                            tr("Play"),
                            ButtonKind::Secondary,
                            ui::ButtonSize::Md,
                        )
                        .on_press(cx.listener(
                            move |this, _: &ClickEvent, _, cx| {
                                this.play_pack(play.clone());
                                cx.notify();
                            },
                        )),
                    )
                    .child(self.pack_delete_opener(remove_id, true, key, cx))
            };
            let detail = trf(
                "{version} from Eclipse · {mods}",
                &[("version", &p.version), ("mods", &p.mods.join(", "))],
            );
            list = list.child(
                ui::list_row(
                    SharedString::from(format!("registry-pack-{}", p.id)),
                    confirming,
                )
                .child(ui::item_text(p.name.clone(), detail))
                .child(right),
            );
        }
        list
    }

    fn pack_delete_opener(
        &self,
        id: SharedString,
        bin: bool,
        key: Asking,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let (label, size) = (tr("Delete…"), ui::ButtonSize::Md);
        if self.game.active() {
            let why = tr("Close the game first");
            return match bin {
                true => ui::remove_button_unavailable(id, label, size, why),
                false => ui::unavailable(
                    ui::button(id, None, label, ButtonKind::Secondary, size),
                    why,
                ),
            };
        }
        let opener = match bin {
            true => ui::remove_button(id, label, size),
            false => ui::button(id, None, label, ButtonKind::Secondary, size),
        };
        opener.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.asking = Some(key.clone());
            cx.notify();
        }))
    }

    fn delete_pack(&mut self, instance: &str) {
        self.asking = None;
        self.delete_instance(instance);
        self.modpacks.open = None;
        self.modpacks.loaded = None;
        self.mods.report_list.reset();
    }

    fn pack_log_plate(&self) -> Div {
        let start = self.modpacks.log.len().saturating_sub(LOG_SHOWN);
        ui::well()
            .flex()
            .flex_col()
            .gap(px(space::XS))
            .child(
                ui::text(step::NOTE, FontWeight::MEDIUM, ink::BODY).child(ui::state_line(
                    if self.modpacks.busy {
                        ui::Tone::Busy
                    } else {
                        ui::Tone::Live
                    },
                    tr(if self.modpacks.busy {
                        "Working"
                    } else {
                        "Done"
                    }),
                )),
            )
            .children(self.modpacks.log.iter().skip(start).map(|line| {
                ui::mono(step::CAPTION, ink::MUTED)
                    .truncate()
                    .child(line.clone())
            }))
    }

    fn pack_detail(&self, cx: &mut Context<Self>) -> Option<Div> {
        let p = self.open_pack()?;
        let mut counts = div().flex().flex_wrap().gap(px(space::MD));
        for (status, label) in FILTERS.iter().skip(1) {
            let status = status.expect("the first filter is All");
            let color = match status {
                Status::Ported => ui::live(),
                Status::Partial => ui::color(ui::look().accent_soft),
                _ => white(ink::FULL),
            };
            counts = counts.child(
                ui::well()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(COUNT_MIN))
                    .gap(px(space::XS))
                    .child(
                        ui::text(step::FIGURE, FontWeight::SEMIBOLD, ink::FULL)
                            .text_color(color)
                            .child(p.count(status).to_string()),
                    )
                    .child(ui::label(tr(label))),
            );
        }
        // Prepared, nothing preparing or running (`play_pack` checks the same).
        let can_play = self.can_start_selected();
        let mut buttons = div().flex().flex_wrap().items_center().gap(px(space::SM));
        let play_label = if p.mods.is_empty() {
            tr("Play").to_string()
        } else {
            trf(
                "Play with {count} mods",
                &[("count", &p.mods.len().to_string())],
            )
        };
        let mut play = ui::button(
            "pack-play",
            None,
            play_label,
            ButtonKind::Secondary,
            ui::ButtonSize::Md,
        );
        if self.modpacks.busy {
            play = ui::disabled(play);
        } else if !p.can_play() {
            play = ui::unavailable(play, tr("Port its mods first"));
        } else if !can_play {
            play = ui::unavailable(play, self.why_not_start());
        } else {
            let work = p.work.clone();
            play = play.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.play_pack(work.clone());
                cx.notify();
            }));
        }
        let port_label = if p.indexed_resources == 0 {
            trf("Port {count} mods", &[("count", &p.portable.to_string())])
        } else if p.portable == 0 {
            trf(
                "Get {count} resource packs",
                &[("count", &p.indexed_resources.to_string())],
            )
        } else {
            trf(
                "Port {mods} mods + {packs} resource packs",
                &[
                    ("mods", &p.portable.to_string()),
                    ("packs", &p.indexed_resources.to_string()),
                ],
            )
        };
        let mut port = ui::button(
            "pack-port",
            None,
            port_label,
            ButtonKind::Secondary,
            ui::ButtonSize::Md,
        );
        let mut from = ui::button(
            "pack-from",
            None,
            tr("Use jars from my instance…"),
            ButtonKind::Secondary,
            ui::ButtonSize::Md,
        );
        if self.modpacks.busy {
            port = ui::disabled(port);
            from = ui::disabled(from);
        } else if !p.can_port() {
            let why = tr("Nothing in this pack can be ported yet");
            port = ui::unavailable(port, why);
            from = ui::unavailable(from, why);
        } else {
            port = port.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.port_pack(false, cx);
                cx.notify();
            }));
            if p.portable == 0 {
                from = ui::unavailable(from, tr("None of its mods can be ported yet"));
            } else {
                from = from.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.port_pack(true, cx);
                    cx.notify();
                }));
            }
        }
        let key = Asking::PortedPack(p.slug.clone());
        let remove = if self.is_asking(&key) {
            let gone = format!("port:{}", p.slug);
            ui::confirm(
                "pack-remove",
                ui::Confirm::Delete(&p.pack.name),
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.asking = None;
                    cx.notify();
                }),
                cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.delete_pack(&gone);
                    cx.notify();
                }),
            )
            .into_any_element()
        } else {
            self.pack_delete_opener("pack-remove".into(), false, key, cx)
                .into_any_element()
        };
        buttons = buttons.child(play).child(port).child(from).child(remove);
        let mut notes = vec![trf(
            "Port gets the jars of the {count} mods this computer can port today (content with data \
             and assets only) from the pack's own links, Modrinth, or your folder, and carries \
             them over. The ports stay here, in {folder}.",
            &[
                ("count", &p.portable.to_string()),
                ("folder", &short(&p.work)),
            ],
        )];
        notes.push(trf(
            "{count} resource packs available. Enable them in the game's Resource Packs screen.",
            &[("count", &p.resourcepacks.to_string())],
        ));
        if let Some(jars) = p.jars() {
            notes.push(jars);
        }
        for line in &p.left_off {
            notes.push(trf("Left off: {line}.", &[("line", line)]));
        }
        if !can_play {
            notes.push(tr("Prepare an Eclipse version first.").into());
        }
        let mut rows: Vec<&eclipse_port_pack::report::Row> = p
            .rows
            .iter()
            .filter(|r| self.modpacks.filter.is_none_or(|s| r.status == s))
            .collect();
        rows.sort_by(|a, b| (a.status, &a.key).cmp(&(b.status, &b.key)));
        let filter = ui::segments(
            "pack-filter",
            tr("Port status"),
            &ui::translated(&FILTERS),
            self.modpacks.filter,
            {
                let this = cx.entity().downgrade();
                move |value, _, cx| {
                    let _ = this.update(cx, |this, cx| {
                        this.modpacks.filter = value;
                        this.mods.report_list.reset();
                        cx.notify();
                    });
                }
            },
        );
        let row = |i: usize| {
            let r = rows[i];
            let (tone, status_label) = match r.status {
                Status::Ported => (ui::Tone::Live, tr("Ported")),
                Status::Partial => (ui::Tone::Accent(ui::look().accent_soft), tr("Partial")),
                Status::NotYet => (ui::Tone::Warn, tr("Not yet")),
                Status::NotNeeded => (ui::Tone::Off, tr("Not needed")),
            };
            let name = if r.name.is_empty() || r.name == r.key {
                r.key.clone()
            } else {
                format!("{}  ({})", r.name, r.key)
            };
            // What it means in plain words, then the technical terms for modders. A report an
            // older build wrote has no plain words.
            let technical = format!("{} · {} · {}: {}", r.class, r.tier, r.license, r.reason);
            let mut text = div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .gap(px(space::XXS))
                .child(
                    ui::text(step::BODY, FontWeight::SEMIBOLD, ink::FULL)
                        .truncate()
                        .child(name),
                );
            if !r.plain.is_empty() {
                text = text.child(
                    ui::text(step::NOTE, FontWeight::NORMAL, ink::BODY)
                        .child(modpacks::row_plain(r)),
                );
            }
            ui::list_row(SharedString::from(format!("pack-row-{}", r.key)), false)
                .items_start()
                .child(text.child(ui::card_note(technical)))
                .child(
                    ui::text(step::NOTE, FontWeight::MEDIUM, ink::BODY)
                        .flex_none()
                        .child(ui::state_line(tone, status_label)),
                )
        };
        let headline = p.headline();
        let list = self.mods.report_list.column(
            &self.content_scroll.handle,
            rows.len(),
            0.,
            |_| false,
            row,
        );
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(space::XL))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(space::XS))
                        .child(ui::entity_title(
                            p.title().to_string(),
                            eclipse_theme::glass::pixel_text::CARD_TITLE,
                        ))
                        .child(ui::card_note(p.summary())),
                )
                .children((!headline.is_empty()).then(|| {
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(space::SM))
                        .max_w(px(layout::MEASURE_TEXT))
                        .children(headline.into_iter().enumerate().map(|(i, line)| {
                            if i == 0 {
                                ui::text(step::BODY, FontWeight::NORMAL, ink::FULL).child(line)
                            } else {
                                ui::card_note(line)
                            }
                        }))
                }))
                .child(counts)
                .child(buttons)
                .children(
                    self.game
                        .problem
                        .as_ref()
                        .and_then(|p| p.failure())
                        .map(ui::error_note),
                )
                .children(
                    notes
                        .into_iter()
                        .map(|n| ui::card_note(n).max_w(px(layout::MEASURE_TEXT))),
                )
                .child(div().flex().child(filter))
                .child(list),
        )
    }
}
