//! The launcher's view of Eclipse's design tokens (`crates/eclipse-theme`): the Corona look
//! (`eclipse_theme::glass`) in Geist, with Geist Mono for paths and codes. The light appearance
//! draws it over the dawn sky (`eclipse_theme::night::DAWN`).
use eclipse_launcher::backend::{Appearance, Settings};
use eclipse_launcher::themes::{self, Accent};
use eclipse_theme as tokens;
use gpui::{rgb, Hsla, WindowAppearance};
use std::time::Duration;

use tokens::night::{self, Night};
pub const FONT: &str = tokens::FONT;
pub const MONO: &str = tokens::MONO;

pub use tokens::DISABLED;

pub const QUICK: Duration = tokens::QUICK.duration();

pub type Inputs = (Appearance, Accent, Option<String>, WindowAppearance);

pub fn inputs(settings: &Settings, system: WindowAppearance) -> Inputs {
    (
        settings.appearance,
        settings.accent,
        settings.theme.clone(),
        system,
    )
}

#[derive(Clone, Copy)]
pub struct Theme {
    pub look: tokens::glass::Glass,
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
    fn with_sky(mut self, sky: [u32; 6]) -> Self {
        self.night.sky = sky;
        self
    }

    pub fn for_settings(settings: &Settings, system: WindowAppearance) -> Self {
        let theme = Self::for_appearance(settings.appearance, settings.accent, system);
        match themes::theme(settings.theme.as_deref()) {
            Some(picked) => theme.with_sky(picked.sky),
            None => theme,
        }
    }

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
            side: c(if dark { BASE } else { night.side }),
            night,
            dark,
        }
    }
}

#[cfg(test)]
mod tests {
    use eclipse_launcher::themes::Accent;
    use eclipse_theme::contrast;
    use eclipse_theme::glass::{fill, ink, light_ink, light_surfaces, Glass, ON_LIGHT, PAPER};
    use eclipse_theme::night::over;

    #[test]
    fn a_light_switch_that_is_on_shows_its_edge() {
        use eclipse_theme::glass::SWITCH_EDGE_LIGHT;
        let hues = (0..360).map(|hue| Accent::custom(hue as f32));
        for accent in Accent::ALL.into_iter().chain(hues) {
            let look = Glass::new(accent.rgb()).light();
            let rim = over(ON_LIGHT, SWITCH_EDGE_LIGHT, look.accent);
            for bg in light_surfaces() {
                let ratio = contrast(rim, bg);
                assert!(
                    ratio >= 3.0,
                    "{}: on switch on #{bg:06x}: {ratio:.2}",
                    accent.name()
                );
            }
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
            let selection = over(look.accent_soft, fill::TEXT_SELECTION, PAPER);
            let ratio = contrast(ON_LIGHT, selection);
            assert!(ratio >= 4.5, "{name}: text on a selection: {ratio:.2}");
        }
    }

    #[test]
    fn a_busy_label_reads_on_its_track() {
        let hues = (0..360).map(|hue| Accent::custom(hue as f32));
        for accent in Accent::ALL.into_iter().chain(hues) {
            let dark = Glass::new(accent.rgb());
            for (look, light) in [(dark, false), (dark.light(), true)] {
                let track = look.busy_track(light);
                let ratio = contrast(look.on_accent, track);
                let full = contrast(look.on_accent, look.accent).min(4.5);
                assert!(
                    ratio >= full,
                    "{}: busy label on #{track:06x}: {ratio:.2}",
                    accent.name()
                );
            }
        }
    }

    #[test]
    fn text_and_marks_read_on_every_launcher_theme_sky() {
        use eclipse_launcher::themes::THEMES;
        let look = Glass::default_look();
        let light = look.light();
        for theme in THEMES {
            for (stop, &sky) in theme.sky[..3].iter().enumerate() {
                for level in ink::LEVELS {
                    if level == ink::DIM && stop == 2 {
                        continue;
                    }
                    let ratio = contrast(over(0xffffff, level, sky), sky);
                    assert!(
                        ratio >= 4.5,
                        "{} stop {stop}, ink {level}: {ratio:.2}",
                        theme.id
                    );
                }
                let mark = contrast(look.mark, sky);
                assert!(mark >= 3., "{} stop {stop}, mark: {mark:.2}", theme.id);
            }
            for sky in theme
                .sky
                .map(|c| over(0xffffff, crate::scene::THEME_SKY_WASH, c))
            {
                for level in ink::LEVELS {
                    let a = light_ink(level);
                    let ratio = contrast(over(ON_LIGHT, a, sky), sky);
                    assert!(
                        ratio >= 4.5,
                        "{} by day #{sky:06x}, ink {a}: {ratio:.2}",
                        theme.id
                    );
                }
                let mark = contrast(light.mark, sky);
                assert!(
                    mark >= 3.,
                    "{} by day #{sky:06x}, mark: {mark:.2}",
                    theme.id
                );
                let link = contrast(light.accent_soft, sky);
                assert!(
                    link >= 4.5,
                    "{} by day #{sky:06x}, link: {link:.2}",
                    theme.id
                );
            }
        }
    }
}
