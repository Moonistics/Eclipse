//! The palette (⌘K, Ctrl+K elsewhere): a card over the dimmed launcher, a search line, then up
//! to eight commands, each its name and a hint. Arrows move, Enter runs, Esc or a click outside
//! closes.
//!
//! The commands: Play each instance, Open each page and Settings, New instance, Open instance
//! folder, and from two letters on the registry's mods that match: Install one, or Open one
//! installed in the selected instance, its Remove… last (asked on the Mods page).

use super::*;
use crate::settings_page::SettingsSection;
use crate::ui;
use eclipse_launcher::browser::{Kind as PackKind, Project};
use eclipse_launcher::instances::Kind as InstanceKind;
use eclipse_theme::glass::{ink, space, step};
use std::path::PathBuf;

/// Commands listed at most.
const SHOWN: usize = 8;

const CARD_WIDTH: f32 = 560.;

const CARD_EDGE: f32 = space::LG;

const CARD_TOP: f32 = 0.14;

const LIST_HEIGHT: f32 = 0.5;

const LIST: &str = "palette-list";

pub(super) struct Palette {
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
    Settings(SettingsSection),
    Look(backend::Appearance),
    NewInstance,
    Folder(PathBuf),
    Install(Box<Project>),
    OpenMod(Box<Project>),
    AskRemove(String),
}

impl Run {
    fn key(&self) -> String {
        match self {
            Run::Play(id) => format!("play-{id}"),
            Run::Go(page) => format!("go-{page:?}"),
            Run::Settings(section) => format!("settings-{section:?}"),
            Run::Look(look) => format!("look-{look:?}"),
            Run::NewInstance => "new-instance".into(),
            Run::Folder(_) => "folder".into(),
            Run::Install(p) => format!("install-{}", p.id),
            Run::OpenMod(p) => format!("open-{}", p.id),
            Run::AskRemove(id) => format!("remove-{id}"),
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
        if self.close_menus(window, cx) {
            if let Some(opener) = self.menu_opener.take() {
                window.focus(&opener, cx);
            }
        }
        ui::modal_opened_from("palette-scrim", window.focused(cx));
        self.fields.text(Field::PaletteSearch).set(String::new());
        let focus = self.fields.handle(Field::PaletteSearch, cx);
        window.focus(&focus, cx);
        ui::set_options_cursor(LIST, 0);
        self.palette = Some(Palette {
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
                label: tr("Open instance folder").to_string(),
                hint: trf("Folder · {instance}", &[("instance", &instance)]),
                run: Run::Folder(home),
            });
        }
        for section in SettingsSection::shown(self.dev_options_shown()) {
            all.push(Command {
                label: trf("Open {section} settings", &[("section", section.name())]),
                hint: tr("Settings").to_string(),
                run: Run::Settings(section),
            });
        }
        let (look, label) = if self.theme.dark {
            (backend::Appearance::Light, tr("Switch to light mode"))
        } else {
            (backend::Appearance::Dark, tr("Switch to dark mode"))
        };
        all.push(Command {
            label: label.to_string(),
            hint: tr("Appearance").to_string(),
            run: Run::Look(look),
        });
        let mut removals = Vec::new();
        if q.chars().count() >= 2 {
            let mods = self.palette.as_ref().map_or(&[][..], |p| &p.mods[..]);
            for p in mods.iter().filter(|p| p.title.to_lowercase().contains(q)) {
                let hint = trf("Mod · {instance}", &[("instance", &instance)]);
                if !self.mods_has(p) {
                    all.push(Command {
                        label: trf("Install {name}", &[("name", &p.title)]),
                        hint,
                        run: Run::Install(Box::new(p.clone())),
                    });
                    continue;
                }
                all.push(Command {
                    label: trf("Open {name}", &[("name", &p.title)]),
                    hint: hint.clone(),
                    run: Run::OpenMod(Box::new(p.clone())),
                });
                removals.push(Command {
                    label: trf("Remove {name}…", &[("name", &p.title)]),
                    hint,
                    run: Run::AskRemove(p.id.clone()),
                });
            }
        }
        all.retain(|c| {
            q.is_empty() || c.label.to_lowercase().contains(q) || c.hint.to_lowercase().contains(q)
        });
        all.truncate(SHOWN.saturating_sub(removals.len().min(SHOWN / 2)));
        all.extend(removals);
        all.truncate(SHOWN);
        all
    }

    pub(super) fn palette_enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let q = self.palette_query();
        let at = ui::options_cursor(LIST);
        if let Some(c) = self.palette_commands(&q).into_iter().nth(at) {
            self.palette_run(c.run, window, cx);
        }
    }

    pub(super) fn palette_edited(&mut self) {
        ui::set_options_cursor(LIST, 0);
        self.palette_ask();
    }

    fn palette_query(&self) -> String {
        self.fields
            .value(Field::PaletteSearch)
            .trim()
            .to_lowercase()
    }

    fn palette_run(&mut self, run: Run, window: &mut Window, cx: &mut Context<Self>) {
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
            Run::Settings(section) => self.show_settings(section),
            Run::Look(look) => {
                self.settings.appearance = look;
                self.save_settings();
                self.restyle();
            }
            Run::NewInstance => self.open_instance_page(None),
            Run::Folder(home) => ui::open_folder(&home, cx),
            Run::Install(p) => {
                let p = *p;
                self.mods_install(p.source, p.id.clone(), Some(p));
            }
            Run::OpenMod(p) => {
                self.show_mods(p.kind);
                self.mods_open(*p);
                cx.on_next_frame(window, |this, window, cx| {
                    window.focus(&this.content_focus, cx);
                    window.focus_next(cx);
                });
            }
            Run::AskRemove(id) => {
                self.show_mods(PackKind::Mod);
                self.mods.picked = Some(id.clone());
                self.asking = Some(menus::Asking::Mod(id));
            }
        }
    }

    /// From two letters on, the registry is asked once for each new query.
    fn palette_ask(&mut self) {
        let q = self.palette_query();
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
        let finds_mods = self
            .selected_instance()
            .is_none_or(|i| i.java.as_ref().is_none_or(|j| j.loader.is_some()));
        let list = self.palette_commands(&q);
        let count = list.len();
        let cursor = ui::options_cursor(LIST).min(count.saturating_sub(1));
        let runs: Vec<Run> = list.iter().map(|c| c.run.clone()).collect();
        let size = window.viewport_size();
        let (w, h) = (f32::from(size.width), f32::from(size.height));
        let width = CARD_WIDTH.min(w - 2. * CARD_EDGE);
        let esc = ui::text(step::CAPTION, FontWeight::NORMAL, ink::DIM).child(tr("Esc"));
        let field = self
            .field_with(
                Field::PaletteSearch,
                Some(ui::field_icon("search-01")),
                if finds_mods {
                    tr("Search instances, mods and pages")
                } else {
                    tr("Search instances and pages")
                },
                width - 2. * space::SM,
                ui::ButtonSize::Lg,
                Some(esc.into_any_element()),
                cx,
            )
            .w_full();
        let field = div()
            .on_key_down(move |ev: &KeyDownEvent, window, cx| {
                let by = match ev.keystroke.key.as_str() {
                    "down" => 1,
                    "up" => -1,
                    _ => return,
                };
                ui::options_step(LIST, by, count);
                window.refresh();
                cx.stop_propagation();
            })
            .child(field);
        let rows = list
            .into_iter()
            .enumerate()
            .map(|(i, c)| {
                let id = SharedString::from(format!("palette-row-{}", runs[i].key()));
                let label: SharedString = c.label.into();
                let row = ui::menu_item(id, label.clone(), c.hint, false);
                (ui::selected(row, i == cursor), label, i == cursor)
            })
            .collect();
        let this = cx.entity().downgrade();
        let picked = runs.clone();
        let rows = ui::options(
            ui::nested_list(LIST)
                .flex()
                .flex_col()
                .gap(px(space::XXS))
                .pt(px(space::XS))
                .pr(px(Scrollbar::ROOM))
                .max_h(px(h * LIST_HEIGHT)),
            tr("Commands"),
            rows,
            false,
            move |i, window, cx| {
                let Some(run) = picked.get(i).cloned() else {
                    return;
                };
                let _ = this.update(cx, |this, cx| {
                    this.palette_run(run, window, cx);
                    cx.notify();
                });
            },
        );
        let empty = (count == 0).then(|| {
            let failed = self
                .palette
                .as_ref()
                .is_some_and(|p| self.palette_search_failed(p.search));
            ui::empty(
                "palette-empty",
                if failed {
                    tr("Couldn't search mods").to_string()
                } else {
                    trf("Nothing matches “{query}”.", &[("query", raw.trim())])
                },
            )
            .px(px(space::MD))
        });
        let card = ui::dialog_plate()
            .id("palette")
            .occlude()
            .w(px(width))
            .p(px(space::SM))
            .child(
                div()
                    .relative()
                    .child(field)
                    .children((count > 0).then(|| ui::with_scrollbar(rows)))
                    .children(empty),
            )
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
