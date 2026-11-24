//! The app's UI font at a weight.
//!
//! `Font::DEFAULT` names the platform's sans-serif, not the UI font, so a label
//! built from it changes typeface as soon as it takes a weight. Build UI text
//! from [`regular`] or [`at`] instead.
//!
//! The family is fixed at launch, as the renderer's default font is: changing
//! the UI font in Preferences applies after a restart.

use iced::Font;
use iced::font::{Family, Weight};
use std::sync::OnceLock;

/// The UI family when no choice is saved.
pub const DEFAULT_FAMILY: &str = "Inter";

static LAUNCH_FAMILY: OnceLock<Family> = OnceLock::new();

/// Fix the UI family for this run from the saved choice, and return the
/// regular face for the renderer's default font. Call once, before the UI runs.
pub fn set_launch_family(name: &str) -> Font {
    let family = *LAUNCH_FAMILY.get_or_init(|| Family::name(name));
    in_family(family, Weight::Normal)
}

/// The UI font at regular weight.
pub fn regular() -> Font {
    at(Weight::Normal)
}

/// The UI font at `weight`.
pub fn at(weight: Weight) -> Font {
    let family = LAUNCH_FAMILY
        .get()
        .copied()
        .unwrap_or(Family::Name(DEFAULT_FAMILY));
    in_family(family, weight)
}

fn in_family(family: Family, weight: Weight) -> Font {
    Font {
        family,
        weight,
        ..Font::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saved_family_is_kept_at_every_weight() {
        let saved = Family::name("Roboto");
        for weight in [Weight::Normal, Weight::Medium, Weight::Semibold] {
            let font = in_family(saved, weight);
            assert_eq!(font.family, saved);
            assert_eq!(font.weight, weight);
        }
    }

    #[test]
    fn the_ui_font_is_never_the_platform_sans() {
        assert_ne!(at(Weight::Medium).family, Font::DEFAULT.family);
        assert_eq!(at(Weight::Medium).family, regular().family);
    }
}
