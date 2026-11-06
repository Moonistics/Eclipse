//! Mods, resource packs, shader packs and data packs for a Java Edition instance: the source's
//! own files, for the instance's Minecraft version and loader, put in its `mods/`,
//! `resourcepacks/`, `shaderpacks/` and a world's `datapacks/` as they are (nothing is ported).
//! The instance's `mod-browser.json` lists all but the data packs, as for Eclipse's folder.
use super::installed::{self, Entry, Installed, Step};
use super::{details, Kind, Project, Query, Source, Support, Version};
use crate::game_folder;
use std::fs;
use std::path::{Path, PathBuf};

/// How deep required dependencies are followed.
const MAX_DEPTH: usize = 4;

const SHADER_PACK_LOADERS: [&str; 3] = ["iris", "optifine", "canvas"];

const SHADER_MOD_JARS: [&str; 3] = ["iris", "oculus", "optifine"];

/// What an instance runs: its Minecraft version and its loader (`fabric`, `quilt`, `neoforge`,
/// `forge`), none for vanilla.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub minecraft: String,
    pub loader: Option<String>,
}

impl Target {
    /// Whether a file made for `loader` runs here ([`crate::loaders::Kind::loads`]).
    pub fn accepts(&self, loader: &str) -> bool {
        match self.loader.as_deref() {
            Some(l) => crate::loaders::Kind::parse(l)
                .map_or(l == loader, |k| k.loads().iter().any(|m| m.id() == loader)),
            None => false,
        }
    }

    /// The folder files of `kind` go in, or why they don't go in this instance.
    pub fn folder(&self, kind: Kind) -> Result<&'static str, String> {
        match kind {
            Kind::Mod if self.loader.is_none() => Err(format!(
                "This instance plays vanilla Minecraft {}: give it a loader (Fabric, Quilt...) \
                 for mods.",
                self.minecraft
            )),
            Kind::Mod => Ok(game_folder::JAVA_MODS),
            Kind::ResourcePack => Ok(game_folder::RESOURCE_PACKS),
            Kind::Modpack => Err("A modpack is imported as its own instance.".into()),
            Kind::DataPack => Err("A data pack goes in a world.".into()),
            Kind::Shader => Ok(game_folder::SHADER_PACKS),
        }
    }

    /// A search of `kind` for what runs here: made for its version and, for mods, its loader.
    pub fn query(&self, text: &str, kind: Kind) -> Query {
        let mut query = Query::new(text, kind);
        query.filters = details::Filters {
            game_version: Some(self.minecraft.clone()),
            loader: match kind {
                Kind::Mod => self.loader.clone(),
                _ => None,
            },
            ..details::Filters::default()
        };
        query
    }

    /// The page's search: the player's categories, side and sort, for this instance's version
    /// and loader (in place of any the player picked).
    pub fn search(&self, text: &str, kind: Kind, picked: &details::Filters) -> Query {
        let mut query = self.query(text, kind);
        query.filters = details::Filters {
            categories: picked.categories.clone(),
            environment: picked.environment,
            sort: picked.sort,
            ..query.filters
        };
        query
    }

    /// Whether this instance takes files of `kind` from the browser as they are: mods, resource
    /// packs, shader packs and data packs.
    pub fn takes(&self, kind: Kind) -> bool {
        matches!(
            kind,
            Kind::Mod | Kind::ResourcePack | Kind::Shader | Kind::DataPack
        )
    }

    pub fn shader_mod(&self) -> Option<(&'static str, &'static str)> {
        match self.loader.as_deref()? {
            "forge" => Some(("oculus", "Oculus")),
            _ => Some(("iris", "Iris")),
        }
    }

    pub fn support(&self, project: &Project) -> Support {
        if project.kind == Kind::Shader && !made_for(&project.loaders, &SHADER_PACK_LOADERS) {
            return Support::No("Not an Iris or OptiFine shader pack".into());
        }
        if project.kind == Kind::Mod && !project.loaders.iter().any(|l| self.accepts(l)) {
            let needed = crate::loaders::Kind::ALL
                .into_iter()
                .find(|k| project.loaders.iter().any(|l| l == k.id()))
                .map_or("a mod loader", |k| k.name());
            return Support::No(format!("Needs {needed}"));
        }
        if !project.game_versions.is_empty() && !project.game_versions.contains(&self.minecraft) {
            return Support::No(format!("Not for Minecraft {}", self.minecraft));
        }
        Support::Works
    }

    /// What it plays, as a search with no results names it: `Minecraft 1.21.1 with fabric`.
    pub fn describe(&self, kind: Kind) -> String {
        match (&self.loader, kind) {
            (Some(l), Kind::Mod) => format!("Minecraft {} with {l}", self.minecraft),
            _ => format!("Minecraft {}", self.minecraft),
        }
    }

    /// The newest version made for this instance, a file for its own loader before one it
    /// only accepts (a Fabric file in a Quilt instance).
    pub fn pick<'a>(&self, kind: Kind, versions: &'a [Version]) -> Option<&'a Version> {
        let mut usable = versions.iter().filter(|v| {
            v.blocked.is_none() && v.file().is_some() && v.game_versions.contains(&self.minecraft)
        });
        if kind == Kind::Shader {
            return usable.find(|v| made_for(&v.loaders, &SHADER_PACK_LOADERS));
        }
        if kind == Kind::DataPack {
            return usable.find(|v| made_for(&v.loaders, &[super::DATAPACK_LOADER]));
        }
        if kind != Kind::Mod {
            return usable.find(|v| v.loaders.iter().any(|l| l == "minecraft"));
        }
        let own = self.loader.as_deref()?;
        usable
            .clone()
            .find(|v| v.loaders.iter().any(|l| l == own))
            .or_else(|| usable.find(|v| v.loaders.iter().any(|l| self.accepts(l))))
    }
}

fn made_for(loaders: &[String], wanted: &[&str]) -> bool {
    loaders.iter().any(|l| wanted.contains(&l.as_str()))
}

pub fn loads_shaders(home: &Path) -> bool {
    fs::read_dir(home.join(game_folder::JAVA_MODS))
        .into_iter()
        .flatten()
        .flatten()
        .any(|f| {
            let name = f.file_name().to_string_lossy().to_lowercase();
            name.ends_with(".jar") && SHADER_MOD_JARS.iter().any(|m| name.starts_with(m))
        })
}

/// What installing `project` into the instance takes, its required dependencies first, each
/// checked against the instance before any byte is downloaded.
pub fn plan(source: &dyn Source, target: &Target, project: &Project) -> Result<Vec<Step>, String> {
    let mut steps = Vec::new();
    add(source, target, project.clone(), false, 0, &mut steps)?;
    Ok(steps)
}

fn add(
    source: &dyn Source,
    target: &Target,
    project: Project,
    dependency: bool,
    depth: usize,
    steps: &mut Vec<Step>,
) -> Result<(), String> {
    if steps.iter().any(|s| s.project.id == project.id) {
        return Ok(());
    }
    if depth > MAX_DEPTH {
        return Err(format!("{}'s dependencies go too deep.", project.title));
    }
    target.folder(project.kind)?;
    let versions = source.versions(&project)?;
    let Some(version) = target.pick(project.kind, &versions).cloned() else {
        return Err(match versions.iter().find_map(|v| v.blocked.clone()) {
            Some(why) => format!("{} can't be installed from here: {why}", project.title),
            None => format!(
                "{} has no version for Minecraft {}{}.",
                project.title,
                target.minecraft,
                match (project.kind, &target.loader) {
                    (Kind::Mod, Some(l)) => format!(" with {l}"),
                    _ => String::new(),
                }
            ),
        });
    };
    for id in &version.required {
        let needed = source
            .project(id)
            .map_err(|e| format!("{} needs {id}: {e}", project.title))?;
        add(source, target, needed, true, depth + 1, steps)?;
    }
    steps.push(Step {
        project,
        version,
        dependency,
    });
    Ok(())
}

/// Installs `project` (and what it needs) into the instance folder `home`; one already there is
/// updated to the version chosen.
pub fn install(
    source: &dyn Source,
    target: &Target,
    project: &Project,
    home: &Path,
    progress: &dyn Fn(&str, f32),
) -> Result<Vec<Entry>, String> {
    let _lock = super::lock_home(home);
    let steps = plan(source, target, project)?;
    let folder = |kind| target.folder(kind).expect("plan checks every kind");
    let was = Installed::load(home).entries;
    let done = installed::put(source, steps, home, &folder, progress)?;
    // A mod the player turned off stays off through an update, and its old file goes.
    for e in &done {
        let Some(old) = was
            .iter()
            .find(|o| o.source == e.source && o.project == e.project && !is_on(home, o))
        else {
            continue;
        };
        if let Some(file) = old.off(home).filter(|_| old.file != e.file) {
            let _ = fs::remove_file(file);
        }
        set_on(home, e, false)?;
    }
    Ok(done)
}

pub fn install_datapack(
    source: &dyn Source,
    target: &Target,
    project: &Project,
    world: &Path,
    progress: &dyn Fn(f32),
) -> Result<(crate::datapacks::DataPack, String), String> {
    let versions = source.versions(project)?;
    let version = target.pick(Kind::DataPack, &versions).ok_or_else(|| {
        format!(
            "{} has no data pack for Minecraft {}.",
            project.title, target.minecraft
        )
    })?;
    let file = version.file().expect("pick keeps versions with a file");
    let folder = std::env::temp_dir().join(format!(
        "eclipse-launcher-datapack-{}-{}",
        std::process::id(),
        project.id
    ));
    let added = source
        .fetch(file, &folder, progress)
        .and_then(|zip| crate::datapacks::add(world, &zip));
    let _ = fs::remove_dir_all(&folder);
    Ok((added?, version.number.clone()))
}

/// Whether the game loads `entry`: everything but a mod turned off.
pub fn is_on(home: &Path, entry: &Entry) -> bool {
    entry.kind != Kind::Mod || !entry.off(home).is_some_and(|o| o.is_file())
}

/// Turns the mod `entry` on or off by renaming its file; it stays installed.
pub fn set_on(home: &Path, entry: &Entry, on: bool) -> Result<(), String> {
    if entry.kind != Kind::Mod {
        return Err(format!(
            "{} isn't a mod: only mods turn on and off.",
            entry.title
        ));
    }
    set_file_on(home, &entry.file, on)
}

pub struct Jar {
    pub file: String,
    pub path: PathBuf,
    pub on: bool,
}

pub fn jars(home: &Path) -> Vec<Jar> {
    let Ok(dir) = fs::read_dir(home.join(game_folder::JAVA_MODS)) else {
        return Vec::new();
    };
    dir.flatten()
        .filter_map(|f| {
            let name = f.file_name().to_string_lossy().into_owned();
            let jar = name.strip_suffix(installed::OFF).unwrap_or(&name);
            (jar.ends_with(".jar") && f.path().is_file()).then(|| Jar {
                file: format!("{}/{jar}", game_folder::JAVA_MODS),
                on: jar.len() == name.len(),
                path: f.path(),
            })
        })
        .collect()
}

pub fn set_file_on(home: &Path, file: &str, on: bool) -> Result<(), String> {
    let _lock = super::lock_home(home);
    if !installed::in_folders(file) {
        return Err(format!("{file:?} isn't a plain file name."));
    }
    let (jar, off) = (
        home.join(file),
        home.join(format!("{file}{}", installed::OFF)),
    );
    let (from, to) = if on { (off, jar) } else { (jar, off) };
    if !from.exists() && to.is_file() {
        return Ok(());
    }
    fs::rename(&from, &to).map_err(|e| format!("{}: {e}", from.display()))
}

/// Removes what the browser installed in the instance for `name` (a project id or slug).
pub fn remove(home: &Path, name: &str) -> Result<Entry, String> {
    let mut installed = Installed::open(home)?;
    let entry = installed
        .lookup(name)
        .cloned()
        .ok_or_else(|| format!("{name} wasn't installed by the browser."))?;
    for path in [entry.path(home), entry.off(home)].into_iter().flatten() {
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("{}: {e}", path.display())),
        }
    }
    installed.forget(&entry)?;
    Ok(entry)
}

#[cfg(test)]
mod tests;
