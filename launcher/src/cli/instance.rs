use super::json;
use super::settings::on_off;
use clap::Subcommand;
use eclipse_launcher::backend;
use eclipse_launcher::instances;
use eclipse_launcher::loaders;
use eclipse_launcher::other_launchers;
use std::path::{Path, PathBuf};

#[derive(Subcommand)]
pub(super) enum InstanceCmd {
    /// The instances, each with its game folder (* = the one Play starts)
    List,
    /// Make an instance: empty but for your options and name, or a copy of another one's
    /// worlds, options and packs
    Create {
        name: String,
        /// The instance to copy (its name or id)
        #[arg(long)]
        copy: Option<String>,
        /// Make a Minecraft: Java Edition instance of this version (1.21.4, latest or
        /// latest-snapshot) instead of a Eclipse one
        #[arg(long, conflicts_with = "copy")]
        minecraft: Option<String>,
        /// With this mod loader: fabric, quilt, forge or neoforge
        #[arg(long, requires = "minecraft")]
        loader: Option<String>,
        /// The loader's version (default: its newest stable one)
        #[arg(long, requires = "loader")]
        loader_version: Option<String>,
    },
    /// The versions of Minecraft: Java Edition a Java instance can play, the newest first
    Versions {
        /// Snapshots too
        #[arg(long)]
        snapshots: bool,
        /// Old betas and alphas too
        #[arg(long)]
        old: bool,
    },
    /// A Java instance's Java: its own Java home or the automatic pick, memory and JVM arguments
    Java {
        instance: String,
        /// A Java home to run it on
        #[arg(long, conflicts_with = "auto")]
        home: Option<PathBuf>,
        /// Go back to the Java the version needs, picked or downloaded by the launcher
        #[arg(long)]
        auto: bool,
        /// The most memory the game may use, in MiB
        #[arg(long)]
        memory: Option<u32>,
        /// Extra JVM arguments (one string; "" clears them)
        #[arg(long, allow_hyphen_values = true)]
        args: Option<String>,
        /// The game window's size, as 1280x720; "default" goes back to the game's own
        #[arg(long)]
        window: Option<String>,
    },
    /// A Java instance's mod loader: fabric, quilt, forge or neoforge, or none. --list shows its versions
    Loader {
        instance: String,
        loader: String,
        /// The loader's version (default: its newest stable one)
        #[arg(long)]
        version: Option<String>,
        /// List the loader's versions for the instance's Minecraft instead
        #[arg(long, conflicts_with = "version")]
        list: bool,
    },
    /// Rename an instance (its folder stays)
    Rename { instance: String, name: String },
    /// Delete an instance: its folder (worlds, options, packs) goes to the Trash, where you can
    /// get it back. Eclipse's own instance stays
    Delete { instance: String },
    /// Set an instance's icon: a block or item name from your prepared files (grass_block),
    /// a colour (#3fa7d6), or none
    Icon { instance: String, icon: String },
    /// Put an instance in a sidebar group, or take it out with no group
    Group {
        instance: String,
        group: Option<String>,
    },
    /// Make Play start this instance
    Select { instance: String },
    /// Keep an instance on one installed Eclipse version, or "latest" to follow updates again
    Version { instance: String, version: String },
    /// Export an instance (or a port) to share: its settings, packs, Eclipse mods and the worlds
    /// named, as a .mrpack (resource packs from Modrinth listed by download) or a zip. Never
    /// logs, screenshots or account data; worlds go without their per-player files
    Export {
        /// The instance (default: the selected one)
        instance: Option<String>,
        #[arg(long, value_enum, default_value_t = ExportFormat::Mrpack)]
        format: ExportFormat,
        /// A world to put in it (its name or folder); repeat for more
        #[arg(long)]
        world: Vec<String>,
        /// Where to write it (default: "<name>.mrpack" in the current folder)
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Show or set what runs around an instance's game: a shell command before it (the game
    /// starts only if it succeeds), a wrapper it runs through (`nice -n 5`), environment
    /// variables ("NAME=value NAME2=value") and a shell command after it exits. An empty value
    /// clears one. The game and the commands also get INST_NAME, INST_ID, INST_DIR,
    /// INST_MC_DIR and, for a Java instance, INST_JAVA. On Linux, also GameMode, MangoHud and
    /// the dedicated graphics card, when the computer has them
    Launch {
        instance: String,
        #[arg(long)]
        pre_launch: Option<String>,
        #[arg(long)]
        wrapper: Option<String>,
        #[arg(long)]
        post_exit: Option<String>,
        #[arg(long)]
        env: Option<String>,
        /// Linux: run it through GameMode (gamemoderun), on or off
        #[arg(long)]
        gamemode: Option<String>,
        /// Linux: MangoHud's overlay, on or off
        #[arg(long)]
        mangohud: Option<String>,
        /// Linux: the dedicated graphics card (DRI_PRIME, or NVIDIA's offload), on or off
        #[arg(long)]
        dedicated_gpu: Option<String>,
    },
    /// Put a shortcut on the desktop that plays an instance (`play --instance`): a .desktop
    /// file (also in the applications menu) on Linux, an app on macOS, a .lnk on Windows
    Shortcut {
        /// The instance (default: the selected one)
        instance: Option<String>,
        /// Move the instance's shortcuts to the trash instead
        #[arg(long)]
        remove: bool,
    },
    /// Import what a file is, as dropping it on the window does: a shared instance (a .mrpack
    /// without mod loaders, such as `instance export` writes, or a game folder zip) becomes a
    /// new instance; a Java modpack goes to the Modpacks import; a resource or shader pack into
    /// the instance (default: the selected one)
    Import {
        file: PathBuf,
        /// The instance a pack goes into
        #[arg(long)]
        instance: Option<String>,
        /// A .mrpack becomes a Minecraft: Java Edition instance with its Minecraft version, mod
        /// loader, mods and overrides, instead of a Eclipse instance or a Modpacks import
        #[arg(long)]
        java: bool,
    },
    /// List the instances other launchers keep (Prism Launcher, MultiMC, the official
    /// launcher, the CurseForge app, the Modrinth App), read without changing anything there
    Others {
        /// Look in this folder instead: a launcher's folder, its instances/ or one instance
        #[arg(long)]
        folder: Option<PathBuf>,
    },
    /// Copy another launcher's instance into a new Java instance: its Minecraft version and its
    /// game folder (worlds, mods, packs, options); the other launcher's copy stays as it is
    CopyFrom {
        /// Its name, as `instance others` lists it
        name: String,
        /// Look in this folder instead: a launcher's folder, its instances/ or one instance
        #[arg(long)]
        folder: Option<PathBuf>,
        /// The new instance's name (default: the same)
        #[arg(long = "as")]
        new_name: Option<String>,
        /// The Minecraft version it plays, when its launcher doesn't say (the CurseForge app,
        /// the Modrinth App)
        #[arg(long)]
        minecraft: Option<String>,
    },
}

#[derive(Clone, Copy, clap::ValueEnum)]
pub(super) enum ExportFormat {
    /// Modrinth's modpack format
    Mrpack,
    /// The game folder's layout, zipped
    Zip,
}

pub(super) fn instance(
    action: InstanceCmd,
    as_json: bool,
    settings: &backend::Settings,
) -> Result<(), String> {
    use instances::Icon;
    let value = |i: &instances::Instance, selected: bool| {
        serde_json::json!({
            "id": i.id, "name": i.name, "kind": format!("{:?}", i.kind).to_lowercase(),
            "home": i.home, "icon": i.icon, "group": i.group, "selected": selected,
            "version": i.version, "java": i.java, "played_secs": i.played.as_secs(),
            "last_played_ms": i.last_played.and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as u64),
        })
    };
    let line = |i: &instances::Instance, selected: bool| {
        let mark = if selected { "*" } else { " " };
        let group = i
            .group
            .as_ref()
            .map(|g| format!("  [{g}]"))
            .unwrap_or_default();
        let pinned = match (&i.java, &i.version) {
            (Some(java), _) => match &java.loader {
                Some(l) => format!(
                    "  Minecraft {} (Java Edition) with {} {}",
                    java.minecraft,
                    l.kind.name(),
                    l.version
                ),
                None => format!("  Minecraft {} (Java Edition)", java.minecraft),
            },
            (None, Some(v)) => format!("  {v}"),
            (None, None) => String::new(),
        };
        let played = match i.last_played {
            Some(_) => format!("  {} played", instances::play_time(i.played)),
            None => String::new(),
        };
        println!(
            "{mark} {} ({}){pinned}{group}{played}\n    {}",
            i.name,
            i.id,
            i.home.display()
        );
    };
    let changed = |i: instances::Instance| {
        let selected = i.id == instances::selected(settings).id;
        match as_json {
            true => json(value(&i, selected)),
            false => line(&i, selected),
        }
    };
    match action {
        InstanceCmd::List => {
            let selected = instances::selected(settings).id;
            let all = instances::list();
            if as_json {
                json(all.iter().map(|i| value(i, i.id == selected)).collect());
            } else {
                all.iter().for_each(|i| line(i, i.id == selected));
            }
        }
        InstanceCmd::Create {
            name,
            copy,
            minecraft: None,
            ..
        } => changed(instances::create(&name, copy.as_deref())?),
        InstanceCmd::Create {
            name,
            minecraft: Some(version),
            loader,
            loader_version,
            ..
        } => {
            let versions = eclipse_launcher::minecraft_versions::load()?;
            let id = &versions.find(&version)?.id;
            // The loader first: a version it doesn't have makes no instance.
            let loader = loader
                .map(|kind| loaders::choose(&kind, id, loader_version.as_deref()))
                .transpose()?;
            changed(instances::create_java(&name, id, loader)?)
        }
        InstanceCmd::Loader {
            instance,
            loader,
            version,
            list,
        } => {
            let Some(java) = instances::find(&instance)?.java else {
                return Err(format!(
                    "{instance} is a Eclipse instance: mod loaders are for Java instances."
                ));
            };
            if list {
                let kind = loaders::Kind::parse(&loader)?;
                let listed = loaders::versions(kind, &java.minecraft, &backend::get_bytes)?;
                if as_json {
                    json(serde_json::json!(listed));
                } else {
                    for l in listed {
                        println!("{}{}", l.version, if l.stable { "" } else { "  (beta)" });
                    }
                }
                return Ok(());
            }
            let loader = match loader.trim() {
                "none" => None,
                kind => Some(loaders::choose(kind, &java.minecraft, version.as_deref())?),
            };
            changed(instances::edit(&instance, |m| {
                m.java.get_or_insert_with(Default::default).loader = loader
            })?)
        }
        InstanceCmd::Versions { snapshots, old } => {
            let versions = eclipse_launcher::minecraft_versions::load()?;
            let shown: Vec<_> = versions.of(snapshots, old).collect();
            if as_json {
                json(serde_json::json!(shown));
            } else {
                for v in shown {
                    let date = v.released.get(..10).unwrap_or("");
                    println!("{}  {date}  {:?}", v.id, v.kind);
                }
            }
        }
        InstanceCmd::Java {
            instance,
            home,
            auto,
            memory,
            args,
            window,
        } => {
            let window = window.as_deref().map(parse_window).transpose()?;
            if instances::find(&instance)?.java.is_none() {
                return Err(format!(
                    "{instance} is a Eclipse instance: Eclipse is not a Java program."
                ));
            }
            changed(instances::edit(&instance, |m| {
                let java = m.java.get_or_insert_with(Default::default);
                if auto {
                    java.home = None;
                }
                java.home = home.or(java.home.take());
                java.memory_mb = memory.or(java.memory_mb);
                if let Some(args) = args {
                    java.jvm_args = args.trim().to_string();
                }
                if let Some(window) = window {
                    java.window = window;
                }
            })?)
        }
        InstanceCmd::Delete { instance } => {
            let gone = instances::delete(&instance)?;
            match as_json {
                true => json(serde_json::json!({"deleted": gone.id, "trashed": gone.work})),
                false => println!("{} is in the Trash.", gone.name),
            }
        }
        InstanceCmd::Rename { instance, name } => {
            changed(instances::edit(&instance, |m| m.name = Some(name))?)
        }
        InstanceCmd::Icon { instance, icon } => {
            let icon = (icon != "none").then(|| Icon::parse(&icon)).transpose()?;
            changed(instances::edit(&instance, |m| m.icon = icon)?)
        }
        InstanceCmd::Group { instance, group } => {
            changed(instances::edit(&instance, |m| m.group = group)?)
        }
        InstanceCmd::Select { instance } => changed(instances::select(&instance)?),
        InstanceCmd::Version { instance, version } => {
            let pin = match version.as_str() {
                "latest" => None,
                tag => Some(backend::resolve_version(Some(tag), settings)?),
            };
            changed(instances::edit(&instance, |m| m.version = pin)?)
        }
        InstanceCmd::Launch {
            instance,
            pre_launch,
            wrapper,
            env,
            gamemode,
            mangohud,
            dedicated_gpu,
            post_exit,
        } => {
            use eclipse_launcher::launch;
            let env = env.as_deref().map(launch::parse_env).transpose()?;
            let [gamemode, mangohud, dedicated_gpu] =
                [gamemode, mangohud, dedicated_gpu].map(|v| v.as_deref().map(on_off));
            let [gamemode, mangohud, dedicated_gpu] = [
                gamemode.transpose()?,
                mangohud.transpose()?,
                dedicated_gpu.transpose()?,
            ];
            let found = launch::Found::here();
            for (on, has, name) in [
                (gamemode, found.gamemode, "GameMode (gamemoderun)"),
                (mangohud, found.mangohud, "MangoHud"),
                (dedicated_gpu, found.two_gpus, "A second graphics card"),
            ] {
                if on == Some(true) && !has {
                    eprintln!(
                        "{name} isn't on this computer: the switch does nothing until it is."
                    );
                }
            }
            let changing = pre_launch.is_some()
                || wrapper.is_some()
                || post_exit.is_some()
                || env.is_some()
                || gamemode.is_some()
                || mangohud.is_some()
                || dedicated_gpu.is_some();
            let i = match changing {
                false => instances::find(&instance)?,
                true => instances::edit(&instance, |m| {
                    if let Some(p) = pre_launch {
                        m.launch.pre_launch = p.trim().to_string();
                    }
                    if let Some(w) = wrapper {
                        m.launch.wrapper = w.trim().to_string();
                    }
                    if let Some(p) = post_exit {
                        m.launch.post_exit = p.trim().to_string();
                    }
                    if let Some(e) = env {
                        m.launch.env = e;
                    }
                    if let Some(on) = gamemode {
                        m.launch.gamemode = on;
                    }
                    if let Some(on) = mangohud {
                        m.launch.mangohud = on;
                    }
                    if let Some(on) = dedicated_gpu {
                        m.launch.dedicated_gpu = on;
                    }
                })?,
            };
            let l = &i.launch;
            if as_json {
                json(serde_json::json!({
                    "instance": i.id, "pre_launch": l.pre_launch, "wrapper": l.wrapper, "env": l.env,
                    "post_exit": l.post_exit,
                    "gamemode": l.gamemode, "mangohud": l.mangohud, "dedicated_gpu": l.dedicated_gpu,
                    "found": {
                        "gamemode": found.gamemode, "mangohud": found.mangohud,
                        "two_gpus": found.two_gpus, "nvidia": found.nvidia,
                    },
                }));
            } else {
                println!(
                    "{}: {}",
                    i.name,
                    l.summary().unwrap_or("nothing around the game".into())
                );
                println!("  pre-launch: {}", l.pre_launch);
                println!("  wrapper:    {}", l.wrapper);
                println!("  env:        {}", launch::env_line(&l.env));
                println!("  post-exit:  {}", l.post_exit);
                let yes = |b: bool| if b { "on" } else { "off" };
                for (name, on, has) in [
                    ("gamemode:", l.gamemode, found.gamemode),
                    ("mangohud:", l.mangohud, found.mangohud),
                    ("gpu:", l.dedicated_gpu, found.two_gpus),
                ] {
                    let note = if has { "" } else { " (not on this computer)" };
                    println!("  {name:<11} {}{note}", yes(on));
                }
            }
        }
        InstanceCmd::Shortcut { instance, remove } => {
            use eclipse_launcher::shortcut;
            let i = match instance {
                Some(name) => instances::find(&name)?,
                None => instances::selected(settings),
            };
            let paths = match remove {
                false => shortcut::make(&backend::update_target(), &i.id, &i.name)?,
                true => shortcut::remove(&i.id)?,
            };
            if as_json {
                json(serde_json::json!({ "instance": i.id, "paths": paths }));
            } else if remove && paths.is_empty() {
                println!("{} has no shortcut.", i.name);
            } else {
                let did = if remove { "Moved to the trash" } else { "Made" };
                for path in paths {
                    println!("{did}: {}", path.display());
                }
            }
        }
        InstanceCmd::Others { folder } => {
            let found = other_instances(folder.as_deref())?;
            if as_json {
                json(found.iter().map(other_json).collect());
            } else if found.is_empty() {
                println!("No other launcher's instance found.");
            }
            for f in found.iter().filter(|_| !as_json) {
                println!(
                    "{}  ({}, {})\n    {}",
                    f.name,
                    f.launcher,
                    other_plays(f),
                    f.game.display()
                );
            }
        }
        InstanceCmd::CopyFrom {
            name,
            folder,
            new_name,
            minecraft,
        } => {
            let found = other_instances(folder.as_deref())?;
            let matching: Vec<_> = found.iter().filter(|f| f.name == name).collect();
            let f = match matching[..] {
                [one] => one,
                [] => {
                    return Err(format!(
                        "No other launcher has an instance called {name}: \
                         eclipse-launcher instance others"
                    ))
                }
                _ => {
                    return Err(format!(
                        "More than one launcher has an instance called {name}: say which \
                         with --folder."
                    ))
                }
            };
            let target = new_name.as_deref().unwrap_or(&f.name);
            let made = other_launchers::import(f, target, minecraft.as_deref()).map_err(|e| {
                if e == instances::name_taken(target.trim()) {
                    format!("{e} (`--as NAME`)")
                } else {
                    e
                }
            })?;
            let unsupported = other_launchers::loader(f).is_none();
            if let Some(l) = f.loader.as_ref().filter(|_| unsupported && !as_json) {
                println!(
                    "{} {} isn't installed for it yet: its mods are copied and wait for it.",
                    l.name, l.version
                );
            }
            changed(made);
        }
        InstanceCmd::Import {
            file, java: true, ..
        } => {
            let i =
                eclipse_launcher::instances_mrpack::import(&file, &|line, _| eprintln!("{line}"))?;
            if as_json {
                json(value(&i, false));
            } else {
                println!(
                    "Made the Java instance {} in {}: eclipse-launcher play --instance \"{}\"",
                    i.name,
                    i.home.display(),
                    i.name
                );
            }
        }
        InstanceCmd::Import { file, instance, .. } => {
            use eclipse_launcher::import::{self, Dropped};
            use eclipse_launcher::modpacks;
            match import::what(&file)? {
                Dropped::Instance => {
                    let i = import::import_instance(&file, &|line, _| eprintln!("{line}"))?;
                    if as_json {
                        json(value(&i, false));
                    } else {
                        println!(
                            "Made the instance {} in {}: eclipse-launcher instance select \"{}\"",
                            i.name,
                            i.home.display(),
                            i.name
                        );
                    }
                }
                Dropped::Modpack => {
                    let pack = modpacks::import(&file, &|line: String| eprintln!("{line}"))?;
                    println!("{} ({})", modpacks::outcome(&pack), pack.summary());
                    println!(
                        "Port and play it: eclipse-launcher modpack port {}",
                        pack.slug
                    );
                }
                // A game build is its own command: it installs a version
                // rather than touching an instance, and takes a `--version` to
                // say which release's game files it plays with.
                Dropped::Build => {
                    let tag = eclipse_launcher::build_import::import(&file, None, settings)?;
                    if as_json {
                        json(serde_json::json!({ "version": tag }));
                    } else {
                        println!("Imported the build as {tag}, and selected it.");
                    }
                }
                Dropped::Pack(_) => {
                    let i = match instance {
                        Some(name) => instances::find(&name)?,
                        None => instances::selected(settings),
                    };
                    let p = eclipse_launcher::packs::add(&i.home, &file)?;
                    println!(
                        "Added {} to {}: turn it on with eclipse-launcher packs on",
                        p.title, i.name
                    );
                }
                Dropped::DataPack => {
                    return Err(format!(
                        "{} is a data pack: put it into a world with eclipse-launcher worlds \
                         datapack-add <world> <file>",
                        file.display()
                    ))
                }
            }
        }
        InstanceCmd::Export {
            instance,
            format,
            world,
            out,
        } => {
            use eclipse_launcher::export::{self, Format};
            let i = match instance {
                Some(name) => instances::find(&name)?,
                None => instances::selected(settings),
            };
            let format = match format {
                ExportFormat::Mrpack => Format::Mrpack,
                ExportFormat::Zip => Format::Zip,
            };
            let worlds = world
                .iter()
                .map(|w| eclipse_launcher::worlds::find(&i.home, w).map(|w| w.folder))
                .collect::<Result<Vec<_>, _>>()?;
            let out = match out {
                Some(out) => out,
                None => export::free_path(
                    &std::env::current_dir().map_err(|e| e.to_string())?,
                    &i.name,
                    format,
                ),
            };
            let done = export::export(&i.home, &i.name, format, &worlds, &out)?;
            if as_json {
                json(serde_json::json!({
                    "path": done.path, "copied": done.copied, "linked": done.linked,
                    "worlds": done.worlds,
                }));
            } else {
                println!(
                    "Exported {} to {}: {} files, {} listed by their Modrinth download{}.",
                    i.name,
                    done.path.display(),
                    done.copied,
                    done.linked,
                    match done.worlds.is_empty() {
                        true => String::new(),
                        false => format!(", worlds: {}", done.worlds.join(", ")),
                    }
                );
            }
        }
    }
    Ok(())
}

fn other_instances(folder: Option<&Path>) -> Result<Vec<other_launchers::Found>, String> {
    use eclipse_launcher::sources::{Folders, Os};
    Ok(match folder {
        Some(dir) if !dir.is_dir() => return Err(format!("{} isn't a folder.", dir.display())),
        Some(dir) => other_launchers::in_folder(dir, None),
        None => {
            let folders = Folders::current().ok_or("No home folder.")?;
            other_launchers::scan(Os::current(), &folders)
        }
    })
}

fn other_plays(f: &other_launchers::Found) -> String {
    match (&f.minecraft, &f.loader) {
        (None, _) => "Minecraft version not recorded: copy-from --minecraft <version>".into(),
        (Some(v), Some(l)) => format!("Minecraft {v} with {} {}", l.name, l.version),
        (Some(v), None) => format!("Minecraft {v}"),
    }
}

fn other_json(f: &other_launchers::Found) -> serde_json::Value {
    serde_json::json!({
        "launcher": f.launcher, "name": f.name, "minecraft": f.minecraft,
        "loader": f.loader.as_ref().map(|l| l.name),
        "loader_version": f.loader.as_ref().map(|l| &l.version),
        "game": f.game, "shared": f.shared,
    })
}

/// `1280x720` (or `1280×720`), or `default` for none.
fn parse_window(text: &str) -> Result<Option<(u32, u32)>, String> {
    let text = text.trim();
    if text.eq_ignore_ascii_case("default") {
        return Ok(None);
    }
    let size = text
        .split_once(['x', 'X', '×'])
        .and_then(|(w, h)| Some((w.trim().parse().ok()?, h.trim().parse().ok()?)))
        .filter(|&(w, h): &(u32, u32)| (1..=16384).contains(&w) && (1..=16384).contains(&h));
    match size {
        Some(size) => Ok(Some(size)),
        None => Err(format!(
            "{text:?} isn't a window size: 1280x720, or default."
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_sizes() {
        assert_eq!(parse_window("1280x720"), Ok(Some((1280, 720))));
        assert_eq!(parse_window(" 1920 × 1080 "), Ok(Some((1920, 1080))));
        assert_eq!(parse_window("Default"), Ok(None));
        assert!(parse_window("0x720").is_err());
        assert!(parse_window("big").is_err());
    }
}
