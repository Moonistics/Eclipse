//! The launcher's look ("Corona" in `docs/design-system.md`): quiet charcoal cards over the
//! eclipse scene, white text at a few opacities, one amber accent. Geist sets the text, Monocraft
//! the titles, navigation and buttons.

pub const ACCENT: u32 = 0xf5a524;
pub const TINT: f32 = 0.06;
pub const BLUR: f32 = 24.;
pub const PAGE: u32 = 0x08090f;
pub const BASE: u32 = 0x10131c;
pub const PLATE_ALPHA: f32 = 1.;
pub const FLOAT: u32 = 0x161a25;
pub const EDGE: f32 = 0.07;
pub const ON_LIGHT: u32 = 0x14151c;
/// A running game or server, and a ready instance.
pub const LIVE: u32 = 0x4ade80;
/// Busy (pulsing) and partly-working states: a red-orange well off the accent's amber.
pub const WARN: u32 = 0xff9a6c;
/// Destructive menu rows, and the fill under one while hovered.
pub const DANGER: u32 = 0xff8a8a;
pub const DANGER_HOVER: u32 = 0xff6b6b;
pub const LIVE_LIGHT: u32 = 0x15803d;
pub const WARN_LIGHT: u32 = 0xc2410c;
pub const DANGER_LIGHT: u32 = 0xdc2626;
/// The Discord button and its hover.
pub const DISCORD: u32 = 0x5865f2;
pub const DISCORD_HOVER: u32 = 0x4752c4;
pub const SCRIM: u32 = 0x05060a;

pub const FONT: &str = crate::FONT;
pub const DISPLAY_FONT: &str = "Monocraft";
pub const HEADING_FONT: &str = "Monocraft";
pub const LABEL_FONT: &str = "Monocraft";

pub mod radius {
    pub const MODAL: f32 = 14.;
    pub const PANEL: f32 = 12.;
    pub const POPOVER: f32 = 10.;
    pub const ROW: f32 = 8.;
    pub const CONTROL: f32 = 6.;
}

pub mod ink {
    pub const FULL: f32 = 1.;
    pub const STRONG: f32 = 0.86;
    pub const BODY: f32 = 0.72;
    pub const MUTED: f32 = 0.6;
    pub const DIM: f32 = 0.5;
}

pub mod fill {
    pub const HOVER: f32 = 0.05;
    pub const FIELD: f32 = 0.07;
    pub const BUTTON: f32 = 0.08;
    pub const SELECTED: f32 = 0.1;
    pub const ACTIVE: f32 = 0.12;
    pub const WELL: f32 = 0.04;
}

pub const FIELD_BG: u32 = 0x1a1e29;
pub const FIELD_BG_FOCUS: u32 = 0x222735;

/// The look's colours for one accent and glass setting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glass {
    pub accent: u32,
    /// The accent mixed half with white: progress bars, ticks, the focus ring, links.
    pub accent_soft: u32,
    /// The charcoal with a trace of the accent: what the plates are filled with.
    pub base: u32,
    /// Text on the accent: white, or [`ON_LIGHT`] on a light accent.
    pub on_accent: u32,
    /// A plate's fill and its opacity.
    pub plate: u32,
    pub plate_alpha: f32,
    pub tint: f32,
    pub blur: f32,
}

impl Glass {
    pub fn new(accent: u32, tint: f32, blur: f32) -> Glass {
        let base = mix_oklab(accent, BASE, 0.03);
        Glass {
            accent,
            accent_soft: mix_oklab(accent, 0xffffff, 0.62),
            base,
            on_accent: if luminance(accent) > 0.4 {
                ON_LIGHT
            } else {
                0xffffff
            },
            plate: base,
            plate_alpha: PLATE_ALPHA,
            tint,
            blur,
        }
    }

    pub fn default_look() -> Glass {
        Glass::new(ACCENT, TINT, BLUR)
    }

    /// Every colour by its CSS name (`--glass-<name>`), for `tokens.css`.
    pub fn css(&self) -> String {
        let hex = |c: u32| format!("#{c:06x}");
        [
            ("accent", hex(self.accent)),
            ("accent-soft", hex(self.accent_soft)),
            ("base", hex(self.base)),
            ("on-accent", hex(self.on_accent)),
            ("page", hex(PAGE)),
            ("live", hex(LIVE)),
            ("warn", hex(WARN)),
            ("danger", hex(DANGER)),
            (
                "plate",
                format!(
                    "rgba({}, {}, {}, {:.3})",
                    self.plate >> 16 & 255,
                    self.plate >> 8 & 255,
                    self.plate & 255,
                    self.plate_alpha
                ),
            ),
            ("blur", format!("{}px", self.blur)),
        ]
        .into_iter()
        .map(|(name, value)| format!("  --glass-{name}: {value};\n"))
        .collect()
    }
}

/// Builds a colour from a function of each channel's shift (16, 8, 0) giving 0..=1.
fn channels(f: impl Fn(u32) -> f32) -> u32 {
    [16, 8, 0].iter().fold(0, |out, &s| {
        out | ((f(s).clamp(0., 1.) * 255.).round() as u32) << s
    })
}

fn to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn from_linear(c: f32) -> f32 {
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1. / 2.4) - 0.055
    }
}

/// sRGB to OKLab (Björn Ottosson's matrices, as CSS Color 4 uses).
// The matrices as CSS Color 4 publishes them, digit for digit.
#[allow(clippy::excessive_precision)]
pub fn oklab(c: u32) -> [f32; 3] {
    let [r, g, b] = [16, 8, 0].map(|s| to_linear(((c >> s) & 255) as f32 / 255.));
    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
    [
        0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
        1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
        0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
    ]
}

/// OKLab back to sRGB, clamped to the gamut.
// The matrices as CSS Color 4 publishes them, digit for digit.
#[allow(clippy::excessive_precision)]
pub fn from_oklab([l, a, b]: [f32; 3]) -> u32 {
    let l_ = (l + 0.3963377774 * a + 0.2158037573 * b).powi(3);
    let m_ = (l - 0.1055613458 * a - 0.0638541728 * b).powi(3);
    let s_ = (l - 0.0894841775 * a - 1.2914855480 * b).powi(3);
    let rgb = [
        4.0767416621 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_,
        -1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_,
        -0.0041960863 * l_ - 0.7034186147 * m_ + 1.7076147010 * s_,
    ];
    channels(|s| from_linear(rgb[(2 - s / 8) as usize]))
}

/// CSS `color-mix(in oklab, a p, b)`: `p` of `a`, the rest `b`.
pub fn mix_oklab(a: u32, b: u32, p: f32) -> u32 {
    let (x, y) = (oklab(a), oklab(b));
    from_oklab([0, 1, 2].map(|i| x[i] * p + y[i] * (1. - p)))
}

/// WCAG relative luminance, as the mockup's `luminance` picks the label colour on the accent.
pub fn luminance(c: u32) -> f32 {
    let [r, g, b] = [16, 8, 0].map(|s| {
        let v = ((c >> s) & 255) as f32 / 255.;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    });
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: u32, b: u32) -> bool {
        [16, 8, 0]
            .iter()
            .all(|s| (((a >> s) & 255) as i32 - ((b >> s) & 255) as i32).abs() <= 1)
    }

    /// Chrome's own results for the mockup's `color-mix` sums (getComputedStyle, then a 1 px
    /// canvas for the sRGB bytes): an independent check of the OKLab maths and the plate's mix.
    #[test]
    fn mixes_match_the_browser() {
        let mixed = [0, 1, 2].map(|i| oklab(0x6b4bff)[i] * 0.52 + oklab(0xffffff)[i] * 0.48);
        for (i, want) in [0.772018, 0.0291158, -0.12693].into_iter().enumerate() {
            assert!((mixed[i] - want).abs() < 1e-3, "oklab {mixed:?}");
        }
        let soft = mix_oklab(0x6b4bff, 0xffffff, 0.52);
        assert!(close(soft, 0xa9a9ff), "{soft:06x}");
        let base = mix_oklab(0x6b4bff, 0x14111d, 0.10);
        assert!(close(base, 0x1b1830), "{base:06x}");
    }

    #[test]
    fn a_light_accent_takes_dark_labels() {
        assert_eq!(Glass::default_look().on_accent, ON_LIGHT);
        assert_eq!(Glass::new(0x6b4bff, TINT, BLUR).on_accent, 0xffffff);
    }

    #[test]
    fn states_read_and_warn_is_not_the_accent() {
        let plate = Glass::default_look().plate;
        for c in [LIVE, WARN, DANGER] {
            for bg in [plate, FLOAT, FIELD_BG, FIELD_BG_FOCUS] {
                let ratio = crate::contrast(c, bg);
                assert!(ratio >= 4.5, "#{c:06x} on #{bg:06x}: {ratio:.2}");
            }
        }
        let light_plate = crate::night::over(0xffffff, 0.84, crate::night::DAWN.sky[3]);
        for c in [LIVE_LIGHT, WARN_LIGHT, DANGER_LIGHT] {
            let ratio = crate::contrast(c, light_plate);
            assert!(ratio >= 4.5, "#{c:06x} on the light plate: {ratio:.2}");
        }
        let hue = |c: u32| {
            let [_, a, b] = oklab(c);
            b.atan2(a).to_degrees()
        };
        for warn in [WARN, WARN_LIGHT] {
            let off = (hue(warn) - hue(ACCENT)).abs();
            assert!(off >= 15., "#{warn:06x} is {off:.0}° from the accent");
        }
    }

    #[test]
    fn oklab_round_trips() {
        for c in [0x000000, 0xffffff, 0x6b4bff, 0x14111d, 0x5865f2, 0xf89d62] {
            assert!(close(from_oklab(oklab(c)), c), "{c:06x}");
        }
    }
}
