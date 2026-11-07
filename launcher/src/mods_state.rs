//! The Mods page: its tabs are the kinds, searched on Eclipse's own registry or Modrinth (a Java
//! instance's files always from Modrinth, for its version), with `eclipse_launcher::browser`.
use crate::menus::Asking;
use crate::page::Page;
use crate::ui;
use crate::{cull, layout, menus, mods_page, preview, Launcher};
use eclipse_launcher::backend;
use eclipse_launcher::browser::{
    installed, java, java_instance, Kind as PackKind, Project, SourceId, Support,
};
use eclipse_launcher::i18n::{tr, trf};
use eclipse_port_pack::check::Check;
use gpui::prelude::*;
use gpui::{App, ClickEvent, SharedString, Window};
use java_instance::Target;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};

/// The kinds, as the Mods page's tabs.
pub(super) const KINDS: [PackKind; 5] = [
    PackKind::Mod,
    PackKind::Modpack,
    PackKind::ResourcePack,
    PackKind::Shader,
    PackKind::DataPack,
];

pub(super) struct KindWords {
    pub(super) tab: &'static str,
    pub(super) search: &'static str,
    pub(super) load_failed: &'static str,
    pub(super) none_yet: &'static str,
    pub(super) open_page: &'static str,
}

pub(super) fn kind_words(kind: PackKind) -> KindWords {
    match kind {
        PackKind::Mod => KindWords {
            tab: "Mods",
            search: "Search mods",
            load_failed: "Couldn't load Eclipse mods",
            none_yet: "No Eclipse mods yet",
            open_page: "Open mod page",
        },
        PackKind::Modpack => KindWords {
            tab: "Modpacks",
            search: "Search modpacks",
            load_failed: "Couldn't load Eclipse modpacks",
            none_yet: "No Eclipse modpacks yet",
            open_page: "Open modpack page",
        },
        PackKind::ResourcePack => KindWords {
            tab: "Resource packs",
            search: "Search resource packs",
            load_failed: "Couldn't load Eclipse resource packs",
            none_yet: "No Eclipse resource packs yet",
            open_page: "Open resource pack page",
        },
        PackKind::Shader => KindWords {
            tab: "Shader packs",
            search: "Search shader packs",
            load_failed: "Couldn't load Eclipse shader packs",
            none_yet: "No Eclipse shader packs yet",
            open_page: "Open shader pack page",
        },
        PackKind::DataPack => KindWords {
            tab: "Data packs",
            search: "Search data packs",
            load_failed: "Couldn't load Eclipse data packs",
            none_yet: "No Eclipse data packs yet",
            open_page: "Open data pack page",
        },
    }
}
/// Java mods checked at once.
pub(super) const CHECKING: usize = 2;
/// The sources to pick from on the tabs that have both (`Kind::picks_source`).
pub(super) const SOURCES: [(SourceId, &str); 2] = [
    (SourceId::Eclipse, "Eclipse"),
    (SourceId::Modrinth, "Modrinth"),
];
pub(super) const PAGE: usize = 20;

pub(super) const DETAIL_FLOAT: &str = "mods-detail-float";

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
    pub(super) everything: bool,
    pub(super) show_unrunnable: bool,
    /// Which search is the current one; answers for older ones are dropped.
    pub(super) search: u64,
    pub(super) searching: bool,
    pub(super) searched: bool,
    pub(super) results: Vec<Project>,
    pub(super) results_list: cull::Cull,
    pub(super) installed_list: cull::Cull,
    pub(super) installed_open: bool,
    pub(super) more: bool,
    pub(super) error: Option<String>,
    pub(super) installed: Vec<installed::Entry>,
    pub(super) installed_read: u64,
    loaded: bool,
    /// Whether the page searched for the instance as it is (its version and loader).
    pub(super) asked: bool,
    /// The game folder the browser installs into and lists: the selected instance's.
    home: Option<PathBuf>,
    /// The selected Java instance's version and loader: its mods and resource packs are the
    /// source's own files for them, put in as they are.
    pub(super) java: Option<Target>,
    loads_shaders: bool,
    pub(super) palette_failed: Option<u64>,
    /// The world data packs go into, as picked (`datapack_world`).
    pub(super) world: Option<PathBuf>,
    pub(super) busy: Option<ModsBusy>,
    pub(super) failed: Option<String>,
    pub(super) updates: HashMap<(SourceId, String), String>,
    /// The project the detail card shows; the first result when none is.
    pub(super) picked: Option<String>,
    /// The project the detail card showed last.
    pub(super) shown: Option<Project>,
    pub(super) float_at: std::rc::Rc<std::cell::Cell<(f32, f32)>>,
    pub(super) float_scroll: gpui::ScrollHandle,
    pub(super) float_for: Option<String>,
    /// A native mod's Install waiting for the one-time trust notice (`native_status::NOTICE`).
    pub(super) native_notice: Option<(SourceId, String, Option<Project>)>,
    pub(super) native_files:
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
    }

    /// The game folder installs go into: the selected instance's, once the page has read it.
    pub(super) fn home(&self) -> PathBuf {
        self.home.clone().unwrap_or_else(backend::game_home)
    }

    pub(super) fn read_installed(&mut self) {
        let home = self.home();
        self.installed = installed::Installed::load(&home).entries;
        self.installed_read += 1;
        self.installed_list.reset();
        self.loads_shaders = java_instance::loads_shaders(&home);
    }

    /// Whether the Modpacks tab is open (the sidebar marks the pack shown there).
    pub(super) fn on_modpacks(&self) -> bool {
        self.kind == PackKind::Modpack
    }

    pub(super) fn new() -> Self {
        let (tx, rx) = channel();
        let kind = match preview::preview() {
            Some(preview::Preview::Mods(kind)) => kind,
            _ => PackKind::Mod,
        };
        Self {
            kind,
            source: kind.first_source(false),
            everything: false,
            show_unrunnable: false,
            search: 0,
            searching: false,
            searched: false,
            results: Vec::new(),
            results_list: cull::Cull::default(),
            installed_list: cull::Cull::default(),
            installed_open: false,
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
            world: None,
            busy: None,
            failed: None,
            native_notice: None,
            native_files: Default::default(),
            updates: HashMap::new(),
            picked: None,
            shown: None,
            float_at: Default::default(),
            float_scroll: Default::default(),
            float_for: None,
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

    pub(super) fn may_hide_unrunnable(&self) -> bool {
        self.java.is_none() && self.kind == PackKind::Mod && self.source == SourceId::Modrinth
    }

    pub(super) fn listed(&self, p: &Project) -> bool {
        self.show_unrunnable
            || !self.may_hide_unrunnable()
            || !self.checking(p) && self.support(p).installable()
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

pub(super) fn titles(done: &[installed::Entry]) -> String {
    let names: Vec<&str> = done.iter().map(|e| e.title.as_str()).collect();
    names.join(", ")
}

pub(super) fn folder_busy() -> String {
    tr("Wait for the current operation to finish.").into()
}

/// `51404735` → `51.4M`.
pub(super) fn count(n: u64) -> String {
    match n {
        0..1_000 => n.to_string(),
        1_000..1_000_000 => format!("{:.1}k", n as f64 / 1e3),
        _ => format!("{:.1}M", n as f64 / 1e6),
    }
}

pub(super) fn licence_name(id: &str) -> String {
    match id.strip_prefix("LicenseRef-") {
        Some("" | "Custom") => tr("Custom licence").to_string(),
        Some(words) => words.replace('-', " "),
        None => id.to_string(),
    }
}

impl Launcher {
    pub(super) fn mod_detail_floats(&self) -> bool {
        self.mods.picked.is_some() && self.window_width < layout::MODS_DETAIL_BESIDE
    }

    pub(super) fn close_mod_detail(&mut self, window: &mut Window, cx: &mut App) {
        self.mods.picked = None;
        self.asking = None;
        let inside =
            ui::try_focus_for(DETAIL_FLOAT).is_some_and(|float| float.contains_focused(window, cx));
        if let Some(results) = ui::options_focus(mods_page::RESULTS).filter(|_| inside) {
            window.focus(&results, cx);
        }
    }

    pub(super) fn mods_escape(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.mods.browse.show_filters {
            return false;
        }
        self.mods.browse.show_filters = false;
        let toggle = menus::opener_focus(crate::mods_project::FILTERS_TOGGLE);
        Self::focus_if_removed(window.focused(cx), toggle, window, cx);
        true
    }

    pub(super) fn mods_scrolled(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.on_next_frame(window, |this, _, cx| {
            if this.mod_detail_floats() || this.mods_sticky.moved(&this.content_scroll.handle) {
                cx.notify();
            }
        });
    }

    /// Opens the Mods page on its Modpacks tab, with the pack `slug` open when given (the
    /// sidebar's packs and its + button).
    pub(super) fn show_modpack(&mut self, slug: Option<String>) {
        self.show_mods(PackKind::Modpack);
        if slug.is_some() {
            self.modpacks.open = slug;
            self.modpacks.report_list.reset();
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
                    m.failed = result.err();
                    m.read_installed();
                    if pack.is_some() {
                        // The Modpacks page reads its list again.
                        self.modpacks.loaded = false;
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
                    self.native.rebuild.extend(rebuild);
                    if !lines.is_empty() {
                        crate::toast::say(lines.join(" "));
                        m.read_installed();
                    }
                }
                Event::TakenDown(lines) => {
                    crate::toast::say_error(lines.join(" "));
                    m.read_installed();
                }
            }
        }
        if self.mods.browse.open.is_none() {
            let wanted = preview::project().and_then(|want| {
                (self.mods.results.iter()).find(|p| p.id == want || p.slug == want)
            });
            if let Some(p) = wanted.cloned() {
                preview::project_opened();
                self.mods_open(p);
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

    pub(super) fn target_name(&self) -> String {
        self.selected_instance()
            .map_or_else(|| "Eclipse".to_owned(), |i| i.name.clone())
    }

    /// Reads what the browser installed in the selected instance (once per instance) and looks
    /// for updates: the Mods page and Home's Mods tab (`home_mods_state.rs`) show it. Packs and mods
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
        if (to_eclipse || to_java)
            && self
                .mods
                .use_source(self.mods.kind.first_source(to_eclipse))
        {
            self.mods.asked = false;
        }
        self.mods.java = java;
        if !self.mods.loaded || (home.is_some() && self.mods.home != home) {
            self.mods.home = home;
            self.mods.read_installed();
            self.mods.updates.clear();
            self.stop_asking(|a| matches!(a, Asking::Mod(_)));
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
            self.mods_set_source(kind.first_source(self.mods.java.is_none()));
        }
    }

    /// Opens a kind's tab: its first source, its own filters, a new search.
    pub(super) fn mods_set_kind(&mut self, kind: PackKind) {
        if self.mods.kind != kind {
            self.mods.kind = kind;
            self.mods.source = kind.first_source(self.mods.java.is_none());
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
                    ui::Tone::Off,
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
            Support::Works => (ui::Tone::Off, in_eclipse, None),
            Support::Partial(_) => (ui::Tone::Warn, in_eclipse, reason),
            Support::No(_) => (ui::Tone::Off, in_eclipse, reason),
        })
    }

    pub(super) fn status_shown(
        &self,
        p: &Project,
        instance: &str,
    ) -> Option<(ui::Tone, String, Option<String>)> {
        let advised = self.mods_vanilla_target().is_some();
        self.mods_status(p, instance)
            .map(|(tone, said, reason)| match tone {
                ui::Tone::Warn if advised => (ui::Tone::Off, said, reason),
                tone => (tone, said, reason),
            })
    }

    pub(super) fn install_refusal(&self, p: &Project) -> Option<SharedString> {
        self.mods_wait().or_else(|| {
            self.needs_world(p)
                .then(|| tr("Create a world in the game first").into())
        })
    }

    pub(super) fn install_label(p: &Project, instance: &str) -> String {
        match p.kind {
            PackKind::Modpack => tr("Install as an instance").into(),
            _ => trf("Install in {instance}", &[("instance", instance)]),
        }
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

    pub(super) fn needs_world(&self, p: &Project) -> bool {
        p.kind == PackKind::DataPack && self.datapack_world().is_none()
    }

    pub(super) fn mods_said(&mut self, said: Result<String, String>) {
        if let Ok(line) = &said {
            crate::toast::say(line.clone());
        }
        self.mods.failed = said.err();
    }
}

pub(super) fn goes_to_trash(kind: PackKind) -> bool {
    kind != PackKind::Mod
}

pub(super) fn removal(kind: PackKind, name: &str) -> ui::Confirm<'_> {
    match goes_to_trash(kind) {
        true => ui::Confirm::Delete(name),
        false => ui::Confirm::Remove(name),
    }
}

pub(super) fn find_on_mods_page(
    kind: PackKind,
    cx: &Context<Launcher>,
) -> ui::Listener<ClickEvent> {
    Box::new(cx.listener(move |this, _: &ClickEvent, _, cx| {
        this.show_mods(kind);
        cx.notify();
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn licences_read_as_words() {
        assert_eq!(licence_name("MIT"), "MIT");
        assert_eq!(
            licence_name("LicenseRef-Polyform-Shield-1.0.0"),
            "Polyform Shield 1.0.0"
        );
        assert_eq!(licence_name("LicenseRef-Custom"), "Custom licence");
    }

    #[test]
    fn counts_read_short() {
        assert_eq!(count(999), "999");
        assert_eq!(count(1_500), "1.5k");
        assert_eq!(count(51_404_735), "51.4M");
    }
}
