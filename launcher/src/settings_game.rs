use crate::fields::Field;
use crate::{layout, ui, Launcher, Load};
use eclipse_launcher::backend;
use eclipse_launcher::backend::WhilePlaying;
use eclipse_launcher::i18n::tr;
use gpui::prelude::*;
use gpui::{div, px, Div};

impl Launcher {
    pub(super) fn while_playing_row(&self, cx: &mut Context<Self>) -> Div {
        let mut options = vec![
            (WhilePlaying::Stay, "Keep launcher open"),
            (WhilePlaying::Minimize, "Minimise"),
        ];
        if cfg!(target_os = "macos") {
            options.push((WhilePlaying::Hide, "Hide"));
        }
        let current = self.settings.while_playing.here();
        let picker = crate::ui::segments(
            "while-playing",
            tr("Launcher window while playing"),
            &ui::translated(&options),
            current,
            {
                let this = cx.entity().downgrade();
                move |choice, _, cx| {
                    let _ = this.update(cx, |this, cx| {
                        this.settings.while_playing = choice;
                        this.save_settings();
                        cx.notify();
                    });
                }
            },
        );
        ui::setting_row_fit(
            layout::settings_controls_beside(self.window_width),
            tr("Launcher window while playing"),
            None,
            picker.into_any_element(),
        )
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
                        this.versions.releases = Load::Loading;
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
        ui::setting_row_stacked(
            tr("Default Eclipse game arguments"),
            None,
            field.into_any_element(),
        )
    }
}
