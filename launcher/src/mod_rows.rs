//! Installed mods' rows, shared by Home's Mods tab and the Mods page: the install under way, native
//! mods' rows and actions, each mod's icon (Modrinth's, asked for in one request and kept beside
//! the image cache, else its jar's), and the Mods page's Installed group.
use crate::menus::Asking;
use crate::mods_state::{goes_to_trash, removal, ModsBusy};
use crate::ui::{ink, ButtonKind, Press};
use crate::{icons, mods_state, ui, Launcher};
use eclipse_launcher::browser::{images, installed, modrinth, Kind as PackKind, SourceId};
use eclipse_launcher::i18n::{tr, trf};
use eclipse_launcher::{github_mods, native_mods, native_status};
use eclipse_theme::glass::space;
use gpui::prelude::*;
use gpui::{div, px, ClickEvent, Div, SharedString, Stateful};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A mod as the tab lists it.
#[derive(Clone)]
pub(super) struct ModRow {
    /// Its file in the instance, as it is when on (`mods/lumen-0.6.2.jar`).
    pub(super) file: String,
    pub(super) name: String,
    pub(super) version: String,
    pub(super) on: bool,
    /// What the browser knows of it; None: a file put in by hand.
    pub(super) entry: Option<installed::Entry>,
    /// The icon its jar names, scaled to the row (`images::ROW`) and decoded once by the UI.
    pub(super) icon: Option<Arc<gpui::Image>>,
    pub(super) native: Option<eclipse_launcher::github_mods::Installed>,
}

/// Modrinth projects' icon URLs by project id, kept on disk beside the image cache so the icons
/// show offline too; each id is asked for once a session.
#[derive(Default)]
pub(super) struct ProjectIcons {
    urls: HashMap<String, String>,
    loaded: bool,
    asked: HashSet<String>,
}

const FOLD_INSTALLED_PAST: usize = 5;

const INSTALLED_ROW_BLEED: f32 = ui::CARD_PADDING - space::MD;

impl Launcher {
    /// The install under way and the last change that failed.
    pub(super) fn mods_busy_plate(&self) -> Vec<Div> {
        let mut out = Vec::new();
        match &self.mods.busy {
            Some(ModsBusy::Install { line, bar, .. }) => out.push(ui::progress_line(
                tr("Installing"),
                Some(*bar),
                line.clone(),
            )),
            Some(ModsBusy::UpdateAll { line, bar }) => {
                out.push(ui::progress_line(tr("Updating"), Some(*bar), line.clone()))
            }
            None => {}
        }
        if let Some(e) = &self.mods.failed {
            let instance = self.target_name();
            out.push(div().child(ui::error_line_for(
                "mods-failed-step",
                trf("Couldn't change {instance}", &[("instance", &instance)]),
                e,
                None,
                Vec::new(),
            )))
        }
        out
    }

    /// Re-read local/GitHub files when the folder changes, not on every frame.
    pub(super) fn native_files(&self) -> Vec<eclipse_launcher::github_mods::Installed> {
        let home = self.mods.home();
        let key = native_status::fingerprint(&home, None);
        let mut memo = self.mods.native_files.borrow_mut();
        if memo.as_ref().is_none_or(|(k, _)| *k != key) {
            *memo = Some((key, eclipse_launcher::github_mods::untracked(&home)));
            self.mods.installed_list.reset();
        }
        memo.as_ref().unwrap().1.clone()
    }

    pub(super) fn native_mod_actions(
        &self,
        row: &github_mods::Installed,
        busy: bool,
        cx: &Context<Self>,
    ) -> Div {
        let busy = busy || self.github.busy();
        let mut right = row.origin.as_ref().map_or_else(
            || div().flex().items_center().gap(px(space::SM)),
            |origin| self.github_actions(origin, busy, cx),
        );
        let home = self.mods.home();
        let on = eclipse_launcher::browser::java::is_enabled(&home, &row.id);
        let id = row.id.clone();
        right = right.child(
            ui::toggle_label(
                format!("native-switch-{id}"),
                if on { tr("On") } else { tr("Off") },
                on,
            )
            .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                let home = this.mods.home();
                // An update holds the folder: the window doesn't wait for it.
                let Some(_lock) = eclipse_launcher::browser::try_lock_home(&home) else {
                    this.mods.failed = Some(mods_state::folder_busy());
                    cx.notify();
                    return;
                };
                this.mods.failed =
                    eclipse_launcher::browser::java::set_enabled(&home, &id, !on).err();
                this.instance_mods_changed();
                cx.notify();
            })),
        );
        let confirming = self.is_asking(&Asking::GithubMod(row.file.clone()));
        let remove_id = SharedString::from(format!("native-remove-{}", row.file));
        if confirming && !busy {
            let (row, name) = (row.clone(), row.id.clone());
            return div().flex_none().child(ui::confirm(
                remove_id,
                ui::Confirm::Remove(&name),
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.asking = None;
                    cx.notify();
                }),
                cx.listener(move |this, _: &ClickEvent, _, cx| {
                    let home = this.mods.home();
                    let result = if row.origin.is_some() {
                        github_mods::remove(&home, &row.id).map(|_| ())
                    } else {
                        std::fs::remove_file(home.join(&row.file)).map_err(|e| e.to_string())
                    };
                    this.mods_said(result.map(|_| trf("Removed {name}", &[("name", &row.id)])));
                    this.asking = None;
                    this.instance_mods_changed();
                    cx.notify();
                }),
            ));
        }
        let remove = ui::remove_button(remove_id, tr("Remove…"), ui::ButtonSize::Md);
        let file = row.file.clone();
        right = right.child(if busy {
            ui::disabled(remove)
        } else {
            remove.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.asking = Some(Asking::GithubMod(file.clone()));
                cx.notify();
            }))
        });
        right
    }

    fn native_mod_row(
        &self,
        row: &github_mods::Installed,
        busy: bool,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let source = row.origin.as_ref().map_or_else(
            || trf("Local file · {file}", &[("file", &row.file)]),
            |o| trf("from GitHub: {source}", &[("source", &o.label())]),
        );
        let confirming = self.is_asking(&Asking::GithubMod(row.file.clone()));
        ui::list_row(
            SharedString::from(format!("native-row-{}", row.id)),
            confirming,
        )
        .child(icons::slot(None, &row.id, ui::TILE_MD))
        .child(ui::item_text(
            row.id.clone(),
            format!("{} · {source}", row.version),
        ))
        .child(self.native_mod_actions(row, busy, cx).flex_none())
    }

    /// Asks Modrinth, in one request, for the icons of `ids` it hasn't been asked for.
    pub(super) fn ask_project_icons(&mut self, ids: Vec<String>, cx: &mut Context<Self>) {
        let icons = &mut self.project_icons;
        if !icons.loaded {
            icons.loaded = true;
            icons.urls = std::fs::read(images::project_icons())
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or_default();
        }
        let todo: Vec<String> = ids
            .into_iter()
            .filter(|id| !icons.urls.contains_key(id) && icons.asked.insert(id.clone()))
            .collect();
        if todo.is_empty() {
            return;
        }
        let task = cx
            .background_executor()
            .spawn(async move { modrinth::icons(&todo) });
        cx.spawn(async move |this, cx| {
            let Ok(found) = task.await else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                let urls = &mut this.project_icons.urls;
                urls.extend(found);
                if let Ok(json) = serde_json::to_vec(urls) {
                    let file = images::project_icons();
                    let _ = std::fs::create_dir_all(file.parent().unwrap_or(Path::new(".")));
                    let _ = std::fs::write(file, json);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// The image of a Modrinth project's icon at a row's size, once its URL and image are here.
    pub(super) fn project_icon(&mut self, e: &installed::Entry) -> Option<PathBuf> {
        let url = (e.source == SourceId::Modrinth)
            .then(|| self.project_icons.urls.get(&e.project).cloned())
            .flatten()?;
        self.mods_image(&url, images::ROW)
    }

    /// A mod's icon at the row's size: Modrinth's, else its jar's, else a glyph.
    pub(super) fn mod_icon(
        &mut self,
        entry: Option<&installed::Entry>,
        jar: Option<Arc<gpui::Image>>,
        side: f32,
    ) -> Div {
        let remote = entry.and_then(|e| self.project_icon(e));
        let tile = ui::tile(side);
        match (remote, jar) {
            (Some(path), _) => tile.child(gpui::img(path).size(px(side))),
            (None, Some(image)) => tile.child(gpui::img(image).size(px(side))),
            (None, None) => tile.child(ui::icon(
                "puzzle",
                ui::IconSize::Px16,
                ink(eclipse_theme::glass::ink::MUTED),
            )),
        }
    }

    pub(super) fn mods_installed_list(&mut self, cx: &mut Context<Self>) -> Option<Div> {
        let kind = self.mods.kind;
        // Each project's own icon (Modrinth's, asked for once), at the row's size.
        let ids = self
            .mods
            .installed
            .iter()
            .filter(|e| e.kind == kind && e.source == SourceId::Modrinth)
            .map(|e| e.project.clone())
            .collect();
        self.ask_project_icons(ids, cx);
        let icons: Vec<Option<PathBuf>> = self
            .mods
            .installed
            .clone()
            .iter()
            .filter(|e| e.kind == kind)
            .map(|e| self.project_icon(e))
            .collect();
        let entries: Vec<&installed::Entry> = self
            .mods
            .installed
            .iter()
            .filter(|e| e.kind == kind)
            .collect();
        let native_entries = if kind == PackKind::Mod && self.mods.java.is_none() {
            self.native_files()
        } else {
            Vec::new()
        };
        if entries.is_empty() && native_entries.is_empty() {
            return None;
        }
        let busy = self.mods.busy.is_some();
        let wait = self.mods_wait();
        let native = !native_entries.is_empty()
            || self.mods.java.is_none()
                && entries
                    .iter()
                    .any(|e| native_status::is_native_file(&e.file));
        let build = native.then(|| self.native_build(cx)).flatten();
        let row = |i: usize| {
            if i >= entries.len() {
                return self.native_mod_row(&native_entries[i - entries.len()], busy, cx);
            }
            let e = entries[i];
            let key = (e.source, e.project.clone());
            let is_native = self.mods.java.is_none() && native_status::is_native_file(&e.file);
            let mut right = div().flex().flex_none().items_center().gap(px(space::SM));
            if let Some(number) = self.mods.updates.get(&key) {
                let mut update = ui::button(
                    SharedString::from(format!("mods-update-{}", e.project)),
                    None,
                    trf("Update to {version}", &[("version", number)]),
                    ButtonKind::Secondary,
                    ui::ButtonSize::Sm,
                );
                if let Some(why) = &wait {
                    update = ui::unavailable(update, why.clone());
                } else {
                    let (source, id) = key.clone();
                    update = update.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.mods_install(source, id.clone(), None);
                        cx.notify();
                    }));
                }
                right = right.child(update);
            }
            let confirming = self.is_asking(&Asking::Mod(e.project.clone()));
            let remove_id = SharedString::from(format!("mods-remove-{}", e.project));
            let id = e.project.clone();
            let trash = goes_to_trash(e.kind);
            let ask = if trash {
                tr("Delete…")
            } else {
                tr("Remove…")
            };
            let remove = ui::remove_button(remove_id.clone(), ask, ui::ButtonSize::Md).on_press(
                cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.asking = Some(Asking::Mod(id.clone()));
                    cx.notify();
                }),
            );
            let switches = e.kind == PackKind::Mod
                && (self.mods.java.is_some() || e.source == SourceId::Eclipse);
            let on = switches
                .then(|| installed::Game::of(self.mods.java.clone()).is_on(&self.mods.home(), e));
            if let Some(on) = on {
                let entry = e.clone();
                right = right.child(
                    ui::toggle_label(
                        SharedString::from(format!("mods-switch-{}", e.project)),
                        if on { tr("On") } else { tr("Off") },
                        on,
                    )
                    .on_press(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            this.mods_switch(&entry);
                            cx.notify();
                        },
                    )),
                );
            }
            if is_native {
                let folder = self.mods.home().join(native_mods::INSTALLED);
                right = right.child(ui::open_folder_icon(
                    SharedString::from(format!("mods-folder-{}", e.project)),
                    tr("Open Eclipse mods folder"),
                    folder,
                    ButtonKind::Ghost,
                    ui::ButtonSize::Md,
                ));
            }
            right = right.child(remove);
            if confirming {
                let id = e.project.clone();
                let asked = removal(e.kind, &e.title);
                right = div().flex_none().child(ui::confirm(
                    remove_id,
                    asked,
                    cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.asking = None;
                        cx.notify();
                    }),
                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                        let _ = this.mods_remove(&id);
                        cx.notify();
                    }),
                ));
            }
            let from = e
                .source
                .shown_for(&e.project)
                .map(|s| trf("from {source}", &[("source", s)]));
            let needed = e
                .dependency
                .then(|| tr("needed by another pack").to_string());
            let note = e
                .verdict
                .iter()
                .map(|v| tr(v).to_string())
                .chain([e.number.clone()])
                .chain(from)
                .chain(needed)
                .collect::<Vec<_>>()
                .join(" · ");
            let skipped = (!e.skipped.is_empty()).then(|| {
                ui::card_note(ui::state_line(
                    ui::Tone::Warn,
                    trf("Not ported: {item}", &[("item", &e.skipped.join(", "))]),
                ))
            });
            let icon = icons::slot(
                icons[i].clone().map(gpui::ImageSource::from),
                &e.title,
                ui::TILE_MD,
            );
            ui::list_row(
                SharedString::from(format!("mods-installed-{}", e.project)),
                confirming,
            )
            .child(icon)
            .child(ui::item_text(e.title.clone(), note).children(skipped))
            .child(right)
        };
        // Every kind's updates at once, whichever tab is open.
        let update_all = (!self.mods.updates.is_empty()).then(|| {
            let b = ui::button(
                "mods-update-all",
                None,
                trf(
                    "Update all ({count})",
                    &[("count", &self.mods.updates.len().to_string())],
                ),
                ButtonKind::Secondary,
                ui::ButtonSize::Md,
            );
            if let Some(why) = &wait {
                ui::unavailable(b, why.clone())
            } else {
                b.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.mods_update_all();
                    cx.notify();
                }))
            }
        });
        let count = entries.len() + native_entries.len();
        let long = count > FOLD_INSTALLED_PAST;
        let open = !long || self.mods.installed_open;
        let fold = long.then(|| {
            ui::icon_button(
                "mods-installed-fold",
                if open {
                    "arrow-down-01"
                } else {
                    "arrow-right-01"
                },
                if open {
                    tr("Hide list")
                } else {
                    tr("Show list")
                },
                ButtonKind::Ghost,
                ui::ButtonSize::Md,
                false,
            )
            .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.mods.installed_open = !this.mods.installed_open;
                cx.notify();
            }))
        });
        let aside = div()
            .flex()
            .items_center()
            .gap(px(space::SM))
            .children(update_all)
            .children(fold);
        let title = trf("Installed ({count})", &[("count", &count.to_string())]);
        if !open && build.is_none() {
            return Some(ui::group_head(
                "mods-installed-title",
                title,
                Some(aside.into_any_element()),
            ));
        }
        let list = open.then(|| {
            self.mods.installed_list.column(
                &self.content_scroll.handle,
                count,
                0.,
                |index| {
                    entries
                        .get(index)
                        .is_some_and(|e| self.is_asking(&Asking::Mod(e.project.clone())))
                },
                row,
            )
        });
        Some(ui::field_group(
            "mods-installed-title",
            title,
            Some(aside.into_any_element()),
            div()
                .flex()
                .flex_col()
                .gap(px(space::MD))
                .children(build)
                .children(list.map(|l| l.mx(px(-INSTALLED_ROW_BLEED)))),
        ))
    }
}
