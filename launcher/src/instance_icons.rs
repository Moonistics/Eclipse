//! What an instance's icon shows (plan R8): its own picture (one the player chose, or its
//! modpack's icon) first; else the block or colour chosen for it; else its mark: the loader
//! it plays with, the grass block for plain Minecraft, Eclipse's own for Eclipse. The letter tile
//! is the last resort. Each is decoded once per size and kept in `Instances::icons`.
use super::*;
use eclipse_launcher::browser::images;
use eclipse_launcher::instances::{self, Instance, Kind as InstanceKind};
use eclipse_theme::mark;
use gpui::RenderImage;
use std::sync::Arc;

/// The loaders' logos, each its project's own: see THIRD_PARTY_NOTICES.md.
const FABRIC: &[u8] = include_bytes!("../assets/logos/fabric.png");
const QUILT: &[u8] = include_bytes!("../assets/logos/quilt.png");
const NEOFORGE: &[u8] = include_bytes!("../assets/logos/neoforge.png");
const FORGE: &[u8] = include_bytes!("../assets/logos/forge.png");
/// Vanilla's mark: our own grass block (`tools/launcher_grass_block.py`), not Mojang's art.
pub(super) const GRASS: &[u8] = include_bytes!("../assets/logos/grass-block.png");
/// The loaders a logo is drawn for.
const LOADERS: [&str; 4] = ["fabric", "quilt", "forge", "neoforge"];
const SMALL_MARK: [&str; 12] = [
    "....oooo....",
    "...occcco...",
    "..ocrrrrco..",
    ".ocbmmmmrco.",
    "ocrmmmmmmrco",
    "ocrmmmmmmrco",
    "ocrmmmmmmrco",
    "ocrmmmmmmrco",
    ".ocrmmmmrco.",
    "..ocrrrrco..",
    "...occcco...",
    "....oooo....",
];
const MODDED_GAP: f32 = 2.;
const PIXEL_ART_MAX: u32 = 256;
const SHARP_FILL: f32 = 0.75;

pub(super) fn has_plate(loader: &str) -> bool {
    matches!(loader, "quilt" | "neoforge" | "forge")
}

impl Launcher {
    /// The instance's own picture, `size` points square. Whether there is one is asked once:
    /// `reload_instances` forgets the answer.
    pub(super) fn instance_picture(
        &mut self,
        instance: &Instance,
        size: f32,
    ) -> Option<Arc<RenderImage>> {
        let file = instance.home.join(instances::PICTURE);
        let key = format!("picture:{}", instance.id);
        self.icon_image(key, size, true, || std::fs::read(file).ok())
    }

    /// What an instance shows when nothing was chosen for it.
    pub(super) fn instance_mark(
        &mut self,
        instance: &Instance,
        size: f32,
    ) -> Option<Arc<RenderImage>> {
        let loader = match instance.kind {
            InstanceKind::Port => {
                let slug = &instance.id["port:".len()..];
                let pack = self.packs.iter().find(|p| p.slug == slug);
                pack.map(|p| p.pack.loader.to_ascii_lowercase())
            }
            _ => instance
                .java
                .as_ref()
                .and_then(|j| j.loader.as_ref())
                .map(|l| l.kind.id().to_string()),
        };
        if let Some(loader) = loader.filter(|l| LOADERS.contains(&l.as_str())) {
            return self.loader_logo(&loader, size);
        }
        if instance.java.is_some() {
            return self.grass_mark(size);
        }
        self.mark_image(size)
    }

    /// Vanilla's grass block, `size` points square.
    pub(super) fn grass_mark(&mut self, size: f32) -> Option<Arc<RenderImage>> {
        self.icon_image("mark:grass".into(), size, false, || Some(GRASS.to_vec()))
    }

    /// Eclipse's own mark, `size` points square.
    pub(super) fn eclipse_mark(&mut self, size: f32) -> AnyElement {
        match self.mark_image(size) {
            Some(image) => centred(image, size, self.scale).into_any_element(),
            None => div().size(px(size)).flex_none().into_any_element(),
        }
    }

    fn mark_image(&mut self, size: f32) -> Option<Arc<RenderImage>> {
        let scale = self.scale;
        self.instances
            .icons
            .entry(("mark:eclipse".into(), (size * scale) as u32))
            .or_insert_with(|| Some(pixel_mark(size, scale)))
            .clone()
    }

    pub(super) fn modded_mark(&mut self, size: f32) -> AnyElement {
        let cell = ((size - MODDED_GAP) / 2.).floor();
        let [fabric, quilt, forge, neoforge] = LOADERS.map(|loader| self.loader_logo(loader, cell));
        let scale = self.scale;
        let row = |left: Option<Arc<RenderImage>>, right: Option<Arc<RenderImage>>| {
            div()
                .flex()
                .gap(px(MODDED_GAP))
                .children(left.map(|image| centred(image, cell, scale)))
                .children(right.map(|image| centred(image, cell, scale)))
        };
        div()
            .flex()
            .flex_col()
            .flex_none()
            .justify_center()
            .gap(px(MODDED_GAP))
            .size(px(size))
            .child(row(fabric, quilt))
            .child(row(neoforge, forge))
            .into_any_element()
    }

    /// A mod loader's logo by its id (`fabric`, `quilt`, `forge`, `neoforge`): None for another
    /// id.
    pub(super) fn loader_logo(&mut self, loader: &str, size: f32) -> Option<Arc<RenderImage>> {
        let png = logo_png(loader)?;
        self.icon_image(format!("mark:{loader}"), size, false, || Some(png.to_vec()))
    }

    /// The image under `key` at `size` points, made from `bytes` the first time. A missing one is
    /// remembered only when `remember_missing` (a file that may never exist); else it is asked
    /// again next frame (a logo still downloading).
    pub(super) fn icon_image(
        &mut self,
        key: String,
        size: f32,
        remember_missing: bool,
        bytes: impl FnOnce() -> Option<Vec<u8>>,
    ) -> Option<Arc<RenderImage>> {
        let key = (key, (size * self.scale) as u32);
        if let Some(image) = self.instances.icons.get(&key) {
            return image.clone();
        }
        let scale = self.scale;
        let image = bytes().and_then(|b| sharp(&b, size, scale).or_else(|| fitted(b, size, scale)));
        if image.is_some() || remember_missing {
            self.instances.icons.insert(key, image.clone());
        }
        image
    }
}

/// An image file's bytes drawn at most `size` points square: scaled down smoothly when larger,
/// up by whole device pixels when smaller, so pixel-art logos stay sharp.
fn fitted(bytes: Vec<u8>, size: f32, scale: f32) -> Option<Arc<RenderImage>> {
    let side = (size * scale).round().max(1.) as u32;
    let png = images::fit_png(bytes, side, 4096, 64 << 20)?;
    let image = image::load_from_memory(&png).ok()?.to_rgba8();
    let (w, h) = (image.width() as usize, image.height() as usize);
    let texels: Vec<[u8; 4]> = image.pixels().map(|p| p.0).collect();
    let whole = (side as usize / w.max(h)).max(1) as f32;
    Some(art::bake(&texels, w, h, whole / scale, scale))
}

fn sharp(bytes: &[u8], size: f32, scale: f32) -> Option<Arc<RenderImage>> {
    let reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let (w, h) = reader.into_dimensions().ok()?;
    if w.max(h) > PIXEL_ART_MAX {
        return None;
    }
    let image = image::load_from_memory(bytes).ok()?.to_rgba8();
    let block = (2..=w.min(h)).rev().find(|&block| {
        w % block == 0
            && h % block == 0
            && image
                .enumerate_pixels()
                .all(|(x, y, p)| p == image.get_pixel(x / block * block, y / block * block))
    })?;
    let (cols, rows) = ((w / block) as usize, (h / block) as usize);
    let side = (size * scale).round() as usize;
    let whole = side / cols.max(rows);
    if whole == 0 || ((whole * cols.max(rows)) as f32) < side as f32 * SHARP_FILL {
        return None;
    }
    let (left, top) = ((side - whole * cols) / 2, (side - whole * rows) / 2);
    let mut pixels = vec![[0; 4]; side * side];
    for y in 0..whole * rows {
        for x in 0..whole * cols {
            let texel = image.get_pixel((x / whole) as u32 * block, (y / whole) as u32 * block);
            pixels[(top + y) * side + left + x] = texel.0;
        }
    }
    Some(art::bake(&pixels, side, side, 1. / scale, scale))
}

fn pixel_mark(size: f32, scale: f32) -> Arc<RenderImage> {
    let side = (size * scale).round();
    let rows: &[&str] = if side < 2. * mark::PIXELS.len() as f32 {
        &SMALL_MARK
    } else {
        &mark::PIXELS
    };
    let cell = (side / rows.len() as f32).floor().max(1.);
    let texels: Vec<[u8; 4]> = rows
        .iter()
        .flat_map(|row| row.bytes())
        .map(|c| match mark::colour(c) {
            Some(rgb) => [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, 255],
            None => [0; 4],
        })
        .collect();
    art::bake(&texels, rows.len(), rows.len(), cell / scale, scale)
}

/// A loader's logo file by its id.
pub(super) fn logo_png(loader: &str) -> Option<&'static [u8]> {
    match loader {
        "fabric" => Some(FABRIC),
        "quilt" => Some(QUILT),
        "neoforge" => Some(NEOFORGE),
        "forge" => Some(FORGE),
        _ => None,
    }
}

/// An image file's bytes as a picture chosen on the New instance page: a loader's logo, Minecraft's
/// grass block.
pub(super) fn chosen(bytes: Vec<u8>) -> Option<Chosen> {
    let png = instances::picture(bytes).ok()?;
    Some(Chosen { png })
}

/// An image made by [`fitted`], centred in a `size` points square.
pub(super) fn centred(image: Arc<RenderImage>, size: f32, scale: f32) -> Div {
    let dim = image.size(0);
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(size))
        .child(
            gpui::img(image)
                .w(px(dim.width.0 as f32 / scale))
                .h(px(dim.height.0 as f32 / scale)),
        )
}

/// A picture chosen on the New instance page, written on Save.
#[derive(Clone)]
pub(super) struct Chosen {
    pub(super) png: Vec<u8>,
}

/// The most a chosen file may weigh: a photo, not a film.
const MAX_PICTURE_FILE: u64 = 64 << 20;

impl Launcher {
    /// Asks for an image file and makes it the page's picture, read and scaled off the UI
    /// thread.
    pub(super) fn choose_instance_picture(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose".into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let made = cx
                .background_executor()
                .spawn(async move {
                    let too_big = std::fs::metadata(&path).map_err(|e| e.to_string())?.len()
                        > MAX_PICTURE_FILE;
                    let bytes = (!too_big)
                        .then(|| std::fs::read(&path).ok())
                        .flatten()
                        .ok_or_else(|| format!("{}: too big, or unreadable", path.display()))?;
                    Ok::<_, String>(Chosen {
                        png: instances::picture(bytes)?,
                    })
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if let Some(d) = &mut this.instances.draft {
                    match made {
                        Ok(chosen) => {
                            d.picture = Some(Some(chosen));
                            d.logo = None;
                            d.error = None;
                        }
                        Err(e) => d.error = Some(e),
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn side(image: &RenderImage) -> (i32, i32) {
        let size = image.size(0);
        (size.width.0, size.height.0)
    }

    #[test]
    fn a_pixel_art_logo_fills_its_slot_in_whole_texels() {
        for (size, scale, texel, margin) in [
            (32., 1., 2, 0),
            (32., 1.5, 3, 0),
            (36., 1., 2, 2),
            (36., 1.5, 3, 3),
            (48., 1., 3, 0),
            (16., 2., 2, 0),
        ] {
            let image = sharp(FABRIC, size, scale).expect("Fabric's logo is pixel art");
            let edge = (size * scale) as usize;
            assert_eq!(
                side(&image),
                (edge as i32, edge as i32),
                "{size} at {scale}x"
            );
            let bytes = image.as_bytes(0).expect("one frame");
            let at = |x: usize, y: usize| &bytes[(y * edge + x) * 4..][..4];
            let block = |v: usize| margin + (v - margin) / texel * texel;
            for y in margin..edge - margin {
                for x in margin..edge - margin {
                    assert_eq!(
                        at(x, y),
                        at(block(x), block(y)),
                        "{size} at {scale}x: ({x}, {y})"
                    );
                }
            }
            let middle = edge / 2;
            assert!((0..margin).all(|v| at(v, middle)[3] == 0 && at(middle, v)[3] == 0));
        }
    }

    #[test]
    fn a_logo_too_small_for_whole_texels_or_not_pixel_art_is_scaled_smoothly() {
        assert!(sharp(FABRIC, 24., 1.).is_none());
        assert!(sharp(FABRIC, 12., 1.).is_none());
        for logo in [GRASS, FORGE, QUILT, NEOFORGE] {
            assert!(sharp(logo, 32., 1.).is_none());
        }
        let smooth = fitted(GRASS.to_vec(), 32., 1.).expect("the grass block decodes");
        assert_eq!(side(&smooth), (32, 32));
    }
}
