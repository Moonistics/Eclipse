use super::*;
use eclipse_theme::glass::space;

const INSTALL_BAR: f32 = layout::FIELD_MD;

pub(super) struct Versions {
    pub(super) releases: Load<Vec<Release>>,
    pub(super) installed: Vec<String>,
    pub(super) installing: Option<Install>,
    pub(super) removing: Vec<String>,
    pub(super) disk: Vec<(String, u64)>,
    pub(super) disk_for: Vec<String>,
    pub(super) install_error: Option<(String, String)>,
    pub(super) importing: bool,
    pub(super) import_error: Option<String>,
    pub(super) update_install: Option<(String, bool)>,
    pub(super) instance_install: Option<String>,
    pub(super) install_next: Option<Release>,
    pub(super) install_started: Option<Instant>,
}

pub(super) struct Install {
    pub(super) tag: String,
    pub(super) bar: (f32, f32),
}

impl Install {
    pub(super) fn done(&self) -> f32 {
        self.bar.1
    }
}

impl Versions {
    pub(super) fn new(installed: Vec<String>) -> Self {
        Self {
            releases: Load::Loading,
            installed,
            installing: None,
            removing: Vec::new(),
            disk: Vec::new(),
            disk_for: Vec::new(),
            install_error: None,
            importing: false,
            import_error: None,
            update_install: None,
            instance_install: None,
            install_next: None,
            install_started: None,
        }
    }

    pub(super) fn installing(&self, tag: &str) -> Option<&Install> {
        self.installing.as_ref().filter(|i| i.tag == tag)
    }

    pub(super) fn release(&self, tag: &str) -> Option<&Release> {
        match &self.releases {
            Load::Ready(list) => list.iter().find(|r| r.tag == tag && r.asset.is_some()),
            _ => None,
        }
    }
}

impl Launcher {
    pub(super) fn install_bar(&self) -> Option<Div> {
        let install = self.versions.installing.as_ref()?;
        Some(
            div()
                .w_full()
                .max_w(px(INSTALL_BAR))
                .child(ui::progress_line(tr("Installing"), Some(install.bar), "")),
        )
    }

    pub(super) fn eclipse_install_state(&self, id: &str) -> Option<Stateful<Div>> {
        let bar = self.install_bar()?;
        Some(
            ui::loading_state(id.to_string(), ui::page_only(tr("Installing Eclipse")))
                .child(bar.mt(px(space::LG))),
        )
    }

    pub(super) fn import_build_button(&self, id: &str, cx: &mut Context<Self>) -> Stateful<Div> {
        let busy = self.versions.importing;
        let button = ui::button(
            id.to_string(),
            Some("folder-01"),
            tr(if busy {
                "Importing…"
            } else {
                "Import a version from file…"
            }),
            ButtonKind::Secondary,
            ui::ButtonSize::Md,
        );
        if busy {
            return ui::disabled(button);
        }
        button.on_press(cx.listener(|this, _: &ClickEvent, _, cx| this.import_build(cx)))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum UpdateCheck {
    Never,
    Checking(Instant),
    Checked(Instant),
    Failed(Instant, String),
}

impl UpdateCheck {
    pub(super) fn since(&self) -> Option<Instant> {
        match self {
            UpdateCheck::Never => None,
            UpdateCheck::Checking(at) | UpdateCheck::Checked(at) | UpdateCheck::Failed(at, _) => {
                Some(*at)
            }
        }
    }

    pub(super) fn checking(&self) -> bool {
        matches!(self, UpdateCheck::Checking(_))
    }

    pub(super) fn clean(&self) -> bool {
        matches!(self, UpdateCheck::Checked(_))
    }

    pub(super) fn error(&self) -> Option<&str> {
        match self {
            UpdateCheck::Failed(_, e) => Some(e),
            _ => None,
        }
    }

    pub(super) fn answered(&mut self, result: Result<(), String>) {
        let at = self.since().unwrap_or_else(Instant::now);
        *self = match result {
            Ok(()) => UpdateCheck::Checked(at),
            Err(e) => UpdateCheck::Failed(at, e),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::UpdateCheck;
    use std::time::Instant;

    #[test]
    fn an_update_check_is_clean_only_once_it_answered_without_an_error() {
        let mut check = UpdateCheck::Never;
        assert!(!check.clean() && check.since().is_none());
        let at = Instant::now();
        check = UpdateCheck::Checking(at);
        assert!(check.checking() && !check.clean());
        check.answered(Err("offline".into()));
        assert_eq!(check.error(), Some("offline"));
        assert_eq!(check.since(), Some(at));
        check = UpdateCheck::Checking(at);
        check.answered(Ok(()));
        assert!(check.clean() && check.error().is_none());
    }
}
