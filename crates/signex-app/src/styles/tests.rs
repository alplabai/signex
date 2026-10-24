//! Light/dark behaviour of the theme-derived helpers, over every builtin.

use super::buttons::{outline_button, subtle_button};
use super::ti;
use super::tokens::*;
use iced::widget::button;
use iced::{Background, Color, Theme};
use signex_types::theme::{ThemeId, ThemeTokens};

fn builtin_tokens() -> Vec<(ThemeId, ThemeTokens)> {
    ThemeId::BUILTINS
        .iter()
        .map(|&id| (id, signex_types::theme::theme_tokens(id)))
        .collect()
}

/// Perceptual (gamma-encoded) lightness. Used for the "visibly differs"
/// check: a 6 % overlay moves linear luminance by <0.02 on near-black
/// panels yet is plainly visible, so linear luminance is the wrong ruler.
fn luma(c: Color) -> f32 {
    0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b
}

fn is_light(t: &ThemeTokens) -> bool {
    relative_luminance(ti(t.panel_bg)) > 0.5
}

#[test]
fn hover_overlay_visibly_shifts_panel_toward_text_on_every_builtin() {
    for (id, t) in builtin_tokens() {
        let panel = ti(t.panel_bg);
        let lit = composite_over(hover_overlay(&t), panel);
        let delta = luma(lit) - luma(panel);
        assert!(
            delta.abs() >= 0.02,
            "{id:?}: hover overlay delta {delta} is not visible over panel_bg"
        );
        if is_light(&t) {
            assert!(
                delta < 0.0,
                "{id:?}: light theme hover must darken, got {delta}"
            );
        } else {
            assert!(
                delta > 0.0,
                "{id:?}: dark theme hover must lighten, got {delta}"
            );
        }
    }
}

#[test]
fn overlay_steps_are_monotonic() {
    let (_, t) = builtin_tokens()[0];
    let a = |s| overlay(&t, s).a;
    assert!(a(Overlay::Faint) < a(Overlay::Hover) && a(Overlay::Hover) < a(Overlay::Pressed));
}

#[test]
fn on_accent_is_the_expected_colour_and_reaches_4_5_to_1_on_every_builtin() {
    // White wherever it reaches 4.5:1 on the accent; black where white fails.
    let expected = |id: ThemeId| match id {
        ThemeId::VsCodeDark => Color::WHITE,
        ThemeId::Signex
        | ThemeId::Alplab
        | ThemeId::CatppuccinMocha
        | ThemeId::GitHubDark
        | ThemeId::SolarizedLight
        | ThemeId::Nord => Color::BLACK,
        ThemeId::Custom => unreachable!("BUILTINS excludes Custom"),
    };
    for (id, t) in builtin_tokens() {
        let got = on_accent(&t);
        assert_eq!(got, expected(id), "{id:?}: on_accent colour");
        let ratio = contrast_ratio(got, accent(&t));
        assert!(ratio >= 4.5, "{id:?}: on_accent contrast {ratio} < 4.5:1");
    }
}

#[test]
fn readable_on_prefers_white_when_it_reaches_4_5_to_1() {
    for (id, t) in builtin_tokens() {
        for bg in [accent(&t), ti(t.selection), ti(t.panel_bg)] {
            if contrast_ratio(Color::WHITE, bg) >= 4.5 {
                assert_eq!(readable_on(bg), Color::WHITE, "{id:?}");
            }
        }
    }
}

#[test]
fn light_themes_have_text_darker_than_panel() {
    // Property, not a roster: a theme is light exactly when its primary
    // text is darker than its panel, and the helpers must handle both.
    for (id, t) in builtin_tokens() {
        let text_darker = relative_luminance(ti(t.text)) < relative_luminance(ti(t.panel_bg));
        assert_eq!(is_light(&t), text_darker, "{id:?}");
    }
}

#[test]
fn subtle_button_is_transparent_at_rest_and_overlays_on_hover() {
    for (id, t) in builtin_tokens() {
        let style = subtle_button(&t);
        assert_eq!(
            style(&Theme::Dark, button::Status::Active).background,
            None,
            "{id:?}"
        );
        assert_eq!(
            style(&Theme::Dark, button::Status::Hovered).background,
            Some(Background::Color(hover_overlay(&t))),
            "{id:?}"
        );
        assert_eq!(
            style(&Theme::Dark, button::Status::Pressed).background,
            Some(Background::Color(overlay(&t, Overlay::Pressed))),
            "{id:?}"
        );
        assert_eq!(
            style(&Theme::Dark, button::Status::Disabled).text_color,
            ti(t.text_secondary),
            "{id:?}"
        );
    }
}

#[test]
fn outline_button_rest_fill_is_faint_overlay() {
    for (id, t) in builtin_tokens() {
        let s = outline_button(&t)(&Theme::Dark, button::Status::Active);
        assert_eq!(
            s.background,
            Some(Background::Color(overlay(&t, Overlay::Faint))),
            "{id:?}"
        );
    }
}
