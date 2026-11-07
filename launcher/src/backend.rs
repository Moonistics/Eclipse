//! Eclipse versions and the game: GitHub releases, installs, resource preparation, the game's
//! processes, settings and the launcher's own updates. Functions here block; the window runs
//! them on threads and reads their messages.

pub const REPO: &str = "Moonistics/Eclipse";
pub const AGENT_DOCS_URL: &str =
    "https://github.com/Moonistics/Eclipse/blob/main/docs/agent-control.md";
/// This launcher's version: the release workflow sets `ECLIPSE_LAUNCHER_VERSION` from the
/// tag it builds (no version-bump commits); other builds say Cargo.toml's.
pub const VERSION: &str = version_or(option_env!("ECLIPSE_LAUNCHER_VERSION"));
const fn version_or(injected: Option<&'static str>) -> &'static str {
    match injected {
        Some(v) if !v.is_empty() => v,
        _ => env!("CARGO_PKG_VERSION"),
    }
}

mod architecture;
mod game_output;
mod jobs;
mod links;
mod minecraft;
mod net;
mod paths;
mod platform;
mod play;
mod prepare;
mod releases;
mod resources;
mod self_update;
mod settings;
mod updates;
mod versions;

pub(crate) use architecture::native_mod_architecture_error;
pub use architecture::{
    can_choose_game_architecture, needs_game_architecture_install, GameArchitecture,
};
pub use game_output::{
    crash_report_since, crash_summary, latest_log_in, logs_dir, logs_dir_in, read_logs,
    switch_from_auto, vulkan_crash_fallback, vulkan_start_crashed, GAME_FIRST_FRAME,
};
pub use jobs::{
    prepare_command, spawn, Ended, Logged, Output, Running, RuntimeGuard, Stream, LOG_KEEP,
};
pub use links::{links, Links};
pub use minecraft::{default_minecraft_dir, minecraft_dir};
pub(crate) use net::{agent, agent_within, get, rate_limited, request};
pub use net::{get_bytes, network_error};
pub(crate) use paths::tool_safe;
pub use paths::{build_dir, data_dir, exports_dir, game_home, own_minecraft_dir, version_dir};
pub(crate) use platform::{pe_machine, CLIENT, GAME_APP, PE_ARM64, PE_X64};
#[cfg(test)]
use play::client_command;
pub use play::{
    agree_eula, join_friend, launch, need_prepared, no_ai, server, server_command, server_dir,
    set_no_ai, split_args, Launched, Play, PlayError, SIGN_IN_HINT,
};
pub use prepare::{prepare, MinecraftFrom, PrepareStep, Preparing};
pub use releases::{
    compare_versions, is_game_tag, is_release_or_nightly, latest, launcher_version,
    markdown_to_text, newest_release_or_nightly, release_notes, releases, Release, CHECK_EVERY,
};
pub use resources::{has_sounds, is_prepared, resources, reuse_prepared_everywhere};
pub(crate) use resources::{resources_in, reuse_prepared_for, RESOURCES_MANIFEST};
#[cfg(test)]
pub(crate) use self_update::new_stage_folder;
#[cfg(windows)]
pub use self_update::remove_replaced_binaries;
pub(crate) use self_update::{already_in_place, installed_launcher_version};
pub use self_update::{
    appimage_path, cannot_replace_itself, cleanup_launcher_updates, download_page, exe_path,
    mark_handed_over, package_managed_launcher, place_checked, place_staged, remove_stage, restart,
    self_update, self_update_allowed, stage_update, update_target, update_work_dir, Staged,
};
pub use settings::{Appearance, NativeProfile, Settings, WhilePlaying};
pub use updates::{check_updates, releases_and_updates, Updates};
pub use versions::{
    after_install, disk_use, followed_version, game_version, install, install_selects, install_tag,
    installed, mod_api_level, nightly_downgrade, no_version_installed, prune_old_versions, remove,
    remove_version, resolve_version, select_after_update, update_selects, InstallWhy,
};
pub(crate) use versions::{
    copy_version_except, download, installed_at, is_installed, mark_installed, needed_versions,
    remove_unused, INSTALL_MARK,
};

#[cfg(test)]
mod release_loading_tests;

#[cfg(test)]
mod java_tests;

#[cfg(test)]
mod tests;
