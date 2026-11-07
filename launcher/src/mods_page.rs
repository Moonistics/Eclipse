//! The Mods page's look: the kinds as tabs and the search.
//! A kind's sources lead the search: Eclipse's registry or Modrinth, on the kinds both
//! serve (`Kind::picks_source`); shader packs are Eclipse's registry's only, and a Java instance's files
//! always Modrinth's. Modrinth's Java mods are offered to an Eclipse instance too, each marked
//! Works or Partly in Eclipse; those that don't run show only when asked.

use crate::fields::Field;
use crate::menus::Menu;
use crate::mods_state::{kind_words, DETAIL_FLOAT, KINDS, SOURCES};
use crate::page::Page;
use crate::tab_strip::{Strip, StripTab};
use crate::ui::{self, ButtonKind, Press};
use crate::{instance_state, layout, menus, mods_project, Launcher};
use eclipse_launcher::browser::{Kind as PackKind, SourceId};
use eclipse_launcher::i18n::{tr, trf};
use eclipse_launcher::instances::{Instance, Kind as InstanceKind};
use eclipse_launcher::worlds::World;
use eclipse_launcher::{browser, native_status};
use eclipse_theme::glass::{radius, space};
use gpui::prelude::*;
use gpui::{div, px, AnyElement, ClickEvent, Div, SharedString, Stateful};

pub(super) const DETAIL: f32 = 360.;
const VANILLA_ACTION: ui::ButtonSize = ui::ButtonSize::Md;

pub(super) const DETAIL_CLOSE: &str = "mods-detail-close";
const FLOAT_MIN_HEIGHT: f32 = ui::TILE_LG + 2. * ui::CARD_PADDING;
const SEARCH_FIELD_MIN: f32 = 200.;

pub(super) const TOOLBAR_INSET: f32 = ui::CARD_PADDING - ui::LIST_INSET;

impl Launcher {
    /// The search's placeholder: the registry's count when it is listed whole.
    fn mods_placeholder(&self) -> SharedString {
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

    pub(super) fn mods_page(&mut self, cx: &mut Context<Self>) -> Div {
        self.mods_load();
        if self.mods.kind == PackKind::DataPack {
            self.load_worlds(&self.mods.home(), cx);
        }
        if !self.mods.asked {
            self.mods_search(0);
        }
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

    pub(super) fn mods_datapack_notice_shown(&self) -> bool {
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
}

impl Launcher {
    fn mods_field(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
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

    fn mods_target_line(&self, cx: &mut Context<Self>) -> Option<Div> {
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
