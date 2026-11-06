//! A Java instance's part of the New instance page (`glass_instance.rs`): Mojang's versions to
//! pick from, and for Modded the loaders, each with its logo, as many as `loaders::Kind` knows.
use super::*;
use crate::glass_ui::{self as ui, white};
use eclipse_launcher::instances::{self, Instance, Plays, Unkept};
use eclipse_launcher::loaders::Loader;
use eclipse_launcher::minecraft_versions::Kind as VersionKind;
use eclipse_theme::glass::{fill, ink, radius};
use glass_instance::{
    choice, list_box, list_choice, loaders, mark_tile, section_card, tick, MARK, MARK_TILE,
};
use new_instance::{JavaChoice, LoaderVersions, NewKind, VersionList};
use servers_page::Field;

/// Mojang versions listed at most: the search finds the others.
const ROWS: usize = 60;
const LIST: f32 = 206.;
const LIST_SHORT: f32 = 154.;

fn released(time: &str) -> String {
    let date = time.get(..10).unwrap_or_default();
    let day = glass_versions::published(date, "");
    let this_year = chrono::Utc::now().format("%Y").to_string();
    match date.get(..4) {
        Some(year) if year != this_year => trf("{day}, {year}", &[("day", &day), ("year", year)]),
        _ => day,
    }
}

impl Launcher {
    pub(super) fn minecraft_versions(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let Some(choice) = self.instances.draft.as_ref().map(|d| d.java.clone()) else {
            return div().id("instance-minecraft-version");
        };
        let typed = self
            .servers
            .value(Field::InstanceVersion)
            .trim()
            .to_string();
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
            .gap(px(20.))
            .child(filter(
                "mc-snapshots",
                "Show snapshots",
                choice.snapshots,
                |j| j.snapshots = !j.snapshots,
            ))
            .child(filter("mc-old", "Show old versions", choice.old, |j| {
                j.old = !j.old
            }));
        let field = div()
            .relative()
            .mb(px(8.))
            .child(
                self.field(Field::InstanceVersion, tr("Search versions"), 600., cx)
                    .w_full()
                    .pl(px(44.)),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left(px(16.))
                    .h_full()
                    .flex()
                    .items_center()
                    .child(ui::icon("search-01", ui::IconSize::Px16, white(ink::DIM))),
            );
        let height = if ui::short_window() { LIST_SHORT } else { LIST };
        let list = match &self.instances.versions {
            VersionList::NotAsked | VersionList::Loading => list_box("mc-versions", height).child(
                ui::loading_state("mc-versions-loading", tr("Loading Minecraft versions…")),
            ),
            VersionList::Failed(e) => {
                let retry = ui::primary("mc-versions-retry", tr("Try again"), ui::Size::Md)
                    .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.instances.versions = VersionList::NotAsked;
                        this.load_versions(cx);
                        cx.notify();
                    }));
                list_box("mc-versions", height).child(ui::error_state(
                    "mc-versions-failed",
                    tr("Couldn't load Minecraft versions"),
                    e,
                    retry,
                ))
            }
            VersionList::Ready(versions) => {
                let rows: Vec<_> = versions
                    .of(choice.snapshots, choice.old)
                    .filter(|v| search.is_empty() || v.id.to_lowercase().contains(&search))
                    .take(ROWS)
                    .map(|v| {
                        let on = chosen.as_deref() == Some(v.id.as_str());
                        let id = v.id.clone();
                        let kind = match v.kind {
                            VersionKind::Release => "Release",
                            VersionKind::Snapshot => "Snapshot",
                            VersionKind::OldBeta => "Beta",
                            VersionKind::OldAlpha => "Alpha",
                        };
                        list_choice(format!("mc-{}", v.id), on)
                            .py(px(8.))
                            .child(
                                ui::text(
                                    eclipse_theme::glass::step::LEAD,
                                    FontWeight::SEMIBOLD,
                                    ink::FULL,
                                )
                                .w(px(160.))
                                .flex_none()
                                .truncate()
                                .child(v.id.clone()),
                            )
                            .child(
                                ui::text(
                                    eclipse_theme::glass::step::NOTE,
                                    FontWeight::NORMAL,
                                    ink::MUTED,
                                )
                                .flex_1()
                                .child(format!(
                                    "{} · {}",
                                    tr(kind),
                                    released(&v.released)
                                )),
                            )
                            .child(tick(on))
                            .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.change(|j| j.version = Some(id.clone()));
                                cx.notify();
                            }))
                    })
                    .collect();
                if rows.is_empty() {
                    list_box("mc-versions", height).child(ui::empty_state(
                        "mc-versions-no-match",
                        "search-01",
                        trf("No version matches “{search}”", &[("search", &typed)]),
                        "",
                        vec![],
                    ))
                } else {
                    list_box("mc-versions", height).children(rows)
                }
            }
        };
        section_card(
            "instance-minecraft-version",
            tr("Minecraft version").into(),
            Some(filters.into_any_element()),
            div().flex().flex_col().child(field).child(list),
        )
    }

    pub(super) fn loader_choice(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let now = self.draft_loader();
        let version = self.loader_version_shown(cx);
        let mut grid = div().grid().grid_cols(2).gap(px(10.));
        for loader in loaders() {
            let logo = match self.loader_logo(loader.id(), MARK) {
                Some(image) => mark_tile(ui::image_tile(image, MARK)),
                None => ui::letter_tile(white(fill::BUTTON), loader.name(), MARK_TILE),
            };
            let title = match &version {
                Some(v) if loader == now => format!("{} {v}", loader.name()),
                _ => loader.name().to_string(),
            };
            let note = match loader.id() {
                "fabric" => tr("Light and fast to update"),
                "quilt" => tr("Fabric mods run here too"),
                "neoforge" => tr("The big modpacks' home"),
                "forge" => tr("For older packs"),
                _ => "",
            };
            grid = grid.child(
                choice(format!("loader-{}", loader.id()), loader == now)
                    .items_center()
                    .gap(px(14.))
                    .p(px(12.))
                    .min_w_0()
                    .child(logo)
                    .child(ui::item_text(title, note))
                    .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.change(|j| j.loader = loader);
                        cx.notify();
                    })),
            );
        }
        section_card("instance-loader", tr("Mod loader").into(), None, grid)
    }

    fn loader_version_shown(&mut self, cx: &mut Context<Self>) -> Option<String> {
        let minecraft = self.draft_minecraft()?;
        self.ask_loader_versions(&minecraft, cx);
        let d = self.instances.draft.as_ref()?;
        let editing = d
            .editing
            .as_ref()
            .and_then(|id| self.instances.list.iter().find(|i| &i.id == id));
        if let Some(Plays::Java {
            loader: Some(l), ..
        }) = editing.and_then(|i| self.draft_plays(i))
        {
            if !l.version.is_empty() {
                return Some(l.version);
            }
        }
        match &d.java.loader_versions {
            LoaderVersions::Ready(_, _, listed) => d.java.loader_version.clone().or_else(|| {
                listed
                    .iter()
                    .find(|l| l.stable)
                    .or(listed.first())
                    .map(|l| l.version.clone())
            }),
            _ => None,
        }
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
        let warn = ui::warn();
        Some(
            div()
                .flex()
                .flex_none()
                .gap(px(12.))
                .p(px(16.))
                .rounded(px(radius::POPOVER))
                .bg(warn.opacity(fill::SELECTED))
                .child(ui::icon("information-circle", ui::IconSize::Px16, warn))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .min_w_0()
                        .child(
                            ui::text(
                                eclipse_theme::glass::step::LEAD,
                                FontWeight::SEMIBOLD,
                                ink::FULL,
                            )
                            .child(tr("Before you switch")),
                        )
                        .children(lines.into_iter().map(|l| {
                            ui::text(
                                eclipse_theme::glass::step::NOTE,
                                FontWeight::NORMAL,
                                ink::BODY,
                            )
                            .child(l)
                        })),
                ),
        )
    }
}
