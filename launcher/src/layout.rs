use eclipse_theme::glass::space;

pub const DEFAULT_WINDOW: (f32, f32) = (1180., 760.);
pub const MIN_WINDOW: (f32, f32) = (800., 500.);
pub const DEFAULT_WINDOW_SHARE: f32 = 0.9;

pub const SIDEBAR: f32 = 236.;
pub const SIDEBAR_NARROW: f32 = 208.;
pub const NARROW_WINDOW: f32 = 960.;
pub const PAGE_GUTTER: f32 = space::XXL;
pub const PAGE_GUTTER_NARROW: f32 = space::XL;
pub const PAGE_MAX: f32 = 1120.;
pub const PAGE_FOOT: f32 = 48.;
pub const CONTENT_INSET: f32 = space::MD + space::XXS;
pub const CARD_GAP: f32 = space::LG;

pub const SHORT_WINDOW: f32 = 720.;
pub const HOME_HEAD_WIDE: f32 = 1280.;
pub const SCREENSHOT_GRID_WIDE: f32 = 1280.;
pub const MODS_DETAIL_BESIDE: f32 = 1280.;
pub const ACCOUNT_CARDS_BESIDE: f32 = 1500.;
pub const SETTINGS_LIST_BESIDE: f32 = 1440.;
pub const DIALOG_ASIDE_WIDE: f32 = 1100.;
pub const SERVER_CARD_MIN: f32 = 540.;
pub const INSTANCE_COMPACT_BELOW: f32 = 600.;
pub const SETTINGS_CONTROL_BESIDE: f32 = 840.;

pub const SETTINGS_RAIL: f32 = FIELD_MD;

pub const MEASURE_NOTE: f32 = 560.;
pub const MEASURE_TEXT: f32 = 720.;

pub const FIELD_XS: f32 = 100.;
pub const FIELD_SM: f32 = 150.;
pub const FIELD_MD: f32 = 240.;
pub const FIELD_LG: f32 = 400.;
pub const FIELD_XL: f32 = MEASURE_NOTE;

fn narrowest_sidebar() -> f32 {
    if crate::ui::pixel_text_grows() {
        SIDEBAR
    } else {
        SIDEBAR_NARROW
    }
}

pub fn sidebar(window: f32) -> f32 {
    sidebar_at(window, narrowest_sidebar())
}

fn sidebar_at(window: f32, narrowest: f32) -> f32 {
    (SIDEBAR - (NARROW_WINDOW - window).max(0.)).max(narrowest)
}

pub fn gutter(window: f32) -> f32 {
    gutter_at(window, narrowest_sidebar())
}

fn gutter_at(window: f32, narrowest: f32) -> f32 {
    let narrowing = NARROW_WINDOW - (SIDEBAR - narrowest);
    (PAGE_GUTTER - (narrowing - window).max(0.) / 2.).max(PAGE_GUTTER_NARROW)
}

pub fn content_width(window: f32) -> f32 {
    (window - sidebar(window) - 2. * gutter(window)).min(PAGE_MAX)
}

pub fn page_margin(window: f32) -> f32 {
    (window - sidebar(window) - content_width(window)) / 2.
}

pub fn settings_controls_beside(window: f32) -> bool {
    let list = if window < SETTINGS_LIST_BESIDE {
        0.
    } else {
        SETTINGS_RAIL + CARD_GAP
    };
    content_width(window) - list >= SETTINGS_CONTROL_BESIDE
}

pub fn default_window(display: Option<(f32, f32)>) -> (f32, f32) {
    let (width, height) = DEFAULT_WINDOW;
    match display {
        Some((w, h)) => (
            width.min(w * DEFAULT_WINDOW_SHARE).max(MIN_WINDOW.0.min(w)),
            height
                .min(h * DEFAULT_WINDOW_SHARE)
                .max(MIN_WINDOW.1.min(h)),
        ),
        None => DEFAULT_WINDOW,
    }
}

pub fn min_window(display: Option<(f32, f32)>) -> (f32, f32) {
    match display {
        Some((w, h)) => (MIN_WINDOW.0.min(w), MIN_WINDOW.1.min(h)),
        None => MIN_WINDOW,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_never_widens_as_the_window_narrows() {
        for narrowest in [SIDEBAR_NARROW, SIDEBAR] {
            let content = |w: f32| {
                (w - sidebar_at(w, narrowest) - 2. * gutter_at(w, narrowest)).min(PAGE_MAX)
            };
            let mut last = content(MIN_WINDOW.0);
            for w in (MIN_WINDOW.0 as u32 + 1)..=2000 {
                let now = content(w as f32);
                assert!(now >= last, "{w} px wide: {now} after {last}");
                last = now;
            }
        }
        assert_eq!(sidebar_at(NARROW_WINDOW, SIDEBAR_NARROW), SIDEBAR);
        assert_eq!(sidebar_at(MIN_WINDOW.0, SIDEBAR_NARROW), SIDEBAR_NARROW);
        assert_eq!(gutter_at(MIN_WINDOW.0, SIDEBAR_NARROW), PAGE_GUTTER_NARROW);
    }

    #[test]
    fn a_window_never_opens_larger_than_the_screen() {
        assert_eq!(default_window(None), DEFAULT_WINDOW);
        assert_eq!(default_window(Some((2560., 1440.))), DEFAULT_WINDOW);
        let small = (910., 480.);
        let (w, h) = default_window(Some(small));
        assert!(w <= small.0 && h <= small.1, "{w}x{h}");
        assert_eq!(min_window(Some(small)), (MIN_WINDOW.0, small.1));
        assert_eq!(min_window(None), MIN_WINDOW);
    }
}
