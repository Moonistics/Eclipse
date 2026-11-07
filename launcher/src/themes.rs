//! The player's own look, on top of Dark, Light or System: an accent colour and a picture behind
//! Home.
//!
//! An accent is a hue, and the player may pick any of them; seven hues have names, as shortcuts.
//! Amber is the default.
use std::path::{Path, PathBuf};

/// An accent: a hue in degrees, or none for the default amber; and for a colour typed as hex,
/// that colour, worn as is.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Accent {
    hue: Option<f32>,
    rgb: Option<u32>,
}

/// A launcher theme: free for everyone in Settings › Appearance, never sold or claimed. Each
/// has a sky (six stops, top to bottom, behind the light appearance).
pub struct Theme {
    /// Its name in `Settings::theme`.
    pub id: &'static str,
    pub old_ids: &'static [&'static str],
    /// Its name in Settings, in English (through `tr`).
    pub name: &'static str,
    pub sky: [u32; 6],
}

/// Daybreak: the sky just before sunrise.
pub const DAYBREAK: Theme = Theme {
    id: "daybreak",
    old_ids: &["first_light"],
    name: "Daybreak",
    sky: [0x0b1030, 0x1a2660, 0x2a3778, 0x7f78c0, 0xe2a0b0, 0xffd6a0],
};

/// The launcher themes past the night sky.
pub const THEMES: [Theme; 1] = [DAYBREAK];

pub const DEFAULT_SKY_NAME: &str = "Night";

/// The theme named `id` (`Settings::theme`); None for the night sky or a name this launcher
/// doesn't know.
pub fn theme(id: Option<&str>) -> Option<&'static Theme> {
    let id = id?;
    THEMES
        .iter()
        .find(|t| t.id == id || t.old_ids.contains(&id))
}

pub(crate) fn stored_theme<'de, D: serde::Deserializer<'de>>(
    stored: D,
) -> Result<Option<String>, D::Error> {
    let id: Option<String> = serde::Deserialize::deserialize(stored)?;
    Ok(id.map(|id| theme(Some(&id)).map_or(id, |t| t.id.to_owned())))
}

const NAMED: [(Accent, &str, &str, u32); 7] = [
    (
        Accent::AMBER,
        "amber",
        "Amber",
        eclipse_theme::glass::ACCENT,
    ),
    (Accent::BLUE, "blue", "Blue", 0x2563eb),
    (Accent::TEAL, "teal", "Teal", 0x0f766e),
    (Accent::GREEN, "green", "Green", 0x15803d),
    (Accent::VIOLET, "violet", "Violet", 0x6b4bff),
    (Accent::RED, "red", "Red", 0xdc2626),
    (Accent::PINK, "pink", "Pink", 0xbe185d),
];

/// The look's colour for each of [`Accent::ALL`]: the swatches Settings offers.
pub const SWATCHES: [u32; 7] = [
    NAMED[0].3, NAMED[1].3, NAMED[2].3, NAMED[3].3, NAMED[4].3, NAMED[5].3, NAMED[6].3,
];

impl Accent {
    pub const AMBER: Accent = Accent {
        hue: None,
        rgb: None,
    };
    const BLUE: Accent = Accent {
        hue: Some(212.),
        rgb: None,
    };
    pub(crate) const TEAL: Accent = Accent {
        hue: Some(176.),
        rgb: None,
    };
    const GREEN: Accent = Accent {
        hue: Some(140.),
        rgb: None,
    };
    const VIOLET: Accent = Accent {
        hue: Some(252.),
        rgb: None,
    };
    const RED: Accent = Accent {
        hue: Some(356.),
        rgb: None,
    };
    const PINK: Accent = Accent {
        hue: Some(322.),
        rgb: None,
    };

    pub const ALL: [Accent; 7] = [
        NAMED[0].0, NAMED[1].0, NAMED[2].0, NAMED[3].0, NAMED[4].0, NAMED[5].0, NAMED[6].0,
    ];

    fn named(self) -> Option<&'static (Accent, &'static str, &'static str, u32)> {
        NAMED.iter().find(|n| n.0 == self)
    }

    pub fn label(self) -> Option<&'static str> {
        self.named().map(|n| n.2)
    }

    /// Any hue the player picks.
    pub fn custom(hue: f32) -> Self {
        Self {
            hue: Some(hue.rem_euclid(360.)),
            rgb: None,
        }
    }

    /// A colour (`0xrrggbb`): one of the swatches is its named accent.
    fn from_rgb(rgb: u32) -> Self {
        let rgb = rgb & 0xff_ffff;
        match NAMED.iter().find(|n| n.3 == rgb) {
            Some(n) => n.0,
            None => Self {
                hue: Some(hsl(rgb).0),
                rgb: Some(rgb),
            },
        }
    }

    /// The colour the look wears: the one typed, a named accent's swatch, else the hue at
    /// amber's saturation and lightness.
    pub fn rgb(self) -> u32 {
        if let Some(rgb) = self.rgb {
            return rgb;
        }
        match (self.named(), self.hue) {
            (Some(n), _) => n.3,
            (None, Some(hue)) => {
                let (_, s, l) = hsl(NAMED[0].3);
                from_hsl(hue, s, l)
            }
            (None, None) => NAMED[0].3,
        }
    }

    /// [`rgb`](Self::rgb) as `#rrggbb`.
    pub fn hex(self) -> String {
        format!("#{:06x}", self.rgb())
    }

    /// Its name in `settings.json` and on the command line: a named hue's, a typed colour's
    /// `#rrggbb`, else its degrees.
    pub fn name(self) -> String {
        if self.rgb.is_some() {
            return self.hex();
        }
        let named = self.named().map(|n| n.1.to_owned());
        named.unwrap_or_else(|| match self.hue {
            None => "amber".to_owned(),
            Some(hue) => format!("{hue}"),
        })
    }

    /// A name, `#rrggbb`, or a hue in degrees.
    pub fn parse(name: &str) -> Option<Self> {
        let name = name.trim().to_ascii_lowercase();
        if let Some(hex) = name.strip_prefix('#') {
            return (hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
                .then(|| u32::from_str_radix(hex, 16).ok())
                .flatten()
                .map(Self::from_rgb);
        }
        let name = match name.as_str() {
            "purple" => "violet",
            "default" => "amber",
            _ => &name,
        };
        if let Some(n) = NAMED.iter().find(|n| n.1 == name) {
            return Some(n.0);
        }
        name.parse::<f32>()
            .ok()
            .filter(|hue| hue.is_finite())
            .map(Self::custom)
    }

    /// Its hue in degrees; None for the default amber.
    pub fn hue(self) -> Option<f32> {
        self.hue
    }

    pub fn shown_hue(self) -> f32 {
        self.hue.unwrap_or_else(|| hsl(self.rgb()).0)
    }
}

impl TryFrom<String> for Accent {
    type Error = String;

    fn try_from(name: String) -> Result<Self, Self::Error> {
        Self::parse(&name).ok_or_else(|| format!("{name}: a colour name or a hue in degrees"))
    }
}

impl From<Accent> for String {
    fn from(accent: Accent) -> Self {
        accent.name()
    }
}

/// The colour the hue picker shows for `hue`: a strip of them, at one saturation and lightness.
pub fn hue_colour(hue: f32) -> u32 {
    from_hsl(hue.rem_euclid(360.), 0.62, 0.55)
}

/// The colour's hue (degrees), saturation and lightness (0 to 1).
fn hsl(c: u32) -> (f32, f32, f32) {
    let [r, g, b] = [16, 8, 0].map(|s| ((c >> s) & 255) as f32 / 255.);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let l = (max + min) / 2.;
    if max == min {
        return (0., 0., l);
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2. - max - min)
    } else {
        d / (max + min)
    };
    let h = if max == r {
        (g - b) / d + if g < b { 6. } else { 0. }
    } else if max == g {
        (b - r) / d + 2.
    } else {
        (r - g) / d + 4.
    };
    (h * 60., s, l)
}

fn from_hsl(h: f32, s: f32, l: f32) -> u32 {
    let c = (1. - (2. * l - 1.).abs()) * s;
    let x = c * (1. - ((h / 60.) % 2. - 1.).abs());
    let m = l - c / 2.;
    let (r, g, b) = match (h.rem_euclid(360.) / 60.) as u32 {
        0 => (c, x, 0.),
        1 => (x, c, 0.),
        2 => (0., c, x),
        3 => (0., x, c),
        4 => (x, 0., c),
        _ => (c, 0., x),
    };
    let byte = |v: f32| ((v + m).clamp(0., 1.) * 255.).round() as u32;
    byte(r) << 16 | byte(g) << 8 | byte(b)
}

/// The pictures Home can show: what the decoders read.
const PICTURES: [&str; 4] = ["png", "jpg", "jpeg", "webp"];
/// A picture above this is refused: a screen's worth is far less.
const MAX_BYTES: u64 = 40 << 20;

/// Keeps `file` as Home's background in `dir` (`background-<hash>.<ext>`: a new picture gets a
/// new name, so the window doesn't show the one it cached), after checking it is a picture, and
/// returns the kept file's name. An older background goes.
pub fn keep_background(dir: &Path, file: &Path) -> Result<String, String> {
    let shown = file.display();
    let ext = file
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .filter(|e| PICTURES.contains(&e.as_str()))
        .ok_or_else(|| format!("{shown}: a PNG, JPEG or WebP picture, please."))?;
    let size = std::fs::metadata(file)
        .map_err(|e| format!("{shown}: {e}"))?
        .len();
    if size > MAX_BYTES {
        return Err(format!("{shown} is over 40 MB."));
    }
    let bytes = std::fs::read(file).map_err(|e| format!("{shown}: {e}"))?;
    let (w, h) = image::ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()
        .ok()
        .and_then(|r| r.into_dimensions().ok())
        .ok_or_else(|| format!("{shown} isn't a picture the launcher can read."))?;
    if w < 64 || h < 64 {
        return Err(format!("{shown} is {w}×{h}: too small to fill Home."));
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let hash = crate::sources::sha1_hex(&bytes);
    let name = format!("background-{}.{ext}", &hash[..12]);
    std::fs::write(dir.join(&name), &bytes).map_err(|e| format!("{name}: {e}"))?;
    for old in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let other = old.file_name().to_string_lossy().into_owned();
        if other.starts_with("background-") && other != name {
            let _ = std::fs::remove_file(old.path());
        }
    }
    Ok(name)
}

/// Home's background, if the player keeps one and it is still there.
pub fn background(dir: &Path, name: Option<&str>) -> Option<PathBuf> {
    let name = name.filter(|n| n.starts_with("background-") && !n.contains(['/', '\\']))?;
    Some(dir.join(name)).filter(|p| p.is_file())
}

#[cfg(test)]
#[path = "themes/tests.rs"]
mod tests;
