//! How the selected option of a segmented control looks in a dark theme:
//! Preferences ▸ Appearance ▸ Selected option, saved in `prefs.json` as
//! `selected_segment`. Light themes keep their own chip under either value.

use super::{Color, ThemeId, ThemeTokens};

/// Label on the accent chip.
const ACCENT_CHIP_INK: Color = Color::new(0xFF, 0xFF, 0xFF, 0xFF);
/// The accent chip casts no shadow.
const NO_SHADOW: Color = Color::new(0, 0, 0, 0);

/// The look of the selected option of a segmented control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectedSegmentLook {
    /// A light neutral chip with a shadow: the theme's own selected-segment
    /// tokens.
    #[default]
    Neutral,
    /// A chip in the theme's accent, with white text and no shadow.
    Accent,
}

impl SelectedSegmentLook {
    /// The value stored in `prefs.json`.
    pub fn as_pref(self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::Accent => "accent",
        }
    }

    /// Parse a stored value; anything unknown is [`Self::Neutral`].
    pub fn from_pref(value: &str) -> Self {
        match value {
            "accent" => Self::Accent,
            _ => Self::Neutral,
        }
    }

    /// `tokens` of theme `id` with this look on their selected segment.
    #[must_use]
    pub fn apply(self, id: ThemeId, mut tokens: ThemeTokens) -> ThemeTokens {
        if self == Self::Accent && !id.is_light() {
            tokens.extended.selected_segment = Some(tokens.accent);
            tokens.extended.selected_segment_ink = Some(ACCENT_CHIP_INK);
            tokens.extended.selected_segment_shadow = Some(NO_SHADOW);
        }
        tokens
    }
}

#[cfg(test)]
mod tests {
    use super::super::theme_tokens;
    use super::*;

    #[test]
    fn neutral_is_the_default_and_an_unknown_value_reads_as_neutral() {
        assert_eq!(SelectedSegmentLook::default(), SelectedSegmentLook::Neutral);
        for stored in ["", "ACCENT", "sideways"] {
            assert_eq!(
                SelectedSegmentLook::from_pref(stored),
                SelectedSegmentLook::Neutral,
                "{stored:?}"
            );
        }
    }

    #[test]
    fn each_look_round_trips_through_its_stored_name() {
        for look in [SelectedSegmentLook::Neutral, SelectedSegmentLook::Accent] {
            assert_eq!(SelectedSegmentLook::from_pref(look.as_pref()), look);
        }
    }

    #[test]
    fn graphite_dark_neutral_has_the_decided_chip_and_keeps_the_shadow() {
        let id = ThemeId::GraphiteDark;
        let tokens = SelectedSegmentLook::Neutral.apply(id, theme_tokens(id));
        let e = tokens.extended;
        assert_eq!(e.selected_segment, Some(Color::from_hex("#3D4047")));
        assert_eq!(e.selected_segment_ink, Some(Color::from_hex("#FFFFFF")));
        assert_eq!(e.selected_segment_shadow, e.shadow);
    }

    #[test]
    fn graphite_dark_accent_has_the_decided_chip_and_no_shadow() {
        let id = ThemeId::GraphiteDark;
        let e = SelectedSegmentLook::Accent
            .apply(id, theme_tokens(id))
            .extended;
        assert_eq!(e.selected_segment, Some(Color::from_hex("#C2552D")));
        assert_eq!(e.selected_segment_ink, Some(Color::from_hex("#FFFFFF")));
        assert_eq!(e.selected_segment_shadow.map(|c| c.a), Some(0));
    }

    #[test]
    fn graphite_light_is_unchanged_by_either_look() {
        let id = ThemeId::GraphiteLight;
        let light = theme_tokens(id);
        for look in [SelectedSegmentLook::Neutral, SelectedSegmentLook::Accent] {
            assert_eq!(look.apply(id, light), light, "{look:?}");
        }
        let e = light.extended;
        assert_eq!(e.selected_segment, e.raised);
        assert_eq!(e.selected_segment_ink, Some(light.text));
        assert_eq!(e.selected_segment_shadow, e.shadow);
    }

    #[test]
    fn a_look_changes_only_the_selected_segment_tokens() {
        let id = ThemeId::GraphiteDark;
        let neutral = theme_tokens(id);
        let mut accent = SelectedSegmentLook::Accent.apply(id, neutral);
        assert_ne!(accent, neutral);
        accent.extended.selected_segment = neutral.extended.selected_segment;
        accent.extended.selected_segment_ink = neutral.extended.selected_segment_ink;
        accent.extended.selected_segment_shadow = neutral.extended.selected_segment_shadow;
        assert_eq!(accent, neutral);
    }
}
