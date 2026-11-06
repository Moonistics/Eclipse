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
pub const PACK_GRID_WIDE: f32 = 1280.;
pub const MODS_DETAIL_BESIDE: f32 = 1280.;
pub const ACCOUNT_CARDS_BESIDE: f32 = 1500.;
pub const SETTINGS_LIST_BESIDE: f32 = 1100.;
pub const DIALOG_ASIDE_WIDE: f32 = 1100.;
pub const SERVER_CARD_MIN: f32 = 540.;
pub const INSTANCE_COMPACT_BELOW: f32 = 600.;
pub const SETTINGS_CONTROL_BESIDE: f32 = 840.;

pub const MEASURE_NOTE: f32 = 560.;
pub const MEASURE_TEXT: f32 = 720.;

pub const FIELD_XS: f32 = 100.;
pub const FIELD_SM: f32 = 150.;
pub const FIELD_MD: f32 = 240.;
pub const FIELD_LG: f32 = 400.;
pub const FIELD_XL: f32 = MEASURE_NOTE;

pub fn sidebar(window: f32) -> f32 {
    if window < NARROW_WINDOW {
        SIDEBAR_NARROW
    } else {
        SIDEBAR
    }
}

pub fn gutter(window: f32) -> f32 {
    if window < NARROW_WINDOW {
        PAGE_GUTTER_NARROW
    } else {
        PAGE_GUTTER
    }
}

pub fn content_width(window: f32) -> f32 {
    (window - sidebar(window) - 2. * gutter(window)).min(PAGE_MAX)
}

pub fn page_margin(window: f32) -> f32 {
    (window - sidebar(window) - content_width(window)) / 2.
}

pub fn default_window(display: Option<(f32, f32)>) -> (f32, f32) {
    let (width, height) = DEFAULT_WINDOW;
    match display {
        Some((w, h)) => (
            width.min(w * DEFAULT_WINDOW_SHARE).max(MIN_WINDOW.0),
            height.min(h * DEFAULT_WINDOW_SHARE).max(MIN_WINDOW.1),
        ),
        None => DEFAULT_WINDOW,
    }
}
