//! #1769 - labels painted on an accent or selection fill must stay readable
//! on the light built-in theme, and the dark themes must not change.

use super::form_rows::check_state_text;
use super::general::{add_filter_button_style, seg_text};
use crate::app::Signex;
use crate::panels::palette::PanelPalette;
use crate::panels::status::erc_row_text;
use crate::styles::ti;
use crate::styles::tokens::{MIN_TEXT_CONTRAST, UNREADABLE_CONTRAST, contrast_ratio};
use iced::Color;
use iced::widget::button;
use signex_types::theme::{ThemeId, theme_tokens};

const LIGHT: ThemeId = ThemeId::SolarizedLight;

fn dark_themes() -> impl Iterator<Item = ThemeId> {
    ThemeId::BUILTINS.iter().copied().filter(|id| *id != LIGHT)
}

fn solid(fill: Option<iced::Background>) -> Color {
    match fill {
        Some(iced::Background::Color(c)) => c,
        other => panic!("expected a solid fill, got {other:?}"),
    }
}

fn assert_readable(what: &str, text: Color, fill: Color) {
    let ratio = contrast_ratio(text, fill);
    println!("{what}: {ratio:.2}");
    assert!(
        ratio >= MIN_TEXT_CONTRAST,
        "{what}: contrast {ratio:.2} < {MIN_TEXT_CONTRAST}"
    );
}

#[test]
fn selected_segment_text_is_readable_on_solarized_light() {
    let p = PanelPalette::from_tokens(&theme_tokens(LIGHT));
    let text = seg_text(true, p.primary, p.muted, p.input_bg);
    assert_readable("selected segment", text, p.input_bg);
}

#[test]
fn add_filter_label_is_readable_on_solarized_light() {
    let tokens = theme_tokens(LIGHT);
    let p = PanelPalette::from_tokens(&tokens);
    let theme = Signex::id_to_iced_theme(LIGHT, None);
    for status in [
        button::Status::Active,
        button::Status::Hovered,
        button::Status::Pressed,
    ] {
        let style = add_filter_button_style(&theme, status, p.primary);
        assert_readable(
            &format!("+ Add Filter {status:?}"),
            style.text_color,
            solid(style.background),
        );
    }
}

#[test]
fn checkbox_state_word_is_readable_on_solarized_light() {
    let tokens = theme_tokens(LIGHT);
    let p = PanelPalette::from_tokens(&tokens);
    let theme = Signex::id_to_iced_theme(LIGHT, None);
    let surfaces = [
        ti(tokens.panel_bg),
        theme.extended_palette().background.base.color,
    ];
    for surface in surfaces {
        assert_readable("On", check_state_text(true, p.muted, surface), surface);
    }
}

#[test]
fn selected_erc_row_text_is_readable_on_solarized_light() {
    let tokens = theme_tokens(LIGHT);
    let fill = ti(tokens.selection);
    let (label, detail) = erc_row_text(&tokens, true);
    assert_readable("ERC row label", label, fill);
    assert_readable("ERC row detail", detail, fill);
}

/// A dark theme keeps the colour it had, unless that colour was lost on
/// its fill (below `UNREADABLE_CONTRAST`), where it must now read.
fn assert_unchanged_unless_lost(what: &str, legacy: Color, resolved: Color, fill: Color) {
    let before = contrast_ratio(legacy, fill);
    if before >= UNREADABLE_CONTRAST {
        assert_eq!(resolved, legacy, "{what}: a visible label must not change");
    } else {
        println!(
            "{what}: was {before:.2}, now {:.2}",
            contrast_ratio(resolved, fill)
        );
        assert!(
            contrast_ratio(resolved, fill) >= MIN_TEXT_CONTRAST,
            "{what}"
        );
    }
}

#[test]
fn dark_themes_keep_every_label_that_was_visible() {
    for id in dark_themes() {
        let tokens = theme_tokens(id);
        let p = PanelPalette::from_tokens(&tokens);
        let theme = Signex::id_to_iced_theme(id, None);
        let surface = theme.extended_palette().background.base.color;

        assert_unchanged_unless_lost(
            &format!("{id:?} active segment"),
            p.primary,
            seg_text(true, p.primary, p.muted, p.input_bg),
            p.input_bg,
        );
        assert_eq!(
            seg_text(false, p.primary, p.muted, p.input_bg),
            p.muted,
            "{id:?} inactive segment"
        );
        assert_unchanged_unless_lost(
            &format!("{id:?} On"),
            Color::WHITE,
            check_state_text(true, p.muted, surface),
            surface,
        );
        assert_unchanged_unless_lost(
            &format!("{id:?} Off"),
            p.muted,
            check_state_text(false, p.muted, surface),
            surface,
        );
        let style = add_filter_button_style(&theme, button::Status::Active, p.primary);
        assert_unchanged_unless_lost(
            &format!("{id:?} + Add Filter"),
            p.primary,
            style.text_color,
            solid(style.background),
        );
        let fill = ti(tokens.selection);
        let (label, detail) = erc_row_text(&tokens, true);
        assert_unchanged_unless_lost(&format!("{id:?} ERC label"), ti(tokens.text), label, fill);
        assert_unchanged_unless_lost(
            &format!("{id:?} ERC detail"),
            ti(tokens.text_secondary),
            detail,
            fill,
        );
    }
}
