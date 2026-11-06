//! The Settings page: the sections as cards in a pane that scrolls on its own under the title,
//! and a list of the sections beside them (a row of tabs under the title in a narrow window, with
//! More tabs for those that don't fit); the list follows the scroll and takes the pane to a section.

use super::*;
use crate::game_run::InstallState;
use crate::tab_strip::StripTab;
use crate::ui::{self, white};
use eclipse_theme::glass::{ink, radius, space, step};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum SettingsSection {
    Account,
    Updates,
    Appearance,
    Language,
    Network,
    Game,
    Ai,
    Folders,
    Developer,
    DataCollection,
}

impl SettingsSection {
    const ALL: [SettingsSection; 10] = [
        SettingsSection::Account,
        SettingsSection::Updates,
        SettingsSection::Appearance,
        SettingsSection::Language,
        SettingsSection::Network,
        SettingsSection::Game,
        SettingsSection::Ai,
        SettingsSection::Folders,
        SettingsSection::Developer,
        SettingsSection::DataCollection,
    ];

    fn shown(developer: bool) -> Vec<SettingsSection> {
        Self::ALL
            .into_iter()
            .filter(|s| match s {
                SettingsSection::Developer => developer,
                SettingsSection::DataCollection => eclipse_telemetry::endpoint().is_some(),
                _ => true,
            })
            .collect()
    }

    fn id(self) -> &'static str {
        match self {
            SettingsSection::Account => "account",
            SettingsSection::Updates => "updates",
            SettingsSection::Appearance => "appearance",
            SettingsSection::Language => "language",
            SettingsSection::Network => "network",
            SettingsSection::Game => "game",
            SettingsSection::Ai => "ai",
            SettingsSection::Folders => "folders",
            SettingsSection::Developer => "developer",
            SettingsSection::DataCollection => "data-collection",
        }
    }

    fn name(self) -> &'static str {
        match self {
            SettingsSection::Account => tr("Account"),
            SettingsSection::Updates => tr("Updates"),
            SettingsSection::Appearance => tr("Appearance"),
            SettingsSection::Language => tr("Language"),
            SettingsSection::Network => tr("Network"),
            SettingsSection::Game => tr("Game"),
            SettingsSection::Ai => tr("AI"),
            SettingsSection::Folders => tr("Folders"),
            SettingsSection::Developer => tr("Developer"),
            SettingsSection::DataCollection => tr("Data collection"),
        }
    }

    fn icon(self) -> &'static str {
        match self {
            SettingsSection::Account => "user",
            SettingsSection::Updates => "refresh",
            SettingsSection::Appearance => "color-picker",
            SettingsSection::Language => "earth",
            SettingsSection::Network => "globe-02",
            SettingsSection::Game => "game-controller-01",
            SettingsSection::Ai => "ai-chat-02",
            SettingsSection::Folders => "folder-01",
            SettingsSection::Developer => "computer-terminal-01",
            SettingsSection::DataCollection => "view",
        }
    }
}

const LANDING_SLACK: f32 = 1.;

#[derive(Clone, Copy)]
pub(super) struct Jump {
    to: SettingsSection,
    landed: bool,
    enter: bool,
}

#[derive(Default)]
pub(super) struct SettingsUi {
    pub(super) pane: Scrollbar,
    /// The section the list marked when last drawn.
    pub(super) marked: usize,
    pub(super) jump: Option<Jump>,
    pub(super) enter: Option<FocusHandle>,
    /// The any-colour swatch shows the hue strip (`settings_appearance.rs`).
    pub(super) accent_strip: bool,
    /// Where the hue strip is on screen, to map a click to a hue.
    pub(super) hue_area: std::rc::Rc<std::cell::Cell<Option<Bounds<gpui::Pixels>>>>,
    /// A drag on that strip is in flight, so a release anywhere writes the accent down.
    pub(super) hue_dragging: bool,
}

fn update_tile(
    lead: AnyElement,
    name: SharedString,
    version: String,
    state: Option<AnyElement>,
    action: Option<AnyElement>,
) -> Div {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .p(px(ui::SETTING_ROW_PADDING))
        .child(div().child(lead))
        .child(ui::card_title(name).mt(px(space::LG)))
        .when(!version.is_empty(), |d| {
            d.child(ui::card_note(version).mt(px(space::XXS)).truncate())
        })
        .children(state.map(|s| div().mt(px(space::MD)).child(s)))
        .children(action.map(|a| div().mt(px(space::LG)).flex().child(a)))
}

fn folder_row(
    id: &'static str,
    name: SharedString,
    open: SharedString,
    dir: std::path::PathBuf,
) -> Div {
    let path = ui::mono(step::CAPTION, ink::MUTED)
        .child(short(&dir))
        .into_any_element();
    let open = ui::open_folder_button(id, open, dir, ui::ButtonSize::Md).into_any_element();
    ui::setting_row(name, Some(path), Some(open))
}

/// The scroll spy: which section is being read, given each section's top from the pane's top
/// (`tops`, in order), the pane's `height`, how far it is `scrolled` and how far it can be (`max`).
/// The last section whose top passed a reading line, which sits at 30 % of the pane and slides
/// down to the pane's bottom over the last 70 % of a pane of scroll: the short last sections,
/// which never reach 30 %, each come in turn as the end nears.
fn spy(tops: &[f32], height: f32, scrolled: f32, max: f32) -> usize {
    if scrolled <= 0. {
        return 0;
    }
    let rest = height * 0.3;
    let travel = height - rest;
    let tail = travel.min(max);
    let line = if tail > 0. {
        rest + travel * ((scrolled - (max - tail)) / tail).clamp(0., 1.)
    } else {
        rest
    };
    tops.iter().rposition(|&top| top <= line + 0.5).unwrap_or(0)
}

impl Launcher {
    pub(super) fn settings_page(&mut self, cx: &mut Context<Self>) -> Div {
        let shown = SettingsSection::shown(self.dev_options_shown());
        let sections: Vec<Stateful<Div>> = shown
            .iter()
            .map(|&section| self.settings_card(section, cx))
            .collect();
        let count = shown.len();
        let jump = self
            .settings_ui
            .jump
            .and_then(|j| Some((j, shown.iter().position(|&s| s == j.to)?)));
        let active = match jump {
            Some((jump, i)) if self.settings_resting_at(i) => {
                if jump.enter && !jump.landed && ui::keyboard_input() {
                    self.settings_ui.enter =
                        ui::try_focus_for(format!("settings-{}", shown[i].id()));
                }
                self.settings_ui.jump = Some(Jump {
                    landed: true,
                    ..jump
                });
                i
            }
            Some((jump, i)) if !jump.landed => {
                self.settings_ui.pane.handle.scroll_to_top_of_item(i);
                cx.notify();
                i
            }
            _ => {
                self.settings_ui.jump = None;
                self.settings_section(count)
            }
        };
        self.settings_ui.marked = active;
        let tabs = self.window_width < layout::SETTINGS_LIST_BESIDE;
        let pane = ui::scroll_pane("settings-pane", &self.settings_ui.pane.handle)
            .size_full()
            // A scroll redraws the pane from its layer without rendering the page again, so the
            // list follows it here: on the next frame, once the offset has moved, and only when
            // the section changed.
            .on_scroll(cx.listener(move |this, _: &gpui::ScrollEvent, window, cx| {
                this.settings_ui.jump = None;
                cx.on_next_frame(window, move |this, _, cx| {
                    if this.settings_section(count) != this.settings_ui.marked {
                        cx.notify();
                    }
                });
            }))
            .rounded(px(radius::PANEL))
            .flex()
            .flex_col()
            .gap(px(layout::CARD_GAP))
            .pb(px(space::SM))
            .children(sections);
        let sky = self.theme.night.sky;
        let clip_line = ui::color(eclipse_theme::night::over(sky[1], 0.5, sky[0]));
        let pane = div()
            .relative()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .child(ui::page_pane(
                pane,
                &self.settings_ui.pane,
                layout::page_margin(self.window_width),
            ))
            .children(ui::more_above_fade(
                &self.settings_ui.pane.handle,
                clip_line,
            ));
        let body = if tabs {
            let named = shown
                .iter()
                .map(|&section| StripTab {
                    value: section,
                    key: section.id().to_string(),
                    label: section.name().into(),
                    count: None,
                })
                .collect();
            let head = self.tab_strip(
                "settings-section",
                named,
                shown[active],
                layout::content_width(self.window_width),
                |this, section, arrowed| {
                    this.settings_ui.jump = Some(Jump {
                        to: section,
                        landed: false,
                        enter: !arrowed,
                    })
                },
                cx,
            );
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .child(head)
                .child(pane)
        } else {
            let entries = shown
                .iter()
                .enumerate()
                .map(|(i, &section)| {
                    let row = ui::nav_row(
                        format!("settings-section-{}", section.id()),
                        section.icon(),
                        section.name(),
                        i == active,
                        ui::NavDensity::List,
                    );
                    (row, section.name().into(), i == active)
                })
                .collect();
            let this = cx.entity().downgrade();
            let picks = shown.clone();
            let rail = ui::options(
                div()
                    .id("settings-sections")
                    .aria_label(tr("Settings sections"))
                    .flex()
                    .flex_col()
                    .flex_none()
                    .self_start()
                    .w(px(layout::SETTINGS_RAIL))
                    .gap(px(space::XXS)),
                entries,
                true,
                move |i, _, cx| {
                    let Some(&section) = picks.get(i) else {
                        return;
                    };
                    let _ = this.update(cx, |this, cx| {
                        this.settings_ui.jump = Some(Jump {
                            to: section,
                            landed: false,
                            enter: i == this.settings_ui.marked,
                        });
                        cx.notify();
                    });
                },
            );
            div()
                .flex()
                .gap(px(layout::CARD_GAP))
                .flex_1()
                .min_h_0()
                .child(rail)
                .child(pane)
        };
        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .child(ui::page_head(Page::Settings.title(), ""))
            .child(body)
    }

    pub(super) fn show_settings(&mut self, section: SettingsSection) {
        self.go(Page::Settings);
        self.settings_ui.jump = Some(Jump {
            to: section,
            landed: false,
            enter: true,
        });
    }

    pub(super) fn proxy_settings_button(id: &str, cx: &mut Context<Self>) -> Stateful<Div> {
        ui::button(
            format!("{id}-proxy"),
            Some("globe-02"),
            tr("Proxy settings"),
            ButtonKind::Secondary,
            ui::ButtonSize::Md,
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.show_settings(SettingsSection::Network);
            cx.notify();
        }))
    }

    fn settings_resting_at(&self, i: usize) -> bool {
        let pane = self.settings_ui.pane.handle.bounds();
        let Some(card) = self.settings_ui.pane.handle.bounds_for_item(i) else {
            return false;
        };
        if pane.size.height <= px(0.) {
            return false;
        }
        let max = f32::from(self.settings_ui.pane.handle.max_offset().y);
        let resting = f32::from(card.top() - pane.top()).clamp(0., max);
        (resting + f32::from(self.settings_ui.pane.handle.offset().y)).abs() < LANDING_SLACK
    }

    pub(super) fn wide_controls_beside(&self) -> bool {
        let content = layout::content_width(self.window_width);
        let list = if self.window_width < layout::SETTINGS_LIST_BESIDE {
            0.
        } else {
            layout::SETTINGS_RAIL + layout::CARD_GAP
        };
        content - list >= layout::SETTINGS_CONTROL_BESIDE
    }

    /// The section the list marks, from where the pane's sections are now ([`spy`]). A section's
    /// bounds are where layout put it, before the scroll (`ScrollHandle` keeps the offset apart):
    /// the scroll is taken off them here. Reading them as they were left the list on the first
    /// section until the pane hit its end.
    fn settings_section(&self, count: usize) -> usize {
        let pane = self.settings_ui.pane.handle.bounds();
        let scrolled = -self.settings_ui.pane.handle.offset().y;
        let tops: Vec<f32> = (0..count)
            .map_while(|i| self.settings_ui.pane.handle.bounds_for_item(i))
            .map(|b| f32::from(b.top() - pane.top() - scrolled))
            .collect();
        spy(
            &tops,
            f32::from(pane.size.height),
            f32::from(scrolled),
            f32::from(self.settings_ui.pane.handle.max_offset().y),
        )
    }

    /// The Updates card's switch for registry mods' automatic updates (`eclipse::auto`).
    fn mods_auto_update_row(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let on = self.settings.auto_updates_mods();
        ui::toggle_row(
            "mods-auto-update",
            tr("Update Eclipse mods automatically"),
            None,
            on,
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.settings.mods_auto_update = Some(!this.settings.auto_updates_mods());
            this.save_settings();
            cx.notify();
        }))
    }

    pub(super) fn native_profile_picker(
        &self,
        id: &'static str,
        cx: &mut Context<Self>,
    ) -> (Stateful<Div>, &'static str) {
        let profile = native_mods::Profile::chosen(&self.settings);
        let picker = ui::segments(
            id,
            tr("Build profile"),
            &ui::translated(&[
                (native_mods::Profile::Performance, "Performance"),
                (native_mods::Profile::Dev, "Dev"),
                (native_mods::Profile::Release, "Release"),
            ]),
            profile,
            {
                let this = cx.entity().downgrade();
                move |choice, _, cx| {
                    let _ = this.update(cx, |this, cx| {
                        this.settings.set_native_profile(Some(choice));
                        this.native.done = None;
                        this.save_settings();
                        cx.notify();
                    });
                }
            },
        );
        let said = match profile {
            native_mods::Profile::Performance => tr("The everyday build for playing with mods"),
            native_mods::Profile::Dev => tr("For mod authors: faster rebuilds, slower game"),
            native_mods::Profile::Release => tr("Slower to build, slightly faster game"),
        };
        (picker, said)
    }

    fn reduce_motion_row(&mut self, cx: &mut Context<Self>) -> Div {
        let system = if crate::motion::system_reduces_motion() {
            "System (On)"
        } else {
            "System (Off)"
        };
        let picker = ui::segments(
            "reduce-motion",
            tr("Reduce motion"),
            &ui::translated(&[(None, system), (Some(true), "On"), (Some(false), "Off")]),
            self.settings.reduce_motion,
            {
                let this = cx.entity().downgrade();
                move |choice, _, cx| {
                    let _ = this.update(cx, |this, cx| {
                        this.settings.reduce_motion = choice;
                        crate::motion::set_reduce_motion(choice);
                        this.save_settings();
                        cx.notify();
                    });
                }
            },
        );
        ui::setting_row(tr("Reduce motion"), None, Some(picker.into_any_element()))
    }

    fn eclipse_update_tile(&mut self, cx: &mut Context<Self>) -> Div {
        let installed = self
            .launcher_version()
            .map(|tag| eclipse_launcher::nightly::display_name(tag));
        let own = self
            .instances
            .list
            .iter()
            .find(|i| i.kind == instances::Kind::Default);
        let busy = own.and_then(|i| self.play_state(i).busy_line());
        let checked =
            self.checked.is_some() && !self.checking_updates && self.update_error.is_none();
        let state = match (busy, own.map(|i| self.install_state(i))) {
            (Some(line), _) => Some(ui::state_note(ui::Tone::Busy, line)),
            (None, Some(InstallState::UpdateReady(tag))) => Some(ui::state_note(
                ui::Tone::Accent(ui::look().accent_soft),
                trf(
                    "{version} available",
                    &[("version", &eclipse_launcher::nightly::display_name(&tag))],
                ),
            )),
            (None, Some(InstallState::NotInstalled) | None) => {
                Some(ui::state_note(ui::Tone::Off, tr("Not installed")))
            }
            (None, Some(_)) if checked => Some(ui::state_note(ui::Tone::Live, tr("Up to date"))),
            _ => None,
        };
        let install = self
            .updates
            .eclipse
            .as_ref()
            .filter(|_| self.installing.is_none())
            .map(|_| {
                ui::button(
                    "settings-install",
                    None,
                    tr("Install"),
                    ButtonKind::Secondary,
                    ui::ButtonSize::Md,
                )
                .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.install_update();
                    cx.notify();
                }))
                .into_any_element()
            });
        update_tile(
            ui::mark_tile(self.eclipse_mark(ui::MARK)).into_any_element(),
            "Eclipse".into(),
            installed.unwrap_or_default(),
            state.map(IntoElement::into_any_element),
            install,
        )
    }

    fn launcher_update_tile(&mut self, cx: &mut Context<Self>) -> Div {
        let available = |version: &str| {
            ui::state_note(
                ui::Tone::Accent(ui::look().accent_soft),
                trf("{version} available", &[("version", version)]),
            )
        };
        let state = match &self.self_update {
            SelfUpdate::Downloading(version, done) => Some(ui::state_note(
                ui::Tone::Busy,
                trf(
                    "Downloading {version}, {percent}%",
                    &[
                        ("version", version),
                        ("percent", &format!("{:.0}", done * 100.)),
                    ],
                ),
            )),
            SelfUpdate::Ready(version) => Some(available(version)),
            SelfUpdate::Stuck(None) => Some(ui::state_note(
                ui::Tone::Warn,
                self.launcher_update_help(None),
            )),
            SelfUpdate::Stuck(Some(e)) | SelfUpdate::Failed(e) => Some(ui::error_note(trf(
                "The update failed: {error}",
                &[("error", ui::plain_error(e))],
            ))),
            SelfUpdate::Idle => match &self.updates.launcher {
                Some(r) => Some(available(
                    backend::launcher_version(&r.tag).unwrap_or_default(),
                )),
                None if self.checked.is_some()
                    && !self.checking_updates
                    && self.update_error.is_none() =>
                {
                    Some(ui::state_note(ui::Tone::Live, tr("Up to date")))
                }
                None => None,
            },
        };
        let action = match &self.self_update {
            SelfUpdate::Ready(_) => Some(
                ui::button(
                    "update-settings",
                    None,
                    tr("Update"),
                    ButtonKind::Secondary,
                    ui::ButtonSize::Md,
                )
                .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.apply_launcher_update(cx);
                    cx.notify();
                }))
                .into_any_element(),
            ),
            SelfUpdate::Stuck(_) if self.launcher_download_available() => {
                let url = backend::download_page(&self.links.website);
                Some(
                    ui::button(
                        "download-settings",
                        None,
                        tr("Download again"),
                        ButtonKind::Secondary,
                        ui::ButtonSize::Md,
                    )
                    .on_press(move |_, _, cx| cx.open_url(&url))
                    .into_any_element(),
                )
            }
            _ => None,
        };
        update_tile(
            ui::tile(ui::TILE_LG)
                .child(ui::icon(
                    "download-04",
                    ui::IconSize::Px24,
                    white(ink::BODY),
                ))
                .into_any_element(),
            tr("Launcher").into(),
            backend::VERSION.to_string(),
            state.map(IntoElement::into_any_element),
            action,
        )
    }

    fn check_updates_row(&mut self, cx: &mut Context<Self>) -> Div {
        let checking = self.checking_updates;
        let failed = self.update_error.clone().filter(|_| !checking);
        let check = ui::button(
            "check-updates",
            failed.is_some().then_some("refresh"),
            tr(if checking {
                "Checking…"
            } else if failed.is_some() {
                "Try again"
            } else {
                "Check for updates"
            }),
            ButtonKind::Secondary,
            ui::ButtonSize::Md,
        );
        let check = if checking {
            ui::disabled(check)
        } else {
            check.on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.check_updates();
                cx.notify();
            }))
        };
        if let Some(e) = failed {
            return div()
                .p(px(ui::SETTING_ROW_PADDING))
                .child(ui::error_line_for(
                    "check-updates-failed",
                    tr("Couldn't check for updates"),
                    &e,
                    Some(check),
                    vec![Self::proxy_settings_button("check-updates", cx)],
                ));
        }
        let beside = match self.checked {
            _ if checking => None,
            Some(at) => {
                let when = dates::ago(std::time::SystemTime::now() - at.elapsed());
                Some(
                    ui::card_note(trf("Last checked {when}", &[("when", &when)]))
                        .into_any_element(),
                )
            }
            None => None,
        };
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(space::MD))
            .p(px(ui::SETTING_ROW_PADDING))
            .child(check)
            .children(beside)
    }

    fn settings_card(&mut self, section: SettingsSection, cx: &mut Context<Self>) -> Stateful<Div> {
        let id = section.id();
        let card = ui::setting_card(format!("settings-{id}"), section.name()).when_some(
            ui::try_focus_for(format!("settings-{id}")),
            |card, handle| card.track_focus(&handle.tab_stop(false)),
        );
        match section {
            SettingsSection::Account => card.child(self.account_card(cx)),
            SettingsSection::Updates => card
                .child(
                    div().flex().child(self.eclipse_update_tile(cx)).child(
                        self.launcher_update_tile(cx)
                            .border_l_1()
                            .border_color(ui::edge()),
                    ),
                )
                .child(self.check_updates_row(cx))
                .child(self.auto_install_row(cx))
                .child(self.auto_prune_row(cx))
                .child(self.mods_auto_update_row(cx)),
            SettingsSection::Appearance => card
                .child(self.colour_mode_row(cx))
                .child(self.accent_row(cx))
                .child(self.launcher_theme_row(cx))
                .child(self.home_background_row(cx))
                .child(self.reduce_motion_row(cx))
                .child(self.reset_look_row(cx)),
            SettingsSection::Language => card.child(self.language_row(cx)),
            SettingsSection::Network => card.child(self.proxy_row(cx)),
            SettingsSection::Game => card
                .child(self.while_playing_row(cx))
                .when(cfg!(target_os = "linux"), |d| {
                    d.child(self.menu_entry_row(cx))
                })
                .child(self.game_architecture_row(cx))
                .child(self.game_args_row(cx)),
            SettingsSection::Ai => card
                .child(self.ai_features_row(cx))
                .children(self.agents_section(cx)),
            SettingsSection::Folders => card
                .child(self.minecraft_folder_row(cx))
                .child(folder_row(
                    "open-data",
                    tr("Data folder").into(),
                    tr("Open data folder").into(),
                    backend::data_dir(),
                ))
                .child(folder_row(
                    "open-logs",
                    tr("Logs").into(),
                    tr("Open logs folder").into(),
                    backend::logs_dir(),
                )),
            SettingsSection::Developer => card.child(self.dev_offline_row(cx)),
            SettingsSection::DataCollection => card.child(self.collection_section(cx)),
        }
    }
}

impl Launcher {
    fn minecraft_folder_row(&mut self, cx: &mut Context<Self>) -> Div {
        let dir = self
            .settings
            .minecraft_dir
            .clone()
            .unwrap_or_else(|| eclipse_prepare::artifacts::minecraft_dir(None));
        let path = ui::mono(step::CAPTION, ink::MUTED)
            .child(short(&dir))
            .into_any_element();
        let choose = ui::button(
            "minecraft-folder-choose",
            Some("folder-01"),
            tr("Choose folder…"),
            ButtonKind::Secondary,
            ui::ButtonSize::Md,
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.choose_folder(cx);
            cx.notify();
        }))
        .into_any_element();
        ui::setting_row(tr("Minecraft folder"), Some(path), Some(choose))
    }

    fn auto_install_row(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        ui::toggle_row(
            "auto-install",
            tr("Install new Eclipse versions automatically"),
            None,
            self.settings.auto_install(),
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.settings.install_new_versions = Some(!this.settings.auto_install());
            this.save_settings();
            if this.settings.auto_install() {
                this.install_update();
            }
            cx.notify();
        }))
    }

    fn auto_prune_row(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let keeps = trf(
            "Keeps the {count} newest and any version an instance plays.",
            &[("count", &self.settings.keep_newest().to_string())],
        );
        ui::toggle_row(
            "auto-prune",
            tr("Delete old Eclipse versions automatically"),
            Some(keeps.into()),
            self.settings.auto_prune(),
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.settings.prune_old_versions = Some(!this.settings.auto_prune());
            this.save_settings();
            cx.notify();
        }))
    }

    fn menu_entry_row(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        // Linux: the launcher's entry in the applications menu (`eclipse_launcher::desktop`).
        ui::toggle_row(
            "desktop-entry",
            tr("Show in applications menu"),
            None,
            self.settings.desktop_entry(),
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            let on = !this.settings.desktop_entry();
            this.settings.desktop_entry = Some(on);
            this.save_settings();
            let _ = if on {
                eclipse_launcher::desktop::install(backend::exe_path()).map(|_| ())
            } else {
                eclipse_launcher::desktop::remove()
            };
            cx.notify();
        }))
    }

    fn colour_mode_row(&mut self, cx: &mut Context<Self>) -> Div {
        let appearance = ui::segments(
            "appearance",
            tr("Colour mode"),
            &ui::translated(&[
                (backend::Appearance::Dark, "Dark"),
                (backend::Appearance::Light, "Light"),
                (backend::Appearance::System, "System"),
            ]),
            self.settings.appearance,
            {
                let this = cx.entity().downgrade();
                move |choice, window, cx| {
                    let _ = this.update(cx, |this, cx| {
                        this.settings.appearance = choice;
                        this.save_settings();
                        titlebar::apply_appearance(window, choice);
                        this.theme = this.settings_theme(window);
                        cx.notify();
                    });
                }
            },
        );
        ui::setting_row(tr("Colour mode"), None, Some(appearance.into_any_element()))
    }

    /// Settings' Windows ARM game architecture choice; release installation, updates and launch
    /// use the saved choice, while launcher updates continue to follow this process.
    pub(super) fn game_architecture_row(&self, cx: &mut Context<Self>) -> Div {
        if !backend::can_choose_game_architecture() {
            return div();
        }
        let picker = crate::ui::segments(
            "game-architecture",
            tr("Game architecture"),
            &ui::translated(&[
                (backend::GameArchitecture::Auto, "Automatic"),
                (backend::GameArchitecture::X64, "x64"),
                (backend::GameArchitecture::Arm64, "ARM64"),
            ]),
            self.settings.game_architecture,
            {
                let this = cx.entity().downgrade();
                move |choice, _, cx| {
                    let _ = this.update(cx, |this, cx| {
                        if this.settings.game_architecture == choice {
                            return;
                        }
                        this.settings.game_architecture = choice;
                        this.save_settings();
                        // The release and update lists contain the selected game's assets.
                        this.releases = Load::Loading;
                        this.updates = backend::Updates::default();
                        this.fetch_startup_releases();
                        cx.notify();
                    });
                }
            },
        );
        crate::ui::setting_row(
            tr("Game architecture"),
            Some(
                div()
                    .child(tr("Only change this on Windows on ARM."))
                    .into_any_element(),
            ),
            Some(picker.into_any_element()),
        )
    }

    pub(super) fn game_args_row(&mut self, cx: &mut Context<Self>) -> Div {
        let field = self
            .field(
                Field::GameArgs,
                tr("For example --server-thread"),
                layout::FIELD_XL,
                ui::ButtonSize::Md,
                cx,
            )
            .w_full()
            .max_w(px(layout::FIELD_XL));
        ui::setting_row_stacked(tr("Game arguments"), None, field.into_any_element())
    }

    /// Settings' Proxy row: the proxy every download and sign-in goes through, kept in
    /// `settings.json` and read back by `proxy.rs` on each request.
    pub(super) fn proxy_row(&mut self, cx: &mut Context<Self>) -> Div {
        let focused = self.fields.has_focus(Field::Proxy);
        let field = self
            .field(
                Field::Proxy,
                tr("For example http://127.0.0.1:7890"),
                layout::FIELD_XL,
                ui::ButtonSize::Md,
                cx,
            )
            .w_full()
            .max_w(px(layout::FIELD_XL));
        // A half-typed address is no reason to complain; one left behind is.
        let detail = match (
            focused,
            eclipse_launcher::proxy::refuse(self.fields.value(Field::Proxy)),
        ) {
            (false, Some(why)) => Some(ui::error_note(why).into_any_element()),
            _ => None,
        };
        ui::setting_row_stacked(tr("Proxy"), detail, field.into_any_element())
    }

    /// Keeps the Proxy field in `settings.json`, and tells the parts that build their own agents
    /// (`eclipse-port-pack`) which proxy to use.
    pub(super) fn save_proxy(&mut self) {
        let value = self.fields.value(Field::Proxy).trim().to_owned();
        self.settings.proxy = (!value.is_empty()).then_some(value);
        self.save_settings();
        eclipse_launcher::proxy::announce(self.settings.proxy.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::spy;

    /// A 600 px pane over eight cards: tall ones, then Folders and Data collection, too short for
    /// their tops ever to reach 30 % of the pane.
    const TOPS: [f32; 8] = [0., 420., 700., 1000., 1300., 1600., 1900., 2100.];
    const HEIGHT: f32 = 600.;
    const MAX: f32 = 2300. - HEIGHT;

    fn at(scrolled: f32) -> usize {
        let tops: Vec<f32> = TOPS.iter().map(|t| t - scrolled).collect();
        spy(&tops, HEIGHT, scrolled, MAX)
    }

    #[test]
    fn the_list_follows_the_section_read_at_30_percent() {
        assert_eq!(at(0.), 0);
        assert_eq!(
            at(300.),
            1,
            "Updates' top is 120 px down: in the upper 30 %"
        );
        assert_eq!(at(600.), 2);
        assert_eq!(
            at(1000.),
            3,
            "Network's top at the pane's top, Game's 300 px down"
        );
        assert_eq!(at(1150.), 4, "Game's top 150 px down: in the upper 30 %");
    }

    #[test]
    fn near_the_end_each_short_section_comes_in_turn() {
        let seen: Vec<usize> = (0..=MAX as usize)
            .step_by(10)
            .map(|s| at(s as f32))
            .collect();
        // Every section is marked on the way down, in order, never skipped.
        let mut order = seen.clone();
        order.dedup();
        assert_eq!(order, (0..8).collect::<Vec<_>>());
        // Folders is marked for a stretch of scroll before the end, not for one step.
        let folders = seen.iter().filter(|&&s| s == 6).count();
        assert!(folders >= 8, "Folders marked for {folders} steps of 10 px");
        assert_eq!(at(MAX), 7);
    }

    #[test]
    fn a_pane_that_does_not_scroll_marks_the_first() {
        assert_eq!(spy(&[0., 200.], 600., 0., 0.), 0);
        assert_eq!(spy(&[], 600., 0., 0.), 0);
    }

    #[test]
    fn a_short_first_section_stays_marked_at_the_top() {
        assert_eq!(spy(&[0., 150., 400.], 600., 0., 900.), 0);
    }
}
