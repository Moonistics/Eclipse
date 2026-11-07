use super::json;
use clap::Subcommand;
use eclipse_launcher::backend;
use eclipse_launcher::instances;
use eclipse_launcher::server_list;
use std::path::PathBuf;

#[derive(Subcommand)]
pub(super) enum MultiplayerCmd {
    /// The servers of the game's Multiplayer screen (the default)
    List {
        /// The instance (default: the selected one)
        #[arg(long)]
        instance: Option<String>,
    },
    /// Add a server to the list
    Add {
        /// Its address, as in the game's Add Server (host or host:port)
        address: String,
        /// Its name in the list (default: Minecraft Server)
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        instance: Option<String>,
    },
    /// Remove a server from the list: its position from 1, name or address
    Remove {
        server: String,
        #[arg(long)]
        instance: Option<String>,
    },
}

pub(super) fn multiplayer(
    action: MultiplayerCmd,
    as_json: bool,
    settings: &backend::Settings,
) -> Result<(), String> {
    let home = |instance: Option<String>| -> Result<PathBuf, String> {
        Ok(match instance {
            Some(name) => instances::find(&name)?.home,
            None => instances::selected(settings).home,
        })
    };
    match action {
        MultiplayerCmd::List { instance } => {
            let home = home(instance)?;
            let servers = server_list::List::load(&home).servers();
            if as_json {
                json(
                    servers
                        .iter()
                        .map(|s| serde_json::json!({ "name": s.name, "address": s.ip, "icon": s.icon.is_some() }))
                        .collect(),
                );
            } else if servers.is_empty() {
                println!(
                    "No servers in {}",
                    home.join(eclipse_launcher::game_folder::SERVERS).display()
                );
            }
            for (i, s) in servers.iter().enumerate().filter(|_| !as_json) {
                println!("{}. {} ({})", i + 1, s.name, s.ip);
            }
        }
        MultiplayerCmd::Add {
            address,
            name,
            instance,
        } => {
            let home = home(instance)?;
            let mut list = server_list::List::load(&home);
            list.add(name.as_deref().unwrap_or(""), &address)?;
            list.save(&home)?;
            println!("Added {}.", address.trim());
        }
        MultiplayerCmd::Remove { server, instance } => {
            let home = home(instance)?;
            let mut list = server_list::List::load(&home);
            let (i, _) = server_list::find(&list, &server)?;
            let gone = list.remove(i)?;
            list.save(&home)?;
            println!("Removed {} ({}).", gone.name, gone.ip);
        }
    }
    Ok(())
}
