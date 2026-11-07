//! Profile-head I/O stays on workers; every account surface shares the same pixel-sharp cache.
use crate::messages::Msg;
use crate::{art, ui};
use eclipse_launcher::account;
use eclipse_theme::glass::{fill, ink};
use gpui::prelude::*;
use gpui::{div, img, px, Div, RenderImage};
use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

/// How long a head that fell back to the default skin waits before the real one is fetched again.
const RETRY: Duration = Duration::from_secs(60);

const BIG_HEAD: f32 = 64.;

#[derive(Default)]
pub(super) struct Heads {
    accounts: HashMap<String, Head>,
    players: HashMap<String, Head>,
}

struct Head {
    version: Option<String>,
    pending: bool,
    face: Option<Vec<[u8; 4]>>,
    /// Set while `face` is a fallback because the skin download failed.
    retry_at: Option<Instant>,
    images: RefCell<HashMap<(u32, u32), Arc<RenderImage>>>,
}

impl Heads {
    /// Asks for the heads of `accounts` and of the offline names `offline` (drawn by
    /// [`heads::offline_key`]); forgets the others.
    pub fn request(
        &mut self,
        accounts: &[account::Account],
        offline: &[String],
        resources: PathBuf,
        tx: &Sender<Msg>,
    ) {
        let keys: Vec<String> = offline
            .iter()
            .map(|n| eclipse_launcher::heads::offline_key(n))
            .collect();
        self.accounts
            .retain(|uuid, _| accounts.iter().any(|a| &a.uuid == uuid) || keys.contains(uuid));
        for (name, key) in offline.iter().zip(keys) {
            // The name's own skin, by when it was last changed, so a new one is read again.
            let file = eclipse_launcher::skins::files(&eclipse_launcher::skins::dir(), name).0;
            let version = std::fs::metadata(&file)
                .and_then(|m| m.modified())
                .ok()
                .map(|t| format!("{t:?}"));
            if self
                .accounts
                .get(&key)
                .is_some_and(|h| h.version == version && (h.pending || h.face.is_some()))
            {
                continue;
            }
            let previous = self.accounts.remove(&key).and_then(|h| h.face);
            self.accounts.insert(
                key.clone(),
                Head {
                    version: version.clone(),
                    pending: true,
                    face: previous,
                    retry_at: None,
                    images: RefCell::default(),
                },
            );
            let (resources, tx) = (resources.clone(), tx.clone());
            std::thread::spawn(move || {
                let face = std::fs::read(&file)
                    .ok()
                    .and_then(|png| eclipse_launcher::heads::face(&png))
                    .or_else(|| eclipse_launcher::heads::default_face(&resources, &key));
                let _ = tx.send(Msg::AccountHead(key, version, face, false));
            });
        }
        for account in accounts {
            let url = account.skin.as_ref().map(|s| s.0.clone());
            if self.accounts.get(&account.uuid).is_some_and(|h| {
                h.version == url
                    && (h.pending
                        || h.face.is_some() && h.retry_at.is_none_or(|t| Instant::now() < t))
            }) {
                continue;
            }
            let previous = self.accounts.remove(&account.uuid).and_then(|h| h.face);
            self.accounts.insert(
                account.uuid.clone(),
                Head {
                    version: url.clone(),
                    pending: true,
                    face: previous,
                    retry_at: None,
                    images: RefCell::default(),
                },
            );
            let (account, resources, tx) = (account.clone(), resources.clone(), tx.clone());
            std::thread::spawn(move || {
                let (face, failed) = eclipse_launcher::heads::profile_face(
                    &account,
                    &eclipse_launcher::heads::cache_dir(),
                    &resources,
                );
                let _ = tx.send(Msg::AccountHead(account.uuid, url, face, failed));
            });
        }
    }

    pub fn loaded(
        &mut self,
        uuid: &str,
        version: &Option<String>,
        face: Option<Vec<[u8; 4]>>,
        failed: bool,
    ) {
        if let Some(head) = self
            .accounts
            .get_mut(uuid)
            .filter(|h| &h.version == version)
        {
            head.pending = false;
            head.retry_at = failed.then(|| Instant::now() + RETRY);
            if face.is_some() {
                head.face = face;
            }
            head.images.get_mut().clear();
        }
    }

    pub fn draw(&self, uuid: Option<&str>, size: f32, scale: f32) -> Div {
        face_tile(uuid.and_then(|uuid| self.accounts.get(uuid)), size, scale)
    }

    pub fn draw_player(&mut self, uuid: &str, resources: &Path, size: f32, scale: f32) -> Div {
        let head = self
            .players
            .entry(uuid.to_string())
            .or_insert_with(|| Head {
                version: None,
                pending: false,
                face: eclipse_launcher::heads::default_face(resources, uuid),
                retry_at: None,
                images: RefCell::default(),
            });
        face_tile(Some(head), size, scale)
    }
}

fn face_tile(head: Option<&Head>, size: f32, scale: f32) -> Div {
    let image = head.and_then(|head| {
        let face = head.face.as_ref()?;
        Some(
            head.images
                .borrow_mut()
                .entry((size.to_bits(), scale.to_bits()))
                .or_insert_with(|| art::bake(face, 8, 8, size / 8., scale))
                .clone(),
        )
    });
    let d = div()
        .flex_none()
        .size(px(size))
        .overflow_hidden()
        .bg(ui::ink(fill::BUTTON))
        .flex()
        .items_center()
        .justify_center();
    let placeholder = if size >= BIG_HEAD {
        ui::IconSize::Px32
    } else {
        ui::IconSize::Px16
    };
    match image {
        Some(image) => d.child(img(image).size(px(size))),
        None => d.child(ui::icon("user", placeholder, ui::ink(ink::MUTED))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_skin_results_are_ignored_and_scaled_images_are_reused() {
        let mut heads = Heads::default();
        let (tx, _rx) = std::sync::mpsc::channel();
        heads.request(
            &[],
            &["HeadsTest".into()],
            PathBuf::from("no-resources"),
            &tx,
        );
        let player = eclipse_launcher::heads::offline_key("HeadsTest");
        let current = None;
        heads.loaded(
            &player,
            &Some("old-skin".into()),
            Some(vec![[255, 0, 0, 255]; 64]),
            false,
        );
        assert!(heads.accounts[&player].face.is_none());
        heads.loaded(&player, &current, Some(vec![[0, 0, 255, 255]; 64]), true);
        assert!(
            heads.accounts[&player].retry_at.is_some(),
            "a fallback face is fetched again later"
        );
        heads.loaded(&player, &current, Some(vec![[0, 255, 0, 255]; 64]), false);
        assert!(heads.accounts[&player].retry_at.is_none());
        let _ = heads.draw(Some(&player), 32., 2.);
        let key = (32f32.to_bits(), 2f32.to_bits());
        let image = heads.accounts[&player].images.borrow()[&key].clone();
        assert_eq!(image.size(0).width.0, 64);
        assert_eq!(image.size(0).height.0, 64);
        let _ = heads.draw(Some(&player), 32., 2.);
        assert!(Arc::ptr_eq(
            &image,
            &heads.accounts[&player].images.borrow()[&key]
        ));
    }
}
