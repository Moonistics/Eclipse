use super::paths::game_home;
use std::fs;
use std::path::{Path, PathBuf};

/// The game's logs (`latest.log` and the day archives), as it writes them in its home.
pub fn logs_dir() -> PathBuf {
    game_home().join("logs")
}

/// Where the game writes `crash-<date>_<time>-client.txt` when it crashes.
pub fn crash_reports_dir() -> PathBuf {
    game_home().join("crash-reports")
}

/// The crash reports in `dir`, oldest first (their names sort by time).
fn crash_reports_in(dir: &Path) -> Vec<PathBuf> {
    let mut reports: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("crash-") && n.ends_with(".txt"))
        })
        .collect();
    reports.sort();
    reports
}

pub fn newest_crash_report() -> Option<PathBuf> {
    crash_reports_in(&crash_reports_dir()).pop()
}

/// The newest crash report the game playing in `home` wrote at or after `since` (one from the
/// run that started then).
pub fn crash_report_since(home: &Path, since: std::time::SystemTime) -> Option<PathBuf> {
    crash_report_in_since(&home.join("crash-reports"), since)
}

pub(super) fn crash_report_in_since(dir: &Path, since: std::time::SystemTime) -> Option<PathBuf> {
    crash_reports_in(dir).into_iter().rev().find(|p| {
        fs::metadata(p)
            .and_then(|m| m.modified())
            .is_ok_and(|t| t >= since)
    })
}

/// The error a crash report describes, on one line: the lines between its description and the
/// stack (`panicked at src/game.rs:10:5: boom`).
pub fn crash_summary(report: &str) -> Option<String> {
    let mut lines = report
        .lines()
        .skip_while(|l| !l.starts_with("Description:"));
    lines.next()?;
    let error: Vec<&str> = lines
        .skip_while(|l| l.trim().is_empty())
        .take_while(|l| !l.trim().is_empty() && !l.starts_with('\t'))
        .map(str::trim)
        .collect();
    (!error.is_empty()).then(|| error.join(" "))
}

/// The line the game prints when it has drawn its first frame.
pub const GAME_FIRST_FRAME: &str = "Eclipse: first frame";

/// Whether a game's output ends in a crash while its Vulkan renderer started: an adapter line
/// (`Eclipse renderer: <GPU> (Vulkan, <type>)`) and no first frame. A native crash in a graphics
/// driver leaves no crash report, only those lines (#746).
pub fn vulkan_start_crashed<S: AsRef<str>>(lines: impl IntoIterator<Item = S>) -> bool {
    let mut vulkan = false;
    for line in lines {
        let line = line.as_ref();
        if line.starts_with(GAME_FIRST_FRAME) {
            return false;
        }
        if let Some(adapter) = line.strip_prefix("Eclipse renderer: ") {
            // The adapter line, not one of the start's steps: "<GPU> (<backend>, <type>)".
            if adapter.ends_with(')') && adapter.contains(" (") {
                vulkan = adapter.contains(" (Vulkan, ");
            }
        }
    }
    vulkan
}

/// The game's graphics settings file in its home (the game's `graphics.rs`).
pub(super) const GRAPHICS_FILE: &str = "eclipse-graphics.txt";

/// The graphics API (the game's setting name, and its label) Auto switches to after a native
/// crash at the Vulkan renderer's start: DirectX 12 on Windows (#746), OpenGL on Linux, where
/// Auto also runs Mesa's hasvk on Ivy Bridge and Haswell, which Mesa itself calls incomplete
/// there (#4293). `None` where the game has no other API (macOS: Metal only).
pub fn vulkan_crash_fallback(os: &str) -> Option<(&'static str, &'static str)> {
    match os {
        "windows" => Some(("dx12", "DirectX 12")),
        "macos" => None,
        _ => Some(("opengl", "OpenGL")),
    }
}

/// Sets `graphicsApi:<api>` in `home`'s graphics settings when it is unset or `auto`, keeping
/// every other line. True when it changed; a player's own choice stays.
pub fn switch_from_auto(home: &Path, api: &str) -> Result<bool, String> {
    let path = home.join(GRAPHICS_FILE);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let mut lines: Vec<String> = Vec::new();
    let mut found = false;
    for line in text.lines() {
        match line.split_once(':') {
            Some((key, value)) if key.trim() == "graphicsApi" => {
                if value.trim() != "auto" {
                    return Ok(false);
                }
                found = true;
                lines.push(format!("graphicsApi:{api}"));
            }
            _ => lines.push(line.to_owned()),
        }
    }
    if !found {
        lines.push(format!("graphicsApi:{api}"));
    }
    fs::create_dir_all(home).map_err(|e| format!("{}: {e}", home.display()))?;
    fs::write(&path, lines.join("\n") + "\n").map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(true)
}

/// The last `lines` lines of `file`, or why it cannot be read.
pub(super) fn tail_of(file: &Path, lines: usize) -> Result<String, String> {
    let bytes = fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let text = String::from_utf8_lossy(&bytes);
    let all: Vec<&str> = text.lines().collect();
    Ok(all[all.len().saturating_sub(lines)..].join("\n") + "\n")
}

/// `logs`: the end of the game's latest.log, or with `crash` its newest crash report. The path
/// read comes first.
pub fn read_logs(lines: usize, crash: bool) -> Result<(PathBuf, String), String> {
    if crash {
        let path = newest_crash_report()
            .ok_or_else(|| format!("No crash report in {}.", crash_reports_dir().display()))?;
        let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        return Ok((path, text));
    }
    let path = logs_dir().join("latest.log");
    let text = tail_of(&path, lines.max(1))?;
    Ok((path, text))
}
