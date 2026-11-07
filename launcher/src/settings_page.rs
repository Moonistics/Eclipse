//! The Settings page: its sections, one open at a time.

use crate::fields::Field;
use crate::launcher_update::SelfUpdate;
use crate::page::Page;
use crate::play_state::PlayState;
use crate::scrollbar::Scrollbar;
use crate::tab_strip::{Strip, StripTab};
use crate::ui::{self, ink, ButtonKind, Press};
use crate::{dates, layout, scene, short, Launcher};
use eclipse_launcher::i18n::{tr, trf};
use eclipse_launcher::{backend, instances};
use eclipse_theme::glass::{ink, radius, space, step};
use gpui::prelude::*;
use gpui::{div, px, AnyElement, ClickEvent, Div, FocusHandle, SharedString, Stateful};
use std::cell::Cell;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(super) enum SettingsSection {
    #[default]
    Account,
    Updates,
    Appearance,
    Language,
    Network,
    Game,
    Ai,
    System,
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
        SettingsSection::System,
        SettingsSection::Developer,
        SettingsSection::DataCollection,
    ];

    pub(super) fn shown(developer: bool) -> Vec<SettingsSection> {
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
            SettingsSection::System => "system",
            SettingsSection::Developer => "developer",
            SettingsSection::DataCollection => "data-collection",
        }
    }

    fn element_id(self) -> String {
        format!("settings-section-{}", self.id())
    }

    pub(super) fn name(self) -> &'static str {
        match self {
            SettingsSection::Account => tr("Account"),
            SettingsSection::Updates => tr("Updates"),
            SettingsSection::Appearance => tr("Appearance"),
            SettingsSection::Language => tr("Language"),
            SettingsSection::Network => tr("Network"),
            SettingsSection::Game => tr("Game"),
            SettingsSection::Ai => tr("AI"),
            SettingsSection::System => tr("System"),
            SettingsSection::Developer => tr("Developer"),
            SettingsSection::DataCollection => tr("Data collection"),
        }
    }

    fn icon(self) -> &'static str {
        match self {
            SettingsSection::Account => crate::icons::ACCOUNT,
            SettingsSection::Updates => crate::icons::UPDATES,
            SettingsSection::Appearance => crate::icons::APPEARANCE,
            SettingsSection::Language => crate::icons::LANGUAGE,
            SettingsSection::Network => crate::icons::NETWORK,
            SettingsSection::Game => crate::icons::GAME,
            SettingsSection::Ai => crate::icons::AI,
            SettingsSection::System => crate::icons::FOLDERS,
            SettingsSection::Developer => crate::icons::DEVELOPER,
            SettingsSection::DataCollection => crate::icons::DATA_COLLECTION,
        }
    }
}

#[derive(Default)]
pub(super) struct SettingsUi {
    pub(super) pane: Scrollbar,
    pub(super) open: SettingsSection,
    pub(super) entering: bool,
    pub(super) enter: Option<FocusHandle>,
    /// The any-colour swatch shows the hue strip (`settings_appearance.rs`).
    pub(super) accent_strip: bool,
    /// A drag on that strip is in flight, so a release anywhere writes the accent down.
    pub(super) hue_dragging: bool,
}

fn update_row(
    lead: AnyElement,
    name: SharedString,
    version: String,
    state: AnyElement,
    action: Option<AnyElement>,
) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(space::LG))
        .p(px(ui::SETTING_ROW_PADDING))
        .child(lead)
        .child(ui::item_text_by(name, version, state))
        .children(action.map(|a| div().flex_none().child(a)))
}

fn track_content_beside(
    scene: &gpui::Entity<scene::Scene>,
    rail_end: Rc<Cell<f32>>,
) -> impl IntoElement {
    let scene = scene.clone();
    gpui::canvas(
        move |bounds, _, cx| {
            let bottom = f32::from(bounds.bottom()).max(rail_end.get());
            cx.defer(move |cx| {
                scene.update(cx, |scene, cx| scene.set_content_bottom(bottom, cx));
            });
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
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

impl Launcher {
    pub(super) fn settings_page(&mut self, cx: &mut Context<Self>) -> Div {
        let shown = SettingsSection::shown(self.dev_options_shown());
        let open = if shown.contains(&self.settings_ui.open) {
            self.settings_ui.open
        } else {
            shown[0]
        };
        let rail_end = Rc::new(Cell::new(0.));
        let card = self
            .settings_card(open, cx)
            .child(track_content_beside(&self.scene, rail_end.clone()));
        if std::mem::take(&mut self.settings_ui.entering) && ui::keyboard_input() {
            self.settings_ui.enter = ui::try_focus_for(format!("settings-{}", open.id()));
        }
        let tabs = self.window_width < layout::SETTINGS_LIST_BESIDE;
        let scrolled = ui::scrolled(&self.settings_ui.pane.handle);
        let pane = ui::scroll_pane("settings-pane", &self.settings_ui.pane.handle)
            .size_full()
            .on_scroll(cx.listener(move |_, _: &gpui::ScrollEvent, window, cx| {
                cx.on_next_frame(window, move |this, _, cx| {
                    if ui::scrolled(&this.settings_ui.pane.handle) != scrolled {
                        cx.notify();
                    }
                });
            }))
            .rounded(px(radius::PANEL))
            .flex()
            .flex_col()
            .pb(px(space::SM))
            .child(card);
        let clip_line = scene::sky_under_title(&self.theme.night);
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
                    label: section.name().into(),
                    count: None,
                })
                .collect();
            let (head, panel) = self.tab_strip(
                Strip::Settings,
                named,
                open,
                layout::content_width(self.window_width),
                |this, section, arrowed| this.open_settings_section(section, !arrowed),
                cx,
            );
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .child(head)
                .child(panel.flex().flex_col().flex_1().min_h_0().child(pane))
        } else {
            let entries = shown
                .iter()
                .map(|&section| {
                    let row = ui::nav_row(
                        section.element_id(),
                        section.icon(),
                        section.name(),
                        section == open,
                        ui::NavDensity::List,
                    );
                    (row, section.name().into(), section == open)
                })
                .collect();
            let this = cx.entity().downgrade();
            let picks = shown.clone();
            let rail = ui::options(
                div()
                    .id("settings-sections")
                    .flex()
                    .flex_col()
                    .flex_none()
                    .self_start()
                    .w(px(layout::SETTINGS_RAIL))
                    .gap(px(space::XXS)),
                tr("Settings sections"),
                entries,
                true,
                move |i, _, cx| {
                    let Some(&section) = picks.get(i) else {
                        return;
                    };
                    let _ = this.update(cx, |this, cx| {
                        let again = section == this.settings_ui.open;
                        this.open_settings_section(section, again);
                        cx.notify();
                    });
                },
            )
            .child(ui::measure(move |bounds| {
                rail_end.set(f32::from(bounds.bottom()))
            }));
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
            .child(ui::page_head(Page::Settings.title(), None))
            .child(body)
    }

    pub(super) fn show_settings(&mut self, section: SettingsSection) {
        self.go(Page::Settings);
        self.open_settings_section(section, true);
    }

    fn open_settings_section(&mut self, section: SettingsSection, enter: bool) {
        if self.settings_ui.open != section {
            self.settings_ui.open = section;
            self.settings_ui.pane.handle.set_offset(Default::default());
        }
        self.settings_ui.entering = enter;
    }

    pub(super) fn proxy_settings_button(id: &str, cx: &mut Context<Self>) -> Stateful<Div> {
        ui::button(
            format!("{id}-proxy"),
            Some(crate::icons::NETWORK),
            tr("Proxy settings"),
            ButtonKind::Secondary,
            ui::ButtonSize::Md,
        )
        .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.show_settings(SettingsSection::Network);
            cx.notify();
        }))
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

    fn eclipse_update_row(&mut self, cx: &mut Context<Self>) -> Div {
        let own = self
            .instances
            .list
            .iter()
            .find(|i| i.kind == instances::Kind::Default)
            .cloned();
        let installing = own
            .as_ref()
            .map(|i| self.play_state(i))
            .filter(|state| matches!(state, PlayState::Updating(_) | PlayState::Installing(_)))
            .and_then(PlayState::busy_line);
        let version = self
            .default_version()
            .map(|tag| eclipse_launcher::nightly::display_name(tag));
        let installed = version.is_some();
        let (tone, line) = match (installing, &self.updates.eclipse) {
            (Some(line), _) => (ui::Tone::Busy, line),
            (None, Some(update)) => (
                ui::Tone::Accent(ui::look().accent_soft),
                trf(
                    "{version} available",
                    &[(
                        "version",
                        &eclipse_launcher::nightly::display_name(&update.tag),
                    )],
                ),
            ),
            (None, None) if !installed => (ui::Tone::Off, tr("Not installed").into()),
            (None, None) if self.update_check.clean() => (ui::Tone::Off, tr("Up to date").into()),
            (None, None) => (ui::Tone::Off, tr("Not checked").into()),
        };
        let install = self
            .updates
            .eclipse
            .as_ref()
            .filter(|_| self.versions.installing.is_none())
            .map(|_| {
                ui::button(
                    "settings-install",
                    Some(crate::icons::DOWNLOAD),
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
        let lead = match &own {
            Some(i) => self.instance_icon(i, ui::TILE_MD),
            None => ui::tile(ui::TILE_MD)
                .child(self.eclipse_mark(ui::TILE_SM))
                .into_any_element(),
        };
        update_row(
            lead,
            "Eclipse".into(),
            version.unwrap_or_default(),
            ui::state_line(tone, line).into_any_element(),
            install,
        )
    }

    fn launcher_update_row(&mut self, cx: &mut Context<Self>) -> Div {
        let available = |version: &str| {
            ui::state_line(
                ui::Tone::Accent(ui::look().accent_soft),
                trf("{version} available", &[("version", version)]),
            )
            .into_any_element()
        };
        let state = match &self.self_update {
            SelfUpdate::Downloading(version, done) => ui::state_line(
                ui::Tone::Busy,
                trf(
                    "Downloading {version}, {percent}%",
                    &[
                        ("version", version),
                        ("percent", &format!("{:.0}", done * 100.)),
                    ],
                ),
            )
            .into_any_element(),
            SelfUpdate::Ready(version) => available(version),
            SelfUpdate::Stuck(None) => {
                ui::state_note(ui::Tone::Warn, self.launcher_update_help(None)).into_any_element()
            }
            SelfUpdate::Stuck(Some(e)) | SelfUpdate::Failed(e) => ui::error_note(trf(
                "The update failed: {error}",
                &[("error", ui::plain_error(e))],
            ))
            .into_any_element(),
            SelfUpdate::Idle => match &self.updates.launcher {
                Some(r) => available(backend::launcher_version(&r.tag).unwrap_or_default()),
                None if self.update_check.clean() => {
                    ui::state_line(ui::Tone::Off, tr("Up to date")).into_any_element()
                }
                None => ui::state_line(ui::Tone::Off, tr("Not checked")).into_any_element(),
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
            SelfUpdate::Stuck(_) if !backend::package_managed_launcher() => {
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
        update_row(
            ui::tile(ui::TILE_MD)
                .child(ui::icon(
                    crate::icons::UPDATES,
                    ui::IconSize::Px16,
                    ink(ink::BODY),
                ))
                .into_any_element(),
            tr("Launcher").into(),
            backend::VERSION.to_string(),
            state,
            action,
        )
    }

    fn check_updates_row(&mut self, cx: &mut Context<Self>) -> Div {
        let checking = self.update_check.checking();
        let failed = self.update_check.error().map(str::to_owned);
        let check = ui::button(
            "check-updates",
            failed.is_some().then_some(crate::icons::RETRY),
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
        let beside = match self.update_check.since() {
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
        let card = ui::setting_card(section.element_id(), section.name()).when_some(
            ui::try_focus_for(format!("settings-{id}")),
            |card, handle| card.track_focus(&handle.tab_stop(false)),
        );
        match section {
            SettingsSection::Account => card.child(self.account_card(cx)),
            SettingsSection::Updates => card
                .child(self.eclipse_update_row(cx))
                .child(ui::rule().mx(px(ui::SETTING_ROW_PADDING)))
                .child(self.launcher_update_row(cx))
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
                .child(self.game_architecture_row(cx))
                .child(self.game_args_row(cx)),
            SettingsSection::Ai => card
                .child(self.ai_features_row(cx))
                .children(self.agents_section(cx)),
            SettingsSection::System => card
                .children(self.data_folder_error())
                .child(self.minecraft_folder_row(cx))
                .child(folder_row(
                    "open-data",
                    tr("Launcher data folder").into(),
                    tr("Open data folder").into(),
                    backend::data_dir(),
                ))
                .child(folder_row(
                    "open-logs",
                    tr("Logs folder").into(),
                    tr("Open logs folder").into(),
                    backend::logs_dir(),
                ))
                .when(cfg!(target_os = "linux"), |d| {
                    d.child(self.menu_entry_row(cx))
                }),
            SettingsSection::Developer => card.child(self.dev_offline_row(cx)),
            SettingsSection::DataCollection => card.child(self.collection_section(cx)),
        }
    }
}

impl Launcher {
    fn minecraft_folder_row(&mut self, cx: &mut Context<Self>) -> Div {
        let dir = self.minecraft_dir();
        let path = ui::mono(step::CAPTION, ink::MUTED)
            .child(short(&dir))
            .into_any_element();
        let choose = ui::button(
            "minecraft-folder-choose",
            Some(crate::icons::OPEN_FOLDER),
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
            tr("Show the launcher in the applications menu"),
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
                move |choice, _, cx| {
                    let _ = this.update(cx, |this, cx| {
                        this.settings.appearance = choice;
                        this.save_settings();
                        cx.notify();
                    });
                }
            },
        );
        ui::setting_row(tr("Colour mode"), None, Some(appearance.into_any_element()))
    }

    /// Settings' Proxy row: the proxy every download and sign-in goes through, kept in
    /// `settings.json` and handed to `proxy.rs` whenever the settings are saved.
    fn proxy_row(&mut self, cx: &mut Context<Self>) -> Div {
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
        ui::setting_row_stacked(tr("Proxy address"), detail, field.into_any_element())
    }

    pub(super) fn save_proxy(&mut self) {
        let value = self.fields.value(Field::Proxy).trim().to_owned();
        self.settings.proxy = (!value.is_empty()).then_some(value);
        self.save_settings();
    }
}
