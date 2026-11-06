//! The Mods page in the glass look: the mockup's `ModsView`. The page's tabs under its
//! title, then two glass cards: the search and the list of what it found (icon, name, author,
//! summary, then downloads for the registry's and how it runs for Modrinth's, a tick on what the
//! instance has), and the picked project's card (its
//! icon, name, line, summary, what it may do, downloads, and Install in or Remove from the
//! instance).
//!
//! The mockup's tabs are its sources (Eclipse, Modrinth, CurseForge soon); the launcher's page
//! is by kind (mods, modpacks, resource packs, shader packs, data packs), each with its sources,
//! so the tabs are the kinds and the sources are a switch under the search: Eclipse's registry
//! or Modrinth, CurseForge beside them as coming soon, on the kinds both serve (`switches`):
//! shader packs are Eclipse's registry's only, and a Java
//! instance's files always Modrinth's. Modrinth's Java mods are offered to a Eclipse instance
//! too, each marked Works, Partly or Doesn't run in Eclipse. Not in the mockup, kept in its
//! patterns: Show everything, the filters, which instance or world installs go to (under the
//! search); the
//! project's page, its rating and why it runs or not (in the project's card); adding a native mod
//! from GitHub, the instance's installed list and its native build, imported modpacks and the
//! install's progress (cards under the two).

use super::*;
use crate::glass_ui::{self as ui, white};
use eclipse_launcher::instances::{Instance, Kind as InstanceKind};
use eclipse_theme::glass::{fill, ink, radius};

/// The project card's width (`lg:grid-cols-[minmax(0,1fr)_360px]`).
const DETAIL: f32 = 360.;
const SIDE_BY_SIDE: f32 = 1280.;

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
        let t = self.theme;
        let instance = self
            .selected_instance()
            .map_or_else(|| "Eclipse".to_owned(), |i| i.name.clone());
        let head = ui::page_head(tr("Mods"), "").pt_0().mb_0();
        let target = self.glass_mods_target(cx);
        let open = self.mods.kind;
        let mut tabs = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(4.))
            .mt(px(20.))
            .ml(px(-12.));
        for (kind, label, _) in KINDS {
            tabs = tabs.child(
                ui::tab(
                    SharedString::from(format!("mods-tab-{label}")),
                    tr(label),
                    kind == open,
                    &t,
                )
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.mods_set_kind(kind);
                    cx.notify();
                })),
            );
        }
        let header = div()
            .flex()
            .flex_col()
            .pt(px(40. - crate::glass::CONTENT_INSET))
            .mb(px(28.))
            .child(head)
            .children(target)
            .child(tabs);

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
                        .p(px(20.))
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
                    div()
                        .id("mods-everything")
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .cursor_pointer()
                        .child(ui::toggle(
                            "mods-everything-switch",
                            everything,
                            ui::look().accent,
                            ui::look().on_accent,
                            &t,
                        ))
                        .child(
                            ui::text(13., FontWeight::NORMAL, ink::BODY)
                                .child(tr("Show all results")),
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
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
            .child(self.mods_filters_button(ui::Size::Md, cx));
        let list = crate::glass::plate(radius::PANEL)
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .p(px(8.))
            .child(search)
            .child(extras)
            .child(self.glass_mods_list(cx));
        // The card stays in view as the list scrolls (`lg:sticky lg:top-0`, `sticky.rs`).
        let wide = self.window_width >= SIDE_BY_SIDE;
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
                32.,
                18.,
                radius::CONTROL,
                false,
                &t,
            )
            .absolute()
            .top(px(16.))
            .right(px(16.))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
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

        // What the mockup doesn't have, as cards under the two.
        let card = |inner: Div| crate::glass::plate(radius::PANEL).p(px(20.)).child(inner);
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
        let t = self.theme;
        let row = div()
            .flex()
            .items_center()
            .gap(px(12.))
            .h(px(36.))
            .mt(px(12.));
        let note = |s: SharedString| ui::text(15., FontWeight::NORMAL, ink::BODY).child(s);
        if self.mods.kind == PackKind::Modpack {
            return Some(row.child(note(tr("Imports as a new instance").into())));
        }
        let current = self.selected_instance()?.clone();
        let kind = self.mods.kind;
        let choices: Vec<Instance> = self
            .instances
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
            .collect();
        let open = self.mods.target_menu;
        let button = ui::dropdown(
            "mods-target",
            Some(self.glass_instance_icon(&current, 24.)),
            current.name.clone(),
            open,
            ui::Size::Md.height(),
            &t,
        )
        .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
            if this.closed_by_this_press(event) {
                return;
            }
            this.mods.target_menu = !this.mods.target_menu;
            cx.notify();
        }));
        let mut wrap = div().relative().flex_none().child(button);
        if open {
            let mut menu = ui::popover(260.)
                .id("mods-target-menu")
                .occlude()
                .on_mouse_down_out(cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    this.mods.target_menu = false;
                    this.menu_closed_at = Some(event.position);
                    cx.notify();
                }));
            for i in choices {
                let picked = i.id == current.id;
                menu = menu.child(
                    ui::pick_row(
                        SharedString::from(format!("mods-target-pick-{}", i.id)),
                        i.name.clone(),
                        picked,
                        &t,
                    )
                    .on_click(cx.listener(
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
                gpui::deferred(div().absolute().top(px(44.)).left_0().child(menu)).with_priority(1),
            );
        }
        Some(row.child(note(tr("Install to").into())).child(wrap))
    }

    /// The source switch (`switches`): Eclipse's registry or Modrinth as small tabs, CurseForge
    /// after them as coming soon, shown, inert, never searched, as in the mockup.
    fn glass_source_switch(&mut self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let current = self.mods.source;
        let mut row = div().flex().items_center().gap(px(4.));
        for (source, label) in SOURCES {
            row = row.child(
                ui::tab(
                    SharedString::from(format!("mods-source-{label}")),
                    tr(label),
                    source == current,
                    &t,
                )
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
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
        row.child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .h(px(36.))
                .px(px(12.))
                .font_family(eclipse_theme::glass::LABEL_FONT)
                .text_size(px(13.))
                .text_color(white(ink::DIM))
                .child("CurseForge")
                .child(tr("soon").to_string()),
        )
    }

    /// The "Sort by" dropdown beside the search; the choices on a popover under it, the chosen one
    /// ticked.
    fn glass_mods_sort(&mut self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
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
            &t,
        )
        .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
            if this.closed_by_this_press(event) {
                return;
            }
            this.mods.browse.sort_menu = !this.mods.browse.sort_menu;
            cx.notify();
        }));
        let mut wrap = div().relative().flex_none().child(button);
        if open {
            let mut menu = ui::popover(220.)
                .id("mods-sort-menu")
                .occlude()
                .on_mouse_down_out(cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    this.mods.browse.sort_menu = false;
                    this.menu_closed_at = Some(event.position);
                    cx.notify();
                }));
            for &(value, label) in &sorts {
                menu = menu.child(
                    ui::pick_row(
                        SharedString::from(format!("mods-sort-pick-{label}")),
                        tr(label).to_string(),
                        Some(value) == shown,
                        &t,
                    )
                    .on_click(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            this.mods.browse.sort_menu = false;
                            this.mods_filter(|f| f.sort = value);
                            cx.notify();
                        },
                    )),
                );
            }
            wrap = wrap.child(
                gpui::deferred(div().absolute().top(px(48.)).right_0().child(menu))
                    .with_priority(1),
            );
        }
        wrap
    }

    /// The search's results as the mockup's rows (`ROW`): a click picks one for the card beside.
    fn glass_mods_list(&mut self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let m = &self.mods;
        if let Some(e) = &m.error {
            return ui::empty().child(e.clone());
        }
        if m.results.is_empty() {
            let line = if m.searching || !m.searched {
                tr("Searching…").to_string()
            } else if let Some(target) = m.java_for(m.kind) {
                trf(
                    "Nothing found for {target}. Try other words or another instance.",
                    &[("target", &target.describe(m.kind))],
                )
            } else if !m.text.text().trim().is_empty() {
                trf(
                    "Nothing matches “{query}”. Try other words.",
                    &[("query", m.text.text().trim())],
                )
            } else if m.source == SourceId::Eclipse {
                tr("No Eclipse mods match yet.").to_string()
            } else if m.everything {
                tr("Nothing found. Try other words or clear the filters.").to_string()
            } else {
                tr("Nothing found that runs in Eclipse. Turn on Show all results to see the rest.")
                    .to_string()
            };
            return ui::empty().child(line);
        }
        let picked = if self.window_width >= SIDE_BY_SIDE || self.mods.picked.is_some() {
            self.mods_picked().map(|p| p.id.clone())
        } else {
            None
        };
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
                    tr("Import as an instance")
                } else {
                    tr("Install")
                };
                let known = p.clone();
                let button = ui::button(
                    SharedString::from(format!("mods-row-install-{}", p.id)),
                    None,
                    label,
                    Kind::Secondary,
                    ui::Size::Sm,
                    &t,
                )
                .when(!current, |d| {
                    d.bg(gpui::transparent_black())
                        .text_color(white(ink::BODY))
                        .group_hover(row_id.clone(), |s| {
                            s.bg(white(fill::BUTTON)).text_color(white(ink::FULL))
                        })
                });
                if can_install {
                    button
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
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
                    ui::text(13., FontWeight::NORMAL, ink::DIM)
                        .child(count(p.downloads).to_uppercase()),
                )
            } else {
                let support = m.support(&p);
                let said = match (m.java_for(p.kind), support.reason()) {
                    (Some(_), Some(reason)) => reason.to_string(),
                    _ => tr(support.label()).to_string(),
                };
                (support != Support::Works && !checking).then(|| {
                    ui::text(13., FontWeight::MEDIUM, ink::FULL)
                        .text_color(ui::warn())
                        .child(said)
                })
            };
            let id = p.id.clone();
            list = list.child(
                ui::row(row_id.clone(), current, 12., &t)
                    .group(row_id)
                    .px(px(12.))
                    .py(px(10.))
                    .child(
                        div()
                            .flex_none()
                            .size(px(40.))
                            .rounded(px(radius::ROW))
                            .overflow_hidden()
                            .child(icon),
                    )
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
                                        ui::text(14., FontWeight::MEDIUM, ink::FULL)
                                            .truncate()
                                            .child(p.title.clone()),
                                    )
                                    .child(
                                        ui::text(13., FontWeight::NORMAL, ink::DIM)
                                            .truncate()
                                            .child(match p.kind {
                                                PackKind::Mod | PackKind::Shader => {
                                                    p.author.clone()
                                                }
                                                _ => tr(kind_name(p.kind)).to_string(),
                                            }),
                                    ),
                            )
                            .child(
                                ui::text(13., FontWeight::NORMAL, ink::MUTED)
                                    .truncate()
                                    .child(p.summary.clone()),
                            ),
                    )
                    .children(right.map(|r| r.flex_none()))
                    .child(action)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
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
                        tr("More"),
                        Kind::Secondary,
                        ui::Size::Md,
                        &t,
                    )
                    .when(searching, crate::disabled)
                    .when(!searching, |d| {
                        d.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.mods_search(offset);
                            cx.notify();
                        }))
                    }),
                ),
            );
        }
        list
    }

    /// The picked result, else the first; when a search finds nothing, the card shown last stays
    /// (the mockup's detail keeps its mod).
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

    /// The picked project's card (`ModDetail`).
    fn glass_mod_detail(&mut self, instance: &str, cx: &mut Context<Self>) -> Option<Div> {
        let t = self.theme;
        let p = self.mods_picked()?;
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
        let in_eclipse = trf("{support} in Eclipse", &[("support", tr(support.label()))]);
        let (tone, status, reason) = match (m.java_for(p.kind), &support) {
            (Some(_), Support::Works) => (
                ui::Tone::Live,
                trf("Works in {instance}", &[("instance", instance)]),
                None,
            ),
            (Some(target), _) => {
                let needs = support.reason().unwrap_or_default();
                let status = if target.loader.is_none() && p.kind == PackKind::Mod {
                    trf(
                        "{needs}: {instance} is vanilla",
                        &[("needs", needs), ("instance", instance)],
                    )
                } else {
                    needs.to_string()
                };
                (ui::Tone::Warn, status, None)
            }
            (None, Support::Works) => (ui::Tone::Live, in_eclipse, None),
            (None, Support::Partial(_)) => (ui::Tone::Warn, in_eclipse, support.reason()),
            (None, Support::No(_)) => (ui::Tone::Off, in_eclipse, support.reason()),
        };
        let status_color = match tone {
            ui::Tone::Live => ui::live(),
            ui::Tone::Warn => ui::warn(),
            _ => white(ink::DIM),
        };
        let about = m
            .about
            .get(&p.id)
            .filter(|_| p.source == SourceId::Eclipse)
            .map(|lines| lines.join(" · "));

        // The action: Remove (then Remove for good), Install or Import, or why not.
        let button: AnyElement = if let Some(e) = &entry {
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
                &t,
            )
            .w_full()
            .when(confirming, |d| d.text_color(ui::danger()))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.mods_remove_click(id.clone());
                cx.notify();
            }))
            .into_any_element()
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
                &t,
            )
            .w_full()
            .map(crate::disabled)
            .into_any_element()
        } else if support.installable() {
            let label = if p.kind == PackKind::Modpack {
                tr("Import as an instance").to_string()
            } else {
                trf("Install in {instance}", &[("instance", instance)])
            };
            let known = p.clone();
            ui::solid(
                SharedString::from(format!("mods-detail-install-{}", p.id)),
                label,
                44.,
                14.,
                &t,
            )
            .w_full()
            .when(busy.is_some(), crate::disabled)
            .when(busy.is_none(), |d| {
                d.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.mods_install(known.source, known.id.clone(), Some(known.clone()));
                    cx.notify();
                }))
            })
            .into_any_element()
        } else {
            ui::button(
                "mods-detail-cannot",
                None,
                tr(support.label()),
                Kind::Secondary,
                ui::Size::Lg,
                &t,
            )
            .w_full()
            .map(crate::disabled)
            .into_any_element()
        };
        let open = p.clone();
        Some(
            crate::glass::plate(radius::PANEL)
                .flex()
                .flex_col()
                .p(px(24.))
                .child(
                    div()
                        .size(px(64.))
                        .rounded(px(radius::PANEL))
                        .overflow_hidden()
                        .child(icon),
                )
                .child(ui::entity_title(p.title.clone(), 24.).mt(px(20.)))
                .child(
                    ui::text(13., FontWeight::NORMAL, ink::DIM)
                        .mt(px(4.))
                        .child(by.join(" · ")),
                )
                .children(
                    (p.source == SourceId::Eclipse)
                        .then(|| stars(&p.id, &t))
                        .flatten()
                        .map(|s| s.mt(px(8.))),
                )
                .child(
                    ui::text(14., FontWeight::NORMAL, ink::BODY)
                        .mt(px(16.))
                        .line_height(px(22.4))
                        .child(p.summary.clone()),
                )
                .children(about.map(|a| {
                    ui::text(13., FontWeight::NORMAL, ink::DIM)
                        .mt(px(16.))
                        .line_height(px(20.))
                        .child(a)
                }))
                .child(
                    div()
                        .mt(px(16.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(ui::dot(tone))
                        .child(
                            ui::text(14., FontWeight::MEDIUM, ink::FULL)
                                .text_color(status_color)
                                .child(status),
                        ),
                )
                .children(reason.map(|r| {
                    ui::text(13., FontWeight::NORMAL, ink::DIM)
                        .mt(px(4.))
                        .child(r.to_string())
                }))
                .child(
                    ui::text(13., FontWeight::NORMAL, ink::DIM)
                        .mt(px(16.))
                        .child(trf(
                            "{count} downloads",
                            &[("count", &super::thousands(p.downloads))],
                        )),
                )
                .child(div().mt(px(24.)).child(button))
                .child(
                    crate::button("mods-page-link", "Full page", Kind::Ghost, &t)
                        .mt(px(8.))
                        .w_full()
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.mods_open(open.clone());
                            cx.notify();
                        })),
                ),
        )
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
