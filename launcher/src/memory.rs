/// The memory slider's stops, in MiB.
const SLIDER_STEP: u32 = 64;
/// The slider's top when the computer's memory is not known.
const UNKNOWN_TOTAL_MB: u32 = 64 * 1024;

/// Java heap size at `fraction` of a slider over `total_mb`, in MiB.
pub fn memory_at(fraction: f32, total_mb: u32) -> u32 {
    if fraction >= 1. {
        return total_mb.max(1);
    }
    let mb = fraction.max(0.) * total_mb as f32;
    ((mb / SLIDER_STEP as f32).round() as u32 * SLIDER_STEP).max(1)
}

/// What [`parse_memory`] says of text that isn't a memory limit.
#[derive(Debug, PartialEq, Eq)]
pub struct NotMemory;

/// A Java heap size as a player types it: MB, or GB with `gb`, and nothing or `off`
/// to restore the default. Nothing keeps a player from going low. Err when it is none of those.
pub fn parse_memory(typed: &str) -> Result<Option<u32>, NotMemory> {
    let typed = typed.trim().to_ascii_lowercase();
    if typed.is_empty() || typed == "off" {
        return Ok(None);
    }
    let (number, scale) = if let Some(gb) = typed.strip_suffix("gb").or(typed.strip_suffix('g')) {
        (gb, 1024)
    } else {
        let mb = typed.strip_suffix("mb").or(typed.strip_suffix('m'));
        (mb.unwrap_or(&typed), 1)
    };
    let number: u32 = number.trim().parse().map_err(|_| NotMemory)?;
    number.checked_mul(scale).map(Some).ok_or(NotMemory)
}

/// Where `mb` sits on a slider over `total_mb`; [`memory_at`]'s inverse.
pub fn memory_fraction(mb: u32, total_mb: u32) -> f32 {
    (mb as f32 / total_mb.max(1) as f32).min(1.)
}

/// This computer's memory in MiB, the top of the slider.
pub fn total_memory_mb() -> u32 {
    system_memory_bytes()
        .map(|bytes| (bytes >> 20) as u32)
        .filter(|mb| *mb > 0)
        .unwrap_or(UNKNOWN_TOTAL_MB)
}

#[cfg(windows)]
fn system_memory_bytes() -> Option<u64> {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    // SAFETY: all-zero is a valid value of this plain C struct, its length is set as the call
    // requires, and the call only writes into it.
    unsafe {
        let mut status: MEMORYSTATUSEX = std::mem::zeroed();
        status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        (GlobalMemoryStatusEx(&mut status) != 0).then_some(status.ullTotalPhys)
    }
}

#[cfg(target_os = "macos")]
fn system_memory_bytes() -> Option<u64> {
    let out = std::process::Command::new("sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

#[cfg(not(any(windows, target_os = "macos")))]
fn system_memory_bytes() -> Option<u64> {
    let info = std::fs::read_to_string("/proc/meminfo").ok()?;
    let line = info.lines().find(|l| l.starts_with("MemTotal:"))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kib * 1024)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_java_heap_slider_reaches_the_computers_memory() {
        assert_eq!(memory_at(1., 16384), 16384);
        assert_eq!(memory_at(0., 16384), 1);
        assert_eq!(memory_at(0.25, 16384), 4096);
        assert_eq!(memory_at(0.5, 8000), 4032);
        assert_eq!(memory_fraction(16384, 16384), 1.);
        assert_eq!(memory_fraction(4096, 16384), 0.25);
        // Saved on a computer with more memory.
        assert_eq!(memory_fraction(32768, 16384), 1.);
    }

    #[test]
    fn a_typed_memory_limit_is_read_in_mb_or_gb_with_no_lowest_value() {
        assert_eq!(parse_memory("2048"), Ok(Some(2048)));
        assert_eq!(parse_memory(" 512 MB "), Ok(Some(512)));
        assert_eq!(parse_memory("4gb"), Ok(Some(4096)));
        assert_eq!(parse_memory("2 G"), Ok(Some(2048)));
        assert_eq!(parse_memory("1"), Ok(Some(1)));
        assert_eq!(parse_memory(""), Ok(None));
        assert_eq!(parse_memory("off"), Ok(None));
        for bad in ["lots", "-5", "1.5gb", "4 tb", "99999999999", "9999999gb"] {
            assert_eq!(parse_memory(bad), Err(NotMemory), "{bad}");
        }
    }
}
