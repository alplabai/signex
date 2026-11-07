//! Light/dark behaviour of the theme-derived helpers, over every builtin.

use super::buttons::{accent_button, outline_button, subtle_button};
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

fn hue_degrees(c: Color) -> f32 {
    let (max, min) = (c.r.max(c.g).max(c.b), c.r.min(c.g).min(c.b));
    let d = max - min;
    if d == 0.0 {
        return 0.0;
    }
    let h = if max == c.r {
        ((c.g - c.b) / d).rem_euclid(6.0)
    } else if max == c.g {
        (c.b - c.r) / d + 2.0
    } else {
        (c.r - c.g) / d + 4.0
    };
    h * 60.0
}

#[test]
fn white_on_accent_fill_reaches_4_5_to_1_in_every_state_on_every_builtin() {
    for (id, t) in builtin_tokens() {
        assert_eq!(on_accent(&t), Color::WHITE, "{id:?}");
        for (state, fill) in [
            ("normal", accent_fill(&t)),
            ("hover", accent_fill_hover(&t)),
            ("pressed", accent_fill_pressed(&t)),
        ] {
            let ratio = contrast_ratio(Color::WHITE, fill);
            assert!(
                ratio >= 4.5,
                "{id:?} {state}: white contrast {ratio} < 4.5:1"
            );
        }
    }
}

#[test]
fn accent_fill_keeps_a_passing_accent_and_the_hue_of_a_darkened_one() {
    // Both Graphite accents pass, so an orange that fails (the retired
    // Signex accent) keeps the darkening branch covered.
    let failing = ThemeTokens {
        accent: signex_types::theme::Color::from_hex("#E8912D"),
        ..crate::styles::legacy_tokens_fixture()
    };
    assert!(contrast_ratio(Color::WHITE, accent(&failing)) < 4.5);
    let mut cases = builtin_tokens();
    cases.push((ThemeId::DEFAULT, failing));
    for (id, t) in cases {
        let (acc, fill) = (accent(&t), accent_fill(&t));
        if contrast_ratio(Color::WHITE, acc) >= 4.5 {
            assert_eq!(fill, acc, "{id:?}: passing accent must be unchanged");
        } else {
            let d = (hue_degrees(acc) - hue_degrees(fill)).abs();
            let d = d.min(360.0 - d);
            assert!(d <= 8.0, "{id:?}: hue moved {d} degrees");
            assert!(relative_luminance(fill) < relative_luminance(acc), "{id:?}");
        }
    }
    let vs = signex_types::theme::theme_tokens(ThemeId::GraphiteDark);
    assert_eq!(accent_fill(&vs), accent(&vs));
}

#[test]
fn accent_button_paints_accent_fill_with_white_text() {
    for (id, t) in builtin_tokens() {
        let style = accent_button(&t);
        let active = style(&Theme::Dark, button::Status::Active);
        assert_eq!(
            active.background,
            Some(Background::Color(accent_fill(&t))),
            "{id:?}"
        );
        assert_eq!(active.text_color, Color::WHITE, "{id:?}");
        let hovered = style(&Theme::Dark, button::Status::Hovered);
        assert_eq!(
            hovered.background,
            Some(Background::Color(accent_fill_hover(&t))),
            "{id:?}"
        );
        let pressed = style(&Theme::Dark, button::Status::Pressed);
        assert_eq!(
            pressed.background,
            Some(Background::Color(accent_fill_pressed(&t))),
            "{id:?}"
        );
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

#[test]
fn status_bar_has_a_top_only_border_in_the_theme_border_colour() {
    for (id, t) in builtin_tokens() {
        let style = super::status_bar(&t)(&Theme::Dark);
        let b = style.border;
        let width = |side: iced::border::Side| side.width.unwrap_or(b.width);
        assert_eq!(width(b.top), 1.0, "{id:?} top");
        assert_eq!(width(b.right), 0.0, "{id:?} right");
        assert_eq!(width(b.bottom), 0.0, "{id:?} bottom");
        assert_eq!(width(b.left), 0.0, "{id:?} left");
        assert_eq!(b.top.color.unwrap_or(b.color), ti(t.border), "{id:?}");
        assert_eq!(b.radius, 0.0.into(), "{id:?} radius");
    }
}

fn soft(c: signex_types::theme::Color) -> Color {
    Color { a: 0.15, ..ti(c) }
}

/// A legacy theme sets no extended token, so every resolved colour is the
/// value the app derived before the extended tokens existed.
#[test]
fn legacy_themes_resolve_to_their_derived_colours() {
    for (id, t) in builtin_tokens() {
        if !t.extended.is_empty() {
            continue;
        }
        let r = resolve(&t);
        let panel = ti(t.panel_bg);
        let border_half = Color {
            a: 0.5,
            ..ti(t.border)
        };
        assert_eq!(r.raised, panel, "{id:?}");
        assert_eq!(r.field, input_bg(&t), "{id:?}");
        assert_eq!(
            r.sunken,
            composite_over(overlay(&t, Overlay::Hover), panel),
            "{id:?}"
        );
        assert_eq!(r.line_soft, composite_over(border_half, panel), "{id:?}");
        assert_eq!(r.line_strong, ti(t.border), "{id:?}");
        assert_eq!(r.ink3, muted_text(&t), "{id:?}");
        assert_eq!(r.ink4, disabled_text(&t), "{id:?}");
        assert_eq!(r.accent_text, accent(&t), "{id:?}");
        assert_eq!(r.accent_soft, soft(t.accent), "{id:?}");
        assert_eq!(r.accent_soft_ink, ti(t.text), "{id:?}");
        assert_eq!(r.primary, accent_fill(&t), "{id:?}");
        assert_eq!(r.primary_ink, on_accent(&t), "{id:?}");
        assert_eq!(r.info, accent(&t), "{id:?}");
        assert_eq!(r.info_soft, soft(t.accent), "{id:?}");
        assert_eq!(r.ok_soft, soft(t.success), "{id:?}");
        assert_eq!(r.warn_soft, soft(t.warning), "{id:?}");
        assert_eq!(r.err_soft, soft(t.error), "{id:?}");
        assert_eq!(r.scrim, scrim(BACKDROP_ALPHA), "{id:?}");
    }
}

/// A Graphite theme resolves to exactly the colours it defines.
#[test]
fn graphite_themes_resolve_to_their_own_tokens() {
    for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let t = signex_types::theme::theme_tokens(id);
        let e = t.extended;
        let r = resolve(&t);
        let own = |c: Option<signex_types::theme::Color>| ti(c.expect("Graphite sets it"));
        assert_eq!(r.raised, own(e.raised), "{id:?}");
        assert_eq!(r.field, own(e.field), "{id:?}");
        assert_eq!(r.sunken, own(e.sunken), "{id:?}");
        assert_eq!(r.line_soft, own(e.line_soft), "{id:?}");
        assert_eq!(r.line_strong, own(e.line_strong), "{id:?}");
        assert_eq!(r.ink3, own(e.ink3), "{id:?}");
        assert_eq!(r.ink4, own(e.ink4), "{id:?}");
        assert_eq!(r.accent_text, own(e.accent_text), "{id:?}");
        assert_eq!(r.accent_soft, own(e.accent_soft), "{id:?}");
        assert_eq!(r.accent_soft_ink, own(e.accent_soft_ink), "{id:?}");
        assert_eq!(r.primary, own(e.primary), "{id:?}");
        assert_eq!(r.primary_ink, own(e.primary_ink), "{id:?}");
        assert_eq!(r.info, own(e.info), "{id:?}");
        assert_eq!(r.ok_soft, own(e.ok_soft), "{id:?}");
        assert_eq!(r.warn_soft, own(e.warn_soft), "{id:?}");
        assert_eq!(r.err_soft, own(e.err_soft), "{id:?}");
        assert_eq!(r.info_soft, own(e.info_soft), "{id:?}");
        assert_eq!(r.shadow, own(e.shadow), "{id:?}");
        assert_eq!(r.scrim, own(e.scrim), "{id:?}");
    }
}

/// The light/dark toggle must always change the look: the twins sit on
/// opposite sides of the lightness line.
#[test]
fn the_graphite_twins_are_one_light_and_one_dark() {
    assert!(is_light(&signex_types::theme::theme_tokens(
        ThemeId::GraphiteLight
    )));
    assert!(!is_light(&signex_types::theme::theme_tokens(
        ThemeId::GraphiteDark
    )));
}
