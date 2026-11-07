use super::json;
use clap::Subcommand;
use eclipse_launcher::instances;
use std::path::PathBuf;

#[derive(Clone, Copy, clap::ValueEnum)]
pub(super) enum PackKindArg {
    Resource,
    Shader,
}

impl From<PackKindArg> for eclipse_launcher::packs::Kind {
    fn from(k: PackKindArg) -> Self {
        match k {
            PackKindArg::Resource => Self::Resource,
            PackKindArg::Shader => Self::Shader,
        }
    }
}

#[derive(Subcommand)]
pub(super) enum PacksCmd {
    /// The resource and shader packs, and which are on (the default)
    List {
        /// Only this kind
        #[arg(long, value_enum)]
        kind: Option<PackKindArg>,
        /// The instance (default: the selected one)
        #[arg(long)]
        instance: Option<String>,
    },
    /// Copy a pack file in: a resource pack .zip, a shader pack .zip or a .eshader
    Add {
        file: PathBuf,
        #[arg(long)]
        instance: Option<String>,
    },
    /// Turn a pack on (a resource pack goes on top; a shader pack replaces the one that was on)
    On {
        /// Its file name or title
        pack: String,
        #[arg(long, value_enum, default_value_t = PackKindArg::Resource)]
        kind: PackKindArg,
        #[arg(long)]
        instance: Option<String>,
    },
    /// Turn a pack off
    Off {
        pack: String,
        #[arg(long, value_enum, default_value_t = PackKindArg::Resource)]
        kind: PackKindArg,
        #[arg(long)]
        instance: Option<String>,
    },
    /// Turn a pack off and move it to the system's trash
    Remove {
        pack: String,
        #[arg(long, value_enum, default_value_t = PackKindArg::Resource)]
        kind: PackKindArg,
        #[arg(long)]
        instance: Option<String>,
    },
}

pub(super) fn packs(action: PacksCmd, as_json: bool) -> Result<(), String> {
    use eclipse_launcher::packs::{self, Kind};
    let home = |instance: Option<String>| -> Result<PathBuf, String> {
        Ok(match instance {
            Some(name) => instances::find(&name)?.home,
            None => instances::selected().home,
        })
    };
    let value = |p: &packs::Pack| {
        serde_json::json!({
            "kind": format!("{:?}", p.kind).to_lowercase(), "file": p.file, "title": p.title,
            "description": p.description, "enabled": p.enabled, "path": p.path,
        })
    };
    match action {
        PacksCmd::List { kind, instance } => {
            let home = home(instance)?;
            let kinds = match kind {
                Some(k) => vec![Kind::from(k)],
                None => vec![Kind::Resource, Kind::Shader],
            };
            let all: Vec<packs::Pack> = kinds.iter().flat_map(|k| packs::list(&home, *k)).collect();
            if as_json {
                json(all.iter().map(value).collect());
            }
            for k in kinds.iter().filter(|_| !as_json) {
                println!("{}:", k.folder());
                let these: Vec<_> = all.iter().filter(|p| p.kind == *k).collect();
                if these.is_empty() {
                    println!("    none");
                }
                for p in these {
                    let on = if p.enabled { "on " } else { "off" };
                    match p.title == p.file {
                        true => println!("  {on} {}", p.file),
                        false => println!("  {on} {} ({})", p.title, p.file),
                    }
                }
            }
        }
        PacksCmd::Add { file, instance } => {
            let pack = packs::add(&home(instance)?, &file)?;
            match as_json {
                true => json(value(&pack)),
                false => println!("Added {} to {} (off).", pack.file, pack.kind.folder()),
            }
        }
        PacksCmd::On {
            pack,
            kind,
            instance,
        } => set_pack(&home(instance)?, kind.into(), &pack, true)?,
        PacksCmd::Off {
            pack,
            kind,
            instance,
        } => set_pack(&home(instance)?, kind.into(), &pack, false)?,
        PacksCmd::Remove {
            pack,
            kind,
            instance,
        } => {
            let home = home(instance)?;
            let p = packs::find(&home, kind.into(), &pack)?;
            packs::remove(&home, &p)?;
            println!("{} is in the trash.", p.file);
        }
    }
    Ok(())
}

fn set_pack(
    home: &std::path::Path,
    kind: eclipse_launcher::packs::Kind,
    query: &str,
    on: bool,
) -> Result<(), String> {
    use eclipse_launcher::packs;
    let pack = packs::find(home, kind, query)?;
    packs::set_enabled(home, &pack, on)?;
    println!("{} is {}.", pack.title, if on { "on" } else { "off" });
    Ok(())
}
