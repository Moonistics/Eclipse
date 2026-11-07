use super::json;
use clap::Subcommand;
use eclipse_launcher::instances;
use std::path::PathBuf;

#[derive(Subcommand)]
pub(super) enum ScreenshotsCmd {
    /// The screenshots, the newest first, with their files (the default)
    List {
        /// The instance (default: the selected one)
        #[arg(long)]
        instance: Option<String>,
    },
    /// Move a screenshot to the system's trash
    Delete {
        /// Its file name, with or without .png
        screenshot: String,
        #[arg(long)]
        instance: Option<String>,
    },
}

pub(super) fn screenshots(action: ScreenshotsCmd, as_json: bool) -> Result<(), String> {
    use eclipse_launcher::screenshots;
    let home = |instance: Option<String>| -> Result<PathBuf, String> {
        Ok(match instance {
            Some(name) => instances::find(&name)?.home,
            None => instances::selected().home,
        })
    };
    match action {
        ScreenshotsCmd::List { instance } => {
            let home = home(instance)?;
            let shots = screenshots::list(&home);
            if as_json {
                json(
                    shots
                        .iter()
                        .map(|s| {
                            let millis = s
                                .taken
                                .duration_since(std::time::UNIX_EPOCH)
                                .map_or(0, |d| d.as_millis() as u64);
                            serde_json::json!({ "name": s.name, "path": s.path, "taken_ms": millis, "size": s.size })
                        })
                        .collect(),
                );
            } else if shots.is_empty() {
                println!("No screenshots in {}", screenshots::folder(&home).display());
            }
            for s in shots.iter().filter(|_| !as_json) {
                println!("{}", s.path.display());
            }
        }
        ScreenshotsCmd::Delete {
            screenshot,
            instance,
        } => {
            let shot = screenshots::find(&home(instance)?, &screenshot)?;
            screenshots::delete(&shot)?;
            println!("{} is in the trash.", shot.name);
        }
    }
    Ok(())
}
