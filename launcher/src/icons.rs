//! The icons served to `svg()`, and the slots that hold icons downloaded at run time.
use crate::ui::{self, white, IconSize};
use eclipse_theme::glass::fill;
use gpui::{img, prelude::*, px, Div, ImageSource, ObjectFit, SharedString};
use std::borrow::Cow;

const LARGE_DIR: &str = "icons/24/";

macro_rules! large_icons {
    ($($name:literal),* $(,)?) => {
        const LARGE: &[(&str, &[u8])] = &[
            $(($name, include_bytes!(concat!("../../crates/eclipse-theme/icons/glass/24/", $name, ".svg")))),*
        ];
    };
}

large_icons!(
    "alert-circle",
    "clock",
    "computer",
    "computer-terminal-01",
    "download-04",
    "earth",
    "folder-01",
    "globe-02",
    "image-01",
    "information-circle",
    "more-horizontal",
    "package-01",
    "play",
    "puzzle",
    "search-01",
    "stop",
);

pub fn path(name: &str, size: IconSize) -> String {
    match size {
        IconSize::Px24 => format!("{LARGE_DIR}{name}.svg"),
        IconSize::Px16 | IconSize::Px32 => format!("icons/{name}.svg"),
    }
}

fn large(name: &str) -> Option<&'static [u8]> {
    LARGE.iter().find(|(n, _)| *n == name).map(|(_, svg)| *svg)
}

/// The icons, served to `svg()` as `icons/<name>.svg`.
pub struct Assets;

impl gpui::AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        let name = |dir: &str| path.strip_prefix(dir)?.strip_suffix(".svg");
        let found = match name(LARGE_DIR) {
            Some(name) => large(name).or_else(|| eclipse_theme::glass_icons::get(name)),
            None => name("icons/").and_then(eclipse_theme::glass_icons::get),
        };
        Ok(found.map(Cow::Borrowed))
    }

    fn list(&self, _path: &str) -> gpui::Result<Vec<SharedString>> {
        let small = eclipse_theme::glass_icons::ALL
            .iter()
            .map(|(name, _)| format!("icons/{name}.svg").into());
        let large = LARGE
            .iter()
            .map(|(name, _)| format!("{LARGE_DIR}{name}.svg").into());
        Ok(small.chain(large).collect())
    }
}

/// A square slot for an icon: the picture at `source` when there is one, else the first letter of
/// `name` on a tile.
pub fn slot(source: Option<ImageSource>, name: &str, size: f32) -> Div {
    match source {
        Some(source) => {
            ui::tile(size).child(img(source).size(px(size)).object_fit(ObjectFit::Cover))
        }
        None => ui::letter_tile(white(fill::BUTTON), name, size),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::AssetSource;

    const CELL: u32 = 2;

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
        let small = eclipse_theme::glass_icons::ALL
            .iter()
            .map(|(name, svg)| (format!("icons/{name}.svg"), *svg));
        let large = LARGE
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
    fn a_large_icon_has_a_small_one_of_the_same_name() {
        for (name, _) in LARGE {
            assert!(
                eclipse_theme::glass_icons::get(name).is_some(),
                "{name} is drawn only at 24 px"
            );
        }
    }

    #[test]
    fn icons_at_24_px_come_from_the_large_set() {
        let load = |path: String| Assets.load(&path).unwrap().map(|svg| svg.into_owned());
        assert_eq!(
            load(path("play", IconSize::Px24)),
            large("play").map(<[u8]>::to_vec)
        );
        assert_eq!(
            load(path("play", IconSize::Px16)),
            eclipse_theme::glass_icons::get("play").map(<[u8]>::to_vec)
        );
        assert_eq!(
            load(path("cancel-01", IconSize::Px24)),
            eclipse_theme::glass_icons::get("cancel-01").map(<[u8]>::to_vec)
        );
        assert!(load(path("no-such-icon", IconSize::Px24)).is_none());
        let listed = Assets.list("").unwrap();
        assert!(listed
            .iter()
            .any(|p| p.as_ref() == path("play", IconSize::Px24)));
    }
}
