//! What the launcher says at startup about its data folder: a folder it couldn't open, or how a
//! data move went.
use super::*;

fn env_value(key: &str) -> Option<String> {
    std::env::var_os(key)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string_lossy().into_owned())
}

pub(super) fn startup_error() -> Option<String> {
    eclipse_launcher::data_location::startup_error()
        .map(|error| {
            trf(
                "Could not open the data folder: {error}",
                &[("error", &error)],
            )
        })
        .or_else(|| {
            env_value("ECLIPSE_RELOCATION_ERROR")
                .map(|error| trf("Data move failed: {error}", &[("error", &error)]))
        })
}

pub(super) fn startup_note() -> Option<String> {
    env_value("ECLIPSE_RELOCATION_BACKUP").map(|paths| {
        trf(
            "Data moved. Old folders still use disk space: {paths}. Verify your worlds and instances before removing them yourself.",
            &[("paths", &paths)],
        )
    })
}
