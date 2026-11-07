//! A shortcut on the desktop that plays one instance: it runs
//! `eclipse-launcher play --instance <id>`, so the game starts without the launcher's window.
//!
//! - Linux: `<name> (Eclipse).desktop` on the desktop (executable, and marked trusted with `gio`
//!   where it exists, so GNOME starts it), and the same entry in the applications menu
//!   (`applications/eclipse-instance-<id>.desktop`, `desktop::data_home`).
//! - macOS: `<name> (Eclipse).app`, a small app whose program is a shell script, with Eclipse's
//!   icon. It has no Dock icon of its own (`LSUIElement`): the game's window is the one to see.
//! - Windows: `<name> (Eclipse).lnk`, a Shell Link as Microsoft's published [MS-SHLLINK] format
//!   lays it out, to the launcher's `.exe` with Eclipse's icon.
//!
//! Each shortcut names its instance's id inside it, so removing finds it after a rename.
//! `ECLIPSE_LAUNCHER_DESKTOP` points the desktop folder elsewhere, for tests.
//!
//! [MS-SHLLINK]: https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-shllink/
use crate::desktop;
use crate::desktop_seams::{self, Trash};
use std::fs;
use std::path::{Path, PathBuf};

const DESKTOP_ENV: &str = "ECLIPSE_LAUNCHER_DESKTOP";
const MAC_ICON: &[u8] = include_bytes!("../../packaging/icons/eclipse.icns");
/// The key a Linux shortcut names its instance with.
const INSTANCE_KEY: &str = "X-Eclipse-Instance";
/// And a macOS one, in its Info.plist.
const MAC_KEY: &str = "<key>EclipseInstance</key>";

/// The kinds of shortcut, one per system: all of them can be written and tested anywhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum System {
    Linux,
    Mac,
    Windows,
}

impl System {
    pub fn here() -> Self {
        if cfg!(target_os = "macos") {
            Self::Mac
        } else if cfg!(windows) {
            Self::Windows
        } else {
            Self::Linux
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Self::Linux => "desktop",
            Self::Mac => "app",
            Self::Windows => "lnk",
        }
    }
}

/// The desktop folder: `ECLIPSE_LAUNCHER_DESKTOP`, else the system's.
pub fn desktop_dir() -> Option<PathBuf> {
    std::env::var_os(DESKTOP_ENV)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(dirs::desktop_dir)
}

/// The shortcut's file name: the instance's name without the characters a file name can't
/// hold on any of the three systems.
pub fn file_name(name: &str, system: System) -> String {
    let clean: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c if c.is_control() => '-',
            c => c,
        })
        .collect();
    let clean = clean.trim().trim_matches('.');
    let clean = if clean.is_empty() { "Instance" } else { clean };
    format!("{clean} (Eclipse).{}", system.extension())
}

/// The id as a file name or bundle id part: letters, digits and `-`.
fn slug(id: &str) -> String {
    id.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// A Linux `.desktop` entry that plays the instance.
pub fn desktop_entry(exe: &Path, id: &str, name: &str) -> String {
    let field = |s: &str| s.replace(['\n', '\r'], " ");
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name={name}\n\
         Comment=Play {name} in Eclipse\n\
         Exec={} play --instance {}\n\
         TryExec={}\n\
         Icon=eclipse\n\
         Terminal=false\n\
         Categories=Game;\n\
         StartupWMClass=eclipse\n\
         {INSTANCE_KEY}={id}\n",
        desktop::quote(exe),
        desktop::quote(Path::new(id)),
        exe.display(),
        name = field(name),
        id = field(id),
    )
}

/// `'text'` for a POSIX shell.
fn sh_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

pub(crate) fn xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// A macOS app's files, by path inside the `.app`, and whether each is a program.
fn mac_app(exe: &Path, id: &str, name: &str) -> Vec<(&'static str, Vec<u8>, bool)> {
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleExecutable</key>
	<string>play</string>
	<key>CFBundleIdentifier</key>
	<string>dev.eclipse.instance.{}</string>
	<key>CFBundleName</key>
	<string>{}</string>
	<key>CFBundleIconFile</key>
	<string>eclipse</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>LSUIElement</key>
	<true/>
	{MAC_KEY}
	<string>{}</string>
</dict>
</plist>
"#,
        slug(id),
        xml(name),
        xml(id),
    );
    let script = format!(
        "#!/bin/sh\n# Plays the Eclipse instance {}.\nexec {} play --instance {}\n",
        name.replace('\n', " "),
        sh_quote(&exe.to_string_lossy()),
        sh_quote(id),
    );
    vec![
        ("Contents/Info.plist", plist.into_bytes(), false),
        ("Contents/MacOS/play", script.into_bytes(), true),
        ("Contents/Resources/eclipse.icns", MAC_ICON.to_vec(), false),
    ]
}

/// The arguments of a Windows shortcut: the id in quotes, with its own quotes escaped.
fn windows_args(id: &str) -> String {
    format!("play --instance \"{}\"", id.replace('"', "\\\""))
}

fn utf16(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

/// A Windows shortcut written by the shell itself (`IShellLinkW`), as Explorer and every installer
/// make one. The icon is the program's own, which is also how an uninstall finds shortcuts made for it.
#[cfg(windows)]
pub fn write_windows_link(
    path: &Path,
    exe: &Path,
    args: &str,
    comment: &str,
) -> Result<(), String> {
    use windows::core::{Interface, HSTRING};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    // SAFETY: plain COM calls on interfaces created and dropped here; COM is initialised for this
    // thread first and released after, when this call is the one that initialised it.
    unsafe {
        let init = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let made = (|| -> windows::core::Result<()> {
            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
            link.SetPath(&HSTRING::from(exe.as_os_str()))?;
            link.SetArguments(&HSTRING::from(args))?;
            if let Some(dir) = exe.parent() {
                link.SetWorkingDirectory(&HSTRING::from(dir.as_os_str()))?;
            }
            link.SetIconLocation(&HSTRING::from(exe.as_os_str()), 0)?;
            link.SetDescription(&HSTRING::from(comment))?;
            let file: IPersistFile = link.cast()?;
            file.Save(&HSTRING::from(path.as_os_str()), true)
        })();
        if init.is_ok() {
            CoUninitialize();
        }
        made.map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// Whether the shortcut at `path` plays the instance `id`.
fn plays(path: &Path, id: &str) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext {
        "desktop" => fs::read_to_string(path)
            .is_ok_and(|t| t.lines().any(|l| l == format!("{INSTANCE_KEY}={id}"))),
        "app" => fs::read_to_string(path.join("Contents/Info.plist"))
            .is_ok_and(|t| t.contains(&format!("{MAC_KEY}\n\t<string>{}</string>", xml(id)))),
        "lnk" => fs::read(path).is_ok_and(|b| {
            let want = utf16(&windows_args(id));
            b.windows(want.len()).any(|w| w == want)
        }),
        _ => false,
    }
}

fn write(path: &Path, bytes: &[u8], program: bool) -> Result<(), String> {
    crate::files::write(path, bytes)?;
    #[cfg(unix)]
    if program {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("{}: {e}", path.display()))?;
    }
    #[cfg(not(unix))]
    let _ = program;
    Ok(())
}

/// Makes the shortcut for `system` in `desktop` (and, for Linux, the menu entry under
/// `menu`, an `applications` folder). The instance's shortcut of the same name is rewritten;
/// one under an older name goes to `trash`. Returns what it wrote.
fn make_in(
    system: System,
    desktop: &Path,
    menu: Option<&Path>,
    exe: &Path,
    id: &str,
    name: &str,
    trash: Trash,
) -> Result<Vec<PathBuf>, String> {
    let path = desktop.join(file_name(name, system));
    let item = menu.map(|m| m.join(format!("eclipse-instance-{}.desktop", slug(id))));
    for old in find_in(desktop, menu, id) {
        if old != path && Some(&old) != item.as_ref() {
            trash(&old)?;
        }
    }
    if path.exists() && !plays(&path, id) {
        return Err(format!(
            "{} is already on the desktop, and isn't this instance's shortcut.",
            path.file_name().unwrap().to_string_lossy()
        ));
    }
    let mut made = vec![path.clone()];
    match system {
        System::Linux => {
            let entry = desktop_entry(exe, id, name);
            write(&path, entry.as_bytes(), true)?;
            if let Some(item) = item {
                write(&item, entry.as_bytes(), false)?;
                made.push(item);
            }
        }
        System::Mac => {
            for (inside, bytes, program) in mac_app(exe, id, name) {
                write(&path.join(inside), &bytes, program)?;
            }
        }
        #[cfg(windows)]
        System::Windows => {
            let comment = format!("Play {name} in Eclipse");
            write_windows_link(&path, exe, &windows_args(id), &comment)?;
        }
        #[cfg(not(windows))]
        System::Windows => return Err("Windows shortcuts are made on Windows.".into()),
    }
    Ok(made)
}

/// The instance's shortcuts in `desktop` and the menu.
pub fn find_in(desktop: &Path, menu: Option<&Path>, id: &str) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = fs::read_dir(desktop)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| plays(p, id))
        .collect();
    if let Some(item) = menu
        .map(|m| m.join(format!("eclipse-instance-{}.desktop", slug(id))))
        .filter(|p| plays(p, id))
    {
        found.push(item);
    }
    found.sort();
    found
}

/// Moves the instance's shortcuts to `trash`. Returns what it moved.
pub fn remove_in(
    desktop: &Path,
    menu: Option<&Path>,
    id: &str,
    trash: Trash,
) -> Result<Vec<PathBuf>, String> {
    let found = find_in(desktop, menu, id);
    for path in &found {
        trash(path)?;
    }
    Ok(found)
}

fn menu() -> Option<PathBuf> {
    (System::here() == System::Linux)
        .then(desktop::data_home)
        .flatten()
        .map(|d| d.join("applications"))
}

fn desktop_or_err() -> Result<PathBuf, String> {
    desktop_dir().ok_or_else(|| "This computer has no desktop folder.".into())
}

/// Makes the shortcut on this computer's desktop that plays `id`, starting `exe`.
pub fn make(exe: &Path, id: &str, name: &str) -> Result<Vec<PathBuf>, String> {
    let desktop = desktop_or_err()?;
    let made = make_in(
        System::here(),
        &desktop,
        menu().as_deref(),
        &desktop::target(exe),
        id,
        name,
        &desktop_seams::to_trash,
    )?;
    if System::here() == System::Linux {
        // GNOME starts a desktop file only once it is trusted; elsewhere this does nothing.
        let _ = std::process::Command::new("gio")
            .args(["set", "-t", "string"])
            .arg(&made[0])
            .args(["metadata::trusted", "true"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
    Ok(made)
}

/// The instance's shortcuts on this computer.
pub fn find(id: &str) -> Vec<PathBuf> {
    desktop_dir()
        .map(|d| find_in(&d, menu().as_deref(), id))
        .unwrap_or_default()
}

/// Moves the instance's shortcuts on this computer to the trash.
pub fn remove(id: &str) -> Result<Vec<PathBuf>, String> {
    remove_in(
        &desktop_or_err()?,
        menu().as_deref(),
        id,
        &desktop_seams::to_trash,
    )
}

#[cfg(test)]
#[path = "shortcut/tests.rs"]
mod tests;
