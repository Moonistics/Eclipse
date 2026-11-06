//! "Add from GitHub" on the Mods page: a link to a repository, release, tag, branch or commit,
//! resolved to an exact release file or commit (`eclipse_launcher::github_mods`), shown with the
//! plain warning and a link to the exact source, and installed only after the player says they
//! trust it. Mods installed that way list where they came from, with "Check for updates".
use super::*;
use crate::menus::Asking;
use crate::ui::{self};
use eclipse_launcher::github_mods::{self, Origin, Packed, Resolved};
use eclipse_theme::glass::{ink, radius, space, step};
use std::path::{Path, PathBuf};

enum Event {
    /// A link (or an update check for a mod id) resolved.
    Resolved(Result<Resolved, String>, Option<String>),
    /// The mods of a source, fetched and checked, when there is a choice to make.
    Fetched(Resolved, Result<Vec<Packed>, String>),
    Installed(Result<String, String>),
}

enum Stage {
    Idle,
    Working(String),
    /// The warning and the source link, before anything is downloaded. `only`: the mod an update
    /// is for.
    Review(Resolved, Option<String>),
    Pick(Resolved, Vec<Packed>),
}

const LINK_FIELD: f32 = 460.;

pub(super) struct Github {
    stage: Stage,
    message: Option<Result<String, String>>,
    /// The mods installed from GitHub in `home`, and what the last update check found for each.
    home: Option<PathBuf>,
    newer: std::collections::HashMap<String, Resolved>,
    tx: Sender<Event>,
    rx: Receiver<Event>,
}

impl Github {
    pub(super) fn new() -> Self {
        let (tx, rx) = channel();
        Self {
            stage: Stage::Idle,
            message: None,
            home: None,
            newer: Default::default(),
            tx,
            rx,
        }
    }

    pub(super) fn busy(&self) -> bool {
        matches!(self.stage, Stage::Working(_))
    }

    fn reread(&mut self, home: &Path) {
        self.home = Some(home.to_owned());
    }
}

/// Installs the chosen mods of `mods` and says what was installed.
fn finish(
    home: &Path,
    found: &Resolved,
    mods: &[Packed],
    only: Option<&str>,
    all: bool,
) -> Result<String, String> {
    let done = github_mods::choose(mods, only, all)?
        .into_iter()
        .map(|p| github_mods::install(home, found, p))
        .collect::<Result<Vec<_>, _>>()?;
    let names: Vec<String> = done
        .iter()
        .map(|o| format!("{} {}", o.id, o.version))
        .collect();
    Ok(trf(
        "Installed {mods} from GitHub ({source}). Build it on the Mods page.",
        &[("mods", &names.join(", ")), ("source", &found.label())],
    ))
}

impl Launcher {
    pub(super) fn github_tick(&mut self) -> bool {
        let mut changed = false;
        while let Ok(event) = self.github.rx.try_recv() {
            changed = true;
            match event {
                Event::Resolved(Err(e), _) => {
                    self.github.stage = Stage::Idle;
                    self.github.message = Some(Err(e));
                }
                Event::Resolved(Ok(found), None) => self.github.stage = Stage::Review(found, None),
                Event::Resolved(Ok(found), Some(id)) => {
                    self.github.stage = Stage::Idle;
                    let have = github_mods::origin(&self.mods.home(), &id);
                    if have
                        .as_ref()
                        .is_some_and(|o| github_mods::is_update(o, &found))
                    {
                        self.github.newer.insert(id, found);
                    } else {
                        self.github.message = Some(Ok(trf("{id} is up to date.", &[("id", &id)])));
                    }
                }
                Event::Fetched(found, Ok(mods)) => self.github.stage = Stage::Pick(found, mods),
                Event::Fetched(_, Err(e)) => {
                    self.github.stage = Stage::Idle;
                    self.github.message = Some(Err(e));
                }
                Event::Installed(result) => {
                    self.github.stage = Stage::Idle;
                    if result.is_ok() {
                        self.fields.text(Field::GithubLink).set(String::new());
                        self.github.newer.clear();
                        // The review above was the trust notice; the Mods page offers Build.
                        self.settings.native_mods_noticed = true;
                        self.save_settings();
                    }
                    self.instance_mods_changed();
                    self.github.message = Some(result);
                    let home = self.mods.home();
                    self.github.reread(&home);
                }
            }
        }
        changed
    }

    /// Resolves what the field holds, through GitHub's API on a thread.
    fn github_resolve(&mut self, link: github_mods::Link, update_of: Option<String>) {
        if self.github.busy() {
            return;
        }
        self.github.message = None;
        self.github.stage = Stage::Working(trf(
            "Asking GitHub about {owner}/{repo}…",
            &[("owner", &link.owner), ("repo", &link.repo)],
        ));
        let tx = self.github.tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Event::Resolved(
                github_mods::resolve(&link, &github_mods::api),
                update_of,
            ));
        });
    }

    pub(super) fn github_add(&mut self) {
        match github_mods::parse(self.fields.value(Field::GithubLink)) {
            Ok(link) => self.github_resolve(link, None),
            Err(e) => self.github.message = Some(Err(e)),
        }
    }

    /// Downloads and checks what the player agreed to; installs it unless there is a choice.
    fn github_install(&mut self, found: Resolved, only: Option<String>) {
        self.github.stage =
            Stage::Working(trf("Downloading {source}…", &[("source", &found.label())]));
        let tx = self.github.tx.clone();
        let home = self.mods.home();
        std::thread::spawn(move || {
            let fetched = github_mods::fetch(&found);
            let result = match fetched {
                Ok(mods) if mods.len() == 1 || only.is_some() => {
                    let done = finish(&home, &found, &mods, only.as_deref(), false);
                    github_mods::discard(&found);
                    Event::Installed(done)
                }
                other => Event::Fetched(found, other),
            };
            let _ = tx.send(result);
        });
    }

    fn github_pick(&mut self, found: Resolved, mods: Vec<Packed>, id: Option<String>) {
        let home = self.mods.home();
        let done = finish(&home, &found, &mods, id.as_deref(), id.is_none());
        github_mods::discard(&found);
        let _ = self.github.tx.send(Event::Installed(done));
        self.github.stage = Stage::Working(tr("Installing…").into());
    }

    fn github_field(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        self.field(
            Field::GithubLink,
            "https://github.com/owner/repo",
            LINK_FIELD,
            ui::ButtonSize::Md,
            cx,
        )
        .flex_1()
        .min_w_0()
        .max_w(px(LINK_FIELD))
    }

    /// The Add from GitHub panel. Installed mods share the main list.
    pub(super) fn github_panel(&mut self, cx: &mut Context<Self>) -> Div {
        let home = self.mods.home();
        if self.github.home.as_deref() != Some(home.as_path()) {
            self.github.reread(&home);
            self.github.newer.clear();
        }
        let busy = self.github.busy();
        let mut add = ui::button(
            "github-add",
            None,
            tr("Add from GitHub"),
            ButtonKind::Secondary,
            ui::ButtonSize::Md,
        );
        add = match busy {
            true => ui::disabled(add),
            false => add.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.github_add();
                cx.notify();
            })),
        };
        let mut panel = div()
            .flex()
            .flex_col()
            .gap(px(space::MD))
            .child(ui::card_title(tr("Add a native mod from GitHub")))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::MD))
                    .child(self.github_field(cx))
                    .child(add),
            );
        match std::mem::replace(&mut self.github.stage, Stage::Idle) {
            Stage::Idle => {}
            Stage::Working(line) => {
                self.github.stage = Stage::Working(line.clone());
                panel =
                    panel.child(ui::text(step::NOTE, FontWeight::NORMAL, ink::BODY).child(line));
            }
            Stage::Review(found, only) => {
                panel = panel.child(self.github_review(&found, only.clone(), cx));
                self.github.stage = Stage::Review(found, only);
            }
            Stage::Pick(found, mods) => {
                panel = panel.child(self.github_choice(&found, &mods, cx));
                self.github.stage = Stage::Pick(found, mods);
            }
        }
        match &self.github.message {
            Some(Ok(done)) => {
                panel = panel
                    .child(ui::text(step::BODY, FontWeight::NORMAL, ink::FULL).child(done.clone()))
            }
            Some(Err(e)) => panel = panel.child(ui::error_note(e.clone())),
            None => {}
        }
        panel
    }

    /// The warning, the exact source, and the only way forward: saying they trust it.
    fn github_review(&self, found: &Resolved, only: Option<String>, cx: &mut Context<Self>) -> Div {
        let what = match (&found.asset, &found.content_only) {
            (Some(asset), _) => trf(
                "The release file {file} ({source}).",
                &[("file", &asset.name), ("source", &found.label())],
            ),
            (None, Some(file)) => trf(
                "The source of {source}. Its release ships {file}, a content mod: installing those from GitHub is not supported yet.",
                &[("source", &found.label()), ("file", file)],
            ),
            (None, None) => trf(
                "The source at commit {commit}.",
                &[("commit", &found.commit)],
            ),
        };
        let (url, open) = (found.source_url(), found.source_url());
        let install = found.clone();
        div()
            .flex()
            .flex_col()
            .gap(px(space::MD))
            .p(px(ui::PANEL_INSET))
            .rounded(px(radius::ROW))
            .bg(ui::inset())
            .child(ui::error_note(tr("Not reviewed by the Eclipse registry")))
            .child(
                ui::text(step::BODY, FontWeight::NORMAL, ink::FULL)
                    .flex()
                    .flex_col()
                    .gap(px(space::XS))
                    .child(what)
                    .child(tr(
                        "Native mods run with full access to this computer. Read the source before you install it.",
                    )),
            )
            .child(
                ui::text(step::NOTE, FontWeight::NORMAL, ink::MUTED)
                    .font_family(theme::MONO)
                    .child(url),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(space::SM))
                    .child(
                        ui::button("github-trust", None, tr("I read it and trust it: install"), ButtonKind::Secondary, ui::ButtonSize::Md)
                            .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.github_install(install.clone(), only.clone());
                                cx.notify();
                            })),
                    )
                    .child(
                        ui::button("github-source", None, tr("Open the source"), ButtonKind::Secondary, ui::ButtonSize::Md)
                            .on_press(move |_, _, cx| cx.open_url(&open)),
                    )
                    .child(
                        ui::button("github-cancel", None, tr("Cancel"), ButtonKind::Ghost, ui::ButtonSize::Md).on_press(cx.listener(
                            |this, _: &ClickEvent, _, cx| {
                                this.github.stage = Stage::Idle;
                                cx.notify();
                            },
                        )),
                    ),
            )
    }

    /// A source with several mods: one, or all.
    fn github_choice(&self, found: &Resolved, mods: &[Packed], cx: &mut Context<Self>) -> Div {
        let mut buttons = div().flex().flex_wrap().gap(px(space::SM));
        for p in mods {
            let (found, all, id) = (found.clone(), mods.to_vec(), p.id.clone());
            buttons = buttons.child(
                ui::button(
                    SharedString::from(format!("github-pick-{id}")),
                    None,
                    format!("{} {}", p.id, p.version),
                    ButtonKind::Secondary,
                    ui::ButtonSize::Md,
                )
                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.github_pick(found.clone(), all.clone(), Some(id.clone()));
                    cx.notify();
                })),
            );
        }
        let (every, listed) = (found.clone(), mods.to_vec());
        buttons = buttons.child(
            ui::button(
                "github-pick-all",
                None,
                tr("Install all"),
                ButtonKind::Secondary,
                ui::ButtonSize::Md,
            )
            .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.github_pick(every.clone(), listed.clone(), None);
                cx.notify();
            })),
        );
        div()
            .flex()
            .flex_col()
            .gap(px(space::MD))
            .p(px(ui::PANEL_INSET))
            .rounded(px(radius::ROW))
            .bg(ui::inset())
            .child(
                ui::text(step::BODY, FontWeight::NORMAL, ink::FULL).child(trf(
                    "{name} holds {count} mods. Install which?",
                    &[("name", &found.name()), ("count", &mods.len().to_string())],
                )),
            )
            .child(buttons)
    }

    /// GitHub-specific actions, shared by both installed-mod lists.
    fn github_actions(&self, origin: &Origin, busy: bool, cx: &Context<Self>) -> Div {
        let id = origin.id.clone();
        let mut right = div().flex().flex_wrap().items_center().gap(px(space::SM));
        let known = self
            .github
            .newer
            .get(&id)
            .filter(|_| self.github.home.as_deref() == Some(self.mods.home().as_path()))
            .cloned();
        let main = match &known {
            Some(latest) => {
                let (latest, only) = (latest.clone(), id.clone());
                ui::button(
                    SharedString::from(format!("github-update-{id}")),
                    None,
                    trf("Update to {version}", &[("version", &latest.label())]),
                    ButtonKind::Secondary,
                    ui::ButtonSize::Md,
                )
                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.go(Page::Mods);
                    this.github.stage = Stage::Review(latest.clone(), Some(only.clone()));
                    cx.notify();
                }))
            }
            None => {
                let (link, only) = (origin.follows(), id.clone());
                let check = ui::button(
                    SharedString::from(format!("github-check-{id}")),
                    None,
                    tr("Check for updates"),
                    ButtonKind::Secondary,
                    ui::ButtonSize::Md,
                );
                match busy {
                    true => ui::disabled(check),
                    false => check.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.go(Page::Mods);
                        this.github_resolve(link.clone(), Some(only.clone()));
                        cx.notify();
                    })),
                }
            }
        };
        right = right.child(main);
        let url = origin.source_url();
        right = right.child(
            ui::button(
                SharedString::from(format!("github-src-{id}")),
                None,
                tr("Source"),
                ButtonKind::Secondary,
                ui::ButtonSize::Md,
            )
            .on_press(move |_, _, cx| cx.open_url(&url)),
        );
        right
    }

    pub(super) fn native_mod_actions(
        &self,
        row: &native_status::Installed,
        busy: bool,
        cx: &Context<Self>,
    ) -> Div {
        let busy = busy || self.github.busy();
        let mut right = row.origin.as_ref().map_or_else(
            || div().flex().items_center().gap(px(space::SM)),
            |origin| self.github_actions(origin, busy, cx),
        );
        let home = self.mods.home();
        let on = eclipse_launcher::browser::java::is_enabled(&home, &row.id);
        let id = row.id.clone();
        right = right.child(
            crate::ui::toggle(format!("native-switch-{id}"), on).on_press(cx.listener(
                move |this, _: &ClickEvent, _, cx| {
                    let home = this.mods.home();
                    // An update holds the folder: the window doesn't wait for it.
                    let Some(_lock) = eclipse_launcher::browser::try_lock_home(&home) else {
                        this.mods.message =
                            Some(Err(tr("Wait for the current operation to finish.").into()));
                        cx.notify();
                        return;
                    };
                    this.mods.message =
                        eclipse_launcher::browser::java::set_enabled(&home, &id, !on)
                            .err()
                            .map(Err);
                    this.instance_mods_changed();
                    cx.notify();
                },
            )),
        );
        let confirming = self.is_asking(&Asking::GithubMod(row.file.clone()));
        let remove_id = SharedString::from(format!("native-remove-{}", row.file));
        if confirming && !busy {
            let row = row.clone();
            return div().flex_none().child(ui::confirm(
                remove_id,
                trf("Remove {name}?", &[("name", &row.id)]),
                tr("Remove"),
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.asking = None;
                    cx.notify();
                }),
                cx.listener(move |this, _: &ClickEvent, _, cx| {
                    let home = this.mods.home();
                    let result = if row.origin.is_some() {
                        github_mods::remove(&home, &row.id).map(|_| ())
                    } else {
                        std::fs::remove_file(home.join(&row.file)).map_err(|e| e.to_string())
                    };
                    this.mods.message =
                        Some(result.map(|_| trf("Removed {name}.", &[("name", &row.id)])));
                    this.github.reread(&home);
                    this.asking = None;
                    this.instance_mods_changed();
                    cx.notify();
                }),
            ));
        }
        let remove = ui::remove_button(remove_id, tr("Remove…"), ui::ButtonSize::Md);
        let file = row.file.clone();
        right = right.child(if busy {
            ui::disabled(remove)
        } else {
            remove.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.asking = Some(Asking::GithubMod(file.clone()));
                cx.notify();
            }))
        });
        right
    }

    pub(super) fn native_mod_row(
        &self,
        row: &native_status::Installed,
        busy: bool,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let source = row.origin.as_ref().map_or_else(
            || trf("Local file · {file}", &[("file", &row.file)]),
            |o| trf("from GitHub: {source}", &[("source", &o.label())]),
        );
        let confirming = self.is_asking(&Asking::GithubMod(row.file.clone()));
        ui::list_row(
            SharedString::from(format!("native-row-{}", row.id)),
            confirming,
        )
        .child(icons::slot(None, &row.id, mods_page::MOD_TILE))
        .child(ui::item_text(
            row.id.clone(),
            format!("{} · {source}", row.version),
        ))
        .child(self.native_mod_actions(row, busy, cx).flex_none())
    }
}
