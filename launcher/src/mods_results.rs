use crate::fields::Field;
use crate::mods_page::{DETAIL_CLOSE, TOOLBAR_INSET};
use crate::mods_state::{count, kind_words};
use crate::ui::{self, ButtonKind, Press};
use crate::{mods_project, Launcher};
use eclipse_launcher::browser;
use eclipse_launcher::browser::{Kind as PackKind, Project, SourceId, Support};
use eclipse_launcher::i18n::{tr, trf};
use eclipse_theme::glass::{ink, space};
use gpui::prelude::*;
use gpui::{div, px, AnyElement, ClickEvent, Div, FontWeight, SharedString, Stateful};

const ROW_PLATE_INSET: f32 = space::MD - ui::LIST_INSET;
const ROW_CONTENT_INSET: f32 = ui::CARD_PADDING - space::MD - ui::ROW_EDGE;
pub(super) const RESULTS: &str = "mods-results";

impl Launcher {
    /// The search's results as rows: a click picks one for the card beside.
    pub(super) fn mods_list(&mut self, cx: &mut Context<Self>) -> (Div, Option<Div>) {
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
    pub(super) fn mods_picked(&mut self) -> Option<Project> {
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
}
