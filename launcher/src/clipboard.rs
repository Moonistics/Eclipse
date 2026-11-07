use crate::toast;
use eclipse_launcher::i18n::{tr, trf};
use gpui::App;

const COPIED_SHOWN: usize = 40;

/// Copies to the clipboard, or into `ECLIPSE_LAUNCHER_CLIPBOARD` when a test run sets it
/// (`eclipse_launcher::desktop_seams`).
pub(super) fn copy_to_clipboard(cx: &mut App, item: gpui::ClipboardItem) {
    match item.text() {
        Some(text) if !text.contains('\n') && text.chars().count() <= COPIED_SHOWN => {
            toast::say(trf("Copied {text}", &[("text", &text)]))
        }
        Some(_) => toast::say(tr("Copied to the clipboard")),
        None => toast::say(tr("Copied the picture")),
    }
    if let Some(file) = eclipse_launcher::desktop_seams::clipboard_file() {
        let bytes = match item.entries().first() {
            Some(gpui::ClipboardEntry::Image(image)) => image.bytes.clone(),
            _ => item.text().unwrap_or_default().into_bytes(),
        };
        if let Err(e) = std::fs::write(&file, bytes) {
            eprintln!("{}: {e}", file.display());
        }
        return;
    }
    cx.write_to_clipboard(item.clone());
    #[cfg(target_os = "linux")]
    if let Some(text) = item.text() {
        sync_system_clipboard(text);
    }
}

/// GPUI's Wayland clipboard sends the last key press's serial, which a compositor ignores
/// after a mouse click: hand the text to the desktop's own tool too, the first one present
/// for this session (`wl-copy` on Wayland, `xclip` or `xsel` on X11).
#[cfg(target_os = "linux")]
fn sync_system_clipboard(text: String) {
    use std::io::Write;
    use std::process::{Command, Stdio};
    const TOOLS: [(&str, &str, &[&str]); 3] = [
        ("WAYLAND_DISPLAY", "wl-copy", &[]),
        ("DISPLAY", "xclip", &["-selection", "clipboard"]),
        ("DISPLAY", "xsel", &["--clipboard", "--input"]),
    ];
    if text.is_empty() {
        return;
    }
    // Off the UI thread: a tool can take its time.
    std::thread::spawn(move || {
        for (session, tool, args) in TOOLS {
            if std::env::var_os(session).is_none() {
                continue;
            }
            let Ok(mut child) = Command::new(tool)
                .args(args)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            else {
                continue;
            };
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            if child.wait().is_ok_and(|status| status.success()) {
                return;
            }
        }
    });
}
