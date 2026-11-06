use crate::art;
use crate::sky::{motion, Clock, Motion};
use gpui::{
    canvas, div, fill, img, point, prelude::*, px, size, Bounds, Context, Pixels, RenderImage,
    Window,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

type Rgb = [f32; 3];

const SKY_STOPS: [(f32, Rgb); 7] = [
    (0., [5., 7., 14.]),
    (0.4, [9., 12., 26.]),
    (0.66, [16., 19., 42.]),
    (0.82, [28., 27., 58.]),
    (0.92, [52., 38., 62.]),
    (0.97, [98., 60., 54.]),
    (1., [150., 92., 48.]),
];
const SKY_BAND: usize = 3;
const SUN_SIZE: f32 = 0.055;
const SUN_LOW: (f32, Option<f32>, f32) = (0.84, None, 0.7);
type Ridge = (u32, (Rgb, Rgb), f32, f32, f32);
const RIDGES: [Ridge; 2] = [(3, FAR, 0.86, 0.3, 70.), (8, MID, 0.92, 0.2, 46.)];
const SUN_HIGH: f32 = 0.38;
const STAR_BRIGHT: Rgb = [176., 182., 222.];
const STAR_MID: Rgb = [104., 110., 160.];
const STAR_DIM: Rgb = [62., 67., 108.];
const CORONA: Rgb = [255., 176., 72.];
const STREAMER: Rgb = [255., 196., 110.];
const RIM: Rgb = [255., 226., 170.];
const BEAD: Rgb = [255., 248., 230.];
const MOON: Rgb = [7., 8., 13.];
const FAR: (Rgb, Rgb) = ([30., 32., 62.], [64., 56., 86.]);
const MID: (Rgb, Rgb) = ([14., 16., 32.], [14., 16., 32.]);
const GRASS: Rgb = [22., 40., 30.];
const GRASS_LIT: Rgb = [30., 54., 38.];
const DIRT: Rgb = [24., 19., 16.];
const DIRT_DEEP: Rgb = [17., 14., 12.];
const STONE: Rgb = [28., 28., 32.];
const TRUNK: Rgb = [34., 24., 18.];
const LEAVES: Rgb = [14., 30., 22.];
const LEAVES_SHADE: Rgb = [10., 22., 16.];
const LEAVES_LIT: Rgb = [22., 44., 30.];
const STICK: Rgb = [60., 44., 30.];
const FLAME: Rgb = [255., 200., 90.];
const GLOW: Rgb = [255., 179., 71.];

const TICK: Duration = Duration::from_millis(250);
const UNFOCUSED: Duration = Duration::from_secs(10);

fn texel(width: f64) -> f32 {
    if width < 760. {
        3.
    } else if width < 1800. {
        4.
    } else {
        5.
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
    ridged * 0.62
        + smooth(seed + 1, x / (span / 2.6)) * 0.26
        + smooth(seed + 2, x / (span / 7.)) * 0.12
}

fn rolling(seed: u32, x: f32, span: f32) -> f32 {
    smooth(seed, x / span) * 0.7 + smooth(seed + 1, x / (span / 3.)) * 0.3
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
        for yy in y..y + h {
            for xx in x..x + w {
                self.set(xx, yy, c);
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

fn sky(position: f32) -> Rgb {
    let next = SKY_STOPS
        .iter()
        .position(|(stop, _)| *stop >= position)
        .unwrap_or(SKY_STOPS.len() - 1);
    if next == 0 {
        return SKY_STOPS[0].1;
    }
    let (a, from) = SKY_STOPS[next - 1];
    let (b, to) = SKY_STOPS[next];
    let t = (position - a) / (b - a);
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

fn sun_at(width: f32, height: f32) -> (f32, Option<f32>, f32) {
    let avail = width - crate::frame::SIDEBAR;
    let column = (avail - 2. * crate::theme::PAGE_GUTTER).min(crate::theme::PAGE_MAX);
    let gutter = (avail - column) / 2.;
    let reach = width.min(height) * SUN_SIZE * 2.4;
    let fit = ((gutter - 48.) / (2. * reach)).min(1.);
    if fit >= 0.6 {
        ((width - gutter / 2.) / width, Some(SUN_HIGH), fit)
    } else {
        SUN_LOW
    }
}

fn ridge_top(seed: u32, base: f32, rise: f32, span: f32, x: i32, h: f32) -> i32 {
    let shape = peaks(seed, x as f32, span);
    (((h * base - shape * h * rise) / 2.).round() * 2.) as i32
}

fn paint(cols: usize, rows: usize, sun: (f32, Option<f32>, f32)) -> Picture {
    let mut c = Canvas {
        w: cols,
        h: rows,
        px: vec![[0.; 3]; cols * rows],
    };
    let (w, h) = (cols as f32, rows as f32);
    let horizon = (h * 0.8).round();

    for y in (0..rows).step_by(SKY_BAND) {
        let colour = sky((y as f32 / horizon).min(1.));
        c.rect(0, y as i32, cols as i32, SKY_BAND as i32, colour);
    }

    let r = (w.min(h) * SUN_SIZE * sun.2).round().max(4.);
    let (sx, sy) = match sun.1 {
        Some(y) => ((w * sun.0).round() as i32, (h * y).round() as i32),
        None => {
            let skyline = |x: i32| {
                RIDGES
                    .iter()
                    .map(|&(seed, _, base, rise, span)| ridge_top(seed, base, rise, span, x, h))
                    .min()
                    .unwrap_or(rows as i32)
            };
            let from = (w * (sun.0 - 0.3)) as i32;
            let to = (w * (sun.0 + 0.1)).min(w - r * 2.) as i32;
            let x = (from..=to.max(from))
                .step_by(2)
                .max_by_key(|&x| skyline(x))
                .unwrap_or(from);
            (x, skyline(x) - (r * 0.8).round() as i32)
        }
    };
    let mut rng = Rng(11);
    let mut stars = Vec::new();
    for _ in 0..((w * horizon) / 700.).round() as usize {
        let x = rng.next() * w;
        let y = rng.next().powf(1.7) * horizon * 0.72;
        let dim = rng.next();
        let twinkles = rng.next() < 0.18;
        let speed = 0.6 + rng.next() * 1.4;
        let phase = rng.next() * std::f32::consts::TAU;
        let near = ((x - sx as f32).powi(2) + (y - sy as f32).powi(2)).sqrt() < r * 2.6;
        if near {
            continue;
        }
        let colour = if dim > 0.9 {
            STAR_BRIGHT
        } else if dim > 0.55 {
            STAR_MID
        } else {
            STAR_DIM
        };
        stars.push(Star {
            x,
            y,
            colour,
            speed: if twinkles && dim > 0.55 { speed } else { 0. },
            phase,
        });
    }

    for ring in (1..=4).rev() {
        c.disc(sx, sy, r + ring as f32 * (r * 0.26).ceil(), CORONA, 0.045);
    }
    let mut rays = Rng(37);
    for i in 0..12 {
        let angle = (i as f32 + rays.next() * 0.3) * std::f32::consts::TAU / 12.;
        let len = r * if i % 2 == 0 { 1. } else { 0.55 } * (0.9 + rays.next() * 0.2);
        let (dx, dy) = (angle.cos(), angle.sin());
        let mut d = r;
        while d < r + len {
            let fade = 1. - (d - r) / len;
            c.blend(
                sx + (dx * d).round() as i32,
                sy + (dy * d).round() as i32,
                STREAMER,
                0.28 * fade,
            );
            d += 0.5;
        }
    }
    c.disc(sx, sy, r + 1., RIM, 1.);
    c.disc(sx, sy, r, MOON, 1.);
    let bead = std::f32::consts::FRAC_PI_4 * 5.;
    let (bx, by) = (
        sx + (bead.cos() * r).round() as i32,
        sy + (bead.sin() * r).round() as i32,
    );
    c.rect(bx - 1, by - 1, 2, 2, BEAD);
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
            BEAD,
            0.35,
        );
    }

    for (seed, (rock, rim), base, rise, span) in RIDGES {
        for x in (0..cols as i32).step_by(2) {
            let top = ridge_top(seed, base, rise, span, x, h);
            c.rect(x, top, 2, rows as i32 - top, rock);
            c.rect(x, top, 2, 1, rim);
        }
    }

    let ground_base = h * 0.975;
    let ground_rise = h * 0.08;
    let ground = |x: i32| -> i32 {
        let block = (x.div_euclid(4) * 4) as f32;
        let y = ground_base - rolling(91, block, 90.) * ground_rise;
        ((y / 2.).round() * 2.) as i32
    };
    let mut soil = Rng(53);
    for x in 0..cols as i32 {
        let top = ground(x);
        c.rect(x, top, 1, 2, GRASS);
        if soil.next() < 0.18 {
            c.set(x, top, GRASS_LIT);
        }
        c.rect(x, top + 2, 1, 4, DIRT);
        c.rect(x, top + 6, 1, rows as i32 - top - 6, DIRT_DEEP);
        if soil.next() < 0.05 {
            c.set(x, top + 4 + (soil.next() * 8.) as i32, STONE);
        }
    }

    let torches = [(w * 0.34) as i32, (w * 0.63) as i32];
    let mut trees = Rng(61);
    for _ in 0..(w / 22.).round() as usize {
        let x = (trees.next() * w) as i32;
        let tall = 3 + (trees.next() * 3.) as i32;
        let wide = if trees.next() < 0.5 { 2 } else { 3 };
        if torches.iter().any(|t| (x - t).abs() < wide + 4) {
            continue;
        }
        let foot = ground(x);
        c.rect(x, foot - tall, 1, tall, TRUNK);
        let (top, height) = (foot - tall - wide * 2 + 1, wide * 2);
        for yy in 0..height {
            let inset = (yy == 0 || yy == height - 1) as i32;
            for xx in -wide + inset..=wide - inset {
                let colour = if xx > 0 {
                    LEAVES_SHADE
                } else if trees.next() < 0.12 {
                    LEAVES_LIT
                } else {
                    LEAVES
                };
                c.set(x + xx, top + yy, colour);
            }
        }
    }

    for x in torches {
        let foot = ground(x);
        for ring in (1..=2).rev() {
            c.disc(x, foot - 4, 1.5 + ring as f32, GLOW, 0.1);
        }
        c.rect(x, foot - 3, 1, 3, STICK);
        c.set(x, foot - 4, FLAME);
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

struct Baked {
    key: (u32, u32, u32),
    image: Arc<RenderImage>,
    texel: f32,
    cols: usize,
    rows: usize,
    stars: Vec<Star>,
}

impl Baked {
    fn new(width: f64, height: f64, scale: f32) -> Baked {
        let t = texel(width);
        let cols = (width as f32 / t).ceil().max(1.) as usize;
        let rows = (height as f32 / t).ceil().max(1.) as usize;
        let picture = paint(cols, rows, sun_at(width as f32, height as f32));
        Baked {
            key: key(width, height, scale),
            image: art::bake(&picture.texels, picture.cols, picture.rows, t, scale),
            texel: t,
            cols: picture.cols,
            rows: picture.rows,
            stars: picture.stars,
        }
    }
}

fn key(width: f64, height: f64, scale: f32) -> (u32, u32, u32) {
    (
        width.round() as u32,
        height.round() as u32,
        (scale * 100.).round() as u32,
    )
}

pub struct Scene {
    baked: Option<Baked>,
    replaced: Vec<Arc<RenderImage>>,
    clock: Clock,
    unfocused: Option<Instant>,
    waking: bool,
}

impl Scene {
    pub fn new() -> Scene {
        Scene {
            baked: None,
            replaced: Vec::new(),
            clock: Clock::default(),
            unfocused: None,
            waking: false,
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
        if self.baked.as_ref().map(|b| b.key) != Some(key(width, height, scale)) {
            if let Some(old) = self.baked.replace(Baked::new(width, height, scale)) {
                self.replaced.push(old.image);
            }
        }

        let now = Instant::now();
        if window.is_window_active() {
            self.unfocused = None;
        } else {
            self.unfocused.get_or_insert(now);
        }
        let run = match motion() {
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
        let dot = (t * 0.5).max(1.5);
        let stars: Vec<(Bounds<Pixels>, gpui::Hsla)> = baked
            .stars
            .iter()
            .map(|s| {
                let a = if s.speed > 0. {
                    0.35 + 0.65 * (0.5 + 0.5 * (time * s.speed + s.phase).sin())
                } else {
                    1.
                };
                let [r, g, b] = s.colour.map(|v| v / 255.);
                (
                    Bounds::new(
                        point(px((s.x * t).round()), px((s.y * t).round())),
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
            .bg(gpui::rgb(eclipse_theme::glass::PAGE))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn at(p: &Picture, x: usize, y: usize) -> [u8; 4] {
        p.texels[y * p.cols + x]
    }

    fn light(c: [u8; 4]) -> u32 {
        c[0] as u32 + c[1] as u32 + c[2] as u32
    }

    #[test]
    fn the_moon_covers_the_sun_and_the_corona_shows_around_it() {
        let p = paint(295, 190, (0.86, Some(0.38), 1.));
        let (sx, sy) = (
            (295. * 0.86f32).round() as usize,
            (190. * 0.38f32).round() as usize,
        );
        let r = (190. * SUN_SIZE).round() as usize;
        assert!(light(at(&p, sx, sy)) < 40);
        assert!(light(at(&p, sx + r + 2, sy)) > light(at(&p, sx + r * 4, sy)));
    }

    #[test]
    fn the_ground_reaches_the_bottom_of_every_column() {
        let p = paint(240, 150, SUN_LOW);
        for x in 0..p.cols {
            assert!(light(at(&p, x, p.rows - 1)) < 150, "column {x}");
        }
    }

    #[test]
    fn a_size_always_paints_the_same_picture() {
        let (a, b) = (paint(180, 120, SUN_LOW), paint(180, 120, SUN_LOW));
        assert!(a.texels == b.texels);
        assert_eq!(a.stars.len(), b.stars.len());
    }
}
