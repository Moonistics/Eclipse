//! The Mods page's look: the kinds as tabs under its title, then two cards: the search and
//! what it found (icon, name, author, summary, then downloads for the registry's and how it runs
//! for Modrinth's, a tick on what the instance has), and the picked project's card (its icon,
//! name, line, summary, what it may do, downloads, and Install in or Remove from the instance).
//! A kind's sources are tabs under the search: Eclipse's registry or Modrinth, on the kinds both
//! serve (`switches`); shader packs are Eclipse's registry's only, and a Java instance's files
//! always Modrinth's. Modrinth's Java mods are offered to an Eclipse instance too, each marked
//! Works, Partly or Doesn't run in Eclipse. Under the two cards: adding a native mod from GitHub,
//! the instance's installed list and its native build, and the install's progress.

use super::*;
use crate::menus::Asking;
use crate::menus::Menu;
use crate::tab_strip::StripTab;
use crate::ui::{self};
use eclipse_launcher::instances::{Instance, Kind as InstanceKind};
use eclipse_theme::glass::{ink, radius, space};

const DETAIL: f32 = 360.;
const VANILLA_ACTION: ui::ButtonSize = ui::ButtonSize::Md;
const FLOAT_MIN_HEIGHT: f32 = 240.;
const SEARCH_FIELD_MIN: f32 = 200.;

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
            tr(match m.kind {
                PackKind::Mod => "Search mods",
                PackKind::Modpack => "Search modpacks",
                PackKind::ResourcePack => "Search resource packs",
                PackKind::Shader => "Search shader packs",
                PackKind::DataPack => "Search data packs",
            })
            .into()
        }
    }

    pub(super) fn mods_view(&mut self, cx: &mut Context<Self>) -> Div {
        let instance = self
            .selected_instance()
            .map_or_else(|| "Eclipse".to_owned(), |i| i.name.clone());
        let target = self.mods_target(cx);
        let open = self.mods.kind;
        let kinds = KINDS
            .into_iter()
            .map(|(kind, label)| StripTab {
                value: kind,
                key: format!("{kind:?}"),
                label: tr(label).into(),
                count: None,
            })
            .collect();
        let tabs = self.tab_strip(
            "mods-tab",
            kinds,
            open,
            layout::content_width(self.window_width),
            |this, kind| this.mods_set_kind(kind),
            cx,
        );
        let header = div()
            .flex()
            .flex_col()
            .child(ui::page_head_with(
                Page::Mods.title(),
                "",
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
            return div()
                .flex()
                .flex_col()
                .child(header)
                .children(notice)
                .child(
                    ui::card()
                        .p(px(ui::CARD_PADDING))
                        .children(self.mods_busy_plate())
                        .child(project),
                );
        }

        let java = self.mods.java_for(self.mods.kind).is_some();
        let registry = self.mods.source == SourceId::Eclipse && !java;
        let everything = self.mods.everything;
        let other_versions = !registry && !java && self.mods.kind != PackKind::Modpack;
        let extras = div()
            .flex()
            .flex_col()
            .gap(px(space::MD))
            .px(px(space::XS))
            .pb(px(space::SM))
            .when(switches(self.mods.kind) && !java, |d| {
                d.child(self.source_switch(cx))
            })
            .when(other_versions, |d| {
                d.child(
                    ui::toggle_label(
                        "mods-everything",
                        tr("Include other Minecraft versions"),
                        everything,
                    )
                    .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.mods.everything = !this.mods.everything;
                        this.mods_search(0);
                        cx.notify();
                    })),
                )
            })
            .children(self.mods_filters(cx))
            .children(self.mods_target_line(cx));
        let search = div()
            .m(px(space::XS))
            .mb(px(space::SM))
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(space::SM))
            .child(
                div()
                    .flex_1()
                    .min_w(px(SEARCH_FIELD_MIN))
                    .child(self.mods_field(cx)),
            )
            .child(self.mods_sort(cx))
            .child(self.mods_filters_button(cx));
        let list = ui::card()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .p(px(ui::LIST_INSET))
            .child(search)
            .child(extras);
        let rows = self.mods_list(cx);
        // The card stays in view as the list scrolls (`sticky.rs`).
        let wide = self.window_width >= crate::layout::MODS_DETAIL_BESIDE;
        let detail = if wide || self.mods.picked.is_some() {
            self.mod_detail(&instance, cx)
        } else {
            None
        };
        let detail = detail.map(|d| {
            let down = self.mods_sticky.offset(&self.content_scroll.handle);
            if wide {
                return d
                    .flex_none()
                    .w(px(DETAIL))
                    .top(px(down))
                    .child(self.mods_sticky.panel());
            }
            let scroll = &self.mods.float_scroll;
            let close = ui::icon_button(
                "mods-detail-close",
                "cancel-01",
                tr("Close"),
                ButtonKind::Ghost,
                ui::ButtonSize::Sm,
                false,
            )
            .absolute()
            .top(px(space::LG))
            .right(px(space::LG))
            .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.mods.picked = None;
                this.asking = None;
                cx.notify();
            }));
            ui::float_card()
                .flex_none()
                .w(px(DETAIL))
                .ml(px(-DETAIL))
                .top(px(down))
                .shadow_lg()
                .occlude()
                .child(
                    ui::hand_back_at_ends(ui::scroll_pane("mods-detail-scroll", scroll), scroll)
                        .max_h(px(self.mods_float_room()))
                        .child(d),
                )
                .children(
                    ui::more_below_fade(scroll, ui::surface_colour(ui::Surface::Float))
                        .map(|fade| fade.rounded_b(px(radius::PANEL))),
                )
                .child(self.mods_sticky.panel())
                .child(self.mods_float_top(cx))
                .child(close)
        });
        let sticky = self.mods_sticky.row(&self.content_scroll.handle);
        let cards = if wide {
            div()
                .relative()
                .flex()
                .items_start()
                .gap(px(layout::CARD_GAP))
                .child(sticky)
                .child(list.child(rows))
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
        };

        let card = |inner: Div| ui::card().p(px(ui::CARD_PADDING)).child(inner);
        let mut more: Vec<Div> = Vec::new();
        more.extend(self.mods_busy_plate().into_iter().map(card));
        let packs = (self.mods.kind == PackKind::Modpack).then(|| {
            card(self.imported_packs(cx))
                .id("mods-imported-packs")
                .mb(px(layout::CARD_GAP))
        });
        if self.mods.kind == PackKind::Mod && self.mods.java.is_none() {
            more.push(card(self.github_panel(cx)));
        }
        more.extend(self.mods_installed_list(cx).map(card));
        let advice = self
            .mods_vanilla_notice(cx)
            .or_else(|| self.mods_modrinth_notice(cx))
            .map(|advice| {
                ui::card()
                    .p(px(ui::CARD_PADDING))
                    .mb(px(layout::CARD_GAP))
                    .child(advice)
            });
        div()
            .flex()
            .flex_col()
            .child(header)
            .children(notice)
            .children(advice)
            .children(packs)
            .child(cards)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(layout::CARD_GAP))
                    .mt(px(layout::CARD_GAP))
                    .children(more),
            )
    }

    fn mods_float_room(&self) -> f32 {
        let view = f32::from(self.content_scroll.handle.bounds().size.height);
        (view - self.mods.float_top.get() - layout::CARD_GAP).max(FLOAT_MIN_HEIGHT)
    }

    fn mods_float_top(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (top, pane, this) = (
            self.mods.float_top.clone(),
            self.content_scroll.handle.clone(),
            cx.entity().downgrade(),
        );
        gpui::canvas(
            move |bounds, window, _| {
                let now = f32::from(bounds.top() - pane.bounds().top()).max(0.);
                if (top.replace(now) - now).abs() >= 1. {
                    window.on_next_frame(move |_, cx| {
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
        let note = |s: SharedString| {
            ui::text(
                eclipse_theme::glass::step::LEAD,
                FontWeight::NORMAL,
                ink::BODY,
            )
            .child(s)
        };
        if self.mods.kind == PackKind::Modpack {
            return Some(row.child(note(tr("Imports as a new instance").into())));
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
        let button = match ui::try_focus_for("opener-mods-target") {
            Some(focus) => button.track_focus(&focus),
            None => button,
        };
        let button = self.menu_opener(button, Menu::ModsTarget, cx);
        let mut wrap = div().relative().flex_none().child(button);
        if open {
            let mut menu = self.menu_popover("mods-target-menu", cx);
            for i in choices {
                let picked = i.id == current.id;
                menu = menu.child(
                    ui::pick_row(
                        SharedString::from(format!("mods-target-pick-{}", i.id)),
                        i.name.clone(),
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
                ui::Align::Start,
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
        let others = self.mods_targets(&current).len() > 1;
        let create = ui::primary(
            "mods-new-fabric",
            tr("Create a Fabric instance"),
            VANILLA_ACTION,
        )
        .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.mods_new_fabric(minecraft.clone(), cx);
            cx.notify();
        }));
        let choose = others.then(|| {
            ui::button(
                "mods-choose-instance",
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
        });
        let said = trf(
            "{instance} is vanilla. Mods need Fabric, Quilt, NeoForge or Forge.",
            &[("instance", &current.name)],
        );
        let actions = std::iter::once(create.into_any_element())
            .chain(choose)
            .collect();
        Some(div().id("mods-vanilla").child(ui::notice(
            ui::Notice::InCard,
            "information-circle",
            said,
            actions,
        )))
    }

    fn mods_modrinth_notice(&mut self, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let m = &self.mods;
        if m.kind != PackKind::Mod || m.java.is_some() || m.source != SourceId::Modrinth {
            return None;
        }
        let show = ui::button(
            "mods-show-eclipse",
            None,
            tr("Show Eclipse mods"),
            ButtonKind::Secondary,
            VANILLA_ACTION,
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.mods_set_source(SourceId::Eclipse);
            cx.notify();
        }));
        Some(div().id("mods-modrinth-eclipse").child(ui::notice(
            ui::Notice::InCard,
            "information-circle",
            tr("Most Modrinth mods are made for Fabric or Forge and don't run in Eclipse."),
            vec![show.into_any_element()],
        )))
    }

    /// The source switch (`switches`): Eclipse's registry or Modrinth.
    fn source_switch(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let this = cx.entity().downgrade();
        ui::segments(
            "mods-source",
            &ui::translated(&SOURCES),
            self.mods.source,
            move |source, _, cx| {
                let _ = this.update(cx, |this, cx| {
                    this.mods_set_source(source);
                    cx.notify();
                });
            },
        )
        .self_start()
    }

    /// The "Sort by" dropdown beside the search; the choices on a popover under it, the chosen one
    /// ticked.
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
            let mut menu = self.menu_popover("mods-sort-menu", cx);
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
    fn mods_list(&mut self, cx: &mut Context<Self>) -> Div {
        let m = &self.mods;
        if let Some(e) = &m.error {
            let title = match (m.java_for(m.kind), m.source) {
                (None, SourceId::Eclipse) => tr(match m.kind {
                    PackKind::Mod => "Couldn't load Eclipse mods",
                    PackKind::Modpack => "Couldn't load Eclipse modpacks",
                    PackKind::ResourcePack => "Couldn't load Eclipse resource packs",
                    PackKind::Shader => "Couldn't load Eclipse shader packs",
                    PackKind::DataPack => "Couldn't load Eclipse data packs",
                }),
                _ => tr("Couldn't search Modrinth"),
            };
            let error = e.clone();
            let retry = match self.mods_vanilla_target() {
                Some(_) => ButtonKind::Secondary,
                None => ButtonKind::Primary,
            };
            return div().child(ui::error_state_with(
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
        }
        if m.results.is_empty() {
            if m.searching || !m.searched {
                return div().child(ui::loading_state("mods-searching", tr("Searching…")));
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
                let title = tr(match m.kind {
                    PackKind::Mod => "No Eclipse mods yet",
                    PackKind::Modpack => "No Eclipse modpacks yet",
                    PackKind::ResourcePack => "No Eclipse resource packs yet",
                    PackKind::Shader => "No Eclipse shader packs yet",
                    PackKind::DataPack => "No Eclipse data packs yet",
                });
                if !switches(m.kind) {
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
            return div().child(ui::empty_state(
                "mods-no-results",
                "search-01",
                title,
                note,
                actions,
            ));
        }
        let picked = if self.window_width >= crate::layout::MODS_DETAIL_BESIDE
            || self.mods.picked.is_some()
        {
            self.mods_picked().map(|p| p.id.clone())
        } else {
            None
        };
        let vanilla = self.mods_vanilla_target().is_some();
        let cursor = ui::options_cursor("mods-results");
        let mut rows = Vec::new();
        let mut ids = Vec::new();
        for i in 0..self.mods.results.len() {
            let p = self.mods.results[i].clone();
            let icon = self.mods_icon(&p, ui::TILE_MD);
            let m = &self.mods;
            let installed = m.entry(&p).is_some();
            let busy = m.busy.as_ref().is_some_and(|b| b.0 == p.id);
            let checking = m.checking(&p);
            let can_install =
                !installed && !checking && m.busy.is_none() && m.support(&p).installable();
            let row_id = SharedString::from(format!("mods-row-{}", p.id));
            let current = picked.as_deref() == Some(p.id.as_str());
            let action: AnyElement = if installed || busy || checking || m.support(&p).installable()
            {
                let label = if installed {
                    tr("Installed")
                } else if checking {
                    tr("Checking…")
                } else if busy {
                    tr("Installing…")
                } else if p.kind == PackKind::Modpack {
                    tr("Import")
                } else {
                    tr("Install")
                };
                let known = p.clone();
                let button_id = SharedString::from(format!("mods-row-install-{}", p.id));
                let button = ui::button(
                    button_id,
                    None,
                    label,
                    ButtonKind::Secondary,
                    ui::ButtonSize::Sm,
                );
                if can_install {
                    button
                        .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            this.mods_install(known.source, known.id.clone(), Some(known.clone()));
                            cx.notify();
                        }))
                        .when(i != cursor, |b| b.tab_stop(false))
                        .into_any_element()
                } else {
                    ui::disabled(button).into_any_element()
                }
            } else {
                div().flex_none().w(px(20.)).into_any_element()
            };
            // The registry's rows end with downloads, the others' with how they run (`compat`).
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
                let said = match (m.java_for(p.kind), support.reason()) {
                    (Some(_), Some(reason)) => Some(reason.to_string()),
                    (None, _) if m.java.is_none() && p.kind != PackKind::Modpack => Some(trf(
                        "{support} in Eclipse",
                        &[("support", tr(support.label()))],
                    )),
                    _ => None,
                };
                let said = said.filter(|_| !vanilla && support != Support::Works && !checking);
                let quiet = vanilla || !support.installable();
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
                    .aria_description(summary.clone())
                    .child(div().relative().child(icon))
                    .child(ui::item_text_by(p.title.clone(), by, summary))
                    .children(right.map(|r| r.flex_none()))
                    .child(action),
                SharedString::from(p.title.clone()),
                current,
            ));
        }
        let open = cx.listener(move |this, i: &usize, _, cx| {
            this.mods.picked = ids.get(*i).cloned();
            this.mods
                .float_scroll
                .set_offset(gpui::point(px(0.), px(0.)));
            this.asking = None;
            cx.notify();
        });
        let mut list = div().flex().flex_col().child(ui::options(
            div()
                .id("mods-results")
                .aria_label(tr("Results"))
                .flex()
                .flex_col(),
            rows,
            false,
            move |i, window, cx| open(&i, window, cx),
        ));
        if self.mods.more {
            let searching = self.mods.searching;
            let offset = self.mods.results.len();
            list = list.child(
                div().flex().p(px(space::SM)).child(
                    ui::button(
                        "mods-more",
                        None,
                        if searching {
                            tr("Loading…")
                        } else {
                            tr("Load more")
                        },
                        ButtonKind::Secondary,
                        ui::ButtonSize::Md,
                    )
                    .when(searching, ui::disabled)
                    .when(!searching, |d| {
                        d.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.mods_search(offset);
                            cx.notify();
                        }))
                    }),
                ),
            );
        }
        list
    }

    /// The picked result, else the first; when a search finds nothing, the card shown last
    /// stays.
    fn mods_picked(&mut self) -> Option<Project> {
        let m = &mut self.mods;
        let found = m
            .picked
            .as_ref()
            .and_then(|id| m.results.iter().find(|p| &p.id == id))
            .or(m.results.first())
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

    /// The picked project's card.
    fn mod_detail(&mut self, instance: &str, cx: &mut Context<Self>) -> Option<Div> {
        let p = self.mods_picked()?;
        let vanilla = self.mods_vanilla_target();
        let icon = self.mods_icon(&p, ui::TILE_LG);
        let m = &self.mods;
        let support = m.support(&p);
        let entry = m.entry(&p).cloned();
        let busy = m.busy.as_ref().map(|b| b.0.clone());
        let checking = m.checking(&p);
        let mut by = Vec::new();
        if !p.author.is_empty() {
            by.push(trf("by {author}", &[("author", &p.author)]));
        }
        if let Some(source) = p.source.shown_for(&p.id) {
            by.push(trf("from {source}", &[("source", source)]));
        }
        if !p.license.is_empty() {
            by.push(p.license.clone());
        }
        let status = self.mods_status(&p, instance).map(|(tone, said, reason)| {
            let tone = match tone {
                ui::Tone::Warn if vanilla.is_some() => ui::Tone::Off,
                known => known,
            };
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
        } else if busy.as_deref() == Some(p.id.as_str()) || checking {
            ui::button(
                "mods-detail-wait",
                None,
                if checking {
                    tr("Checking…")
                } else {
                    tr("Installing…")
                },
                ButtonKind::Secondary,
                ui::ButtonSize::Lg,
            )
            .w_full()
            .map(ui::disabled)
            .into_any_element()
            .into()
        } else if support.installable() {
            let label = if p.kind == PackKind::Modpack {
                tr("Import as an instance").to_string()
            } else {
                trf("Install in {instance}", &[("instance", instance)])
            };
            let known = p.clone();
            ui::primary(
                SharedString::from(format!("mods-detail-install-{}", p.id)),
                label,
                ui::ButtonSize::Lg,
            )
            .w_full()
            .when(busy.is_some(), ui::disabled)
            .when(busy.is_none(), |d| {
                d.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.mods_install(known.source, known.id.clone(), Some(known.clone()));
                    cx.notify();
                }))
            })
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
                .children(
                    (p.source == SourceId::Eclipse)
                        .then(|| stars(&p.id))
                        .flatten()
                        .map(|s| s.mt(px(space::SM))),
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
                                tr(page_label(p.kind)),
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

fn page_label(kind: PackKind) -> &'static str {
    match kind {
        PackKind::Mod => "Open mod page",
        PackKind::Modpack => "Open modpack page",
        PackKind::ResourcePack => "Open resource pack page",
        PackKind::Shader => "Open shader pack page",
        PackKind::DataPack => "Open data pack page",
    }
}
