use crate::art;
use crate::motion::{self, Clock, Motion};
use eclipse_theme::night::{over, Night, DAWN, NIGHT};
use gpui::{
    canvas, div, fill, img, point, prelude::*, px, size, Bounds, Context, Entity, Pixels,
    RenderImage, Window,
};
use std::f32::consts::{FRAC_PI_4, TAU};
use std::sync::Arc;
use std::time::{Duration, Instant};

type Rgb = [f32; 3];

const SKY_AT: [f32; 6] = [0., 0.4, 0.66, 0.82, 0.92, 1.];
const SKY_BAND: usize = 3;
const HORIZON: f32 = 0.8;
const THEME_SKY_WASH: f32 = 0.75;

const SUN_SIZE: f32 = 0.055;
const SUN_HIGH: f32 = 0.38;
const SUN_REACH: f32 = 2.4;
const GUTTER_MARGIN: f32 = 48.;
const SMALLEST_HIGH_SUN: f32 = 0.6;
const SETTING_RADIUS: f32 = 12.;
const HOME_BOTTOM: f32 = 516.;
const SHORT_HOME_BOTTOM: f32 = 484.;
const HILLS_HIGHEST: f32 = 0.62;
const SETTING_ACROSS: (f32, f32) = (0.74, 0.92);
const HILLS_BELOW_CENTRE: f32 = 0.1;
const DIP_REACH: f32 = 4.5;
const DIP_CALM: f32 = 0.5;
const DIP_CALM_REACH: f32 = 2.;

const CORONA_ALPHAS: [f32; 4] = [0.14, 0.09, 0.06, 0.035];
const CORONA_RING: f32 = 0.26;
const STREAMERS: usize = 12;
const STREAMER_ALPHA: f32 = 0.45;
const SHORT_STREAMER: f32 = 0.55;
const STREAMER_TWIST: f32 = 0.3;
const STREAMER_LENGTH: (f32, f32) = (0.9, 1.1);
const BEAD_ANGLE: f32 = FRAC_PI_4 * 5.;
const BEAD_SPARKLE: f32 = 0.35;
const BEAD_SPARKLE_FROM: f32 = 10.;

const STAR_EVERY: f32 = 700.;
const STAR_HIGH_BIAS: f32 = 1.7;
const STAR_REACH: f32 = 0.72;
const SUN_CLEARANCE: f32 = 2.6;
const BRIGHT_STAR: f32 = 0.9;
const MID_STAR: f32 = 0.55;
const TWINKLING: f32 = 0.18;
const TWINKLE_SPEED: (f32, f32) = (0.6, 2.);
const TWINKLE_LOW: f32 = 0.35;
const STAR_DOT: f32 = 0.5;
const SMALLEST_STAR_DOT: f32 = 1.5;

const CLOUD_EVERY: f32 = 24.;
const CLOUD_BAND: (f32, f32) = (0.45, 0.85);
const CLOUD_LENGTH: (f32, f32) = (14., 34.);
const CLOUD_ALPHA: f32 = 0.7;
const CLOUD_CLEARANCE: f32 = 4.;
const CLOUD_ROWS: [(f32, f32); 4] = [(0.3, 0.35), (0.1, 0.75), (0., 1.), (0.15, 0.65)];

const SNOW_LINE: f32 = 0.34;
const SNOW_SHARE: f32 = 0.6;
const SNOW_WAVE: f32 = 5.;
const SNOW_RAGGED: f32 = 8.;
const SNOW_SEED: u32 = 17;
const LIT_FACE: i32 = 2;

const RIDGED: f32 = 0.62;
const PEAK_DETAIL: [(f32, f32); 2] = [(2.6, 0.26), (7., 0.12)];
const ROLLING_OCTAVES: [(f32, f32); 2] = [(1., 0.7), (3., 0.3)];

const GROUND_FOOT: f32 = 0.975;
const GROUND_RISE: f32 = 0.08;
const GROUND_SPAN: f32 = 90.;
const GROUND_BLOCK: i32 = 4;
const GRASS_ROWS: i32 = 2;
const DIRT_ROWS: i32 = 4;
const LIT_GRASS: f32 = 0.18;
const BURIED_STONE: f32 = 0.05;
const TREE_EVERY: f32 = 22.;
const LIT_LEAVES: f32 = 0.12;
const TORCHES: [f32; 2] = [0.34, 0.63];
const TORCH_GLOW: f32 = 0.1;

struct Ridge {
    seed: u32,
    from: f32,
    depth: f32,
    span: f32,
}

const FAR_RIDGE: Ridge = Ridge {
    seed: 3,
    from: 0.,
    depth: 0.75,
    span: 70.,
};
const MID_RIDGE: Ridge = Ridge {
    seed: 8,
    from: 0.35,
    depth: 0.55,
    span: 46.,
};

struct Shape {
    body: Rgb,
    edge: Rgb,
}

struct Palette {
    page: u32,
    stars: Option<[Rgb; 3]>,
    clouds: Option<Shape>,
    corona: Rgb,
    streamer: Rgb,
    rim: Rgb,
    bead: Rgb,
    moon: Rgb,
    far: Shape,
    mid: Shape,
    grass: Rgb,
    grass_lit: Rgb,
    dirt: Rgb,
    dirt_deep: Rgb,
    stone: Rgb,
    trunk: Rgb,
    leaves: Rgb,
    leaves_shade: Rgb,
    leaves_lit: Rgb,
    stick: Rgb,
    flame: Rgb,
    glow: Rgb,
}

const AT_NIGHT: Palette = Palette {
    page: eclipse_theme::glass::PAGE,
    stars: Some([[176., 182., 222.], [104., 110., 160.], [62., 67., 108.]]),
    clouds: None,
    corona: [255., 176., 72.],
    streamer: [255., 196., 110.],
    rim: [255., 226., 170.],
    bead: [255., 248., 230.],
    moon: [7., 8., 13.],
    far: Shape {
        body: [26., 28., 60.],
        edge: [48., 50., 88.],
    },
    mid: Shape {
        body: [16., 18., 40.],
        edge: [22., 24., 50.],
    },
    grass: [22., 40., 30.],
    grass_lit: [30., 54., 38.],
    dirt: [24., 19., 16.],
    dirt_deep: [17., 14., 12.],
    stone: [28., 28., 32.],
    trunk: [34., 24., 18.],
    leaves: [14., 30., 22.],
    leaves_shade: [10., 22., 16.],
    leaves_lit: [22., 44., 30.],
    stick: [60., 44., 30.],
    flame: [255., 200., 90.],
    glow: [255., 179., 71.],
};

const BY_DAY: Palette = Palette {
    page: DAWN.page,
    stars: None,
    clouds: Some(Shape {
        body: [255., 255., 255.],
        edge: [226., 222., 220.],
    }),
    corona: [248., 170., 60.],
    streamer: [242., 158., 52.],
    rim: [245., 165., 36.],
    bead: [255., 250., 236.],
    moon: [40., 44., 56.],
    far: Shape {
        body: [185., 193., 208.],
        edge: [210., 216., 226.],
    },
    mid: Shape {
        body: [154., 163., 181.],
        edge: [170., 178., 195.],
    },
    grass: [116., 140., 112.],
    grass_lit: [136., 160., 126.],
    dirt: [130., 112., 94.],
    dirt_deep: [112., 97., 84.],
    stone: [140., 140., 146.],
    trunk: [104., 86., 72.],
    leaves: [94., 118., 96.],
    leaves_shade: [80., 102., 84.],
    leaves_lit: [114., 138., 110.],
    stick: [120., 94., 68.],
    flame: [255., 200., 90.],
    glow: [255., 179., 71.],
};

const TICK: Duration = Duration::from_millis(250);
const UNFOCUSED: Duration = Duration::from_secs(10);

fn texel(width: f64, scale: f32) -> f32 {
    let base = if width < 1100. {
        3.
    } else if width < 1800. {
        4.
    } else {
        5.
    };
    (base * scale).round() / scale
}

#[derive(Clone, Copy, PartialEq)]
struct Look {
    sky: [u32; 6],
    dark: bool,
}

impl Look {
    fn of(night: &Night) -> Look {
        let sky = if night.dark || night.sky == DAWN.sky {
            night.sky
        } else {
            night.sky.map(|c| over(0xffffff, THEME_SKY_WASH, c))
        };
        Look {
            sky,
            dark: night.dark,
        }
    }

    fn palette(self) -> &'static Palette {
        if self.dark {
            &AT_NIGHT
        } else {
            &BY_DAY
        }
    }
}

struct Rng(u32);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x6d2b79f5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(1 | t);
        t = (t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t))) ^ t;
        (t ^ (t >> 14)) as f32 / 4294967296.
    }

    fn between(&mut self, (low, high): (f32, f32)) -> f32 {
        low + self.next() * (high - low)
    }
}

fn hash(seed: u32, n: i32) -> f32 {
    Rng(seed
        .wrapping_mul(7919)
        .wrapping_add((n as u32).wrapping_mul(104729)))
    .next()
}

fn smooth(seed: u32, x: f32) -> f32 {
    let left = x.floor();
    let t = x - left;
    let eased = t * t * (3. - 2. * t);
    hash(seed, left as i32) * (1. - eased) + hash(seed, left as i32 + 1) * eased
}

fn peaks(seed: u32, x: f32, span: f32) -> f32 {
    let ridged = 1. - (smooth(seed, x / span) * 2. - 1.).abs();
    PEAK_DETAIL
        .iter()
        .zip(1..)
        .fold(ridged * RIDGED, |sum, (&(split, weight), n)| {
            sum + smooth(seed + n, x / (span / split)) * weight
        })
}

fn rolling(seed: u32, x: f32, span: f32) -> f32 {
    ROLLING_OCTAVES
        .iter()
        .zip(0..)
        .map(|(&(split, weight), n)| smooth(seed + n, x / (span / split)) * weight)
        .sum()
}

fn even(y: f32) -> i32 {
    ((y / 2.).round() * 2.) as i32
}

impl Ridge {
    fn top(&self, x: i32, skyline: f32, base: f32) -> i32 {
        let fall = self.from + (1. - peaks(self.seed, x as f32, self.span)) * self.depth;
        even(skyline + (base - skyline) * fall)
    }
}

#[derive(Clone, Copy)]
struct Stage {
    texel: f32,
    sidebar: i32,
    column: (i32, i32),
    skyline: f32,
}

fn home_bottom(height: f32) -> f32 {
    if height < crate::layout::SHORT_WINDOW {
        SHORT_HOME_BOTTOM
    } else {
        HOME_BOTTOM
    }
}

fn stage(width: f32, height: f32, texel: f32) -> Stage {
    use crate::layout::{content_width, SIDEBAR};
    let column = content_width(width);
    let left = SIDEBAR + (width - SIDEBAR - column) / 2.;
    let skyline = (home_bottom(height) / texel).max(height / texel * HILLS_HIGHEST);
    Stage {
        texel,
        sidebar: (SIDEBAR / texel) as i32,
        column: (
            (left / texel) as i32,
            ((left + column) / texel).ceil() as i32,
        ),
        skyline: (skyline / 2.).ceil() * 2.,
    }
}

#[derive(Clone, Copy)]
struct Dip {
    x: i32,
    line: f32,
    radius: f32,
}

impl Dip {
    fn mean(&self, top: impl Fn(i32) -> i32) -> f32 {
        let span = self.radius as i32;
        let tops = (self.x - span..=self.x + span).map(top);
        tops.sum::<i32>() as f32 / (2 * span + 1) as f32
    }

    fn off(&self, top: impl Fn(i32) -> i32) -> f32 {
        let span = self.radius as i32;
        (self.x - span..=self.x + span)
            .map(|x| (self.line - top(x) as f32).abs())
            .sum()
    }

    fn over(&self, top: impl Fn(i32) -> i32) -> f32 {
        let span = self.radius as i32;
        (self.x - span..=self.x + span)
            .map(|x| (self.line - top(x) as f32).max(0.))
            .sum()
    }

    fn lower(&self, top: i32, x: i32, depth: f32, calm: f32) -> i32 {
        let near = |reach: f32| {
            let t = (((x - self.x).abs() as f32 - self.radius) / (self.radius * (reach - 1.)))
                .clamp(0., 1.);
            1. - t * t * (3. - 2. * t)
        };
        let shifted = top as f32 + depth * near(DIP_REACH);
        even(self.line + (shifted - self.line) * (1. - calm * near(DIP_CALM_REACH)))
    }
}

struct Canvas {
    w: usize,
    h: usize,
    px: Vec<Rgb>,
}

impl Canvas {
    fn set(&mut self, x: i32, y: i32, c: Rgb) {
        self.blend(x, y, c, 1.);
    }

    fn blend(&mut self, x: i32, y: i32, c: Rgb, a: f32) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return;
        }
        let p = &mut self.px[y as usize * self.w + x as usize];
        for i in 0..3 {
            p[i] += (c[i] - p[i]) * a;
        }
    }

    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgb) {
        self.veil(x, y, w, h, c, 1.);
    }

    fn veil(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgb, a: f32) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.blend(xx, yy, c, a);
            }
        }
    }

    fn disc(&mut self, cx: i32, cy: i32, r: f32, c: Rgb, a: f32) {
        let reach = r.ceil() as i32;
        for dy in -reach..=reach {
            let half = (r * r - (dy * dy) as f32).max(0.).sqrt().floor() as i32;
            for dx in -half..half {
                self.blend(cx + dx, cy + dy, c, a);
            }
        }
    }
}

fn rgb(c: u32) -> Rgb {
    [16, 8, 0].map(|shift| ((c >> shift) & 255) as f32)
}

fn sky(stops: &[u32; 6], position: f32) -> Rgb {
    let next = SKY_AT
        .iter()
        .position(|&at| at >= position)
        .unwrap_or(SKY_AT.len() - 1);
    if next == 0 {
        return rgb(stops[0]);
    }
    let (from, to) = (rgb(stops[next - 1]), rgb(stops[next]));
    let t = (position - SKY_AT[next - 1]) / (SKY_AT[next] - SKY_AT[next - 1]);
    [0, 1, 2].map(|i| from[i] + (to[i] - from[i]) * t)
}

struct Star {
    x: f32,
    y: f32,
    colour: Rgb,
    speed: f32,
    phase: f32,
}

struct Picture {
    cols: usize,
    rows: usize,
    texels: Vec<[u8; 4]>,
    stars: Vec<Star>,
}

#[derive(Clone, Copy, PartialEq)]
enum Sun {
    Beside { x: f32, scale: f32 },
    Setting,
    Set,
}

fn sun_at(width: f32, height: f32, stage: Stage, content_bottom: f32) -> Sun {
    let avail = width - crate::layout::SIDEBAR;
    let column = crate::layout::content_width(width);
    let gutter = (avail - column) / 2.;
    let reach = width.min(height) * SUN_SIZE * SUN_REACH;
    let fit = ((gutter - GUTTER_MARGIN) / (2. * reach)).min(1.);
    if fit >= SMALLEST_HIGH_SUN {
        Sun::Beside {
            x: (width - gutter / 2.) / width,
            scale: fit,
        }
    } else if content_bottom > stage.skyline * stage.texel {
        Sun::Set
    } else {
        Sun::Setting
    }
}

fn corona_ring(r: f32) -> f32 {
    (r * CORONA_RING).ceil()
}

fn paint(cols: usize, rows: usize, stage: Stage, sun: Sun, look: Look) -> Picture {
    let palette = look.palette();
    let mut c = Canvas {
        w: cols,
        h: rows,
        px: vec![[0.; 3]; cols * rows],
    };
    let (w, h) = (cols as f32, rows as f32);

    let hills = |x: i32| -> i32 {
        let block = x.div_euclid(GROUND_BLOCK) * GROUND_BLOCK;
        even(h * GROUND_FOOT - rolling(91, block as f32, GROUND_SPAN) * h * GROUND_RISE)
    };
    let base = h * (GROUND_FOOT - GROUND_RISE / 2.);
    let far = |x: i32| FAR_RIDGE.top(x, stage.skyline, base);
    let mid = |x: i32| MID_RIDGE.top(x, stage.skyline, base);

    let (disc, dip) = match sun {
        Sun::Beside { x, scale } => (
            Some((
                (w.min(h) * SUN_SIZE * scale).round().max(4.),
                (w * x).round() as i32,
                (h * SUN_HIGH).round() as i32,
            )),
            None,
        ),
        Sun::Setting => {
            let r = SETTING_RADIUS;
            let reach = r + CORONA_ALPHAS.len() as f32 * corona_ring(r);
            let highest = stage.skyline + 1. + r + HILLS_BELOW_CENTRE * r;
            let from = (w * SETTING_ACROSS.0) as i32;
            let to = ((w * SETTING_ACROSS.1).min(w - reach) as i32).max(from);
            let at = |x: i32| {
                let mut tops: Vec<i32> = (x - r as i32..=x + r as i32).map(far).collect();
                tops.sort_unstable();
                Dip {
                    x,
                    line: (tops[tops.len() / 2] as f32).max(highest),
                    radius: r,
                }
            };
            let effort = |x: i32| {
                let dip = at(x);
                dip.off(far) + dip.over(mid) + dip.over(hills)
            };
            let x = (from..=to)
                .step_by(2)
                .min_by(|a, b| effort(*a).total_cmp(&effort(*b)))
                .unwrap_or(from);
            let dip = at(x);
            let cy = (dip.line - HILLS_BELOW_CENTRE * r).round();
            (Some((r, x, cy as i32)), Some(dip))
        }
        Sun::Set => (None, None),
    };

    let horizon = match disc {
        Some((r, _, sy)) if dip.is_some() => (h * HORIZON).max(sy as f32 + r),
        _ => h * HORIZON,
    }
    .round();
    for y in (0..rows).step_by(SKY_BAND) {
        let colour = sky(&look.sky, (y as f32 / horizon).min(1.));
        c.rect(0, y as i32, cols as i32, SKY_BAND as i32, colour);
    }

    let mut stars = Vec::new();
    if let Some([bright, mid, dim]) = palette.stars {
        let mut rng = Rng(11);
        for _ in 0..((w * horizon) / STAR_EVERY).round() as usize {
            let x = rng.next() * w;
            let y = rng.next().powf(STAR_HIGH_BIAS) * horizon * STAR_REACH;
            let shine = rng.next();
            let twinkles = rng.next() < TWINKLING;
            let speed = rng.between(TWINKLE_SPEED);
            let phase = rng.next() * TAU;
            let near = disc.is_some_and(|(r, sx, sy)| {
                ((x - sx as f32).powi(2) + (y - sy as f32).powi(2)).sqrt() < r * SUN_CLEARANCE
            });
            if near {
                continue;
            }
            let colour = if shine > BRIGHT_STAR {
                bright
            } else if shine > MID_STAR {
                mid
            } else {
                dim
            };
            stars.push(Star {
                x,
                y,
                colour,
                speed: if twinkles && shine > MID_STAR {
                    speed
                } else {
                    0.
                },
                phase,
            });
        }
    }

    if let Some(cloud) = &palette.clouds {
        let mut drift = Rng(29);
        for _ in 0..(w / CLOUD_EVERY).round() as usize {
            let x = drift.next() * w;
            let y = drift.between(CLOUD_BAND) * horizon;
            let len = drift.between(CLOUD_LENGTH);
            let near_sun = disc.is_some_and(|(r, sx, sy)| {
                (x + len / 2. - sx as f32).abs() < len / 2. + r * SUN_CLEARANCE
                    && (y - sy as f32).abs() < r * SUN_CLEARANCE
            });
            let (left, right) = stage.column;
            let in_gutter = x >= stage.sidebar as f32 && x + len < left as f32 - CLOUD_CLEARANCE;
            if near_sun || !(in_gutter || x > right as f32 + CLOUD_CLEARANCE) {
                continue;
            }
            let (x, y) = (x as i32, y as i32);
            let part = |share: f32| (len * share).round() as i32;
            for (row, &(from, share)) in CLOUD_ROWS.iter().enumerate() {
                let underside = row == CLOUD_ROWS.len() - 1;
                let colour = if underside { cloud.edge } else { cloud.body };
                c.veil(
                    x + part(from),
                    y + row as i32,
                    part(share),
                    1,
                    colour,
                    CLOUD_ALPHA,
                );
            }
        }
    }

    if let Some((r, sx, sy)) = disc {
        let ring = corona_ring(r);
        for (i, alpha) in CORONA_ALPHAS.iter().enumerate().rev() {
            c.disc(sx, sy, r + (i + 1) as f32 * ring, palette.corona, *alpha);
        }
        let mut rays = Rng(37);
        for i in 0..STREAMERS {
            let angle = (i as f32 + rays.next() * STREAMER_TWIST) * TAU / STREAMERS as f32;
            let length = if i % 2 == 0 { 1. } else { SHORT_STREAMER };
            let len = r * length * rays.between(STREAMER_LENGTH);
            let (dx, dy) = (angle.cos(), angle.sin());
            let mut d = r;
            while d < r + len {
                let fade = 1. - (d - r) / len;
                c.blend(
                    sx + (dx * d).round() as i32,
                    sy + (dy * d).round() as i32,
                    palette.streamer,
                    STREAMER_ALPHA * fade,
                );
                d += 0.5;
            }
        }
        c.disc(sx, sy, r + 1., palette.rim, 1.);
        c.disc(sx, sy, r, palette.moon, 1.);
        let (bx, by) = (
            sx + (BEAD_ANGLE.cos() * r).round() as i32,
            sy + (BEAD_ANGLE.sin() * r).round() as i32,
        );
        c.rect(bx - 1, by - 1, 2, 2, palette.bead);
        if r >= BEAD_SPARKLE_FROM {
            for (dx, dy) in [
                (-3, 0),
                (2, 0),
                (0, -3),
                (0, 2),
                (-2, 0),
                (1, 0),
                (0, -2),
                (0, 1),
            ] {
                c.blend(
                    bx + dx - (dx < 0) as i32,
                    by + dy - (dy < 0) as i32,
                    palette.bead,
                    BEAD_SPARKLE,
                );
            }
        }
    }

    let snowline = stage.skyline + (base - stage.skyline) * SNOW_LINE;
    for (top, shape, farthest) in [
        (&far as &dyn Fn(i32) -> i32, &palette.far, true),
        (&mid, &palette.mid, false),
    ] {
        let (depth, calm) = match dip {
            Some(dip) if farthest => (dip.line - dip.mean(top), DIP_CALM),
            Some(dip) => ((dip.line - dip.mean(top)).max(0.), 0.),
            None => (0., 0.),
        };
        let top = |x: i32| dip.map_or(top(x), |dip| dip.lower(top(x), x, depth, calm));
        for x in (0..cols as i32).step_by(2) {
            let (here, before) = (top(x), top(x - 2));
            c.rect(x, here, 2, rows as i32 - here, shape.body);
            let snow = if farthest {
                let ragged = (smooth(SNOW_SEED, x as f32 / SNOW_WAVE) - 0.5) * SNOW_RAGGED;
                even((snowline - here as f32) * SNOW_SHARE + ragged)
            } else {
                0
            };
            let face = if before > here { LIT_FACE } else { 0 };
            c.rect(x, here, 2, snow.max(face), shape.edge);
        }
    }

    let sunk = dip.map_or(0., |dip| (dip.line - dip.mean(hills)).max(0.));
    let ground = |x: i32| dip.map_or(hills(x), |dip| dip.lower(hills(x), x, sunk, 0.));
    let mut soil = Rng(53);
    for x in 0..cols as i32 {
        let top = ground(x);
        c.rect(x, top, 1, GRASS_ROWS, palette.grass);
        if soil.next() < LIT_GRASS {
            c.set(x, top, palette.grass_lit);
        }
        c.rect(x, top + GRASS_ROWS, 1, DIRT_ROWS, palette.dirt);
        let deep = top + GRASS_ROWS + DIRT_ROWS;
        c.rect(x, deep, 1, rows as i32 - deep, palette.dirt_deep);
        if soil.next() < BURIED_STONE {
            c.set(x, top + 4 + (soil.next() * 8.) as i32, palette.stone);
        }
    }

    let torches = TORCHES.map(|at| (w * at) as i32);
    let mut trees = Rng(61);
    for _ in 0..(w / TREE_EVERY).round() as usize {
        let x = (trees.next() * w) as i32;
        let tall = 3 + (trees.next() * 3.) as i32;
        let wide = if trees.next() < 0.5 { 2 } else { 3 };
        let before_sun =
            dip.is_some_and(|dip| ((x - dip.x).abs() as f32) < dip.radius + wide as f32);
        let under_sidebar = x + wide < stage.sidebar;
        if before_sun || under_sidebar || torches.iter().any(|t| (x - t).abs() < wide + 4) {
            continue;
        }
        let foot = ground(x);
        c.rect(x, foot - tall, 1, tall, palette.trunk);
        let (top, height) = (foot - tall - wide * 2 + 1, wide * 2);
        for yy in 0..height {
            let inset = (yy == 0 || yy == height - 1) as i32;
            for xx in -wide + inset..=wide - inset {
                let colour = if xx > 0 {
                    palette.leaves_shade
                } else if trees.next() < LIT_LEAVES {
                    palette.leaves_lit
                } else {
                    palette.leaves
                };
                c.set(x + xx, top + yy, colour);
            }
        }
    }

    for x in torches {
        let foot = ground(x);
        for ring in (1..=2).rev() {
            c.disc(x, foot - 4, 1.5 + ring as f32, palette.glow, TORCH_GLOW);
        }
        c.rect(x, foot - 3, 1, 3, palette.stick);
        c.set(x, foot - 4, palette.flame);
    }

    let texels =
        c.px.iter()
            .map(|p| {
                let b = |v: f32| v.round().clamp(0., 255.) as u8;
                [b(p[0]), b(p[1]), b(p[2]), 255]
            })
            .collect();
    Picture {
        cols,
        rows,
        texels,
        stars,
    }
}

type Key = (u32, u32, u32, Look, Sun);

struct Baked {
    key: Key,
    image: Arc<RenderImage>,
    texel: f32,
    cols: usize,
    rows: usize,
    stars: Vec<Star>,
}

impl Baked {
    fn new(width: f64, height: f64, scale: f32, key: Key) -> Baked {
        let (.., look, sun) = key;
        let t = texel(width, scale);
        let cols = (width as f32 / t).ceil().max(1.) as usize;
        let rows = (height as f32 / t).ceil().max(1.) as usize;
        let stage = stage(width as f32, height as f32, t);
        let picture = paint(cols, rows, stage, sun, look);
        Baked {
            key,
            image: art::bake(&picture.texels, picture.cols, picture.rows, t, scale),
            texel: t,
            cols: picture.cols,
            rows: picture.rows,
            stars: picture.stars,
        }
    }
}

fn key(width: f64, height: f64, scale: f32, look: Look, content_bottom: f32) -> Key {
    let (w, h) = (width as f32, height as f32);
    let stage = stage(w, h, texel(width, scale));
    (
        width.round() as u32,
        height.round() as u32,
        (scale * 100.).round() as u32,
        look,
        sun_at(w, h, stage, content_bottom),
    )
}

pub struct Scene {
    look: Look,
    content_bottom: f32,
    baked: Option<Baked>,
    replaced: Vec<Arc<RenderImage>>,
    clock: Clock,
    unfocused: Option<Instant>,
    waking: bool,
}

impl Scene {
    pub fn new() -> Scene {
        Scene {
            look: Look::of(&NIGHT),
            content_bottom: 0.,
            baked: None,
            replaced: Vec::new(),
            clock: Clock::default(),
            unfocused: None,
            waking: false,
        }
    }

    pub fn set_look(&mut self, night: &Night) {
        self.look = Look::of(night);
    }

    pub fn set_content_bottom(&mut self, bottom: f32, cx: &mut Context<Self>) {
        if (bottom - self.content_bottom).abs() >= 1. {
            self.content_bottom = bottom;
            cx.notify();
        }
    }

    fn wake(&mut self, cx: &mut Context<Self>) {
        if self.waking {
            return;
        }
        self.waking = true;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(TICK).await;
            let _ = this.update(cx, |this, cx| {
                this.waking = false;
                cx.notify();
            });
        })
        .detach();
    }
}

impl Render for Scene {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        for image in self.replaced.drain(..) {
            let _ = window.drop_image(image);
        }
        let viewport = window.viewport_size();
        let (width, height) = (
            f64::from(f32::from(viewport.width)),
            f64::from(f32::from(viewport.height)),
        );
        let scale = window.scale_factor();
        let look = self.look;
        let key = key(width, height, scale, look, self.content_bottom);
        if self.baked.as_ref().map(|b| b.key) != Some(key) {
            if let Some(old) = self.baked.replace(Baked::new(width, height, scale, key)) {
                self.replaced.push(old.image);
            }
        }

        let now = Instant::now();
        if !window.is_window_active() {
            self.unfocused.get_or_insert(now);
        } else if self.unfocused.take().is_some() {
            motion::reread_system();
            self.wake(cx);
        }
        let twinkles = self
            .baked
            .as_ref()
            .is_some_and(|b| b.stars.iter().any(|s| s.speed > 0.));
        let run = twinkles
            && match motion::current() {
                Motion::Still => false,
                Motion::InFront => self.unfocused.is_none_or(|since| now - since < UNFOCUSED),
                Motion::Always => true,
            };
        self.clock.set(run, now);
        if run {
            self.wake(cx);
        }
        let time = self.clock.at(now).as_secs_f32();

        let baked = self.baked.as_ref().expect("baked above");
        let t = baked.texel;
        let snap = |v: f32| (v * scale).round() / scale;
        let dot = snap((t * STAR_DOT).max(SMALLEST_STAR_DOT));
        let stars: Vec<(Bounds<Pixels>, gpui::Hsla)> = baked
            .stars
            .iter()
            .map(|s| {
                let a = if s.speed > 0. {
                    TWINKLE_LOW
                        + (1. - TWINKLE_LOW) * (0.5 + 0.5 * (time * s.speed + s.phase).sin())
                } else {
                    1.
                };
                let [r, g, b] = s.colour.map(|v| v / 255.);
                (
                    Bounds::new(
                        point(px(snap(s.x * t)), px(snap(s.y * t))),
                        size(px(dot), px(dot)),
                    ),
                    gpui::Rgba { r, g, b, a }.into(),
                )
            })
            .collect();
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .overflow_hidden()
            .bg(gpui::rgb(look.palette().page))
            .child(
                img(baked.image.clone())
                    .absolute()
                    .top_0()
                    .left_0()
                    .w(px(baked.cols as f32 * t))
                    .h(px(baked.rows as f32 * t)),
            )
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, (), window, _| {
                        for (b, colour) in stars {
                            let b = Bounds::new(b.origin + bounds.origin, b.size);
                            window.paint_quad(fill(b, colour));
                        }
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
    }
}

pub fn track_content(scene: &Entity<Scene>) -> impl IntoElement {
    let scene = scene.clone();
    canvas(
        move |bounds, _, cx| {
            let bottom = f32::from(bounds.bottom());
            if (bottom - scene.read(cx).content_bottom).abs() >= 1. {
                cx.defer(move |cx| {
                    scene.update(cx, |scene, cx| scene.set_content_bottom(bottom, cx));
                });
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(p: &Picture, x: usize, y: usize) -> [u8; 4] {
        p.texels[y * p.cols + x]
    }

    fn light(c: [u8; 4]) -> u32 {
        c[0] as u32 + c[1] as u32 + c[2] as u32
    }

    fn window(width: f32, height: f32, look: Look) -> Picture {
        showing(width, height, 0., look)
    }

    fn showing(width: f32, height: f32, content_bottom: f32, look: Look) -> Picture {
        let t = texel(width as f64, 1.);
        let (cols, rows) = ((width / t).ceil() as usize, (height / t).ceil() as usize);
        let stage = stage(width, height, t);
        let sun = sun_at(width, height, stage, content_bottom);
        paint(cols, rows, stage, sun, look)
    }

    fn moon_rows(p: &Picture, look: Look) -> Vec<usize> {
        let moon = look.palette().moon.map(|v| v as u8);
        (0..p.rows)
            .filter(|&y| (0..p.cols).any(|x| at(p, x, y)[..3] == moon))
            .collect()
    }

    #[test]
    fn the_moon_covers_the_sun_and_the_corona_shows_around_it() {
        let sun = Sun::Beside { x: 0.86, scale: 1. };
        let p = paint(295, 190, stage(1180., 760., 4.), sun, Look::of(&NIGHT));
        let (sx, sy) = (
            (295. * 0.86f32).round() as usize,
            (190. * SUN_HIGH).round() as usize,
        );
        let r = (190. * SUN_SIZE).round() as usize;
        assert!(light(at(&p, sx, sy)) < 40);
        assert!(light(at(&p, sx + r + 2, sy)) > light(at(&p, sx + r * 4, sy)));
    }

    #[test]
    fn a_small_window_shows_the_whole_top_of_the_eclipse_below_the_content() {
        for (width, height) in [(960., 600.), (1180., 760.), (1440., 900.)] {
            for night in [NIGHT, DAWN] {
                let look = Look::of(&night);
                let p = window(width, height, look);
                let t = texel(width as f64, 1.);
                let rows_with_moon = moon_rows(&p, look);
                let first = *rows_with_moon.first().expect("the moon shows");
                assert!(
                    first as f32 * t >= home_bottom(height),
                    "{width}x{height}: the moon's top at {} px",
                    first as f32 * t
                );
                assert!(
                    rows_with_moon.len() as f32 >= SETTING_RADIUS - 1.,
                    "{width}x{height}: {} rows of the moon show",
                    rows_with_moon.len()
                );
            }
        }
    }

    #[test]
    fn the_ground_reaches_the_bottom_of_every_column() {
        let p = window(960., 600., Look::of(&NIGHT));
        for x in 0..p.cols {
            assert!(light(at(&p, x, p.rows - 1)) < 150, "column {x}");
        }
    }

    #[test]
    fn a_size_always_paints_the_same_picture() {
        let (a, b) = (
            window(960., 600., Look::of(&NIGHT)),
            window(960., 600., Look::of(&NIGHT)),
        );
        assert!(a.texels == b.texels);
        assert_eq!(a.stars.len(), b.stars.len());
    }

    #[test]
    fn the_day_has_no_stars_and_keeps_its_own_sky() {
        let day = Look::of(&DAWN);
        assert_eq!(day.sky, DAWN.sky);
        assert!(window(1920., 1080., day).stars.is_empty());
        let themed = Night {
            sky: eclipse_launcher::themes::FIRST_LIGHT.sky,
            ..DAWN
        };
        let washed = Look::of(&themed).sky;
        for (theme, shown) in themed.sky.iter().zip(washed) {
            assert!(rgb(shown).iter().sum::<f32>() > rgb(*theme).iter().sum::<f32>());
        }
    }

    const SIZES: [(f32, f32); 4] = [(960., 600.), (1180., 760.), (1440., 900.), (1920., 1080.)];

    #[test]
    fn the_hills_stay_under_home() {
        for (width, height) in SIZES {
            for night in [NIGHT, DAWN] {
                let look = Look::of(&night);
                let palette = look.palette();
                let hills = [
                    palette.far.body,
                    palette.far.edge,
                    palette.mid.body,
                    palette.mid.edge,
                ]
                .map(|c| c.map(|v| v as u8));
                let p = window(width, height, look);
                let above = (home_bottom(height) / texel(width as f64, 1.)).ceil() as usize;
                for y in 0..above {
                    for x in 0..p.cols {
                        assert!(
                            !hills.iter().any(|c| at(&p, x, y)[..3] == *c),
                            "{width}x{height}: a hill at ({x}, {y})"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_setting_sun_never_shows_over_the_content() {
        for (width, height) in [(960., 600.), (1180., 760.), (1440., 900.)] {
            let t = texel(width as f64, 1.);
            for content_bottom in [0., home_bottom(height) - 12., height - 14., height + 400.] {
                for night in [NIGHT, DAWN] {
                    let look = Look::of(&night);
                    let palette = look.palette();
                    let disc = [palette.moon, palette.rim].map(|c| c.map(|v| v as u8));
                    let p = showing(width, height, content_bottom, look);
                    let rows: Vec<usize> = (0..p.rows)
                        .filter(|&y| {
                            (0..p.cols).any(|x| disc.iter().any(|c| at(&p, x, y)[..3] == *c))
                        })
                        .collect();
                    assert!(
                        rows.iter().all(|&y| y as f32 * t >= content_bottom),
                        "{width}x{height} under content to {content_bottom}: the sun at row {:?}",
                        rows.first()
                    );
                    if content_bottom > home_bottom(height) {
                        assert!(
                            rows.is_empty(),
                            "{width}x{height}: the sun shows on a long page"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_setting_sun_sinks_behind_the_hills() {
        for (width, height) in [(960., 600.), (1180., 760.), (1440., 900.)] {
            for night in [NIGHT, DAWN] {
                let look = Look::of(&night);
                let shown = moon_rows(&window(width, height, look), look).len() as f32;
                let whole = 2. * SETTING_RADIUS - 1.;
                assert!(
                    shown < whole * 0.75,
                    "{width}x{height}: {shown} of the moon's {whole} rows show"
                );
            }
        }
    }

    #[test]
    fn a_high_sun_stays_in_the_gutter() {
        let (width, height) = (1920., 1080.);
        let (_, right) = stage(width, height, texel(1920., 1.)).column;
        for content_bottom in [0., height + 400.] {
            let look = Look::of(&NIGHT);
            let p = showing(width, height, content_bottom, look);
            let moon = look.palette().moon.map(|v| v as u8);
            let columns: Vec<usize> = (0..p.cols)
                .filter(|&x| (0..p.rows).any(|y| at(&p, x, y)[..3] == moon))
                .collect();
            assert!(!columns.is_empty() && columns.iter().all(|&x| x as i32 > right));
        }
    }

    #[test]
    fn no_cloud_runs_behind_a_card() {
        let look = Look::of(&DAWN);
        for (width, height) in SIZES {
            let stage = stage(width, height, texel(width as f64, 1.));
            let p = window(width, height, look);
            let (left, right) = stage.column;
            let reach = CLOUD_CLEARANCE as i32;
            let edges = (left - reach..left).chain(right + 1..=right + reach);
            let edges: Vec<usize> = edges
                .filter(|&x| x >= stage.sidebar && (x as usize) < p.cols)
                .map(|x| x as usize)
                .collect();
            for y in 0..stage.skyline as usize {
                let sky = at(&p, left as usize, y);
                for &x in &edges {
                    assert_eq!(at(&p, x, y), sky, "{width}x{height}: a cloud at ({x}, {y})");
                }
            }
        }
    }

    #[test]
    fn a_texel_covers_whole_device_pixels_at_any_scale() {
        for width in [960., 1180., 1920.] {
            for scale in [1., 1.25, 1.5, 1.75, 2., 2.5] {
                let device = texel(width, scale) * scale;
                assert!(
                    (device - device.round()).abs() < 1e-3,
                    "{width} px at {scale}x: {device} device px a texel"
                );
            }
        }
    }
}
