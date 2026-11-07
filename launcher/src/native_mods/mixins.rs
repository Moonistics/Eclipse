use super::run::run;
use super::step::Step;
use crate::native_cache::{self, Dirs};
use crate::rust_toolchain::Toolchain;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// horizon-mixins, for bundles that don't name the revision they were tested with.
pub(super) const REPOSITORY: &str = "horizon-rust/horizon-mixins";
pub const MIXINS_REV: &str = "5a64af938ec86b00d53d16a408aabc0038fb3e57";
pub(super) const DEFAULT_PRIORITY: i64 = 0;
pub(super) const STAGE_PLAN: &str = ".horizon-mixins-stage.json";
pub(super) const STAGE_SCHEMA: u64 = 6;

pub(super) fn stage_plan(stage: &Path) -> Option<serde_json::Value> {
    fs::read(stage.join(STAGE_PLAN))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .filter(|plan| plan["schema"].as_u64() == Some(STAGE_SCHEMA))
}

pub(super) struct Diagnostic {
    pub(super) code: Option<String>,
    pub(super) message: Option<String>,
    mod_id: Option<String>,
    injection_id: Option<String>,
    pub(super) file: Option<String>,
}

impl Diagnostic {
    pub(super) fn read(text: &str) -> Option<Self> {
        let value: serde_json::Value = serde_json::from_str(&text[text.find('{')?..]).ok()?;
        let field = |key: &str| value[key].as_str().map(str::to_owned);
        Some(Self {
            code: field("code"),
            message: field("message"),
            mod_id: field("mod_id"),
            injection_id: field("injection_id"),
            file: field("file"),
        })
    }

    pub(super) fn mod_id(&self) -> Option<&str> {
        self.mod_id
            .as_deref()
            .or_else(|| self.file.as_deref().and_then(mod_id_from_file))
    }
}

/// Extracts the mod id from a mixin manifest or mod file path (e.g. `.../mixins.incoming/<mod_id>/...`
/// or `.../mods.incoming/<mod_id>/...`).
fn mod_id_from_file(file: &str) -> Option<&str> {
    for prefix in native_cache::STAGING {
        if let Some(pos) = file.find(prefix) {
            let rest = &file[pos + prefix.len()..];
            let sub = rest.trim_start_matches(['/', '\\']);
            if let Some(end) = sub.find(['/', '\\']) {
                let id = &sub[..end];
                if !id.is_empty() {
                    return Some(id);
                }
            }
        }
    }
    None
}

/// A horizon-mixins diagnostic (`{"code", "message", "file"}`) in one line, led by the mod it
/// names (`mod_id`, its `injection_id`, or extracted from its `file` path for `E_MODULE_CONFLICT`),
/// so a player with several mods sees which one doesn't apply to the build's Eclipse: their files'
/// paths are deep in the launcher's folder.
pub(super) fn diagnostic(text: &str) -> Option<String> {
    let found = Diagnostic::read(text)?;
    let culprit = match (found.mod_id(), found.injection_id.as_deref()) {
        (Some(m), Some(i)) => format!("{m} ({i}): "),
        (Some(m), None) => format!("{m}: "),
        _ => String::new(),
    };
    let mut line = format!(
        "{culprit}{}: {}",
        found.code.as_deref()?,
        found.message.as_deref()?
    );
    if let Some(file) = &found.file {
        line += &format!(" ({file})");
    }
    Some(line)
}

/// horizon-mixins at commit `rev`, built once with the launcher's cargo (`cargo install --locked`);
/// `progress` hears [`Step::Tool`] only when it is built.
pub(super) fn tool(
    rust: &Toolchain,
    rev: &str,
    progress: &impl Fn(Step),
) -> Result<PathBuf, String> {
    let dir = Dirs::current().tools().join(&rev[..12]);
    let bin = dir
        .join("bin")
        .join(format!("horizon-mixins{}", std::env::consts::EXE_SUFFIX));
    if !bin.is_file() {
        progress(Step::Tool);
        let mut cmd = rust.cargo_command();
        cmd.args(["install", "--locked", "--git"])
            .arg(format!("https://github.com/{REPOSITORY}"))
            .args(["--rev", rev, "--root"])
            .arg(&dir)
            .arg("horizon-mixins-cli");
        run(cmd, "Building the mod tools", |_| {})?;
    }
    Ok(bin)
}

/// `horizon-mixins apply`: the mods in `mods` applied to `source`, into `out`. It runs
/// `cargo metadata`, so it gets the launcher's toolchain like a build does: on a computer with no
/// Rust of its own, a plain command found no cargo (E_METADATA).
pub(super) fn apply(
    rust: &Toolchain,
    tool: &Path,
    source: &Path,
    mods: &Path,
    out: &Path,
) -> Command {
    let mut cmd = rust.command(tool);
    cmd.arg("apply").arg("--workspace").arg(source);
    cmd.arg("--mods").arg(mods).arg("--out").arg(out);
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_mods::tests::toolchains;

    #[test]
    fn horizon_mixins_finds_the_launchers_cargo() {
        for rust in toolchains("/rust/1.99.1-x") {
            horizon_mixins_finds(&rust);
        }
    }

    fn horizon_mixins_finds(rust: &Toolchain) {
        let cmd = apply(
            rust,
            Path::new("/tools/horizon-mixins"),
            Path::new("/src"),
            Path::new("/mods"),
            Path::new("/out"),
        );
        let path = cmd
            .get_envs()
            .find(|(k, _)| *k == "PATH")
            .and_then(|(_, v)| v)
            .unwrap();
        assert_eq!(
            std::env::split_paths(path).next(),
            Some(rust.dir.join("bin"))
        );
    }

    #[test]
    fn tool_errors_read_in_one_line() {
        let err = "{\n  \"code\": \"E_REVISION\",\n  \"message\": \"requires exact Eclipse revision a\",\n  \"file\": \"m.toml\"\n}";
        assert_eq!(
            diagnostic(err).as_deref(),
            Some("E_REVISION: requires exact Eclipse revision a (m.toml)")
        );
    }

    #[test]
    fn a_tool_error_starts_with_the_mod_that_does_not_apply() {
        // horizon-mixins e8cd0aa on Eclipse 998b1d8 with fast_items 0.1.0 and appleskin 1.0.0
        // (made for ba6344a), as a player's build was on 2026-11-04.
        let err = r#"{
  "code": "E_TARGET_NOT_FOUND",
  "message": "no source function 'eclipse_client::game::titles::Game<'_>::draw_titles'",
  "file": "/home/p/.local/share/eclipse-launcher/native/mixins.incoming/appleskin/eclipse-mixin.toml",
  "mod_id": "appleskin",
  "injection_id": "hud",
  "target": "eclipse_client::game::titles::Game<'_>::draw_titles"
}"#;
        let line = diagnostic(err).unwrap();
        assert!(
            line.starts_with("appleskin (hud): E_TARGET_NOT_FOUND: no source function"),
            "{line}"
        );
        // About a whole mod, no injection in it.
        let err = r#"{"code": "E_REVISION", "message": "pinned to a", "mod_id": "chop"}"#;
        assert_eq!(
            diagnostic(err).as_deref(),
            Some("chop: E_REVISION: pinned to a")
        );
    }

    #[test]
    fn a_tool_error_without_mod_id_extracts_culprit_from_file_path() {
        // horizon-mixins e8cd0aa emits E_MODULE_CONFLICT without mod_id (issue #9391).
        let err = r#"{
  "code": "E_MODULE_CONFLICT",
  "message": "conflict in module eclipse_physics",
  "file": "/home/p/.local/share/eclipse-launcher/native/mixins.incoming/eclipse_physics/eclipse-mixin.toml"
}"#;
        let line = diagnostic(err).unwrap();
        assert!(
            line.starts_with(
                "eclipse_physics: E_MODULE_CONFLICT: conflict in module eclipse_physics"
            ),
            "{line}"
        );

        // Windows path with backslashes
        let err_win = r#"{
  "code": "E_MODULE_CONFLICT",
  "message": "conflict in module biomecompass",
  "file": "C:\\Users\\p\\AppData\\Local\\eclipse-launcher\\native\\mixins.incoming\\biomecompass\\eclipse-mixin.toml"
}"#;
        let line_win = diagnostic(err_win).unwrap();
        assert!(
            line_win
                .starts_with("biomecompass: E_MODULE_CONFLICT: conflict in module biomecompass"),
            "{line_win}"
        );

        // mods.incoming path
        let err_mods = r#"{
  "code": "E_MODULE_CONFLICT",
  "message": "conflict in module worldedit",
  "file": "/home/p/.local/share/eclipse-launcher/native/mods.incoming/worldedit/eclipse-mod.toml"
}"#;
        let line_mods = diagnostic(err_mods).unwrap();
        assert!(
            line_mods.starts_with("worldedit: E_MODULE_CONFLICT: conflict in module worldedit"),
            "{line_mods}"
        );
    }
}
