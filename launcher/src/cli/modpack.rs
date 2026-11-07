use super::json;
use clap::Subcommand;
use eclipse_launcher::backend;
use eclipse_launcher::instances;
use std::path::PathBuf;

#[derive(Subcommand)]
pub(super) enum ModpackCmd {
    /// The imported packs and their coverage (the default)
    List,
    /// Import a .mrpack, a CurseForge zip or an instance folder, and write its coverage report
    Import { path: PathBuf },
    /// Get the jars of what this computer can port and carry them over
    Port {
        /// The pack's folder name, as list shows it
        pack: String,
        /// A folder with your own jars (an installed instance, a downloads folder)
        #[arg(long)]
        from: Option<PathBuf>,
        /// The installed version the ports are for (default: the selected one)
        #[arg(long)]
        version: Option<String>,
    },
    /// Play a ported pack as its own profile
    Play {
        pack: String,
        #[arg(long)]
        version: Option<String>,
    },
    /// Remove an imported pack: its jars, ports and the profile's saves
    Remove { pack: String },
}

pub(super) fn modpack(action: ModpackCmd, as_json: bool) -> Result<(), String> {
    use eclipse_launcher::browser::eclipse;
    use eclipse_launcher::modpacks::{self, Imported};
    use eclipse_port_pack::report::Status;
    let log = &|line: String| eprintln!("{line}");
    let work = |pack: &str| {
        let registry = eclipse::pack::packs_dir().join(pack);
        if eclipse::pack::is_pack(&registry) {
            return Ok(registry);
        }
        let work = modpacks::ports_dir().join(pack);
        modpacks::load(&work).map(|_| work)
    };
    let show = |p: &Imported| {
        let counts: Vec<String> = Status::ALL
            .iter()
            .map(|s| format!("{} {}", p.count(*s), s.label()))
            .collect();
        if as_json {
            json(serde_json::json!({
                "pack": p.slug, "name": p.title(), "summary": p.summary(),
                "counts": Status::ALL.iter().map(|s| (s.label(), p.count(*s))).collect::<std::collections::BTreeMap<_, _>>(),
                "portable": p.portable, "ported_mods": p.mods.len(), "jars": p.jars(),
                "report": p.work.join("coverage.md"),
            }));
        } else {
            println!(
                "{}  ({})\n  {}\n  {}",
                p.title(),
                p.slug,
                p.summary(),
                counts.join(", ")
            );
            println!("  {} portable, {} carried over", p.portable, p.mods.len());
            if let Some(jars) = p.jars() {
                println!("  {jars}");
            }
            println!("  {}", p.work.join("coverage.md").display());
        }
    };
    match action {
        ModpackCmd::List => {
            let packs = modpacks::list();
            let registry = eclipse::pack::list();
            if packs.is_empty() && registry.is_empty() && !as_json {
                println!("No modpack imported yet: eclipse-launcher modpack import <pack>");
            }
            packs.iter().for_each(show);
            for p in &registry {
                if as_json {
                    json(
                        serde_json::json!({"pack": p.id, "name": p.name, "version": p.version, "source": "eclipse", "mods": p.mods}),
                    );
                } else {
                    println!(
                        "{} {}  ({}, from Eclipse's registry)\n  {}",
                        p.name,
                        p.version,
                        p.id,
                        p.mods.join(", ")
                    );
                }
            }
        }
        ModpackCmd::Import { path } => show(&modpacks::import(&path, log)?),
        ModpackCmd::Port {
            pack,
            from,
            version,
        } => {
            let tag = backend::resolve_version(version.as_deref()).ok();
            let resources = tag
                .filter(|t| backend::is_prepared(t))
                .map(|t| backend::resources(&t));
            show(&modpacks::port(
                &work(&pack)?,
                resources.as_deref(),
                from.as_deref(),
                log,
            )?)
        }
        ModpackCmd::Play { pack, version } => {
            let work = work(&pack)?;
            let instance = instances::list()
                .into_iter()
                .find(|i| i.work == work)
                .ok_or_else(|| format!("{} is not an installed modpack.", work.display()))?;
            super::play::launch(backend::Play {
                version,
                ..backend::Play::of(instance)
            })?
        }
        ModpackCmd::Remove { pack } => modpacks::remove(&work(&pack)?)?,
    }
    Ok(())
}
