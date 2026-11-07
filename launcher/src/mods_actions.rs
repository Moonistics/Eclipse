use crate::fields::Field;
use crate::menus::Asking;
use crate::mods_state::{
    folder_busy, goes_to_trash, titles, Checking, Event, ModsBusy, CHECKING, PAGE,
};
use crate::{instance_state, menus, Launcher};
use eclipse_launcher::browser::installed::Done;
use eclipse_launcher::browser::{
    eclipse, installed, java, Kind as PackKind, Project, Query, SourceId,
};
use eclipse_launcher::i18n::{tr, trf};
use eclipse_launcher::{backend, browser, instances, native_status};
use gpui::prelude::*;
use gpui::px;
use std::collections::HashMap;

impl Launcher {
    pub(super) fn mods_search(&mut self, offset: usize) {
        self.mods_search_keeping(offset, false);
    }

    /// Searches; with `keep`, the rows already listed stay on screen until the new ones arrive,
    /// so the page doesn't shrink (and the scroll doesn't jump to the top) while it waits.
    pub(super) fn mods_search_keeping(&mut self, offset: usize, keep: bool) {
        let text = self.fields.value(Field::ModsSearch).to_owned();
        let instance = self.selected_instance().cloned();
        let m = &mut self.mods;
        m.search += 1;
        m.searching = true;
        m.asked = true;
        if offset == 0 {
            if !keep {
                m.results.clear();
                m.more = false;
                m.results_list.reset();
            }
            m.installed_list.reset();
        }
        let query = match m.java_for(m.kind) {
            Some(target) => Query {
                offset,
                limit: PAGE,
                ..target.search(&text, m.kind, &m.browse.filters)
            },
            None => Query {
                everything: m.everything,
                offset,
                limit: PAGE,
                filters: m.browse.filters.clone(),
                ..Query::new(&text, m.kind)
            },
        };
        // A Java instance's files are Modrinth's; Eclipse's registry is for Eclipse.
        let source = match m.java_for(m.kind) {
            Some(_) => SourceId::Modrinth,
            None => m.source,
        };
        let (tx, search) = (m.tx.clone(), m.search);
        let settings = self.settings.clone();
        std::thread::spawn(move || {
            let result = browser::source(source)
                .ok_or_else(|| format!("{} isn't set up.", source.label()))
                .and_then(|s| s.search(&query));
            let about: HashMap<String, Vec<String>> = match (&result, source) {
                (Ok(found), SourceId::Eclipse) => found
                    .iter()
                    .map(|p| {
                        let lines =
                            eclipse::trust_and_capabilities(&p.id, instance.as_ref(), &settings);
                        (p.id.clone(), lines)
                    })
                    .collect(),
                _ => HashMap::new(),
            };
            let _ = tx.send(Event::Found(search, offset, result));
            if !about.is_empty() {
                let _ = tx.send(Event::About(about));
            }
        });
    }

    /// Reads the registry's index again when something installed came from it, and turns off
    /// what it took down since (`eclipse::turn_off_revoked`).
    pub(super) fn mods_taken_down(&mut self) {
        if !self
            .mods
            .installed
            .iter()
            .any(|e| e.source == SourceId::Eclipse)
        {
            return;
        }
        let tx = self.mods.tx.clone();
        let home = self.mods.home();
        std::thread::spawn(move || {
            let Ok(index) = eclipse::index() else {
                return;
            };
            let lines = eclipse::turn_off_revoked(&index, &home);
            if !lines.is_empty() {
                let _ = tx.send(Event::TakenDown(lines));
            }
        });
    }

    /// At start, with the setting on: every Eclipse instance's registry projects update on a
    /// thread (`eclipse::auto::update_all`), the selected instance's list read again after.
    pub(super) fn mods_auto_update(&mut self) {
        if !self.settings.auto_updates_mods() {
            return;
        }
        let list: Vec<eclipse::auto::Target> = self
            .instances
            .list
            .iter()
            .map(|i| eclipse::auto::Target {
                level: self.version_of(i).map(|tag| backend::mod_api_level(tag)),
                natives: !eclipse::auto::pinned_to_nightly(i),
                instance: i.clone(),
            })
            .collect();
        let tx = self.mods.tx.clone();
        std::thread::spawn(move || {
            let (lines, rebuild) = eclipse::auto::update_all(&list);
            if !lines.is_empty() || !rebuild.is_empty() {
                let _ = tx.send(Event::AutoUpdated(lines, rebuild));
            }
        });
    }

    /// Turns a Eclipse mod the browser installed on or off, as the game's Options > Mods does;
    /// in a Java instance, renames its file.
    pub(super) fn mods_switch(&mut self, e: &installed::Entry) {
        let home = self.mods.home();
        // An update holds the folder: the window doesn't wait for it.
        let Some(_lock) = browser::try_lock_home(&home) else {
            self.mods.failed = Some(folder_busy());
            return;
        };
        let game = installed::Game::of(self.mods.java.clone());
        let on = !game.is_on(&home, e);
        let index = eclipse::known_index();
        self.mods_said(
            game.set_on(&home, e, on, index.as_deref())
                .map(|()| switched_line(&e.title, on)),
        );
        self.mods.read_installed();
    }

    /// Checks the Java mods among `found` that aren't checked yet, a few at a time, on threads:
    /// a cached check answers at once, else the jar is downloaded and read.
    pub(super) fn mods_check(&mut self, found: Vec<Project>) {
        let m = &mut self.mods;
        if m.java.is_some() {
            return;
        }
        let todo: Vec<Project> = found
            .into_iter()
            .filter(|p| p.kind == PackKind::Mod && p.source != SourceId::Eclipse)
            .filter(|p| p.support().installable())
            .filter(|p| !m.checks.contains_key(&(p.source, p.id.clone())))
            .collect();
        if todo.is_empty() {
            return;
        }
        for p in &todo {
            m.checks.insert((p.source, p.id.clone()), Checking::Running);
        }
        // A version other than 26.3 asked for: check the newest file for 1.21 and later.
        let everything = m.everything || m.browse.filters.other_version();
        let tx = m.tx.clone();
        let queue = std::sync::Mutex::new(todo.into_iter());
        std::thread::spawn(move || {
            // Checks are kept in the default game folder, shared by every instance.
            let home = backend::game_home();
            std::thread::scope(|s| {
                for _ in 0..CHECKING {
                    s.spawn(|| {
                        while let Some(p) = queue.lock().ok().and_then(|mut q| q.next()) {
                            let result = browser::source(p.source)
                                .ok_or_else(|| format!("{} isn't set up.", p.source.label()))
                                .and_then(|source| {
                                    java::check_project(&home, &*source, &p, everything, &|_| {})
                                });
                            // The project's page, if open, shows the verdict beside its version.
                            if let Ok((version, check)) = &result {
                                let key = (p.source, p.id.clone());
                                let _ =
                                    tx.send(Event::Loaded(super::mods_project::Loaded::Checked(
                                        key,
                                        version.id.clone(),
                                        check.verdict,
                                    )));
                            }
                            let result = result.map(|(_, check)| Box::new(check));
                            let _ = tx.send(Event::Checked((p.source, p.id), result));
                        }
                    });
                }
            });
        });
    }

    pub(super) fn mods_check_updates(&mut self) {
        let entries = self.mods.installed.clone();
        if entries.is_empty() {
            return;
        }
        let tx = self.mods.tx.clone();
        let game = installed::Game::of(self.mods.java.clone());
        let (home, settings) = (self.mods.home(), self.settings.clone());
        std::thread::spawn(move || {
            let level = instances::mod_api_level_at(&home, &settings);
            let mut found = HashMap::new();
            for e in entries {
                let Some(source) = browser::source(e.source) else {
                    continue;
                };
                let newer = game.update_for(&*source, &e, level);
                if let Ok(Some(v)) = newer {
                    found.insert((e.source, e.project.clone()), v.number);
                }
            }
            let _ = tx.send(Event::Updates(found));
        });
    }

    /// Updates every installed pack a newer version was found for, one after the other on a
    /// thread, each checked as an install is. One failing doesn't stop the others.
    pub(super) fn mods_update_all(&mut self) {
        if self.mods.busy.is_some() || self.mods.updates.is_empty() {
            return;
        }
        let mut todo: Vec<(SourceId, String, String)> = self
            .mods
            .installed
            .iter()
            .filter(|e| {
                self.mods
                    .updates
                    .contains_key(&(e.source, e.project.clone()))
            })
            .map(|e| (e.source, e.project.clone(), e.title.clone()))
            .collect();
        todo.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);
        self.mods.failed = None;
        self.mods.busy = Some(ModsBusy::UpdateAll {
            line: tr("Updating…").into(),
            bar: (0., 0.),
        });
        self.mods.updates.clear();
        let tx = self.mods.tx.clone();
        let home = self.mods.home();
        let game = installed::Game::of(self.mods.java.clone());
        let settings = self.settings.clone();
        std::thread::spawn(move || {
            let level = instances::mod_api_level_at(&home, &settings);
            let count = todo.len();
            let (mut done, mut failed) = (Vec::new(), Vec::new());
            for (i, (source_id, id, title)) in todo.into_iter().enumerate() {
                let step = |line: &str, f: f32| {
                    let overall = (i as f32 + f) / count as f32;
                    let _ = tx.send(Event::Progress(
                        format!("{} of {count}: {line}", i + 1),
                        overall,
                    ));
                };
                let updated = browser::source(source_id)
                    .ok_or_else(|| format!("{} isn't set up", source_id.label()))
                    .and_then(|source| {
                        let p = source.project(&id)?;
                        game.install(&*source, &p, true, &home, level, &step)
                    });
                match updated {
                    Ok(entries) => done.extend(
                        entries
                            .into_iter()
                            .filter(|e| e.project == id)
                            .map(|e| e.title),
                    ),
                    Err(e) => failed.push(format!("{title}: {e}")),
                }
            }
            let (names, failed) = (done.join(", "), failed.join("; "));
            let result = match (done.is_empty(), failed.is_empty()) {
                (true, true) => Ok(String::new()),
                (false, true) => Ok(trf("Updated {names}", &[("names", &names)])),
                (true, false) => Err(trf("Not updated: {failed}", &[("failed", &failed)])),
                (false, false) => Err(trf(
                    "Updated {names}. Not updated: {failed}",
                    &[("names", &names), ("failed", &failed)],
                )),
            };
            let _ = tx.send(Event::Done(result, None));
        });
    }

    /// Installs (or updates) a project on a thread: a pack into its folder, a modpack into
    /// Modpacks. `known` is the project when the page has it; else it is asked for first.
    pub(super) fn mods_install(&mut self, source: SourceId, id: String, known: Option<Project>) {
        if self.mods.busy.is_some() {
            return;
        }
        // The first native mod: the trust notice, before anything is installed.
        let first_native = source == SourceId::Eclipse
            && !self.settings.native_mods_noticed
            && eclipse::is_native_mod(&id);
        if first_native {
            self.mods.native_notice = Some((source, id, known));
            self.content_scroll
                .handle
                .set_offset(gpui::point(px(0.), px(0.)));
            return;
        }
        self.mods.native_notice = None;
        self.mods.failed = None;
        let name = known
            .as_ref()
            .map(|p| p.title.clone())
            .or_else(|| {
                let had = self.mods.installed.iter().find(|e| e.project == id);
                had.map(|e| e.title.clone())
            })
            .unwrap_or_else(|| id.clone());
        self.mods.busy = Some(ModsBusy::Install {
            id: id.clone(),
            name,
            line: trf("Asking {source}…", &[("source", source.label())]),
            bar: (0., 0.),
        });
        self.mods.updates.remove(&(source, id.clone()));
        let everything = self.mods.everything || self.mods.browse.filters.other_version();
        let tx = self.mods.tx.clone();
        let home = self.mods.home();
        let java = self.mods.java.clone();
        let settings = self.settings.clone();
        let instance = self.target_name();
        let world = self
            .datapack_world()
            .map(|w| (w.path.clone(), w.name.clone()));
        std::thread::spawn(move || {
            let last = std::cell::Cell::new(-1);
            let progress = |line: &str, f: f32| {
                let pct = (f * 100.) as i32;
                if last.replace(pct) != pct {
                    let _ = tx.send(Event::Progress(line.to_string(), f));
                }
            };
            let mut pack = None;
            let result = browser::source(source)
                .ok_or_else(|| format!("{} isn't set up.", source.label()))
                .and_then(|source| {
                    let p = match known {
                        Some(p) => p,
                        None => source.project(&id)?,
                    };
                    let world = match p.kind {
                        PackKind::DataPack => {
                            let pick = || tr("Pick the world the data pack goes into first.");
                            Some(world.ok_or_else(|| pick().to_string())?)
                        }
                        _ => None,
                    };
                    let to = installed::Destination {
                        home: &home,
                        settings: &settings,
                        java: java.as_ref(),
                        world: world.as_ref().map(|(path, _)| path.as_path()),
                        everything,
                    };
                    let log = |line: String| progress(&line, 1.);
                    Ok(
                        match installed::install_into(&*source, &p, &to, &progress, &log)? {
                            Done::DataPack { .. } => {
                                let name = world.map(|(_, name)| name).unwrap_or_default();
                                trf(
                                    "Added {title} to {world}",
                                    &[("title", &p.title), ("world", &name)],
                                )
                            }
                            Done::Pack(installed) => {
                                pack = Some(installed.work);
                                trf(
                                    "Installed {pack} as a new instance",
                                    &[("pack", &installed.name)],
                                )
                            }
                            Done::Port(imported) => {
                                pack = Some(imported.work.clone());
                                trf(
                                    "Installed {pack} as a new instance",
                                    &[("pack", imported.title())],
                                )
                            }
                            Done::Files(done) => {
                                installed_said(&p, &done, &instance, java.is_some())
                            }
                        },
                    )
                });
            let _ = tx.send(Event::Done(result, pack));
        });
    }

    /// The project `id` leaves the selected instance now; what happened.
    pub(super) fn mods_remove(&mut self, id: &str) -> Result<String, String> {
        let home = self.mods.home();
        let Some(_lock) = browser::try_lock_home(&home) else {
            self.mods.failed = Some(folder_busy());
            return Err(folder_busy());
        };
        let removed = installed::Game::of(self.mods.java.clone())
            .remove(&home, id, &eclipse_launcher::desktop_seams::to_trash)
            .map(|e| match goes_to_trash(e.kind) {
                true => trf("{name} moved to the trash", &[("name", &e.title)]),
                false => trf("Removed {name}", &[("name", &e.title)]),
            });
        if let Ok(said) = &removed {
            crate::toast::say(said.clone());
        }
        self.mods.failed = removed.clone().err();
        self.mods.read_installed();
        self.stop_asking(|a| matches!(a, Asking::Mod(_)));
        removed
    }

    /// The palette's search (`palette`): up to four mods for `text` that go in the selected
    /// instance (Eclipse's registry's, or Modrinth's for a Java instance's loader and version),
    /// answered as [`Event::Palette`].
    pub(super) fn palette_mods(&mut self, text: &str, search: u64) {
        self.mods_load();
        let (source, query) = match &self.mods.java {
            Some(target) if target.folder(PackKind::Mod).is_err() => return,
            Some(target) => (SourceId::Modrinth, target.query(text, PackKind::Mod)),
            None => (SourceId::Eclipse, Query::new(text, PackKind::Mod)),
        };
        let query = Query { limit: 4, ..query };
        self.mods.palette_failed = None;
        let tx = self.mods.tx.clone();
        std::thread::spawn(move || {
            let found = browser::source(source)
                .ok_or_else(|| format!("{} isn't set up.", source.label()))
                .and_then(|s| s.search(&query));
            let _ = tx.send(Event::Palette(search, found));
        });
    }

    pub(super) fn mods_add_fabric(&mut self, cx: &mut Context<Self>) {
        let Some(instance) = self.selected_instance().cloned() else {
            return;
        };
        self.open_instance_page(Some(instance.clone()));
        self.sync_launch_options(&instance, true);
        self.remember_draft();
        self.pick_kind(instance_state::NewKind::Modded, cx);
        self.change(|j| j.loader = eclipse_launcher::loaders::Kind::Fabric);
    }

    pub(super) fn mods_ask_remove(&mut self, id: String) {
        self.show_mods(PackKind::Mod);
        self.mods.picked = Some(id.clone());
        self.asking = Some(menus::Asking::Mod(id));
    }
}

fn switched_line(name: &str, on: bool) -> String {
    let args = [("name", name)];
    match on {
        true => trf(
            "{name} is on. Applies the next time the game starts.",
            &args,
        ),
        false => trf(
            "{name} is off. Applies the next time the game starts.",
            &args,
        ),
    }
}

fn installed_line(done: &[installed::Entry]) -> String {
    trf("Installed {names}", &[("names", &titles(done))])
}

fn installed_said(p: &Project, done: &[installed::Entry], instance: &str, java: bool) -> String {
    if java {
        return installed_line(done);
    }
    if done.iter().any(|e| native_status::is_native_file(&e.file)) {
        return trf(
            "Installed {names} in {instance}. Build to play with it.",
            &[("names", &titles(done)), ("instance", instance)],
        );
    }
    if p.kind == PackKind::ResourcePack {
        return trf(
            "Installed {names}. Turn it on in the game: Options > Resource Packs.",
            &[("names", &titles(done))],
        );
    }
    installed_line(done)
}
