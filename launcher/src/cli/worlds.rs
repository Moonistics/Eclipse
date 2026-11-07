use super::json;
use clap::Subcommand;
use eclipse_launcher::backend;
use eclipse_launcher::instances;
use std::path::PathBuf;

#[derive(Subcommand)]
pub(super) enum WorldsCmd {
    /// The worlds: name, mode, version, last played and size (the default)
    List {
        /// The instance (default: the selected one)
        #[arg(long)]
        instance: Option<String>,
    },
    /// Zip a world into the instance's backups/ folder, as the game's Make Backup does
    Backup {
        /// Its folder or name
        world: String,
        #[arg(long)]
        instance: Option<String>,
    },
    /// Copy a world beside it, as "<name> (1)"
    Copy {
        world: String,
        #[arg(long)]
        instance: Option<String>,
    },
    /// Move a world to the system's trash
    Delete {
        world: String,
        #[arg(long)]
        instance: Option<String>,
    },
    /// A world's data packs, and which are on
    DatapackList {
        world: String,
        #[arg(long)]
        instance: Option<String>,
    },
    /// Copy a data pack zip into a world (the game turns it on when the world opens)
    DatapackAdd {
        world: String,
        file: PathBuf,
        #[arg(long)]
        instance: Option<String>,
    },
    /// Move a world's data pack to the system's trash
    DatapackRemove {
        world: String,
        /// Its file name, with or without .zip
        pack: String,
        #[arg(long)]
        instance: Option<String>,
    },
}

pub(super) fn worlds(
    action: WorldsCmd,
    as_json: bool,
    settings: &backend::Settings,
) -> Result<(), String> {
    use eclipse_launcher::datapacks;
    use eclipse_launcher::worlds::{self, World};
    let home = |instance: Option<String>| -> Result<PathBuf, String> {
        Ok(match instance {
            Some(name) => instances::find(&name)?.home,
            None => instances::selected(settings).home,
        })
    };
    let value = |w: &World| {
        let millis = w
            .last_played
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        serde_json::json!({
            "folder": w.folder, "name": w.name, "mode": w.mode, "version": w.version,
            "last_played_ms": millis, "size": w.size, "path": w.path, "icon": w.icon,
        })
    };
    match action {
        WorldsCmd::List { instance } => {
            let home = home(instance)?;
            let all = worlds::list(&home);
            if as_json {
                json(all.iter().map(value).collect());
            } else if all.is_empty() {
                println!(
                    "No worlds in {}",
                    eclipse_launcher::worlds::saves_dir(&home).display()
                );
            }
            for w in all.iter().filter(|_| !as_json) {
                let version = w.version.as_deref().unwrap_or("unknown version");
                let mb = w.size as f64 / 1_000_000.0;
                println!(
                    "{} ({})\n    {}, {version}, {mb:.1} MB",
                    w.name, w.folder, w.mode
                );
            }
        }
        WorldsCmd::Backup { world, instance } => {
            let home = home(instance)?;
            let zip = worlds::backup(&home, &worlds::find(&home, &world)?)?;
            match as_json {
                true => json(serde_json::json!({ "backup": zip })),
                false => println!("{}", zip.display()),
            }
        }
        WorldsCmd::Copy { world, instance } => {
            let home = home(instance)?;
            let copy = worlds::copy(&worlds::find(&home, &world)?)?;
            match as_json {
                true => json(value(&copy)),
                false => println!("Copied to {} ({})", copy.name, copy.folder),
            }
        }
        WorldsCmd::Delete { world, instance } => {
            let home = home(instance)?;
            let w = worlds::find(&home, &world)?;
            worlds::delete(&w)?;
            println!("{} is in the trash.", w.name);
        }
        WorldsCmd::DatapackList { world, instance } => {
            let w = worlds::find(&home(instance)?, &world)?;
            let packs = datapacks::list(&w.path);
            if as_json {
                json(
                    packs
                        .iter()
                        .map(|p| serde_json::json!({ "file": p.file, "description": p.description, "enabled": p.enabled, "path": p.path }))
                        .collect(),
                );
            } else if packs.is_empty() {
                println!("No data packs in {}", w.name);
            }
            for p in packs.iter().filter(|_| !as_json) {
                let on = if p.enabled { "on " } else { "off" };
                println!("  {on} {}  {}", p.file, p.description);
            }
        }
        WorldsCmd::DatapackAdd {
            world,
            file,
            instance,
        } => {
            let w = worlds::find(&home(instance)?, &world)?;
            let p = datapacks::add(&w.path, &file)?;
            println!(
                "Added {} to {}: the game turns it on when the world opens.",
                p.file, w.name
            );
        }
        WorldsCmd::DatapackRemove {
            world,
            pack,
            instance,
        } => {
            let w = worlds::find(&home(instance)?, &world)?;
            let p = datapacks::find(&w.path, &pack)?;
            datapacks::remove(&w.path, &p)?;
            println!("{} is in the trash.", p.file);
        }
    }
    Ok(())
}
