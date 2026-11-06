use std::fs;
use std::path::{Path, PathBuf};

pub const OPTIONS: &str = "options.txt";
pub const GRAPHICS: &str = "eclipse-graphics.txt";
pub const EXTRAS: &str = "eclipse-extras.txt";
pub const CONTROLLER: &str = "eclipse-controller.txt";
pub const MODS_LIST: &str = "eclipse-mods.txt";
pub const SERVERS: &str = "servers.dat";
pub const PLAYER: &str = "eclipse-player.txt";
pub const BROWSER_LIST: &str = "mod-browser.json";
pub const BROWSER_FILES: &str = "mod-browser";
pub const BLUEMAP: &str = "eclipse-bluemap.txt";

pub const RESOURCE_PACKS: &str = "resourcepacks";
pub const SHADER_PACKS: &str = "shaderpacks";
pub const ECLIPSE_MODS: &str = "eclipse-mods";
pub const JAVA_MODS: &str = "mods";
pub const CONFIG: &str = "config";
pub const SAVES: &str = "saves";
pub const SCREENSHOTS: &str = "screenshots";
pub const LOGS: &str = "logs";
pub const CRASH_REPORTS: &str = "crash-reports";
// Far terrain's copies of what servers sent, rebuilt as the instance plays.
pub const SERVER_TERRAIN: &str = "server-terrain";
pub const SESSIONS: &str = "sessions";
pub const TELEMETRY: &str = "telemetry";
pub const CACHE: &str = ".eclipse-cache";
pub const INSTANCES: &str = "instances";
pub const PORTS: &str = "ports";
pub const PACKS: &str = "eclipse-packs";
pub const PROFILE: &str = "home";

pub const SETTINGS: [&str; 6] = [OPTIONS, GRAPHICS, EXTRAS, CONTROLLER, MODS_LIST, SERVERS];
pub const CONTENT: [&str; 4] = [RESOURCE_PACKS, SHADER_PACKS, ECLIPSE_MODS, CONFIG];
pub const FROM_PLAYER: [&str; 2] = [OPTIONS, PLAYER];
pub const REBUILT: [&str; 5] = [LOGS, CRASH_REPORTS, SERVER_TERRAIN, CACHE, BROWSER_FILES];
pub const PRIVATE: [&str; 2] = [SESSIONS, TELEMETRY];
pub const NESTED: [&str; 3] = [INSTANCES, PORTS, PACKS];
// The browser removes and replaces the files its list names, and the game asks the map servers in
// eclipse-bluemap.txt for tiles around the player: only the player and this launcher write them,
// never a pack or another launcher.
pub const ONLY_OURS: [&str; 2] = [BROWSER_LIST, BLUEMAP];

pub fn copy_leaves_out() -> Vec<&'static str> {
    [&NESTED[..], &REBUILT, &PRIVATE].concat()
}

pub mod world {
    pub const LEVEL: &str = "level.dat";
    pub const LOCK: &str = "session.lock";
    pub const DATAPACKS: &str = "datapacks";
    pub const PER_PLAYER: [&str; 3] = ["playerdata", "stats", "advancements"];
}

pub fn worlds(home: &Path) -> Vec<PathBuf> {
    fs::read_dir(home.join(SAVES))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join(world::LEVEL).is_file())
        .collect()
}

pub fn setting<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines()
        .filter_map(|l| l.split_once(':'))
        .find(|(k, _)| k.trim() == key)
        .map(|(_, v)| v.trim())
}

pub fn set_setting(path: &Path, key: &str, value: Option<&str>) -> Result<(), String> {
    let text = fs::read_to_string(path).unwrap_or_default();
    let mut value = value.map(|v| format!("{key}:{v}"));
    let mut lines: Vec<String> = Vec::new();
    for line in text.lines() {
        match line.split_once(':') {
            Some((k, _)) if k.trim() == key => lines.extend(value.take()),
            _ => lines.push(line.to_string()),
        }
    }
    lines.extend(value);
    let mut out = lines.join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    fs::write(path, out).map_err(|e| format!("{}: {e}", path.display()))
}

const PLAYER_KEY: &str = "name";

pub fn player_name(home: &Path) -> Option<String> {
    let text = fs::read_to_string(home.join(PLAYER)).ok()?;
    setting(&text, PLAYER_KEY)
        .filter(|n| !n.is_empty())
        .map(str::to_owned)
}

pub fn set_player_name(home: &Path, name: &str) -> Result<(), String> {
    let path = home.join(PLAYER);
    fs::write(&path, format!("{PLAYER_KEY}:{name}\n"))
        .map_err(|e| format!("{}: {e}", home.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_setting_is_changed_in_place_and_every_other_line_kept() {
        let dir = std::env::temp_dir().join(format!("eclipse-game-folder-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join(MODS_LIST);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            &path,
            "mod.a:true\r\nkey_x:key.mouse.left\nmod.b:false\nmod.b:true\n",
        )
        .unwrap();
        assert_eq!(
            setting(&fs::read_to_string(&path).unwrap(), "mod.a"),
            Some("true")
        );
        set_setting(&path, "mod.b", Some("true")).unwrap();
        set_setting(&path, "mod.c", Some("true")).unwrap();
        set_setting(&path, "mod.a", None).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "key_x:key.mouse.left\nmod.b:true\nmod.c:true\n"
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}
