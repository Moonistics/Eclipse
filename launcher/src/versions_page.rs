//! The Eclipse versions page: each Nightly on its own card (its name, Newest on the first, when
//! it came out and which instances play it), Install or Use and a bin on the right; the space
//! versions take, Import a version from file, the native and imported builds, install progress
//! and errors, the confirmation before a removal.

use super::*;
use crate::menus::Asking;
use crate::ui::{self, Tone};
use eclipse_theme::glass::space;

impl Launcher {
    /// The Versions page; `on_disk` and `used` as `versions_page` measured them.
    pub(super) fn versions_view(
        &self,
        on_disk: &dyn Fn(&String) -> Option<u64>,
        used: u64,
        cx: &mut Context<Self>,
    ) -> Div {
        let import = self
            .import_build_button("import-build", cx)
            .into_any_element();
        let head = ui::page_head_with(Page::Versions.title(), None, Some(import));
        let mut page = div().flex().flex_col().child(head).when(used > 0, |d| {
            d.child(
                ui::card_note(trf(
                    "Installed versions use {size}.",
                    &[("size", &megabytes(used))],
                ))
                .mb(px(space::MD)),
            )
        });
        let native = self.local_builds(
            "versions-native",
            tr("Builds with mods").into(),
            &native_mods::is_native,
            None,
            on_disk,
            cx,
        );
        let imported = self.local_builds(
            "versions-imported",
            tr("Imported builds").into(),
            &build_import::is_imported,
            Some(tr("Not checked by Eclipse. Only use builds you trust.")),
            on_disk,
            cx,
        );
        let mut grouped = native.is_some() || imported.is_some();
        page = page
            .children(self.versions.import_error.clone().map(|e| {
                ui::error_line_for(
                    "import-error",
                    tr("Couldn't import the version"),
                    &e,
                    None,
                    Vec::new(),
                )
                .mb(px(layout::CARD_GAP))
            }))
            .children(native)
            .children(imported);
        let downloaded =
            |tag: &str| !native_mods::is_native(tag) && !build_import::is_imported(tag);
        let releases = match &self.versions.releases {
            Load::Loading => {
                return page.child(ui::empty_card(ui::loading_state(
                    "releases-loading",
                    ui::page_only(tr("Loading versions…")),
                )))
            }
            Load::Failed(e) => {
                let offline = self.local_builds(
                    "versions-installed",
                    tr("Installed versions").into(),
                    &downloaded,
                    None,
                    on_disk,
                    cx,
                );
                return page
                    .children(offline)
                    .child(ui::empty_card(ui::error_state_with(
                        "releases-failed",
                        ui::page_only(tr("Couldn't load versions")),
                        e,
                        cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.fetch_releases();
                            cx.notify();
                        }),
                        vec![Self::proxy_settings_button("releases-failed", cx)],
                        ButtonKind::Primary,
                    )));
            }
            Load::Ready(r) if r.is_empty() => {
                let installed = self.local_builds(
                    "versions-installed",
                    tr("Installed versions").into(),
                    &downloaded,
                    None,
                    on_disk,
                    cx,
                );
                return page
                    .children(installed)
                    .child(ui::empty_card(ui::empty_state(
                        "releases-empty",
                        crate::icons::VERSION,
                        ui::page_only(tr("No versions yet")),
                        tr("Eclipse hasn't published a version yet."),
                        vec![],
                    )));
            }
            Load::Ready(r) => r,
        };
        let unlisted = |tag: &str| downloaded(tag) && !releases.iter().any(|r| r.tag == tag);
        let unlisted = self.local_builds(
            "versions-installed",
            tr("Installed versions").into(),
            &unlisted,
            None,
            on_disk,
            cx,
        );
        grouped |= unlisted.is_some();
        page = page.children(unlisted);
        let keep = |i: usize| {
            let tag = &releases[i].tag;
            self.versions.installing(tag).is_some()
                || self
                    .versions
                    .install_error
                    .as_ref()
                    .is_some_and(|(t, _)| t == tag)
                || self.is_asking(&Asking::Version(tag.clone()))
                || self.versions.removing.contains(tag)
        };
        let list = self.long_list.clone().column(
            &self.content_scroll.handle,
            releases.len(),
            layout::CARD_GAP,
            keep,
            |i| {
                let release = releases[i].clone();
                let title = eclipse_launcher::nightly::display_name(&release.tag);
                let mut line = dates::published(&release.date, &release.time);
                let right = self.release_action(&release, cx);
                self.version_card(&release.tag, title, &mut line, i == 0, on_disk, right)
            },
        );
        match grouped {
            true => page.child(ui::field_group(
                "versions-all-title",
                tr("All versions"),
                None,
                list,
            )),
            false => page.child(list),
        }
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
        let installed = self.versions.installed.iter().any(|v| v == tag);
        let in_use = self.selected_version().map(String::as_str) == Some(tag);
        let used_by: Vec<&str> = self
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
        if !used_by.is_empty() {
            add(trf("used by {names}", &[("names", &used_by.join(", "))]));
        } else if installed {
            add(tr("installed").to_string());
        }
        if let Some(n) = on_disk(&tag.to_string()) {
            add(trf("{size} on disk", &[("size", &megabytes(n))]));
        }
        let error = self
            .versions
            .install_error
            .as_ref()
            .filter(|(e, _)| e == tag)
            .map(|(_, e)| e.clone());
        ui::card()
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
                    .child(ui::release_head(
                        title,
                        line.clone(),
                        newest
                            .then(|| ui::state_line(Tone::Accent(look.accent_soft), tr("Newest")))
                            .into_iter()
                            .chain(in_use.then(|| ui::state_line(Tone::Live, tr("In use"))))
                            .collect(),
                    ))
                    .child(div().relative().flex().flex_none().child(right)),
            )
            .children(error.map(|e| {
                ui::error_line_for(
                    format!("install-error-{tag}"),
                    tr("Couldn't install this version"),
                    &e,
                    None,
                    Vec::new(),
                )
            }))
    }

    /// What a release's card offers on its right: its install's progress, Install, or Use and
    /// the bin once installed.
    fn release_action(&self, release: &Release, cx: &mut Context<Self>) -> AnyElement {
        let tag = release.tag.clone();
        if let Some(bar) = self
            .install_bar()
            .filter(|_| self.versions.installing(&tag).is_some())
        {
            return bar.into_any_element();
        }
        if self.versions.removing.contains(&tag) {
            return self.removing_bar().into_any_element();
        }
        if self.versions.installed.contains(&tag) {
            if backend::needs_game_architecture_install(&tag, release) {
                let b = ui::button(
                    SharedString::from(format!("install-selected-architecture-{tag}")),
                    Some("download-04"),
                    tr("Install selected build"),
                    ButtonKind::Secondary,
                    ui::ButtonSize::Md,
                );
                if self.versions.installing.is_some() {
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
            return self
                .version_actions(
                    &tag,
                    &eclipse_launcher::nightly::display_name(&release.tag),
                    cx,
                )
                .into_any_element();
        }
        if release.asset.is_none() {
            return ui::card_note(tr("No build for this platform")).into_any_element();
        }
        let b = ui::button(
            SharedString::from(format!("install-{tag}")),
            Some("download-04"),
            tr("Install"),
            ButtonKind::Secondary,
            ui::ButtonSize::Md,
        );
        if self.versions.installing.is_some() {
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

    fn removing_bar(&self) -> Div {
        div()
            .w(px(layout::FIELD_MD))
            .child(ui::progress_line("", None, tr("Removing…")))
    }

    /// An installed version's actions: Use (when it isn't the one in use) and the bin, which asks
    /// again before it removes.
    fn version_actions(&self, tag: &str, name: &str, cx: &mut Context<Self>) -> Div {
        let selected = self.selected_version().map(String::as_str) == Some(tag);
        let confirming = self.is_asking(&Asking::Version(tag.to_string()));
        let bin = SharedString::from(format!("bin-{tag}"));
        let (t1, t2, t3) = (tag.to_string(), tag.to_string(), tag.to_string());
        let mut d = div().flex().items_center().gap(px(space::SM));
        if confirming {
            return d.child(ui::confirm(
                bin,
                ui::Confirm::Remove(name),
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
        let broken = selected && self.game.prepare_error.is_some();
        if broken && self.versions.release(tag).is_some() {
            let again = tag.to_string();
            d = d.child(
                ui::button(
                    SharedString::from(format!("reinstall-{tag}")),
                    Some("download-04"),
                    tr("Reinstall"),
                    ButtonKind::Secondary,
                    ui::ButtonSize::Md,
                )
                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.reinstall(&again);
                    cx.notify();
                })),
            );
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
                    this.use_version(t1.clone());
                    cx.notify();
                })),
            );
        }
        d.child(
            ui::remove_button(bin, tr("Remove…"), ui::ButtonSize::Md).on_press(cx.listener(
                move |this, _: &ClickEvent, _, cx| {
                    this.asking = Some(Asking::Version(t3.clone()));
                    cx.notify();
                },
            )),
        )
    }

    /// The installed builds of one kind made here, as cards under a small heading; None when
    /// there are none.
    fn local_builds(
        &self,
        id: &str,
        title: SharedString,
        is: &dyn Fn(&str) -> bool,
        notice_: Option<&str>,
        on_disk: &dyn Fn(&String) -> Option<u64>,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let tags: Vec<String> = self
            .versions
            .installed
            .iter()
            .chain(&self.versions.removing)
            .filter(|v| is(v))
            .cloned()
            .collect();
        (!tags.is_empty()).then(|| {
            let mut d =
                div()
                    .flex()
                    .flex_col()
                    .gap(px(layout::CARD_GAP))
                    .children(notice_.map(|n| {
                        ui::notice(ui::Notice::Warn, "alert-circle", n.to_string(), Vec::new())
                    }));
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
                let title = eclipse_launcher::nightly::display_name(&tag);
                let right = if self.versions.removing.contains(&tag) {
                    self.removing_bar().into_any_element()
                } else {
                    self.version_actions(&tag, &title, cx).into_any_element()
                };
                d = d.child(self.version_card(&tag, title, &mut line, false, on_disk, right));
            }
            ui::field_group(format!("{id}-title"), title, None, d).mb(px(layout::CARD_GAP))
        })
    }
}

impl Launcher {
    pub(super) fn versions_page(&mut self, cx: &mut Context<Self>) -> Div {
        // Measured again when the installed versions change.
        if self.versions.disk_for != self.versions.installed {
            self.versions.disk_for = self.versions.installed.clone();
            let (tags, tx) = (self.versions.installed.clone(), self.tx.clone());
            std::thread::spawn(move || {
                let disk = tags.into_iter().map(|t| {
                    let n = backend::disk_use(&t);
                    (t, n)
                });
                let _ = tx.send(Msg::DiskUse(disk.collect()));
            });
        }
        let on_disk = |tag: &String| {
            let found = self.versions.disk.iter().find(|(t, _)| t == tag);
            found
                .filter(|_| self.versions.installed.contains(tag))
                .map(|(_, n)| *n)
        };
        let used: u64 = self.versions.installed.iter().filter_map(on_disk).sum();
        self.versions_view(&on_disk, used, cx)
    }
}
