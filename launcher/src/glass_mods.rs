//! The Mods page's look: the kinds as tabs under its title, then two cards: the search and
//! what it found (icon, name, author, summary, then downloads for the registry's and how it runs
//! for Modrinth's, a tick on what the instance has), and the picked project's card (its icon,
//! name, line, summary, what it may do, downloads, and Install in or Remove from the instance).
//! A kind's sources are tabs under the search: Eclipse's registry or Modrinth, on the kinds both
//! serve (`switches`); shader packs are Eclipse's registry's only, and a Java instance's files
//! always Modrinth's. Modrinth's Java mods are offered to an Eclipse instance too, each marked
//! Works, Partly or Doesn't run in Eclipse. Under the two cards: adding a native mod from GitHub,
//! the instance's installed list and its native build, imported modpacks and the install's
//! progress.

use super::*;
use crate::glass_ui::{self as ui, white};
use eclipse_launcher::instances::{Instance, Kind as InstanceKind};
use eclipse_theme::glass::{fill, ink, radius};

const DETAIL: f32 = 360.;

impl Launcher {
    /// The search's placeholder: the registry's count when it is listed whole.
    pub(super) fn mods_placeholder(&self) -> SharedString {
        let m = &self.mods;
        let registry = m.source == SourceId::Eclipse && m.java.is_none();
        if registry && m.kind == PackKind::Mod && m.searched && m.text.text().is_empty() {
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

    pub(super) fn glass_mods_page(&mut self, cx: &mut Context<Self>) -> Div {
        let instance = self
            .selected_instance()
            .map_or_else(|| "Eclipse".to_owned(), |i| i.name.clone());
        let target = self.glass_mods_target(cx);
        let open = self.mods.kind;
        let mut tabs = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(4.))
            .mt(px(20.))
            .ml(px(-ui::TAB_PAD));
        for (kind, label, _) in KINDS {
            tabs = tabs.child(
                ui::tab(
                    SharedString::from(format!("mods-tab-{label}")),
                    tr(label),
                    kind == open,
                )
                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.mods_set_kind(kind);
                    cx.notify();
                })),
            );
        }
        let header = ui::page_head(tr("Mods"), "").children(target).child(tabs);

        // Installation can pause for consent in either view. Keep the notice before
        // the project or results, rather than below a potentially long detail card.
        let notice = self.native_notice(cx).map(|notice| notice.mb(px(20.)));
        // A project's own page replaces the two cards, as before.
        if let Some(project) = self.mods_project_page(cx) {
            return div()
                .flex()
                .flex_col()
                .child(header)
                .children(notice)
                .child(
                    crate::glass::plate(radius::PANEL)
                        .p(px(ui::CARD_PADDING))
                        .children(self.mods_busy_plate())
                        .child(project),
                );
        }

        let java = self.mods.java_for(self.mods.kind).is_some();
        let registry = self.mods.source == SourceId::Eclipse && !java;
        let everything = self.mods.everything;
        let extras = div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .px(px(4.))
            .pb(px(8.))
            .when(!registry && !java, |d| {
                d.child(
                    ui::toggle_label("mods-everything", tr("Show all results"), everything)
                        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.mods.everything = !this.mods.everything;
                            this.mods_search(0);
                            cx.notify();
                        })),
                )
            })
            .when(switches(self.mods.kind) && !java, |d| {
                d.child(self.glass_source_switch(cx))
            })
            .children(self.mods_filters(cx))
            .children(self.mods_target_line(cx));
        let search = div()
            .m(px(4.))
            .mb(px(8.))
            .flex()
            .items_center()
            .gap(px(8.))
            .child(div().flex_1().min_w(px(200.)).child(self.mods_field(cx)))
            .child(self.glass_mods_sort(cx))
            .child(self.mods_filters_button(cx));
        let list = crate::glass::plate(radius::PANEL)
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .p(px(ui::LIST_INSET))
            .child(search)
            .child(extras)
            .children(self.mods_vanilla_notice(cx))
            .child(self.glass_mods_list(cx));
        // The card stays in view as the list scrolls (`sticky.rs`).
        let wide = self.window_width >= crate::layout::MODS_DETAIL_BESIDE;
        let detail = if wide || self.mods.picked.is_some() {
            self.glass_mod_detail(&instance, cx)
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
            let close = ui::icon_button(
                "mods-detail-close",
                "cancel-01",
                tr("Close"),
                ui::Size::Sm,
                false,
            )
            .absolute()
            .top(px(16.))
            .right(px(16.))
            .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.mods.picked = None;
                this.mods.confirm_remove = None;
                cx.notify();
            }));
            crate::glass::float(radius::PANEL)
                .flex_none()
                .w(px(DETAIL))
                .ml(px(-DETAIL))
                .top(px(down))
                .shadow_lg()
                .occlude()
                .child(d)
                .child(self.mods_sticky.panel())
                .child(close)
        });
        let cards = div()
            .relative()
            .flex()
            .items_start()
            .gap(px(if wide { 10. } else { 0. }))
            .child(self.mods_sticky.row(&self.content_scroll.handle))
            .child(list)
            .children(detail);

        let card = |inner: Div| {
            crate::glass::plate(radius::PANEL)
                .p(px(ui::CARD_PADDING))
                .child(inner)
        };
        let mut more: Vec<Div> = Vec::new();
        more.extend(self.mods_busy_plate().into_iter().map(card));
        if self.mods.kind == PackKind::Modpack {
            more.push(card(self.imported_packs(cx)));
        }
        if self.mods.kind == PackKind::Mod && self.mods.java.is_none() {
            more.push(card(self.github_panel(cx)));
        }
        more.extend(self.mods_installed_list(cx).map(card));
        div()
            .flex()
            .flex_col()
            .child(header)
            .children(notice)
            .child(cards)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .mt(px(10.))
                    .children(more),
            )
    }

    fn glass_mods_target(&mut self, cx: &mut Context<Self>) -> Option<Div> {
        let row = div()
            .flex()
            .items_center()
            .gap(px(12.))
            .h(px(36.))
            .mt(px(12.));
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
        let open = self.mods.target_menu;
        let button = ui::dropdown(
            "mods-target",
            Some(self.glass_instance_icon(&current, 24.)),
            current.name.clone(),
            open,
            ui::Size::Md.height(),
        )
        .on_press(cx.listener(|this, event: &ClickEvent, window, cx| {
            if this.closed_by_this_press(event) {
                return;
            }
            this.mods.target_menu = !this.mods.target_menu;
            if this.mods.target_menu {
                this.opened_menu(window, cx);
            }
            cx.notify();
        }));
        let mut wrap = div().relative().flex_none().child(button);
        if open {
            let mut menu = ui::popover("mods-target-menu").on_mouse_down_out(cx.listener(
                |this, event: &MouseDownEvent, _, cx| {
                    this.mods.target_menu = false;
                    this.menu_closed_at = Some(event.position);
                    cx.notify();
                },
            ));
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
                            this.mods.target_menu = false;
                            if !picked {
                                this.select_instance(&i);
                            }
                            cx.notify();
                        },
                    )),
                );
            }
            wrap = wrap.child(
                gpui::deferred(
                    div()
                        .absolute()
                        .top(px(ui::Size::Md.height() + ui::MENU_GAP))
                        .left_0()
                        .child(menu),
                )
                .with_priority(1),
            );
        }
        Some(row.child(note(tr("Install to").into())).child(wrap))
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
            ui::Size::Sm,
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
                Kind::Ghost,
                ui::Size::Sm,
            )
            .on_press(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.mods.target_menu = true;
                this.opened_menu(window, cx);
                cx.notify();
            }))
        });
        Some(
            div()
                .id("mods-vanilla")
                .flex()
                .flex_col()
                .gap(px(12.))
                .mx(px(4.))
                .mb(px(8.))
                .p(px(ui::PANEL_INSET))
                .rounded(px(radius::ROW))
                .bg(white(fill::WELL))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .child(ui::icon(
                            "information-circle",
                            ui::IconSize::Px16,
                            white(ink::BODY),
                        ))
                        .child(
                            ui::text(
                                eclipse_theme::glass::step::BODY,
                                FontWeight::NORMAL,
                                ink::FULL,
                            )
                            .min_w_0()
                            .child(trf(
                                "{instance} is vanilla. Mods need Fabric, Quilt, NeoForge or Forge.",
                                &[("instance", &current.name)],
                            )),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(px(8.))
                        .child(create)
                        .children(choose),
                ),
        )
    }

    /// The source switch (`switches`): Eclipse's registry or Modrinth as small tabs.
    fn glass_source_switch(&mut self, cx: &mut Context<Self>) -> Div {
        let current = self.mods.source;
        let mut row = div().flex().items_center().gap(px(4.));
        for (source, label) in SOURCES {
            row = row.child(
                ui::tab(
                    SharedString::from(format!("mods-source-{label}")),
                    tr(label),
                    source == current,
                )
                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if this.mods.source != source {
                        this.mods.source = source;
                        this.mods.browse.filters = Default::default();
                        this.mods.browse.open = None;
                        this.mods.picked = None;
                        this.mods_search(0);
                    }
                    cx.notify();
                })),
            );
        }
        row
    }

    /// The "Sort by" dropdown beside the search; the choices on a popover under it, the chosen one
    /// ticked.
    fn glass_mods_sort(&mut self, cx: &mut Context<Self>) -> Div {
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
        let open = self.mods.browse.sort_menu;
        let button = ui::dropdown(
            "mods-sort",
            None,
            trf("Sort by: {order}", &[("order", tr(name))]),
            open,
            ui::Size::Md.height(),
        )
        .on_press(cx.listener(|this, event: &ClickEvent, window, cx| {
            if this.closed_by_this_press(event) {
                return;
            }
            this.mods.browse.sort_menu = !this.mods.browse.sort_menu;
            if this.mods.browse.sort_menu {
                this.opened_menu(window, cx);
            }
            cx.notify();
        }));
        let mut wrap = div().relative().flex_none().child(button);
        if open {
            let mut menu = ui::popover("mods-sort-menu").on_mouse_down_out(cx.listener(
                |this, event: &MouseDownEvent, _, cx| {
                    this.mods.browse.sort_menu = false;
                    this.menu_closed_at = Some(event.position);
                    cx.notify();
                },
            ));
            for &(value, label) in &sorts {
                menu = menu.child(
                    ui::pick_row(
                        SharedString::from(format!("mods-sort-pick-{label}")),
                        tr(label).to_string(),
                        Some(value) == shown,
                    )
                    .on_press(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            this.mods.browse.sort_menu = false;
                            this.mods_filter(|f| f.sort = value);
                            cx.notify();
                        },
                    )),
                );
            }
            wrap = wrap.child(
                gpui::deferred(
                    div()
                        .absolute()
                        .top(px(ui::Size::Md.height() + ui::MENU_GAP))
                        .right_0()
                        .child(menu),
                )
                .with_priority(1),
            );
        }
        wrap
    }

    /// The search's results as rows: a click picks one for the card beside.
    fn glass_mods_list(&mut self, cx: &mut Context<Self>) -> Div {
        let m = &self.mods;
        if let Some(e) = &m.error {
            let retry = ui::primary("mods-retry", tr("Try again"), ui::Size::Md).on_press(
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.mods_search(0);
                    cx.notify();
                }),
            );
            return div().child(ui::error_state(
                "mods-failed",
                tr("Couldn't search"),
                e,
                retry,
            ));
        }
        if m.results.is_empty() {
            if m.searching || !m.searched {
                return div().child(ui::loading_state("mods-searching", tr("Searching…")));
            }
            let mut actions = Vec::new();
            let (title, note) = if let Some(target) = m.java_for(m.kind) {
                (
                    trf(
                        "Nothing found for {target}",
                        &[("target", &target.describe(m.kind))],
                    ),
                    tr("Try other words or another instance."),
                )
            } else if !m.text.text().trim().is_empty() {
                (
                    trf(
                        "Nothing matches “{query}”",
                        &[("query", m.text.text().trim())],
                    ),
                    tr("Try other words."),
                )
            } else if m.source == SourceId::Eclipse {
                (tr("No Eclipse mods match yet").to_string(), "")
            } else if m.everything {
                (
                    tr("Nothing found").to_string(),
                    tr("Try other words or clear the filters."),
                )
            } else {
                actions.push(
                    ui::button(
                        "mods-show-all",
                        None,
                        tr("Show all results"),
                        Kind::Secondary,
                        ui::Size::Md,
                    )
                    .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.mods.everything = true;
                        this.mods_search(0);
                        cx.notify();
                    })),
                );
                (tr("Nothing found that runs in Eclipse").to_string(), "")
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
        let mut list = div().flex().flex_col();
        for i in 0..self.mods.results.len() {
            let p = self.mods.results[i].clone();
            let icon = self.mods_icon(&p, 40.);
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
                let button = ui::button(
                    SharedString::from(format!("mods-row-install-{}", p.id)),
                    None,
                    label,
                    if current {
                        Kind::Secondary
                    } else {
                        Kind::Ghost
                    },
                    ui::Size::Sm,
                );
                if can_install {
                    button
                        .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            this.mods_install(known.source, known.id.clone(), Some(known.clone()));
                            cx.notify();
                        }))
                        .into_any_element()
                } else {
                    disabled(button).into_any_element()
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
                    (None, _) if m.java.is_none() && p.kind != PackKind::Modpack => {
                        Some(tr(support.label()).to_string())
                    }
                    _ => None,
                };
                let said = said.filter(|_| support != Support::Works && !checking);
                let quiet = vanilla || !support.installable();
                said.map(|said| {
                    let line = ui::text(
                        eclipse_theme::glass::step::NOTE,
                        FontWeight::MEDIUM,
                        ink::FULL,
                    );
                    match quiet {
                        true => line
                            .font_weight(FontWeight::NORMAL)
                            .text_color(white(ink::DIM)),
                        false => line.text_color(ui::warn()),
                    }
                    .child(said)
                })
            };
            let id = p.id.clone();
            list = list.child(
                ui::row(row_id, current, 11.)
                    .px(px(11.))
                    .py(px(9.))
                    .child(div().relative().child(icon))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .flex()
                                    .items_baseline()
                                    .gap(px(8.))
                                    .min_w_0()
                                    .child(
                                        ui::text(
                                            eclipse_theme::glass::step::BODY,
                                            FontWeight::MEDIUM,
                                            ink::FULL,
                                        )
                                        .truncate()
                                        .child(p.title.clone()),
                                    )
                                    .child(
                                        ui::text(
                                            eclipse_theme::glass::step::NOTE,
                                            FontWeight::NORMAL,
                                            ink::DIM,
                                        )
                                        .truncate()
                                        .child(
                                            match p.kind {
                                                PackKind::Mod | PackKind::Shader => {
                                                    p.author.clone()
                                                }
                                                _ => tr(kind_name(p.kind)).to_string(),
                                            },
                                        ),
                                    ),
                            )
                            .child(
                                ui::text(
                                    eclipse_theme::glass::step::NOTE,
                                    FontWeight::NORMAL,
                                    ink::MUTED,
                                )
                                .truncate()
                                .child(p.summary.split_whitespace().collect::<Vec<_>>().join(" ")),
                            ),
                    )
                    .children(right.map(|r| r.flex_none()))
                    .child(action)
                    .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.mods.picked = Some(id.clone());
                        this.mods.confirm_remove = None;
                        cx.notify();
                    })),
            );
        }
        if self.mods.more {
            let searching = self.mods.searching;
            let offset = self.mods.results.len();
            list = list.child(
                div().flex().p(px(8.)).child(
                    ui::button(
                        "mods-more",
                        None,
                        tr("Load more"),
                        Kind::Secondary,
                        ui::Size::Md,
                    )
                    .when(searching, crate::disabled)
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
    fn glass_mod_detail(&mut self, instance: &str, cx: &mut Context<Self>) -> Option<Div> {
        let p = self.mods_picked()?;
        let vanilla = self.mods_vanilla_target();
        let icon = self.mods_icon(&p, 64.);
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
            let color = match tone {
                ui::Tone::Live => ui::live(),
                ui::Tone::Warn => ui::warn(),
                _ => white(ink::DIM),
            };
            div()
                .mt(px(16.))
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(ui::dot(tone))
                        .child(
                            ui::text(
                                eclipse_theme::glass::step::BODY,
                                FontWeight::MEDIUM,
                                ink::FULL,
                            )
                            .text_color(color)
                            .child(said),
                        ),
                )
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

        // The action: Remove (then Remove for good), Install or Import, or a Fabric instance
        // for a mod a vanilla one can't take.
        let button: Option<AnyElement> = if let Some(e) = &entry {
            let confirming = m.confirm_remove.as_deref() == Some(e.project.as_str());
            let id = e.project.clone();
            ui::button(
                SharedString::from(format!("mods-detail-remove-{}", p.id)),
                None,
                if confirming {
                    tr("Remove for good").to_string()
                } else {
                    trf("Remove from {instance}", &[("instance", instance)])
                },
                Kind::Secondary,
                ui::Size::Lg,
            )
            .w_full()
            .when(confirming, |d| d.text_color(ui::danger()))
            .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.mods_remove_click(id.clone());
                cx.notify();
            }))
            .into_any_element()
            .into()
        } else if busy.as_deref() == Some(p.id.as_str()) || checking {
            ui::button(
                "mods-detail-wait",
                None,
                if checking {
                    tr("Checking…")
                } else {
                    tr("Installing…")
                },
                Kind::Secondary,
                ui::Size::Lg,
            )
            .w_full()
            .map(crate::disabled)
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
                ui::Size::Lg,
            )
            .w_full()
            .when(busy.is_some(), crate::disabled)
            .when(busy.is_none(), |d| {
                d.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.mods_install(known.source, known.id.clone(), Some(known.clone()));
                    cx.notify();
                }))
            })
            .into_any_element()
            .into()
        } else {
            vanilla.map(|minecraft| {
                ui::button(
                    "mods-detail-new-fabric",
                    None,
                    tr("Create a Fabric instance"),
                    Kind::Secondary,
                    ui::Size::Lg,
                )
                .w_full()
                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.mods_new_fabric(minecraft.clone(), cx);
                    cx.notify();
                }))
                .into_any_element()
            })
        };
        let page_kind = if button.is_some() {
            Kind::Ghost
        } else {
            Kind::Secondary
        };
        let open = p.clone();
        Some(
            crate::glass::plate(radius::PANEL)
                .flex()
                .flex_col()
                .p(px(ui::CARD_PADDING))
                .child(icon)
                .child(
                    ui::entity_title(
                        p.title.clone(),
                        eclipse_theme::glass::pixel_text::CARD_TITLE,
                    )
                    .mt(px(20.)),
                )
                .child(
                    ui::text(
                        eclipse_theme::glass::step::NOTE,
                        FontWeight::NORMAL,
                        ink::DIM,
                    )
                    .mt(px(4.))
                    .child(by.join(" · ")),
                )
                .children(
                    (p.source == SourceId::Eclipse)
                        .then(|| stars(&p.id))
                        .flatten()
                        .map(|s| s.mt(px(8.))),
                )
                .child(
                    ui::text(
                        eclipse_theme::glass::step::BODY,
                        FontWeight::NORMAL,
                        ink::BODY,
                    )
                    .mt(px(16.))
                    .child(p.summary.clone()),
                )
                .children(about.map(|a| {
                    ui::text(
                        eclipse_theme::glass::step::NOTE,
                        FontWeight::NORMAL,
                        ink::DIM,
                    )
                    .mt(px(16.))
                    .child(a)
                }))
                .children(status)
                .child(
                    ui::text(
                        eclipse_theme::glass::step::NOTE,
                        FontWeight::NORMAL,
                        ink::DIM,
                    )
                    .mt(px(16.))
                    .child(trf(
                        "{count} downloads",
                        &[("count", &super::thousands(p.downloads))],
                    )),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .mt(px(24.))
                        .children(button)
                        .child(
                            crate::button("mods-page-link", page_label(p.kind), page_kind)
                                .w_full()
                                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.mods_open(open.clone());
                                    cx.notify();
                                })),
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

/// One project's kind, for a Modrinth result's second word (`Resource pack`).
fn kind_name(kind: PackKind) -> &'static str {
    match kind {
        PackKind::Mod => "Mod",
        PackKind::Modpack => "Modpack",
        PackKind::ResourcePack => "Resource pack",
        PackKind::Shader => "Shader pack",
        PackKind::DataPack => "Data pack",
    }
}
