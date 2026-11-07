//! The Mods page's look: the kinds as tabs, the search and its results, the picked project's card.
//! A kind's sources lead the search: Eclipse's registry or Modrinth, on the kinds both
//! serve (`Kind::picks_source`); shader packs are Eclipse's registry's only, and a Java instance's files
//! always Modrinth's. Modrinth's Java mods are offered to an Eclipse instance too, each marked
//! Works or Partly in Eclipse; those that don't run show only when asked.

use crate::fields::Field;
use crate::menus::{Asking, Menu};
use crate::mods_state::{
    count, goes_to_trash, kind_words, licence_name, removal, DETAIL_FLOAT, KINDS, SOURCES,
};
use crate::page::Page;
use crate::tab_strip::{Strip, StripTab};
use crate::ui::{self, ButtonKind, Press};
use crate::{instance_state, layout, menus, mods_project, Launcher};
use eclipse_launcher::browser::{Kind as PackKind, Project, SourceId, Support};
use eclipse_launcher::i18n::{tr, trf};
use eclipse_launcher::instances::{Instance, Kind as InstanceKind};
use eclipse_launcher::worlds::World;
use eclipse_launcher::{browser, native_status};
use eclipse_theme::glass::{ink, radius, space};
use gpui::prelude::*;
use gpui::{div, px, AnyElement, ClickEvent, Div, FontWeight, SharedString, Stateful};

const DETAIL: f32 = 360.;
const VANILLA_ACTION: ui::ButtonSize = ui::ButtonSize::Md;

const DETAIL_CLOSE: &str = "mods-detail-close";
const FLOAT_MIN_HEIGHT: f32 = ui::TILE_LG + 2. * ui::CARD_PADDING;
const SEARCH_FIELD_MIN: f32 = 200.;

pub(super) const RESULTS: &str = "mods-results";
const TOOLBAR_INSET: f32 = ui::CARD_PADDING - ui::LIST_INSET;
const ROW_PLATE_INSET: f32 = space::MD - ui::LIST_INSET;
const ROW_CONTENT_INSET: f32 = ui::CARD_PADDING - space::MD - ui::ROW_EDGE;

impl Launcher {
    /// The search's placeholder: the registry's count when it is listed whole.
    pub(super) fn mods_placeholder(&self) -> SharedString {
        let m = &self.mods;
        let registry = m.source == SourceId::Eclipse && m.java.is_none();
        if registry
            && m.kind == PackKind::Mod
            && m.searched
            && !m.searching
            && m.error.is_none()
            && !m.results.is_empty()
            && mods_project::active_filters(&m.browse.filters) == 0
            && self.fields.value(Field::ModsSearch).is_empty()
        {
            // Results come in pages: only a last page gives the real count.
            if m.more {
                return tr("Search Eclipse mods").into();
            }
            trf(
                "Search {count} Eclipse mods",
                &[("count", &m.results.len().to_string())],
            )
            .into()
        } else {
            tr(kind_words(m.kind).search).into()
        }
    }

    pub(super) fn mods_view(&mut self, cx: &mut Context<Self>) -> Div {
        let instance = self.target_name();
        let target = self.mods_target(cx);
        let open = self.mods.kind;
        let kinds = KINDS
            .into_iter()
            .map(|kind| StripTab {
                value: kind,
                label: tr(kind_words(kind).tab).into(),
                count: None,
            })
            .collect();
        let (tabs, panel) = self.tab_strip(
            Strip::Mods,
            kinds,
            open,
            layout::content_width(self.window_width),
            |this, kind, _| this.mods_set_kind(kind),
            cx,
        );
        let header = div()
            .flex()
            .flex_col()
            .child(ui::page_head_with(
                Page::Mods.title(),
                None,
                target.map(IntoElement::into_any_element),
            ))
            .child(tabs);

        // Installation can pause for consent in either view. Keep the notice before
        // the project or results, rather than below a potentially long detail card.
        let notice = self
            .native_notice(cx)
            .map(|notice| notice.mb(px(layout::CARD_GAP)));
        // A project's own page replaces the two cards.
        if let Some(project) = self.mods_project_page(cx) {
            return div().flex().flex_col().child(header).child(
                panel.flex().flex_col().children(notice).child(
                    ui::card()
                        .p(px(ui::CARD_PADDING))
                        .children(self.mods_busy_plate())
                        .child(project),
                ),
            );
        }

        let java = self.mods.java_for(self.mods.kind).is_some();
        let registry = self.mods.source == SourceId::Eclipse && !java;
        let everything = self.mods.everything;
        let other_versions = !registry && !java && self.mods.kind != PackKind::Modpack;
        let versions = other_versions.then(|| {
            ui::toggle_label(
                "mods-everything",
                tr("Include other Minecraft versions"),
                everything,
            )
            .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.mods.everything = !this.mods.everything;
                this.mods_search(0);
                cx.notify();
            }))
        });
        let unrunnable = self.mods.may_hide_unrunnable().then(|| {
            ui::toggle_label(
                "mods-show-unrunnable",
                tr("Show mods that don't run in Eclipse"),
                self.mods.show_unrunnable,
            )
            .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.mods.show_unrunnable = !this.mods.show_unrunnable;
                cx.notify();
            }))
        });
        let toggles = (versions.is_some() || unrunnable.is_some()).then(|| {
            div()
                .flex()
                .flex_wrap()
                .gap_x(px(space::XL))
                .gap_y(px(space::MD))
                .children(versions)
                .children(unrunnable)
        });
        let extras = div()
            .flex()
            .flex_col()
            .gap(px(space::MD))
            .px(px(TOOLBAR_INSET))
            .pb(px(space::SM))
            .children(toggles)
            .children(self.mods_filters(cx))
            .children(self.mods_target_line(cx));
        let source = (self.mods.kind.picks_source() && !java).then(|| self.source_switch(cx));
        let search = div()
            .mx(px(TOOLBAR_INSET))
            .mt(px(TOOLBAR_INSET))
            .mb(px(space::SM))
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(space::SM))
            .children(source)
            .child(
                div()
                    .flex_1()
                    .min_w(px(SEARCH_FIELD_MIN))
                    .child(self.mods_field(cx)),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(space::SM))
                    .child(self.mods_sort(cx))
                    .child(self.mods_filters_button(cx)),
            );
        let list = ui::card()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .p(px(ui::LIST_INSET))
            .child(search)
            .child(extras);
        let (rows, more) = self.mods_list(cx);
        // The card stays in view as the list scrolls (`sticky.rs`).
        let wide = self.window_width >= crate::layout::MODS_DETAIL_BESIDE;
        let close = (!wide).then(|| {
            ui::icon_button(
                DETAIL_CLOSE,
                "cancel-01",
                tr("Close"),
                ButtonKind::Ghost,
                ui::ButtonSize::Sm,
                false,
            )
            .absolute()
            .top(px(space::LG))
            .right(px(space::LG))
            .when_some(ui::try_focus_for(DETAIL_CLOSE), |d, handle| {
                ui::keep_focus(d, &handle)
            })
            .on_press(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.close_mod_detail(window, cx);
                cx.notify();
            }))
        });
        let detail = if wide || self.mods.picked.is_some() {
            self.mod_detail(&instance, close, cx)
        } else {
            None
        };
        let floats = !wide && detail.is_some();
        let fresh = floats && self.mods.float_for != self.mods.picked;
        self.mods.float_for = self.mods.picked.clone().filter(|_| floats);
        let detail = detail.map(|d| {
            let down = self.mods_sticky.offset(&self.content_scroll.handle);
            if wide {
                return d
                    .flex_none()
                    .w(px(DETAIL))
                    .top(px(down))
                    .child(self.mods_sticky.panel())
                    .into_any_element();
            }
            let scroll = &self.mods.float_scroll;
            ui::float_card()
                .id(DETAIL_FLOAT)
                .when_some(ui::try_focus_for(DETAIL_FLOAT), |d, handle| {
                    d.track_focus(&handle.tab_stop(false))
                })
                .flex_none()
                .flex()
                .flex_col()
                .w(px(DETAIL))
                .max_h(px(self.mods_float_room(down)))
                .ml(px(-DETAIL))
                .top(px(down))
                .occlude()
                .child(
                    ui::hand_back_at_ends(ui::scroll_pane("mods-detail-scroll", scroll), scroll)
                        .min_h_0()
                        .child(d),
                )
                .children(
                    ui::more_below_fade(scroll, ui::surface_colour(ui::Surface::Float))
                        .map(|fade| fade.rounded_b(px(radius::POPOVER))),
                )
                .child(self.mods_sticky.panel())
                .child(self.mods_float_at(down, fresh, cx))
                .into_any_element()
        });
        let sticky = self.mods_sticky.row(&self.content_scroll.handle);
        let cards = if wide {
            div()
                .relative()
                .flex()
                .items_start()
                .gap(px(layout::CARD_GAP))
                .child(sticky)
                .child(list.child(rows).children(more))
                .children(detail)
        } else {
            list.child(
                div()
                    .relative()
                    .flex()
                    .items_start()
                    .child(sticky)
                    .child(rows.flex_1().min_w_0())
                    .children(detail),
            )
            .children(more)
        };

        let card = |inner: Div| ui::card().p(px(ui::CARD_PADDING)).child(inner);
        let packs = (self.mods.kind == PackKind::Modpack).then(|| {
            card(self.imported_packs(cx))
                .id("mods-imported-packs")
                .mb(px(layout::CARD_GAP))
        });
        let plate = self.mods_busy_plate();
        let installed = self.mods_installed_list(cx);
        let installed = (installed.is_some() || !plate.is_empty()).then(|| {
            card(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(space::LG))
                    .children(plate)
                    .children(installed),
            )
            .id("mods-installed")
            .mb(px(layout::CARD_GAP))
        });
        let github = (self.mods.kind == PackKind::Mod && self.mods.java.is_none())
            .then(|| card(self.github_panel(cx)).mt(px(layout::CARD_GAP)));
        let advice: Vec<_> = self
            .mods_vanilla_notice(cx)
            .or_else(|| self.mods_shader_notice(cx))
            .or_else(|| self.mods_world_notice(cx))
            .into_iter()
            .chain(self.mods_datapack_notice())
            .collect();
        let advice = (!advice.is_empty()).then(|| {
            ui::card()
                .flex()
                .flex_col()
                .gap(px(space::LG))
                .p(px(ui::CARD_PADDING))
                .mb(px(layout::CARD_GAP))
                .children(advice)
        });
        div().flex().flex_col().child(header).child(
            panel
                .flex()
                .flex_col()
                .children(notice)
                .children(advice)
                .children(packs)
                .children(installed)
                .child(cards)
                .children(github),
        )
    }

    fn mods_float_room(&self, down: f32) -> f32 {
        let (top, foot) = self.mods.float_at.get();
        let pane = &self.content_scroll.handle;
        let top = f32::from(pane.bounds().top() + pane.offset().y) + top + down;
        (foot - top - layout::CARD_GAP).max(FLOAT_MIN_HEIGHT)
    }

    fn mods_float_at(&self, down: f32, fresh: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let (at, detail, pane, this) = (
            self.mods.float_at.clone(),
            self.mods.float_scroll.clone(),
            self.content_scroll.handle.clone(),
            cx.entity().downgrade(),
        );
        gpui::canvas(
            move |bounds, window, _| {
                let foot = f32::from(window.viewport_size().height);
                let top = f32::from(bounds.top() - pane.bounds().top() - pane.offset().y) - down;
                let (was, was_foot) = at.replace((top, foot));
                let room = foot - f32::from(bounds.top()) - layout::CARD_GAP;
                let short = match fresh {
                    true => {
                        let whole = f32::from(detail.bounds().size.height + detail.max_offset().y);
                        let most = foot - f32::from(pane.bounds().top()) - layout::CARD_GAP;
                        let above = f32::from(bounds.top() - pane.bounds().top());
                        (whole.min(most) - room).min(above)
                    }
                    false => 0.,
                };
                let moved = (was - top).abs() >= 1. || (was_foot - foot).abs() >= 1.;
                if moved || short >= 1. {
                    let pane = pane.clone();
                    window.on_next_frame(move |_, cx| {
                        if short >= 1. {
                            let at = pane.offset();
                            pane.set_offset(gpui::point(at.x, at.y - px(short)));
                        }
                        let _ = this.update(cx, |_, cx| cx.notify());
                    });
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
    }

    fn mods_target(&mut self, cx: &mut Context<Self>) -> Option<Div> {
        let row = div()
            .flex()
            .items_center()
            .gap(px(space::MD))
            .h(px(ui::ButtonSize::Md.height()));
        if self.mods.kind == PackKind::Modpack {
            let fixed = ui::labelled_dropdown(
                "mods-target",
                tr("Install to"),
                None,
                tr("New instance"),
                false,
                ui::ButtonSize::Md,
            );
            return Some(row.child(ui::unavailable(
                fixed,
                tr("Modpacks install as a new instance"),
            )));
        }
        let current = self.selected_instance()?.clone();
        let choices = self.mods_targets(&current);
        let open = self.menu_is(&Menu::ModsTarget);
        let button = ui::labelled_dropdown(
            "mods-target",
            tr("Install to"),
            Some(self.instance_icon(&current, ui::TILE_SM)),
            current.name.clone(),
            open,
            ui::ButtonSize::Md,
        );
        let button = match menus::opener_focus("mods-target") {
            Some(focus) => ui::keep_focus(button, &focus),
            None => button,
        };
        let button = self.menu_opener(button, Menu::ModsTarget, cx);
        let mut wrap = div().relative().flex_none().child(button);
        if open {
            let mut menu = self.menu_popover("mods-target-menu", tr("Install to"), cx);
            for i in choices {
                let picked = i.id == current.id;
                let game = match (&i.java, self.version_of(&i)) {
                    (Some(java), _) => java.line(),
                    (None, Some(tag)) => eclipse_launcher::nightly::display_name(tag),
                    (None, None) => tr("Eclipse").to_string(),
                };
                menu = menu.child(
                    ui::pick_item(
                        SharedString::from(format!("mods-target-pick-{}", i.id)),
                        i.name.clone(),
                        game,
                        picked,
                    )
                    .on_press(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            this.close_menu();
                            if !picked {
                                this.select_instance(&i);
                            }
                            cx.notify();
                        },
                    )),
                );
            }
            wrap = wrap.child(ui::menu_under(
                ui::ButtonSize::Md.height(),
                ui::Align::End,
                menu,
            ));
        }
        Some(row.child(wrap))
    }

    fn mods_targets(&self, current: &Instance) -> Vec<Instance> {
        let kind = self.mods.kind;
        self.instances
            .list
            .iter()
            .filter(|i| matches!(i.kind, InstanceKind::Default | InstanceKind::Own))
            .filter(|i| {
                i.id == current.id
                    || i.java.as_ref().is_none_or(|j| {
                        kind == PackKind::DataPack || j.target().folder(kind).is_ok()
                    })
            })
            .cloned()
            .collect()
    }

    fn mods_vanilla_notice(&mut self, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let minecraft = self.mods_vanilla_target()?;
        let current = self.selected_instance()?.clone();
        let name = [("instance", current.name.as_str())];
        let create = self.mods_new_fabric_button("mods-new-fabric", minecraft, cx);
        let add = ui::button(
            "mods-add-fabric",
            None,
            trf("Add Fabric to {instance}…", &name),
            ButtonKind::Secondary,
            VANILLA_ACTION,
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.mods_add_fabric(cx);
            cx.notify();
        }));
        let choose = self.mods_choose_instance_button("mods-choose-instance", cx);
        let said = match self.mods.kind {
            PackKind::Shader => trf(
                "{instance} is vanilla. Shader packs need Iris, which runs on Fabric.",
                &name,
            ),
            _ => trf(
                "{instance} is vanilla. Mods need Fabric, Quilt, NeoForge or Forge.",
                &name,
            ),
        };
        let actions = [create.into_any_element(), add.into_any_element()]
            .into_iter()
            .chain(choose)
            .collect();
        Some(div().id("mods-vanilla").child(ui::notice(
            ui::Notice::InCard,
            crate::icons::INFO,
            said,
            actions,
        )))
    }

    fn mods_choose_instance_button(
        &mut self,
        id: &'static str,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let current = self.selected_instance()?.clone();
        (self.mods_targets(&current).len() > 1).then(|| {
            ui::button(
                id,
                None,
                tr("Choose another instance"),
                ButtonKind::Secondary,
                VANILLA_ACTION,
            )
            .on_press(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.open_menu_for(Menu::ModsTarget, "mods-target", window, cx);
                cx.notify();
            }))
            .into_any_element()
        })
    }

    fn mods_shader_notice(&mut self, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let (slug, name) = self.mods_missing_shader_mod()?;
        let current = self.selected_instance()?.name.clone();
        let installing = self.mods.busy.as_ref().is_some_and(|b| b.installs(slug));
        let install = ui::button(
            "mods-install-shader-mod",
            None,
            match installing {
                true => tr("Installing…").to_string(),
                false => trf("Install {name}", &[("name", name)]),
            },
            ButtonKind::Secondary,
            VANILLA_ACTION,
        );
        let install = match self.mods_wait() {
            Some(why) => ui::unavailable(install, why),
            None => install.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.mods_install(SourceId::Modrinth, slug.to_string(), None);
                cx.notify();
            })),
        };
        Some(div().id("mods-shader-mod").child(ui::notice(
            ui::Notice::InCard,
            crate::icons::INFO,
            trf(
                "Shader packs need a shader mod like Iris, and {instance} has none.",
                &[("instance", &current)],
            ),
            vec![install.into_any_element()],
        )))
    }

    fn mods_world_notice(&mut self, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let worlds = self.worlds.list.of(&self.mods.home());
        if self.mods.kind != PackKind::DataPack || worlds.is_none_or(|w| !w.is_empty()) {
            return None;
        }
        let current = self.selected_instance()?.clone();
        let choose = self.mods_choose_instance_button("mods-world-choose-instance", cx);
        Some(div().id("mods-no-worlds").child(ui::notice(
            ui::Notice::InCard,
            crate::icons::INFO,
            trf(
                "{instance} has no worlds yet. Create one in the game first.",
                &[("instance", &current.name)],
            ),
            choose.into_iter().collect(),
        )))
    }

    fn mods_datapack_notice_shown(&self) -> bool {
        self.mods.kind == PackKind::DataPack
            && self.mods.source == SourceId::Modrinth
            && self.mods.java_for(PackKind::DataPack).is_none()
    }

    fn mods_datapack_notice(&self) -> Option<Stateful<Div>> {
        self.mods_datapack_notice_shown().then(|| {
            div().id("mods-datapack-support").child(ui::notice(
                ui::Notice::InCard,
                crate::icons::INFO,
                tr(browser::DATAPACK_SUPPORT),
                Vec::new(),
            ))
        })
    }

    /// The source switch (`Kind::picks_source`): Eclipse's registry or Modrinth.
    fn source_switch(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let this = cx.entity().downgrade();
        ui::segments_sized(
            "mods-source",
            tr("Search in"),
            &ui::translated(&SOURCES),
            self.mods.source,
            ui::ButtonSize::Md,
            move |source, _, cx| {
                let _ = this.update(cx, |this, cx| {
                    this.mods_set_source(source);
                    cx.notify();
                });
            },
        )
    }

    fn mods_sort(&mut self, cx: &mut Context<Self>) -> Div {
        let sorts = self.mods_sorts();
        let current = self.mods.browse.filters.sort;
        // A sort the source can't do (Follows, picked on Modrinth) reads as the default.
        let shown = sorts
            .iter()
            .find(|(value, _)| *value == current)
            .or(sorts.first())
            .map(|(value, _)| *value);
        let name = sorts
            .iter()
            .find(|(value, _)| Some(*value) == shown)
            .map_or("", |(_, label)| *label);
        let open = self.menu_is(&Menu::ModsSort);
        let button = ui::labelled_dropdown(
            "mods-sort",
            tr("Sort by"),
            None,
            tr(name),
            open,
            ui::ButtonSize::Md,
        );
        let button = self.menu_opener(button, Menu::ModsSort, cx);
        let mut wrap = div().relative().flex_none().child(button);
        if open {
            let mut menu = self.menu_popover("mods-sort-menu", tr("Sort by"), cx);
            for &(value, label) in &sorts {
                menu = menu.child(
                    ui::pick_row(
                        SharedString::from(format!("mods-sort-pick-{value:?}")),
                        tr(label).to_string(),
                        Some(value) == shown,
                    )
                    .on_press(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            this.close_menu();
                            this.mods_filter(|f| f.sort = value);
                            cx.notify();
                        },
                    )),
                );
            }
            wrap = wrap.child(ui::menu_under(
                ui::ButtonSize::Md.height(),
                ui::Align::End,
                menu,
            ));
        }
        wrap
    }

    /// The search's results as rows: a click picks one for the card beside.
    fn mods_list(&mut self, cx: &mut Context<Self>) -> (Div, Option<Div>) {
        let m = &self.mods;
        if let Some(e) = &m.error {
            let title = match (m.java_for(m.kind), m.source) {
                (None, SourceId::Eclipse) => tr(kind_words(m.kind).load_failed),
                _ => tr("Couldn't search Modrinth"),
            };
            let error = e.clone();
            let retry = match self.mods_vanilla_target() {
                Some(_) => ButtonKind::Secondary,
                None => ButtonKind::Primary,
            };
            let failed = div().child(ui::error_state_with(
                "mods-failed",
                title,
                &error,
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.mods_search(0);
                    cx.notify();
                }),
                vec![Self::proxy_settings_button("mods-failed", cx)],
                retry,
            ));
            return (failed, None);
        }
        if m.results.is_empty() {
            if m.searching || !m.searched {
                let searching = ui::loading_state("mods-searching", tr("Searching…"));
                return (div().child(searching), None);
            }
            let mut actions = Vec::new();
            let filtered = mods_project::active_filters(&m.browse.filters) > 0;
            let (title, note) = if let Some(target) = m.java_for(m.kind) {
                (
                    trf(
                        "Nothing found for {target}",
                        &[("target", &target.describe(m.kind))],
                    ),
                    tr("Try other words or another instance."),
                )
            } else if !self.fields.value(Field::ModsSearch).trim().is_empty() {
                (
                    trf(
                        "Nothing matches “{query}”",
                        &[("query", self.fields.value(Field::ModsSearch).trim())],
                    ),
                    tr("Try other words."),
                )
            } else if m.source == SourceId::Eclipse && !filtered {
                let title = tr(kind_words(m.kind).none_yet);
                if !m.kind.picks_source() {
                    (title.to_string(), tr("None have been published yet."))
                } else {
                    actions.push(
                        ui::button(
                            "mods-search-modrinth",
                            None,
                            tr("Search Modrinth"),
                            ButtonKind::Secondary,
                            ui::ButtonSize::Md,
                        )
                        .on_press(cx.listener(
                            |this, _: &ClickEvent, _, cx| {
                                this.mods_set_source(SourceId::Modrinth);
                                cx.notify();
                            },
                        )),
                    );
                    (title.to_string(), tr("Search Modrinth instead."))
                }
            } else if filtered || m.everything || m.kind == PackKind::Modpack {
                (
                    tr("Nothing found").to_string(),
                    tr("Try other words or clear the filters."),
                )
            } else {
                actions.push(
                    ui::button(
                        "mods-show-all",
                        None,
                        tr("Include other Minecraft versions"),
                        ButtonKind::Secondary,
                        ui::ButtonSize::Md,
                    )
                    .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.mods.everything = true;
                        this.mods_search(0);
                        cx.notify();
                    })),
                );
                (
                    tr("Nothing found that runs in Eclipse").to_string(),
                    tr("Some made for other Minecraft versions still run."),
                )
            };
            let empty = div().child(ui::empty_state(
                "mods-no-results",
                "search-01",
                title,
                note,
                actions,
            ));
            return (empty, None);
        }
        let shown: Vec<Project> = m.results.iter().filter(|p| m.listed(p)).cloned().collect();
        let checking = m.may_hide_unrunnable()
            && !m.show_unrunnable
            && m.results.iter().any(|p| m.checking(p));
        if shown.is_empty() && checking {
            let checking = div().child(ui::loading_state(
                "mods-checking",
                tr("Checking which mods run in Eclipse…"),
            ));
            return (checking, None);
        }
        if shown.is_empty() {
            let show = ui::primary(
                "mods-show-eclipse",
                None,
                tr("Show Eclipse mods"),
                ui::ButtonSize::Md,
            )
            .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.mods_set_source(SourceId::Eclipse);
                cx.notify();
            }));
            let more = m
                .more
                .then(|| self.mods_more_button(tr("Check more mods"), cx));
            let none = div().child(ui::empty_state(
                "mods-none-run",
                "search-01",
                tr("Nothing found that runs in Eclipse"),
                tr("Most Modrinth mods are made for Fabric or Forge."),
                std::iter::once(show).chain(more).collect(),
            ));
            return (none, None);
        }
        let picked = if self.window_width >= crate::layout::MODS_DETAIL_BESIDE
            || self.mods.picked.is_some()
        {
            self.mods_picked().map(|p| p.id.clone())
        } else {
            None
        };
        let advised =
            self.mods_vanilla_target().is_some() || self.mods_missing_shader_mod().is_some();
        let said_above = self.mods_datapack_notice_shown();
        let cursor = ui::options_cursor(RESULTS);
        let keyboard = ui::options_has_keyboard(RESULTS);
        let floats = self.mod_detail_floats();
        let instance = self.target_name();
        let mut rows = Vec::new();
        let mut ids = Vec::new();
        for (i, p) in shown.into_iter().enumerate() {
            let icon = self.mods_icon(&p, ui::TILE_MD);
            let m = &self.mods;
            let installed = m.entry(&p).is_some();
            let busy = m.busy.as_ref().is_some_and(|b| b.installs(&p.id));
            let checking = m.checking(&p);
            let row_id = SharedString::from(format!("mods-row-{}", p.id));
            let current = picked.as_deref() == Some(p.id.as_str());
            let install_label = tr("Install");
            let action_room = ui::button_width(
                install_label,
                true,
                ButtonKind::Secondary,
                ui::ButtonSize::Sm,
            );
            let state = |tone: ui::Tone, said: &str| {
                ui::card_note(ui::state_line(tone, said.to_string())).into_any_element()
            };
            let action: AnyElement = if installed {
                state(ui::Tone::Off, tr("Installed"))
            } else if busy {
                state(ui::Tone::Busy, tr("Installing…"))
            } else if checking {
                state(ui::Tone::Busy, tr("Checking…"))
            } else if m.support(&p).installable() {
                let known = p.clone();
                let button_id = SharedString::from(format!("mods-row-install-{}", p.id));
                let button = ui::button(
                    button_id,
                    Some(crate::icons::DOWNLOAD),
                    install_label,
                    ButtonKind::Secondary,
                    ui::ButtonSize::Sm,
                );
                let shown = current || (keyboard && i == cursor) || ui::hover_within(&row_id);
                let why = self.install_refusal(&p);
                let button = match why {
                    Some(why) => ui::unavailable(button, why),
                    None => button.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        this.mods_install(known.source, known.id.clone(), Some(known.clone()));
                        cx.notify();
                    })),
                };
                let button = button.when(i != cursor || floats, ui::skipped_by_tab);
                ui::shown_on_focus(button, shown).into_any_element()
            } else {
                div().into_any_element()
            };
            let action = div()
                .flex()
                .flex_none()
                .justify_end()
                .min_w(px(action_room))
                .child(action);
            // The registry's rows end with downloads, the others' with how they run.
            let right = if p.source == SourceId::Eclipse {
                Some(
                    ui::text(
                        eclipse_theme::glass::step::NOTE,
                        FontWeight::NORMAL,
                        ink::DIM,
                    )
                    .child(count(p.downloads).to_uppercase()),
                )
            } else {
                let support = m.support(&p);
                let said = (p.kind != PackKind::Modpack)
                    .then(|| self.mods_status(&p, &instance))
                    .flatten()
                    .map(|(_, said, _)| said);
                let noticed = said_above && support.reason() == Some(browser::DATAPACK_SUPPORT);
                let said =
                    said.filter(|_| !advised && !noticed && support != Support::Works && !checking);
                let quiet = advised || !support.installable();
                said.map(|said| {
                    ui::card_note(ui::state_line(
                        if quiet { ui::Tone::Off } else { ui::Tone::Warn },
                        said,
                    ))
                })
            };
            ids.push(p.id.clone());
            let by = p.author.clone();
            let summary = p.summary.split_whitespace().collect::<Vec<_>>().join(" ");
            rows.push((
                ui::row(row_id, p.title.clone(), current)
                    .px(px(ROW_CONTENT_INSET))
                    .aria_description(summary.clone())
                    .child(div().relative().child(icon))
                    .child(ui::item_text_by(p.title.clone(), by, summary))
                    .children(right.map(|r| r.flex_none()))
                    .child(action),
                SharedString::from(p.title.clone()),
                current,
            ));
        }
        let open = cx.listener(move |this, i: &usize, window, cx| {
            this.mods.picked = ids.get(*i).cloned();
            this.mods
                .float_scroll
                .set_offset(gpui::point(px(0.), px(0.)));
            this.asking = None;
            if this.mod_detail_floats() && window.last_input_was_keyboard() {
                cx.on_next_frame(window, |_, window, cx| {
                    if let Some(close) = ui::try_focus_for(DETAIL_CLOSE) {
                        window.focus(&close, cx);
                    }
                });
            }
            cx.notify();
        });
        let more = self.mods.more.then(|| {
            div()
                .flex()
                .px(px(TOOLBAR_INSET))
                .py(px(space::SM))
                .child(self.mods_more_button(tr("Load more"), cx))
        });
        let still = checking.then(|| {
            div()
                .px(px(TOOLBAR_INSET))
                .py(px(space::SM))
                .child(ui::progress_line(
                    "",
                    None,
                    tr("Checking which mods run in Eclipse…"),
                ))
        });
        let list = div()
            .flex()
            .flex_col()
            .child(ui::options(
                div().id(RESULTS).flex().flex_col().px(px(ROW_PLATE_INSET)),
                tr("Results"),
                rows,
                false,
                move |i, window, cx| open(&i, window, cx),
            ))
            .children(still);
        (list, more)
    }

    fn mods_more_button(&self, label: &'static str, cx: &mut Context<Self>) -> Stateful<Div> {
        let searching = self.mods.searching;
        let offset = self.mods.results.len();
        ui::button(
            "mods-more",
            None,
            if searching { tr("Loading…") } else { label },
            ButtonKind::Secondary,
            ui::ButtonSize::Md,
        )
        .when(searching, ui::disabled)
        .when(!searching, |d| {
            d.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.mods_search(offset);
                ui::focus_option(RESULTS, offset);
                cx.notify();
            }))
        })
    }

    /// The picked result, else the first; when a search finds nothing, the card shown last
    /// stays.
    fn mods_picked(&mut self) -> Option<Project> {
        let m = &mut self.mods;
        let found = m
            .picked
            .as_ref()
            .and_then(|id| m.results.iter().find(|p| &p.id == id))
            .or(m.results.iter().find(|p| m.listed(p)))
            .cloned();
        match found {
            Some(p) => {
                m.shown = Some(p.clone());
                Some(p)
            }
            None if m.searched && !m.searching && m.error.is_none() => m
                .shown
                .clone()
                .filter(|p| p.kind == m.kind && p.source == m.source),
            None => None,
        }
    }

    fn mod_detail(
        &mut self,
        instance: &str,
        close: Option<Stateful<Div>>,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let p = self.mods_picked()?;
        let icon = self.mods_icon(&p, ui::TILE_LG);
        let m = &self.mods;
        let support = m.support(&p);
        let entry = m.entry(&p).cloned();
        let busy = m.busy.clone();
        let checking = m.checking(&p);
        let mut by = Vec::new();
        if !p.author.is_empty() {
            by.push(trf("by {author}", &[("author", &p.author)]));
        }
        if let Some(source) = p.source.shown_for(&p.id) {
            by.push(trf("from {source}", &[("source", source)]));
        }
        if !p.license.is_empty() {
            by.push(licence_name(&p.license));
        }
        let said_above = self.mods_datapack_notice_shown()
            && support.reason() == Some(browser::DATAPACK_SUPPORT);
        let status = self.status_shown(&p, instance).filter(|_| !said_above);
        let status = status.map(|(tone, said, reason)| {
            div()
                .mt(px(space::LG))
                .flex()
                .flex_col()
                .gap(px(space::XS))
                .child(ui::state_title(tone, said))
                .children(reason.map(|r| {
                    ui::text(
                        eclipse_theme::glass::step::NOTE,
                        FontWeight::NORMAL,
                        ink::DIM,
                    )
                    .child(r)
                }))
        });
        let about = m
            .about
            .get(&p.id)
            .filter(|_| p.source == SourceId::Eclipse)
            .map(|lines| lines.join(" · "));

        let button: Option<AnyElement> = if let Some(e) = &entry {
            let confirming = self.is_asking(&Asking::Mod(e.project.clone()));
            let remove_id = SharedString::from(format!("mods-detail-remove-{}", p.id));
            let id = e.project.clone();
            let trash = goes_to_trash(e.kind);
            if confirming {
                let asked = removal(e.kind, &e.title);
                ui::confirm(
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
                )
                .into_any_element()
                .into()
            } else {
                let ask = match trash {
                    true => trf("Delete from {instance}…", &[("instance", instance)]),
                    false => trf("Remove from {instance}…", &[("instance", instance)]),
                };
                let short = if trash {
                    tr("Delete…")
                } else {
                    tr("Remove…")
                };
                let ask = fitted(ask, short, false, ButtonKind::Secondary);
                ui::button(
                    remove_id,
                    None,
                    ask,
                    ButtonKind::Secondary,
                    ui::ButtonSize::Lg,
                )
                .w_full()
                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.asking = Some(Asking::Mod(id.clone()));
                    cx.notify();
                }))
                .into_any_element()
                .into()
            }
        } else if busy.as_ref().is_some_and(|b| b.installs(&p.id)) || checking {
            ui::disabled(
                ui::primary(
                    SharedString::from(format!("mods-detail-install-{}", p.id)),
                    Some(crate::icons::DOWNLOAD),
                    if checking {
                        tr("Checking…")
                    } else {
                        tr("Installing…")
                    },
                    ui::ButtonSize::Lg,
                )
                .w_full(),
            )
            .into_any_element()
            .into()
        } else if support.installable() {
            let short = match p.kind {
                PackKind::Modpack => tr("Install as instance"),
                _ => tr("Install"),
            };
            let label = Self::install_label(&p, instance);
            let label = fitted(label, short, true, ButtonKind::Primary);
            let known = p.clone();
            let why = self.install_refusal(&p);
            let install = ui::primary(
                SharedString::from(format!("mods-detail-install-{}", p.id)),
                Some(crate::icons::DOWNLOAD),
                label,
                ui::ButtonSize::Lg,
            )
            .w_full();
            match why {
                Some(why) => ui::unavailable(install, why),
                None => install.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.mods_install(known.source, known.id.clone(), Some(known.clone()));
                    cx.notify();
                })),
            }
            .into_any_element()
            .into()
        } else {
            None
        };
        let open = p.clone();
        Some(
            ui::card()
                .flex()
                .flex_col()
                .p(px(ui::CARD_PADDING))
                .children(close)
                .child(icon)
                .child(
                    ui::entity_title(
                        p.title.clone(),
                        eclipse_theme::glass::pixel_text::CARD_TITLE,
                    )
                    .mt(px(space::XL)),
                )
                .child(
                    ui::text(
                        eclipse_theme::glass::step::NOTE,
                        FontWeight::NORMAL,
                        ink::DIM,
                    )
                    .mt(px(space::XS))
                    .child(by.join(" · ")),
                )
                .child(
                    ui::text(
                        eclipse_theme::glass::step::BODY,
                        FontWeight::NORMAL,
                        ink::BODY,
                    )
                    .mt(px(space::LG))
                    .child(p.summary.clone()),
                )
                .children(about.map(|a| {
                    ui::text(
                        eclipse_theme::glass::step::NOTE,
                        FontWeight::NORMAL,
                        ink::DIM,
                    )
                    .mt(px(space::LG))
                    .child(a)
                }))
                .children(status)
                .child(
                    ui::text(
                        eclipse_theme::glass::step::NOTE,
                        FontWeight::NORMAL,
                        ink::DIM,
                    )
                    .mt(px(space::LG))
                    .child(trf(
                        "{count} downloads",
                        &[("count", &super::thousands(p.downloads))],
                    )),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(space::SM))
                        .mt(px(space::XL))
                        .children(button)
                        .child(
                            ui::button(
                                "mods-page-link",
                                None,
                                tr(kind_words(p.kind).open_page),
                                ButtonKind::Secondary,
                                ui::ButtonSize::Md,
                            )
                            .w_full()
                            .on_press(cx.listener(
                                move |this, _: &ClickEvent, _, cx| {
                                    this.mods_open(open.clone());
                                    cx.notify();
                                },
                            )),
                        ),
                ),
        )
    }
}

fn fitted(label: String, short: &str, icon: bool, kind: ButtonKind) -> String {
    let room = DETAIL - 2. * ui::CARD_PADDING;
    match ui::button_width(&label, icon, kind, ui::ButtonSize::Lg) <= room {
        true => label,
        false => short.to_string(),
    }
}

impl Launcher {
    pub(super) fn mods_field(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let placeholder = self.mods_placeholder();
        self.field_with(
            Field::ModsSearch,
            Some(ui::field_icon("search-01")),
            placeholder,
            layout::FIELD_XL,
            ui::ButtonSize::Md,
            None,
            cx,
        )
        .w_full()
    }

    pub(super) fn mods_page(&mut self, cx: &mut Context<Self>) -> Div {
        self.mods_load();
        if self.mods.kind == PackKind::DataPack {
            self.load_worlds(&self.mods.home(), cx);
        }
        if !self.mods.asked {
            self.mods_search(0);
        }
        self.mods_view(cx)
    }

    pub(super) fn mods_target_line(&self, cx: &mut Context<Self>) -> Option<Div> {
        let worlds = self.worlds.list.of(&self.mods.home())?;
        (self.mods.kind == PackKind::DataPack && !worlds.is_empty())
            .then(|| self.mods_world_picker(worlds, cx))
    }

    /// The world data packs go into: one of the instance's, picked here.
    fn mods_world_picker(&self, worlds: &[World], cx: &mut Context<Self>) -> Div {
        let note = |s: String| ui::card_note(s);
        let mut chips = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(space::SM))
            .child(note(tr("World").to_string()).mr(px(space::SM)));
        let picked = self.datapack_world().map(|w| w.path.clone());
        for w in worlds {
            let on = picked.as_ref() == Some(&w.path);
            let id = SharedString::from(format!("mods-world-{}", w.folder));
            let path = w.path.clone();
            chips = chips.child(
                ui::filter_chip(id, w.name.clone(), on).on_press(cx.listener(
                    move |this, _: &ClickEvent, _, cx| {
                        this.mods.world = Some(path.clone());
                        cx.notify();
                    },
                )),
            );
        }
        chips
    }

    /// The one-time notice before the first native mod is installed: native mods aren't
    /// sandboxed, and what the build takes. Its button installs the mod.
    pub(super) fn native_notice(&self, cx: &mut Context<Self>) -> Option<Div> {
        self.mods.native_notice.as_ref()?;
        let cancel = ui::button(
            "native-notice-cancel",
            None,
            tr("Cancel"),
            ButtonKind::Ghost,
            ui::ButtonSize::Md,
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.mods.native_notice = None;
            cx.notify();
        }));
        let install = ui::button(
            "native-notice-install",
            Some(crate::icons::DOWNLOAD),
            tr("Install"),
            ButtonKind::Secondary,
            ui::ButtonSize::Md,
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.settings.native_mods_noticed = true;
            this.save_settings();
            if let Some((source, id, known)) = this.mods.native_notice.take() {
                this.mods_install(source, id, known);
            }
            cx.notify();
        }));
        Some(ui::card().p(px(ui::LIST_INSET)).child(ui::notice(
            ui::Notice::Warn,
            "alert-circle",
            tr(native_status::NOTICE),
            vec![cancel.into_any_element(), install.into_any_element()],
        )))
    }

    pub(super) fn mods_new_fabric_button(
        &self,
        id: &'static str,
        minecraft: String,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        ui::primary(
            id,
            Some(crate::icons::ADD),
            tr("Create a Fabric instance…"),
            ui::ButtonSize::Md,
        )
        .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.open_instance_page(None);
            this.pick_kind(instance_state::NewKind::Modded, cx);
            this.change(|j| {
                j.loader = eclipse_launcher::loaders::Kind::Fabric;
                j.version = Some(minecraft.clone());
            });
            cx.notify();
        }))
    }
}
