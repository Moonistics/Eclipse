use super::{json, progress};
use clap::Subcommand;
use eclipse_launcher::backend;
use eclipse_launcher::github_mods;
use eclipse_launcher::instances;
use eclipse_launcher::native_status;
use std::path::Path;

#[derive(Subcommand)]
pub(super) enum ModsCmd {
    /// What the browser installed in an instance (the default)
    List {
        /// The instance (default: the selected one)
        #[arg(long)]
        instance: Option<String>,
    },
    /// Search for packs Eclipse loads, or for what a Java instance runs (made for its
    /// Minecraft version and loader)
    Search {
        text: Vec<String>,
        #[arg(long, value_enum, default_value_t = KindArg::Resource)]
        kind: KindArg,
        /// Also what was made for other versions or runs only partly, each marked
        #[arg(long)]
        everything: bool,
        #[arg(long, value_enum, default_value_t = SourceArg::Modrinth)]
        source: SourceArg,
        /// The instance (default: the selected one)
        #[arg(long)]
        instance: Option<String>,
    },
    /// Say how well a Java mod runs in Eclipse: its jar is downloaded (checksum checked),
    /// carried over and read; the answer is kept
    Check {
        project: String,
        /// Allow a version made for another Minecraft version (1.21 and later)
        #[arg(long)]
        everything: bool,
        #[arg(long, value_enum, default_value_t = SourceArg::Modrinth)]
        source: SourceArg,
    },
    /// Install a pack and what it needs into an instance, by its id or slug (checksums
    /// checked); a Java mod is ported into eclipse-mods and turned on (a Java instance gets the
    /// jar made for its version and loader in mods/); a modpack is imported as an instance of
    /// its own; a data pack goes into the world `--world` names
    Install {
        project: String,
        /// Install the project's data pack into this world of the instance (its name or folder)
        #[arg(long)]
        world: Option<String>,
        /// The instance (default: the selected one)
        #[arg(long)]
        instance: Option<String>,
        /// Allow a version made for another Minecraft version
        #[arg(long)]
        everything: bool,
        #[arg(long, value_enum, default_value_t = SourceArg::Modrinth)]
        source: SourceArg,
    },
    /// Update what the browser installed (or one pack)
    Update {
        project: Option<String>,
        /// Only say what is newer
        #[arg(long)]
        check: bool,
        /// The instance (default: the selected one)
        #[arg(long)]
        instance: Option<String>,
    },
    /// Install a native mod from a GitHub link (docs/native-mods.md, "Install from GitHub"):
    /// https://github.com/OWNER/REPO (its latest release, else its default branch),
    /// .../releases/tag/TAG, .../tree/REF or .../commit/SHA. Resolved to an exact release file or
    /// commit, checked like a registry mod. NOT reviewed by the Eclipse registry
    AddGithub {
        url: String,
        /// The mod to install when the link holds several
        #[arg(long = "mod")]
        mod_id: Option<String>,
        /// Install every mod the link holds
        #[arg(long)]
        all: bool,
        /// The instance (default: the selected one)
        #[arg(long)]
        instance: Option<String>,
        /// You read the mod's source (the link printed) and trust it with full access to this computer
        #[arg(long)]
        trust_native_code: bool,
    },
    /// Look for newer versions of the native mods installed from GitHub (the latest release, or the
    /// default branch's head for a mod installed from a commit), and install them
    UpdateGithub {
        /// One mod (default: all of them)
        id: Option<String>,
        /// Only say what is newer
        #[arg(long)]
        check: bool,
        /// The instance (default: the selected one)
        #[arg(long)]
        instance: Option<String>,
        /// You read the new source and trust it with full access to this computer
        #[arg(long)]
        trust_native_code: bool,
    },
    /// Remove a pack the browser installed
    Remove {
        project: String,
        /// The instance (default: the selected one)
        #[arg(long)]
        instance: Option<String>,
    },
    /// Turn on a mod the browser installed (eclipse-mods.txt, as the game's Options > Mods does;
    /// in a Java instance, its file loses `.disabled`)
    Enable {
        project: String,
        /// The instance (default: the selected one)
        #[arg(long)]
        instance: Option<String>,
    },
    /// Turn off a mod the browser installed; it stays installed (in a Java instance, as
    /// `<file>.disabled`)
    Disable {
        project: String,
        /// The instance (default: the selected one)
        #[arg(long)]
        instance: Option<String>,
    },
}

#[derive(Clone, Copy, clap::ValueEnum)]
pub(super) enum KindArg {
    Mod,
    Resource,
    Modpack,
    /// Eclipse's own shader packs (Eclipse's registry only)
    Shader,
    /// Data packs (Modrinth): install one into a world with `mods install <pack> --world <name>`
    Datapack,
}

#[derive(Clone, Copy, clap::ValueEnum)]
pub(super) enum SourceArg {
    Modrinth,
    /// CurseForge support is coming soon
    Curseforge,
    /// Eclipse's own registry: mods, modpacks and resource packs made for Eclipse, reviewed
    Eclipse,
}

impl SourceArg {
    fn get(self) -> Result<Box<dyn eclipse_launcher::browser::Source>, String> {
        use eclipse_launcher::browser::{self, SourceId};
        let id = match self {
            SourceArg::Modrinth => SourceId::Modrinth,
            SourceArg::Curseforge => SourceId::CurseForge,
            SourceArg::Eclipse => SourceId::Eclipse,
        };
        browser::source(id).ok_or_else(|| browser::CURSEFORGE_SOON.into())
    }
}

pub(super) fn mods(action: ModsCmd, as_json: bool) -> Result<(), String> {
    use eclipse_launcher::browser::{eclipse, installed, java_instance, Kind, Query, SourceId};
    use eclipse_launcher::modpacks;
    // What the browser installs goes into an instance's game folder; Java mods' checks are kept
    // in the default one, shared by all.
    let instance = match &action {
        ModsCmd::List { instance }
        | ModsCmd::Search { instance, .. }
        | ModsCmd::Install { instance, .. }
        | ModsCmd::Update { instance, .. }
        | ModsCmd::Remove { instance, .. }
        | ModsCmd::AddGithub { instance, .. }
        | ModsCmd::UpdateGithub { instance, .. }
        | ModsCmd::Enable { instance, .. }
        | ModsCmd::Disable { instance, .. } => match instance {
            Some(name) => Some(instances::find(name)?),
            None => Some(instances::selected()),
        },
        ModsCmd::Check { .. } => None,
    };
    // A Java instance gets the source's own files, for its version and loader.
    let java = instance
        .as_ref()
        .and_then(|i| i.java.as_ref())
        .map(|j| j.target());
    let game = installed::Game::of(java.clone());
    let home = match (&action, &instance) {
        (ModsCmd::Search { .. }, _) | (_, None) => backend::game_home(),
        (_, Some(i)) => i.home.clone(),
    };
    let entry_json = |e: &installed::Entry| serde_json::to_value(e).unwrap_or_default();
    let line = super::steps();
    let from_registry = installed::Installed::load(&home)
        .entries
        .iter()
        .any(|e| e.source == SourceId::Eclipse);
    if from_registry {
        if let Ok(index) = eclipse::index() {
            for said in eclipse::turn_off_revoked(&index, &home) {
                eprintln!("{said}");
            }
        }
    }
    let on = matches!(action, ModsCmd::Enable { .. });
    match action {
        ModsCmd::List { .. } => {
            let list = installed::Installed::load(&home).entries;
            if as_json {
                json(list.iter().map(entry_json).collect());
            } else if list.is_empty() && github_mods::origins(&home).is_empty() {
                println!(
                    "Nothing installed from the browser yet: eclipse-launcher mods search <text>"
                );
            }
            for o in eclipse_launcher::github_mods::origins(&home)
                .iter()
                .filter(|_| !as_json && java.is_none())
            {
                println!(
                    "{} {}  (native mod, from GitHub: {})\n  {}",
                    o.id,
                    o.version,
                    o.label(),
                    o.source_url()
                );
            }
            for e in list.iter().filter(|_| !as_json) {
                let off = java.is_some() && !java_instance::is_on(&home, e);
                println!(
                    "{}{}  {}  ({}, {})\n  {}  {}",
                    e.title,
                    if off { " (off)" } else { "" },
                    e.number,
                    e.source.label(),
                    e.slug,
                    e.file,
                    e.checksum
                );
            }
        }
        ModsCmd::Search {
            text,
            kind,
            everything,
            source,
            ..
        } => {
            let kind = match kind {
                KindArg::Resource => Kind::ResourcePack,
                KindArg::Modpack => Kind::Modpack,
                KindArg::Mod => Kind::Mod,
                KindArg::Shader => Kind::Shader,
                KindArg::Datapack => Kind::DataPack,
            };
            if let Some(target) = &java {
                let found = source.get()?.search(&target.query(&text.join(" "), kind))?;
                if as_json {
                    json(found.iter().map(|p| serde_json::json!({
                        "id": p.id, "slug": p.slug, "title": p.title, "summary": p.summary,
                        "author": p.author, "downloads": p.downloads, "page": p.page,
                        "license": p.license,
                    })).collect());
                }
                for p in found.iter().filter(|_| !as_json) {
                    println!(
                        "{}  ({})  {} downloads\n  {}",
                        p.title, p.slug, p.downloads, p.summary
                    );
                }
                return Ok(());
            }
            let query = Query {
                everything,
                ..Query::new(&text.join(" "), kind)
            };
            let found = source.get()?.search(&query)?;
            let datapack = kind == Kind::DataPack;
            if as_json {
                json(found.iter().map(|p| {
                    let support = p.support();
                    serde_json::json!({
                        "id": p.id, "slug": p.slug, "title": p.title, "summary": p.summary,
                        "author": p.author, "downloads": p.downloads, "page": p.page,
                        "license": p.license, "support": support.label(), "reason": support.reason(),
                        "about": if p.source == SourceId::Eclipse { eclipse::trust_and_capabilities(&p.id, instance.as_ref()) } else { Vec::new() },
                    })
                }).collect());
            } else {
                if datapack && !found.is_empty() {
                    println!("{}\n", eclipse_launcher::browser::DATAPACK_SUPPORT);
                }
                for p in &found {
                    let support = p.support();
                    if p.source == SourceId::Eclipse {
                        println!("{}  ({})  {}\n  {}", p.title, p.slug, p.author, p.summary);
                        for line in eclipse::trust_and_capabilities(&p.id, instance.as_ref()) {
                            println!("  {line}");
                        }
                        continue;
                    }
                    let unchecked = p.kind == Kind::Mod && support.installable();
                    println!(
                        "{}  ({})  {} · {} downloads\n  {}",
                        p.title,
                        p.slug,
                        if unchecked {
                            "Not checked"
                        } else {
                            support.label()
                        },
                        p.downloads,
                        p.summary
                    );
                    if unchecked {
                        println!("  eclipse-launcher mods check {}", p.slug);
                    } else if datapack {
                        println!("  eclipse-launcher mods install {} --world <world>", p.slug);
                    } else if let Some(why) = support.reason() {
                        println!("  {why}");
                    }
                }
            }
        }
        ModsCmd::Check {
            project,
            everything,
            source,
        } => {
            let source = source.get()?;
            let p = source.project(&project)?;
            if p.kind != Kind::Mod {
                return Err(format!("{} isn't a Java mod.", p.title));
            }
            let (version, c) = eclipse_launcher::browser::java::check_project(
                &home,
                &*source,
                &p,
                everything,
                &progress(p.title.clone()),
            )?;
            if as_json {
                let mut v = serde_json::to_value(&c).unwrap_or_default();
                v["version"] = version.number.clone().into();
                v["label"] = c.verdict.label().into();
                println!("{v}");
                return Ok(());
            }
            println!(
                "{} {} ({} {}): {}. {}",
                p.title,
                version.number,
                c.loader,
                c.mod_id,
                c.verdict.label(),
                c.summary
            );
            for line in &c.ports {
                println!("  ports: {line}");
            }
            for line in &c.skipped {
                println!("  doesn't: {line}");
            }
        }
        ModsCmd::Install {
            project,
            everything,
            source,
            world,
            ..
        } => {
            let source = source.get()?;
            let p = source.project(&project)?;
            let world = world
                .map(|w| eclipse_launcher::worlds::find(&home, &w))
                .transpose()?;
            if p.kind == Kind::DataPack && world.is_none() {
                return Err(format!(
                    "{} is a data pack: say which world with --world.",
                    p.title
                ));
            }
            let to = installed::Destination {
                home: &home,
                java: java.as_ref(),
                world: world.as_ref().map(|w| w.path.as_path()),
                everything,
            };
            let log = |line: String| eprintln!("{line}");
            match installed::install_into(&*source, &p, &to, &line, &log)? {
                installed::Done::DataPack {
                    pack,
                    number,
                    coverage,
                } => {
                    let world = world.expect("a data pack went into the world given");
                    if as_json {
                        json(serde_json::json!({
                            "world": world.folder, "file": pack.file, "version": number,
                            "runs": coverage.as_ref().map(|c| &c.read),
                            "not_read": coverage.as_ref().map(|c| &c.unread),
                        }));
                    } else {
                        println!(
                            "{} {} is in {}'s data packs as {}: the game turns it on when the \
                             world opens.{}",
                            p.title,
                            number,
                            world.name,
                            pack.file,
                            coverage.map_or(String::new(), |c| format!(" {}", c.summary()))
                        );
                    }
                }
                installed::Done::Pack(pack) => {
                    if as_json {
                        json(
                            serde_json::json!({"pack": pack.id, "version": pack.version, "mods": pack.mods, "home": eclipse::pack::home(&pack.work)}),
                        );
                    } else {
                        println!(
                            "{} {} is installed as its own profile, with {}.",
                            pack.name,
                            pack.version,
                            pack.mods.join(", ")
                        );
                        println!("Play it: eclipse-launcher modpack play {}", pack.id);
                    }
                }
                installed::Done::Port(pack) => {
                    println!("{} ({})", modpacks::outcome(&pack), pack.summary());
                    println!("Play it: eclipse-launcher modpack play {}", pack.slug);
                }
                installed::Done::Files(done) => {
                    // The first native mod: the window's trust notice, once.
                    let mut settings = backend::Settings::load();
                    let native = done.iter().any(|e| native_status::is_native_file(&e.file));
                    if native && !settings.native_mods_noticed {
                        eprintln!("{}", native_status::NOTICE);
                        settings.native_mods_noticed = true;
                        settings.save()?;
                    }
                    if as_json {
                        json(done.iter().map(entry_json).collect());
                    }
                    for e in done.iter().filter(|_| !as_json) {
                        println!("{} {}: {} ({})", e.title, e.number, e.file, e.checksum);
                        for s in &e.skipped {
                            println!("  not ported: {s}");
                        }
                    }
                }
            }
        }
        ModsCmd::Update { project, check, .. } => {
            let list = installed::Installed::load(&home).entries;
            let chosen: Vec<_> = list
                .iter()
                .filter(|e| {
                    project
                        .as_deref()
                        .is_none_or(|n| e.project == n || e.slug == n)
                })
                .collect();
            if chosen.is_empty() {
                return Err(match project {
                    Some(n) => format!("{n} wasn't installed by the browser."),
                    None => "Nothing installed from the browser yet.".into(),
                });
            }
            for e in chosen {
                let Some(source) = eclipse_launcher::browser::source(e.source) else {
                    println!("{}: {} isn't available here.", e.title, e.source.label());
                    continue;
                };
                let newer = game.update_for(&*source, e)?;
                match newer {
                    None => println!("{} {}: up to date.", e.title, e.number),
                    Some(v) if check => {
                        println!("{} {}: {} is available.", e.title, e.number, v.number)
                    }
                    Some(_) => {
                        let p = source.project(&e.project)?;
                        let done = game.install(&*source, &p, true, &home, &line)?;
                        for done in done {
                            println!("{} {}: {}", done.title, done.number, done.file);
                        }
                    }
                }
            }
        }
        ModsCmd::AddGithub {
            url,
            mod_id,
            all,
            trust_native_code,
            ..
        } => {
            let link = github_mods::parse(&url)?;
            let found = github_mods::resolve(&link, &github_mods::api)?;
            github_native(
                &home,
                java.is_some(),
                &found,
                mod_id.as_deref(),
                all,
                trust_native_code,
            )?;
        }
        ModsCmd::UpdateGithub {
            id,
            check,
            trust_native_code,
            ..
        } => {
            if java.is_some() {
                return Err("Native mods from GitHub are for Eclipse instances.".into());
            }
            let mut origins = github_mods::origins(&home);
            origins.retain(|o| id.as_deref().is_none_or(|i| o.id == i));
            if origins.is_empty() {
                return Err(match id {
                    Some(i) => format!("{i} wasn't installed from GitHub."),
                    None => "No native mod was installed from GitHub: eclipse-launcher mods add-github <url>".into(),
                });
            }
            for o in origins {
                let latest = github_mods::resolve(&o.follows(), &github_mods::api)?;
                if !github_mods::is_update(&o, &latest) {
                    println!("{} {}: up to date ({}).", o.id, o.version, o.label());
                    continue;
                }
                println!(
                    "{}: {} is available (now {}).",
                    o.id,
                    latest.label(),
                    o.label()
                );
                if !check {
                    github_native(&home, false, &latest, Some(&o.id), false, trust_native_code)?;
                }
            }
        }
        ModsCmd::Remove { project, .. }
            if java.is_none() && github_mods::origin(&home, &project).is_some() =>
        {
            github_mods::remove(&home, &project)?;
            println!("Removed {project} (installed from GitHub).");
        }
        ModsCmd::Remove { project, .. } => {
            let e = game.remove(&home, &project, &eclipse_launcher::desktop_seams::to_trash)?;
            println!("Removed {} ({}).", e.title, e.file);
        }
        ModsCmd::Enable { project, .. } | ModsCmd::Disable { project, .. } => {
            let e = installed::Installed::load(&home)
                .lookup(&project)
                .cloned()
                .ok_or_else(|| format!("{project} wasn't installed by the browser."))?;
            let index = (on && e.source == SourceId::Eclipse)
                .then(eclipse::index)
                .and_then(Result::ok);
            game.set_on(&home, &e, on, index.as_deref())?;
            println!("{} is {}.", e.title, if on { "on" } else { "off" });
        }
    }
    Ok(())
}

/// Installs the native mods of `found` (a GitHub link, resolved) into the game folder `home`: the
/// warning and the exact source link first, and nothing downloaded without `trust`.
fn github_native(
    home: &Path,
    java: bool,
    found: &github_mods::Resolved,
    only: Option<&str>,
    all: bool,
    trust: bool,
) -> Result<(), String> {
    if java {
        return Err("Native mods from GitHub are for Eclipse instances.".into());
    }
    println!("{}", found.label());
    println!("Source, to read: {}", found.source_url());
    eprintln!("{}", github_mods::WARNING);
    if !trust {
        return Err(format!(
            "Read the source at {}, then pass --trust-native-code.",
            found.source_url()
        ));
    }
    let mods = github_mods::fetch(found)?;
    let chosen = github_mods::choose(&mods, only, all);
    let done = chosen.and_then(|chosen| {
        chosen
            .into_iter()
            .map(|p| github_mods::install(home, found, p))
            .collect::<Result<Vec<_>, _>>()
    });
    github_mods::discard(found);
    let done = done?;
    // The window's trust notice, once. Nothing is turned on: native mods are always on.
    let mut settings = backend::Settings::load();
    if !settings.native_mods_noticed {
        eprintln!("{}", native_status::NOTICE);
        settings.native_mods_noticed = true;
        settings.save()?;
    }
    for o in done {
        println!(
            "Installed {} {} from GitHub ({}), sha256 {}.",
            o.id,
            o.version,
            o.label(),
            o.sha256
        );
    }
    println!("Build it next: eclipse-launcher native-build --trust-native-code");
    println!(
        "Developing a mod? eclipse-launcher settings native-profile dev (or native-build --profile dev): a mod edit rebuilds about 6× faster, while game logic runs up to about twice as slow. Use it while developing, not for playing."
    );
    Ok(())
}
