//! The Instance row's desktop shortcut (`eclipse_launcher::shortcut`): make one that plays the
//! selected instance, or take it away.
use crate::Launcher;
use eclipse_launcher::i18n::trf;
use eclipse_launcher::{backend, shortcut};

#[derive(Default)]
pub(super) struct Shortcuts {
    /// The instance last looked at, and whether it has a shortcut: read again only when the
    /// selection changes or a button is pressed, not on every frame.
    seen: Option<(String, bool)>,
}

impl Launcher {
    pub(super) fn has_shortcut(&mut self, id: &str) -> bool {
        match &self.shortcuts.seen {
            Some((seen, has)) if seen == id => *has,
            _ => {
                let has = !shortcut::find(id).is_empty();
                self.shortcuts.seen = Some((id.to_string(), has));
                has
            }
        }
    }

    pub(super) fn toggle_shortcut(&mut self, id: String, name: String) {
        let has = self.has_shortcut(&id);
        let done = match has {
            false => shortcut::make(&backend::update_target(), &id, &name)
                .map(|_| trf("Added a desktop shortcut for {name}", &[("name", &name)])),
            true => shortcut::remove(&id)
                .map(|_| trf("{name}'s shortcut is in the trash", &[("name", &name)])),
        };
        self.shortcuts.seen = None;
        match done {
            Ok(said) => crate::toast::say(said),
            Err(e) => crate::toast::say_error(e),
        }
    }
}
