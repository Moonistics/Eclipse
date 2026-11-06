//! A Java instance's part of the New instance page (`instance_page.rs`): Mojang's versions to
//! pick from, and for Modded the loaders, each with its logo, as many as `loaders::Kind` knows.
use super::*;
use crate::ui::{self, white};
use eclipse_launcher::instances::{self, Instance, Plays, Unkept};
use eclipse_launcher::loaders::Loader;
use eclipse_launcher::minecraft_versions::Kind as VersionKind;
use eclipse_theme::glass::{fill, ink, space};
use fields::Field;
use instance_page::loaders;
use instance_state::{JavaChoice, NewKind, VersionList};

/// Mojang versions listed at most: the search finds the others.
const ROWS: usize = 60;
const LIST: f32 = 206.;
const LIST_SHORT: f32 = 180.;
const SEARCH_FIELD_WIDTH: f32 = 600.;
const VERSION_COLUMN: f32 = 160.;

fn released(time: &str) -> String {
    dates::published(time.get(..10).unwrap_or_default(), "")
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
                SEARCH_FIELD_WIDTH,
                ui::ButtonSize::Lg,
                None,
                cx,
            )
            .w_full()
            .mb(px(space::SM));
        let height = if ui::short_window() { LIST_SHORT } else { LIST };
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
                        let row = ui::list_choice(format!("mc-{}", v.id), on)
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
                                .child(v.id.clone()),
                            )
                            .child(
                                ui::card_note(format!("{} · {}", tr(kind), released(&v.released)))
                                    .flex_1()
                                    .min_w_0()
                                    .truncate(),
                            )
                            .child(ui::tick_slot(on));
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
                        ui::scroll_list_to("mc-versions", at);
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
            let logo = if instance_icons::has_plate(loader.id()) {
                self.loader_logo(loader.id(), ui::TILE_LG)
                    .map(|image| ui::image_tile(image, ui::TILE_LG))
            } else {
                self.loader_logo(loader.id(), ui::MARK)
                    .map(|image| ui::mark_tile(ui::image_tile(image, ui::MARK)).into_any_element())
            };
            let logo = logo.unwrap_or_else(|| {
                ui::letter_tile(white(fill::BUTTON), loader.name(), ui::TILE_LG).into_any_element()
            });
            let note = match loader.id() {
                "fabric" => tr("Most mods for new versions"),
                "quilt" => tr("Fabric mods run here too"),
                "neoforge" => tr("The big modpacks' home"),
                "forge" => tr("Mods and packs for older versions"),
                _ => "",
            };
            let card = ui::choice_card(format!("loader-{}", loader.id()), loader == now)
                .items_center()
                .gap(px(space::LG))
                .p(px(space::MD))
                .min_w_0()
                .child(logo)
                .child(ui::item_text_wrapped(
                    loader.name(),
                    note,
                    instance_page::CHOICE_NOTE_LINES,
                ));
            cards.push((card, SharedString::from(loader.name()), loader == now));
        }
        let this = cx.entity().downgrade();
        let grid = ui::options(
            div()
                .id("instance-loader-choices")
                .grid()
                .grid_cols(2)
                .gap(px(space::MD)),
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

    /// Before Save switches what `editing` plays: what can't keep working. Nothing in its folder
    /// is deleted or changed.
    pub(super) fn switch_notice(&self, editing: &Instance) -> Option<Div> {
        let to = self.draft_plays(editing)?;
        if to == Plays::of(editing) {
            return None;
        }
        let eclipse = backend::game_version(&self.draft_eclipse().unwrap_or_default());
        let lines: Vec<String> = instances::unkept(editing, &to, &eclipse)
            .into_iter()
            .map(|u| match u {
                Unkept::Mods { made_for, .. } => trf(
                    "Its mods made for {loader} stay in its folder but won't load.",
                    &[("loader", &made_for)],
                ),
                Unkept::ModsVersion { from, to, .. } => trf(
                    "Its mods made for Minecraft {from} stay in its folder; some may not load in {to}.",
                    &[("from", &from), ("to", &to)],
                ),
                Unkept::Worlds { from, to, .. } => trf(
                    "Its worlds were saved by Minecraft {from}: {to} upgrades them for good, and can't open them if it is older.",
                    &[("from", &from), ("to", &to)],
                ),
            })
            .collect();
        if lines.is_empty() {
            return None;
        }
        let said = div()
            .flex()
            .flex_col()
            .gap(px(space::SM))
            .min_w_0()
            .child(ui::card_title(tr("Before you switch")))
            .children(lines.into_iter().map(|l| {
                ui::text(
                    eclipse_theme::glass::step::NOTE,
                    FontWeight::NORMAL,
                    ink::BODY,
                )
                .child(l)
            }));
        Some(ui::notice(ui::Notice::Warn, "information-circle", said, vec![]).flex_none())
    }
}
