//! The pictures Home draws from the player's disk (screenshots, their blurred copies, the
//! background), made in the background at the size they are drawn and kept under a byte budget
//! (`pictures`).
//!
//! GPUI's `img(path)` keeps every picture it ever drew, at full size, until the launcher quits,
//! in RAM and in its sprite atlas: one 32768 × 16384 screenshot is 2 GiB, and Home drew the
//! three newest as soon as it opened (24 GiB reported on Linux, 2026-11-05).
use crate::pictures::{self, Lru};
use crate::{art, Launcher};
use gpui::{Context, RenderImage, Window};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A screenshot tile's widest, in points; made at this times the screen's scale.
const THUMB_POINTS: f32 = 360.;
/// What the made pictures may take together, unless more than that is on screen at once.
const BUDGET: u64 = 384 << 20;
/// A picture drawn this recently is on screen: it stays even over the budget.
const GRACE: Duration = Duration::from_secs(2);
/// Pictures made at the same time; the others wait for the next frame.
const AT_ONCE: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Look {
    /// Fitted inside this many pixels square.
    Fit(u32),
    /// Small and blurred (`home_screenshots.rs`).
    Blurred,
}

type Key = (PathBuf, Look);

pub(crate) struct Pictures {
    /// None: the file couldn't be read as a picture.
    made: Lru<Key, Option<Arc<RenderImage>>>,
    making: HashSet<Key>,
    /// Gone from `made`; freed from the window's sprite atlas on the next frame.
    retired: Vec<Arc<RenderImage>>,
}

impl Default for Pictures {
    fn default() -> Self {
        Pictures {
            made: Lru::new(BUDGET, GRACE),
            making: HashSet::new(),
            retired: Vec::new(),
        }
    }
}

impl Pictures {
    fn retire(&mut self, gone: Vec<Option<Arc<RenderImage>>>) {
        self.retired.extend(gone.into_iter().flatten());
    }

    pub(crate) fn made(&self, path: &Path, look: Look) -> Option<Arc<RenderImage>> {
        self.made
            .peek(&(path.to_path_buf(), look))
            .cloned()
            .flatten()
    }

    /// Frees the pictures dropped since the last frame where GPUI keeps them too.
    pub(crate) fn free_retired(&mut self, window: &mut Window) {
        for image in self.retired.drain(..) {
            let _ = window.drop_image(image);
        }
    }
}

impl Launcher {
    /// The side to make a screenshot tile at, for this screen.
    pub(crate) fn thumb_side(&self) -> u32 {
        pictures::drawn_side(THUMB_POINTS, self.scale, pictures::gpu_limit())
    }

    /// The side to make a picture filling the window at (the viewer, the background): every
    /// pixel the screen can show, even on an 8K one, within the GPU's limit.
    pub(crate) fn window_side(&self) -> u32 {
        pictures::drawn_side(self.window_long_side, self.scale, pictures::gpu_limit())
    }

    /// The picture at `path` as `look`; None while it is being made, or when it can't be read
    /// (draw a plain fill then).
    pub(crate) fn picture(
        &mut self,
        path: &Path,
        look: Look,
        cx: &mut Context<Self>,
    ) -> Option<Arc<RenderImage>> {
        let now = Instant::now();
        let key = (path.to_path_buf(), look);
        if let Some(made) = self.pictures.made.get(&key, now).cloned() {
            if self.pictures.made.over_budget() {
                let gone = self.pictures.made.trim(now);
                self.pictures.retire(gone);
            }
            return made;
        }
        if self.pictures.making.len() < AT_ONCE && self.pictures.making.insert(key.clone()) {
            let task = cx.background_executor().spawn(async move {
                let image = make(&key.0, look);
                (key, image)
            });
            cx.spawn(async move |this, cx| {
                let (key, image) = task.await;
                let _ = this.update(cx, |this, cx| {
                    this.pictures.making.remove(&key);
                    let bytes = image
                        .as_ref()
                        .and_then(|i| i.as_bytes(0))
                        .map_or(0, |b| b.len() as u64);
                    let gone = this.pictures.made.insert(key, image, bytes, Instant::now());
                    this.pictures.retire(gone);
                    cx.notify();
                });
            })
            .detach();
        }
        None
    }
}

/// The blurred copy is made this wide (and 16:9 tall), its blur this many of its pixels: about
/// 40 px over the tiles (163 to 335 px wide).
const SMALL: (u32, u32) = (96, 54);
const SIGMA: f32 = 15.;
/// Then smoothed up to this, so the stretched picture shows no texels.
const SHOWN: (u32, u32) = (288, 162);

/// Makes the picture as GPUI draws it. Runs off the UI thread.
fn make(path: &Path, look: Look) -> Option<Arc<RenderImage>> {
    match look {
        Look::Blurred => blurred(path),
        Look::Fit(max) => {
            // Never above the GPU's limit, whatever the caller asked: GPUI's atlas can't hold it.
            let image = pictures::load_fitted(path, max.min(pictures::gpu_limit())).ok()?;
            Some(art::gpu_image(image))
        }
    }
}

/// The picture at `path`, small, blurred and smoothed up, as GPUI draws it.
/// Read already shrunk (`pictures::load_fitted`): never the whole screenshot.
fn blurred(path: &Path) -> Option<Arc<RenderImage>> {
    let picture =
        image::DynamicImage::ImageRgba8(crate::pictures::load_fitted(path, SMALL.0 * 4).ok()?);
    let small = picture
        .resize_to_fill(SMALL.0, SMALL.1, image::imageops::FilterType::Triangle)
        .to_rgba8();
    Some(art::gpu_image(soften(&small)))
}

/// `small` blurred as `blur-2xl` would at its size, then smoothed up to [`SHOWN`].
fn soften(small: &image::RgbaImage) -> image::RgbaImage {
    let soft = image::imageops::blur(small, SIGMA);
    image::imageops::resize(
        &soft,
        SHOWN.0,
        SHOWN.1,
        image::imageops::FilterType::Triangle,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blurred_copy_keeps_the_colours_but_not_the_edges() {
        // Left half black, right half white: blurred, the middle turns grey, the far sides
        // keep most of their colour.
        let small = image::RgbaImage::from_fn(SMALL.0, SMALL.1, |x, _| {
            let v = if x < SMALL.0 / 2 { 0 } else { 255 };
            image::Rgba([v, v, v, 255])
        });
        let soft = soften(&small);
        assert_eq!(soft.dimensions(), SHOWN);
        let at = |x: u32| soft.get_pixel(x, SHOWN.1 / 2).0[0] as i32;
        assert!((at(SHOWN.0 / 2) - 128).abs() < 30, "{}", at(SHOWN.0 / 2));
        assert!(
            at(0) < 60 && at(SHOWN.0 - 1) > 195,
            "{} {}",
            at(0),
            at(SHOWN.0 - 1)
        );
    }
}
