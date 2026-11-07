use crate::{ui, Launcher};
use eclipse_launcher::i18n::trf;
use gpui::prelude::*;
use gpui::{div, px, Div};

pub(super) fn startup_error() -> Option<String> {
    eclipse_launcher::data_location::startup_error().map(|error| {
        trf(
            "Could not open the data folder: {error}",
            &[("error", &error)],
        )
    })
}

impl Launcher {
    pub(super) fn data_folder_error(&self) -> Option<Div> {
        let error = self.location_error.clone()?;
        Some(
            div()
                .p(px(ui::SETTING_ROW_PADDING))
                .child(ui::error_note(error)),
        )
    }
}
