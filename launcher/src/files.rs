use serde::{de::DeserializeOwned, Serialize};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

pub fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    replace(path, bytes, false)
}

// Without the flush a power cut can leave a file of the right length full of zeros.
fn replace(path: &Path, bytes: &[u8], flush: bool) -> Result<(), String> {
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
    let written = fs::File::create(&part)
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
    replace(path, text.as_bytes(), true)
}

pub fn copy_dir(from: &Path, to: &Path, leave_out: &[&str]) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if leave_out.iter().any(|n| name == *n) {
            continue;
        }
        let target = to.join(&name);
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target, &[])?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
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
}
