//! The launcher's view of Eclipse's design tokens (`crates/eclipse-theme`): the Corona look
//! (`eclipse_theme::glass`) in Geist, with Geist Mono for paths and codes. The light appearance
//! draws it over the dawn sky (`eclipse_theme::night::DAWN`).
use eclipse_launcher::backend::Appearance;
use eclipse_launcher::themes::Accent;
use eclipse_theme as tokens;
use gpui::{rgb, Hsla, WindowAppearance};
use std::time::Duration;

pub use tokens::night::{self, Night};
pub const FONT: &str = tokens::FONT;
pub const MONO: &str = tokens::MONO;

/// A disabled control's opacity, shared with the game's menus and the website.
pub use tokens::DISABLED;

/// Motion: short and eased out, never bouncy.
pub const QUICK: Duration = tokens::QUICK.duration();

#[derive(Clone, Copy)]
pub struct Theme {
    /// The look's colours for the accent.
    pub look: tokens::glass::Glass,
    /// The page behind everything the sky doesn't cover.
    pub page: Hsla,
    pub side: Hsla,
    /// The scene's colours (`scene.rs`).
    pub night: Night,
    pub dark: bool,
}

fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}

/// Whether the launcher is pinned dark (`Some(true)`) or light, or follows the system (`None`):
/// `ECLIPSE_LAUNCHER_APPEARANCE=light` or `dark` (for screenshots and tests), else the player's
/// choice in Settings.
pub fn pinned_dark(choice: Appearance) -> Option<bool> {
    match std::env::var("ECLIPSE_LAUNCHER_APPEARANCE").as_deref() {
        Ok("light") => Some(false),
        Ok("dark") => Some(true),
        _ => match choice {
            Appearance::Dark => Some(true),
            Appearance::Light => Some(false),
            Appearance::System => None,
        },
    }
}

impl Theme {
    /// This look under another sky (a launcher theme from Settings), top to bottom.
    pub fn with_sky(mut self, sky: [u32; 6]) -> Self {
        self.night.sky = sky;
        self
    }

    /// The pinned appearance (`pinned_dark`), the system's when the player asked to follow it, in
    /// the player's accent.
    pub fn for_appearance(choice: Appearance, accent: Accent, system: WindowAppearance) -> Self {
        use tokens::glass::{Glass, BASE};
        let system_dark = !matches!(
            system,
            WindowAppearance::Light | WindowAppearance::VibrantLight
        );
        let dark = pinned_dark(choice).unwrap_or(system_dark);
        let night = if dark { night::NIGHT } else { night::DAWN };
        let look = Glass::new(accent.rgb());
        let look = if dark { look } else { look.light() };
        Self {
            look,
            page: c(if dark {
                tokens::glass::PAGE
            } else {
                night.page
            }),
            side: c(if dark { BASE } else { night.side }),
            night,
            dark,
        }
    }
}

/// How strongly a selected suggestion in a text field is tinted with the accent.
pub const SELECTION: f32 = 0.22;

#[cfg(test)]
mod tests {
    use super::SELECTION;
    use eclipse_launcher::themes::Accent;
    use eclipse_theme::contrast;
    use eclipse_theme::glass::{
        ink, light_ink, light_surfaces, Glass, DANGER, DANGER_LIGHT, FIELD_BG, FLOAT, ON_LIGHT,
        PAPER,
    };
    use eclipse_theme::night::over;

    /// What the dark look draws besides text: the keyboard focus ring and the border of a
    /// refused field on every surface (WCAG 1.4.11 asks 3:1), and text over a selected
    /// suggestion.
    #[test]
    fn marks_and_selections_read_clearly() {
        let look = Glass::default_look();
        for (name, bg) in [("plate", look.plate), ("float", FLOAT), ("field", FIELD_BG)] {
            let ring = contrast(look.accent_soft, bg);
            assert!(ring >= 3.0, "focus ring on {name}: {ring:.2}");
            let refused = contrast(DANGER, bg);
            assert!(refused >= 3.0, "refused field on {name}: {refused:.2}");
        }
        let selection = over(look.accent_soft, SELECTION, FLOAT);
        let ratio = contrast(0xffffff, selection);
        assert!(ratio >= 4.5, "text on a selection: {ratio:.2}");
    }

    #[test]
    fn a_refused_field_reads_on_the_light_plates() {
        for bg in light_surfaces() {
            let refused = contrast(DANGER_LIGHT, bg);
            assert!(refused >= 3.0, "refused field on #{bg:06x}: {refused:.2}");
        }
    }

    #[test]
    fn a_switch_that_is_off_shows_its_edge() {
        let dim = light_ink(ink::DIM);
        for bg in light_surfaces() {
            let edge = contrast(over(ON_LIGHT, dim, bg), bg);
            assert!(edge >= 3.0, "off switch on #{bg:06x}: {edge:.2}");
        }
        for bg in [Glass::default_look().plate, FLOAT] {
            let edge = contrast(over(0xffffff, ink::DIM, bg), bg);
            assert!(edge >= 3.0, "off switch on #{bg:06x}: {edge:.2}");
        }
    }

    #[test]
    fn light_marks_read_for_every_accent_the_picker_makes() {
        let hues = (0..360).map(|hue| Accent::custom(hue as f32));
        for accent in Accent::ALL.into_iter().chain(hues) {
            let look = Glass::new(accent.rgb()).light();
            let name = accent.name();
            for bg in light_surfaces() {
                let icon = contrast(look.mark, bg);
                assert!(icon >= 3.0, "{name}: icon on #{bg:06x}: {icon:.2}");
                let ring = contrast(look.accent_soft, bg);
                assert!(ring >= 4.5, "{name}: link and ring on #{bg:06x}: {ring:.2}");
            }
            let selection = over(look.accent_soft, SELECTION, PAPER);
            let ratio = contrast(ON_LIGHT, selection);
            assert!(ratio >= 4.5, "{name}: text on a selection: {ratio:.2}");
        }
    }
}
