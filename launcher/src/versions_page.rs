//! The Eclipse versions page: each Nightly on its own card (its name, Newest on the first, when
//! it came out and which instances play it), Install or Use and a bin on the right; the space
//! versions take, Import a version from file, the native and imported builds, install progress
//! and errors, the confirmation before a removal.

use super::*;
use crate::menus::Asking;
use crate::ui::{self, Tone};
use eclipse_theme::glass::{radius, space};

const PROGRESS_WIDTH: f32 = 180.;

fn problem_line(id: String, error: &str) -> Div {
    let copy = error
        .trim_end()
        .contains('\n')
        .then(|| ui::copy_details(format!("{id}-copy"), error.to_owned(), ui::ButtonSize::Sm));
    ui::error_line(
        ui::plain_error(error).to_string(),
        copy.into_iter()
            .map(IntoElement::into_any_element)
            .collect(),
    )
}

impl Launcher {
    /// The Versions page; `on_disk` and `used` as `versions_page` measured them.
    pub(super) fn versions_view(
        &self,
        on_disk: &dyn Fn(&String) -> Option<u64>,
        used: u64,
        cx: &mut Context<Self>,
    ) -> Div {
        let import: AnyElement = if self.importing_build {
            self.install_progress(None, tr("Importing…"))
                .into_any_element()
        } else {
            ui::button(
                "import-build",
                None,
                tr("Import a version from file…"),
                ButtonKind::Secondary,
                ui::ButtonSize::Md,
            )
            .on_press(cx.listener(|this, _: &ClickEvent, _, cx| this.import_build(cx)))
            .into_any_element()
        };
        let head = div()
            .flex()
            .flex_wrap()
            .items_end()
            .justify_between()
            .gap(px(space::LG))
            .mb(px(ui::page_gap()))
            .child(ui::page_head(tr("Eclipse versions"), "").mb_0())
            .child(import);
        let mut page = div().flex().flex_col().child(head).when(used > 0, |d| {
            d.child(
                ui::card_note(trf(
                    "Installed versions use {size}.",
                    &[("size", &megabytes(used))],
                ))
                .mb(px(space::MD)),
            )
        });
        page = page
            .children(
                self.import_error
                    .clone()
                    .map(|e| problem_line("import-error".into(), &e).mb(px(space::MD))),
            )
            .children(self.local_builds(
                "Native mod builds",
                native_mods::is_native,
                None,
                on_disk,
                cx,
            ))
            .children(self.local_builds(
                "Imported builds",
                build_import::is_imported,
                Some(tr("Not checked by Eclipse. Only use builds you trust.")),
                on_disk,
                cx,
            ));
        let releases = match &self.releases {
            Load::Loading => {
                return page.child(ui::empty_card(ui::loading_state(
                    "releases-loading",
                    tr("Loading versions…"),
                )))
            }
            Load::Failed(e) => {
                return page.child(ui::empty_card(ui::error_state_with(
                    "releases-failed",
                    tr("Couldn't load versions"),
                    e,
                    cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.fetch_releases();
                        cx.notify();
                    }),
                    Some(Self::proxy_settings_button("releases-failed", cx)),
                    ButtonKind::Primary,
                )));
            }
            Load::Ready(r) if r.is_empty() => {
                return page.child(ui::empty_card(ui::empty_state(
                    "releases-empty",
                    "package-01",
                    tr("No versions yet"),
                    tr("Eclipse hasn't published a version yet."),
                    vec![],
                )));
            }
            Load::Ready(r) => r,
        };
        let keep = |i: usize| {
            let tag = &releases[i].tag;
            self.installing.as_ref().is_some_and(|(t, _)| t == tag)
                || self.install_error.as_ref().is_some_and(|(t, _)| t == tag)
                || self.is_asking(&Asking::Version(tag.clone()))
                || self.removing.contains(tag)
        };
        let list = self.long_list.clone().column(
            &self.content_scroll.handle,
            releases.len(),
            layout::CARD_GAP,
            keep,
            |i| {
                let release = releases[i].clone();
                let title = release
                    .name
                    .split_once(": ")
                    .map(|(_, rest)| rest.to_string())
                    .unwrap_or_else(|| release.name.clone());
                let title = if title.is_empty() {
                    release.tag.clone()
                } else {
                    title
                };
                let mut line = dates::published(&release.date, &release.time);
                let right = self.release_action(&release, cx);
                self.version_card(&release.tag, title, &mut line, i == 0, on_disk, right)
            },
        );
        page.child(list)
    }

    /// One version's card: its name (Newest on the first, In use on the one in use), `line`
    /// with who plays it, and `right`.
    fn version_card(
        &self,
        tag: &str,
        title: String,
        line: &mut String,
        newest: bool,
        on_disk: &dyn Fn(&String) -> Option<u64>,
        right: AnyElement,
    ) -> Div {
        let look = ui::look();
        let installed = self.installed.iter().any(|v| v == tag);
        let in_use = self.selected().map(String::as_str) == Some(tag);
        let players: Vec<&str> = self
            .instances
            .list
            .iter()
            .filter(|i| i.java.is_none() && self.version_of(i).map(String::as_str) == Some(tag))
            .map(|i| i.name.as_str())
            .collect();
        let mut add = |part: String| {
            if !line.is_empty() {
                line.push_str(" · ");
            }
            line.push_str(&part);
        };
        if !players.is_empty() {
            add(trf("played by {names}", &[("names", &players.join(", "))]));
        } else if installed {
            add(tr("installed").to_string());
        }
        if let Some(n) = on_disk(&tag.to_string()) {
            add(trf("{size} on disk", &[("size", &megabytes(n))]));
        }
        let error = self
            .install_error
            .as_ref()
            .filter(|(e, _)| e == tag)
            .map(|(_, e)| e.clone());
        ui::card(radius::PANEL)
            .flex()
            .flex_col()
            .gap(px(space::MD))
            .p(px(ui::CARD_PADDING))
            .child(
                div()
                    .relative()
                    .flex()
                    .items_center()
                    .gap(px(space::LG))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(space::XXS))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(space::MD))
                                    .child(ui::card_title(title).truncate())
                                    .when(newest, |d| {
                                        d.child(ui::state_line(
                                            Tone::Accent(look.accent_soft),
                                            tr("Newest"),
                                        ))
                                    })
                                    .when(in_use, |d| {
                                        d.child(ui::state_line(Tone::Live, tr("In use")))
                                    }),
                            )
                            .child(ui::card_note(line.clone()).truncate()),
                    )
                    .child(div().relative().flex().flex_none().child(right)),
            )
            .children(error.map(|e| problem_line(format!("install-error-{tag}"), &e)))
    }

    /// What a release's card offers on its right: its install's progress, Install, or Use and
    /// the bin once installed.
    fn release_action(&self, release: &Release, cx: &mut Context<Self>) -> AnyElement {
        let tag = release.tag.clone();
        if let Some((_, p)) = self.installing.as_ref().filter(|(t, _)| *t == tag) {
            return self
                .install_progress(Some(*p), tr("Installing"))
                .into_any_element();
        }
        if self.removing.contains(&tag) {
            return self
                .install_progress(None, tr("Removing…"))
                .into_any_element();
        }
        if self.installed.contains(&tag) {
            if backend::needs_game_architecture_install(&tag, release) {
                let b = ui::button(
                    SharedString::from(format!("install-selected-architecture-{tag}")),
                    None,
                    tr("Install selected build"),
                    ButtonKind::Secondary,
                    ui::ButtonSize::Md,
                );
                if self.installing.is_some() {
                    return ui::disabled(b).into_any_element();
                }
                let release = release.clone();
                return b
                    .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.install(&release);
                        cx.notify();
                    }))
                    .into_any_element();
            }
            return self.version_actions(&tag, cx).into_any_element();
        }
        if release.asset.is_none() {
            return ui::card_note(tr("No build for this platform")).into_any_element();
        }
        let b = ui::button(
            SharedString::from(format!("install-{tag}")),
            None,
            tr("Install"),
            ButtonKind::Secondary,
            ui::ButtonSize::Md,
        );
        if self.installing.is_some() {
            ui::disabled(b).into_any_element()
        } else {
            let release = release.clone();
            b.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.install(&release);
                cx.notify();
            }))
            .into_any_element()
        }
    }

    /// A bar with its word: an install's progress, or a removal's.
    fn install_progress(&self, fraction: Option<f32>, label: &str) -> Div {
        let word = match fraction {
            Some(p) => format!("{:.0}%", p * 100.),
            None => label.to_string(),
        };
        div()
            .flex()
            .items_center()
            .gap(px(space::MD))
            .w(px(PROGRESS_WIDTH))
            .child(ui::progress_bar(fraction.map(|_| self.bar)))
            .child(ui::card_note(word).flex_none())
    }

    /// An installed version's actions: Use (when it isn't the one in use) and the bin, which asks
    /// again before it removes.
    fn version_actions(&self, tag: &str, cx: &mut Context<Self>) -> Div {
        let selected = self.selected().map(String::as_str) == Some(tag);
        let confirming = self.is_asking(&Asking::Version(tag.to_string()));
        let bin = SharedString::from(format!("bin-{tag}"));
        let (t1, t2, t3) = (tag.to_string(), tag.to_string(), tag.to_string());
        let mut d = div().flex().items_center().gap(px(space::SM));
        if confirming {
            return d.child(ui::confirm(
                bin,
                trf("Remove {name}?", &[("name", tag)]),
                tr("Remove"),
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.asking = None;
                    cx.notify();
                }),
                cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.remove(&t2);
                    cx.notify();
                }),
            ));
        }
        if !selected {
            d = d.child(
                ui::button(
                    SharedString::from(format!("use-{tag}")),
                    None,
                    tr("Use"),
                    ButtonKind::Secondary,
                    ui::ButtonSize::Md,
                )
                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.settings.selected = Some(t1.clone());
                    this.save_settings();
                    cx.notify();
                })),
            );
        }
        d.child(
            ui::icon_button(
                bin,
                "delete-02",
                tr("Remove…"),
                ButtonKind::Ghost,
                ui::ButtonSize::Md,
                false,
            )
            .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.asking = Some(Asking::Version(t3.clone()));
                cx.notify();
            })),
        )
    }

    /// The installed builds of one kind made here, as cards under a small heading; None when
    /// there are none.
    fn local_builds(
        &self,
        title: &str,
        is: fn(&str) -> bool,
        notice_: Option<&str>,
        on_disk: &dyn Fn(&String) -> Option<u64>,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let tags: Vec<String> = self
            .installed
            .iter()
            .chain(&self.removing)
            .filter(|v| is(v))
            .cloned()
            .collect();
        (!tags.is_empty()).then(|| {
            let mut d = div()
                .flex()
                .flex_col()
                .gap(px(space::MD))
                .mb(px(space::XL))
                .child(ui::label(tr(title).to_string()))
                .children(notice_.map(|n| ui::notice_line(ui::warn(), n.to_string())));
            for tag in tags {
                let mut line = Vec::new();
                if let Some(about) = native_mods::about(&tag) {
                    line.push(trf("Built on {base}", &[("base", &about.base)]));
                }
                if let Some(about) = build_import::about(&tag) {
                    let base = eclipse_launcher::nightly::label(&about.base).unwrap_or(about.base);
                    line.push(trf("Game files from {base}", &[("base", &base)]));
                    line.push(about.file);
                }
                let mut line = line.join(" · ");
                let right = if self.removing.contains(&tag) {
                    self.install_progress(None, tr("Removing…"))
                        .into_any_element()
                } else {
                    self.version_actions(&tag, cx).into_any_element()
                };
                let title = eclipse_launcher::nightly::label(&tag)
                    .or_else(|| build_import::label(&tag))
                    .unwrap_or_else(|| tag.trim_start_matches('v').to_string());
                d = d.child(self.version_card(&tag, title, &mut line, false, on_disk, right));
            }
            d
        })
    }
}

impl Launcher {
    pub(super) fn versions_page(&mut self, cx: &mut Context<Self>) -> Div {
        // Measured again when the installed versions change.
        if self.disk_for != self.installed {
            self.disk_for = self.installed.clone();
            let (tags, tx) = (self.installed.clone(), self.tx.clone());
            std::thread::spawn(move || {
                let disk = tags.into_iter().map(|t| {
                    let n = backend::disk_use(&t);
                    (t, n)
                });
                let _ = tx.send(Msg::DiskUse(disk.collect()));
            });
        }
        let on_disk = |tag: &String| {
            let found = self.disk.iter().find(|(t, _)| t == tag);
            found
                .filter(|_| self.installed.contains(tag))
                .map(|(_, n)| *n)
        };
        let used: u64 = self.installed.iter().filter_map(on_disk).sum();
        self.versions_view(&on_disk, used, cx)
    }
}
