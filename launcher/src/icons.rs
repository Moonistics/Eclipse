//! The icons served to `svg()`, and the slots that hold icons downloaded at run time.
use crate::ui::{self, ink, IconSize};
use eclipse_theme::glass::fill;
use eclipse_theme::glass_icons;
use gpui::{img, prelude::*, px, Div, ImageSource, ObjectFit, SharedString};
use std::borrow::Cow;

const LARGE_DIR: &str = "icons/24/";
const CELLS: f32 = 8.;
const LARGE_CELLS: f32 = 12.;

macro_rules! meanings {
    ($($meaning:ident = $drawing:literal),* $(,)?) => {
        $(pub const $meaning: &str = $drawing;)*
        #[cfg(test)]
        const MEANINGS: &[&str] = &[$($meaning),*];
    };
}

meanings!(
    PACK = "cube",
    WORLD = "earth",
    SERVER = "computer",
    MULTIPLAYER = "user-multiple-02",
    VERSION = "package-01",
    LANGUAGE = "language-square",
    NETWORK = "globe-02",
    CONSOLE = "computer-terminal-01",
    DEVELOPER = "source-code",
    HOME = "home-01",
    NOTES = "note-01",
    INFO = "information-circle",
    SETTINGS = "settings-01",
    UPDATES = "download-04",
    RETRY = "refresh",
    RANDOM = "dice",
    LAUNCH_OPTIONS = "sliders-horizontal",
    APPEARANCE = "contrast",
    ANY_COLOUR = "color-picker",
    SERVER_PANEL = "dashboard-square-01",
    MODS = "puzzle",
    NEW_INSTANCE = "plus-sign",
    ACCOUNT = "user",
    GAME = "game-controller-01",
    AI = "ai-chat-02",
    FOLDERS = "folder-01",
    DATA_COLLECTION = "view",
    COPY = "copy-01",
);

pub const OPEN_FOLDER: &str = FOLDERS;
pub const DOWNLOAD: &str = UPDATES;
pub const ADD: &str = NEW_INSTANCE;

pub fn path(name: &str, size: IconSize) -> String {
    match size {
        IconSize::Px24 => format!("{LARGE_DIR}{name}.svg"),
        IconSize::Px16 | IconSize::Px32 => format!("icons/{name}.svg"),
    }
}

pub fn cells(name: &str, size: IconSize) -> f32 {
    match size {
        IconSize::Px24 if glass_icons::large(name).is_some() => LARGE_CELLS,
        _ => CELLS,
    }
}

pub struct Assets;

impl gpui::AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        let name = |dir: &str| path.strip_prefix(dir)?.strip_suffix(".svg");
        let found = match name(LARGE_DIR) {
            Some(name) => glass_icons::large(name).or_else(|| glass_icons::get(name)),
            None => name("icons/").and_then(glass_icons::get),
        };
        Ok(found.map(Cow::Borrowed))
    }

    fn list(&self, _path: &str) -> gpui::Result<Vec<SharedString>> {
        let small = glass_icons::ALL
            .iter()
            .map(|(name, _)| format!("icons/{name}.svg").into());
        let large = glass_icons::LARGE
            .iter()
            .map(|(name, _)| format!("{LARGE_DIR}{name}.svg").into());
        Ok(small.chain(large).collect())
    }
}

pub fn slot(source: Option<ImageSource>, name: &str, size: f32) -> Div {
    match source {
        Some(source) => {
            ui::tile(size).child(img(source).size(px(size)).object_fit(ObjectFit::Cover))
        }
        None => ui::letter_tile(ink(fill::BUTTON), name, size),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::AssetSource;

    const CELL: u32 = 2;

    const MOST_SHARED: f32 = 0.75;

    struct Drawing {
        side: u32,
        cells: Vec<[u32; 4]>,
    }

    fn drawing(svg: &[u8]) -> Option<Drawing> {
        let svg = std::str::from_utf8(svg).expect("an svg is text");
        if !svg.contains("crispEdges") {
            return None;
        }
        let side = svg
            .split("viewBox=\"0 0 ")
            .nth(1)
            .and_then(|rest| rest.split(' ').next())
            .and_then(|n| n.parse().ok())
            .expect("a square viewBox");
        let path = svg
            .split(" d=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .expect("one path");
        let cells = path
            .split_terminator('z')
            .map(|rect| {
                let numbers: Vec<u32> = rect
                    .split(|c: char| !c.is_ascii_digit())
                    .filter(|n| !n.is_empty())
                    .map(|n| n.parse().expect("a whole number"))
                    .collect();
                let [x, y, w, h, back] = numbers[..] else {
                    panic!("{rect} is not one rectangle");
                };
                assert_eq!(w, back, "{rect} does not close");
                [x, y, w, h]
            })
            .collect();
        Some(Drawing { side, cells })
    }

    fn every_drawing() -> Vec<(String, Drawing)> {
        let small = glass_icons::ALL
            .iter()
            .map(|(name, svg)| (format!("icons/{name}.svg"), *svg));
        let large = glass_icons::LARGE
            .iter()
            .map(|(name, svg)| (format!("{LARGE_DIR}{name}.svg"), *svg));
        small
            .chain(large)
            .filter_map(|(path, svg)| drawing(svg).map(|d| (path, d)))
            .collect()
    }

    #[test]
    fn every_pixel_icon_lands_on_whole_pixels_at_one_and_a_half_times() {
        for (path, drawing) in every_drawing() {
            for [x, y, w, h] in drawing.cells {
                assert!(
                    [x, y, w, h].iter().all(|v| v % CELL == 0),
                    "{path}: a cell at ({x}, {y}) {w}x{h} falls between pixels at 1.5x"
                );
                assert!(
                    x + w <= drawing.side && y + h <= drawing.side,
                    "{path}: a cell at ({x}, {y}) leaves the icon"
                );
            }
        }
    }

    #[test]
    fn each_set_is_drawn_at_its_own_size() {
        for (path, drawing) in every_drawing() {
            let side = if path.starts_with(LARGE_DIR) {
                IconSize::Px24.px()
            } else {
                IconSize::Px16.px()
            };
            assert_eq!(drawing.side, side as u32, "{path}");
        }
    }

    #[test]
    fn every_icon_snaps_on_the_grid_it_is_drawn_on() {
        let grid = |svg: &[u8]| drawing(svg).map(|d| (d.side / CELL) as f32);
        for (name, svg) in glass_icons::ALL {
            let Some(small) = grid(svg) else { continue };
            let large = glass_icons::large(name).and_then(grid);
            assert_eq!(cells(name, IconSize::Px16), small, "{name}");
            assert_eq!(cells(name, IconSize::Px32), small, "{name}");
            assert_eq!(
                cells(name, IconSize::Px24),
                large.unwrap_or(small),
                "{name}"
            );
        }
    }

    fn filled(drawing: &Drawing) -> std::collections::HashSet<(u32, u32)> {
        let mut cells = std::collections::HashSet::new();
        for [x, y, w, h] in &drawing.cells {
            for cy in (*y..y + h).step_by(CELL as usize) {
                for cx in (*x..x + w).step_by(CELL as usize) {
                    cells.insert((cx, cy));
                }
            }
        }
        cells
    }

    #[test]
    fn each_meaning_has_a_drawing_of_its_own() {
        let meanings = MEANINGS;
        for (i, name) in meanings.iter().enumerate() {
            assert!(!meanings[..i].contains(name), "{name} means two things");
        }
        let drawn: Vec<_> = meanings
            .iter()
            .map(|name| {
                let svg = glass_icons::get(name).expect(name);
                (name, filled(&drawing(svg).expect(name)))
            })
            .collect();
        for (i, (a, cells)) in drawn.iter().enumerate() {
            for (b, others) in &drawn[..i] {
                let both = cells.intersection(others).count() as f32;
                let shared = both / cells.union(others).count() as f32;
                assert!(
                    shared <= MOST_SHARED,
                    "{a} and {b} share {:.0}% of their cells",
                    shared * 100.
                );
            }
        }
    }

    #[test]
    fn icons_at_24_px_come_from_the_large_set() {
        let load = |path: String| Assets.load(&path).unwrap().map(|svg| svg.into_owned());
        assert_eq!(
            load(path("play", IconSize::Px24)),
            glass_icons::large("play").map(<[u8]>::to_vec)
        );
        assert_eq!(
            load(path("play", IconSize::Px16)),
            glass_icons::get("play").map(<[u8]>::to_vec)
        );
        assert_eq!(
            load(path("cancel-01", IconSize::Px24)),
            glass_icons::get("cancel-01").map(<[u8]>::to_vec)
        );
        assert!(load(path("no-such-icon", IconSize::Px24)).is_none());
        let listed = Assets.list("").unwrap();
        assert!(listed
            .iter()
            .any(|p| p.as_ref() == path("play", IconSize::Px24)));
    }
}
