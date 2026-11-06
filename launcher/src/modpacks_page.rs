//! Imported modpacks, on the Mods page's Modpacks tab: import a pack, read its coverage report,
//! port what this computer can and play it as its own profile (`eclipse_launcher::modpacks`).
use super::*;
use crate::ui::{self, white};
use eclipse_launcher::browser::eclipse::pack as eclipse_pack;
use eclipse_launcher::modpacks::{self, Imported};
use eclipse_port_pack::report::Status;
use eclipse_theme::glass::{fill, ink, radius, step};
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

const NOTE_WIDTH: f32 = 720.;

const COUNT_MIN: f32 = 120.;

impl Launcher {
    fn open_pack(&self) -> Option<&Imported> {
        let slug = self.pack_open.as_ref()?;
        self.packs.iter().find(|p| &p.slug == slug)
    }

    /// Runs a pipeline step on a thread; its lines and its end arrive as messages.
    pub(super) fn pack_job(
        &mut self,
        job: impl FnOnce(&dyn Fn(String)) -> Result<Imported, String> + Send + 'static,
    ) {
        if self.pack_busy {
            return;
        }
        self.pack_busy = true;
        self.pack_log.clear();
        self.pack_error = None;
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
        self.pack_busy = false;
        match result {
            Ok(pack) => {
                self.mods.report_list.reset();
                self.pack_open = Some(pack.slug.clone());
                self.packs.retain(|p| p.slug != pack.slug);
                self.packs.push(*pack);
                self.packs.sort_by_key(|p| p.title().to_ascii_lowercase());
            }
            Err(e) => self.pack_error = Some(e),
        }
    }

    fn choose_pack(&mut self, folder: bool, cx: &mut Context<Self>) {
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
            .selected()
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
        let Some(tag) = pack
            .and_then(|i| self.version_of(i))
            .or_else(|| self.launcher_version())
            .cloned()
        else {
            return;
        };
        if !self.can_start(&tag) {
            return;
        }
        self.starting_pack = Some(work);
        self.start_client(tag);
    }

    /// Reads the imported packs, once (the sidebar and the Modpacks tab both show them).
    pub(super) fn load_packs(&mut self) {
        if self.packs_loaded.is_none() {
            self.mods.report_list.reset();
            self.packs = modpacks::list();
            self.registry_packs = eclipse_pack::list();
            self.reload_instances();
            if self.pack_open.is_none() {
                self.pack_open = self.packs.first().map(|p| p.slug.clone());
            }
            self.packs_loaded = Some(Instant::now());
        }
    }

    /// The imported packs, above the Modpacks tab's search.
    pub(super) fn imported_packs(&mut self, cx: &mut Context<Self>) -> Div {
        self.load_packs();
        let busy = self.pack_busy;
        let pick = |id: &'static str, label: &'static str, folder: bool| {
            let b = button(id, label, ButtonKind::Secondary);
            if busy {
                disabled(b)
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
            .gap(px(8.))
            .child(pick("import-file", "Import a pack file…", false))
            .child(pick("import-folder", "Import an instance folder…", true));
        let mut page = div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(ui::card_title(tr("Your modpacks")))
                    .child(ui::card_note(tr(
                        "A .mrpack, a CurseForge .zip, or a Prism, MultiMC or CurseForge instance.",
                    ))),
            )
            .child(actions);
        if busy || !self.pack_log.is_empty() {
            page = page.child(self.pack_log_plate());
        }
        if let Some(e) = &self.pack_error {
            page = page.child(ui::error_note(e.clone()));
        }
        if !self.registry_packs.is_empty() {
            page = page.child(self.registry_pack_rows(cx));
        }
        if self.packs.is_empty() && !self.registry_packs.is_empty() {
            return page;
        }
        if self.packs.is_empty() {
            return page.child(ui::empty().p_0().child(tr("No modpacks yet.")));
        }
        if self.packs.len() > 1 {
            let mut list = div().flex().flex_col();
            for p in self.packs.iter() {
                let open = self.pack_open.as_ref() == Some(&p.slug);
                let slug = p.slug.clone();
                list = list.child(
                    ui::row(SharedString::from(format!("open-{slug}")), open, 12.)
                        .child(ui::item_text(p.title().to_string(), p.summary()))
                        .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.pack_open = Some(slug.clone());
                            this.mods.report_list.reset();
                            cx.notify();
                        })),
                );
            }
            page = page.child(list);
        }
        match self.pack_detail(cx) {
            Some(detail) => page.child(detail),
            None => page,
        }
    }

    /// The modpacks installed from Eclipse's registry: each plays its own profile.
    fn registry_pack_rows(&self, cx: &mut Context<Self>) -> Div {
        let mut list = div().flex().flex_col();
        for p in self.registry_packs.iter() {
            let (play, remove) = (p.work.clone(), p.work.clone());
            let id = p.id.clone();
            let key = format!("eclipse:{id}");
            let confirming = self.confirm_remove.as_deref() == Some(key.as_str());
            let remove_id = SharedString::from(format!("remove-pack-{id}"));
            let right =
                if confirming {
                    div().flex_none().child(ui::confirm(
                        remove_id,
                        trf("Remove {name}?", &[("name", &p.name)]),
                        tr("Remove"),
                        cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.confirm_remove = None;
                            cx.notify();
                        }),
                        cx.listener(move |this, _: &ClickEvent, _, cx| {
                            match eclipse_pack::remove(&remove) {
                                Ok(()) => this.packs_loaded = None,
                                Err(e) => this.pack_error = Some(e),
                            }
                            this.confirm_remove = None;
                            cx.notify();
                        }),
                    ))
                } else {
                    div()
                        .flex()
                        .flex_none()
                        .gap(px(8.))
                        .child(
                            button(format!("play-pack-{id}"), "Play", ButtonKind::Primary)
                                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.play_pack(play.clone());
                                    cx.notify();
                                })),
                        )
                        .child(button(remove_id, "Remove…", ButtonKind::Ghost).on_press(
                            cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.confirm_remove = Some(key.clone());
                                cx.notify();
                            }),
                        ))
                };
            let detail = trf(
                "{version} from Eclipse's registry, its own profile: {mods}",
                &[("version", &p.version), ("mods", &p.mods.join(", "))],
            );
            list = list.child(
                ui::list_row(
                    SharedString::from(format!("registry-pack-{}", p.id)),
                    confirming,
                    |_, _, _| {},
                )
                .child(ui::item_text(p.name.clone(), detail))
                .child(right),
            );
        }
        list
    }

    fn pack_log_plate(&self) -> Div {
        let start = self.pack_log.len().saturating_sub(LOG_SHOWN);
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .p(px(ui::PANEL_INSET))
            .rounded(px(radius::ROW))
            .bg(white(fill::WELL))
            .child(
                ui::text(step::NOTE, FontWeight::MEDIUM, ink::BODY).child(ui::state_line(
                    if self.pack_busy {
                        ui::Tone::Busy
                    } else {
                        ui::Tone::Live
                    },
                    tr(if self.pack_busy { "Working" } else { "Done" }),
                )),
            )
            .children(self.pack_log.iter().skip(start).map(|line| {
                ui::text(step::CAPTION, FontWeight::NORMAL, ink::MUTED)
                    .font_family(theme::MONO)
                    .truncate()
                    .child(line.clone())
            }))
    }

    fn pack_detail(&self, cx: &mut Context<Self>) -> Option<Div> {
        let p = self.open_pack()?;
        let mut counts = div().flex().flex_wrap().gap(px(12.));
        for (status, label) in FILTERS.iter().skip(1) {
            let status = status.expect("the first filter is All");
            let color = match status {
                Status::Ported => ui::live(),
                Status::Partial => ui::color(ui::look().accent_soft),
                _ => white(ink::FULL),
            };
            counts = counts.child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(COUNT_MIN))
                    .gap(px(4.))
                    .p(px(ui::PANEL_INSET))
                    .rounded(px(radius::ROW))
                    .bg(white(fill::WELL))
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
        let mut buttons = div().flex().flex_wrap().items_center().gap(px(8.));
        let play_label = if p.mods.is_empty() {
            tr("Play").to_string()
        } else {
            trf(
                "Play with {count} mods",
                &[("count", &p.mods.len().to_string())],
            )
        };
        let mut play = button("pack-play", play_label, ButtonKind::Primary);
        if !p.can_play() || !can_play || self.pack_busy {
            play = disabled(play);
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
            format!("Get {} resource packs", p.indexed_resources)
        } else {
            format!(
                "Port {} mods + {} resource packs",
                p.portable, p.indexed_resources
            )
        };
        let mut port = button("pack-port", port_label, ButtonKind::Secondary);
        let mut from = button("pack-from", "Use jars from my instance…", ButtonKind::Ghost);
        if self.pack_busy || !p.can_port() {
            port = disabled(port);
            from = disabled(from);
        } else {
            port = port.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.port_pack(false, cx);
                cx.notify();
            }));
            if p.portable == 0 {
                from = disabled(from);
            } else {
                from = from.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.port_pack(true, cx);
                    cx.notify();
                }));
            }
        }
        let confirming = self.confirm_remove.as_deref() == Some(p.slug.as_str());
        let slug = p.slug.clone();
        let remove = if confirming {
            let work = p.work.clone();
            ui::confirm(
                "pack-remove",
                trf("Remove {name}?", &[("name", &p.pack.name)]),
                tr("Remove"),
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.confirm_remove = None;
                    cx.notify();
                }),
                cx.listener(move |this, _: &ClickEvent, _, cx| {
                    match modpacks::remove(&work) {
                        Ok(()) => {
                            this.packs.retain(|p| p.slug != slug);
                            this.pack_open = this.packs.first().map(|p| p.slug.clone());
                            this.mods.report_list.reset();
                        }
                        Err(e) => this.pack_error = Some(e),
                    }
                    this.confirm_remove = None;
                    cx.notify();
                }),
            )
            .into_any_element()
        } else {
            button("pack-remove", "Remove…", ButtonKind::Ghost)
                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.confirm_remove = Some(slug.clone());
                    cx.notify();
                }))
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
        notes.push(format!(
            "{} resource packs available. Enable them in the game's Resource Packs screen.",
            p.resourcepacks
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
            .filter(|r| self.pack_filter.is_none_or(|s| r.status == s))
            .collect();
        rows.sort_by(|a, b| (a.status, &a.key).cmp(&(b.status, &b.key)));
        let filter = ui::segments("pack-filter", &FILTERS, self.pack_filter, {
            let this = cx.entity().downgrade();
            move |value, _, cx| {
                let _ = this.update(cx, |this, cx| {
                    this.pack_filter = value;
                    this.mods.report_list.reset();
                    cx.notify();
                });
            }
        });
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
                .gap(px(2.))
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
            ui::list_row(
                SharedString::from(format!("pack-row-{i}")),
                false,
                |_, _, _| {},
            )
            .items_start()
            .child(
                text.child(ui::text(step::NOTE, FontWeight::NORMAL, ink::MUTED).child(technical)),
            )
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
                .gap(px(24.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
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
                        .gap(px(6.))
                        .max_w(px(NOTE_WIDTH))
                        .children(headline.into_iter().enumerate().map(|(i, line)| {
                            if i == 0 {
                                ui::text(step::BODY, FontWeight::NORMAL, ink::FULL).child(line)
                            } else {
                                ui::text(step::NOTE, FontWeight::NORMAL, ink::MUTED).child(line)
                            }
                        }))
                }))
                .child(counts)
                .child(buttons)
                .children(self.client_error.clone().map(ui::error_note))
                .children(notes.into_iter().map(|n| {
                    ui::text(step::NOTE, FontWeight::NORMAL, ink::MUTED)
                        .max_w(px(NOTE_WIDTH))
                        .child(n)
                }))
                .child(div().flex().child(filter))
                .child(list),
        )
    }
}
