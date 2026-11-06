//! Settings' accent colour and Home background rows (`eclipse_launcher::themes`).
use super::*;
use crate::fields::Field;
use crate::ui;
use eclipse_launcher::themes::{self, Accent};
use gpui::{
    canvas, fill, point, size, Bounds, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    Pixels, Point,
};
use std::path::PathBuf;

/// Each accent's swatch, in points.
const SWATCH: f32 = 36.;
const CHIP: f32 = 24.;
/// The hue strip under the swatches, in points: 0 to 360 degrees across it.
const STRIP: f32 = 212.;

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

    /// Settings' sky colours: Eclipse's own sky or First Light, free for everyone.
    pub(super) fn launcher_theme_row(&mut self, cx: &mut Context<Self>) -> Div {
        let current = themes::theme(self.settings.theme.as_deref()).map(|th| th.id);
        let mut options: Vec<(Option<&'static str>, &'static str)> = vec![(None, "Eclipse")];
        options.extend(themes::THEMES.iter().map(|th| (Some(th.id), th.name)));
        let this = cx.entity().downgrade();
        let picker = ui::segments("launcher-theme", &options, current, move |id, _, cx| {
            let _ = this.update(cx, |this, cx| {
                this.settings.theme = id.map(str::to_owned);
                this.save_settings();
                this.restyle = true;
                cx.notify();
            });
        });
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
                    .size(px(SWATCH))
                    .cursor_pointer(),
                on,
            )
        };
        let chip = |rgb: u32| {
            div()
                .size(px(CHIP))
                .rounded(px(eclipse_theme::glass::radius::ROW))
                .bg(gpui::rgb(rgb))
        };
        let mut row = div().flex().flex_wrap().items_center().gap(px(4.));
        for (accent, rgb) in Accent::ALL.into_iter().zip(themes::SWATCHES) {
            row = row.child(
                swatch(
                    SharedString::from(format!("accent-{}", accent.name())),
                    current == accent,
                )
                .child(chip(rgb))
                .on_press(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.settings.accent = accent;
                    this.save_settings();
                    this.restyle = true;
                    cx.notify();
                })),
            );
        }
        row = row
            .child(
                swatch("accent-any".into(), custom)
                    .child(if custom {
                        chip(current.rgb()).into_any_element()
                    } else {
                        ui::icon(
                            "color-picker",
                            ui::IconSize::Px16,
                            ui::white(eclipse_theme::glass::ink::BODY),
                        )
                        .into_any_element()
                    })
                    .on_press(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.accent_strip = !this.accent_strip;
                        cx.notify();
                    })),
            )
            .child(self.field(Field::AccentHex, "#f5a524", 96., cx).ml(px(4.)));
        let control = div()
            .flex()
            .flex_col()
            .when(beside, |d| d.items_end())
            .gap(px(10.))
            .child(row)
            .when(self.accent_strip, |d| d.child(self.hue_strip(cx)));
        settings_page::wide_row(
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
            Some(
                div()
                    .child(tr("Also resets the language."))
                    .into_any_element(),
            ),
            Some(
                ui::button(
                    "look-reset",
                    Some("refresh"),
                    tr("Reset to defaults"),
                    ButtonKind::Secondary,
                    ui::Size::Md,
                )
                .on_press(cx.listener(|this, _: &ClickEvent, window, cx| {
                    let s = &mut this.settings;
                    s.appearance = Default::default();
                    s.accent = Accent::AMBER;
                    s.theme = None;
                    s.background = None;
                    s.language = None;
                    s.reduce_motion = None;
                    this.save_settings();
                    i18n::set(this.settings.language());
                    crate::motion::set_reduce_motion(None);
                    titlebar::apply_appearance(window, this.settings.appearance);
                    this.accent_strip = false;
                    this.background_error = None;
                    this.theme = this.settings_theme(window);
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
            self.restyle = true;
            cx.notify();
        }
    }

    /// A strip of every hue: a click or a drag picks any of them, the swatches being shortcuts.
    fn hue_strip(&self, cx: &mut Context<Self>) -> Div {
        let current = self.settings.accent.hue();
        let area = self.hue_area.clone();
        let ink = ui::white(eclipse_theme::glass::ink::FULL);
        // Hues per painted quad: 60 of them, fine enough at this width.
        const STEPS: usize = 60;
        div()
            .relative()
            .flex_none()
            .w(px(STRIP))
            .h(px(CHIP))
            .rounded(px(eclipse_theme::glass::radius::CONTROL))
            .overflow_hidden()
            .cursor_pointer()
            .child(
                canvas(
                    move |bounds, _, _| {
                        area.set(Some(bounds));
                        bounds
                    },
                    move |bounds, _, window, _| {
                        let width: f32 = bounds.size.width.into();
                        let step = width / STEPS as f32;
                        for i in 0..STEPS {
                            let left = px(step * i as f32);
                            let colour: Hsla =
                                gpui::rgb(themes::hue_colour(i as f32 * 360. / STEPS as f32))
                                    .into();
                            window.paint_quad(fill(
                                Bounds::new(
                                    point(bounds.left() + left, bounds.top()),
                                    size(px(step), bounds.size.height),
                                ),
                                colour,
                            ));
                        }
                        if let Some(hue) = current {
                            let at = px(width * hue / 360.);
                            window.paint_quad(fill(
                                Bounds::new(
                                    point(bounds.left() + at - px(1.5), bounds.top()),
                                    size(px(3.), bounds.size.height),
                                ),
                                ink,
                            ));
                        }
                    },
                )
                .size_full(),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, ev: &MouseDownEvent, window, cx| {
                    this.pick_hue(ev.position, window, cx);
                }),
            )
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, window, cx| {
                if ev.pressed_button == Some(MouseButton::Left) {
                    this.pick_hue(ev.position, window, cx);
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, cx| {
                    this.finish_hue();
                    cx.notify();
                }),
            )
            // `on_mouse_up` fires only while the strip is hovered, so a drag released off it would
            // keep its accent in memory and never write it.
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, cx| {
                    this.finish_hue();
                    cx.notify();
                }),
            )
    }

    /// A release ends the drag and writes `settings.json` once.
    fn finish_hue(&mut self) {
        if self.hue_dragging {
            self.hue_dragging = false;
            self.save_settings();
        }
    }

    /// The hue under the pointer, as the accent. Saving waits until the mouse comes up, so a drag
    /// writes `settings.json` once. A move that stays on the same degree changes nothing.
    fn pick_hue(&mut self, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(area) = self.hue_area.get() else {
            return;
        };
        let width: f32 = area.size.width.into();
        if width <= 0. {
            return;
        }
        let across: f32 = (at.x - area.origin.x).into();
        let accent = Accent::custom(((across / width).clamp(0., 1.) * 360.).round());
        self.hue_dragging = true;
        if accent == self.settings.accent {
            return;
        }
        self.settings.accent = accent;
        self.theme = self.settings_theme(window);
        cx.notify();
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
        let mut actions = div().flex().gap(px(4.)).child(
            button("background-choose", "Choose image…", ButtonKind::Secondary).on_press(
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.choose_background(cx);
                    cx.notify();
                }),
            ),
        );
        if custom {
            actions = actions.child(
                button("background-reset", "Default background", ButtonKind::Ghost).on_press(
                    cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.settings.background = None;
                        this.save_settings();
                        cx.notify();
                    }),
                ),
            );
        }
        let detail = self
            .background_error
            .clone()
            .map(|e| div().text_color(ui::danger()).child(e).into_any_element());
        ui::setting_row(
            tr("Home background"),
            detail,
            Some(actions.into_any_element()),
        )
    }
}
