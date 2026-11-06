use serde::{de::DeserializeOwned, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    replace(path, bytes, false, false)
}

/// [`write`] for what only this computer's user may read (a secret): flushed, and on Unix
/// readable and writable by its owner alone from the moment it exists.
pub fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    replace(path, bytes, true, true)
}

// Without the flush a power cut can leave a file of the right length full of zeros.
fn replace(path: &Path, bytes: &[u8], flush: bool, private: bool) -> Result<(), String> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let error = |e: std::io::Error| format!("{}: {e}", path.display());
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        fs::create_dir_all(dir).map_err(error)?;
    }
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let part = path.with_file_name(format!(
        ".{name}.{}-{}.part",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    if private {
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    }
    #[cfg(not(unix))]
    let _ = private;
    let written = options
        .open(&part)
        .and_then(|mut f| {
            f.write_all(bytes)?;
            if flush {
                f.sync_all()?;
            }
            Ok(())
        })
        .and_then(|()| fs::rename(&part, path));
    if let Err(e) = written {
        let _ = fs::remove_file(&part);
        return Err(error(e));
    }
    Ok(())
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    serde_json::from_str(&text).map(Some).map_err(|e| {
        format!(
            "{} can't be read ({e}): nothing was changed",
            path.display()
        )
    })
}

pub fn write_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    replace(path, text.as_bytes(), true, false)
}

/// Copies folder `from` into `to`, all but the `leave_out` names at its top.
pub fn copy_dir(from: &Path, to: &Path, leave_out: &[&str]) -> std::io::Result<()> {
    copy_tree(from, to, &|path| {
        path.parent() == Some(from)
            && path
                .file_name()
                .is_some_and(|name| leave_out.iter().any(|n| name == *n))
    })
}

pub fn copy_dir_without(from: &Path, to: &Path, skip: &Path) -> std::io::Result<()> {
    copy_tree(from, to, &|path| path == skip)
}

fn copy_tree(from: &Path, to: &Path, skip: &dyn Fn(&Path) -> bool) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let path = entry.path();
        if skip(&path) {
            continue;
        }
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&path, &target, skip)?;
        } else {
            copy_file(&path, &target)?;
        }
    }
    Ok(())
}

/// A clone where the disk can (it takes no space until one of them changes); a reflink starts
/// with the default permissions, so the copy is given the file's.
pub fn copy_file(from: &Path, to: &Path) -> std::io::Result<()> {
    eclipse_prepare::clone_or_copy(from, to)?;
    fs::set_permissions(to, fs::metadata(from)?.permissions())
}

/// A canonical path without Windows' verbatim prefix: `canonicalize` gives `\\?\C:\...` there,
/// and some tools stat `\\?\C:` — the volume itself — and fail. A drive path loses its `\\?\`;
/// any other path is left as it is.
pub(crate) fn plain(path: PathBuf) -> PathBuf {
    match path.to_str().and_then(|p| p.strip_prefix(r"\\?\")) {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_record_that_cant_be_read_is_an_error_not_nothing() {
        let dir = std::env::temp_dir().join(format!("eclipse-files-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("list.json");
        assert_eq!(read_json::<Vec<u32>>(&path), Ok(None));
        write_json(&path, &[1, 2]).unwrap();
        assert_eq!(read_json::<Vec<u32>>(&path), Ok(Some(vec![1, 2])));
        fs::write(&path, "[1, 2").unwrap();
        assert!(read_json::<Vec<u32>>(&path).is_err());
        assert_eq!(
            fs::read_dir(&dir).unwrap().count(),
            1,
            "no part file is left"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A canonical path loses Windows' verbatim prefix on a drive (horizon-mixins' output
    /// check stats `\\?\C:`, the volume itself, which Windows refuses, os error 1), and keeps
    /// it where there is no drive form.
    #[test]
    fn a_canonical_windows_path_loses_only_a_drives_verbatim_prefix() {
        let plain = |p: &str| plain(PathBuf::from(p));
        assert_eq!(
            plain(r"\\?\C:\Users\me\AppData\Roaming\eclipse-launcher\native"),
            PathBuf::from(r"C:\Users\me\AppData\Roaming\eclipse-launcher\native")
        );
        // A share has no drive form to fall back to; other paths have no prefix.
        assert_eq!(
            plain(r"\\?\UNC\server\share\native"),
            PathBuf::from(r"\\?\UNC\server\share\native")
        );
        assert_eq!(
            plain("/home/me/.local/share/eclipse-launcher/native"),
            PathBuf::from("/home/me/.local/share/eclipse-launcher/native")
        );
    }
}
