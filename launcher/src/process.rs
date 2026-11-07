//! The programs the launcher runs in the background: on Windows they open no console window
//! (the launcher has none to share), and keep the streams their caller gives them.
use std::ffi::OsStr;
use std::process::Command;

pub fn command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    hide_console(&mut command);
    command
}

/// Windows' flag for a console program started without a console window. `creation_flags` sets
/// the whole value, so a caller adding its own flags includes this one
/// ([`crate::rust_toolchain::low_priority`]).
#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// [`command`] for a `Command` made elsewhere.
pub fn hide_console(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

pub struct Running {
    pub name: String,
    pub command: String,
}

pub fn running() -> Result<Vec<Running>, String> {
    #[cfg(windows)]
    let output = command("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Get-CimInstance Win32_Process | Select-Object Name,ExecutablePath,CommandLine | ConvertTo-Json -Compress",
        ])
        .output();
    #[cfg(not(windows))]
    let output = command("ps")
        .args(["-A", "-ww", "-o", "comm=,args="])
        .output();
    let output = output.map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    #[cfg(windows)]
    {
        let value: serde_json::Value =
            serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
        let entries = value.as_array().cloned().unwrap_or_else(|| vec![value]);
        Ok(entries
            .into_iter()
            .map(|p| Running {
                name: p["Name"].as_str().unwrap_or("").to_owned(),
                command: format!(
                    "{} {}",
                    p["ExecutablePath"].as_str().unwrap_or(""),
                    p["CommandLine"].as_str().unwrap_or("")
                ),
            })
            .collect())
    }
    #[cfg(not(windows))]
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().split_once(char::is_whitespace))
        .map(|(name, command)| Running {
            name: name.to_owned(),
            command: command.trim_start().to_owned(),
        })
        .collect())
}
