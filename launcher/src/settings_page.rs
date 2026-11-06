//! The Settings page: the sections as cards in a pane that scrolls on its own under the title,
//! and a list of the sections beside them (a row of tabs under the title in a narrow window);
//! the list follows the scroll and takes the pane to a section.

use super::*;
use crate::ui::{self, white};
use eclipse_theme::glass::{ink, radius, space, step};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum SettingsSection {
    Account,
    Updates,
    Appearance,
    Network,
    Game,
    Ai,
    About,
    DataCollection,
}

impl SettingsSection {
    const ALL: [SettingsSection; 8] = [
        SettingsSection::Account,
        SettingsSection::Updates,
        SettingsSection::Appearance,
        SettingsSection::Network,
        SettingsSection::Game,
        SettingsSection::Ai,
        SettingsSection::About,
        SettingsSection::DataCollection,
    ];

    fn shown() -> Vec<SettingsSection> {
        Self::ALL
            .into_iter()
            .filter(|s| {
                *s != SettingsSection::DataCollection || eclipse_telemetry::endpoint().is_some()
            })
            .collect()
    }

    fn id(self) -> &'static str {
        match self {
            SettingsSection::Account => "account",
            SettingsSection::Updates => "updates",
            SettingsSection::Appearance => "appearance",
            SettingsSection::Network => "network",
            SettingsSection::Game => "game",
            SettingsSection::Ai => "ai",
            SettingsSection::About => "about",
            SettingsSection::DataCollection => "data-collection",
        }
    }

    fn name(self) -> &'static str {
        match self {
            SettingsSection::Account => "Account",
            SettingsSection::Updates => "Updates",
            SettingsSection::Appearance => "Appearance",
            SettingsSection::Network => "Network",
            SettingsSection::Game => "Game",
            SettingsSection::Ai => "AI",
            SettingsSection::About => "About",
            SettingsSection::DataCollection => "Data collection",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            SettingsSection::Account => "user",
            SettingsSection::Updates => "refresh",
            SettingsSection::Appearance => "color-picker",
            SettingsSection::Network => "globe-02",
            SettingsSection::Game => "game-controller-01",
            SettingsSection::Ai => "ai-chat-02",
            SettingsSection::About => "information-circle",
            SettingsSection::DataCollection => "view",
        }
    }
}

const SECTION_LIST_WIDTH: f32 = 240.;

const SECTION_ROW: f32 = 32.;

const SECTION_ROW_SHORT: f32 = 28.;

const WIDE_CONTROL_BESIDE: f32 = 840.;

fn card() -> Div {
    ui::card(radius::PANEL)
        .flex()
        .flex_col()
        .p(px(ui::LIST_INSET))
}

/// An About tile: its lead, name, a line under it and what it does.
fn about_tile(lead: AnyElement, title: String, note: String, action: AnyElement) -> Div {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .rounded(px(radius::ROW))
        .bg(ui::inset())
        .p(px(space::XL))
        .child(div().child(lead))
        .child(
            ui::text(step::LEAD, FontWeight::SEMIBOLD, ink::FULL)
                .mt(px(space::LG))
                .child(title),
        )
        .child(
            ui::text(step::NOTE, FontWeight::NORMAL, ink::MUTED)
                .mt(px(space::XXS))
                .truncate()
                .child(note),
        )
        .child(
            div()
                .mt(px(space::LG))
                .flex()
                .flex_col()
                .items_start()
                .child(action),
        )
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
        let shown = SettingsSection::shown();
        let sections: Vec<Stateful<Div>> = shown
            .iter()
            .map(|&section| self.settings_card(section, cx))
            .collect();
        let count = shown.len();
        let active = self.settings_section(count);
        self.settings_marked = active;
        let tabs = self.window_width < layout::SETTINGS_LIST_BESIDE;
        let row_height = if ui::short_window() {
            SECTION_ROW_SHORT
        } else {
            SECTION_ROW
        };
        let entries = shown.iter().enumerate().map(|(i, section)| {
            let id = format!("settings-section-{}", section.id());
            let entry = if tabs {
                ui::tab(id, tr(section.name()), i == active)
            } else {
                ui::nav_row(id, section.icon(), tr(section.name()), i == active)
                    .h(px(row_height))
                    .gap(px(space::SM))
                    .px(px(space::SM))
            };
            entry.on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.settings_pane.scroll_to_top_of_item(i);
                cx.notify();
            }))
        });
        let pane = ui::scroll_pane("settings-pane", &self.settings_pane)
            .size_full()
            // A scroll redraws the pane from its layer without rendering the page again, so the
            // list follows it here: on the next frame, once the offset has moved, and only when
            // the section changed.
            .on_scroll(cx.listener(move |_, _: &gpui::ScrollEvent, window, cx| {
                cx.on_next_frame(window, move |this, _, cx| {
                    if this.settings_section(count) != this.settings_marked {
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
        let pane = div()
            .relative()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .child(pane)
            .children(ui::more_below_fade(&self.settings_pane, self.theme.page));
        let body = if tabs {
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .child(ui::tab_strip("settings-sections", entries))
                .child(pane)
        } else {
            div()
                .flex()
                .gap(px(layout::CARD_GAP))
                .flex_1()
                .min_h_0()
                .child(
                    card()
                        .id("settings-sections")
                        .flex_none()
                        .self_start()
                        .w(px(SECTION_LIST_WIDTH))
                        .gap(px(space::XXS))
                        .children(entries),
                )
                .child(pane)
        };
        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .child(ui::page_head(tr("Settings"), ""))
            .child(body)
    }

    pub(super) fn wide_controls_beside(&self) -> bool {
        let content = layout::content_width(self.window_width);
        let list = if self.window_width < layout::SETTINGS_LIST_BESIDE {
            0.
        } else {
            SECTION_LIST_WIDTH + layout::CARD_GAP
        };
        content - list >= WIDE_CONTROL_BESIDE
    }

    /// The section the list marks, from where the pane's sections are now ([`spy`]). A section's
    /// bounds are where layout put it, before the scroll (`ScrollHandle` keeps the offset apart):
    /// the scroll is taken off them here. Reading them as they were left the list on the first
    /// section until the pane hit its end.
    fn settings_section(&self, count: usize) -> usize {
        let pane = self.settings_pane.bounds();
        let scrolled = -self.settings_pane.offset().y;
        let tops: Vec<f32> = (0..count)
            .map_while(|i| self.settings_pane.bounds_for_item(i))
            .map(|b| f32::from(b.top() - pane.top() - scrolled))
            .collect();
        spy(
            &tops,
            f32::from(pane.size.height),
            f32::from(scrolled),
            f32::from(self.settings_pane.max_offset().y),
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

    /// The Game card's Native mods row: native mods are always on, so the row picks the profile
    /// they build with, as the Mods page does.
    fn native_mods_row(&mut self, cx: &mut Context<Self>) -> Div {
        let profile = native_mods::Profile::chosen(&self.settings);
        let picker = ui::segments(
            "settings-native-profile",
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
                        this.settings.native_profile = Some(choice);
                        this.settings.native_release = false;
                        this.native_done = None;
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
        ui::setting_row_fit(
            self.wide_controls_beside(),
            tr("Native mod build"),
            Some(
                div()
                    .child(format!("{} {said}.", tr("Experimental.")))
                    .into_any_element(),
            ),
            picker.into_any_element(),
        )
    }

    fn reduce_motion_row(&mut self, cx: &mut Context<Self>) -> Div {
        let system = if crate::motion::system_reduces_motion() {
            "System (On)"
        } else {
            "System (Off)"
        };
        let picker = ui::segments(
            "reduce-motion",
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

    /// About's three tiles: Eclipse and its update, the launcher and its own, and the data folder.
    fn about_tiles(
        &mut self,
        eclipse: String,
        eclipse_action: Option<AnyElement>,
        launcher: String,
        launcher_action: Option<AnyElement>,
    ) -> Div {
        let installed = self
            .settings
            .selected
            .clone()
            .map(|tag| match &self.releases {
                Load::Ready(list) => list
                    .iter()
                    .find(|r| r.tag == tag)
                    .map_or(tag.clone(), |r| update_name(r).to_owned()),
                _ => tag,
            });
        let tone = if self.updates.eclipse.is_some() {
            ui::Tone::Accent(ui::look().accent_soft)
        } else if self.update_error.is_some() {
            ui::Tone::Warn
        } else if self.checked.is_some() {
            ui::Tone::Live
        } else {
            ui::Tone::Busy
        };
        let colour = match tone {
            ui::Tone::Live => ui::live(),
            ui::Tone::Warn => ui::warn(),
            _ => white(ink::BODY),
        };
        let status = ui::text(step::NOTE, FontWeight::NORMAL, ink::BODY)
            .flex()
            .w_full()
            .gap(px(space::SM))
            .text_color(colour)
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .h(px(step::NOTE.line))
                    .child(ui::dot(tone)),
            )
            .child(div().min_w_0().child(eclipse));
        let square = |icon: &'static str| {
            ui::tile(ui::TILE_LG)
                .child(ui::icon(icon, ui::IconSize::Px16, white(ink::STRONG)))
                .into_any_element()
        };
        let open = ui::button(
            "about-open-data",
            None,
            tr("Open"),
            ButtonKind::Secondary,
            ui::ButtonSize::Md,
        )
        .on_press(|_, _, cx| crate::toast::open(cx, &backend::data_dir()))
        .into_any_element();
        div()
            .flex()
            .gap(px(space::SM))
            .child(about_tile(
                self.eclipse_mark(42.),
                "Eclipse".into(),
                installed.unwrap_or_else(|| tr("Nightly").to_string()),
                div()
                    .flex()
                    .flex_col()
                    .items_start()
                    .w_full()
                    .gap(px(space::MD))
                    .child(status)
                    .children(eclipse_action)
                    .into_any_element(),
            ))
            .child(about_tile(
                square("computer"),
                tr("Launcher").to_string(),
                launcher,
                div().children(launcher_action).into_any_element(),
            ))
            .child(about_tile(
                square("folder-01"),
                tr("Data folder").to_string(),
                short(&backend::data_dir()),
                open,
            ))
    }

    /// The cards, one per entry of [`SECTIONS`], in its order.
    fn settings_card(&mut self, section: SettingsSection, cx: &mut Context<Self>) -> Stateful<Div> {
        let card = card().id(format!("settings-{}", section.id()));
        match section {
            SettingsSection::Account => card.child(self.account_card(cx)),
            SettingsSection::Updates => card
                .child(self.auto_install_row(cx))
                .child(self.auto_prune_row(cx))
                .child(self.mods_auto_update_row(cx)),
            SettingsSection::Appearance => card
                .child(self.colour_mode_row(cx))
                .child(self.accent_row(cx))
                .child(self.launcher_theme_row(cx))
                .child(self.home_background_row(cx))
                .child(self.reduce_motion_row(cx))
                .child(self.language_row(cx))
                .child(self.reset_look_row(cx)),
            SettingsSection::Network => card.child(self.proxy_row(cx)),
            SettingsSection::Game => card
                .child(self.while_playing_row(cx))
                .child(self.game_architecture_row(cx))
                .child(self.game_args_row(cx))
                .child(self.native_mods_row(cx)),
            SettingsSection::Ai => card
                .child(self.ai_features_row(cx))
                .children(self.agents_section(cx)),
            SettingsSection::About => {
                let mono = |s: String| {
                    div()
                        .font_family(theme::MONO)
                        .text_size(px(step::CAPTION.size))
                        .child(s)
                        .into_any_element()
                };
                let (eclipse, eclipse_action) = self.eclipse_status(cx);
                let (launcher, launcher_action) = self.launcher_status(cx);
                card.child(self.about_tiles(eclipse, eclipse_action, launcher, launcher_action))
                    .child(ui::setting_row(
                        tr("Logs"),
                        Some(mono(short(&backend::logs_dir()))),
                        Some(
                            ui::button(
                                "open-logs",
                                None,
                                tr("Open"),
                                ButtonKind::Secondary,
                                ui::ButtonSize::Md,
                            )
                            .on_press(|_, _, cx| {
                                let dir = backend::logs_dir();
                                let _ = std::fs::create_dir_all(&dir);
                                crate::toast::open(cx, &dir);
                            })
                            .into_any_element(),
                        ),
                    ))
                    .when(cfg!(target_os = "linux"), |d| {
                        d.child(self.menu_entry_row(cx))
                    })
                    .children(self.dev_options_section(cx))
            }
            SettingsSection::DataCollection => card.child(self.collection_section(cx)),
        }
    }
}

impl Launcher {
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
            Some(div().child(keeps).into_any_element()),
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

    fn eclipse_status(&mut self, cx: &mut Context<Self>) -> (String, Option<AnyElement>) {
        let eclipse = match (&self.updates.eclipse, &self.update_error) {
            (Some(r), _) => trf("{version} available", &[("version", update_name(r))]),
            (None, Some(e)) => ui::plain_error(e).to_string(),
            (None, None) if self.checked.is_some() => tr("Up to date").to_string(),
            _ => tr("Checking…").to_string(),
        };
        let eclipse_action = self
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
        (eclipse, eclipse_action)
    }

    fn launcher_status(&mut self, cx: &mut Context<Self>) -> (String, Option<AnyElement>) {
        let launcher = match &self.self_update {
            SelfUpdate::Downloading(v, p) => {
                let done = format!("{:.0}", p * 100.);
                let said = trf(
                    "downloading {version}, {percent}%",
                    &[("version", v), ("percent", &done)],
                );
                format!("{}  ·  {said}", backend::VERSION)
            }
            SelfUpdate::Ready(v) => format!(
                "{}  ·  {}",
                backend::VERSION,
                trf("{version} available", &[("version", v)])
            ),
            SelfUpdate::Stuck(Some(e)) => format!(
                "{}  ·  {}",
                backend::VERSION,
                trf("the update failed: {error}", &[("error", e)])
            ),
            SelfUpdate::Stuck(None) => format!(
                "{}  ·  {}",
                backend::VERSION,
                self.launcher_update_help(None)
            ),
            SelfUpdate::Failed(e) => format!(
                "{}  ·  {}",
                backend::VERSION,
                trf("the update failed: {error}", &[("error", e)])
            ),
            SelfUpdate::Idle => match &self.updates.launcher {
                Some(r) => format!(
                    "{}  ·  {}",
                    backend::VERSION,
                    trf(
                        "{version} available",
                        &[(
                            "version",
                            backend::launcher_version(&r.tag).unwrap_or_default()
                        )]
                    )
                ),
                None if self.checked.is_some() && self.update_error.is_none() => {
                    format!("{}  ·  {}", backend::VERSION, tr("up to date"))
                }
                None => backend::VERSION.to_string(),
            },
        };
        let launcher_action = match &self.self_update {
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
            SelfUpdate::Downloading(..) | SelfUpdate::Stuck(_) => None,
            _ => Some(
                ui::button(
                    "check",
                    None,
                    tr("Check now"),
                    ButtonKind::Secondary,
                    ui::ButtonSize::Md,
                )
                .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.check_updates();
                    cx.notify();
                }))
                .into_any_element(),
            ),
        };
        (launcher, launcher_action)
    }

    fn colour_mode_row(&mut self, cx: &mut Context<Self>) -> Div {
        let appearance = ui::segments(
            "appearance",
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
                ui::WIDE_FIELD_MAX,
                ui::ButtonSize::Md,
                cx,
            )
            .w_full()
            .max_w(px(ui::WIDE_FIELD_MAX));
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
                ui::WIDE_FIELD_MAX,
                ui::ButtonSize::Md,
                cx,
            )
            .w_full()
            .max_w(px(ui::WIDE_FIELD_MAX));
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

/// How an available update is named: a Nightly by its title ("Nightly 2026-10-28 · 0c02067"), a
/// release by its tag.
pub(super) fn update_name(release: &Release) -> &str {
    match release.nightly {
        Some(_) => &release.name,
        None => &release.tag,
    }
}

#[cfg(test)]
mod tests {
    use super::spy;

    /// A 600 px pane over eight cards: tall ones, then About and Data collection, too short for
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
        // About is marked for a stretch of scroll before the end, not for one step.
        let about = seen.iter().filter(|&&s| s == 6).count();
        assert!(about >= 8, "About marked for {about} steps of 10 px");
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
