//! What an instance's icon shows: its own picture (one the player chose, or its
//! modpack's icon) first; else the block or colour chosen for it; else its mark: the loader
//! it plays with, the grass block for plain Minecraft, Eclipse's own for Eclipse. The letter tile
//! is the last resort. Each is decoded once per size and kept in `Instances::icons`.
use crate::{art, instance_state, ui, Launcher, Pick};
use eclipse_launcher::browser::images;
use eclipse_launcher::i18n::tr;
use eclipse_launcher::instances::{self, Instance, Kind as InstanceKind};
use eclipse_launcher::loaders::Kind as Loader;
use eclipse_theme::mark;
use gpui::prelude::*;
use gpui::{div, px, AnyElement, Div, RenderImage};
use std::sync::Arc;

/// The loaders' logos, each its project's own: see THIRD_PARTY_NOTICES.md.
const FABRIC: &[u8] = include_bytes!("../assets/logos/fabric.png");
const QUILT: &[u8] = include_bytes!("../assets/logos/quilt.png");
const NEOFORGE: &[u8] = include_bytes!("../assets/logos/neoforge.png");
const FORGE: &[u8] = include_bytes!("../assets/logos/forge.png");
/// Vanilla's mark: our own grass block (`tools/launcher_grass_block.py`), not Mojang's art.
pub(super) const GRASS: &[u8] = include_bytes!("../assets/logos/grass-block.png");
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
const PIXEL_ART_MAX: u32 = 256;
const SHARP_FILL: f32 = 0.75;

const MARK_SHARE: f32 = 2. / 3.;
const ICON_GRID: f32 = 8.;

#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Mark {
    Eclipse,
    Grass,
    Loader(Loader),
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub(super) enum IconKey {
    Picture(String),
    Chosen(u64),
    Grass,
    Eclipse,
    Loader(&'static str),
    Texture(String),
}

pub(super) enum Picture {
    None,
    Saved(Box<Instance>),
    Picked(Vec<u8>),
}

pub(super) struct IconSpec {
    pub(super) picture: Picture,
    pub(super) icon: Option<instances::Icon>,
    pub(super) mark: Mark,
    pub(super) name: String,
}

fn inner(size: f32) -> f32 {
    (size * MARK_SHARE / ICON_GRID).round() * ICON_GRID
}

impl Launcher {
    pub(super) fn instance_icon(&mut self, i: &Instance, size: f32) -> AnyElement {
        let spec = self.icon_spec(i);
        self.icon_for(spec, size)
    }

    pub(super) fn icon_spec(&self, i: &Instance) -> IconSpec {
        let own = i.kind == InstanceKind::Default && i.icon.is_none();
        IconSpec {
            picture: if own {
                Picture::None
            } else {
                Picture::Saved(Box::new(i.clone()))
            },
            icon: i.icon.clone(),
            mark: if own { Mark::Eclipse } else { self.mark_of(i) },
            name: i.name.clone(),
        }
    }

    fn mark_of(&self, i: &Instance) -> Mark {
        match self.instance_loader(i).and_then(|l| Loader::parse(&l).ok()) {
            Some(loader) => Mark::Loader(loader),
            None if i.java.is_some() => Mark::Grass,
            None => Mark::Eclipse,
        }
    }

    /// An instance's icon on a `size` tile: its picture, else the block or colour chosen for it,
    /// else its mark, a share of the tile on the icon grid; its initial on its colour (the accent
    /// when none was chosen) when none of these can be drawn.
    pub(super) fn icon_for(&mut self, spec: IconSpec, size: f32) -> AnyElement {
        let inner = inner(size);
        let tile = ui::tile(size);
        let scale = self.scale;
        let picture = match spec.picture {
            Picture::None => None,
            Picture::Saved(i) => self.instance_picture(&i, inner),
            Picture::Picked(png) => {
                let key = IconKey::Chosen(instance_state::picture_key(&png));
                self.icon_image(key, inner, false, || Some(png))
            }
        };
        let mut colour = ui::look().accent;
        match (picture, &spec.icon) {
            (Some(image), _) => return tile.child(centred(image, inner, scale)).into_any_element(),
            (None, Some(instances::Icon::Colour(hex))) => {
                colour = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(colour);
            }
            (None, Some(instances::Icon::Texture(name))) => {
                if let Some(image) = self.texture_image(name, inner) {
                    return tile
                        .child(gpui::img(image).flex_none().size(px(inner)))
                        .into_any_element();
                }
            }
            (None, None) => {
                if let Some(mark) = self.mark(&spec.mark, inner) {
                    return tile.child(mark).into_any_element();
                }
            }
        }
        ui::letter_tile(ui::color(colour), &spec.name, size).into_any_element()
    }

    pub(super) fn mark(&mut self, mark: &Mark, size: f32) -> Option<AnyElement> {
        let image = match mark {
            Mark::Eclipse => return Some(self.eclipse_mark(size)),
            Mark::Grass => self.grass_mark(size),
            Mark::Loader(loader) => self.loader_logo(*loader, size),
        }?;
        Some(centred(image, size, self.scale).into_any_element())
    }

    /// The instance's own picture, `size` points square. Whether there is one is asked once:
    /// `reload_instances` forgets the answer.
    pub(super) fn instance_picture(
        &mut self,
        instance: &Instance,
        size: f32,
    ) -> Option<Arc<RenderImage>> {
        let file = instance.home.join(instances::PICTURE);
        let key = IconKey::Picture(instance.id.clone());
        self.icon_image(key, size, true, || std::fs::read(file).ok())
    }

    pub(super) fn instance_loader(&self, instance: &Instance) -> Option<String> {
        match &instance.kind {
            InstanceKind::Port { slug } => {
                let pack = self.modpacks.imported.iter().find(|p| p.slug == *slug);
                pack.map(|p| p.pack.loader.to_ascii_lowercase())
            }
            _ => instance
                .java
                .as_ref()
                .and_then(|j| j.loader.as_ref())
                .map(|l| l.kind.id().to_string()),
        }
    }

    pub(super) fn grass_mark(&mut self, size: f32) -> Option<Arc<RenderImage>> {
        self.icon_image(IconKey::Grass, size, false, || Some(GRASS.to_vec()))
    }

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
            .entry((IconKey::Eclipse, (size * scale) as u32))
            .or_insert_with(|| Some(pixel_mark(size, scale)))
            .clone()
    }

    pub(super) fn loader_logo(&mut self, loader: Loader, size: f32) -> Option<Arc<RenderImage>> {
        let png = logo_png(loader);
        self.icon_image(IconKey::Loader(loader.id()), size, false, || {
            Some(png.to_vec())
        })
    }

    /// The image under `key` at `size` points, made from `bytes` the first time. A missing one is
    /// remembered only when `remember_missing` (a file that may never exist); else it is asked
    /// again next frame (a logo still downloading).
    pub(super) fn icon_image(
        &mut self,
        key: IconKey,
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

pub(super) fn modded_mark() -> gpui::Svg {
    ui::icon(
        crate::icons::MODS,
        ui::IconSize::Px24,
        ui::color(ui::look().mark),
    )
}

pub(super) fn logo_png(loader: Loader) -> &'static [u8] {
    match loader {
        Loader::Fabric => FABRIC,
        Loader::Quilt => QUILT,
        Loader::NeoForge => NEOFORGE,
        Loader::Forge => FORGE,
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
        self.pick_paths(Pick::File, tr("Choose"), cx, |this, mut paths, cx| {
            if let Some(path) = paths.pop() {
                this.read_instance_picture(path, cx);
            }
        });
    }

    fn read_instance_picture(&mut self, path: std::path::PathBuf, cx: &mut Context<Self>) {
        let made = cx.background_executor().spawn(async move {
            let too_big =
                std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > MAX_PICTURE_FILE;
            let bytes = (!too_big)
                .then(|| std::fs::read(&path).ok())
                .flatten()
                .ok_or_else(|| format!("{}: too big, or unreadable", path.display()))?;
            Ok::<_, String>(Chosen {
                png: instances::picture(bytes)?,
            })
        });
        cx.spawn(async move |this, cx| {
            let made = made.await;
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
