//! The Mods page: its tabs are the kinds (mods, modpacks, resource packs, shader packs, data
//! packs). Search Eclipse's own registry for what was made for Eclipse, or Modrinth for Java
//! mods, modpacks, resource packs and data packs (a Java instance's files
//! always come from Modrinth, for its version), see where each comes
//! from and how well it runs (a Java mod's jar is checked first; a Eclipse mod says whether it is
//! signed and what it may do), install it with its checksum checked, turn Eclipse mods on and
//! off, and update or remove what was installed (`eclipse_launcher::browser`). Imported modpacks
//! are listed on its Modpacks tab.
use super::line_edit;
use super::*;
use crate::ui::{self, white};
use eclipse_launcher::browser::{
    self, eclipse, installed, java, java_instance, Kind as PackKind, Project, Query, SourceId,
    Support,
};
use eclipse_launcher::modpacks;
use eclipse_launcher::native_status;
use eclipse_port_pack::check::Check;
use eclipse_theme::glass::{fill, ink, radius, step};
use java_instance::Target;
use std::collections::HashMap;
use std::path::PathBuf;

/// The kinds, as the Mods page's tabs: name and icon.
const KINDS: [(PackKind, &str, &str); 5] = [
    (PackKind::Mod, "Mods", "box"),
    (PackKind::Modpack, "Modpacks", "archive"),
    (PackKind::ResourcePack, "Resource packs", "image"),
    (PackKind::Shader, "Shaders", "zap"),
    (PackKind::DataPack, "Data packs", "notes"),
];
/// Java mods checked at once.
const CHECKING: usize = 2;
/// The sources to pick from on the tabs that have both ([`switches`]).
pub(super) const SOURCES: [(SourceId, &str); 2] = [
    (SourceId::Eclipse, "Eclipse"),
    (SourceId::Modrinth, "Modrinth"),
];
const PAGE: usize = 20;
pub(super) const MOD_TILE: f32 = 40.;

#[path = "mods_view.rs"]
mod mods_view;

pub(super) enum Event {
    /// A search's answer, for the search it was (stale answers are dropped) at that offset.
    Found(u64, usize, Result<Vec<Project>, String>),
    Progress(String, f32),
    Done(Result<String, String>, bool),
    Updates(HashMap<(SourceId, String), String>),
    /// A Java mod's check: its jar read, or why it couldn't be.
    Checked((SourceId, String), Result<Box<Check>, String>),
    /// What a Eclipse registry project says before install: signed or not, what it may do.
    About(HashMap<String, Vec<String>>),
    /// Registry files taken down that the launcher turned off.
    TakenDown(Vec<String>),
    /// What the automatic updates did (`eclipse::auto`), and the instances (ids) whose native
    /// mods moved on: their game is built again before it plays.
    AutoUpdated(Vec<String>, Vec<String>),
    /// A project page's part, an image or a filter's values (`mods_project`).
    Loaded(super::mods_project::Loaded),
    /// The palette's registry mods for its search of that number.
    Palette(u64, Vec<Project>),
}

/// Where a Java mod's check is.
pub(super) enum Checking {
    Running,
    Done(Box<Check>),
    Failed(String),
}

pub(super) struct Mods {
    pub(super) kind: PackKind,
    pub(super) source: SourceId,
    everything: bool,
    /// Which search is the current one; answers for older ones are dropped.
    search: u64,
    searching: bool,
    searched: bool,
    results: Vec<Project>,
    results_list: cull::Cull,
    installed_list: cull::Cull,
    pub(super) report_list: cull::Cull,
    more: bool,
    error: Option<String>,
    pub(super) installed: Vec<installed::Entry>,
    loaded: bool,
    /// Whether the page searched for the instance as it is (its version and loader).
    asked: bool,
    /// The game folder the browser installs into and lists: the selected instance's.
    home: Option<PathBuf>,
    /// The selected Java instance's version and loader: its mods and resource packs are the
    /// source's own files for them, put in as they are.
    pub(super) java: Option<Target>,
    /// Its worlds (folder, name), the last played first, and the one data packs go into.
    worlds: Vec<(PathBuf, String)>,
    world: Option<PathBuf>,
    /// The project being installed, and the line and fraction of its download.
    pub(super) busy: Option<(String, String, f32)>,
    pub(super) message: Option<Result<String, String>>,
    pub(super) updates: HashMap<(SourceId, String), String>,
    pub(super) confirm_remove: Option<String>,
    /// Instances (ids) whose native mods the automatic update moved on: Play builds their game
    /// first (`build_native`), then plays it when `play_after_build` is set.
    pub(super) rebuild: Vec<String>,
    pub(super) play_after_build: bool,
    /// The project the detail card shows; the first result when none is.
    pub(super) picked: Option<String>,
    /// The project the detail card showed last.
    pub(super) shown: Option<Project>,
    pub(super) target_menu: bool,
    /// A native mod's Install waiting for the one-time trust notice (`native_status::NOTICE`).
    native_notice: Option<(SourceId, String, Option<Project>)>,
    /// The selected instance's native build state, and the `native_status::fingerprint` it was
    /// read for: read again only when that changes.
    /// With it, the mods turned on that pin the commit an older build compiled
    /// (`native_status::pinning`, for builds that didn't record it).
    native_state: std::cell::RefCell<Option<(String, native_status::State, Vec<String>)>>,
    native_files: std::cell::RefCell<Option<(String, Vec<native_status::Installed>)>>,
    /// Java mods' checks, by source and project.
    pub(super) checks: HashMap<(SourceId, String), Checking>,
    /// Eclipse registry projects' trust and capabilities, by id ([`eclipse::trust_and_capabilities`]).
    pub(super) about: HashMap<String, Vec<String>>,
    /// The project page, filters and images.
    pub(super) browse: super::mods_project::Browse,
    pub(super) tx: Sender<Event>,
    rx: Receiver<Event>,
}

impl Mods {
    pub(super) fn reset_lists(&self) {
        self.results_list.reset();
        self.installed_list.reset();
        self.report_list.reset();
    }

    /// The game folder installs go into: the selected instance's, once the page has read it.
    pub(super) fn home(&self) -> PathBuf {
        self.home.clone().unwrap_or_else(backend::game_home)
    }

    /// Reads the instance's worlds again; the world picked for data packs stays when it is
    /// still there, else the last played.
    fn read_worlds(&mut self) {
        self.worlds = eclipse_launcher::worlds::list(&self.home())
            .into_iter()
            .map(|w| (w.path, w.name))
            .collect();
        if !self
            .worlds
            .iter()
            .any(|(p, _)| Some(p) == self.world.as_ref())
        {
            self.world = self.worlds.first().map(|(p, _)| p.clone());
        }
    }

    /// Whether the Modpacks tab is open (the sidebar marks the pack shown there).
    pub(super) fn on_modpacks(&self) -> bool {
        self.kind == PackKind::Modpack
    }

    pub(super) fn new() -> Self {
        let (tx, rx) = channel();
        let page = std::env::var("ECLIPSE_LAUNCHER_PAGE");
        let kind = match page.as_deref() {
            Ok("modpacks") => PackKind::Modpack,
            Ok("resourcepacks") => PackKind::ResourcePack,
            Ok("shaders") => PackKind::Shader,
            Ok("datapacks") => PackKind::DataPack,
            _ => PackKind::Mod,
        };
        Self {
            kind,
            source: source_for(kind),
            everything: false,
            search: 0,
            searching: false,
            searched: false,
            results: Vec::new(),
            results_list: cull::Cull::default(),
            installed_list: cull::Cull::default(),
            report_list: cull::Cull::default(),
            more: false,
            error: None,
            installed: Vec::new(),
            loaded: false,
            asked: false,
            home: None,
            java: None,
            worlds: Vec::new(),
            world: None,
            busy: None,
            message: None,
            native_notice: None,
            native_state: Default::default(),
            native_files: Default::default(),
            updates: HashMap::new(),
            confirm_remove: None,
            rebuild: Vec::new(),
            play_after_build: false,
            picked: None,
            shown: None,
            target_menu: false,
            checks: HashMap::new(),
            about: HashMap::new(),
            browse: Default::default(),
            tx,
            rx,
        }
    }

    pub(super) fn entry(&self, p: &Project) -> Option<&installed::Entry> {
        self.installed
            .iter()
            .find(|e| e.source == p.source && e.project == p.id)
    }

    /// The Java instance `kind` goes into as files, when one is selected.
    pub(super) fn java_for(&self, kind: PackKind) -> Option<&Target> {
        self.java.as_ref().filter(|t| t.takes(kind))
    }

    /// Whether a Java mod's check is still to come before it can be installed.
    pub(super) fn checking(&self, p: &Project) -> bool {
        self.java_for(p.kind).is_none()
            && p.kind == PackKind::Mod
            && p.source != SourceId::Eclipse
            && matches!(
                self.checks.get(&(p.source, p.id.clone())),
                None | Some(Checking::Running)
            )
            && p.support().installable()
    }

    /// How well a result runs: a Java mod's from its check once it has one. In a Java
    /// instance, whether it has the loader and version the result was made for.
    pub(super) fn support(&self, p: &Project) -> Support {
        if let Some(target) = self.java_for(p.kind) {
            return target.support(p);
        }
        let metadata = p.support();
        if p.kind != PackKind::Mod || p.source == SourceId::Eclipse || !metadata.installable() {
            return metadata;
        }
        match self.checks.get(&(p.source, p.id.clone())) {
            Some(Checking::Done(c)) => java::support_of(c),
            Some(Checking::Failed(e)) => {
                Support::Partial(trf("Not checked: {error}", &[("error", e)]))
            }
            _ => Support::Partial(tr("Checking its jar…").into()),
        }
    }
}

/// `51404735` → `51.4M`.
fn count(n: u64) -> String {
    match n {
        0..1_000 => n.to_string(),
        1_000..1_000_000 => format!("{:.1}k", n as f64 / 1e3),
        _ => format!("{:.1}M", n as f64 / 1e6),
    }
}

/// A registry project's rating: five pixel stars, the mean's nearest whole number of them lit, the
/// mean and how many; None while the ratings aren't known.
pub(super) fn stars(id: &str) -> Option<Div> {
    let row = ui::text(step::NOTE, FontWeight::NORMAL, ink::MUTED)
        .flex()
        .items_center()
        .gap(px(2.));
    Some(match eclipse::rating(id)? {
        None => row.child(tr("No ratings yet")),
        Some(r) => {
            let lit = r.average.round() as usize;
            row.children((0..5).map(|i| {
                ui::icon(
                    "star",
                    ui::IconSize::Px16,
                    if i < lit {
                        ui::color(ui::look().mark)
                    } else {
                        white(ink::DIM)
                    },
                )
            }))
            .child(div().ml(px(4.)).child(format!(
                "{:.1} ({})",
                r.average,
                count(r.count)
            )))
        }
    })
}

impl Launcher {
    /// Opens the Mods page on its Modpacks tab, with the pack `slug` open when given (the
    /// sidebar's packs and its + button).
    pub(super) fn show_modpack(&mut self, slug: Option<String>) {
        self.go(Page::Mods);
        if slug.is_some() {
            self.pack_open = slug;
            self.mods.report_list.reset();
        }
        if self.mods.kind != PackKind::Modpack {
            self.mods.kind = PackKind::Modpack;
            self.mods.source = source_for(PackKind::Modpack);
            self.mods.browse.filters = Default::default();
            self.mods.browse.open = None;
            if self.mods.loaded {
                self.mods_search(0);
            }
        }
    }

    /// Called with the launcher's other work: what the background sent.
    pub(super) fn mods_tick(&mut self) -> bool {
        let mut changed = false;
        let mut to_check = Vec::new();
        let mut loaded = Vec::new();
        let mut done = false;
        while let Ok(event) = self.mods.rx.try_recv() {
            changed = true;
            let m = &mut self.mods;
            match event {
                Event::Palette(search, found) => {
                    if let Some(p) = self.palette.as_mut().filter(|p| p.search == search) {
                        p.mods = found;
                    }
                }
                Event::Found(search, offset, result) if search == m.search => {
                    m.searching = false;
                    m.searched = true;
                    match result {
                        Ok(found) => {
                            m.more = found.len() == PAGE;
                            m.results.truncate(offset);
                            if offset == 0 {
                                m.results_list.reset();
                            }
                            m.results.extend(found.iter().cloned());
                            m.error = None;
                            to_check.extend(found);
                        }
                        Err(e) => {
                            if offset == 0 {
                                m.results.clear();
                                m.more = false;
                                m.results_list.reset();
                            }
                            m.error = Some(e);
                        }
                    }
                }
                Event::Found(..) => {}
                Event::Progress(line, f) => {
                    if let Some(busy) = &mut m.busy {
                        busy.1 = line;
                        busy.2 = f;
                    }
                }
                Event::Done(result, modpack) => {
                    done = true;
                    m.busy = None;
                    if let Ok(said) = &result {
                        crate::toast::say(said.clone());
                    }
                    m.message = Some(result);
                    m.installed = installed::Installed::load(&m.home()).entries;
                    m.installed_list.reset();
                    if modpack {
                        // The Modpacks page reads its list again.
                        self.packs_loaded = None;
                    }
                }
                Event::Updates(found) => m.updates = found,
                Event::Checked(key, result) => {
                    let state = match result {
                        Ok(c) => Checking::Done(c),
                        Err(e) => Checking::Failed(e),
                    };
                    m.checks.insert(key, state);
                }
                Event::Loaded(l) => loaded.push(l),
                Event::About(about) => {
                    // Rows grow by a line: measure them again (`cull`).
                    m.about.extend(about);
                    m.results_list.reset();
                }
                Event::AutoUpdated(lines, rebuild) => {
                    m.rebuild.extend(rebuild);
                    if !lines.is_empty() {
                        m.message = Some(Ok(lines.join("\n")));
                        m.installed = installed::Installed::load(&m.home()).entries;
                        m.installed_list.reset();
                    }
                }
                Event::TakenDown(lines) => {
                    m.message = Some(Ok(lines.join("\n")));
                    m.installed = installed::Installed::load(&m.home()).entries;
                    m.installed_list.reset();
                }
            }
        }
        // `ECLIPSE_LAUNCHER_PROJECT=<slug>` (with `ECLIPSE_LAUNCHER_PAGE=mods`): opens that
        // project's page once a search finds it, for screenshots.
        if self.mods.browse.open.is_none() {
            if let Ok(want) = std::env::var("ECLIPSE_LAUNCHER_PROJECT") {
                if let Some(p) = self
                    .mods
                    .results
                    .iter()
                    .find(|p| p.id == want || p.slug == want)
                    .cloned()
                {
                    std::env::remove_var("ECLIPSE_LAUNCHER_PROJECT");
                    self.mods_open(p);
                }
            }
        }
        self.mods_check(to_check);
        if done {
            // A data pack put into a world shows in its list on Home.
            self.world_packs.reread();
        }
        for l in loaded {
            self.mods_loaded(l);
        }
        changed
    }

    pub(super) fn mods_search(&mut self, offset: usize) {
        self.mods_search_keeping(offset, false);
    }

    /// Searches; with `keep`, the rows already listed stay on screen until the new ones arrive,
    /// so the page doesn't shrink (and the scroll doesn't jump to the top) while it waits.
    pub(super) fn mods_search_keeping(&mut self, offset: usize, keep: bool) {
        let text = self.fields.value(Field::ModsSearch).to_owned();
        let m = &mut self.mods;
        m.search += 1;
        m.searching = true;
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
        std::thread::spawn(move || {
            let result = browser::source(source)
                .ok_or_else(|| format!("{} isn't set up.", source.label()))
                .and_then(|s| s.search(&query));
            let about: HashMap<String, Vec<String>> = match (&result, source) {
                (Ok(found), SourceId::Eclipse) => {
                    let instance = eclipse_launcher::instances::selected();
                    found
                        .iter()
                        .map(|p| {
                            let lines = eclipse::trust_and_capabilities(&p.id, Some(&instance));
                            (p.id.clone(), lines)
                        })
                        .collect()
                }
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
    fn mods_taken_down(&mut self) {
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
    /// thread ([`auto_update`]), the selected instance's list read again after.
    pub(super) fn mods_auto_update(&mut self) {
        if !self.settings.auto_updates_mods() {
            return;
        }
        let list: Vec<AutoTarget> = self
            .instances
            .list
            .iter()
            .map(|i| AutoTarget {
                level: self.version_of(i).map(|tag| backend::mod_api_level(tag)),
                natives: !pinned_to_nightly(i),
                instance: i.clone(),
            })
            .collect();
        let tx = self.mods.tx.clone();
        std::thread::spawn(move || {
            let (lines, rebuild) = auto_update(&list);
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
            self.mods.message = Some(Err(tr("Wait for the current operation to finish.").into()));
            return;
        };
        if self.mods.java.is_some() {
            let on = !java_instance::is_on(&home, e);
            self.mods.message = Some(java_instance::set_on(&home, e, on).map(|()| {
                format!(
                    "{} is {}. Applies the next time the game starts.",
                    e.title,
                    if on { "on" } else { "off" }
                )
            }));
            return;
        }
        let Some(id) = eclipse::mod_id(e) else {
            return;
        };
        let on = !java::is_enabled(&home, id);
        let revoked = eclipse::known_index().and_then(|index| {
            let hash = e.checksum.trim_start_matches("SHA-256:");
            index.revoked(&e.project, hash).map(|r| r.reason.clone())
        });
        self.mods.message = Some(match revoked.filter(|_| on) {
            Some(reason) => Err(format!("{} was taken down: {reason}", e.title)),
            None => java::set_enabled(&home, id, on).map(|()| {
                format!(
                    "{} is {}. Applies the next time the game starts.",
                    e.title,
                    if on { "on" } else { "off" }
                )
            }),
        });
    }

    /// Checks the Java mods among `found` that aren't checked yet, a few at a time, on threads:
    /// a cached check answers at once, else the jar is downloaded and read.
    fn mods_check(&mut self, found: Vec<Project>) {
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

    /// Looks for newer versions of what the browser installed.
    fn mods_check_updates(&mut self) {
        let entries = self.mods.installed.clone();
        if entries.is_empty() {
            return;
        }
        let tx = self.mods.tx.clone();
        let java = self.mods.java.clone();
        std::thread::spawn(move || {
            let mut found = HashMap::new();
            for e in entries {
                let Some(source) = browser::source(e.source) else {
                    continue;
                };
                let newer = match &java {
                    Some(target) => java_instance::update_for(&*source, target, &e),
                    None => installed::update_for(&*source, &e),
                };
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
        self.mods.message = None;
        self.mods.busy = Some(("all".into(), "Updating…".into(), 0.));
        self.mods.updates.clear();
        let tx = self.mods.tx.clone();
        let home = self.mods.home();
        let java = self.mods.java.clone();
        std::thread::spawn(move || {
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
                        match &java {
                            Some(target) => {
                                java_instance::install(&*source, target, &p, &home, &step)
                            }
                            None => installed::install(&*source, &p, true, &home, &step),
                        }
                    });
                match updated {
                    Ok(entries) => done.extend(
                        entries
                            .into_iter()
                            .filter(|e| e.project == id)
                            .map(|e| format!("{} {}", e.title, e.number)),
                    ),
                    Err(e) => failed.push(format!("{title}: {e}")),
                }
            }
            let updated = match done.is_empty() {
                true => String::new(),
                false => format!("Updated {}.", done.join(", ")),
            };
            let result = match failed.is_empty() {
                true => Ok(updated),
                false => Err(format!("{updated} Not updated: {}", failed.join("; "))
                    .trim()
                    .to_string()),
            };
            let _ = tx.send(Event::Done(result, false));
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
        self.mods.message = None;
        self.mods.busy = Some((id.clone(), format!("Asking {}…", source.label()), 0.));
        self.mods.updates.remove(&(source, id.clone()));
        let everything = self.mods.everything || self.mods.browse.filters.other_version();
        // A modpack is ported for the selected version as soon as it is imported.
        let resources = self
            .selected()
            .filter(|tag| backend::is_prepared(tag))
            .map(|tag| backend::resources(tag));
        let tx = self.mods.tx.clone();
        let home = self.mods.home();
        let java = self.mods.java.clone();
        let instance = self
            .selected_instance()
            .map_or_else(|| "Eclipse".to_string(), |i| i.name.clone());
        let world = self.mods.world.clone().and_then(|path| {
            let (_, name) = self.mods.worlds.iter().find(|(p, _)| *p == path)?;
            Some((path, name.clone()))
        });
        std::thread::spawn(move || {
            let last = std::cell::Cell::new(-1);
            let progress = |line: &str, f: f32| {
                let pct = (f * 100.) as i32;
                if last.replace(pct) != pct {
                    let _ = tx.send(Event::Progress(line.to_string(), f));
                }
            };
            let mut modpack = false;
            let result = browser::source(source)
                .ok_or_else(|| format!("{} isn't set up.", source.label()))
                .and_then(|source| {
                    let p = match known {
                        Some(p) => p,
                        None => source.project(&id)?,
                    };
                    if let Some(target) = java.as_ref().filter(|t| t.takes(p.kind)) {
                        let done = java_instance::install(&*source, target, &p, &home, &progress)?;
                        let names: Vec<String> = done
                            .iter()
                            .map(|e| format!("{} {}", e.title, e.number))
                            .collect();
                        return Ok(format!("Installed {}.", names.join(", ")));
                    }
                    if p.kind == PackKind::DataPack {
                        let (world, name) =
                            world.ok_or("Pick the world the data pack goes into first.")?;
                        let done =
                            installed::install_datapack(&*source, &p, everything, &world, &|f| {
                                progress(&p.title, f)
                            })?;
                        return Ok(format!(
                            "Put {} {} in {name}'s data packs, checksum checked: the game turns \
                             it on when the world opens. {}",
                            p.title,
                            done.number,
                            done.coverage.summary()
                        ));
                    }
                    modpack = p.kind == PackKind::Modpack;
                    if modpack && p.source == SourceId::Eclipse {
                        let pack = eclipse::pack::install(&p.id, &progress)?;
                        if let Some(url) = &p.icon {
                            eclipse_launcher::instances::adopt_icon(
                                &eclipse::pack::home(&pack.work),
                                url,
                            );
                        }
                        return Ok(format!(
                            "Installed {} {} as its own profile with {}, each checked. Play it \
                             from the list above.",
                            pack.name,
                            pack.version,
                            pack.mods.join(", ")
                        ));
                    }
                    if modpack {
                        let folder = modpacks::ports_dir().join("downloads");
                        let file = installed::fetch_modpack(&*source, &p, &folder, &|f| {
                            progress(&p.title, f)
                        })?;
                        let log = |line: String| progress(&line, 1.);
                        let pack = modpacks::import_and_port(&file, resources.as_deref(), &log)?;
                        if let Some(url) = &p.icon {
                            eclipse_launcher::instances::adopt_icon(
                                &modpacks::profile_home(&pack.work),
                                url,
                            );
                        }
                        return Ok(modpacks::outcome(&pack));
                    }
                    let done = installed::install(&*source, &p, everything, &home, &progress)?;
                    let names: Vec<String> = done
                        .iter()
                        .map(|e| format!("{} {}", e.title, e.number))
                        .collect();
                    if done.iter().any(|e| native_status::is_native_file(&e.file)) {
                        return Ok(format!(
                            "Installed {} into {instance}. Build to play with it.",
                            names.join(", ")
                        ));
                    }
                    if p.source == SourceId::Eclipse && p.kind == PackKind::Mod {
                        return Ok(format!(
                            "Installed {} into eclipse-mods and turned it on, checksum checked \
                             (Options > Mods in the game).",
                            names.join(", ")
                        ));
                    }
                    if p.kind == PackKind::Mod {
                        return Ok(format!(
                            "Ported {} into eclipse-mods and turned on (Options > Mods in the \
                             game).",
                            names.join(", ")
                        ));
                    }
                    Ok(format!(
                        "Installed {}, checksums checked. Turn it on in the game: Options > \
                         Resource Packs.",
                        names.join(", ")
                    ))
                });
            let _ = tx.send(Event::Done(result, modpack));
        });
    }

    pub(super) fn mods_field(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let focused = self.fields.has_focus(Field::ModsSearch);
        let placeholder = self.mods_placeholder();
        let content = line_edit::content_with(
            self.fields.text(Field::ModsSearch),
            focused,
            false,
            false,
            placeholder,
        );
        let field = ui::field(focused, false)
            .id("mods-search")
            .gap(px(10.))
            .w_full()
            .h(px(ui::Size::Md.height()))
            .pl(px(12.))
            .cursor_text()
            .child(ui::icon("search-01", ui::IconSize::Px16, white(ink::MUTED)))
            .child(content)
            .when(focused, |d| d.child(ui::focus_ring(radius::ROW)));
        self.text_input(Field::ModsSearch, field, cx)
    }

    /// Reads what the browser installed in the selected instance (once per instance) and looks
    /// for updates: the Mods page and Home's Mods tab (`instance_mods.rs`) show it. Packs and mods
    /// go into the selected instance: picking another reads its list.
    pub(super) fn mods_load(&mut self) {
        let home = self.selected_instance().map(|i| i.home.clone());
        let java = self
            .selected_instance()
            .and_then(|i| i.java.as_ref())
            .map(|j| j.target());
        // Another version or loader: other results, on the page's next draw.
        if java != self.mods.java {
            self.mods.asked = false;
        }
        self.mods.java = java;
        if !self.mods.loaded || (home.is_some() && self.mods.home != home) {
            self.mods.home = home;
            self.mods.installed = installed::Installed::load(&self.mods.home()).entries;
            self.mods.installed_list.reset();
            self.mods.updates.clear();
            self.mods.confirm_remove = None;
            self.mods.read_worlds();
            self.mods_check_updates();
            self.mods_taken_down();
            self.mods.loaded = true;
        }
    }

    /// Opens a kind's tab: its first source, its own filters, a new search.
    pub(super) fn mods_set_kind(&mut self, kind: PackKind) {
        if self.mods.kind != kind {
            self.mods.kind = kind;
            self.mods.source = source_for(kind);
            // A data pack goes into a world: one may have been made since.
            if kind == PackKind::DataPack {
                self.mods.read_worlds();
            }
            // Categories and loaders differ by kind.
            self.mods.browse.filters = Default::default();
            self.mods.browse.open = None;
            self.mods.picked = None;
            self.mods_search(0);
        }
    }

    /// The project `id` leaves the selected instance now; what happened, for the page's notice.
    pub(super) fn mods_remove(&mut self, id: &str) -> Result<String, String> {
        let home = self.mods.home();
        let Some(_lock) = browser::try_lock_home(&home) else {
            let busy: Result<String, String> =
                Err(tr("Wait for the current operation to finish.").into());
            self.mods.message = Some(busy.clone());
            return busy;
        };
        let removed = match self.mods.java {
            Some(_) => java_instance::remove(&home, id),
            None => installed::remove(&home, id),
        }
        .map(|e| trf("Removed {name}.", &[("name", &e.title)]));
        if let Ok(said) = &removed {
            crate::toast::say(said.clone());
        }
        self.mods.message = Some(removed.clone());
        self.mods.installed = installed::Installed::load(&home).entries;
        self.mods.installed_list.reset();
        self.mods.confirm_remove = None;
        removed
    }

    /// The palette's search (`palette`): up to four of Eclipse's registry mods for `text`,
    /// answered as [`Event::Palette`]. None for a Java instance, which takes Modrinth's files.
    pub(super) fn palette_mods(&mut self, text: &str, search: u64) {
        self.mods_load();
        if self.mods.java.is_some() {
            return;
        }
        let tx = self.mods.tx.clone();
        let query = Query {
            limit: 4,
            ..Query::new(text, PackKind::Mod)
        };
        std::thread::spawn(move || {
            let found = browser::source(SourceId::Eclipse)
                .ok_or_else(|| "not set up".to_string())
                .and_then(|s| s.search(&query))
                .unwrap_or_default();
            let _ = tx.send(Event::Palette(search, found));
        });
    }

    pub(super) fn mods_status(
        &self,
        p: &Project,
        instance: &str,
    ) -> Option<(ui::Tone, String, Option<String>)> {
        let support = self.mods.support(p);
        let reason = support.reason().map(str::to_owned);
        if self.mods.java_for(p.kind).is_some() {
            return Some(match support {
                Support::Works => (
                    ui::Tone::Live,
                    trf("Works in {instance}", &[("instance", instance)]),
                    None,
                ),
                _ => (ui::Tone::Warn, reason.unwrap_or_default(), None),
            });
        }
        if self.mods.java.is_some() && p.kind != PackKind::Modpack {
            return None;
        }
        let in_eclipse = trf("{support} in Eclipse", &[("support", tr(support.label()))]);
        Some(match support {
            Support::Works => (ui::Tone::Live, in_eclipse, None),
            Support::Partial(_) => (ui::Tone::Warn, in_eclipse, reason),
            Support::No(_) => (ui::Tone::Off, in_eclipse, reason),
        })
    }

    pub(super) fn mods_new_fabric(&mut self, minecraft: String, cx: &mut Context<Self>) {
        self.open_instance_page(None);
        self.pick_kind(new_instance::NewKind::Modded, cx);
        self.change(|j| {
            j.loader = eclipse_launcher::loaders::Kind::Fabric;
            j.version = Some(minecraft);
        });
    }

    pub(super) fn mods_vanilla_target(&self) -> Option<String> {
        self.mods
            .java_for(self.mods.kind)
            .filter(|t| t.loader.is_none() && self.mods.kind == PackKind::Mod)
            .map(|t| t.minecraft.clone())
    }

    /// Whether the selected instance has `p` (the palette's Install or Remove).
    pub(super) fn mods_has(&self, p: &Project) -> bool {
        self.mods.entry(p).is_some()
    }

    pub(super) fn mods_page(&mut self, cx: &mut Context<Self>) -> Div {
        self.mods_load();
        if !self.mods.asked {
            self.mods.asked = true;
            self.mods_search(0);
        }
        self.mods_view(cx)
    }

    fn mods_target_line(&self, cx: &mut Context<Self>) -> Option<Div> {
        let instance = self.selected_instance()?;
        (self.mods.kind == PackKind::DataPack)
            .then(|| self.mods_world_picker(&instance.name.clone(), cx))
    }

    /// The world data packs go into: one of the instance's, picked here.
    fn mods_world_picker(&self, instance: &str, cx: &mut Context<Self>) -> Div {
        let note = |s: String| ui::text(step::NOTE, FontWeight::NORMAL, ink::MUTED).child(s);
        if self.mods.worlds.is_empty() {
            return div().child(note(trf(
                "{instance} has no worlds yet. Create one in the game first.",
                &[("instance", instance)],
            )));
        }
        let mut chips = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(6.))
            .child(note(tr("World").to_string()).mr(px(6.)));
        for (i, (path, name)) in self.mods.worlds.iter().enumerate() {
            let on = self.mods.world.as_ref() == Some(path);
            let path = path.clone();
            chips = chips.child(
                ui::selected(
                    ui::button(
                        SharedString::from(format!("mods-world-{i}")),
                        None,
                        name.clone(),
                        ButtonKind::Ghost,
                        ui::Size::Sm,
                    ),
                    on,
                )
                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.mods.world = Some(path.clone());
                    cx.notify();
                })),
            );
        }
        chips
    }

    /// The one-time notice before the first native mod is installed: native mods aren't
    /// sandboxed, and what the build takes. Its button installs the mod.
    fn native_notice(&self, cx: &mut Context<Self>) -> Option<Div> {
        self.mods.native_notice.as_ref()?;
        Some(
            ui::card(radius::PANEL)
                .flex()
                .flex_col()
                .gap(px(16.))
                .p(px(ui::CARD_PADDING))
                .child(ui::error_note(tr(native_status::NOTICE)))
                .child(
                    div()
                        .flex()
                        .gap(px(8.))
                        .child(
                            button("native-notice-install", "Install", ButtonKind::Primary)
                                .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.settings.native_mods_noticed = true;
                                    this.save_settings();
                                    if let Some((source, id, known)) =
                                        this.mods.native_notice.take()
                                    {
                                        this.mods_install(source, id, known);
                                    }
                                    cx.notify();
                                })),
                        )
                        .child(
                            button("native-notice-cancel", "Cancel", ButtonKind::Secondary)
                                .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.mods.native_notice = None;
                                    cx.notify();
                                })),
                        ),
                ),
        )
    }

    /// The install under way and the last install's answer.
    pub(super) fn mods_busy_plate(&self) -> Vec<Div> {
        let mut out = Vec::new();
        if let Some((what, line, f)) = &self.mods.busy {
            let caption = if what == "all" {
                tr("Updating")
            } else {
                tr("Installing")
            };
            out.push(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(12.))
                            .child(ui::label(caption))
                            .child(
                                ui::text(step::NOTE, FontWeight::MEDIUM, ink::BODY)
                                    .child(format!("{:.0}%", f * 100.)),
                            ),
                    )
                    .child(ui::bar(*f))
                    .child(
                        ui::text(step::NOTE, FontWeight::NORMAL, ink::MUTED)
                            .truncate()
                            .child(line.clone()),
                    ),
            );
        }
        match &self.mods.message {
            Some(Ok(done)) => {
                out.push(ui::text(step::BODY, FontWeight::NORMAL, ink::FULL).child(done.clone()))
            }
            Some(Err(e)) => out.push(ui::error_note(e.clone())),
            None => {}
        }
        out
    }

    /// Re-read local/GitHub files when the folder changes, not on every frame.
    fn native_files(&self) -> Vec<native_status::Installed> {
        let home = self.mods.home();
        let key = native_status::fingerprint(&home, None);
        let mut memo = self.mods.native_files.borrow_mut();
        if memo.as_ref().is_none_or(|(k, _)| *k != key) {
            *memo = Some((key, native_status::untracked(&home)));
            self.mods.installed_list.reset();
        }
        memo.as_ref().unwrap().1.clone()
    }

    fn mods_installed_list(&mut self, cx: &mut Context<Self>) -> Option<Div> {
        let kind = self.mods.kind;
        // Each project's own icon (Modrinth's, asked for once), at the row's size.
        let ids = self
            .mods
            .installed
            .iter()
            .filter(|e| e.kind == kind && e.source == SourceId::Modrinth)
            .map(|e| e.project.clone())
            .collect();
        self.ask_project_icons(ids, cx);
        let icons: Vec<Option<PathBuf>> = self
            .mods
            .installed
            .clone()
            .iter()
            .filter(|e| e.kind == kind)
            .map(|e| self.project_icon(e))
            .collect();
        let entries: Vec<&installed::Entry> = self
            .mods
            .installed
            .iter()
            .filter(|e| e.kind == kind)
            .collect();
        let native_entries = if kind == PackKind::Mod && self.mods.java.is_none() {
            self.native_files()
        } else {
            Vec::new()
        };
        if entries.is_empty() && native_entries.is_empty() {
            return None;
        }
        let busy = self.mods.busy.is_some();
        let native = !native_entries.is_empty()
            || self.mods.java.is_none()
                && entries
                    .iter()
                    .any(|e| native_status::is_native_file(&e.file));
        let row = |i: usize| {
            if i >= entries.len() {
                return self.native_mod_row(&native_entries[i - entries.len()], busy, cx);
            }
            let e = entries[i];
            let key = (e.source, e.project.clone());
            let is_native = self.mods.java.is_none() && native_status::is_native_file(&e.file);
            let mut right = div().flex().flex_none().items_center().gap(px(8.));
            if let Some(number) = self.mods.updates.get(&key) {
                let mut update = button(
                    SharedString::from(format!("mods-update-{}", e.project)),
                    trf("Update to {version}", &[("version", number)]),
                    ButtonKind::Secondary,
                );
                if busy {
                    update = disabled(update);
                } else {
                    let (source, id) = key.clone();
                    update = update.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.mods_install(source, id.clone(), None);
                        cx.notify();
                    }));
                }
                right = right.child(update);
            }
            let confirming = self.mods.confirm_remove.as_deref() == Some(e.project.as_str());
            let remove_id = SharedString::from(format!("mods-remove-{}", e.project));
            let id = e.project.clone();
            let remove = button(remove_id.clone(), "Remove…", ButtonKind::Ghost).on_press(
                cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.mods.confirm_remove = Some(id.clone());
                    cx.notify();
                }),
            );
            let on = match &self.mods.java {
                _ if e.kind != PackKind::Mod => None,
                Some(_) => Some(java_instance::is_on(&self.mods.home(), e)),
                None if e.source == SourceId::Eclipse => Some(
                    eclipse::mod_id(e).is_some_and(|id| java::is_enabled(&self.mods.home(), id)),
                ),
                None => None,
            };
            if let Some(on) = on {
                let entry = e.clone();
                right = right.child(
                    ui::toggle_label(
                        SharedString::from(format!("mods-switch-{}", e.project)),
                        if on { tr("On") } else { tr("Off") },
                        on,
                    )
                    .on_press(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            this.mods_switch(&entry);
                            cx.notify();
                        },
                    )),
                );
            }
            if is_native {
                let folder = self.mods.home().join(native_mods::INSTALLED);
                right = right.child(
                    ui::icon_button(
                        SharedString::from(format!("mods-folder-{}", e.project)),
                        "folder-01",
                        tr("Open folder"),
                        ui::Size::Sm,
                        false,
                    )
                    .on_press(move |_, _, cx| {
                        let _ = std::fs::create_dir_all(&folder);
                        crate::toast::open(cx, &folder);
                    }),
                );
            }
            right = right.child(remove);
            if confirming {
                let id = e.project.clone();
                right = div().flex_none().child(ui::confirm(
                    remove_id,
                    trf("Remove {name}?", &[("name", &e.title)]),
                    tr("Remove"),
                    cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.mods.confirm_remove = None;
                        cx.notify();
                    }),
                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                        let _ = this.mods_remove(&id);
                        cx.notify();
                    }),
                ));
            }
            let from = e
                .source
                .shown_for(&e.project)
                .map(|s| trf("from {source}", &[("source", s)]));
            let needed = e
                .dependency
                .then(|| tr("needed by another pack").to_string());
            let note = e
                .verdict
                .iter()
                .map(|v| tr(v).to_string())
                .chain([e.number.clone()])
                .chain(from)
                .chain(needed)
                .collect::<Vec<_>>()
                .join(" · ");
            let skipped = (!e.skipped.is_empty()).then(|| {
                ui::text(step::NOTE, FontWeight::NORMAL, ink::MUTED).child(ui::state_line(
                    ui::Tone::Warn,
                    trf("Not ported: {item}", &[("item", &e.skipped.join(", "))]),
                ))
            });
            let icon = icons::slot(
                icons[i].clone().map(gpui::ImageSource::from),
                &e.title,
                MOD_TILE,
            );
            ui::list_row(
                SharedString::from(format!("mods-installed-{}", e.project)),
                confirming,
                |_, _, _| {},
            )
            .child(icon)
            .child(ui::item_text(e.title.clone(), note).children(skipped))
            .child(right)
        };
        // Every kind's updates at once, whichever tab is open.
        let update_all = (!self.mods.updates.is_empty()).then(|| {
            let b = button(
                "mods-update-all",
                trf(
                    "Update all ({count})",
                    &[("count", &self.mods.updates.len().to_string())],
                ),
                ButtonKind::Secondary,
            );
            if busy {
                disabled(b)
            } else {
                b.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.mods_update_all();
                    cx.notify();
                }))
            }
        });
        let list = self.mods.installed_list.column(
            &self.content_scroll.handle,
            entries.len() + native_entries.len(),
            0.,
            |index| {
                entries
                    .get(index)
                    .is_some_and(|e| self.mods.confirm_remove.as_ref() == Some(&e.project))
            },
            row,
        );
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(12.))
                        .child(ui::card_title(tr("Installed")))
                        .children(update_all),
                )
                .children(native.then(|| self.native_build(cx)).flatten())
                .child(list),
        )
    }

    /// The selected instance's native build (docs/native-mods.md): which build it plays, the
    /// Eclipse it was built on, and whether it holds the native mods turned on (Up to date,
    /// Needs rebuild, Not built), with Build, and Update when a newer Eclipse is installed. Both
    /// open the trust warning first (`native_warning`). A build under way, or its error, shows
    /// only on the instance it is for.
    pub(super) fn native_build(&self, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        use native_status::State;
        let instance = self.selected_instance()?;
        let version = self.version_of(instance).cloned();
        let home = self.mods.home();
        let key = native_status::fingerprint(&home, version.as_deref());
        let build = version.as_deref().and_then(|v| self.native_build_of(v));
        let mut memo = self.mods.native_state.borrow_mut();
        let (state, pinned) = match memo.as_ref() {
            Some((k, state, pinned)) if *k == key => (*state, pinned.clone()),
            _ => {
                let built = version.as_deref().and_then(native_status::built);
                let state = native_status::state(&native_status::enabled(&home), built.as_deref());
                let pinned = match &build {
                    Some(b) if b.pinned_unknown => native_status::pinning(&home, &b.eclipse),
                    _ => Vec::new(),
                };
                *memo = Some((key, state, pinned.clone()));
                (state, pinned)
            }
        };
        let pinned = match &build {
            Some(b) if !b.pinned_by.is_empty() => b.pinned_by.clone(),
            _ => pinned,
        };
        // Rebuilding on a newer Eclipse, or on the build's own one when mods pinned another
        // commit, builds every mod on that Eclipse's commit (`on_base`).
        let update = build
            .as_ref()
            .filter(|_| state != State::NoMods)
            .and_then(|b| {
                native_mods::newer_than(&b.base, &self.installed).or_else(|| {
                    (!pinned.is_empty() && self.installed.contains(&b.base)).then(|| b.base.clone())
                })
            });
        let mine = self.native_for.as_deref() == Some(instance.id.as_str());
        let step = self.native_step.as_ref().filter(|_| mine);
        let error = self.native_error.as_ref().filter(|_| mine);
        let name = [("instance", instance.name.as_str())];
        let line = |s: String| div().child(s);
        let (status, badge) = match (step, error, state) {
            (Some(step), _, _) => (
                line(format!(
                    "{step} - {}",
                    native_mods::duration_label(
                        self.native_started.map_or(0, |(s, _)| s.elapsed().as_secs())
                    )
                ))
                .text_color(ui::color(ui::look().accent_soft)),
                Some((ui::Tone::Busy, tr("Building"))),
            ),
            (None, Some(e), _) => (ui::error_note(e.clone()), None),
            (None, None, State::NoMods) => (line(tr("No native mod is turned on.").into()), None),
            (None, None, State::NotBuilt) => (
                line(trf(
                    "{instance} doesn't play its native mods yet: build the game with them.",
                    &name,
                )),
                Some((ui::Tone::Warn, tr("Not built"))),
            ),
            (None, None, State::Outdated) => (
                line(trf(
                    "The native mods turned on changed since {instance} was built: build again to play them.",
                    &name,
                )),
                Some((ui::Tone::Warn, tr("Needs rebuild"))),
            ),
            (None, None, State::UpToDate) => (
                line(trf("Up to date: {instance} plays these mods.", &name)),
                Some((ui::Tone::Live, tr("Up to date"))),
            ),
        };
        let mut detail = ui::text(step::NOTE, FontWeight::NORMAL, ink::BODY)
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(status);
        if let Some(done) = self.native_done.as_ref().filter(|_| mine && step.is_none()) {
            detail = detail.child(div().text_color(white(ink::MUTED)).child(done.clone()));
        }
        if let (Some(tag), Some(build)) = (&version, &build) {
            let said = match pinned.is_empty() {
                true => trf(
                    "Plays {build}, built on Eclipse {version}.",
                    &[("build", tag), ("version", &build.eclipse())],
                ),
                false => trf(
                    "Plays {build}, built on Eclipse {commit}, the commit pinned by {mods}, not on {version}.",
                    &[
                        ("build", tag),
                        ("commit", &build.eclipse[..build.eclipse.len().min(7)]),
                        ("mods", &pinned.join(", ")),
                        ("version", &build.eclipse()),
                    ],
                ),
            };
            detail = detail.child(div().text_color(white(ink::MUTED)).child(said));
            for left in &build.left_out {
                detail = detail.child(div().text_color(ui::warn()).child(left.label()));
            }
        }
        if self.native_step.is_some() && !mine {
            let other = self
                .native_for
                .as_deref()
                .and_then(|id| self.instances.list.iter().find(|i| i.id == id));
            detail = detail.child(div().text_color(white(ink::MUTED)).child(trf(
                "{instance} is building. Wait for it to finish.",
                &[("instance", other.map_or("", |i| i.name.as_str()))],
            )));
        }
        let mismatch = self
            .native_step
            .is_none()
            .then_some(self.native_mismatch.as_ref())
            .flatten();
        let idle =
            self.native_step.is_none() && self.native_confirm.is_none() && mismatch.is_none();
        let can_build =
            idle && (error.is_some() || matches!(state, State::NotBuilt | State::Outdated));
        let mut right = div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(12.))
            .children(badge.map(|(tone, label)| {
                ui::text(step::NOTE, FontWeight::MEDIUM, ink::BODY)
                    .child(ui::state_line(tone, label))
            }));
        if let Some(newer) = update.filter(|_| idle) {
            right = right.child(
                button(
                    "native-update",
                    trf(
                        "Rebuild on Eclipse {version}",
                        &[("version", &native_status::eclipse_label(&newer))],
                    ),
                    ButtonKind::Secondary,
                )
                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.native_update = Some(newer.clone());
                    this.native_on_base = true;
                    this.native_confirm = Some(this.native_mods_to_build().map(|(_, b)| b));
                    cx.notify();
                })),
            );
        }
        if can_build {
            right = right.child(
                button("native-build", "Build", ButtonKind::Primary).on_press(cx.listener(
                    |this, _: &ClickEvent, _, cx| {
                        this.native_update = None;
                        this.native_on_base = false;
                        this.native_confirm = Some(this.native_mods_to_build().map(|(_, b)| b));
                        cx.notify();
                    },
                )),
            );
        }
        let profile = native_mods::Profile::chosen(&self.settings);
        let picker = ui::segments(
            "native-profile",
            &[
                (native_mods::Profile::Performance, "Performance build"),
                (native_mods::Profile::Dev, "Dev build"),
                (native_mods::Profile::Release, "Release build"),
            ],
            profile,
            {
                let this = cx.entity().downgrade();
                move |choice, _, cx| {
                    let _ = this.update(cx, |this, cx| {
                        this.settings.native_profile = Some(choice);
                        this.settings.native_release = false;
                        this.native_done = None;
                        this.save_settings();
                        cx.notify();
                    });
                }
            },
        );
        let said = match profile {
            native_mods::Profile::Performance => tr("The everyday build for playing with mods"),
            native_mods::Profile::Dev => tr("For mod authors: faster rebuilds, slower game"),
            native_mods::Profile::Release => tr("Slower to build, slightly faster game"),
        };
        detail = detail.child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(8.))
                .mt(px(8.))
                .child(picker)
                .child(said),
        );
        if let Some(list) = mismatch {
            detail = detail.child(self.native_mismatch_panel(list, cx));
        }
        if let Some(mods) = &self.native_confirm {
            detail = detail.child(self.native_warning(mods, cx));
        }
        Some(
            div()
                .id("native-build")
                .flex()
                .items_start()
                .gap(px(16.))
                .p(px(ui::PANEL_INSET))
                .rounded(px(radius::ROW))
                .bg(white(fill::WELL))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .gap(px(4.))
                        .child(
                            ui::text(step::BODY, FontWeight::SEMIBOLD, ink::FULL)
                                .child(trf("Native build of {instance}", &name)),
                        )
                        .child(detail),
                )
                .child(right),
        )
    }
}

/// An instance whose registry projects may update: the mod API level of the game it plays, and
/// whether its native mods may (`eclipse::auto::Context`).
pub(super) struct AutoTarget {
    pub(super) instance: instances::Instance,
    pub(super) level: Option<u32>,
    pub(super) natives: bool,
}

/// Whether `instance` keeps a Nightly, or a native build made on one: its native mods stay as
/// they are, built for that Eclipse.
pub(super) fn pinned_to_nightly(instance: &instances::Instance) -> bool {
    let nightly = |v: &str| v.starts_with("nightly-");
    instance
        .version
        .as_deref()
        .is_some_and(|v| nightly(v) || native_mods::about(v).is_some_and(|a| nightly(&a.base)))
}

/// Updates the registry projects of `targets`: Eclipse's own instances only, never a modpack's
/// profile, whose pack pins its mods. What happened, for the page's notice, and the instances
/// whose native mods were updated.
pub(super) fn auto_update(targets: &[AutoTarget]) -> (Vec<String>, Vec<String>) {
    let (mut lines, mut rebuild) = (Vec::new(), Vec::new());
    for t in targets {
        let i = &t.instance;
        let pack = i.id.starts_with("pack:") || i.id.starts_with("port:");
        let (false, None, Some(level)) = (pack, &i.java, t.level) else {
            continue;
        };
        let ctx = eclipse::auto::Context {
            in_pack: eclipse::auto::in_pack(&i.home),
            level,
            natives: t.natives,
        };
        if let Some(report) = eclipse::auto::run(&i.home, &ctx, &|_, _| {}) {
            lines.extend(report.lines());
            if report.rebuild {
                rebuild.push(i.id.clone());
            }
        }
    }
    (lines, rebuild)
}

/// Where a kind's tab searches first: Eclipse's registry for shader packs, Modrinth for the rest.
fn source_for(kind: PackKind) -> SourceId {
    match kind {
        PackKind::Shader => SourceId::Eclipse,
        PackKind::Mod | PackKind::ResourcePack | PackKind::Modpack | PackKind::DataPack => {
            SourceId::Modrinth
        }
    }
}

/// Whether a kind's tab picks its source: mods, modpacks, resource packs and data packs are on
/// both Eclipse's registry and Modrinth; Eclipse's shader packs are on its registry only.
pub(super) fn switches(kind: PackKind) -> bool {
    kind != PackKind::Shader
}

impl Launcher {}

impl Launcher {
    /// The warning before a native build: what the mods can do and which mods. The build starts
    /// only from its button.
    pub(super) fn native_warning(
        &self,
        mods: &Result<Vec<native_mods::Bundle>, String>,
        cx: &mut Context<Self>,
    ) -> Div {
        let list = match mods {
            Ok(found) => found
                .iter()
                .map(|b| format!("{} {}", b.id, b.version))
                .collect::<Vec<_>>()
                .join(", "),
            Err(e) => e.clone(),
        };
        let panel = native_panel()
            .child(ui::error_note(tr("These mods will run with full access to your computer")))
            .child(tr("They can read, change or delete your files and use your accounts. Build them only if you trust their authors.").to_string())
            .child(trf("Mods: {mods}", &[("mods", &list)]))
            .child(tr("The first build takes 20 to 30 minutes and a few GB of disk. Later ones take a few minutes.").to_string())
            .children(self.native_update.as_ref().map(|newer| {
                div().text_color(ui::warn()).child(trf(
                    "This moves the instance to Eclipse {version}. It compiles the game from scratch, so it takes longer than a normal rebuild: up to 30 minutes.",
                    &[("version", &native_status::eclipse_label(newer))],
                ))
            }));
        let mut buttons = div().flex().gap(px(8.)).mt(px(4.));
        if mods.is_ok() {
            buttons = buttons.child(
                button(
                    "native-trust",
                    "I trust these mods: build",
                    ButtonKind::Primary,
                )
                .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.build_native(false);
                    cx.notify();
                })),
            );
        }
        panel.child(buttons.child(
            button("native-cancel", "Cancel", ButtonKind::Secondary).on_press(cx.listener(
                |this, _: &ClickEvent, _, cx| {
                    this.native_confirm = None;
                    this.native_update = None;
                    this.native_on_base = false;
                    cx.notify();
                },
            )),
        ))
    }

    /// Mods made for other Eclipse versions: which, and the version the build uses. They often
    /// still work, so the player may build anyway.
    pub(super) fn native_mismatch_panel(&self, list: &str, cx: &mut Context<Self>) -> Div {
        native_panel()
            .child(ui::error_note(tr("These mods were made for different Eclipse versions")))
            .child(list.to_owned())
            .child(tr("They often still work together. If the build fails, install versions made for the same Nightly.").to_string())
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .mt(px(4.))
                    .child(
                        button("native-anyway", "Build anyway", ButtonKind::Primary).on_press(
                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.build_native(true);
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        button("native-mismatch-cancel", "Cancel", ButtonKind::Secondary).on_press(
                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.native_mismatch = None;
                                cx.notify();
                            }),
                        ),
                    ),
            )
    }
}

pub(super) fn native_panel() -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .mt(px(12.))
        .pt(px(12.))
        .border_t_1()
        .border_color(ui::white(eclipse_theme::glass::EDGE))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_read_short() {
        assert_eq!(count(999), "999");
        assert_eq!(count(1_500), "1.5k");
        assert_eq!(count(51_404_735), "51.4M");
    }
}
