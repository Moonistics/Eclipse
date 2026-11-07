//! The selected instance's server list on Home (`eclipse_launcher::server_list`, the game's own
//! `servers.dat`). The official server is put on top once, as the game does, at the address
//! links.json gives (`server_list::add_official`).
use crate::fields::Field;
use crate::folder_list::FolderList;
use crate::menus::Asking;
use crate::Launcher;
use eclipse_launcher::server_list::{self, Server};
use gpui::prelude::*;
use gpui::{Image, ImageFormat};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

#[derive(Default)]
pub(super) struct Multiplayer {
    /// The list as last read, with each picture decoded.
    pub(super) list: FolderList<(Server, Option<Arc<Image>>)>,
    pub(super) adding: bool,
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

    pub(super) fn load_servers(&mut self, home: &Path, cx: &mut Context<Self>) {
        if self.multiplayer.list.home() != Some(home) {
            self.multiplayer = Multiplayer::default();
        }
        let official = self.links.official_server.clone();
        let previous = self.links.official_server_previous.clone();
        let read = move |home: &Path| {
            // A list that can't be read is the game's to report: the page shows it empty.
            let _ = server_list::add_official(home, &official, &previous);
            server_list::List::load(home)
                .servers()
                .into_iter()
                .map(|s| {
                    let icon = s
                        .icon
                        .clone()
                        .map(|png| Arc::new(Image::from_bytes(ImageFormat::Png, png)));
                    (s, icon)
                })
                .collect()
        };
        self.read_folder(home, |this| &mut this.multiplayer.list, read, |_| {}, cx);
    }

    pub(super) fn edit_servers(
        &mut self,
        edit: impl FnOnce(&mut server_list::List) -> Result<(), String>,
    ) {
        let Some(home) = self.multiplayer.list.home().map(Path::to_path_buf) else {
            return;
        };
        let mut list = server_list::List::load(&home);
        self.multiplayer.error = edit(&mut list).and_then(|()| list.save(&home)).err();
        self.stop_asking(|a| matches!(a, Asking::SavedServer(_)));
        self.multiplayer.list.stale();
    }

    /// Enter in the address field, or Save server.
    pub(super) fn add_server(&mut self) {
        let address = self.fields.value(Field::MultiplayerAddress).to_string();
        let name = match self.fields.value(Field::MultiplayerName).trim() {
            "" => address.clone(),
            name => name.to_string(),
        };
        self.edit_servers(|list| list.add(&name, &address));
        if self.multiplayer.error.is_none() {
            self.fields.text(Field::MultiplayerName).set(String::new());
            self.fields
                .text(Field::MultiplayerAddress)
                .set(String::new());
            self.multiplayer.adding = false;
        }
    }

    /// How many servers the selected instance's list has; None while it is read.
    pub(super) fn servers_count(&mut self, cx: &mut Context<Self>) -> Option<usize> {
        let home = self.selected_instance()?.home.clone();
        self.load_servers(&home, cx);
        self.multiplayer
            .list
            .fresh()
            .then_some(self.multiplayer.list.len())
    }
}
