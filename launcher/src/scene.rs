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
const AFTER_SUNSET: usize = SKY_AT.len() - 2;
const HORIZON: f32 = 0.8;
pub(crate) const THEME_SKY_WASH: f32 = 0.88;

const SUN_SIZE: f32 = 0.03;
const SUN_HIGH: f32 = 0.58;
const GUTTER_MARGIN: f32 = 64.;
const SMALLEST_HIGH_SUN: f32 = 0.6;
const LARGEST_HIGH_SUN: f32 = 7.;
const SETTING_RADIUS: f32 = 12.;
const CLEAR_UNDER_HOME_HEAD: f32 = 310.;
const HILLS_HIGHEST: f32 = 0.62;
const SETTING_ACROSS: (f32, f32) = (0.6, 0.95);
const SETTING_AT: f32 = 0.84;
const SETTING_SHOWN: f32 = 0.4;
const SETTING_SHOWN_BEST: f32 = 0.45;

const CORONA_ALPHAS: [f32; 4] = [0.10, 0.065, 0.04, 0.02];
const DAY_CORONA_ALPHAS: [f32; 4] = [0.26, 0.15, 0.08, 0.035];
const CORONA_RING: f32 = 0.26;
const CORONA_FADE_RINGS: f32 = 2.;
const STREAMERS: usize = 12;
const STREAMER_ALPHA: f32 = 0.2;
const SHORT_STREAMER: f32 = 0.55;
const STREAMER_TWIST: f32 = 0.3;
const STREAMER_LENGTH: (f32, f32) = (0.9, 1.1);
const BEAD_ANGLE: f32 = FRAC_PI_4 * 5.;
const BEAD_SPARKLE: f32 = 0.6;
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
const TEXT_CLEARANCE: f32 = 24.;

const CLOUD_BAND: (f32, f32) = (0.45, 0.85);
const CLOUD_LENGTH: (f32, f32) = (14., 34.);
const CLOUD_ALPHA: f32 = 0.7;
const CLOUD_CLEARANCE: f32 = 4.;
const CLOUD_GROUP: usize = 3;
const CLOUD_STEP: (f32, f32) = (6., 10.);
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
const TORCH_HIGH: i32 = 4;
const TORCH_FLAME_GLOW: (f32, f32) = (1.5, 0.32);
const TORCH_HALO: (f32, f32) = (2.5, 0.12);

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
    corona_alphas: [f32; 4],
    streamer: Rgb,
    rim: Rgb,
    bead: Option<Rgb>,
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
    glow: Option<Rgb>,
}

const AT_NIGHT: Palette = Palette {
    page: eclipse_theme::glass::PAGE,
    stars: Some([[176., 182., 222.], [104., 110., 160.], [62., 67., 108.]]),
    clouds: None,
    corona: [255., 176., 72.],
    corona_alphas: CORONA_ALPHAS,
    streamer: [255., 196., 110.],
    rim: [255., 226., 170.],
    bead: Some([255., 248., 230.]),
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
    glow: Some([255., 179., 71.]),
};

const BY_DAY: Palette = Palette {
    page: DAWN.page,
    stars: None,
    clouds: Some(Shape {
        body: [255., 255., 255.],
        edge: [226., 222., 220.],
    }),
    corona: [244., 150., 40.],
    corona_alphas: DAY_CORONA_ALPHAS,
    streamer: [236., 140., 36.],
    rim: [240., 148., 16.],
    bead: None,
    moon: [80., 82., 102.],
    far: Shape {
        body: [196., 196., 218.],
        edge: [214., 214., 232.],
    },
    mid: Shape {
        body: [170., 170., 196.],
        edge: [184., 184., 208.],
    },
    grass: [116., 140., 112.],
    grass_lit: [136., 160., 126.],
    dirt: [152., 134., 114.],
    dirt_deep: [136., 120., 104.],
    stone: [140., 140., 146.],
    trunk: [104., 86., 72.],
    leaves: [94., 118., 96.],
    leaves_shade: [80., 102., 84.],
    leaves_lit: [114., 138., 110.],
    stick: [120., 94., 68.],
    flame: [255., 200., 90.],
    glow: None,
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

impl Stage {
    fn shows(&self, star: &Star, content_bottom: f32) -> bool {
        let reach = TEXT_CLEARANCE / self.texel;
        let (left, right) = self.column;
        let over_content = (left as f32 - reach..right as f32 + reach).contains(&star.x)
            && star.y * self.texel < content_bottom + TEXT_CLEARANCE;
        star.x >= self.sidebar as f32 && !over_content
    }
}

fn stage(width: f32, height: f32, texel: f32, home_bottom: f32) -> Stage {
    use crate::layout::{content_width, sidebar};
    let column = content_width(width);
    let side = sidebar(width);
    let left = side + (width - side - column) / 2.;
    let skyline = (home_bottom / texel).max(height / texel * HILLS_HIGHEST);
    Stage {
        texel,
        sidebar: (side / texel) as i32,
        column: (
            (left / texel) as i32,
            ((left + column) / texel).ceil() as i32,
        ),
        skyline: (skyline / 2.).ceil() * 2.,
    }
}

#[derive(Clone, Copy)]
struct Tree {
    x: i32,
    foot: i32,
    tall: i32,
    wide: i32,
}

impl Tree {
    fn top(&self) -> i32 {
        self.foot - self.tall - self.wide * 2 + 1
    }
}

struct Land {
    skyline: f32,
    base: f32,
    cols: f32,
    rows: f32,
    torches: [i32; 2],
    trees: Vec<Tree>,
}

impl Land {
    fn new(cols: usize, rows: usize, stage: Stage) -> Land {
        let (w, h) = (cols as f32, rows as f32);
        let mut land = Land {
            skyline: stage.skyline,
            base: (h * (GROUND_FOOT - GROUND_RISE / 2.)).max(stage.skyline),
            cols: w,
            rows: h,
            torches: TORCHES.map(|at| (w * at) as i32),
            trees: Vec::new(),
        };
        let mut rng = Rng(61);
        for _ in 0..(w / TREE_EVERY).round() as usize {
            let x = (rng.next() * w) as i32;
            let tall = 3 + (rng.next() * 3.) as i32;
            let wide = if rng.next() < 0.5 { 2 } else { 3 };
            let tree = Tree {
                x,
                foot: land.ground(x),
                tall,
                wide,
            };
            let under_sidebar = x + wide < stage.sidebar;
            let over_home = (tree.top() as f32) < stage.skyline;
            let by_torch = land.torches.iter().any(|t| (x - t).abs() < wide + 4);
            if !(under_sidebar || over_home || by_torch) {
                land.trees.push(tree);
            }
        }
        land
    }

    fn far(&self, x: i32) -> i32 {
        FAR_RIDGE.top(x, self.skyline, self.base)
    }

    fn mid(&self, x: i32) -> i32 {
        MID_RIDGE.top(x, self.skyline, self.base)
    }

    fn ground(&self, x: i32) -> i32 {
        let block = x.div_euclid(GROUND_BLOCK) * GROUND_BLOCK;
        let rise = rolling(91, block as f32, GROUND_SPAN) * self.rows * GROUND_RISE;
        even(self.rows * GROUND_FOOT - rise)
    }

    fn top(&self, x: i32) -> i32 {
        let ridge = x - x.rem_euclid(2);
        let crowns = self
            .trees
            .iter()
            .filter(|t| (x - t.x).abs() <= t.wide)
            .map(|t| t.top() + ((x - t.x).abs() == t.wide) as i32);
        let flames = self
            .torches
            .iter()
            .filter(|&&t| t == x)
            .map(|_| self.ground(x) - TORCH_HIGH);
        let hills = self.far(ridge).min(self.mid(ridge)).min(self.ground(x));
        crowns.chain(flames).fold(hills, i32::min)
    }

    fn setting_sun(&self) -> Option<(i32, i32)> {
        let r = SETTING_RADIUS + 1.;
        let reach = r as i32;
        let w = self.cols;
        let corona = SETTING_RADIUS + CORONA_ALPHAS.len() as f32 * corona_ring(SETTING_RADIUS);
        let from = (w * SETTING_ACROSS.0) as i32;
        let to = ((w * SETTING_ACROSS.1).min(w - corona) as i32).max(from);
        let chords: Vec<(i32, i32)> = (-reach..reach)
            .map(|dx| {
                let rows: Vec<i32> = (-reach..reach).filter(|&dy| inside(r, dx, dy)).collect();
                (rows[0], rows.len() as i32)
            })
            .collect();
        let disc: i32 = chords.iter().map(|&(_, len)| len).sum();
        let tops: Vec<i32> = (from - reach..to + reach).map(|x| self.top(x)).collect();
        let shown = |x: i32, y: i32| {
            let seen: Vec<i32> = (x - reach..x + reach)
                .zip(&chords)
                .map(|(at, &(first, len))| {
                    (tops[(at - from + reach) as usize] - y - first).clamp(0, len)
                })
                .collect();
            let pieces = seen
                .windows(2)
                .filter(|pair| pair[0] == 0 && pair[1] > 0)
                .count()
                + (seen[0] > 0) as usize;
            let middle = seen[reach as usize - 1] > 0 && seen[reach as usize] > 0;
            let share = seen.iter().sum::<i32>() as f32 / disc as f32;
            (pieces == 1 && middle && share >= SETTING_SHOWN).then_some(share)
        };
        let rise = |x: i32| {
            let under = &tops[(x - from) as usize..(x - from + 2 * reach) as usize];
            under.iter().max().unwrap_or(&0) - under.iter().min().unwrap_or(&0)
        };
        let mut across: Vec<i32> = (from..=to).collect();
        across.sort_by_key(|&x| (rise(x), (x - (w * SETTING_AT) as i32).abs()));
        let highest = self.skyline as i32 + reach;
        across.into_iter().find_map(|x| {
            (highest..self.rows as i32)
                .filter_map(|y| shown(x, y).map(|share| (y, share)))
                .min_by(|a, b| {
                    let off = |share: f32| (share - SETTING_SHOWN_BEST).abs();
                    off(a.1).total_cmp(&off(b.1))
                })
                .map(|(y, _)| (x, y))
        })
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

    fn disc(&mut self, cx: i32, cy: i32, r: f32, c: Rgb, a: impl Fn(i32) -> f32) {
        let reach = r.ceil() as i32;
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                if inside(r, dx, dy) {
                    self.blend(cx + dx, cy + dy, c, a(cy + dy));
                }
            }
        }
    }

    fn halo(&mut self, x: i32, y: i32, (r, a): (f32, f32), c: Rgb) {
        let reach = r as i32;
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                if ((dx * dx + dy * dy) as f32) <= r * r {
                    self.blend(x + dx, y + dy, c, a);
                }
            }
        }
    }
}

fn inside(r: f32, dx: i32, dy: i32) -> bool {
    (dx as f32 + 0.5).powi(2) + (dy as f32 + 0.5).powi(2) <= r * r
}

fn mix(from: Rgb, to: Rgb, t: f32) -> Rgb {
    [0, 1, 2].map(|i| from[i] + (to[i] - from[i]) * t)
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
    let t = (position - SKY_AT[next - 1]) / (SKY_AT[next] - SKY_AT[next - 1]);
    mix(rgb(stops[next - 1]), rgb(stops[next]), t)
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
    Beside { x: i32, r: i32 },
    Setting,
    Set,
}

fn sun_at(width: f32, height: f32, stage: Stage, content_bottom: f32) -> Sun {
    let t = stage.texel;
    let gutter = crate::layout::page_margin(width);
    let x = ((width - gutter / 2.) / t).round();
    let to_cards = x * t - (width - gutter);
    let to_edge = width - x * t;
    let room = (to_cards.min(to_edge) - GUTTER_MARGIN) / t;
    let largest = (width.min(height) / t * SUN_SIZE)
        .round()
        .min(LARGEST_HIGH_SUN);
    let smallest = (largest * SMALLEST_HIGH_SUN).ceil() as i32;
    let fits = (smallest..=largest as i32)
        .rev()
        .find(|&r| sun_reach(r as f32) <= room);
    if let Some(r) = fits {
        Sun::Beside { x: x as i32, r }
    } else if content_bottom > stage.skyline * t {
        Sun::Set
    } else {
        Sun::Setting
    }
}

fn corona_ring(r: f32) -> f32 {
    (r * CORONA_RING).ceil()
}

fn sun_reach(r: f32) -> f32 {
    let corona = r + CORONA_ALPHAS.len() as f32 * corona_ring(r);
    let streamers = (r * (1. + STREAMER_LENGTH.1)).round() + 1.;
    corona.max(streamers)
}

fn paint(cols: usize, rows: usize, stage: Stage, sun: Sun, look: Look) -> Picture {
    let palette = look.palette();
    let mut c = Canvas {
        w: cols,
        h: rows,
        px: vec![[0.; 3]; cols * rows],
    };
    let (w, h) = (cols as f32, rows as f32);
    let land = Land::new(cols, rows, stage);

    let disc = match sun {
        Sun::Beside { x, r } => Some((r as f32, x, (h * SUN_HIGH).round() as i32)),
        Sun::Setting => land.setting_sun().map(|(x, y)| (SETTING_RADIUS, x, y)),
        Sun::Set => None,
    };

    let horizon = match (sun, disc) {
        (Sun::Beside { .. }, _) => h * HORIZON,
        (_, Some((r, _, sy))) => (h * HORIZON).max(sy as f32 + r),
        (_, None) => {
            let lowest_sky = (stage.sidebar..cols as i32).map(|x| land.top(x)).max();
            lowest_sky.unwrap_or(rows as i32) as f32 / SKY_AT[AFTER_SUNSET]
        }
    }
    .max(stage.skyline)
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

    if let Some(shape) = &palette.clouds {
        for cloud in clouds(stage, cols, disc) {
            let part = |share: f32| (cloud.len * share).round() as i32;
            for (row, &(from, share)) in CLOUD_ROWS.iter().enumerate() {
                let underside = row == CLOUD_ROWS.len() - 1;
                let colour = if underside { shape.edge } else { shape.body };
                c.veil(
                    cloud.x + part(from),
                    cloud.y + row as i32,
                    part(share),
                    1,
                    colour,
                    CLOUD_ALPHA,
                );
            }
        }
    }

    if let Some(disc) = disc {
        let ring = corona_ring(disc.0);
        let under_home = |y: i32| match sun {
            Sun::Setting => ((y as f32 - stage.skyline) / (ring * CORONA_FADE_RINGS)).clamp(0., 1.),
            _ => 1.,
        };
        eclipse(&mut c, disc, palette, under_home);
    }

    let snowline = stage.skyline + (land.base - stage.skyline) * SNOW_LINE;
    let far = |x: i32| land.far(x);
    let mid = |x: i32| land.mid(x);
    for (top, shape, farthest) in [
        (&far as &dyn Fn(i32) -> i32, &palette.far, true),
        (&mid, &palette.mid, false),
    ] {
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

    let mut soil = Rng(53);
    for x in 0..cols as i32 {
        let top = land.ground(x);
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

    let mut leaves = Rng(67);
    for tree in &land.trees {
        let Tree {
            x,
            foot,
            tall,
            wide,
        } = *tree;
        let (top, height) = (tree.top(), wide * 2);
        c.rect(x, foot - tall, 1, tall, palette.trunk);
        for yy in 0..height {
            let inset = (yy == 0 || yy == height - 1) as i32;
            for xx in -wide + inset..=wide - inset {
                let colour = if xx > 0 {
                    palette.leaves_shade
                } else if leaves.next() < LIT_LEAVES {
                    palette.leaves_lit
                } else {
                    palette.leaves
                };
                c.set(x + xx, top + yy, colour);
            }
        }
    }

    for x in land.torches {
        torch(&mut c, x, land.ground(x), palette);
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

struct Cloud {
    x: i32,
    y: i32,
    len: f32,
}

fn clouds(stage: Stage, cols: usize, disc: Option<(f32, i32, i32)>) -> Vec<Cloud> {
    let (left, right) = stage.column;
    let clear = CLOUD_CLEARANCE as i32;
    let gutters = [
        (stage.sidebar + clear, left - clear),
        (right + clear + 1, cols as i32 - clear),
    ];
    let tallest = CLOUD_ROWS.len() as f32 + (CLOUD_GROUP - 1) as f32 * CLOUD_STEP.1;
    let mut out = Vec::new();
    for ((from, to), seed) in gutters.into_iter().zip([34, 31]) {
        let room = (to - from) as f32;
        let holds_sun = disc.is_some_and(|(r, sx, _)| {
            let reach = r * SUN_CLEARANCE;
            sx as f32 + reach > from as f32 && (sx as f32) - reach < to as f32
        });
        if room < CLOUD_LENGTH.0 || holds_sun {
            continue;
        }
        let mut drift = Rng(seed);
        let mut y = drift.between(CLOUD_BAND) * (stage.skyline - tallest);
        out.extend((0..CLOUD_GROUP).map(|_| {
            let len = drift.between(CLOUD_LENGTH).min(room);
            let x = from + (drift.next() * (room - len)) as i32;
            let cloud = Cloud {
                x,
                y: y as i32,
                len,
            };
            y += drift.between(CLOUD_STEP);
            cloud
        }));
    }
    out
}

fn eclipse(
    c: &mut Canvas,
    (r, sx, sy): (f32, i32, i32),
    palette: &Palette,
    under_home: impl Fn(i32) -> f32,
) {
    let ring = corona_ring(r);
    for (i, alpha) in palette.corona_alphas.iter().enumerate().rev() {
        c.disc(sx, sy, r + (i + 1) as f32 * ring, palette.corona, |y| {
            alpha * under_home(y)
        });
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
            let y = sy + (dy * d).round() as i32;
            c.blend(
                sx + (dx * d).round() as i32,
                y,
                palette.streamer,
                STREAMER_ALPHA * fade * under_home(y),
            );
            d += 0.5;
        }
    }
    c.disc(sx, sy, r + 1., palette.rim, |_| 1.);
    c.disc(sx, sy, r, palette.moon, |_| 1.);
    let Some(bead) = palette.bead else {
        return;
    };
    let (bx, by) = (
        sx + (BEAD_ANGLE.cos() * r).round() as i32,
        sy + (BEAD_ANGLE.sin() * r).round() as i32,
    );
    c.rect(bx - 1, by - 1, 2, 2, bead);
    if r >= BEAD_SPARKLE_FROM {
        for (dx, dy) in [
            (-2, 0),
            (2, 0),
            (0, -2),
            (0, 2),
            (-1, 0),
            (1, 0),
            (0, -1),
            (0, 1),
        ] {
            let (x, y) = (bx + dx - (dx < 0) as i32, by + dy - (dy < 0) as i32);
            if !inside(r, x - sx, y - sy) {
                c.blend(x, y, palette.rim, BEAD_SPARKLE);
            }
        }
    }
}

fn torch(c: &mut Canvas, x: i32, foot: i32, palette: &Palette) {
    let flame = foot - TORCH_HIGH;
    if let Some(glow) = palette.glow {
        c.halo(x, flame, TORCH_HALO, glow);
        c.halo(x, flame, TORCH_FLAME_GLOW, palette.flame);
    }
    c.rect(x, flame + 1, 1, TORCH_HIGH - 1, palette.stick);
    c.set(x, flame, palette.flame);
}

type Key = (u32, u32, u32, u32, Look, Sun);

struct Baked {
    key: Key,
    image: Arc<RenderImage>,
    stage: Stage,
    cols: usize,
    rows: usize,
    stars: Vec<Star>,
}

impl Baked {
    fn new(width: f64, height: f64, scale: f32, key: Key) -> Baked {
        let (.., home_bottom, look, sun) = key;
        let t = texel(width, scale);
        let cols = (width as f32 / t).ceil().max(1.) as usize;
        let rows = (height as f32 / t).ceil().max(1.) as usize;
        let stage = stage(width as f32, height as f32, t, home_bottom as f32);
        let picture = paint(cols, rows, stage, sun, look);
        Baked {
            key,
            image: art::bake(&picture.texels, picture.cols, picture.rows, t, scale),
            stage,
            cols: picture.cols,
            rows: picture.rows,
            stars: picture.stars,
        }
    }

    fn shown(&self, content_bottom: f32) -> impl Iterator<Item = &Star> {
        self.stars
            .iter()
            .filter(move |s| self.stage.shows(s, content_bottom))
    }
}

fn key(
    width: f64,
    height: f64,
    scale: f32,
    look: Look,
    home_bottom: f32,
    content_bottom: f32,
) -> Key {
    let (w, h) = (width as f32, height as f32);
    let home_bottom = home_bottom.round();
    let stage = stage(w, h, texel(width, scale), home_bottom);
    (
        width.round() as u32,
        height.round() as u32,
        (scale * 100.).round() as u32,
        home_bottom as u32,
        look,
        sun_at(w, h, stage, content_bottom),
    )
}

pub struct Scene {
    look: Look,
    home_head: Option<f32>,
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
            home_head: None,
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

    fn set_home_head(&mut self, height: f32, cx: &mut Context<Self>) {
        if self.home_head.is_none_or(|was| (height - was).abs() >= 1.) {
            self.home_head = Some(height);
            cx.notify();
        }
    }

    fn home_bottom(&self) -> f32 {
        self.home_head.map_or(0., |head| {
            crate::layout::CONTENT_INSET + head + CLEAR_UNDER_HOME_HEAD
        })
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
        let key = key(
            width,
            height,
            scale,
            look,
            self.home_bottom(),
            self.content_bottom,
        );
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
        let content_bottom = self.content_bottom;
        let twinkles = self
            .baked
            .as_ref()
            .is_some_and(|b| b.shown(content_bottom).any(|s| s.speed > 0.));
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
        let t = baked.stage.texel;
        let snap = |v: f32| (v * scale).round() / scale;
        let dot = snap((t * STAR_DOT).max(SMALLEST_STAR_DOT));
        let stars: Vec<(Bounds<Pixels>, gpui::Hsla)> = baked
            .shown(content_bottom)
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

pub fn track_home_head(scene: &Entity<Scene>) -> impl IntoElement {
    let scene = scene.clone();
    canvas(
        move |bounds, _, cx| {
            let height = f32::from(bounds.size.height);
            let was = scene.read(cx).home_head;
            if was.is_none_or(|was| (height - was).abs() >= 1.) {
                cx.defer(move |cx| {
                    scene.update(cx, |scene, cx| scene.set_home_head(height, cx));
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

    fn home(height: f32) -> f32 {
        let head = if height < crate::layout::SHORT_WINDOW {
            160.
        } else {
            192.
        };
        crate::layout::CONTENT_INSET + head + CLEAR_UNDER_HOME_HEAD
    }

    fn window(width: f32, height: f32, look: Look) -> Picture {
        showing(width, height, 0., look)
    }

    fn showing(width: f32, height: f32, content_bottom: f32, look: Look) -> Picture {
        let t = texel(width as f64, 1.);
        let (cols, rows) = ((width / t).ceil() as usize, (height / t).ceil() as usize);
        let stage = stage(width, height, t, home(height));
        let sun = sun_at(width, height, stage, content_bottom);
        paint(cols, rows, stage, sun, look)
    }

    const SETTING_SIZES: [(f32, f32); 5] = [
        (960., 600.),
        (1180., 760.),
        (1280., 800.),
        (1366., 768.),
        (1440., 900.),
    ];

    fn moon_rows(p: &Picture, look: Look) -> Vec<usize> {
        let moon = look.palette().moon.map(|v| v as u8);
        (0..p.rows)
            .filter(|&y| (0..p.cols).any(|x| at(p, x, y)[..3] == moon))
            .collect()
    }

    #[test]
    fn the_moon_covers_the_sun_and_the_corona_shows_around_it() {
        let (sx, r) = (254, 10);
        let sun = Sun::Beside {
            x: sx as i32,
            r: r as i32,
        };
        let p = paint(
            295,
            190,
            stage(1180., 760., 4., home(760.)),
            sun,
            Look::of(&NIGHT),
        );
        let sy = (190. * SUN_HIGH).round() as usize;
        assert!(light(at(&p, sx, sy)) < 40);
        assert!(light(at(&p, sx + r + 2, sy)) > light(at(&p, sx + r * 4, sy)));
    }

    fn land(width: f32, height: f32) -> Land {
        let t = texel(width as f64, 1.);
        let (cols, rows) = ((width / t).ceil() as usize, (height / t).ceil() as usize);
        Land::new(cols, rows, stage(width, height, t, home(height)))
    }

    #[test]
    fn the_setting_sun_shows_a_readable_cap_below_the_content() {
        let r = SETTING_RADIUS + 1.;
        let reach = r as i32;
        for (width, height) in SETTING_SIZES {
            let land = land(width, height);
            let (cx, cy) = land.setting_sun().expect("a setting sun");
            let top = (cy - reach) as f32 * texel(width as f64, 1.);
            assert!(
                top >= home(height),
                "{width}x{height}: the sun's top at {top} px"
            );
            for night in [NIGHT, DAWN] {
                let look = Look::of(&night);
                let palette = look.palette();
                let bead = palette.bead.unwrap_or(palette.rim);
                let sun = [palette.moon, palette.rim, bead].map(colour);
                let arc = [palette.rim, bead].map(colour);
                let p = window(width, height, look);
                let shade = |x: i32, y: i32| {
                    let [red, green, blue, _] = at(&p, x as usize, y as usize);
                    [red, green, blue]
                };
                let size = format!("{width}x{height}, dark {}", look.dark);
                let (mut shown, mut disc, mut columns, mut cut) = (0, 0, Vec::new(), Vec::new());
                for dx in -reach..reach {
                    let x = cx + dx;
                    let column: Vec<i32> = (-reach..reach)
                        .filter(|&dy| inside(r, dx, dy))
                        .map(|dy| cy + dy)
                        .collect();
                    for &y in &column {
                        let showing = sun.contains(&shade(x, y));
                        assert_eq!(showing, y < land.top(x), "{size}: the sun at ({x}, {y})");
                    }
                    let seen = column.iter().filter(|&&y| y < land.top(x)).count();
                    if seen > 0 {
                        assert!(arc.contains(&shade(x, column[0])), "{size}: the arc at {x}");
                        columns.push(dx);
                        cut.push(land.top(x));
                    }
                    (shown, disc) = (shown + seen, disc + column.len());
                }
                assert!(
                    shown as f32 >= disc as f32 * SETTING_SHOWN,
                    "{size}: {shown} of the disc's {disc} texels show"
                );
                assert!(
                    columns.windows(2).all(|pair| pair[1] == pair[0] + 1)
                        && columns.contains(&-1)
                        && columns.contains(&0),
                    "{size}: the arc shows over {columns:?}"
                );
                let rise = cut.iter().max().unwrap_or(&0) - cut.iter().min().unwrap_or(&0);
                assert!(rise >= 2, "{size}: the hills cut the sun level at {cut:?}");
            }
        }
    }

    #[test]
    fn the_setting_suns_glow_stays_under_home() {
        let glow = SETTING_RADIUS + CORONA_ALPHAS.len() as f32 * corona_ring(SETTING_RADIUS);
        let reach = glow.ceil() as i32 + 1;
        for (width, height) in SETTING_SIZES {
            let t = texel(width as f64, 1.);
            let skyline = stage(width, height, t, home(height)).skyline as usize;
            let (sx, _) = land(width, height).setting_sun().expect("a setting sun");
            for night in [NIGHT, DAWN] {
                let p = window(width, height, Look::of(&night));
                for y in 0..skyline {
                    for x in (sx - reach).max(0)..(sx + reach).min(p.cols as i32) {
                        assert_eq!(
                            at(&p, x as usize, y),
                            at(&p, 0, y),
                            "{width}x{height}: the glow at ({x}, {y})"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_setting_sun_leaves_the_hills_as_they_are() {
        for (width, height) in SETTING_SIZES {
            let t = texel(width as f64, 1.);
            let (cols, rows) = ((width / t).ceil() as usize, (height / t).ceil() as usize);
            let stage = stage(width, height, t, home(height));
            let land = Land::new(cols, rows, stage);
            for night in [NIGHT, DAWN] {
                let look = Look::of(&night);
                let setting = paint(cols, rows, stage, Sun::Setting, look);
                let set = paint(cols, rows, stage, Sun::Set, look);
                assert!(!moon_rows(&setting, look).is_empty(), "{width}x{height}");
                for x in 0..cols {
                    for y in land.top(x as i32).max(0) as usize..rows {
                        assert_eq!(
                            at(&setting, x, y),
                            at(&set, x, y),
                            "{width}x{height}: the land at ({x}, {y})"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn once_the_sun_has_set_no_glow_shows_over_the_hills() {
        let look = Look::of(&NIGHT);
        let dusk = rgb(NIGHT.sky[AFTER_SUNSET])[0] as u8;
        for (width, height) in [SMALLEST, (960., 600.), (1180., 760.), (1440., 900.)] {
            let p = showing(width, height, height + 400., look);
            let land = land(width, height);
            let sidebar = stage(width, height, texel(width as f64, 1.), home(height)).sidebar;
            for x in sidebar..p.cols as i32 {
                if land
                    .torches
                    .iter()
                    .any(|t| (x - t).abs() <= TORCH_HALO.0 as i32)
                {
                    continue;
                }
                for y in 0..land.top(x) {
                    assert!(
                        at(&p, x as usize, y as usize)[0] <= dusk,
                        "{width}x{height}: a glow at ({x}, {y})"
                    );
                }
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
            sky: eclipse_launcher::themes::DAYBREAK.sky,
            ..DAWN
        };
        let washed = Look::of(&themed).sky;
        for (theme, shown) in themed.sky.iter().zip(washed) {
            assert!(rgb(shown).iter().sum::<f32>() > rgb(*theme).iter().sum::<f32>());
        }
    }

    const SIZES: [(f32, f32); 4] = [(960., 600.), (1180., 760.), (1440., 900.), (1920., 1080.)];
    const SMALLEST: (f32, f32) = (800., 500.);

    #[test]
    fn the_hills_stay_under_home() {
        for (width, height) in [SMALLEST].into_iter().chain(SIZES) {
            for night in [NIGHT, DAWN] {
                let look = Look::of(&night);
                let palette = look.palette();
                let hills = [
                    palette.far.body,
                    palette.far.edge,
                    palette.mid.body,
                    palette.mid.edge,
                    palette.trunk,
                    palette.leaves,
                    palette.leaves_shade,
                ]
                .map(|c| c.map(|v| v as u8));
                let p = window(width, height, look);
                let above = (home(height) / texel(width as f64, 1.)).ceil() as usize;
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
        for (width, height) in SETTING_SIZES {
            let t = texel(width as f64, 1.);
            for content_bottom in [0., home(height) - 12., height - 14., height + 400.] {
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
                    if content_bottom > home(height) {
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
        for (width, height) in SETTING_SIZES {
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
        let (_, right) = stage(width, height, texel(1920., 1.), home(height)).column;
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

    const NARROW_PLAY: f32 = 240.;

    const HIGH_SUN_SIZES: [(f32, f32); 5] = [
        (1920., 1040.),
        (1920., 1080.),
        (1920., 1200.),
        (2560., 1440.),
        (3440., 1440.),
    ];

    fn high_sun(width: f32, height: f32) -> (Stage, i32, i32) {
        let stage = stage(width, height, texel(width as f64, 1.), home(height));
        match sun_at(width, height, stage, 0.) {
            Sun::Beside { x, r } => (stage, x, r),
            _ => panic!("{width}x{height}: no high sun"),
        }
    }

    fn hex(c: Rgb) -> u32 {
        c.iter()
            .fold(0, |out, v| (out << 8) | v.round().clamp(0., 255.) as u32)
    }

    fn apart(a: Rgb, b: Rgb) -> f32 {
        let (a, b) = (
            eclipse_theme::glass::oklab(hex(a)),
            eclipse_theme::glass::oklab(hex(b)),
        );
        (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f32>().sqrt()
    }

    fn sky_at_sun(look: Look, stage: Stage, rows: usize) -> Rgb {
        let sy = (rows as f32 * SUN_HIGH).round();
        let horizon = (rows as f32 * HORIZON).max(stage.skyline).round();
        sky(&look.sky, (sy / horizon).min(1.))
    }

    #[test]
    fn a_high_sun_keeps_clear_of_the_cards_and_the_window_edge() {
        let look = Look::of(&NIGHT);
        let tight = (1800..=2600)
            .step_by(40)
            .flat_map(|width| [(width as f32, 1000.), (width as f32, 1200.)]);
        for (width, height) in HIGH_SUN_SIZES.into_iter().chain(tight) {
            let t = texel(width as f64, 1.);
            let stage = stage(width, height, t, home(height));
            let sun @ Sun::Beside { r, .. } = sun_at(width, height, stage, 0.) else {
                continue;
            };
            let (cols, rows) = ((width / t).ceil() as usize, (height / t).ceil() as usize);
            let with = paint(cols, rows, stage, sun, look);
            let without = paint(
                cols,
                rows,
                stage,
                Sun::Beside {
                    x: -(cols as i32),
                    r,
                },
                look,
            );
            let cards = width - crate::layout::page_margin(width);
            let lit: Vec<usize> = (0..cols)
                .filter(|&x| (0..rows).any(|y| at(&with, x, y) != at(&without, x, y)))
                .collect();
            assert!(!lit.is_empty(), "{width}x{height}: no sun");
            for x in lit {
                let (left, right) = (x as f32 * t, (x + 1) as f32 * t);
                assert!(
                    left >= cards + GUTTER_MARGIN && right <= width - GUTTER_MARGIN,
                    "{width}x{height}: the sun at {left}..{right} px, the cards end at {cards}"
                );
            }
        }
    }

    #[test]
    fn the_high_sun_is_quieter_than_play() {
        use eclipse_theme::glass::ACCENT;
        let play_size = NARROW_PLAY * crate::ui::ButtonSize::Hero.height();
        for (width, height) in HIGH_SUN_SIZES {
            let (stage, _, r) = high_sun(width, height);
            let rows = (height / stage.texel).ceil() as usize;
            for night in [NIGHT, DAWN] {
                let look = Look::of(&night);
                let behind = sky_at_sun(look, stage, rows);
                let side = 2 * sun_reach(r as f32) as i32 + 2;
                let mut c = Canvas {
                    w: side as usize,
                    h: side as usize,
                    px: vec![behind; (side * side) as usize],
                };
                eclipse(
                    &mut c,
                    (r as f32, side / 2, side / 2),
                    look.palette(),
                    |_| 1.,
                );
                let texel_size = stage.texel * stage.texel;
                let sun: f32 = c.px.iter().map(|&p| apart(p, behind)).sum::<f32>() * texel_size;
                let play = apart(rgb(ACCENT), rgb(look.sky[0])) * play_size;
                assert!(
                    sun < play,
                    "{width}x{height}, dark {}: the sun weighs {sun:.0}, Play {play:.0}",
                    look.dark
                );
            }
        }
    }

    #[test]
    fn at_night_the_ring_draws_the_eclipse_not_the_disc() {
        for (width, height) in HIGH_SUN_SIZES {
            let (stage, _, _) = high_sun(width, height);
            let rows = (height / stage.texel).ceil() as usize;
            let look = Look::of(&NIGHT);
            let behind = hex(sky_at_sun(look, stage, rows));
            let moon = eclipse_theme::contrast(hex(look.palette().moon), behind);
            assert!(
                moon < 3.,
                "{width}x{height}: the moon is {moon:.2}:1 on its sky"
            );
        }
    }

    fn corona_glow(palette: &Palette, behind: Rgb) -> f32 {
        let inner = palette
            .corona_alphas
            .iter()
            .rev()
            .fold(behind, |c, &a| mix(c, palette.corona, a));
        apart(inner, behind)
    }

    #[test]
    fn by_day_a_dark_moon_and_its_corona_stand_out_on_the_sky() {
        let day = Look::of(&DAWN);
        let palette = day.palette();
        let (stage, _, _) = high_sun(1920., 1080.);
        let night = corona_glow(&AT_NIGHT, sky_at_sun(Look::of(&NIGHT), stage, 216));
        let mut behind = Vec::new();
        for (width, height) in HIGH_SUN_SIZES {
            let (stage, _, _) = high_sun(width, height);
            let sky = sky_at_sun(day, stage, (height / stage.texel).ceil() as usize);
            let glow = corona_glow(palette, sky);
            assert!(
                glow >= night * 2. / 3.,
                "{width}x{height}: the corona stands {glow:.3} off its sky, {night:.3} at night"
            );
            behind.push(sky);
        }
        for (width, height) in SETTING_SIZES {
            let land = land(width, height);
            let (_, cy) = land.setting_sun().expect("a setting sun");
            let horizon = (land.rows * HORIZON)
                .max(cy as f32 + SETTING_RADIUS)
                .max(land.skyline)
                .round();
            let shown = cy - SETTING_RADIUS as i32..cy;
            behind.extend(shown.map(|y| sky(&day.sky, (y as f32 / horizon).min(1.))));
        }
        for sky in behind {
            let moon = eclipse_theme::contrast(hex(palette.moon), hex(sky));
            assert!(moon >= 3., "the moon is {moon:.2}:1 on #{:06x}", hex(sky));
        }
    }

    #[test]
    fn a_themes_washed_sky_keeps_the_light_look_readable() {
        use eclipse_launcher::themes::{SWATCHES, THEMES};
        use eclipse_theme::glass::{ink, light_ink, Glass, ACCENT, ON_LIGHT};
        for theme in THEMES {
            let washed = Look::of(&Night {
                sky: theme.sky,
                ..DAWN
            });
            for stop in washed.sky {
                for level in ink::LEVELS {
                    let a = light_ink(level);
                    let text = eclipse_theme::contrast(over(ON_LIGHT, a, stop), stop);
                    assert!(
                        text >= 4.5,
                        "{}: ink {a} on #{stop:06x}: {text:.2}",
                        theme.id
                    );
                }
                for accent in [ACCENT].into_iter().chain(SWATCHES) {
                    let look = Glass::new(accent).light();
                    let mark = eclipse_theme::contrast(look.mark, stop);
                    assert!(
                        mark >= 3.,
                        "{}: #{accent:06x}'s mark on #{stop:06x}: {mark:.2}",
                        theme.id
                    );
                    let link = eclipse_theme::contrast(look.accent_soft, stop);
                    assert!(
                        link >= 4.5,
                        "{}: #{accent:06x}'s links on #{stop:06x}: {link:.2}",
                        theme.id
                    );
                }
            }
        }
    }

    #[test]
    fn no_cloud_runs_behind_a_card() {
        let look = Look::of(&DAWN);
        for (width, height) in SIZES {
            let stage = stage(width, height, texel(width as f64, 1.), home(height));
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

    fn colour(c: Rgb) -> [u8; 3] {
        c.map(|v| v as u8)
    }

    fn torch_on_page(palette: &Palette) -> Canvas {
        let mut c = Canvas {
            w: 11,
            h: 11,
            px: vec![rgb(palette.page); 121],
        };
        torch(&mut c, 5, 9, palette);
        c
    }

    #[test]
    fn a_torch_glows_around_its_flame_at_night_only() {
        let night = torch_on_page(&AT_NIGHT);
        let shade = |c: &Canvas, x: usize, y: usize| c.px[y * c.w + x];
        let page = rgb(AT_NIGHT.page);
        assert_eq!(shade(&night, 5, 5), AT_NIGHT.flame);
        for (x, y) in [
            (4, 5),
            (6, 5),
            (5, 4),
            (4, 4),
            (6, 4),
            (3, 4),
            (7, 4),
            (4, 3),
            (5, 3),
        ] {
            let warm = shade(&night, x, y);
            assert!(
                warm[0] > page[0] && warm[0] - warm[2] > page[0] - page[2],
                "({x}, {y}) is {warm:?}"
            );
        }
        for (left, right) in [((4, 5), (6, 5)), ((4, 4), (6, 4)), ((3, 6), (7, 6))] {
            assert_eq!(
                shade(&night, left.0, left.1),
                shade(&night, right.0, right.1)
            );
        }
        for (x, y) in [(2, 5), (8, 5), (3, 3), (7, 3), (5, 2)] {
            assert_eq!(shade(&night, x, y), page, "({x}, {y})");
        }
        let day = torch_on_page(&BY_DAY);
        let lit: Vec<(usize, usize)> = (0..121)
            .map(|i| (i % 11, i / 11))
            .filter(|&(x, y)| shade(&day, x, y) != rgb(BY_DAY.page))
            .collect();
        assert_eq!(lit, [(5, 5), (5, 6), (5, 7), (5, 8)]);
    }

    #[test]
    fn a_window_too_short_for_the_setting_sun_drops_it() {
        for (width, height) in [SMALLEST, (1180., 520.)] {
            for night in [NIGHT, DAWN] {
                let look = Look::of(&night);
                let p = window(width, height, look);
                assert!(moon_rows(&p, look).is_empty(), "{width}x{height}");
                for x in 0..p.cols {
                    assert!(
                        light(at(&p, x, p.rows - 1)) < 450,
                        "{width}x{height}: column {x}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_diamond_ring_glints_only_off_the_moon() {
        let (cols, rows) = (384, 216);
        let (sx, sy, r) = (330, (rows as f32 * SUN_HIGH).round() as i32, 12.);
        assert!(r >= BEAD_SPARKLE_FROM);
        let sun = Sun::Beside { x: sx, r: r as i32 };
        for night in [NIGHT, DAWN] {
            let look = Look::of(&night);
            let p = paint(cols, rows, stage(1920., 1080., 5., home(1080.)), sun, look);
            let palette = look.palette();
            let ends: Vec<[u8; 3]> = [Some(palette.moon), palette.bead]
                .into_iter()
                .flatten()
                .map(colour)
                .collect();
            let reach = r as i32;
            for dy in -reach..=reach {
                for dx in -reach..=reach {
                    if inside(r, dx, dy) {
                        let here = at(&p, (sx + dx) as usize, (sy + dy) as usize);
                        assert!(
                            ends.iter().any(|c| here[..3] == *c),
                            "dark {}: a glint on the moon at ({dx}, {dy}): {here:?}",
                            look.dark
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn clouds_drift_in_groups_never_alone() {
        let day = Look::of(&DAWN);
        let wide = (1500..=3440)
            .step_by(80)
            .flat_map(|width| [900., 1080., 1200., 1440.].map(|height| (width as f32, height)));
        for (width, height) in SIZES.into_iter().chain(wide) {
            let stage = stage(width, height, texel(width as f64, 1.), home(height));
            let p = window(width, height, day);
            let white = |x: i32, y: i32| {
                let (here, sky) = (at(&p, x as usize, y as usize), at(&p, 0, y as usize));
                (0..3).all(|i| here[i] > sky[i])
            };
            let (left, right) = stage.column;
            let gutters = [
                ("left", stage.sidebar, left),
                ("right", right + 1, p.cols as i32),
            ];
            for (side, from, to) in gutters {
                let mut tops = 0;
                for y in 1..stage.skyline as i32 {
                    let mut x = from;
                    while x < to {
                        let start = x;
                        while x < to && white(x, y) {
                            x += 1;
                        }
                        let above = start - 1..=x.min(to - 1);
                        if x > start && !above.into_iter().any(|a| white(a, y - 1)) {
                            tops += 1;
                        }
                        x += 1;
                    }
                }
                assert!(
                    tops == 0 || tops >= CLOUD_GROUP,
                    "{width}x{height}: {tops} clouds in the {side} gutter"
                );
                if (width, height, side) == (1920., 1080., "left") {
                    assert!(
                        tops >= CLOUD_GROUP,
                        "1920x1080: no clouds beside the sidebar"
                    );
                }
            }
        }
    }

    #[test]
    fn no_star_shows_over_the_content() {
        use crate::layout::{content_width, sidebar};
        let look = Look::of(&NIGHT);
        for (width, height) in [SMALLEST].into_iter().chain(SIZES) {
            let column = content_width(width);
            let side = sidebar(width);
            let left = side + (width - side - column) / 2.;
            for content_bottom in [home(height) - 28., height / 2., height + 400.] {
                let key = key(
                    width as f64,
                    height as f64,
                    1.,
                    look,
                    home(height),
                    content_bottom,
                );
                let baked = Baked::new(width as f64, height as f64, 1., key);
                let t = baked.stage.texel;
                for s in baked.shown(content_bottom) {
                    let (x, y) = (s.x * t, s.y * t);
                    let beside = x <= left - TEXT_CLEARANCE || x >= left + column + TEXT_CLEARANCE;
                    assert!(
                        beside || y >= content_bottom + TEXT_CLEARANCE,
                        "{width}x{height} under content to {content_bottom}: a star at ({x}, {y})"
                    );
                }
            }
        }
    }
}
