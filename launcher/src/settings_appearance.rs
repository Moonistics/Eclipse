//! Settings' accent colour and Home background rows (`eclipse_launcher::themes`).
use super::*;
use crate::fields::Field;
use crate::ui;
use eclipse_launcher::themes::{self, Accent};
use eclipse_theme::glass::space;
use std::path::PathBuf;

const SWATCH: f32 = ui::TILE_MD;
const SWATCH_FILL: f32 = ui::TILE_SM;
const HUE_KEY_STEP: f32 = 5.;

impl Launcher {
    /// The theme Settings ask for, under the window's own light or dark: the player's look,
    /// under the launcher theme's sky if they picked one.
    pub(super) fn settings_theme(&self, window: &Window) -> Theme {
        let theme = Theme::for_appearance(
            self.settings.appearance,
            self.settings.accent,
            window.appearance(),
        );
        match themes::theme(self.settings.theme.as_deref()) {
            Some(picked) => theme.with_sky(picked.sky),
            None => theme,
        }
    }

    /// Settings' sky colours: Eclipse's own sky or Daybreak, free for everyone.
    pub(super) fn launcher_theme_row(&mut self, cx: &mut Context<Self>) -> Div {
        let current = themes::theme(self.settings.theme.as_deref()).map(|th| th.id);
        let mut options: Vec<(Option<&'static str>, &'static str)> = vec![(None, "Eclipse")];
        options.extend(themes::THEMES.iter().map(|th| (Some(th.id), th.name)));
        let this = cx.entity().downgrade();
        let picker = ui::segments(
            "launcher-theme",
            tr("Sky colours"),
            &ui::translated(&options),
            current,
            move |id, _, cx| {
                let _ = this.update(cx, |this, cx| {
                    this.settings.theme = id.map(str::to_owned);
                    this.save_settings();
                    this.restyle();
                    cx.notify();
                });
            },
        );
        ui::setting_row(tr("Sky colours"), None, Some(picker.into_any_element()))
    }

    /// The picture behind Home, if the player keeps one.
    pub(super) fn home_background(&self) -> Option<PathBuf> {
        themes::background(&backend::data_dir(), self.settings.background.as_deref())
    }

    /// The accent row: its seven swatches, the any-colour swatch (a click shows the hue strip
    /// under them) and the hex field, which takes the colour as soon as it is one.
    pub(super) fn accent_row(&mut self, cx: &mut Context<Self>) -> Div {
        self.sync_accent_hex(cx);
        let beside = self.wide_controls_beside();
        let current = self.settings.accent;
        let custom = !Accent::ALL.contains(&current);
        let swatch = |id: SharedString, on: bool| {
            ui::selected(
                ui::hoverable(id, eclipse_theme::glass::radius::ROW, ui::Hover::Row)
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(SWATCH)),
                on,
            )
        };
        let chip = |rgb: u32, on: bool| {
            let tick = eclipse_theme::glass::Glass::new(rgb).on_accent;
            div()
                .flex()
                .items_center()
                .justify_center()
                .size(px(SWATCH_FILL))
                .rounded(px(eclipse_theme::glass::radius::ROW))
                .bg(gpui::rgb(rgb))
                .when(on, |d| {
                    d.child(ui::icon("tick-02", ui::IconSize::Px16, ui::color(tick)))
                })
        };
        let mut swatches: Vec<(Stateful<Div>, SharedString, bool)> = Vec::new();
        let mut picks: Vec<Option<Accent>> = Vec::new();
        for (accent, rgb) in Accent::ALL.into_iter().zip(themes::SWATCHES) {
            let on = current == accent;
            swatches.push((
                swatch(SharedString::from(format!("accent-{}", accent.name())), on)
                    .child(chip(rgb, on)),
                tr(accent.label().unwrap_or_default()).into(),
                on,
            ));
            picks.push(Some(accent));
        }
        swatches.push((
            swatch("accent-any".into(), custom).child(if custom {
                chip(current.rgb(), true).into_any_element()
            } else {
                ui::icon(
                    crate::icons::ANY_COLOUR,
                    ui::IconSize::Px16,
                    ui::white(eclipse_theme::glass::ink::BODY),
                )
                .into_any_element()
            }),
            tr("Any colour").into(),
            custom,
        ));
        picks.push(None);
        let pick = cx.listener(move |this, i: &usize, _, cx| {
            match picks.get(*i) {
                Some(Some(accent)) => {
                    this.settings.accent = *accent;
                    this.save_settings();
                    this.restyle();
                }
                Some(None) => this.settings_ui.accent_strip = !this.settings_ui.accent_strip,
                None => {}
            }
            cx.notify();
        });
        let swatches = ui::options(
            div()
                .id("accent-swatches")
                .flex()
                .flex_wrap()
                .items_center()
                .justify_end()
                .gap(px(space::XS)),
            tr("Accent colour"),
            swatches,
            false,
            move |i, window, cx| pick(&i, window, cx),
        );
        let mut row = div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_end()
            .gap(px(space::XS));
        row = row.child(swatches).child(
            self.field(
                Field::AccentHex,
                format!("#{:06x}", eclipse_theme::glass::ACCENT),
                layout::FIELD_XS,
                ui::ButtonSize::Md,
                cx,
            )
            .ml(px(space::XS)),
        );
        let control = div()
            .flex()
            .flex_col()
            .items_end()
            .gap(px(space::MD))
            .child(row)
            .when(self.settings_ui.accent_strip, |d| {
                d.child(self.hue_strip(cx))
            });
        ui::setting_row_fit(
            beside,
            tr("Accent colour"),
            None,
            control.into_any_element(),
        )
    }

    /// Reset to defaults: the Appearance card's options go back to how Eclipse ships.
    pub(super) fn reset_look_row(&mut self, cx: &mut Context<Self>) -> Div {
        ui::setting_row(
            tr("Reset appearance"),
            None,
            Some(
                ui::button(
                    "look-reset",
                    None,
                    tr("Reset to defaults"),
                    ButtonKind::Secondary,
                    ui::ButtonSize::Md,
                )
                .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    let s = &mut this.settings;
                    s.appearance = Default::default();
                    s.accent = Accent::AMBER;
                    s.theme = None;
                    s.background = None;
                    s.reduce_motion = None;
                    this.save_settings();
                    crate::motion::set_reduce_motion(None);
                    this.settings_ui.accent_strip = false;
                    this.background_error = None;
                    this.restyle();
                    crate::toast::say(tr("Appearance is back to its defaults"));
                    cx.notify();
                }))
                .into_any_element(),
            ),
        )
    }

    /// The hex field shows the accent, except while typed in: then each whole colour typed is
    /// the accent at once.
    fn sync_accent_hex(&mut self, cx: &mut Context<Self>) {
        let shown = self.settings.accent.hex();
        if !self.fields.has_focus(Field::AccentHex) {
            if self.fields.value(Field::AccentHex) != shown {
                self.fields.text(Field::AccentHex).set(shown);
            }
            return;
        }
        let typed = self.fields.value(Field::AccentHex).trim();
        let hex = format!("#{}", typed.trim_start_matches('#'));
        if let Some(accent) = Accent::parse(&hex).filter(|a| *a != self.settings.accent) {
            self.settings.accent = accent;
            self.save_settings();
            self.restyle();
            cx.notify();
        }
    }

    fn hue_strip(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let hue = self.settings.accent.shown_hue();
        let this = cx.entity().downgrade();
        ui::slider(
            "accent-hue",
            tr("Accent colour"),
            hue / 360.,
            f64::from(hue),
            (0., 360.),
            ui::Track::Hues,
            move |to, _, cx| {
                let _ = this.update(cx, |this, cx| {
                    match to {
                        ui::SliderMove::To(at) => this.pick_hue(at),
                        ui::SliderMove::Step(n) => this.step_hue(n as f32 * HUE_KEY_STEP),
                        ui::SliderMove::Done => this.finish_hue(),
                    }
                    cx.notify();
                });
            },
        )
    }

    /// A release ends the drag and writes `settings.json` once.
    fn finish_hue(&mut self) {
        if self.settings_ui.hue_dragging {
            self.settings_ui.hue_dragging = false;
            self.save_settings();
        }
    }

    /// The hue under the pointer, as the accent. Saving waits until the mouse comes up, so a drag
    /// writes `settings.json` once. A move that stays on the same degree changes nothing.
    fn step_hue(&mut self, by: f32) {
        let hue = self.settings.accent.hue().unwrap_or(0.);
        self.settings.accent = Accent::custom((hue + by).rem_euclid(360.));
        self.save_settings();
        self.restyle();
    }

    fn pick_hue(&mut self, at: f32) {
        let accent = Accent::custom((at * 360.).round());
        self.settings_ui.hue_dragging = true;
        if accent == self.settings.accent {
            return;
        }
        self.settings.accent = accent;
        self.restyle();
    }

    fn choose_background(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr("Choose").to_string().into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(mut paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.pop() else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                this.take_background(&path);
                cx.notify();
            });
        })
        .detach();
    }

    fn take_background(&mut self, path: &std::path::Path) {
        match themes::keep_background(&backend::data_dir(), path) {
            Ok(name) => {
                self.settings.background = Some(name);
                self.save_settings();
                self.background_error = None;
            }
            Err(e) => self.background_error = Some(e),
        }
    }

    pub(super) fn home_background_row(&self, cx: &mut Context<Self>) -> Div {
        let custom = self.home_background().is_some();
        let mut actions = div().flex().gap(px(space::XS)).child(
            ui::button(
                "background-choose",
                Some("folder-01"),
                tr("Choose image…"),
                ButtonKind::Secondary,
                ui::ButtonSize::Md,
            )
            .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.choose_background(cx);
                cx.notify();
            })),
        );
        if custom {
            actions = actions.child(
                ui::button(
                    "background-reset",
                    None,
                    tr("Default background"),
                    ButtonKind::Secondary,
                    ui::ButtonSize::Md,
                )
                .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.settings.background = None;
                    this.save_settings();
                    cx.notify();
                })),
            );
        }
        let detail = self
            .background_error
            .clone()
            .map(|e| ui::error_note(e).into_any_element());
        ui::setting_row(
            tr("Home background"),
            detail,
            Some(actions.into_any_element()),
        )
    }
}
