//! Everything that is not UI: GitHub, installs, resource preparation, the game's processes and
//! updates. The GUI, the CLI and the MCP server all go through this module. Functions here
//! block; the GUI runs them on threads and reads their messages.

pub const REPO: &str = "Moonistics/Eclipse";
/// The page on connecting an AI agent.
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
    can_choose_game_architecture, game_architecture_launch_error, needs_game_architecture_install,
    GameArchitecture,
};
pub use game_output::{
    crash_report_since, crash_reports_dir, crash_summary, logs_dir, newest_crash_report, read_logs,
    switch_from_auto, vulkan_crash_fallback, vulkan_start_crashed, GAME_FIRST_FRAME,
};
pub use jobs::{prepare_command, spawn, Job, Logged, Msg, Running, RuntimeGuard, LOG_KEEP};
pub use links::{links, Links};
pub use minecraft::{default_minecraft_dir, download_version_for_owner, minecraft_dir};
pub(crate) use net::{agent, agent_within, get, rate_limited, request};
pub use net::{get_bytes, network_error, NetError};
pub use paths::{build_dir, data_dir, game_home, own_minecraft_dir, version_dir};
pub use platform::{ASSET, LAUNCHER_APPIMAGE_ASSET, LAUNCHER_ASSET, PLATFORMS};
pub(crate) use platform::{CLIENT, GAME_APP};
#[cfg(test)]
use play::client_command;
pub use play::{
    agree_eula, eula_agreed, game_args, join_friend, launch, need_prepared, no_ai, server,
    server_command, server_dir, set_no_ai, split_args, Launched, Play, PlayError,
    LAUNCHER_RENDER_ENV, SIGN_IN_HINT,
};
pub use prepare::{
    prepare_in_process, prepare_step, preparer, set_prepare_step, sounds, MinecraftFrom,
    PrepareStep, Preparer,
};
pub use releases::{
    compare_versions, installable_updates, is_game_tag, is_release_or_nightly, latest,
    launcher_version, markdown_to_text, newest_release_or_nightly, own_tag, release_notes,
    release_order, releases, Identifier, Prerelease, Release, ReleaseOrder, CHECK_EVERY,
    LAUNCHER_TAG_PREFIX, OLD_LAUNCHER_TAG_PREFIX,
};
pub(crate) use resources::reuse_prepared_for;
pub use resources::{
    complete_assets, has_sounds, is_prepared, resources, resources_summary,
    reuse_prepared_everywhere,
};
#[cfg(test)]
pub(crate) use self_update::new_stage_folder;
#[cfg(windows)]
pub use self_update::remove_replaced_binaries;
pub(crate) use self_update::{already_in_place, installed_launcher_version};
pub use self_update::{
    app_bundle, appimage_path, cannot_replace_itself, cleanup_launcher_updates, download_page,
    exe_path, launcher_asset, mark_handed_over, package_managed_launcher, place_checked,
    place_staged, remove_stage, restart, self_update, self_update_allowed, stage_folder,
    stage_update, update_target, update_work_dir, Staged,
};
pub use settings::{Appearance, NativeProfile, Settings, WhilePlaying};
pub use updates::{check_updates, eclipse_update, launcher_update, releases_and_updates, Updates};
pub use versions::{
    after_install, disk_use, game_mod_api_level, game_version, install, install_tag, installed,
    mod_api_level, nightly_downgrade, no_version_installed, prune_old_versions, remove,
    remove_version, resolve_version, select_after_update,
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
