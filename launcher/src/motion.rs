//! The launcher's motion, as the website's launcher plays it (eclipse-site's `launcher.css` and
//! Tailwind's defaults): hovers ease over the design's quick curve (150 ms,
//! `cubic-bezier(0.4, 0, 0.2, 1)`); toasts and a new page arrive as `hzl-pop` does (a fade and a
//! 6 px rise over 200 ms); a switch's knob slides for 200 ms. Under reduced motion nothing moves:
//! everything is drawn where it ends. Menus open at once:
//! an animation inside a `deferred` menu kept its first frame (invisible) under GPUI Fast.
use eclipse_theme as tokens;
use gpui::{px, Animation, AnimationExt, AnyElement, Div, ElementId, IntoElement, Styled};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{LazyLock, Once};
use std::time::{Duration, Instant};

/// `hzl-pop`: `0.2s cubic-bezier(0.2, 0.7, 0.2, 1)`, from transparent and 6 px lower.
pub const POP: tokens::Motion = tokens::Motion {
    name: "pop",
    ms: 200,
    curve: [0.2, 0.7, 0.2, 1.],
};
pub const POP_RISE: f32 = 6.;

/// A switch's knob (`transition-[translate] duration-200`, Tailwind's default curve).
const SLIDE: tokens::Motion = tokens::Motion {
    name: "slide",
    ms: 200,
    curve: tokens::QUICK.curve,
};

pub fn still() -> bool {
    current() == Motion::Still
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Motion {
    Still,
    InFront,
    Always,
}

const FOLLOW_SYSTEM: u8 = 0;
const REDUCE: u8 = 1;
const DONT_REDUCE: u8 = 2;

static CHOICE: AtomicU8 = AtomicU8::new(FOLLOW_SYSTEM);
static SYSTEM_REDUCES: AtomicBool = AtomicBool::new(false);
static SYSTEM_READ: Once = Once::new();
static REREADING: AtomicBool = AtomicBool::new(false);

static PINNED: LazyLock<Option<Motion>> = LazyLock::new(pinned_by_env);

fn pinned_by_env() -> Option<Motion> {
    match std::env::var("ECLIPSE_LAUNCHER_MOTION").as_deref() {
        Ok("off" | "reduced") => Some(Motion::Still),
        Ok("always") => Some(Motion::Always),
        Ok("full") => Some(Motion::InFront),
        _ => None,
    }
}

pub(crate) fn current() -> Motion {
    if let Some(pinned) = *PINNED {
        return pinned;
    }
    let reduce = match CHOICE.load(Ordering::Relaxed) {
        REDUCE => true,
        DONT_REDUCE => false,
        _ => system_reduces_motion(),
    };
    if reduce {
        Motion::Still
    } else {
        Motion::InFront
    }
}

pub fn set_reduce_motion(choice: Option<bool>) {
    let choice = match choice {
        None => FOLLOW_SYSTEM,
        Some(true) => REDUCE,
        Some(false) => DONT_REDUCE,
    };
    CHOICE.store(choice, Ordering::Relaxed);
}

pub fn system_reduces_motion() -> bool {
    SYSTEM_READ.call_once(|| SYSTEM_REDUCES.store(read_system(), Ordering::Relaxed));
    SYSTEM_REDUCES.load(Ordering::Relaxed)
}

pub(crate) fn reread_system() {
    if PINNED.is_some() || REREADING.swap(true, Ordering::AcqRel) {
        return;
    }
    let reread = std::thread::Builder::new()
        .name("reduce-motion".into())
        .spawn(|| {
            SYSTEM_REDUCES.store(read_system(), Ordering::Relaxed);
            REREADING.store(false, Ordering::Release);
        });
    if reread.is_err() {
        REREADING.store(false, Ordering::Release);
    }
}

fn read_system() -> bool {
    #[cfg(target_os = "macos")]
    if objc2_app_kit::NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion() {
        return true;
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            SystemParametersInfoW, SPI_GETCLIENTAREAANIMATION,
        };
        let mut animate: i32 = 1;
        let read = unsafe {
            SystemParametersInfoW(
                SPI_GETCLIENTAREAANIMATION,
                0,
                (&mut animate as *mut i32).cast(),
                0,
            )
        };
        if read != 0 && animate == 0 {
            return true;
        }
    }
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    if linux_reduces_motion() {
        return true;
    }
    false
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn linux_reduces_motion() -> bool {
    let kde = std::env::var("XDG_CURRENT_DESKTOP")
        .is_ok_and(|desktop| desktop.to_ascii_uppercase().contains("KDE"));
    if kde {
        let kdeglobals = dirs::config_dir().map(|dir| dir.join("kdeglobals"));
        return kdeglobals
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|text| kde_animation_speed(&text))
            .is_some_and(|factor| factor <= 0.);
    }
    std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "enable-animations"])
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|out| out.status.success())
        .is_some_and(|out| String::from_utf8_lossy(&out.stdout).trim() == "false")
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn kde_animation_speed(kdeglobals: &str) -> Option<f32> {
    let mut in_kde = false;
    for line in kdeglobals.lines().map(str::trim) {
        if line.starts_with('[') {
            in_kde = line == "[KDE]";
            continue;
        }
        let factor = line
            .strip_prefix("AnimationDurationFactor")
            .and_then(|rest| rest.trim_start().strip_prefix('='));
        if let (true, Some(factor)) = (in_kde, factor) {
            return factor.trim().parse().ok();
        }
    }
    None
}

#[derive(Default)]
pub(crate) struct Clock {
    banked: Duration,
    running: Option<Instant>,
}

impl Clock {
    pub(crate) fn at(&self, now: Instant) -> Duration {
        self.banked + self.running.map_or(Duration::ZERO, |since| now - since)
    }

    pub(crate) fn set(&mut self, run: bool, now: Instant) {
        match (run, self.running) {
            (true, None) => self.running = Some(now),
            (false, Some(since)) => {
                self.banked += now - since;
                self.running = None;
            }
            _ => {}
        }
    }
}

/// `motion` as GPUI plays it: its length on its own curve; at its end from the first frame under
/// reduced motion.
pub fn eased(motion: tokens::Motion) -> Animation {
    let animation = Animation::new(motion.duration());
    if still() {
        return animation.with_easing(|_| 1.);
    }
    animation.with_easing(move |t| motion.ease(t))
}

thread_local! {
    /// Each switch's last state, and when it last changed.
    static SWITCHES: RefCell<HashMap<String, (bool, Option<Instant>)>> = RefCell::new(HashMap::new());
}

/// When switch `id`'s knob started sliding towards `on`, while it still slides: a switch seen
/// for the first time, or changed long ago, is already there.
fn slide(id: &str, on: bool, now: Instant) -> Option<Instant> {
    SWITCHES.with_borrow_mut(|switches| {
        let seen = switches.entry(id.to_string()).or_insert((on, None));
        if seen.0 != on {
            *seen = (on, Some(now));
        }
        seen.1
            .filter(|since| now.duration_since(*since) < SLIDE.duration())
    })
}

/// A switch's knob at `left` px when off and `left + travel` when on, sliding there when the
/// switch `id` changed in the last 200 ms.
pub fn knob(id: &str, on: bool, knob: Div, left: f32, travel: f32) -> AnyElement {
    let at = move |p: f32| left + travel * if on { p } else { 1. - p };
    match slide(id, on, Instant::now()) {
        Some(since) => {
            // Each slide gets its own animation id, from when it started, so it plays once.
            static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
            let start = *START.get_or_init(Instant::now);
            let key = since.saturating_duration_since(start).as_micros() as usize;
            knob.with_animation(
                ElementId::named_usize(format!("{id}-knob"), key),
                eased(SLIDE),
                move |d, p| d.left(px(at(p))),
            )
            .into_any_element()
        }
        None => knob.left(px(at(1.))).into_any_element(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_switch_slides_only_after_it_changed() {
        let t0 = Instant::now();
        assert_eq!(slide("test-a", true, t0), None, "first seen: already there");
        assert_eq!(slide("test-a", true, t0 + Duration::from_millis(50)), None);
        let flip = t0 + Duration::from_millis(100);
        assert_eq!(slide("test-a", false, flip), Some(flip));
        assert_eq!(
            slide("test-a", false, flip + Duration::from_millis(150)),
            Some(flip),
            "the same slide while it runs"
        );
        assert_eq!(
            slide("test-a", false, flip + Duration::from_millis(250)),
            None
        );
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    #[test]
    fn kde_animation_speed_comes_from_its_own_section() {
        let kdeglobals = "[General]\nAnimationDurationFactor=3\n\n[KDE]\nLookAndFeelPackage=org.kde.breeze\nAnimationDurationFactor = 0\n";
        assert_eq!(kde_animation_speed(kdeglobals), Some(0.));
        assert_eq!(
            kde_animation_speed("[KDE]\nAnimationDurationFactor=0.5"),
            Some(0.5)
        );
        assert_eq!(kde_animation_speed("[KDE]\nSingleClick=false"), None);
    }

    #[test]
    fn the_clock_stops_and_resumes_without_a_jump() {
        let start = Instant::now();
        let mut clock = Clock::default();
        clock.set(true, start);
        let s = Duration::from_secs(1);
        clock.set(false, start + 2 * s);
        assert_eq!(clock.at(start + 10 * s), 2 * s);
        clock.set(true, start + 10 * s);
        assert_eq!(clock.at(start + 11 * s), 3 * s);
    }
}
