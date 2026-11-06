//! The Mods page: its tabs are the kinds (mods, modpacks, resource packs, shader packs, data
//! packs). Search Eclipse's own registry for what was made for Eclipse, or Modrinth for Java
//! mods, modpacks, resource packs and data packs (a Java instance's files
//! always come from Modrinth, for its version), see where each comes
//! from and how well it runs (a Java mod's jar is checked first; a Eclipse mod says whether it is
//! signed and what it may do), install it with its checksum checked, turn Eclipse mods on and
//! off, and update or remove what was installed (`eclipse_launcher::browser`). Imported modpacks
//! are listed on its Modpacks tab.
use super::*;
use crate::menus::Asking;
use crate::ui::{self, white};
use eclipse_launcher::browser::{
    self, eclipse, installed, java, java_instance, Kind as PackKind, Project, Query, SourceId,
    Support,
};
use eclipse_launcher::modpacks;
use eclipse_launcher::native_status;
use eclipse_port_pack::check::Check;
use eclipse_theme::glass::{ink, space, step};
use java_instance::Target;
use std::collections::HashMap;
use std::path::PathBuf;

/// The kinds, as the Mods page's tabs.
const KINDS: [(PackKind, &str); 5] = [
    (PackKind::Mod, "Mods"),
    (PackKind::Modpack, "Modpacks"),
    (PackKind::ResourcePack, "Resource packs"),
    (PackKind::Shader, "Shader packs"),
    (PackKind::DataPack, "Data packs"),
];
/// Java mods checked at once.
const CHECKING: usize = 2;
/// The sources to pick from on the tabs that have both ([`switches`]).
pub(super) const SOURCES: [(SourceId, &str); 2] = [
    (SourceId::Eclipse, "Eclipse"),
    (SourceId::Modrinth, "Modrinth"),
];
const PAGE: usize = 20;

#[path = "mods_view.rs"]
mod mods_view;

pub(super) const DETAIL_FLOAT: &str = "mods-detail-float";

#[path = "native_build_view.rs"]
mod native_build_view;

pub(super) enum Event {
    /// A search's answer, for the search it was (stale answers are dropped) at that offset.
    Found(u64, usize, Result<Vec<Project>, String>),
    Progress(String, f32),
    Done(Result<String, String>, Option<PathBuf>),
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
    /// The palette's mods for its search of that number, or why the search failed.
    Palette(u64, Result<Vec<Project>, String>),
}

/// Where a Java mod's check is.
pub(super) enum Checking {
    Running,
    Done(Box<Check>),
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum ModsBusy {
    Install {
        id: String,
        name: String,
        line: String,
        bar: (f32, f32),
    },
    UpdateAll {
        line: String,
        bar: (f32, f32),
    },
}

impl ModsBusy {
    pub(super) fn installs(&self, project: &str) -> bool {
        matches!(self, ModsBusy::Install { id, .. } if id == project)
    }

    fn moved(&mut self, now: String, to: f32) {
        let (ModsBusy::Install { line, bar, .. } | ModsBusy::UpdateAll { line, bar }) = self;
        *line = now;
        *bar = (bar.1, to);
    }
}

pub(super) struct Mods {
    pub(super) kind: PackKind,
    pub(super) source: SourceId,
    everything: bool,
    show_unrunnable: bool,
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
    pub(super) installed_read: u64,
    loaded: bool,
    /// Whether the page searched for the instance as it is (its version and loader).
    asked: bool,
    /// The game folder the browser installs into and lists: the selected instance's.
    home: Option<PathBuf>,
    /// The selected Java instance's version and loader: its mods and resource packs are the
    /// source's own files for them, put in as they are.
    pub(super) java: Option<Target>,
    loads_shaders: bool,
    palette_failed: Option<u64>,
    /// Its worlds (folder, name), the last played first, and the one data packs go into.
    worlds: Vec<(PathBuf, String)>,
    world: Option<PathBuf>,
    pub(super) busy: Option<ModsBusy>,
    pub(super) message: Option<Result<String, String>>,
    pub(super) updates: HashMap<(SourceId, String), String>,
    /// Instances (ids) whose native mods the automatic update moved on: Play builds their game
    /// first (`build_native`), then plays it when `play_after_build` is set.
    pub(super) rebuild: Vec<String>,
    pub(super) play_after_build: bool,
    /// The project the detail card shows; the first result when none is.
    pub(super) picked: Option<String>,
    /// The project the detail card showed last.
    pub(super) shown: Option<Project>,
    float_top: std::rc::Rc<std::cell::Cell<f32>>,
    float_scroll: gpui::ScrollHandle,
    /// A native mod's Install waiting for the one-time trust notice (`native_status::NOTICE`).
    native_notice: Option<(SourceId, String, Option<Project>)>,
    /// The selected instance's native build state, and the `native_status::fingerprint` it was
    /// read for: read again only when that changes.
    /// With it, the mods turned on that pin the commit an older build compiled
    /// (`native_status::pinning`, for builds that didn't record it).
    native_state: std::cell::RefCell<Option<(String, native_status::State, Vec<String>)>>,
    native_files:
        std::cell::RefCell<Option<(String, Vec<eclipse_launcher::github_mods::Installed>)>>,
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

    fn read_installed(&mut self) {
        let home = self.home();
        self.installed = installed::Installed::load(&home).entries;
        self.installed_read += 1;
        self.installed_list.reset();
        self.loads_shaders = java_instance::loads_shaders(&home);
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
            source: source_for(kind, false),
            everything: false,
            show_unrunnable: false,
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
            installed_read: 0,
            loaded: false,
            asked: false,
            home: None,
            java: None,
            loads_shaders: false,
            palette_failed: None,
            worlds: Vec::new(),
            world: None,
            busy: None,
            message: None,
            native_notice: None,
            native_state: Default::default(),
            native_files: Default::default(),
            updates: HashMap::new(),
            rebuild: Vec::new(),
            play_after_build: false,
            picked: None,
            shown: None,
            float_top: Default::default(),
            float_scroll: Default::default(),
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

    pub(super) fn needs_world(&self, p: &Project) -> bool {
        p.kind == PackKind::DataPack && self.world.is_none()
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

    fn use_source(&mut self, source: SourceId) -> bool {
        if self.source == source {
            return false;
        }
        self.source = source;
        self.browse.filters = Default::default();
        self.browse.open = None;
        self.picked = None;
        true
    }

    fn may_hide_unrunnable(&self) -> bool {
        self.java.is_none() && self.kind == PackKind::Mod && self.source == SourceId::Modrinth
    }

    fn listed(&self, p: &Project) -> bool {
        self.show_unrunnable || !self.may_hide_unrunnable() || self.support(p).installable()
    }

    fn runnable_first(&mut self) {
        if self.results.iter().any(|p| self.checking(p)) {
            return;
        }
        let mut results = std::mem::take(&mut self.results);
        results.sort_by_key(|p| !self.support(p).installable());
        self.results = results;
    }

    /// How well a result runs: a Java mod's from its check once it has one. In a Java
    /// instance, whether it has the loader and version the result was made for.
    pub(super) fn support(&self, p: &Project) -> Support {
        if let Some(target) = self.java_for(p.kind) {
            return match target.support(p) {
                Support::Works if p.kind == PackKind::Shader && !self.loads_shaders => {
                    let needs = tr("Needs a shader mod like Iris").to_string();
                    match target.loader {
                        Some(_) => Support::Partial(needs),
                        None => Support::No(needs),
                    }
                }
                support => support,
            };
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

fn titles(done: &[installed::Entry]) -> String {
    let names: Vec<&str> = done.iter().map(|e| e.title.as_str()).collect();
    names.join(", ")
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
    trf("Installed {names}.", &[("names", &titles(done))])
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
        .gap(px(space::XXS));
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
            .child(div().ml(px(space::XS)).child(format!(
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
        self.show_mods(PackKind::Modpack);
        if slug.is_some() {
            self.modpacks.open = slug;
            self.mods.report_list.reset();
        }
    }

    /// Called with the launcher's other work: what the background sent.
    pub(super) fn mods_tick(&mut self) -> bool {
        let mut changed = false;
        let mut to_check = Vec::new();
        let mut loaded = Vec::new();
        let mut done = false;
        let mut new_pack = None;
        while let Ok(event) = self.mods.rx.try_recv() {
            changed = true;
            let m = &mut self.mods;
            match event {
                Event::Palette(search, found) => {
                    if let Some(p) = self.palette.as_mut().filter(|p| p.search == search) {
                        m.palette_failed = found.is_err().then_some(search);
                        p.mods = found.unwrap_or_default();
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
                        busy.moved(line, f);
                    }
                }
                Event::Done(result, pack) => {
                    done = true;
                    m.busy = None;
                    if let Ok(said) = &result {
                        crate::toast::say(said.clone());
                    }
                    m.message = Some(result);
                    m.read_installed();
                    if pack.is_some() {
                        // The Modpacks page reads its list again.
                        self.modpacks.loaded = None;
                        new_pack = pack;
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
                        m.read_installed();
                    }
                }
                Event::TakenDown(lines) => {
                    m.message = Some(Ok(lines.join("\n")));
                    m.read_installed();
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
        if changed {
            self.mods.runnable_first();
        }
        if done {
            // A data pack put into a world shows in its list on Home.
            self.world_packs.reread();
        }
        if let Some(work) = new_pack {
            self.load_packs();
            let made = self.instances.list.iter().find(|i| i.work == work).cloned();
            if let Some(instance) = made {
                self.select_instance(&instance);
            }
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
            self.mods.message =
                Some(java_instance::set_on(&home, e, on).map(|()| switched_line(&e.title, on)));
            self.mods.read_installed();
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
            Some(reason) => Err(trf(
                "{name} was taken down: {reason}",
                &[("name", &e.title), ("reason", &reason)],
            )),
            None => java::set_enabled(&home, id, on).map(|()| switched_line(&e.title, on)),
        });
        self.mods.read_installed();
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
        let game = installed::Game::of(self.mods.java.clone());
        std::thread::spawn(move || {
            let mut found = HashMap::new();
            for e in entries {
                let Some(source) = browser::source(e.source) else {
                    continue;
                };
                let newer = game.update_for(&*source, &e);
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
        self.mods.busy = Some(ModsBusy::UpdateAll {
            line: tr("Updating…").into(),
            bar: (0., 0.),
        });
        self.mods.updates.clear();
        let tx = self.mods.tx.clone();
        let home = self.mods.home();
        let game = installed::Game::of(self.mods.java.clone());
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
                        game.install(&*source, &p, true, &home, &step)
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
            let updated = match done.is_empty() {
                true => String::new(),
                false => trf("Updated {names}.", &[("names", &done.join(", "))]),
            };
            let result = match failed.is_empty() {
                true => Ok(updated),
                false => Err(trf(
                    "{updated} Not updated: {failed}",
                    &[("updated", &updated), ("failed", &failed.join("; "))],
                )
                .trim()
                .to_string()),
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
        self.mods.message = None;
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
        // A modpack is ported for the selected version as soon as it is imported.
        let resources = self
            .selected_version()
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
            let mut pack = None;
            let result = browser::source(source)
                .ok_or_else(|| format!("{} isn't set up.", source.label()))
                .and_then(|source| {
                    let p = match known {
                        Some(p) => p,
                        None => source.project(&id)?,
                    };
                    if p.kind == PackKind::DataPack {
                        let (world, name) =
                            world.ok_or("Pick the world the data pack goes into first.")?;
                        let into = |f| progress(&p.title, f);
                        if let Some(target) = &java {
                            java_instance::install_datapack(&*source, target, &p, &world, &into)?;
                        } else {
                            installed::install_datapack(&*source, &p, everything, &world, &into)?;
                        }
                        return Ok(trf(
                            "Added {title} to {world}.",
                            &[("title", &p.title), ("world", &name)],
                        ));
                    }
                    if let Some(target) = java.as_ref().filter(|t| t.takes(p.kind)) {
                        let done = java_instance::install(&*source, target, &p, &home, &progress)?;
                        return Ok(installed_line(&done));
                    }
                    if p.kind == PackKind::Modpack && p.source == SourceId::Eclipse {
                        let installed = eclipse::pack::install(&p.id, &progress)?;
                        if let Some(url) = &p.icon {
                            eclipse_launcher::instances::adopt_icon(
                                &eclipse::pack::home(&installed.work),
                                url,
                            );
                        }
                        pack = Some(installed.work);
                        return Ok(trf(
                            "Installed {pack} as a new instance.",
                            &[("pack", &installed.name)],
                        ));
                    }
                    if p.kind == PackKind::Modpack {
                        let folder = modpacks::ports_dir().join("downloads");
                        let file = installed::fetch_modpack(&*source, &p, &folder, &|f| {
                            progress(&p.title, f)
                        })?;
                        let log = |line: String| progress(&line, 1.);
                        let imported =
                            modpacks::import_and_port(&file, resources.as_deref(), &log)?;
                        if let Some(url) = &p.icon {
                            eclipse_launcher::instances::adopt_icon(
                                &modpacks::profile_home(&imported.work),
                                url,
                            );
                        }
                        pack = Some(imported.work.clone());
                        return Ok(trf(
                            "Installed {pack} as a new instance.",
                            &[("pack", imported.title())],
                        ));
                    }
                    let done = installed::install(&*source, &p, everything, &home, &progress)?;
                    if done.iter().any(|e| native_status::is_native_file(&e.file)) {
                        return Ok(trf(
                            "Installed {names} in {instance}. Build to play with it.",
                            &[("names", &titles(&done)), ("instance", &instance)],
                        ));
                    }
                    if p.kind == PackKind::ResourcePack {
                        return Ok(trf(
                            "Installed {names}. Turn it on in the game: Options > Resource Packs.",
                            &[("names", &titles(&done))],
                        ));
                    }
                    Ok(installed_line(&done))
                });
            let _ = tx.send(Event::Done(result, pack));
        });
    }

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
        let to_eclipse = java.is_none() && (!self.mods.loaded || self.mods.java.is_some());
        let to_java = java.is_some() && self.mods.java.is_none();
        if (to_eclipse || to_java) && self.mods.use_source(source_for(self.mods.kind, to_eclipse)) {
            self.mods.asked = false;
        }
        self.mods.java = java;
        if !self.mods.loaded || (home.is_some() && self.mods.home != home) {
            self.mods.home = home;
            self.mods.read_installed();
            self.mods.updates.clear();
            self.stop_asking(|a| matches!(a, Asking::Mod(_)));
            self.mods.read_worlds();
            self.mods_check_updates();
            self.mods_taken_down();
            self.mods.loaded = true;
        }
    }

    pub(super) fn show_mods(&mut self, kind: PackKind) {
        self.go(Page::Mods);
        self.mods_load();
        self.mods.browse.open = None;
        if self.mods.kind != kind {
            self.mods_set_kind(kind);
        } else {
            self.mods_set_source(source_for(kind, self.mods.java.is_none()));
        }
    }

    /// Opens a kind's tab: its first source, its own filters, a new search.
    pub(super) fn mods_set_kind(&mut self, kind: PackKind) {
        if self.mods.kind != kind {
            self.mods.kind = kind;
            self.mods.source = source_for(kind, self.mods.java.is_none());
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

    pub(super) fn mods_set_source(&mut self, source: SourceId) {
        if self.mods.use_source(source) {
            self.mods_search(0);
        }
    }

    /// The project `id` leaves the selected instance now; what happened.
    pub(super) fn mods_remove(&mut self, id: &str) -> Result<String, String> {
        let home = self.mods.home();
        let Some(_lock) = browser::try_lock_home(&home) else {
            let busy: Result<String, String> =
                Err(tr("Wait for the current operation to finish.").into());
            self.mods.message = Some(busy.clone());
            return busy;
        };
        let trashed = self
            .mods
            .installed
            .iter()
            .find(|e| e.project == id && goes_to_trash(e.kind))
            .and_then(|e| e.path(&home))
            .filter(|path| path.exists())
            .map_or(Ok(()), |path| {
                eclipse_launcher::desktop_seams::to_trash(&path)
                    .map_err(|e| format!("{}: {e}", path.display()))
            });
        let removed = trashed
            .and_then(|()| installed::Game::of(self.mods.java.clone()).remove(&home, id))
            .map(|e| match goes_to_trash(e.kind) {
                true => trf("{name} moved to the trash.", &[("name", &e.title)]),
                false => trf("Removed {name}.", &[("name", &e.title)]),
            });
        if let Ok(said) = &removed {
            crate::toast::say(said.clone());
        }
        self.mods.message = removed.clone().err().map(Err);
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

    pub(super) fn palette_search_failed(&self, search: u64) -> bool {
        self.mods.palette_failed == Some(search)
    }

    pub(super) fn mods_status(
        &self,
        p: &Project,
        instance: &str,
    ) -> Option<(ui::Tone, String, Option<String>)> {
        let support = self.mods.support(p);
        let reason = support.reason().map(|r| tr(r).to_owned());
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
        self.pick_kind(instance_state::NewKind::Modded, cx);
        self.change(|j| {
            j.loader = eclipse_launcher::loaders::Kind::Fabric;
            j.version = Some(minecraft);
        });
    }

    pub(super) fn mods_vanilla_target(&self) -> Option<String> {
        self.mods
            .java_for(self.mods.kind)
            .filter(|t| {
                t.loader.is_none() && matches!(self.mods.kind, PackKind::Mod | PackKind::Shader)
            })
            .map(|t| t.minecraft.clone())
    }

    pub(super) fn mods_missing_shader_mod(&self) -> Option<(&'static str, &'static str)> {
        let target = self.mods.java_for(PackKind::Shader)?;
        (self.mods.kind == PackKind::Shader && !self.mods.loads_shaders)
            .then(|| target.shader_mod())
            .flatten()
    }

    pub(super) fn mods_wait(&self) -> Option<SharedString> {
        Some(match self.mods.busy.as_ref()? {
            ModsBusy::Install { name, .. } => {
                trf("Wait for {name} to finish installing", &[("name", name)]).into()
            }
            ModsBusy::UpdateAll { .. } => tr("Wait for the updates to finish").into(),
        })
    }

    /// Whether the selected instance has `p` (the palette's Install or Remove).
    pub(super) fn mods_has(&self, p: &Project) -> bool {
        self.mods.entry(p).is_some()
    }

    pub(super) fn mods_page(&mut self, cx: &mut Context<Self>) -> Div {
        self.mods_load();
        if !self.mods.asked {
            self.mods_search(0);
        }
        self.mods_view(cx)
    }

    fn mods_target_line(&self, cx: &mut Context<Self>) -> Option<Div> {
        (self.mods.kind == PackKind::DataPack && !self.mods.worlds.is_empty())
            .then(|| self.mods_world_picker(cx))
    }

    /// The world data packs go into: one of the instance's, picked here.
    fn mods_world_picker(&self, cx: &mut Context<Self>) -> Div {
        let note = |s: String| ui::card_note(s);
        let mut chips = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(space::SM))
            .child(note(tr("World").to_string()).mr(px(space::SM)));
        for (path, name) in &self.mods.worlds {
            let on = self.mods.world.as_ref() == Some(path);
            let folder = path.file_name().unwrap_or_default().to_string_lossy();
            let id = SharedString::from(format!("mods-world-{folder}"));
            let path = path.clone();
            chips = chips.child(ui::filter_chip(id, name.clone(), on).on_press(cx.listener(
                move |this, _: &ClickEvent, _, cx| {
                    this.mods.world = Some(path.clone());
                    cx.notify();
                },
            )));
        }
        chips
    }

    /// The one-time notice before the first native mod is installed: native mods aren't
    /// sandboxed, and what the build takes. Its button installs the mod.
    fn native_notice(&self, cx: &mut Context<Self>) -> Option<Div> {
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
            Some("download-04"),
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

    /// The install under way and the last install's answer.
    pub(super) fn mods_busy_plate(&self) -> Vec<Div> {
        let mut out = Vec::new();
        match &self.mods.busy {
            Some(ModsBusy::Install { line, bar, .. }) => out.push(ui::progress_line(
                tr("Installing"),
                Some(*bar),
                line.clone(),
            )),
            Some(ModsBusy::UpdateAll { line, bar }) => {
                out.push(ui::progress_line(tr("Updating"), Some(*bar), line.clone()))
            }
            None => {}
        }
        match &self.mods.message {
            Some(Ok(done)) => {
                out.push(ui::text(step::BODY, FontWeight::NORMAL, ink::FULL).child(done.clone()))
            }
            Some(Err(e)) => {
                let instance = self
                    .selected_instance()
                    .map_or_else(|| "Eclipse".to_string(), |i| i.name.clone());
                out.push(div().child(ui::error_line_for(
                    "mods-failed-step",
                    trf("Couldn't change {instance}", &[("instance", &instance)]),
                    e,
                    None,
                    Vec::new(),
                )))
            }
            None => {}
        }
        out
    }

    /// Re-read local/GitHub files when the folder changes, not on every frame.
    fn native_files(&self) -> Vec<eclipse_launcher::github_mods::Installed> {
        let home = self.mods.home();
        let key = native_status::fingerprint(&home, None);
        let mut memo = self.mods.native_files.borrow_mut();
        if memo.as_ref().is_none_or(|(k, _)| *k != key) {
            *memo = Some((key, eclipse_launcher::github_mods::untracked(&home)));
            self.mods.installed_list.reset();
        }
        memo.as_ref().unwrap().1.clone()
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
        use eclipse_launcher::instances::Kind;
        let pack = matches!(i.kind, Kind::Port { .. } | Kind::Pack { .. });
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

fn source_for(kind: PackKind, eclipse: bool) -> SourceId {
    match kind {
        PackKind::Mod | PackKind::Shader if eclipse => SourceId::Eclipse,
        PackKind::Mod
        | PackKind::Shader
        | PackKind::ResourcePack
        | PackKind::Modpack
        | PackKind::DataPack => SourceId::Modrinth,
    }
}

fn goes_to_trash(kind: PackKind) -> bool {
    kind != PackKind::Mod
}

fn removal(kind: PackKind, name: &str) -> ui::Confirm<'_> {
    match goes_to_trash(kind) {
        true => ui::Confirm::Delete(name),
        false => ui::Confirm::Remove(name),
    }
}

/// Whether a kind's tab picks its source: mods, modpacks, resource packs and data packs are on
/// both Eclipse's registry and Modrinth; Eclipse's shader packs are on its registry only.
pub(super) fn switches(kind: PackKind) -> bool {
    kind != PackKind::Shader
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
