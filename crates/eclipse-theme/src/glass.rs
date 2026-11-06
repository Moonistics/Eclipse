//! The launcher's look ("Corona" in `docs/design-system.md`): quiet charcoal cards over the
//! eclipse scene, white text at a few opacities, one amber accent. Geist sets the text, Monocraft
//! the titles, navigation and buttons.

pub const ACCENT: u32 = 0xf5a524;
pub const PAGE: u32 = 0x08090f;
pub const BASE: u32 = 0x10131c;
pub const FLOAT: u32 = 0x161a25;
pub const PAPER: u32 = 0xffffff;
pub const EDGE: f32 = 0.07;
pub const EDGE_LIGHT: f32 = 0.12;
pub const FIELD_EDGE: f32 = 0.34;
pub const FIELD_EDGE_LIGHT: f32 = 0.48;
pub const SELECTED_EDGE: f32 = FIELD_EDGE;
pub const SELECTED_EDGE_LIGHT: f32 = FIELD_EDGE_LIGHT;
pub const SWITCH_EDGE_LIGHT: f32 = 0.56;
pub const ON_LIGHT: u32 = 0x14151c;
/// A running game or server, and a ready instance.
pub const LIVE: u32 = 0x4ade80;
/// Busy and partly-working states: a red-orange well off the accent's amber.
pub const WARN: u32 = 0xff7059;
/// Destructive menu rows, and the fill under one while hovered.
pub const DANGER: u32 = 0xff8a8a;
pub const LIVE_LIGHT: u32 = 0x126e34;
pub const WARN_LIGHT: u32 = 0xaf331a;
pub const DANGER_LIGHT: u32 = 0xb91c1c;
pub const SCRIM: u32 = 0x05060a;

pub const FONT: &str = crate::FONT;
pub const DISPLAY_FONT: &str = "Monocraft";

pub const MONOCRAFT_PIXEL: f32 = 9.;

pub const MONOCRAFT_ADVANCE: f32 = 6.;

pub const GEIST_ADVANCE: f32 = 7.5;

pub mod pixel_text {
    use super::MONOCRAFT_PIXEL;
    pub const PRESS: f32 = 2. * MONOCRAFT_PIXEL;
    pub const CARD_TITLE: f32 = 3. * MONOCRAFT_PIXEL;
    pub const PAGE_TITLE: f32 = 4. * MONOCRAFT_PIXEL;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStep {
    pub size: f32,
    pub line: f32,
}

pub mod step {
    use super::TextStep;
    pub const LABEL: TextStep = TextStep {
        size: 12.,
        line: 16.,
    };
    pub const CAPTION: TextStep = TextStep {
        size: 12.,
        line: 18.,
    };
    pub const NOTE: TextStep = TextStep {
        size: 13.,
        line: 20.,
    };
    pub const BODY: TextStep = TextStep {
        size: 14.,
        line: 21.,
    };
    pub const LEAD: TextStep = TextStep {
        size: 15.,
        line: 22.,
    };
    pub const TITLE: TextStep = TextStep {
        size: 16.,
        line: 22.,
    };
    pub const FIGURE: TextStep = TextStep {
        size: 22.,
        line: 28.,
    };
    pub const CODE: TextStep = TextStep {
        size: 28.,
        line: 36.,
    };
}

pub mod space {
    pub const XXS: f32 = 2.;
    pub const XS: f32 = 4.;
    pub const SM: f32 = 8.;
    pub const MD: f32 = 12.;
    pub const LG: f32 = 16.;
    pub const XL: f32 = 24.;
    pub const XXL: f32 = 32.;
}

pub mod radius {
    pub const MODAL: f32 = 14.;
    pub const PANEL: f32 = 12.;
    pub const POPOVER: f32 = 10.;
    pub const ROW: f32 = 8.;
    pub const CONTROL: f32 = 6.;
    pub const BLOCK: f32 = 4.;
}

pub mod ink {
    pub const FULL: f32 = 1.;
    pub const STRONG: f32 = 0.86;
    pub const BODY: f32 = 0.72;
    pub const MUTED: f32 = 0.6;
    pub const DIM: f32 = 0.5;
    pub const LEVELS: [f32; 5] = [FULL, STRONG, BODY, MUTED, DIM];
}

pub mod ink_light {
    pub const FULL: f32 = 1.;
    pub const STRONG: f32 = 0.9;
    pub const BODY: f32 = 0.82;
    pub const MUTED: f32 = 0.74;
    pub const DIM: f32 = 0.68;
    pub const LEVELS: [f32; 5] = [FULL, STRONG, BODY, MUTED, DIM];
}

pub fn light_ink(a: f32) -> f32 {
    ink::LEVELS
        .iter()
        .position(|&level| level == a)
        .map_or(a, |i| ink_light::LEVELS[i])
}

pub mod fill {
    pub const HOVER: f32 = 0.05;
    pub const PRIMARY_HOVER: f32 = 0.14;
    pub const FIELD: f32 = 0.07;
    pub const BUTTON: f32 = 0.08;
    pub const BUTTON_LIGHT: f32 = 0.12;
    pub const SELECTED: f32 = 0.1;
    pub const ACTIVE: f32 = 0.12;
    pub const WELL_SHADE: f32 = 0.25;
    pub const DANGER: f32 = 0.12;
}

pub const FIELD_BG: u32 = 0x1a1e29;
pub const FIELD_BG_FOCUS: u32 = 0x222735;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glass {
    pub accent: u32,
    /// The accent mixed with white (62 % accent): progress bars, the focus ring, links.
    pub accent_soft: u32,
    pub mark: u32,
    /// Text on the accent: white, or [`ON_LIGHT`] on a light accent.
    pub on_accent: u32,
    /// The charcoal with a trace of the accent: what the plates are filled with.
    pub plate: u32,
}

impl Glass {
    pub fn new(accent: u32) -> Glass {
        Glass {
            accent,
            accent_soft: mix_oklab(accent, 0xffffff, 0.62),
            mark: accent,
            on_accent: if luminance(accent) > 0.4 {
                ON_LIGHT
            } else {
                0xffffff
            },
            plate: mix_oklab(accent, BASE, 0.03),
        }
    }

    pub fn default_look() -> Glass {
        Glass::new(ACCENT)
    }

    pub fn light(self) -> Glass {
        Glass {
            accent_soft: on_light(self.accent, 4.5),
            mark: on_light(self.accent, 3.),
            ..self
        }
    }

    /// Every colour by its CSS name (`--glass-<name>`), for `tokens.css`.
    pub fn css(&self) -> String {
        let hex = |c: u32| format!("#{c:06x}");
        [
            ("accent", hex(self.accent)),
            ("accent-soft", hex(self.accent_soft)),
            ("on-accent", hex(self.on_accent)),
            ("page", hex(PAGE)),
            ("live", hex(LIVE)),
            ("warn", hex(WARN)),
            ("danger", hex(DANGER)),
            ("plate", hex(self.plate)),
        ]
        .into_iter()
        .map(|(name, value)| format!("  --glass-{name}: {value};\n"))
        .collect()
    }
}

pub fn light_surfaces() -> Vec<u32> {
    use crate::night::{over, DAWN};
    let tinted = |a, under| over(ON_LIGHT, a, under);
    let mut out = vec![
        DAWN.side,
        tinted(fill::SELECTED, DAWN.side),
        DAWN.page,
        PAPER,
        tinted(fill::SELECTED, PAPER),
        tinted(fill::FIELD, PAPER),
    ];
    out.extend(DAWN.sky);
    out
}

pub fn dark_surfaces(look: &Glass) -> Vec<u32> {
    use crate::night::over;
    let raised = |under| over(0xffffff, fill::SELECTED, under);
    vec![
        PAGE,
        look.plate,
        raised(look.plate),
        FLOAT,
        raised(FLOAT),
        FIELD_BG,
        FIELD_BG_FOCUS,
    ]
}

fn on_light(accent: u32, ratio: f32) -> u32 {
    let surfaces = light_surfaces();
    let [l, a, b] = oklab(accent);
    (0..200)
        .map(|step| from_oklab([l - step as f32 * 0.005, a, b]))
        .find(|&c| surfaces.iter().all(|&s| crate::contrast(c, s) >= ratio))
        .unwrap_or(0)
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

/// WCAG relative luminance, which picks the label colour on the accent.
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

    /// Chrome's own results for these `color-mix` sums (getComputedStyle, then a 1 px
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
        assert_eq!(Glass::new(0x6b4bff).on_accent, 0xffffff);
    }

    #[test]
    fn states_read_and_warn_is_not_the_accent() {
        let look = Glass::default_look();
        for c in [LIVE, WARN, DANGER] {
            for bg in dark_surfaces(&look) {
                let ratio = crate::contrast(c, bg);
                assert!(ratio >= 4.5, "#{c:06x} on #{bg:06x}: {ratio:.2}");
            }
        }
        use crate::night::{over, DAWN};
        let plates = light_surfaces()
            .into_iter()
            .filter(|bg| !DAWN.sky.contains(bg));
        for c in [LIVE_LIGHT, WARN_LIGHT, DANGER_LIGHT] {
            let hovered = [fill::HOVER, fill::SELECTED, fill::ACTIVE]
                .map(|a| over(ON_LIGHT, a, PAPER))
                .into_iter()
                .chain([over(c, fill::DANGER, PAPER)]);
            for bg in plates.clone().chain(hovered) {
                let ratio = crate::contrast(c, bg);
                assert!(ratio >= 4.5, "#{c:06x} on #{bg:06x}: {ratio:.2}");
            }
        }
        let oklab_hue = |c: u32| {
            let [_, a, b] = oklab(c);
            b.atan2(a).to_degrees()
        };
        let hsl_hue = |c: u32| {
            let [r, g, b] = [16, 8, 0].map(|s| ((c >> s) & 255) as f32);
            let (max, min) = (r.max(g).max(b), r.min(g).min(b));
            let h = if max == r {
                (g - b) / (max - min)
            } else if max == g {
                2. + (b - r) / (max - min)
            } else {
                4. + (r - g) / (max - min)
            };
            (h * 60.).rem_euclid(360.)
        };
        for warn in [WARN, WARN_LIGHT] {
            for hue in [oklab_hue, hsl_hue] {
                let off = (hue(warn) - hue(ACCENT)).abs();
                assert!(off >= 25., "#{warn:06x} is {off:.0}° from the accent");
            }
        }
    }

    #[test]
    fn every_ink_level_reads_on_every_surface() {
        use crate::night::over;
        for level in ink::LEVELS {
            let a = light_ink(level);
            for bg in light_surfaces() {
                let ratio = crate::contrast(over(ON_LIGHT, a, bg), bg);
                assert!(ratio >= 4.5, "light ink {a} on #{bg:06x}: {ratio:.2}");
            }
            for bg in dark_surfaces(&Glass::default_look()) {
                let ratio = crate::contrast(over(0xffffff, level, bg), bg);
                assert!(ratio >= 4.5, "ink {level} on #{bg:06x}: {ratio:.2}");
            }
        }
        assert_eq!(
            light_ink(fill::SELECTED),
            fill::SELECTED,
            "fills keep their weight"
        );
    }

    #[test]
    fn a_field_edge_stands_out_from_its_card() {
        use crate::night::over;
        let look = Glass::default_look();
        for card in [look.plate, FLOAT] {
            let ratio = crate::contrast(over(0xffffff, FIELD_EDGE, card), card);
            assert!(ratio >= 3., "field edge on #{card:06x}: {ratio:.2}");
        }
        let ratio = crate::contrast(over(ON_LIGHT, FIELD_EDGE_LIGHT, PAPER), PAPER);
        assert!(ratio >= 3., "light field edge: {ratio:.2}");
    }

    #[test]
    fn a_chosen_card_shows_its_edge() {
        use crate::night::over;
        let look = Glass::default_look();
        for card in [look.plate, FLOAT] {
            let plate = over(0xffffff, fill::SELECTED, card);
            let ratio = crate::contrast(over(0xffffff, SELECTED_EDGE, plate), card);
            assert!(ratio >= 3., "chosen edge on #{card:06x}: {ratio:.2}");
        }
        let plate = over(ON_LIGHT, fill::SELECTED, PAPER);
        let ratio = crate::contrast(over(ON_LIGHT, SELECTED_EDGE_LIGHT, plate), PAPER);
        assert!(ratio >= 3., "light chosen edge: {ratio:.2}");
    }

    #[test]
    fn the_light_look_marks_read_on_every_light_surface() {
        let typed = (0..216).map(|i: u32| {
            [i / 36, i / 6 % 6, i % 6]
                .iter()
                .fold(0, |c, n| c << 8 | n * 0x33)
        });
        for accent in [ACCENT].into_iter().chain(typed) {
            let look = Glass::new(accent).light();
            for bg in light_surfaces() {
                let icon = crate::contrast(look.mark, bg);
                assert!(icon >= 3., "#{accent:06x}'s mark on #{bg:06x}: {icon:.2}");
                let link = crate::contrast(look.accent_soft, bg);
                assert!(link >= 4.5, "#{accent:06x}'s links on #{bg:06x}: {link:.2}");
            }
            assert_eq!(look.accent, accent, "the fill stays the accent");
        }
    }

    #[test]
    fn the_dark_look_keeps_its_colours() {
        let look = Glass::default_look();
        assert_eq!(look.mark, ACCENT);
        assert!(
            close(look.accent_soft, 0xfbc989),
            "{:06x}",
            look.accent_soft
        );
        let light = look.light();
        assert_ne!(light.mark, ACCENT);
        assert_eq!((light.plate, light.on_accent), (look.plate, look.on_accent));
    }

    #[test]
    fn oklab_round_trips() {
        for c in [0x000000, 0xffffff, 0x6b4bff, 0x14111d, 0x5865f2, 0xf89d62] {
            assert!(close(from_oklab(oklab(c)), c), "{c:06x}");
        }
    }
}
