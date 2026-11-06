//! The palette (⌘K, Ctrl+K elsewhere): a card over the dimmed launcher, a search line, then up
//! to eight commands, each its name and a hint. Arrows move, Enter runs, Esc or a click outside
//! closes.
//!
//! The commands: Play each instance, Open each page and Settings, New instance, Open the game
//! folder, and from two letters on, Install or Remove the registry's mods that match, in the
//! selected instance.

use super::line_edit::{self, Key};
use super::*;
use crate::ui::{self, white};
use eclipse_launcher::browser::Project;
use eclipse_launcher::instances::Kind as InstanceKind;
use eclipse_theme::glass::{ink, space};
use std::path::PathBuf;

/// Commands listed at most.
const SHOWN: usize = 8;

const CARD_WIDTH: f32 = 560.;

const CARD_TOP: f32 = 0.14;

const LIST_HEIGHT: f32 = 0.5;

const SEARCH_HEIGHT: f32 = 48.;

const COMMAND_HEIGHT: f32 = 44.;

pub(super) struct Palette {
    /// The highlighted command.
    index: usize,
    /// The registry's mods for the query [`asked`](Self::asked) last.
    pub(super) mods: Vec<Project>,
    asked: String,
    /// The registry search's number: an answer to an older one is dropped.
    pub(super) search: u64,
}

#[derive(Clone)]
enum Run {
    Play(String),
    Go(Page),
    NewInstance,
    Folder(PathBuf),
    Install(Box<Project>),
    Remove(String),
}

impl Run {
    fn key(&self) -> String {
        match self {
            Run::Play(id) => format!("play-{id}"),
            Run::Go(page) => format!("go-{page:?}"),
            Run::NewInstance => "new-instance".into(),
            Run::Folder(_) => "folder".into(),
            Run::Install(p) => format!("install-{}", p.id),
            Run::Remove(id) => format!("remove-{id}"),
        }
    }
}

struct Command {
    label: String,
    hint: String,
    run: Run,
}

impl Launcher {
    /// ⌘K: the palette opens, or closes when it is open.
    pub(super) fn palette_toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette.take().is_some() {
            return;
        }
        if self.close_menus() {
            if let Some(opener) = self.menu_opener.take() {
                window.focus(&opener, cx);
            }
        }
        ui::modal_opened_from("palette-scrim", window.focused(cx));
        self.fields.text(Field::PaletteSearch).set(String::new());
        let focus = self.fields.handle(Field::PaletteSearch, cx);
        window.focus(&focus, cx);
        self.palette = Some(Palette {
            index: 0,
            mods: Vec::new(),
            asked: String::new(),
            search: 0,
        });
    }

    pub(super) fn palette_close(&mut self) {
        self.palette = None;
    }

    /// The commands for `q` (lower case, trimmed).
    fn palette_commands(&self, q: &str) -> Vec<Command> {
        let instance = self
            .selected_instance()
            .map_or_else(|| "Eclipse".to_owned(), |i| i.name.clone());
        let mut all = Vec::new();
        for i in &self.instances.list {
            if matches!(i.kind, InstanceKind::Default | InstanceKind::Own) {
                all.push(Command {
                    label: trf("Play {name}", &[("name", &i.name)]),
                    hint: tr("Instance").to_string(),
                    run: Run::Play(i.id.clone()),
                });
            }
        }
        for page in Page::NAV {
            all.push(Command {
                label: trf("Open {page}", &[("page", page.label())]),
                hint: tr("Page").to_string(),
                run: Run::Go(page),
            });
        }
        all.push(Command {
            label: tr("New instance").to_string(),
            hint: tr("Instance").to_string(),
            run: Run::NewInstance,
        });
        if let Some(home) = self.selected_instance().map(|i| i.home.clone()) {
            all.push(Command {
                label: tr("Open the game folder").to_string(),
                hint: crate::toast::tilde(&home),
                run: Run::Folder(home),
            });
        }
        if q.chars().count() >= 2 {
            let mods = self.palette.as_ref().map_or(&[][..], |p| &p.mods[..]);
            for p in mods.iter().filter(|p| p.title.to_lowercase().contains(q)) {
                let has = self.mods_has(p);
                all.push(Command {
                    label: if has {
                        trf("Remove {name}", &[("name", &p.title)])
                    } else {
                        trf("Install {name}", &[("name", &p.title)])
                    },
                    hint: trf("Mod · {instance}", &[("instance", &instance)]),
                    run: if has {
                        Run::Remove(p.id.clone())
                    } else {
                        Run::Install(Box::new(p.clone()))
                    },
                });
            }
        }
        all.retain(|c| {
            q.is_empty() || c.label.to_lowercase().contains(q) || c.hint.to_lowercase().contains(q)
        });
        all.truncate(SHOWN);
        all
    }

    fn palette_run(&mut self, run: Run, cx: &mut Context<Self>) {
        self.palette_close();
        match run {
            Run::Play(id) => {
                let Some(instance) = self.instances.list.iter().find(|i| i.id == id).cloned()
                else {
                    return;
                };
                self.select_instance(&instance);
                self.go(Page::Home);
                self.press_play();
            }
            Run::Go(page) => self.go(page),
            Run::NewInstance => self.open_instance_page(None),
            Run::Folder(home) => {
                let _ = std::fs::create_dir_all(&home);
                crate::toast::open(cx, &home);
            }
            Run::Install(p) => {
                let p = *p;
                self.mods_install(p.source, p.id.clone(), Some(p));
            }
            Run::Remove(id) => {
                let _ = self.mods_remove(&id);
            }
        }
    }

    /// From two letters on, the registry is asked once for each new query.
    fn palette_ask(&mut self) {
        let q = self
            .fields
            .value(Field::PaletteSearch)
            .trim()
            .to_lowercase();
        let Some(p) = self.palette.as_mut() else {
            return;
        };
        if q.chars().count() < 2 || q == p.asked {
            return;
        }
        p.asked = q.clone();
        p.search += 1;
        let search = p.search;
        self.palette_mods(&q, search);
    }

    pub(super) fn palette_overlay(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Stateful<Div>> {
        self.palette.as_ref()?;
        let raw = self.fields.value(Field::PaletteSearch).to_string();
        let q = raw.trim().to_lowercase();
        let focused = self.fields.has_focus(Field::PaletteSearch);
        let focus = self.fields.handle(Field::PaletteSearch, cx);
        let input = line_edit::content(
            self.fields.text(Field::PaletteSearch),
            focused,
            false,
            false,
            "Search instances, mods and pages",
        );
        let list = self.palette_commands(&q);
        let index = self
            .palette
            .as_ref()
            .map_or(0, |p| p.index)
            .min(list.len().saturating_sub(1));
        let runs: Vec<Run> = list.iter().map(|c| c.run.clone()).collect();
        let count = list.len();
        let empty = list.is_empty();
        let size = window.viewport_size();
        let (w, h) = (f32::from(size.width), f32::from(size.height));

        let field = div()
            .id("palette-search")
            .role(gpui::Role::TextInput)
            .aria_label(tr("Search commands"))
            .aria_value(raw.clone())
            .track_focus(&focus)
            .flex()
            .items_center()
            .gap(px(space::MD))
            .h(px(SEARCH_HEIGHT))
            .px(px(space::MD))
            .cursor_text()
            .font_family(eclipse_theme::glass::FONT)
            .text_size(px(eclipse_theme::glass::step::LEAD.size))
            .text_color(white(ink::FULL))
            .child(ui::icon("search-01", ui::IconSize::Px16, white(ink::DIM)))
            .child(div().flex_1().min_w_0().overflow_hidden().child(input))
            .child(
                ui::text(
                    eclipse_theme::glass::step::CAPTION,
                    FontWeight::NORMAL,
                    ink::DIM,
                )
                .child(tr("Esc")),
            )
            .on_key_down(cx.listener(move |this, ev: &KeyDownEvent, _, cx| {
                let Some(p) = this.palette.as_mut() else {
                    return;
                };
                match ev.keystroke.key.as_str() {
                    "down" => p.index = p.index.saturating_add(1),
                    "up" => p.index = p.index.saturating_sub(1),
                    _ => match line_edit::handle(this.fields.text(Field::PaletteSearch), ev, cx) {
                        Key::Enter => {
                            let q = this
                                .fields
                                .value(Field::PaletteSearch)
                                .trim()
                                .to_lowercase();
                            let index = p.index;
                            let list = this.palette_commands(&q);
                            if let Some(c) = list.into_iter().nth(index) {
                                this.palette_run(c.run, cx);
                            }
                        }
                        Key::Escape => this.palette_close(),
                        Key::Edited => {
                            p.index = 0;
                            this.palette_ask();
                        }
                        Key::Pass => return,
                        _ => {}
                    },
                }
                // Past the last command the highlight stays on it.
                if this.palette.is_some() {
                    let q = this
                        .fields
                        .value(Field::PaletteSearch)
                        .trim()
                        .to_lowercase();
                    let last = this.palette_commands(&q).len().saturating_sub(1);
                    if let Some(p) = this.palette.as_mut() {
                        p.index = p.index.min(last);
                    }
                }
                cx.stop_propagation();
                cx.notify();
            }));

        let mut rows = div()
            .id("palette-list")
            .role(gpui::Role::ListBox)
            .aria_label(tr("Commands"))
            .flex()
            .flex_col()
            .pt(px(space::XS))
            .max_h(px(h * LIST_HEIGHT))
            .overflow_y_scroll();
        for (i, (c, run)) in list.into_iter().zip(runs).enumerate() {
            let id = SharedString::from(format!("palette-row-{}", run.key()));
            let label: SharedString = c.label.clone().into();
            rows = rows.child(
                ui::chosen(
                    div()
                        .id(id)
                        .role(gpui::Role::ListBoxOption)
                        .aria_label(label)
                        .aria_selected(i == index)
                        .aria_position_in_set(i + 1)
                        .aria_size_of_set(count)
                        .when(i == index, |d| d.aria_active_descendant())
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(space::LG))
                        .h(px(COMMAND_HEIGHT))
                        .px(px(space::MD))
                        .cursor_pointer(),
                    i == index,
                )
                .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                    if let (true, Some(p)) = (*hovered, this.palette.as_mut()) {
                        if p.index != i {
                            p.index = i;
                            cx.notify();
                        }
                    }
                }))
                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.palette_run(run.clone(), cx);
                    cx.notify();
                }))
                .child(
                    ui::text(
                        eclipse_theme::glass::step::BODY,
                        FontWeight::NORMAL,
                        ink::FULL,
                    )
                    .min_w_0()
                    .truncate()
                    .child(c.label),
                )
                .child(
                    ui::text(
                        eclipse_theme::glass::step::NOTE,
                        FontWeight::NORMAL,
                        ink::DIM,
                    )
                    .flex_none()
                    .child(c.hint),
                ),
            );
        }
        if empty {
            rows = rows.child(
                ui::text(
                    eclipse_theme::glass::step::BODY,
                    FontWeight::NORMAL,
                    ink::MUTED,
                )
                .px(px(space::MD))
                .py(px(space::LG))
                .child(trf("Nothing matches “{query}”.", &[("query", raw.trim())])),
            );
        }

        let card = ui::dialog_plate()
            .id("palette")
            .occlude()
            .w(px(CARD_WIDTH.min(w - 32.)))
            .p(px(space::SM))
            .child(div().relative().child(field).child(rows))
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation());
        let dismiss = cx.listener(|this, _: &MouseDownEvent, _, cx| {
            this.palette_close();
            cx.notify();
        });
        Some(
            ui::modal("palette-scrim", tr("Commands"), Some(Box::new(dismiss)))
                .flex()
                .justify_center()
                .items_start()
                .pt(px(h * CARD_TOP))
                .child(card),
        )
    }
}
