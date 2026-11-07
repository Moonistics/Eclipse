//! A Java instance's part of the New instance page (`instance_page.rs`): Mojang's versions to
//! pick from, and for Modded the loaders, each with its logo, as many as `loaders::Kind` knows.
use crate::fields::Field;
use crate::instance_page::loaders;
use crate::instance_state::{JavaChoice, NewKind, VersionList};
use crate::ui::{ButtonKind, Press};
use crate::{dates, instance_icons, layout, ui, Launcher};
use eclipse_launcher::backend;
use eclipse_launcher::i18n::{tr, trf};
use eclipse_launcher::instances::{self, Instance, Plays, Unkept};
use eclipse_launcher::loaders::Loader;
use eclipse_launcher::minecraft_versions::Kind as VersionKind;
use eclipse_theme::glass::{ink, space};
use gpui::prelude::*;
use gpui::{div, px, ClickEvent, Div, FontWeight, SharedString, Stateful, Window};

/// Mojang versions listed at most: the search finds the others.
const ROWS: usize = 60;
const LIST: f32 = 206.;
const LIST_SHORT: f32 = 180.;
const VERSION_COLUMN: f32 = layout::FIELD_MD;
const LOADER_COLUMNS: u16 = 2;

fn released(time: &str) -> String {
    dates::published(time.get(..10).unwrap_or_default(), "")
}

pub(super) fn version_list_height() -> f32 {
    if ui::short_window() {
        LIST_SHORT
    } else {
        LIST
    }
}

pub(super) fn reveal_version_row(list: &str, at: usize) {
    ui::scroll_list_to(list, at);
    ui::start_options_cursor(list, at);
}

pub(super) fn version_row(
    id: String,
    name: impl Into<SharedString>,
    note: impl IntoElement,
    on: bool,
) -> Stateful<Div> {
    ui::list_choice(id, on)
        .py(px(space::SM))
        .child(
            ui::text(
                eclipse_theme::glass::step::LEAD,
                FontWeight::SEMIBOLD,
                ink::FULL,
            )
            .w(px(VERSION_COLUMN))
            .flex_none()
            .truncate()
            .child(name.into()),
        )
        .child(
            ui::text(
                eclipse_theme::glass::step::NOTE,
                FontWeight::NORMAL,
                ink::MUTED,
            )
            .flex_1()
            .min_w_0()
            .truncate()
            .child(note),
        )
        .child(ui::tick_slot(on))
}

impl Launcher {
    pub(super) fn minecraft_versions(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let Some(choice) = self.instances.draft.as_ref().map(|d| d.java.clone()) else {
            return div().id("instance-minecraft-version");
        };
        let typed = self.fields.value(Field::VersionSearch).trim().to_string();
        let search = typed.to_lowercase();
        let chosen = self.draft_minecraft();
        let filter = |id: &'static str,
                      label: &'static str,
                      on: bool,
                      flip: fn(&mut JavaChoice)| {
            let flip =
                move |this: &mut Self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>| {
                    this.change(flip);
                    cx.notify();
                };
            ui::toggle_label(id, tr(label), on).on_press(cx.listener(flip))
        };
        let filters = div()
            .flex()
            .items_center()
            .gap(px(space::XL))
            .child(filter(
                "mc-snapshots",
                "Show snapshots",
                choice.snapshots,
                |j| j.snapshots = !j.snapshots,
            ))
            .child(filter("mc-old", "Show old versions", choice.old, |j| {
                j.old = !j.old
            }));
        let field = self
            .field_with(
                Field::VersionSearch,
                Some(ui::field_icon("search-01")),
                tr("Search versions"),
                layout::FIELD_XL,
                ui::ButtonSize::Lg,
                None,
                cx,
            )
            .w_full()
            .mb(px(space::SM));
        let height = version_list_height();
        let list = match &self.instances.versions {
            VersionList::NotAsked | VersionList::Loading => {
                ui::with_scrollbar(ui::list_box("mc-versions", height).child(ui::loading_state(
                    "mc-versions-loading",
                    tr("Loading Minecraft versions…"),
                )))
                .into_any_element()
            }
            VersionList::Failed(e) => ui::error_state_with(
                "mc-versions-failed",
                tr("Couldn't load Minecraft versions"),
                e,
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.instances.versions = VersionList::NotAsked;
                    this.load_versions(cx);
                    cx.notify();
                }),
                vec![Self::proxy_settings_button("mc-versions-failed", cx)],
                ButtonKind::Secondary,
            )
            .into_any_element(),
            VersionList::Ready(versions) => {
                let shown: Vec<_> = versions
                    .of(choice.snapshots, choice.old)
                    .filter(|v| search.is_empty() || v.id.to_lowercase().contains(&search))
                    .take(ROWS)
                    .collect();
                let ids: Vec<String> = shown.iter().map(|v| v.id.clone()).collect();
                let rows: Vec<_> = shown
                    .into_iter()
                    .map(|v| {
                        let on = chosen.as_deref() == Some(v.id.as_str());
                        let kind = match v.kind {
                            VersionKind::Release => "Release",
                            VersionKind::Snapshot => "Snapshot",
                            VersionKind::OldBeta => "Beta",
                            VersionKind::OldAlpha => "Alpha",
                        };
                        let row = version_row(
                            format!("mc-{}", v.id),
                            v.id.clone(),
                            ui::card_note(format!("{} · {}", tr(kind), released(&v.released))),
                            on,
                        );
                        (row, SharedString::from(v.id.clone()), on)
                    })
                    .collect();
                let reveal = self.instances.draft.as_mut().map(|d| {
                    let reveal = d.reveal_version;
                    d.reveal_version = false;
                    reveal
                });
                if reveal == Some(true) {
                    let at = versions
                        .of(choice.snapshots, choice.old)
                        .filter(|v| search.is_empty() || v.id.to_lowercase().contains(&search))
                        .take(ROWS)
                        .position(|v| chosen.as_deref() == Some(v.id.as_str()));
                    if let Some(at) = at {
                        reveal_version_row("mc-versions", at);
                    }
                }
                if rows.is_empty() {
                    ui::empty_state(
                        "mc-versions-no-match",
                        "search-01",
                        trf("No version matches “{search}”", &[("search", &typed)]),
                        tr("Try another version number."),
                        vec![],
                    )
                    .into_any_element()
                } else {
                    let this = cx.entity().downgrade();
                    let list = ui::options(
                        ui::list_box("mc-versions", height),
                        tr("Minecraft version"),
                        rows,
                        true,
                        move |i, _, cx| {
                            let id = ids[i].clone();
                            let _ = this.update(cx, |this, cx| {
                                this.change(|j| j.version = Some(id));
                                cx.notify();
                            });
                        },
                    );
                    ui::with_scrollbar(list).into_any_element()
                }
            }
        };
        let (aside, filters_above) = if self.instance_page_compact() {
            (None, Some(filters.mb(px(space::MD))))
        } else {
            (Some(filters.into_any_element()), None)
        };
        ui::section_card(
            "instance-minecraft-version",
            tr("Minecraft version"),
            aside,
            div()
                .flex()
                .flex_col()
                .children(filters_above)
                .child(field)
                .child(list),
        )
    }

    pub(super) fn loader_choice(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let now = self.draft_loader();
        let mut cards = Vec::new();
        let all = loaders();
        for &loader in &all {
            let logo = self.choice_icon(instance_icons::Mark::Loader(loader), loader.name());
            let note = match loader.id() {
                "fabric" => tr("Most mods for new versions"),
                "quilt" => tr("Fabric mods run here too"),
                "neoforge" => tr("The big modpacks' home"),
                "forge" => tr("Mods and packs for older versions"),
                _ => "",
            };
            let card = self.choice_row(
                format!("loader-{}", loader.id()),
                loader == now,
                logo,
                loader.name().into(),
                note.into(),
            );
            cards.push((card, SharedString::from(loader.name()), loader == now));
        }
        let this = cx.entity().downgrade();
        let grid = ui::options(
            self.choices("instance-loader-choices", LOADER_COLUMNS),
            tr("Mod loader"),
            cards,
            true,
            move |i, _, cx| {
                let loader = all[i];
                let _ = this.update(cx, |this, cx| {
                    this.change(|j| j.loader = loader);
                    cx.notify();
                });
            },
        );
        ui::section_card("instance-loader", tr("Mod loader"), None, grid)
    }

    /// What the page has `editing` play: Eclipse, or a Java game with its loader. The loader
    /// keeps its version while its kind and Minecraft stay; a new one's (empty here) is the
    /// newest stable, asked of its service on Save.
    pub(super) fn draft_plays(&self, editing: &Instance) -> Option<Plays> {
        let d = self.instances.draft.as_ref()?;
        if d.java.kind == NewKind::Eclipse {
            return Some(Plays::Eclipse);
        }
        let minecraft = self.draft_minecraft()?;
        let loader = (d.java.kind == NewKind::Modded).then(|| {
            editing
                .java
                .as_ref()
                .filter(|j| j.minecraft == minecraft)
                .and_then(|j| j.loader.clone())
                .filter(|l| l.kind == d.java.loader)
                .unwrap_or(Loader {
                    kind: d.java.loader,
                    version: String::new(),
                })
        });
        Some(Plays::Java { minecraft, loader })
    }

    /// Before Save switches what `instance` plays: what can't keep working. Nothing in its folder
    /// is deleted or changed.
    pub(super) fn switch_notice(&self, instance: &Instance) -> Option<Div> {
        let to = self.draft_plays(instance)?;
        if to == Plays::of(instance) {
            return None;
        }
        let eclipse = backend::game_version(&self.draft_eclipse().unwrap_or_default());
        let lines: Vec<(&str, String)> = instances::unkept(instance, &to, &eclipse)
            .into_iter()
            .map(|u| match u {
                Unkept::Mods { made_for, .. } => (
                    "instance-switch-mods",
                    trf(
                        "Its mods made for {loader} stay in its folder but won't load.",
                        &[("loader", &made_for)],
                    ),
                ),
                Unkept::ModsVersion { from, to, .. } => (
                    "instance-switch-mods-version",
                    trf(
                        "Its mods made for Minecraft {from} stay in its folder; some may not load in {to}.",
                        &[("from", &from), ("to", &to)],
                    ),
                ),
                Unkept::Worlds { from, to, .. } => {
                    let names = [("from", from.as_str()), ("to", to.as_str())];
                    let said = match self.older(&to, &from) {
                        Some(true) => trf(
                            "Minecraft {to} can't open its worlds: they were saved by {from}.",
                            &names,
                        ),
                        Some(false) => trf(
                            "Minecraft {to} upgrades its worlds for good: {from} can't open them again.",
                            &names,
                        ),
                        None => trf(
                            "Its worlds were saved by Minecraft {from}: {to} upgrades them for good, and can't open them if it is older.",
                            &names,
                        ),
                    };
                    ("instance-switch-worlds", said)
                }
            })
            .collect();
        if lines.is_empty() {
            return None;
        }
        let said = div()
            .flex()
            .flex_col()
            .gap(px(space::XS))
            .min_w_0()
            .children(lines.into_iter().map(|(id, line)| ui::said(id, line)));
        Some(ui::notice(ui::Notice::Warn, "alert-circle", said, vec![]))
    }

    fn older(&self, version: &str, than: &str) -> Option<bool> {
        let VersionList::Ready(versions) = &self.instances.versions else {
            return None;
        };
        let at = |id: &str| versions.all.iter().position(|v| v.id == id);
        Some(at(version)? > at(than)?)
    }
}
