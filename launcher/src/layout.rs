pub const DEFAULT_WINDOW: (f32, f32) = (1180., 760.);
pub const MIN_WINDOW: (f32, f32) = (960., 600.);

pub const SIDEBAR: f32 = 236.;
pub const PAGE_GUTTER: f32 = 32.;
pub const PAGE_MAX: f32 = 1120.;
pub const PAGE_FOOT: f32 = 48.;
pub const CONTENT_INSET: f32 = 14.;

pub const SHORT_WINDOW: f32 = 720.;
pub const HOME_HEAD_WIDE: f32 = 1280.;
pub const PACK_GRID_WIDE: f32 = 1280.;
pub const MODS_DETAIL_BESIDE: f32 = 1280.;
pub const ACCOUNT_CARDS_BESIDE: f32 = 1500.;
pub const SETTINGS_LIST_BESIDE: f32 = 1100.;
pub const DIALOG_ASIDE_WIDE: f32 = 1100.;
pub const SERVER_CARD_MIN: f32 = 540.;

pub fn content_width(window: f32) -> f32 {
    (window - SIDEBAR - 2. * PAGE_GUTTER).min(PAGE_MAX)
}
