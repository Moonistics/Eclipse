//! The Mods page's project page, search filters and project icons (`eclipse_launcher::browser`:
//! `details` for the data, `images` for the icons and gallery). Drawn only with the launcher's
//! shared components, so a restyle of those restyles this.
//!
//! - A result's title opens its page inside the launcher: icon, author, downloads, followers,
//!   licence, how well it runs and Install, then Description (Markdown), Gallery, Versions and
//!   the project's links.
//! - Filters: sort, loader, game version, environment and categories, with the values the
//!   source lists for the kind.
//! - Icons and gallery images come from the source's image servers at run time, cached on disk;
//!   a placeholder stands in until one arrives.
use super::mods_page::{Checking, Event};
use super::*;
use crate::ui::{self, white};
use eclipse_launcher::browser::{
    self,
    details::{self, Details, Environment, Filters, Image, Sort, Tags},
    images, java, Kind as PackKind, Project, SourceId, Version,
};
use eclipse_theme::glass::{ink, pixel_text, radius, step};
use gpui::img;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

/// Category and tag names from Modrinth / CurseForge, present in code so the i18n
/// tables can provide translations for them without failing the unused strings test.
const CATEGORY_NAMES: &[&str] = &[
    "adventure",
    "audio",
    "blocks",
    "combat",
    "core shaders",
    "cursed",
    "decoration",
    "economy",
    "entities",
    "environment",
    "equipment",
    "food",
    "fonts",
    "game mechanics",
    "gui",
    "items",
    "locale",
    "magic",
    "management",
    "minigame",
    "mobs",
    "modded",
    "models",
    "optimization",
    "realistic",
    "simplistic",
    "social",
    "storage",
    "technology",
    "themed",
    "transportation",
    "tweaks",
    "utility",
    "vanilla like",
    "worldgen",
];

/// A category's name in the launcher's language: the tables' translation of its words where they
/// have one (Modrinth's names), else the registry's own name for it.
fn category_name(slug: &str) -> String {
    let words = slug.replace('-', " ");
    let said = tr(&words);
    if said == words {
        return details::category_label(slug);
    }
    let mut chars = said.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// One of the registry's tags as a chip shows it.
fn tag_name(tag: &str) -> String {
    format!("#{tag}")
}

fn active_filters(f: &Filters) -> usize {
    f.categories.len()
        + f.tags.len()
        + f.loader.is_some() as usize
        + f.game_version.is_some() as usize
        + f.environment.is_some() as usize
}

/// A project page's tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tab {
    Description,
    Gallery,
    Versions,
}

const TABS: [(Tab, &str); 3] = [
    (Tab::Description, "Description"),
    (Tab::Gallery, "Gallery"),
    (Tab::Versions, "Versions"),
];

/// The open project page.
pub(super) struct Opened {
    pub(super) project: Project,
    tab: Tab,
    details: Option<Result<Arc<Details>, String>>,
    versions: Option<Result<Arc<[ProjectVersion]>, String>>,
    version_list: cull::Cull,
    checked_versions: HashMap<String, eclipse_port_pack::check::Verdict>,
    /// The gallery image shown large.
    shot: usize,
}

/// Cached verdicts are read with the version request, never while drawing a frame.
#[derive(Clone)]
pub(super) struct ProjectVersion {
    version: Version,
    verdict: Option<eclipse_port_pack::check::Verdict>,
}

/// The project page, filters and images' share of the Mods page's state.
#[derive(Default)]
pub(super) struct Browse {
    pub(super) open: Option<Opened>,
    pub(super) filters: Filters,
    pub(super) show_filters: bool,
    /// The Sort by dropdown's list is open.
    pub(super) sort_menu: bool,
    tags: HashMap<(SourceId, PackKind), Tags>,
    tags_asked: HashSet<(SourceId, PackKind)>,
    /// Images on disk, by URL and size; `None` while they load or when they failed.
    images: HashMap<(String, u32), Option<PathBuf>>,
    image_queue: Option<std::sync::mpsc::Sender<(String, u32)>>,
}

/// What the background sends this module.
pub(super) enum Loaded {
    Details((SourceId, String), Result<Arc<Details>, String>),
    Versions((SourceId, String), Result<Vec<ProjectVersion>, String>),
    Checked(
        (SourceId, String),
        String,
        eclipse_port_pack::check::Verdict,
    ),
    Image((String, u32), Option<PathBuf>),
    Tags((SourceId, PackKind), Result<Tags, String>),
}

/// Images fetched at once.
const IMAGE_WORKERS: usize = 4;

const LEADING_SLOT: f32 = 24.;

const FILTER_NAME: f32 = 120.;

const PAGE_TEXT: f32 = 760.;

const HEAD_TEXT_MIN: f32 = 240.;

const GALLERY_HEIGHT: f32 = 428.;

const THUMB: (f32, f32) = (120., 68.);

impl Launcher {
    pub(super) fn mods_loaded(&mut self, loaded: Loaded) {
        let b = &mut self.mods.browse;
        fn open<'a>(b: &'a mut Browse, key: &(SourceId, String)) -> Option<&'a mut Opened> {
            b.open
                .as_mut()
                .filter(|o| (o.project.source, &o.project.id) == (key.0, &key.1))
        }
        match loaded {
            Loaded::Details(key, d) => {
                if let Some(o) = open(b, &key) {
                    o.details = Some(d);
                }
            }
            Loaded::Versions(key, v) => {
                if let Some(o) = open(b, &key) {
                    o.versions = Some(v.map(Arc::from));
                    o.apply_checks();
                    o.version_list.reset();
                }
            }
            Loaded::Checked(key, version, verdict) => {
                if let Some(o) = open(b, &key) {
                    o.checked_versions.insert(version, verdict);
                    o.apply_checks();
                }
            }
            Loaded::Image(key, path) => {
                // A refresh that failed (offline) keeps the cached image.
                if path.is_some() || !matches!(b.images.get(&key), Some(Some(_))) {
                    b.images.insert(key, path);
                }
            }
            Loaded::Tags(key, Ok(tags)) => {
                b.tags.insert(key, tags);
            }
            Loaded::Tags(_, Err(_)) => {}
        }
    }

    /// Opens a project's page and asks its source for the rest of it.
    pub(super) fn mods_open(&mut self, project: Project) {
        let key = (project.source, project.id.clone());
        let tx = self.mods.tx.clone();
        let p = project.clone();
        std::thread::spawn(move || {
            let Some(source) = browser::source(p.source) else {
                return;
            };
            let d = source.details(&p).map(Arc::new);
            let _ = tx.send(Event::Loaded(Loaded::Details(key.clone(), d)));
            let v = source.versions(&p).map(|versions| {
                let home = backend::game_home();
                versions
                    .into_iter()
                    .map(|version| {
                        let verdict = version.file().and_then(|file| {
                            (p.kind == PackKind::Mod)
                                .then(|| java::cached(&home, file))
                                .flatten()
                                .map(|check| check.verdict)
                        });
                        ProjectVersion { version, verdict }
                    })
                    .collect()
            });
            let _ = tx.send(Event::Loaded(Loaded::Versions(key, v)));
        });
        self.mods.browse.open = Some(Opened {
            project,
            tab: Tab::Description,
            details: None,
            versions: None,
            version_list: cull::Cull::default(),
            checked_versions: HashMap::new(),
            shot: 0,
        });
    }

    /// The image of `url` at `size` when it is on disk; else asks for it once. One cached longer
    /// than `images::FRESH` is shown and asked for again, with its validators.
    pub(super) fn mods_image(&mut self, url: &str, size: u32) -> Option<PathBuf> {
        let key = (url.to_owned(), size);
        if let Some(state) = self.mods.browse.images.get(&key) {
            return state.clone();
        }
        // The launcher's data folder, shared by every instance.
        let home = backend::data_dir();
        if let Some(p) = images::cached(&home, url, size) {
            self.mods.browse.images.insert(key.clone(), Some(p.clone()));
            if images::stale(&home, url, size) {
                let _ = self.mods_image_queue().send(key);
            }
            return Some(p);
        }
        self.mods.browse.images.insert(key.clone(), None);
        if images::refuse(url).is_some() {
            return None;
        }
        let _ = self.mods_image_queue().send(key);
        None
    }

    /// The image workers' queue, started the first time an image is asked for.
    fn mods_image_queue(&mut self) -> std::sync::mpsc::Sender<(String, u32)> {
        let tx = self.mods.tx.clone();
        self.mods
            .browse
            .image_queue
            .get_or_insert_with(|| {
                let (send, receive) = std::sync::mpsc::channel::<(String, u32)>();
                let receive = std::sync::Arc::new(std::sync::Mutex::new(receive));
                let home = backend::data_dir();
                for _ in 0..IMAGE_WORKERS {
                    let (receive, tx, home) = (receive.clone(), tx.clone(), home.clone());
                    std::thread::spawn(move || loop {
                        let Ok(next) = receive.lock().map(|r| r.recv()) else {
                            break;
                        };
                        let Ok((url, size)) = next else {
                            break;
                        };
                        let path = images::fetch(&home, &url, size).ok();
                        if tx
                            .send(Event::Loaded(Loaded::Image((url, size), path)))
                            .is_err()
                        {
                            break;
                        }
                    });
                }
                send
            })
            .clone()
    }

    /// A project's icon, `side` pixels: its image, or its first letter on a plate until then.
    pub(super) fn mods_icon(&mut self, p: &Project, side: f32) -> AnyElement {
        let path = p
            .icon
            .clone()
            .and_then(|url| self.mods_image(&url, images::ICON));
        icons::slot(path.map(gpui::ImageSource::from), &p.title, side).into_any_element()
    }

    /// The values the filters can take for the current kind and source, asked for once.
    fn mods_tags(&mut self) -> Option<&Tags> {
        let key = (self.mods.source, self.mods.kind);
        if self.mods.browse.tags_asked.insert(key) {
            let tx = self.mods.tx.clone();
            std::thread::spawn(move || {
                let tags = browser::source(key.0)
                    .ok_or_else(|| "not set up".to_string())
                    .and_then(|s| s.tags(key.1));
                let _ = tx.send(Event::Loaded(Loaded::Tags(key, tags)));
            });
        }
        self.mods.browse.tags.get(&key)
    }

    /// Sets the filters and searches again.
    pub(super) fn mods_filter(&mut self, change: impl FnOnce(&mut Filters)) {
        change(&mut self.mods.browse.filters);
        self.mods_search_keeping(0, true);
    }

    /// The orders the current source can sort by, "Best match" first.
    pub(super) fn mods_sorts(&self) -> Vec<(Option<Sort>, &'static str)> {
        let own =
            self.mods.source == SourceId::Eclipse && self.mods.java_for(self.mods.kind).is_none();
        let all: &[Sort] = if own {
            &browser::eclipse::SORTS
        } else {
            &Sort::ALL
        };
        std::iter::once((None, "Best match"))
            .chain(all.iter().map(|s| (Some(*s), s.label())))
            .collect()
    }

    pub(super) fn mods_filters_button(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let shown = self.mods.browse.show_filters;
        let active = active_filters(&self.mods.browse.filters);
        let label = if active > 0 {
            trf("Filters ({count})", &[("count", &active.to_string())])
        } else {
            tr("Filters").to_string()
        };
        let sliders = div()
            .flex()
            .items_center()
            .justify_center()
            .size(px(LEADING_SLOT))
            .child(ui::icon(
                "sliders-horizontal",
                ui::IconSize::Px16,
                white(ink::MUTED),
            ));
        ui::dropdown(
            "mods-filters",
            Some(sliders.into_any_element()),
            label,
            shown,
            ui::Size::Md.height(),
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.mods.browse.show_filters = !this.mods.browse.show_filters;
            cx.notify();
        }))
    }

    /// The filter panel: loader, version, environment and categories when shown, only categories
    /// and tags for the registry. While the panel is closed, what is picked stays in view as chips
    /// that each remove theirs.
    pub(super) fn mods_filters(&mut self, cx: &mut Context<Self>) -> Option<Div> {
        let f = self.mods.browse.filters.clone();
        let registry =
            self.mods.source == SourceId::Eclipse && self.mods.java_for(self.mods.kind).is_none();
        let shown = self.mods.browse.show_filters;
        let active = active_filters(&f);
        if !shown && active == 0 {
            return None;
        }
        let mut bar = div().flex().flex_col().gap(px(12.));
        if !shown {
            if active > 0 {
                bar = bar.child(self.mods_picked_filters(&f, cx));
            }
            return Some(bar);
        }
        let Some(tags) = self.mods_tags().cloned() else {
            return Some(bar.child(
                ui::text(step::NOTE, FontWeight::NORMAL, ink::MUTED).child(tr("Loading filters")),
            ));
        };
        // A chip: on or off, one value of a filter.
        let chip = |id: String, label: String, on: bool| {
            ui::selected(
                ui::button(id, None, label, ButtonKind::Ghost, ui::Size::Sm),
                on,
            )
        };
        let row = |title: &'static str| {
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(4.))
                .child(ui::label(tr(title)).w(px(FILTER_NAME)))
        };
        let mut loaders = row("Loader");
        for l in &tags.loaders {
            let on = f.loader.as_deref() == Some(l.as_str());
            let value = l.clone();
            loaders = loaders.child(chip(format!("mods-loader-{l}"), l.clone(), on).on_press(
                cx.listener(move |this, _: &ClickEvent, _, cx| {
                    let v = value.clone();
                    this.mods_filter(|f| f.loader = if on { None } else { Some(v) });
                    cx.notify();
                }),
            ));
        }
        let mut versions = row("Game version");
        if let Some(grass) = self.grass_mark(20.) {
            versions = versions.child(img(grass).size(px(20.)).flex_none());
        }
        for v in tags.game_versions.iter().take(12) {
            let on = f.game_version.as_deref() == Some(v.as_str());
            let value = v.clone();
            versions = versions.child(chip(format!("mods-version-{v}"), v.clone(), on).on_press(
                cx.listener(move |this, _: &ClickEvent, _, cx| {
                    let v = value.clone();
                    this.mods_filter(|f| f.game_version = if on { None } else { Some(v) });
                    cx.notify();
                }),
            ));
        }
        let mut environment = row("Environment");
        for e in Environment::ALL {
            let on = f.environment == Some(e);
            environment = environment.child(
                chip(format!("mods-env-{}", e.label()), e.label().into(), on).on_press(
                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.mods_filter(|f| f.environment = if on { None } else { Some(e) });
                        cx.notify();
                    }),
                ),
            );
        }
        let mut categories = row("Categories");
        for c in &tags.categories {
            let on = f.categories.contains(&c.name);
            let value = c.name.clone();
            categories = categories.child(
                chip(format!("mods-cat-{}", c.name), category_name(&c.name), on).on_press(
                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                        let v = value.clone();
                        this.mods_filter(|f| {
                            if !f.categories.contains(&v) {
                                f.categories.push(v);
                            } else {
                                f.categories.retain(|x| *x != v);
                            }
                        });
                        cx.notify();
                    }),
                ),
            );
        }
        let _ = CATEGORY_NAMES;
        let mut tag_row = row("Tags");
        for tag in &tags.tags {
            let on = f.tags.contains(tag);
            let value = tag.clone();
            tag_row = tag_row.child(chip(format!("mods-tag-{tag}"), tag_name(tag), on).on_press(
                cx.listener(move |this, _: &ClickEvent, _, cx| {
                    let v = value.clone();
                    this.mods_filter(|f| {
                        if !f.tags.contains(&v) {
                            f.tags.push(v);
                        } else {
                            f.tags.retain(|x| *x != v);
                        }
                    });
                    cx.notify();
                }),
            ));
        }
        let clear = button("mods-clear", "Clear filters", ButtonKind::Ghost).on_press(cx.listener(
            |this, _: &ClickEvent, _, cx| {
                this.mods_filter(|f| *f = Filters::default());
                cx.notify();
            },
        ));
        if !registry {
            bar = bar.child(loaders).child(versions).child(environment);
        }
        bar = bar.child(categories);
        if registry && !tags.tags.is_empty() {
            bar = bar.child(tag_row);
        }
        if active > 0 {
            bar = bar.child(div().flex().child(clear));
        }
        Some(bar)
    }

    /// What the filters pick, each a chip with a cross that removes it, then Clear filters.
    fn mods_picked_filters(&self, f: &Filters, cx: &mut Context<Self>) -> Div {
        type Remove = Rc<dyn Fn(&mut Filters)>;
        let mut picked: Vec<(String, String, Remove)> = Vec::new();
        for c in &f.categories {
            let v = c.clone();
            picked.push((
                format!("cat-{c}"),
                category_name(c),
                Rc::new(move |f| f.categories.retain(|x| *x != v)),
            ));
        }
        for tag in &f.tags {
            let v = tag.clone();
            picked.push((
                format!("tag-{tag}"),
                tag_name(tag),
                Rc::new(move |f| f.tags.retain(|x| *x != v)),
            ));
        }
        if let Some(l) = &f.loader {
            picked.push(("loader".into(), l.clone(), Rc::new(|f| f.loader = None)));
        }
        if let Some(v) = &f.game_version {
            picked.push((
                "version".into(),
                v.clone(),
                Rc::new(|f| f.game_version = None),
            ));
        }
        if let Some(e) = f.environment {
            picked.push((
                "env".into(),
                e.label().into(),
                Rc::new(|f| f.environment = None),
            ));
        }
        let mut row = div().flex().flex_wrap().items_center().gap(px(6.));
        for (id, label, remove) in picked {
            let cross = ui::icon("cancel-01", ui::IconSize::Px16, white(ink::BODY));
            row = row.child(
                ui::button(
                    format!("mods-picked-{id}"),
                    None,
                    label,
                    ButtonKind::Secondary,
                    ui::Size::Sm,
                )
                .child(cross)
                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    let remove = remove.clone();
                    this.mods_filter(move |f| remove(f));
                    cx.notify();
                })),
            );
        }
        row.child(
            button("mods-picked-clear", "Clear filters", ButtonKind::Ghost).on_press(cx.listener(
                |this, _: &ClickEvent, _, cx| {
                    // The sort has its own control: only what filters goes.
                    this.mods_filter(|f| {
                        *f = Filters {
                            sort: f.sort,
                            ..Filters::default()
                        }
                    });
                    cx.notify();
                },
            )),
        )
    }

    /// The open project's page.
    pub(super) fn mods_project_page(&mut self, cx: &mut Context<Self>) -> Option<Div> {
        let o = self.mods.browse.open.as_ref()?;
        let p = o.project.clone();
        let tab = o.tab;
        let shot = o.shot;
        let back = button("mods-back", "Back to results", ButtonKind::Ghost).on_press(cx.listener(
            |this, _: &ClickEvent, _, cx| {
                this.mods.browse.open = None;
                cx.notify();
            },
        ));
        let details = o.details.clone();
        let versions = o.versions.clone();
        let version_list = o.version_list.clone();
        let mut meta = vec![trf(
            "{count} downloads",
            &[("count", &thousands(p.downloads))],
        )];
        if let Some(Ok(d)) = &details {
            // A registry project has no followers: its source repository's stars stand there.
            match &d.github {
                Some(g) => meta.push(trf(
                    "{count} stars on GitHub",
                    &[("count", &thousands(g.stars))],
                )),
                None if p.source != SourceId::Eclipse => meta.push(trf(
                    "{count} followers",
                    &[("count", &thousands(d.followers))],
                )),
                None => {}
            }
            if let Some(day) = d.updated.get(..10) {
                meta.push(trf(
                    "updated {date}",
                    &[("date", &versions_page::published(day, ""))],
                ));
            }
        }
        if !p.license.is_empty() {
            meta.push(p.license.clone());
        }
        if let Some(source) = p.source.shown_for(&p.id) {
            meta.push(trf("from {source}", &[("source", source)]));
        }
        let authors = match &details {
            Some(Ok(d)) if !d.authors.is_empty() => d.authors.join(", "),
            _ => p.author.clone(),
        };
        let categories = match &details {
            Some(Ok(d)) if !d.categories.is_empty() => Some(
                d.categories
                    .iter()
                    .map(|c| category_name(c))
                    .collect::<Vec<_>>()
                    .join(" · "),
            ),
            _ => None,
        };
        let icon = self.mods_icon(&p, 72.);
        let busy = self.mods.busy.as_ref().map(|b| b.0.clone());
        let installed = self.mods.entry(&p).is_some();
        let checking = self.mods.checking(&p);
        let instance = self
            .selected_instance()
            .map_or_else(|| "Eclipse".to_owned(), |i| i.name.clone());
        let vanilla = self.mods_vanilla_target();
        let status = match checking {
            true => Some((ui::Tone::Busy, tr("Checking its jar…").to_string(), None)),
            false => self
                .mods_status(&p, &instance)
                .map(|(tone, said, reason)| match tone {
                    ui::Tone::Warn if vanilla.is_some() => (ui::Tone::Off, said, reason),
                    tone => (tone, said, reason),
                }),
        };
        let state = |tone: ui::Tone, said: String| {
            ui::text(step::NOTE, FontWeight::MEDIUM, ink::BODY).child(ui::state_line(tone, said))
        };
        let mut action = div().flex().flex_none().items_center().gap(px(12.));
        if busy.as_deref() == Some(p.id.as_str()) {
            action = action.child(state(ui::Tone::Busy, tr("Installing…").to_string()));
        } else if installed {
            action = action.child(state(
                ui::Tone::Live,
                trf("Installed in {instance}", &[("instance", &instance)]),
            ));
        } else if self.mods.support(&p).installable() && !checking {
            let label = if p.kind == PackKind::Modpack {
                tr("Import as an instance").to_string()
            } else {
                trf("Install in {instance}", &[("instance", &instance)])
            };
            let mut install = ui::primary("mods-page-install", label, ui::Size::Md);
            if busy.is_some() {
                install = disabled(install);
            } else {
                let p = p.clone();
                install = install.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.mods_install(p.source, p.id.clone(), Some(p.clone()));
                    cx.notify();
                }));
            }
            action = action.child(install);
        } else if let Some(minecraft) = vanilla {
            action = action.child(
                ui::button(
                    "mods-page-new-fabric",
                    None,
                    tr("Create a Fabric instance"),
                    ButtonKind::Secondary,
                    ui::Size::Md,
                )
                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.mods_new_fabric(minecraft.clone(), cx);
                    cx.notify();
                })),
            );
        }
        let head = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(20.))
            .child(icon)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(HEAD_TEXT_MIN))
                    .gap(px(4.))
                    .child(ui::entity_title(p.title.clone(), pixel_text::CARD_TITLE))
                    .child(ui::text(step::BODY, FontWeight::NORMAL, ink::BODY).child(
                        if authors.is_empty() {
                            p.summary.clone()
                        } else {
                            format!(
                                "{} · {}",
                                trf("by {author}", &[("author", &authors)]),
                                p.summary
                            )
                        },
                    ))
                    .child(
                        ui::text(step::NOTE, FontWeight::NORMAL, ink::MUTED)
                            .child(meta.join(" · ")),
                    )
                    .children(
                        categories
                            .map(|c| ui::text(step::NOTE, FontWeight::NORMAL, ink::DIM).child(c)),
                    ),
            )
            .child(action);
        let mut page =
            div()
                .flex()
                .flex_col()
                .gap(px(24.))
                .child(div().flex().child(back))
                .child(head)
                .children(status.map(|(tone, said, reason)| {
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .child(state(tone, said))
                        .children(reason.map(|why| {
                            ui::text(step::NOTE, FontWeight::NORMAL, ink::MUTED).child(why)
                        }))
                }));
        // Registry projects are starred on their source repository, which the player does there
        // themselves.
        if p.source == SourceId::Eclipse {
            let repo = match &details {
                Some(Ok(d)) => d.github.as_ref().map(|g| g.url.clone()),
                _ => None,
            };
            page = page.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(16.))
                    .children(super::mods_page::stars(&p.id))
                    .children(repo.map(|repo| {
                        button("mods-star", "Star on GitHub", ButtonKind::Ghost)
                            .on_press(move |_, _, cx| cx.open_url(&repo))
                    })),
            );
        }
        if let Some(Checking::Done(c)) = self.mods.checks.get(&(p.source, p.id.clone())) {
            let lines = |title: &str, list: &[String]| {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(ui::label(tr(title)))
                    .children(list.iter().map(|l| {
                        ui::text(step::NOTE, FontWeight::NORMAL, ink::BODY).child(l.clone())
                    }))
            };
            let mut what = div().flex().flex_wrap().gap(px(32.));
            if !c.ports.is_empty() {
                what = what.child(lines("Ports", &c.ports));
            }
            if !c.skipped.is_empty() {
                what = what.child(lines("Doesn't port", &c.skipped));
            }
            page = page.child(what);
        }
        if let Some(Ok(d)) = &details {
            let mut links = div().flex().flex_wrap().gap(px(8.));
            let page_url = p.page.clone();
            if !page_url.is_empty() {
                links = links.child(
                    button(
                        "mods-link-page",
                        trf("Open on {source}", &[("source", p.source.label())]),
                        ButtonKind::Secondary,
                    )
                    .on_press(move |_, _, cx| cx.open_url(&page_url)),
                );
            }
            for (i, l) in d.links.iter().enumerate() {
                let url = l.url.clone();
                links = links.child(
                    button(format!("mods-link-{i}"), l.label.clone(), ButtonKind::Ghost)
                        .on_press(move |_, _, cx| cx.open_url(&url)),
                );
            }
            page = page.child(links);
        }
        let this = cx.entity().downgrade();
        // No Gallery tab for a project without images.
        let no_gallery = matches!(&details, Some(Ok(d)) if d.gallery.is_empty());
        let tabs: Vec<(Tab, &str)> = TABS
            .into_iter()
            .filter(|(tab, _)| !(no_gallery && *tab == Tab::Gallery))
            .collect();
        page = page.child(div().flex().child(ui::segments("mods-tab", &tabs, tab, {
            move |value, _, cx| {
                let _ = this.update(cx, |this, cx| {
                    if let Some(o) = &mut this.mods.browse.open {
                        o.tab = value;
                    }
                    cx.notify();
                });
            }
        })));
        let retry = |cx: &mut Context<Self>| {
            let again = p.clone();
            ui::button(
                "mods-project-retry",
                None,
                tr("Try again"),
                ButtonKind::Secondary,
                ui::Size::Md,
            )
            .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.mods_open(again.clone());
                cx.notify();
            }))
        };
        let failed = |e: &str, cx: &mut Context<Self>| {
            ui::error_state(
                "mods-project-failed",
                tr("Couldn't load the page"),
                e,
                retry(cx),
            )
            .into_any_element()
        };
        let loading =
            || ui::loading_state("mods-project-loading", tr("Loading…")).into_any_element();
        let body = match tab {
            Tab::Description => match &details {
                None => loading(),
                Some(Err(e)) => failed(e, cx),
                Some(Ok(d)) => div()
                    .max_w(px(PAGE_TEXT))
                    .child(markdown::render(&d.body))
                    .into_any_element(),
            },
            Tab::Gallery => match &details {
                None => loading(),
                Some(Err(e)) => failed(e, cx),
                Some(Ok(d)) if d.gallery.is_empty() => ui::empty()
                    .child(tr("No gallery images."))
                    .into_any_element(),
                Some(Ok(d)) => self.mods_gallery(&d.gallery, shot, cx).into_any_element(),
            },
            Tab::Versions => match &versions {
                None => loading(),
                Some(Err(e)) => failed(e, cx),
                Some(Ok(list)) => version_list
                    .column(
                        &self.content_scroll.handle,
                        list.len().min(50),
                        0.,
                        |_| false,
                        |i| {
                            let v = &list[i].version;
                            let file = v.file();
                            let mut detail = vec![
                                versions_page::published(
                                    v.published.get(..10).unwrap_or_default(),
                                    "",
                                ),
                                v.loaders.join(", "),
                            ];
                            let games: Vec<&str> = v
                                .game_versions
                                .iter()
                                .rev()
                                .take(4)
                                .map(String::as_str)
                                .collect();
                            detail.push(games.join(", "));
                            if let Some(f) = file {
                                detail.push(format!("{} · {:.1} MB", f.name, f.size as f64 / 1e6));
                            }
                            ui::list_row(
                                SharedString::from(format!("mods-version-{i}")),
                                false,
                                |_, _, _| {},
                            )
                            .child(ui::item_text(v.number.clone(), detail.join(" · ")))
                            .children(list[i].verdict.as_ref().map(|verdict| {
                                ui::text(step::NOTE, FontWeight::NORMAL, ink::MUTED)
                                    .flex_none()
                                    .child(tr(verdict.label()))
                            }))
                        },
                    )
                    .into_any_element(),
            },
        };
        Some(page.child(body))
    }
}

impl Launcher {
    /// A gallery: image `shot` large, the previous and next ones a click away, and every image
    /// as a strip of thumbnails that picks the large one.
    fn mods_gallery(&mut self, gallery: &[Image], shot: usize, cx: &mut Context<Self>) -> Div {
        let count = gallery.len();
        let shot = shot.min(count.saturating_sub(1));
        let pick = |to: usize| {
            cx.listener(move |this: &mut Self, _: &ClickEvent, _, cx| {
                if let Some(o) = &mut this.mods.browse.open {
                    o.shot = to;
                }
                cx.notify();
            })
        };
        let image = |this: &mut Self, url: &str, w: f32, h: f32| match this
            .mods_image(url, images::GALLERY)
        {
            Some(path) => img(path)
                .w(px(w))
                .h(px(h))
                .object_fit(gpui::ObjectFit::Contain)
                .into_any_element(),
            None => ui::tile(w).h(px(h)).into_any_element(),
        };
        let current = &gallery[shot];
        let large = image(self, &current.url, PAGE_TEXT, GALLERY_HEIGHT);
        let mut out = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .max_w(px(PAGE_TEXT))
            .child(
                ui::hover_over(
                    "mods-shot",
                    radius::ROW,
                    div()
                        .rounded(px(radius::ROW))
                        .overflow_hidden()
                        .bg(ui::well())
                        .child(large),
                )
                .cursor_pointer()
                .on_press(pick((shot + 1) % count)),
            );
        if !current.title.is_empty() {
            out = out.child(
                ui::text(step::NOTE, FontWeight::NORMAL, ink::BODY).child(current.title.clone()),
            );
        }
        if count < 2 {
            return out;
        }
        let mut strip = div().flex().flex_wrap().items_center().gap(px(8.));
        for (i, g) in gallery.iter().enumerate() {
            let thumb = image(self, &g.url, THUMB.0, THUMB.1);
            strip = strip.child(
                ui::selected(
                    div()
                        .id(SharedString::from(format!("mods-shot-{i}")))
                        .flex_none()
                        .p(px(2.))
                        .cursor_pointer()
                        .child(
                            div()
                                .rounded(px(radius::CONTROL))
                                .overflow_hidden()
                                .child(thumb),
                        ),
                    i == shot,
                )
                .on_press(pick(i)),
            );
        }
        strip = strip.child(
            ui::text(step::NOTE, FontWeight::NORMAL, ink::MUTED)
                .child(format!("{} / {count}", shot + 1)),
        );
        out.child(strip)
    }
}

impl Opened {
    /// A check can finish before or after the version request; retain either event order.
    fn apply_checks(&mut self) {
        let Some(Ok(versions)) = &mut self.versions else {
            return;
        };
        for row in Arc::make_mut(versions) {
            if let Some(verdict) = self.checked_versions.get(&row.version.id) {
                row.verdict = Some(*verdict);
            }
        }
    }
}
