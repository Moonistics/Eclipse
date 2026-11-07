//! Eclipse's own registry (docs/api/mod-index.md): mods, modpacks and resource packs made for
//! Eclipse, reviewed as pull requests in horizon-rust/horizon-mods. One file lists them all,
//! `index.json`; every download is checked against it before it reaches the game folder
//! (`eclipse_mods::index::check_file`: size, SHA-256, the author's signature, the file's own
//! manifest), and files the index says were taken down are refused and turned off.
use super::{
    details::{Details, GitHub, Link},
    download, installed, java, FileRef, Kind, Project, Query, Source, SourceId, Version, GAME,
};
use crate::game_folder;
use eclipse_mods::index::{self, Index, Trust};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The registry's repository: its projects' entries, reports and policy.
const REPOSITORY: &str = "https://github.com/horizon-rust/horizon-mods";
/// Its GitHub Pages: the index and the projects' icons.
pub(super) const PAGES: &str = "https://horizon-rust.github.io/horizon-mods/";
/// Its Pages and its files at the address they had before the repositories moved to the
/// Moonistics organization (GitHub redirects them).
pub(super) const MOVED_PAGES: &str = "https://ludovic111.github.io/horizon-mods/";
pub(super) const MOVED_FILES: &str =
    "https://github.com/ludovic111/horizon-mods/releases/download/";
/// The loader name the browser gives registry versions, so `support` and `pick` know them.
pub(crate) const LOADER: &str = "eclipse";
/// How long a fetched index is used before it is fetched again.
const FRESH: Duration = Duration::from_secs(600);
/// Larger than any index should be.
const MAX_INDEX: u64 = 16 << 20;
/// The last index fetched, kept in the game folder for when the registry can't be reached.
const CACHE: &str = "mod-browser/eclipse-index.json";

const UNREADABLE: &str =
    "Eclipse's mod list came back unreadable. A firewall or proxy may be in the way.";

/// Where the index comes from: `ECLIPSE_MOD_INDEX` (a file or an `https://` address, for a
/// test registry), else the registry's own on its [`PAGES`].
fn location() -> String {
    std::env::var("ECLIPSE_MOD_INDEX")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| format!("{PAGES}index.json"))
}

/// True when the index is a local file: its files may then be local too (`file://`), for
/// trying a registry out before it is published.
pub fn local() -> bool {
    !location().starts_with("https://")
}

static LOADED: Mutex<Option<(Instant, Arc<Index>)>> = Mutex::new(None);

/// The index read last in this run, without asking the network: for what must not wait.
pub fn known_index() -> Option<Arc<Index>> {
    LOADED
        .lock()
        .unwrap()
        .as_ref()
        .map(|(_, index)| index.clone())
}

/// Whether the registry's project `id` is a native mod, by the index already read (never the
/// network); false when none was read.
pub fn is_native_mod(id: &str) -> bool {
    known_index()
        .and_then(|index| Some(index.project(id)?.kind == index::Kind::NativeMod))
        .unwrap_or(false)
}

/// The index: the one fetched in the last ten minutes, else fetched again (and kept in the game
/// folder), else the copy kept last time.
pub fn index() -> Result<Arc<Index>, String> {
    if let Some((at, index)) = LOADED.lock().unwrap().as_ref() {
        if at.elapsed() < FRESH {
            return Ok(index.clone());
        }
    }
    let cache = crate::backend::game_home().join(CACHE);
    let index = match fetch_index() {
        Ok((index, text)) => {
            if let Some(dir) = cache.parent() {
                let _ = fs::create_dir_all(dir);
            }
            let _ = fs::write(&cache, text);
            index
        }
        Err(e) => {
            let kept = fs::read_to_string(&cache).map_err(|_| e.clone())?;
            parse(&kept).map_err(|_| e)?
        }
    };
    let index = Arc::new(index);
    *LOADED.lock().unwrap() = Some((Instant::now(), index.clone()));
    Ok(index)
}

fn fetch_index() -> Result<(Index, String), String> {
    use std::io::Read;
    let at = location();
    let text = if at.starts_with("https://") {
        let response = download::agent(&at)
            .get(&at)
            .call()
            .map_err(|e| crate::backend::network_error(&at, e))?;
        let mut text = String::new();
        response
            .into_reader()
            .take(MAX_INDEX)
            .read_to_string(&mut text)
            .map_err(|e| e.to_string())?;
        text
    } else {
        fs::read_to_string(&at).map_err(|e| format!("{at}: {e}"))?
    };
    Ok((read_index(&text)?, text))
}

fn read_index(text: &str) -> Result<Index, String> {
    parse(text).map_err(|e| format!("{UNREADABLE}\n{e}"))
}

fn parse(text: &str) -> Result<Index, String> {
    match local() {
        true => Index::parse_local(text),
        false => Index::parse(text),
    }
}

/// The browser's kind for a registry kind; None for one the launcher doesn't install yet
/// (kinds newer than this launcher).
fn kind_of(kind: &index::Kind) -> Option<Kind> {
    match kind {
        index::Kind::Mod => Some(Kind::Mod),
        index::Kind::ResourcePack => Some(Kind::ResourcePack),
        index::Kind::Modpack => Some(Kind::Modpack),
        // Both go in shaderpacks/: the game reads `.eshader` packs and shaders/-folder zips.
        index::Kind::Shader | index::Kind::ShaderPack => Some(Kind::Shader),
        // Mixin mods: installed beside the others, compiled into the game by `native_mods`.
        index::Kind::NativeMod => Some(Kind::Mod),
        // Into a world the player picks ([`installed::install_datapack`]).
        index::Kind::DataPack => Some(Kind::DataPack),
        index::Kind::Other(_) => None,
    }
}

/// The registry kind whose category list a browser kind filters by (`page::categories`): a
/// native mod's list is a mod's, a shaders/-folder pack's a shader pack's.
fn categories_of(kind: Kind) -> &'static [&'static str] {
    index::page::categories(&match kind {
        Kind::Mod => index::Kind::Mod,
        Kind::ResourcePack => index::Kind::ResourcePack,
        Kind::Modpack => index::Kind::Modpack,
        Kind::Shader => index::Kind::Shader,
        Kind::DataPack => index::Kind::DataPack,
    })
}

/// The most tags the Filters panel offers for a kind.
const MAX_TAGS_SHOWN: usize = 30;

/// The tags the registry's projects of `kind` carry, plain ones only (as the site keeps them):
/// the most used first, then by name.
fn tags_of(index: &Index, kind: Kind) -> Vec<String> {
    let plain = |t: &str| {
        t.len() <= index::page::MAX_TAG
            && t.bytes().next().is_some_and(|b| b != b'-')
            && t.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    };
    let mut uses: HashMap<&str, usize> = HashMap::new();
    for p in index
        .projects
        .iter()
        .filter(|p| kind_of(&p.kind) == Some(kind))
    {
        let mut own: Vec<&str> = p
            .tags
            .iter()
            .map(String::as_str)
            .filter(|t| plain(t))
            .collect();
        own.sort_unstable();
        own.dedup();
        for t in own {
            *uses.entry(t).or_default() += 1;
        }
    }
    let mut tags: Vec<(&str, usize)> = uses.into_iter().collect();
    tags.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    tags.into_iter()
        .take(MAX_TAGS_SHOWN)
        .map(|(t, _)| t.to_owned())
        .collect()
}

/// Whether the Eclipse project made `p` (the mod policy's "Official": author Eclipse, source in
/// the Moonistics organisation).
pub fn official(p: &index::Project) -> bool {
    let (owner, _) = crate::backend::REPO.split_once('/').unwrap_or_default();
    p.authors == ["Eclipse"]
        && p.source
            .starts_with(&format!("https://github.com/{owner}/"))
}

/// Whether the registry's project `id` is Eclipse's own ([`official`]), by the index already read
/// (never the network); false when none was read.
pub(crate) fn is_official(id: &str) -> bool {
    official_in(known_index().as_deref(), id)
}

fn official_in(index: Option<&Index>, id: &str) -> bool {
    index.and_then(|i| i.project(id)).is_some_and(official)
}

/// What the player reads about a registry project before installing it: who made it, whether its
/// files are signed, and for a mod what it may do, in the game's words (Options > Mods). A native
/// mod says it runs with the player's rights, and how it fits the Eclipse `instance` plays.
pub fn trust_and_capabilities(
    id: &str,
    instance: Option<&crate::instances::Instance>,
    settings: &crate::backend::Settings,
) -> Vec<String> {
    let Ok(index) = index() else {
        return Vec::new();
    };
    let Some(p) = index.project(id) else {
        return Vec::new();
    };
    let mut lines = vec![if official(p) {
        "Made by Eclipse".to_owned()
    } else {
        format!(
            "Made by {}. Not made or endorsed by the Eclipse project ({REPOSITORY}/blob/main/MOD-POLICY.md)",
            p.authors.join(", ")
        )
    }];
    lines.push(match &p.key {
        Some(_) => "Signed by its author".to_owned(),
        None => "Not signed by its author".to_owned(),
    });
    if p.kind == index::Kind::NativeMod {
        lines.push(
            "Native code: runs with full access to your computer. Its source was reviewed".into(),
        );
        if let Some(v) = p.versions.iter().find(|v| v.game == GAME) {
            let pin = v.api.split('/').nth(1).unwrap_or_default();
            lines.push(
                instance
                    .and_then(|i| {
                        crate::native_fit::line(i, settings, &p.id, v.hooks, pin, &v.broken_on)
                    })
                    .unwrap_or_else(|| {
                        format!(
                            "Built by the launcher on Eclipse {}",
                            &pin[..pin.len().min(7)]
                        )
                    }),
            );
        }
    }
    if let Some(v) = p.versions.first().filter(|_| p.kind == index::Kind::Mod) {
        lines.extend(
            v.capabilities
                .iter()
                .map(|c| format!("May {}", c.describe())),
        );
        if v.capabilities.is_empty() {
            lines.push("Asks for nothing beyond its own log".into());
        }
    }
    lines
}

fn project_of(p: &index::Project) -> Option<Project> {
    let mut game_versions: Vec<String> = p.versions.iter().map(|v| v.game.clone()).collect();
    game_versions.dedup();
    Some(Project {
        source: SourceId::Eclipse,
        id: p.id.clone(),
        slug: p.id.clone(),
        title: p.name.clone(),
        summary: p.summary.clone(),
        author: p.authors.join(", "),
        downloads: p.downloads,
        page: format!("{REPOSITORY}/blob/main/mods/{}.toml", p.id),
        icon: p.icon.clone(),
        license: p.license.clone(),
        kind: kind_of(&p.kind)?,
        game_versions,
        loaders: vec![LOADER.into()],
    })
}

/// Version `v` of `p` as the browser shows it; blocked when taken down, or when it needs a newer
/// mod API level than `level`, the game's.
fn version_of(
    index: &Index,
    p: &index::Project,
    v: &index::Version,
    level: Option<u32>,
) -> Version {
    Version {
        id: v.version.clone(),
        number: v.version.clone(),
        published: v.published.clone(),
        game_versions: vec![v.game.clone()],
        loaders: vec![LOADER.into()],
        files: vec![FileRef {
            url: v.file.url.clone(),
            name: v.file.name.clone(),
            size: v.file.size,
            sha512: None,
            sha256: Some(v.file.sha256.clone()),
            sha1: None,
            primary: true,
        }],
        required: v.requires.iter().map(|r| r.id.clone()).collect(),
        blocked: index
            .revoked(&p.id, &v.file.sha256)
            .map(|r| format!("taken down: {}", r.reason))
            .or_else(|| needs_port(v).then(|| NEEDS_PORT.to_string()))
            .or_else(|| needs_newer(v, level).then(|| NEEDS_NEWER.to_string())),
    }
}

/// A GitHub login as GitHub allows it (letters, digits, single dashes, at most 39), so an index
/// entry can't turn a link into something else.
fn github_login(login: &str) -> bool {
    (1..=39).contains(&login.len())
        && login
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && !login.starts_with('-')
        && !login.ends_with('-')
        && !login.contains("--")
}

/// The GitHub repository a project's `source` is in, `https://github.com/<owner>/<repo>`: a
/// deeper path (a folder of a monorepo) or a trailing `.git` names the same repository. None
/// when the source isn't on GitHub, or its owner or name isn't one GitHub allows.
fn github_repo(source: &str) -> Option<String> {
    let path = source.strip_prefix("https://github.com/")?;
    let mut parts = path.split(['/', '?', '#']);
    let owner = parts.next().filter(|o| github_login(o))?;
    let repo = parts.next()?;
    let repo = repo.strip_suffix(".git").unwrap_or(repo);
    let allowed = |b: u8| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.');
    if repo.is_empty() || repo.len() > 100 || repo.starts_with('.') || !repo.bytes().all(allowed) {
        return None;
    }
    Some(format!("https://github.com/{owner}/{repo}"))
}

fn matches(p: &index::Project, text: &str) -> bool {
    let hay = format!(
        "{} {} {} {} {}",
        p.id,
        p.name,
        p.summary,
        p.authors.join(" "),
        p.tags.join(" ")
    )
    .to_lowercase();
    text.split_whitespace()
        .all(|word| hay.contains(&word.to_lowercase()))
}

/// The sorts the registry can honor: it has downloads and each version's publish day, but no
/// follows and no relevance score (its search is a word match).
pub const SORTS: [super::details::Sort; 3] = [
    super::details::Sort::Downloads,
    super::details::Sort::Newest,
    super::details::Sort::Updated,
];

/// The projects of the query's kind whose words match and that have every category and tag the
/// filters ask for, for this game unless `everything`: the
/// most downloaded first, or the newest (first published) or most recently updated (latest
/// version) when the query asks, ties by downloads.
fn search(index: &Index, query: &Query) -> Vec<Project> {
    use super::details::Sort;
    let mut found: Vec<&index::Project> = index
        .projects
        .iter()
        .filter(|p| kind_of(&p.kind) == Some(query.kind) && matches(p, &query.text))
        .filter(|p| {
            query
                .filters
                .categories
                .iter()
                .all(|c| p.categories.contains(c))
                && query.filters.tags.iter().all(|t| p.tags.contains(t))
        })
        .filter(|p| query.everything || p.versions.iter().any(|v| v.game == GAME))
        // A WebAssembly mod stays in the index for the modpacks that need it, not in the browser.
        .filter(|p| !p.versions.iter().filter(|v| v.game == GAME).all(needs_port))
        .collect();
    let day = |p: &index::Project, newest: bool| {
        let days = p.versions.iter().map(|v| v.published.as_str());
        if newest { days.min() } else { days.max() }
            .unwrap_or("")
            .to_string()
    };
    match query.filters.sort {
        Some(Sort::Newest) => found.sort_by_cached_key(|p| {
            (
                std::cmp::Reverse(day(p, true)),
                std::cmp::Reverse(p.downloads),
            )
        }),
        Some(Sort::Updated) => found.sort_by_cached_key(|p| {
            (
                std::cmp::Reverse(day(p, false)),
                std::cmp::Reverse(p.downloads),
            )
        }),
        _ => found.sort_by_key(|p| std::cmp::Reverse(p.downloads)),
    }
    found
        .into_iter()
        .filter_map(project_of)
        .skip(query.offset)
        .take(query.limit)
        .collect()
}

/// A project's screenshots as its page's gallery, in the index's order: those the registry
/// mirrored (`screenshots/<id>/`, served beside its icons), never another host's.
fn gallery_of(p: &index::Project) -> Vec<super::details::Image> {
    p.screenshots
        .iter()
        .filter(|url| super::images::refuse(url).is_none())
        .map(|url| super::details::Image {
            url: url.clone(),
            title: String::new(),
            description: String::new(),
            featured: false,
        })
        .collect()
}

pub struct Registry;

impl Source for Registry {
    fn id(&self) -> SourceId {
        SourceId::Eclipse
    }

    fn search(&self, query: &Query) -> Result<Vec<Project>, String> {
        Ok(search(&*index()?, query))
    }

    fn project(&self, id: &str) -> Result<Project, String> {
        let index = index()?;
        let p = index
            .project(id)
            .ok_or_else(|| format!("{id} isn't in Eclipse's registry."))?;
        project_of(p).ok_or_else(|| {
            format!(
                "{} is a {} in Eclipse's registry: this launcher can't install those yet.",
                p.name,
                p.kind.name()
            )
        })
    }

    fn versions(&self, project: &Project, level: Option<u32>) -> Result<Vec<Version>, String> {
        let index = index()?;
        let p = index
            .project(&project.id)
            .ok_or_else(|| format!("{} isn't in Eclipse's registry.", project.title))?;
        Ok(p.versions
            .iter()
            .map(|v| version_of(&index, p, v, level))
            .collect())
    }

    /// Only categories and tags: every registry file is for Eclipse on the version it plays. The
    /// tags come from the index the search already read (never the network).
    fn tags(&self, kind: Kind) -> Result<super::details::Tags, String> {
        Ok(super::details::Tags {
            tags: known_index().map(|i| tags_of(&i, kind)).unwrap_or_default(),
            categories: categories_of(kind)
                .iter()
                .map(|name| super::details::Tag {
                    name: (*name).to_owned(),
                    header: "categories".into(),
                })
                .collect(),
            ..Default::default()
        })
    }

    fn details(&self, project: &Project) -> Result<Details, String> {
        let index = index()?;
        let p = index
            .project(&project.id)
            .ok_or_else(|| format!("{} isn't in Eclipse's registry.", project.title))?;
        let newest = p.versions.first();
        let mut body = p.description.clone();
        if let Some(v) = newest.filter(|v| !v.capabilities.is_empty()) {
            body.push_str("\n\n## What it may do\n\n");
            for c in &v.capabilities {
                body.push_str(&format!("- May {}\n", c.describe()));
            }
        }
        if let Some(v) = newest.filter(|v| !v.requires.is_empty()) {
            body.push_str(
                "\n\n## Its mods\n\nInstalled with it into its own profile, each checked:\n\n",
            );
            for r in &v.requires {
                let found = index
                    .project(&r.id)
                    .and_then(|m| Some((m, m.versions.iter().find(|w| w.version == r.version)?)));
                let Some((m, w)) = found else { continue };
                let may: Vec<&str> = w.capabilities.iter().map(|c| c.describe()).collect();
                let may = match may.is_empty() {
                    true => String::new(),
                    false => format!(": may {}", may.join(", ")),
                };
                body.push_str(&format!("- {} {}{may}\n", m.name, w.version));
            }
        }
        // Who publishes it: the GitHub accounts the registry checked at submission.
        let owners = p.owners.iter().filter(|o| github_login(o)).map(|o| Link {
            label: format!("@{o}"),
            url: format!("https://github.com/{o}"),
        });
        Ok(Details {
            body,
            links: [
                Link {
                    label: "Source code".into(),
                    url: p.source.clone(),
                },
                Link {
                    label: "Page".into(),
                    url: project.page.clone(),
                },
                Link {
                    label: "Report to Eclipse".into(),
                    url: format!("{REPOSITORY}/issues/new?template=report.yml&title={}", p.id),
                },
            ]
            .into_iter()
            .chain(owners)
            .collect(),
            authors: p.authors.clone(),
            published: p
                .versions
                .last()
                .map(|v| v.published.clone())
                .unwrap_or_default(),
            updated: newest.map(|v| v.published.clone()).unwrap_or_default(),
            categories: p.categories.clone(),
            gallery: gallery_of(p),
            github: github_repo(&p.source).map(|url| GitHub {
                url,
                stars: p.stars,
            }),
            ..Details::default()
        })
    }

    fn fetch(
        &self,
        file: &FileRef,
        folder: &Path,
        progress: &dyn Fn(f32),
    ) -> Result<PathBuf, String> {
        fetch(file, folder, local(), progress)
    }
}

/// Downloads a registry file, checked; `file://` files only from a local index.
fn fetch(
    file: &FileRef,
    folder: &Path,
    local: bool,
    progress: &dyn Fn(f32),
) -> Result<PathBuf, String> {
    match file.url.strip_prefix("file://").filter(|_| local) {
        Some(path) => {
            let path = local_path(path);
            let reader = fs::File::open(path).map_err(|e| format!("{path}: {e}"))?;
            download::save_from_anywhere(file, reader, folder, progress)
        }
        None => download::fetch(file, folder, progress),
    }
}

/// The path of a `file:///` URL: on Windows `/C:/x` is `C:/x`.
fn local_path(path: &str) -> &str {
    match path.strip_prefix('/') {
        Some(rest) if cfg!(windows) && rest.get(1..2) == Some(":") => rest,
        _ => path,
    }
}

fn trust_label(trust: Trust) -> &'static str {
    match trust {
        Trust::Verified => "Verified",
        Trust::Unsigned => "Unsigned",
    }
}

fn relative(kind: Kind, id: &str, file_name: &str) -> Result<String, String> {
    match kind {
        Kind::Mod => Ok(format!("{}/{id}.emod", java::MODS_DIR)),
        Kind::ResourcePack | Kind::Shader => Ok(format!(
            "{}/{file_name}",
            kind.folder().expect("packs have a folder")
        )),
        Kind::Modpack => Err("A modpack installs as its own profile (pack::install).".into()),
        Kind::DataPack => Err("A data pack goes into a world (fetch_datapack).".into()),
    }
}

/// Installs a registry project into the game folder `home`: its newest version for this game that
/// the game playing `home` can load, downloaded into `mod-browser/downloads/`, checked against
/// the index, then moved into place. A mod is `eclipse-mods/<id>.emod`, turned on in
/// `eclipse-mods.txt`.
pub fn install(
    project: &Project,
    home: &Path,
    level: Option<u32>,
    progress: &dyn Fn(&str, f32),
) -> Result<Vec<installed::Entry>, String> {
    install_from(&*index()?, local(), level, project, home, progress)
}

fn install_from(
    index: &Index,
    local: bool,
    level: Option<u32>,
    project: &Project,
    home: &Path,
    progress: &dyn Fn(&str, f32),
) -> Result<Vec<installed::Entry>, String> {
    let p = index
        .project(&project.id)
        .ok_or_else(|| format!("{} isn't in Eclipse's registry.", project.title))?;
    let v = newest(index, p, level)?;
    if needs_port(v) {
        return Err(format!("{} {NEEDS_PORT}.", p.name));
    }
    install_version(index, local, p, v, home, &|_| Ok(()), progress).map(|e| vec![e])
}

/// Downloads the newest version of the registry's data pack `project` into `folder`, checked
/// against the index as every registry file is (size, SHA-256, signature, `pack.mcmeta` and
/// `data/`): the file and its version. [`installed::install_datapack`] puts it in a world.
pub(crate) fn fetch_datapack(
    project: &Project,
    folder: &Path,
    progress: &dyn Fn(f32),
) -> Result<(PathBuf, String), String> {
    fetch_datapack_from(&*index()?, local(), project, folder, progress)
}

fn fetch_datapack_from(
    index: &Index,
    local: bool,
    project: &Project,
    folder: &Path,
    progress: &dyn Fn(f32),
) -> Result<(PathBuf, String), String> {
    let p = index
        .project(&project.id)
        .filter(|p| p.kind == index::Kind::DataPack)
        .ok_or_else(|| format!("{} isn't a data pack in Eclipse's registry.", project.title))?;
    let v = newest(index, p, None)?;
    let file = &version_of(index, p, v, None).files[0];
    let path = fetch(file, folder, local, progress)?;
    index::check_file(index, p, v, &path).inspect_err(|_| {
        let _ = fs::remove_file(&path);
    })?;
    Ok((path, v.version.clone()))
}

/// A project's newest version for this game that wasn't taken down and that a game serving mod
/// API `level` runs (any, when the level isn't known).
fn newest<'a>(
    index: &Index,
    p: &'a index::Project,
    level: Option<u32>,
) -> Result<&'a index::Version, String> {
    let (mut taken_down, mut too_new) = (None, false);
    for v in p.versions.iter().filter(|v| v.game == GAME) {
        if let Some(r) = index.revoked(&p.id, &v.file.sha256) {
            taken_down.get_or_insert(r);
        } else if needs_newer(v, level) {
            too_new = true;
        } else {
            return Ok(v);
        }
    }
    Err(match taken_down {
        _ if too_new => format!("{} {NEEDS_NEWER}: update it first.", p.name),
        Some(r) => format!("{} was taken down: {}", p.name, r.reason),
        None => format!("{} has no version for {GAME} to install.", p.name),
    })
}

const NEEDS_NEWER: &str = "needs a newer Eclipse than the game you play";
const NEEDS_PORT: &str = "needs a native port: Eclipse doesn't run WebAssembly mods any more";

/// Whether version `v` is a WebAssembly mod with code: a `.emod` that may do more than add
/// content. The registry keeps it for the modpacks that need it; a modpack installs it with its
/// code left out (the game doesn't run it), and the browser neither lists nor installs it.
fn needs_port(v: &index::Version) -> bool {
    v.file.name.ends_with(".emod")
        && v.capabilities
            .iter()
            .any(|c| *c != eclipse_mods::Capability::Registries)
}

fn needs_newer(v: &index::Version, level: Option<u32>) -> bool {
    matches!((v.api_level, level), (Some(needed), Some(level)) if needed > level)
}

/// Installs version `v` of the registry's mod, resource pack or shader pack `p` into the game
/// folder `home`, with `guard` asked about the downloaded file once it passed the index's checks
/// and before it replaces anything: its error leaves everything as it was.
fn install_version(
    index: &Index,
    local: bool,
    p: &index::Project,
    v: &index::Version,
    home: &Path,
    guard: &dyn Fn(&Path) -> Result<(), String>,
    progress: &dyn Fn(&str, f32),
) -> Result<installed::Entry, String> {
    let mut list = installed::Installed::open(home)?;
    let project = project_of(p).ok_or_else(|| format!("{} can't be installed here.", p.name))?;
    if let Some(r) = index.revoked(&p.id, &v.file.sha256) {
        return Err(format!("{} was taken down: {}", p.name, r.reason));
    }
    let native = p.kind == index::Kind::NativeMod;
    let relative = match native {
        true => format!("{}/{}.hnmod", crate::native_mods::INSTALLED, p.id),
        false => relative(project.kind, &p.id, &v.file.name)?,
    };
    let ours = |e: &installed::Entry| e.source == SourceId::Eclipse && e.project == p.id;
    let old = list.entries.iter().find(|e| ours(e)).cloned();
    if let Some(old) = old
        .clone()
        .filter(|o| o.version == v.version && o.path(home).is_some_and(|f| f.exists()))
    {
        return Ok(old);
    }
    let target = home.join(&relative);
    let taken_by = list.entries.iter().find(|e| !ours(e) && e.file == relative);
    let folder_mod = home.join(java::MODS_DIR).join(&p.id);
    if let Some(other) = taken_by {
        return Err(format!(
            "{relative} belongs to {}: remove it first.",
            other.title
        ));
    }
    let clash = target.exists() && old.as_ref().is_none_or(|o| o.file != relative);
    if project.kind == Kind::Mod && (clash || (!native && folder_mod.exists())) {
        return Err(format!(
            "A mod with the id {} is already in {}: move it away first.",
            p.id,
            java::MODS_DIR
        ));
    }
    if clash {
        return Err(format!("{relative} is already there: move it away first."));
    }
    let line = format!("{} {}", p.name, v.version);
    let version = version_of(index, p, v, None);
    let file = &version.files[0];
    let staging = home.join(game_folder::BROWSER_FILES).join("downloads");
    let path = fetch(file, &staging, local, &|f| progress(&line, f))?;
    let checked = index::check_file(index, p, v, &path).and_then(|trust| {
        // Only Rust and TOML sources, for the mod the index names: nothing in it runs here.
        match native {
            true => index::native::Bundle::read(&path).and_then(|b| match b.manifest.id == p.id {
                true => Ok(trust),
                false => Err(format!("it is {}, not {}", b.manifest.id, p.id)),
            }),
            false => Ok(trust),
        }
    });
    let trust = checked
        .and_then(|t| guard(&path).map(|()| t))
        .inspect_err(|_| {
            let _ = fs::remove_file(&path);
        })?;
    if let Some(dir) = target.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    fs::rename(&path, &target).map_err(|e| format!("{}: {e}", target.display()))?;
    if let Some(old) = old
        .as_ref()
        .filter(|o| o.file != relative && !stays(native, &o.file))
        .and_then(|o| o.path(home))
    {
        let _ = fs::remove_file(old);
    }
    if project.kind == Kind::Mod {
        java::set_enabled(home, &p.id, true)?;
    }
    let entry = installed::Entry {
        verdict: Some(trust_label(trust).into()),
        skipped: match needs_port(v) {
            true => vec![format!("its code ({NEEDS_PORT})")],
            false => Vec::new(),
        },
        ..installed::Entry::of(&project, &version, relative)
    };
    list.record(entry.clone())?;
    Ok(entry)
}

/// Whether an update to a native version keeps the old file `old`: a mod that turned native
/// keeps its `.emod` until the player builds the game with the new version. The game runs it
/// meanwhile and skips it once the native one is built in; removing the mod removes both.
fn stays(native: bool, old: &str) -> bool {
    native && old.ends_with(".emod")
}

/// The mod id of an installed mod (`eclipse-mods/<id>`, `eclipse-mods/<id>.emod` or
/// `eclipse-native-mods/<id>.hnmod`).
pub fn mod_id(entry: &installed::Entry) -> Option<&str> {
    if let Some(rest) = entry.file.strip_prefix(crate::native_mods::INSTALLED) {
        return rest.strip_prefix('/')?.strip_suffix(".hnmod");
    }
    let rest = entry.file.strip_prefix(java::MODS_DIR)?.strip_prefix('/')?;
    Some(rest.strip_suffix(".emod").unwrap_or(rest))
}

pub fn taken_down<'a>(index: &'a Index, entry: &installed::Entry) -> Option<&'a index::Revoked> {
    (entry.source == SourceId::Eclipse)
        .then(|| index.revoked(&entry.project, entry.hash("SHA-256")?))
        .flatten()
}

/// Turns the registry's files that were taken down off in the game folder `home`, and says
/// which. Their files stay, so the player sees them and can remove them.
pub fn turn_off_revoked(index: &Index, home: &Path) -> Vec<String> {
    let _lock = crate::browser::lock_home(home);
    let mut out = Vec::new();
    for e in installed::Installed::load(home).entries {
        let hash = e.hash("SHA-256").unwrap_or_default();
        let Some(r) = (e.source == SourceId::Eclipse)
            .then(|| index.revoked(&e.project, hash))
            .flatten()
        else {
            continue;
        };
        if let (Kind::Mod, Some(id)) = (e.kind, mod_id(&e)) {
            if java::is_enabled(home, id) && java::set_enabled(home, id, false).is_ok() {
                out.push(format!(
                    "{} was taken down and is off: {}",
                    e.title, r.reason
                ));
            }
        } else if matches!(e.kind, Kind::ResourcePack | Kind::Shader) {
            out.push(format!(
                "{} was taken down: {} Remove it.",
                e.title, r.reason
            ));
        }
    }
    out
}

/// Before the game starts: registry files taken down are turned off, by the index kept last
/// time (no waiting on the network), and the index is fetched again in the background for the
/// next start. Nothing happens when nothing came from the registry.
pub fn before_play(home: &Path) {
    let from_registry = installed::Installed::load(home)
        .entries
        .iter()
        .any(|e| e.source == SourceId::Eclipse);
    if !from_registry {
        return;
    }
    let kept = fs::read_to_string(crate::backend::game_home().join(CACHE)).ok();
    if let Some(index) = kept.and_then(|t| parse(&t).ok()) {
        for line in turn_off_revoked(&index, home) {
            eprintln!("{line}");
        }
    }
    let home = home.to_owned();
    std::thread::spawn(move || {
        if let Ok(index) = index() {
            turn_off_revoked(&index, &home);
        }
    });
}

pub mod auto;
pub mod pack;
#[cfg(test)]
mod tests;
