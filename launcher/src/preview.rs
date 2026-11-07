//! `ECLIPSE_LAUNCHER_PAGE=<name>` opens the window on a page or a state, for screenshots:
//! `mods` (or `modpacks`, `resourcepacks`, `shaders`, `datapacks`), `servers`, `versions`,
//! `notes`, `settings`; Home on a tab (`instance-mods`, `worlds`, `multiplayer`, `packs`,
//! `screenshots`, `console`, `patch-notes`); `getting-ready` (Home as if Play had been pressed on
//! a version that downloads from Mojang, a third of the way through), `crash` (Home with a crash
//! notice), `sign-in` (the owner gate's sheet with a made-up code: nothing asks Microsoft);
//! `new-instance` (the New instance page on Modded; `new-instance-eclipse`,
//! `new-instance-vanilla` on those), `edit-instance`, `duplicate-instance` (the selected one's
//! Edit or Duplicate page). Nothing runs for a preview.
use crate::game_run::Crash;
use crate::home_tabs::HomeTab;
use crate::instance_state::NewKind;
use crate::page::Page;
use crate::settings_page;
use eclipse_launcher::backend;
use eclipse_launcher::browser::Kind as PackKind;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Preview {
    Page(Page),
    Mods(PackKind),
    HomeTab(HomeTab),
    GettingReady,
    Crash,
    SignIn,
    NewInstance(NewKind),
    EditInstance,
    DuplicateInstance,
}

impl Preview {
    fn named(name: &str) -> Option<Preview> {
        Some(match name {
            "mods" => Preview::Mods(PackKind::Mod),
            "modpacks" => Preview::Mods(PackKind::Modpack),
            "resourcepacks" => Preview::Mods(PackKind::ResourcePack),
            "shaders" => Preview::Mods(PackKind::Shader),
            "datapacks" => Preview::Mods(PackKind::DataPack),
            "servers" => Preview::Page(Page::Servers),
            "versions" => Preview::Page(Page::Versions),
            "notes" => Preview::Page(Page::Notes),
            "settings" => Preview::Page(Page::Settings),
            "instance-mods" => Preview::HomeTab(HomeTab::Mods),
            "worlds" => Preview::HomeTab(HomeTab::Worlds),
            "multiplayer" => Preview::HomeTab(HomeTab::Multiplayer),
            "packs" => Preview::HomeTab(HomeTab::Packs),
            "screenshots" => Preview::HomeTab(HomeTab::Screenshots),
            "console" => Preview::HomeTab(HomeTab::Console),
            "patch-notes" => Preview::HomeTab(HomeTab::PatchNotes),
            "getting-ready" => Preview::GettingReady,
            "crash" => Preview::Crash,
            "sign-in" => Preview::SignIn,
            "new-instance" => Preview::NewInstance(NewKind::Modded),
            "new-instance-eclipse" => Preview::NewInstance(NewKind::Eclipse),
            "new-instance-vanilla" => Preview::NewInstance(NewKind::Vanilla),
            "edit-instance" => Preview::EditInstance,
            "duplicate-instance" => Preview::DuplicateInstance,
            _ => return None,
        })
    }

    fn page(self) -> Page {
        match self {
            Preview::Page(page) => page,
            Preview::Mods(_) => Page::Mods,
            _ => Page::Home,
        }
    }
}

pub(super) fn preview() -> Option<Preview> {
    static PREVIEW: std::sync::OnceLock<Option<Preview>> = std::sync::OnceLock::new();
    *PREVIEW.get_or_init(|| Preview::named(&std::env::var("ECLIPSE_LAUNCHER_PAGE").ok()?))
}

pub(super) fn shows(this: Preview) -> bool {
    preview() == Some(this)
}

/// A data folder that did not open starts on Settings, where its message is; an explicit page
/// still wins.
pub(super) fn start_at(
    preview: Option<Preview>,
    data_folder_failed: bool,
) -> (Page, settings_page::SettingsSection) {
    match preview {
        Some(preview) => (preview.page(), Default::default()),
        None if data_folder_failed => (Page::Settings, settings_page::SettingsSection::System),
        None => (Page::Home, Default::default()),
    }
}

/// `ECLIPSE_LAUNCHER_PROJECT=<slug>` (with `ECLIPSE_LAUNCHER_PAGE=mods`): opens that
/// project's page once a search finds it, for screenshots.
pub(super) fn project() -> Option<&'static str> {
    static PROJECT: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    let project = PROJECT.get_or_init(|| std::env::var("ECLIPSE_LAUNCHER_PROJECT").ok());
    project.as_deref().filter(|_| !PROJECT_OPENED.load(Relaxed))
}

static PROJECT_OPENED: AtomicBool = AtomicBool::new(false);

pub(super) fn project_opened() {
    PROJECT_OPENED.store(true, Relaxed);
}

pub(super) fn crash() -> Option<Crash> {
    shows(Preview::Crash).then(|| Crash {
        report: Some(
            backend::game_home()
                .join(eclipse_launcher::game_folder::CRASH_REPORTS)
                .join("crash-2026-11-06_20.14.03-client.txt"),
        ),
        summary: "java.lang.IllegalStateException: Rendersystem called from wrong thread".into(),
        home: backend::game_home(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_data_folder_that_did_not_open_starts_where_its_message_is() {
        use settings_page::SettingsSection;
        assert_eq!(
            start_at(None, true),
            (Page::Settings, SettingsSection::System)
        );
        assert_eq!(
            start_at(None, false),
            (Page::Home, SettingsSection::Account)
        );
        let page = |name| start_at(Preview::named(name), true).0;
        assert_eq!(page("modpacks"), Page::Mods);
        assert_eq!(page("worlds"), Page::Home);
        assert_eq!(Preview::named("nowhere"), None);
    }
}
