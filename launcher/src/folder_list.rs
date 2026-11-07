use crate::Launcher;
use gpui::Context;
use std::path::{Path, PathBuf};

pub(super) struct FolderList<T> {
    home: Option<PathBuf>,
    list: Vec<T>,
    generation: u64,
    read_at: Option<u64>,
    reading: Option<u64>,
}

impl<T> Default for FolderList<T> {
    fn default() -> Self {
        Self {
            home: None,
            list: Vec::new(),
            generation: 0,
            read_at: None,
            reading: None,
        }
    }
}

impl<T> std::ops::Deref for FolderList<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        &self.list
    }
}

impl<T> FolderList<T> {
    pub(super) fn stale(&mut self) {
        self.generation += 1;
    }

    pub(super) fn home(&self) -> Option<&Path> {
        self.home.as_deref()
    }

    pub(super) fn fresh(&self) -> bool {
        self.read_at == Some(self.generation)
    }

    pub(super) fn of(&self, home: &Path) -> Option<&[T]> {
        (self.home() == Some(home) && self.read_at.is_some()).then_some(&self.list)
    }

    fn wants_read(&mut self, home: &Path) -> Option<u64> {
        if self.home() != Some(home) {
            *self = Self {
                home: Some(home.to_path_buf()),
                ..Self::default()
            };
        }
        let wanted = self.generation;
        if self.read_at == Some(wanted) || self.reading == Some(wanted) {
            return None;
        }
        self.reading = Some(wanted);
        Some(wanted)
    }

    fn arrived(&mut self, home: &Path, generation: u64, list: Vec<T>) {
        if self.home() != Some(home) {
            return;
        }
        if self.reading == Some(generation) {
            self.reading = None;
        }
        self.list = list;
        self.read_at = Some(generation);
    }
}

impl Launcher {
    pub(super) fn read_folder<T: Send + 'static>(
        &mut self,
        home: &Path,
        list: fn(&mut Launcher) -> &mut FolderList<T>,
        read: impl FnOnce(&Path) -> Vec<T> + Send + 'static,
        then: impl FnOnce(&mut Launcher) + 'static,
        cx: &mut Context<Self>,
    ) {
        let Some(generation) = list(self).wants_read(home) else {
            return;
        };
        let home = home.to_path_buf();
        let task = cx.background_executor().spawn({
            let home = home.clone();
            async move { read(&home) }
        });
        cx.spawn(async move |this, cx| {
            let found = task.await;
            let _ = this.update(cx, |this, cx| {
                list(this).arrived(&home, generation, found);
                then(this);
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn game_folder_changed(&mut self) {
        self.worlds.list.stale();
        self.multiplayer.list.stale();
        self.screenshots.list.stale();
        self.game_packs.list.stale();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_list_read_before_the_game_saved_is_read_again() {
        let home = Path::new("/games/survival");
        let mut worlds = FolderList::<&str>::default();
        let first = worlds.wants_read(home).unwrap();
        assert_eq!(worlds.wants_read(home), None, "one read at a time");
        worlds.stale();
        worlds.arrived(home, first, vec![]);
        assert!(!worlds.fresh(), "the read began before the game saved");
        let second = worlds.wants_read(home).unwrap();
        worlds.arrived(home, second, vec!["New World"]);
        assert!(worlds.fresh());
        assert_eq!(worlds.of(home), Some(&["New World"][..]));
    }

    #[test]
    fn a_list_belongs_to_the_folder_it_was_read_from() {
        let (survival, creative) = (Path::new("/games/survival"), Path::new("/games/creative"));
        let mut worlds = FolderList::<&str>::default();
        let read = worlds.wants_read(survival).unwrap();
        worlds.wants_read(creative);
        worlds.arrived(survival, read, vec!["Old World"]);
        assert_eq!(worlds.of(creative), None);
        assert_eq!(worlds.of(survival), None);
        assert!(worlds.is_empty());
    }
}
