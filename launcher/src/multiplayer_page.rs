//! The selected instance's server list on Home (`eclipse_launcher::server_list`, the game's own
//! `servers.dat`): each server with the picture the game last saw, Join (the game starts straight
//! into it), Remove after a confirmation, and a line to add one. The official server is put on top
//! once, as the game does, at the address links.json gives (`server_list::add_official`).
use super::*;
use eclipse_launcher::server_list::{self, Server};
use gpui::{Image, ImageFormat};
use servers_page::Field;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Default)]
pub(super) struct Multiplayer {
    /// The game folder the list is of, and the list as last read with each picture decoded.
    home: Option<PathBuf>,
    pub(super) list: Vec<(Server, Option<Arc<Image>>)>,
    fresh: bool,
    pub(super) adding: bool,
    pub(super) confirm_remove: Option<usize>,
    pub(super) error: Option<String>,
    /// What each address answered to the status ping, asked once per list.
    pub(super) reach: HashMap<String, Reach>,
}

/// A saved server as its status ping found it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Reach {
    Asking,
    Online { players: u64, max: u64 },
    Offline,
}

impl Multiplayer {
    /// Read again on the next frame: the game may have changed its list.
    pub(super) fn stale(&mut self) {
        self.fresh = false;
    }
}

impl Launcher {
    /// Asks each listed server for its players once, as the game's Multiplayer list does when it
    /// opens; the rows show the answer (a dot and "3 of 10" or "Offline").
    pub(super) fn ping_servers(&mut self, cx: &mut Context<Self>) {
        let asks: Vec<String> = self
            .multiplayer
            .list
            .iter()
            .map(|(s, _)| s.ip.clone())
            .filter(|ip| !self.multiplayer.reach.contains_key(ip))
            .collect();
        for ip in asks {
            self.multiplayer.reach.insert(ip.clone(), Reach::Asking);
            let address = ip.clone();
            let task = cx
                .background_executor()
                .spawn(async move { eclipse_launcher::telemetry::server_players(&address) });
            cx.spawn(async move |this, cx| {
                let answer = task.await;
                let _ = this.update(cx, |this, cx| {
                    let reach = match answer {
                        Some(p) => Reach::Online {
                            players: p.online,
                            max: p.max,
                        },
                        None => Reach::Offline,
                    };
                    this.multiplayer.reach.insert(ip, reach);
                    cx.notify();
                });
            })
            .detach();
        }
    }

    pub(super) fn load_servers(&mut self, home: PathBuf) {
        if self.multiplayer.home.as_ref() != Some(&home) {
            self.multiplayer = Multiplayer {
                home: Some(home.clone()),
                ..Multiplayer::default()
            };
        }
        if self.multiplayer.fresh {
            return;
        }
        // A list that can't be read is the game's to report: the page shows it empty.
        let _ = server_list::add_official(
            &home,
            &self.links.official_server,
            &self.links.official_server_previous,
        );
        // A few servers and their 64×64 pictures: small enough to read while drawing.
        self.multiplayer.list = server_list::List::load(&home)
            .servers()
            .into_iter()
            .map(|s| {
                let icon = s
                    .icon
                    .clone()
                    .map(|png| Arc::new(Image::from_bytes(ImageFormat::Png, png)));
                (s, icon)
            })
            .collect();
        self.multiplayer.fresh = true;
    }

    pub(super) fn edit_servers(
        &mut self,
        edit: impl FnOnce(&mut server_list::List) -> Result<(), String>,
    ) {
        let Some(home) = self.multiplayer.home.clone() else {
            return;
        };
        let mut list = server_list::List::load(&home);
        self.multiplayer.error = edit(&mut list).and_then(|()| list.save(&home)).err();
        self.multiplayer.confirm_remove = None;
        self.multiplayer.stale();
    }

    /// Enter in the address field, or Add.
    pub(super) fn add_server(&mut self) {
        let name = self.servers.value(Field::ServerName).to_string();
        let address = self.servers.value(Field::ServerAddress).to_string();
        self.edit_servers(|list| list.add(&name, &address));
        if self.multiplayer.error.is_none() {
            self.servers.text(Field::ServerName).set(String::new());
            self.servers.text(Field::ServerAddress).set(String::new());
            self.multiplayer.adding = false;
        }
    }

    /// How many servers the selected instance's list has.
    pub(super) fn servers_count(&mut self) -> usize {
        let Some(home) = self.selected_instance().map(|i| i.home.clone()) else {
            return 0;
        };
        self.load_servers(home);
        self.multiplayer.list.len()
    }
}
