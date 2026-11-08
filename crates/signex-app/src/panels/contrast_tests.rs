//! #1769 - labels painted on an accent or selection fill must stay readable
//! on the light built-in theme, and the dark themes must not change.

use super::palette::PanelPalette;
use super::properties_parameters::{add_filter_button_style, check_state_color, seg_text};
use super::status::{erc_row_text, erc_severity_text};
use crate::app::Signex;
use crate::styles::ti;
use crate::styles::tokens::{
    MIN_TEXT_CONTRAST, UNREADABLE_CONTRAST, contrast_ratio, relative_luminance,
};
use iced::Color;
use iced::widget::button;
use signex_types::theme::{ThemeId, ThemeTokens, theme_tokens};

/// A theme is light when its panel is light, whatever its name.
fn is_light(id: ThemeId) -> bool {
    relative_luminance(ti(theme_tokens(id).panel_bg)) > 0.5
}

fn light_themes() -> impl Iterator<Item = ThemeId> {
    ThemeId::BUILTINS.iter().copied().filter(|id| is_light(*id))
}

fn dark_themes() -> impl Iterator<Item = ThemeId> {
    ThemeId::BUILTINS
        .iter()
        .copied()
        .filter(|id| !is_light(*id))
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
fn selected_segment_text_is_readable_on_every_light_theme() {
    for id in light_themes() {
        let p = PanelPalette::from_tokens(&theme_tokens(id));
        let text = seg_text(true, p.primary, p.muted, p.input_bg);
        assert_readable(&format!("{id:?} selected segment"), text, p.input_bg);
    }
}

/// The retired Solarized Light tokens and the iced theme they drew with: the
/// light palette whose accent equals its selection fill, the collision the
/// label rules below exist for. No shipped theme has it any more, but the
/// rules stay, so they are pinned on these inputs.
fn collision_tokens() -> ThemeTokens {
    let c = |hex: &str| signex_types::theme::Color::from_hex(hex);
    ThemeTokens {
        bg: c("#FDF6E3"),
        paper: c("#EEE8D5"),
        text: c("#657B83"),
        text_secondary: c("#93A1A1"),
        accent: c("#268BD2"),
        border: c("#93A1A1"),
        panel_bg: c("#FDF6E3"),
        toolbar_bg: c("#EEE8D5"),
        statusbar_bg: c("#EEE8D5"),
        selection: c("#268BD2"),
        hover: c("#EEE8D5"),
        error: c("#DC322F"),
        warning: c("#CB4B16"),
        success: c("#859900"),
        extended: signex_types::theme::ExtendedTokens::NONE,
    }
}

#[test]
fn add_filter_label_is_readable_on_the_accent_selection_collision() {
    let tokens = collision_tokens();
    assert_eq!(tokens.accent, tokens.selection, "fixture precondition");
    let p = PanelPalette::from_tokens(&tokens);
    let theme = iced::Theme::Light;
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
fn checkbox_state_word_is_readable_on_every_light_theme() {
    for id in light_themes() {
        let tokens = theme_tokens(id);
        let p = PanelPalette::from_tokens(&tokens);
        let theme = Signex::id_to_iced_theme(id);
        let surfaces = [ti(tokens.panel_bg), theme.palette().background.base.color];
        for surface in surfaces {
            assert_readable(
                &format!("{id:?} On"),
                check_state_color(true, p.muted, surface),
                surface,
            );
        }
    }
}

#[test]
fn selected_erc_row_text_is_readable_on_every_light_theme() {
    for id in light_themes() {
        let tokens = theme_tokens(id);
        let fill = ti(tokens.selection);
        let (label, detail) = erc_row_text(&tokens, true);
        assert_readable(&format!("{id:?} ERC row label"), label, fill);
        assert_readable(&format!("{id:?} ERC row detail"), detail, fill);
    }
}

#[test]
fn the_light_and_dark_sets_split_the_builtins() {
    assert!(light_themes().any(|id| id == ThemeId::GraphiteLight));
    assert!(dark_themes().any(|id| id == ThemeId::GraphiteDark));
    assert_eq!(
        light_themes().count() + dark_themes().count(),
        ThemeId::BUILTINS.len()
    );
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
        let theme = Signex::id_to_iced_theme(id);
        let surface = theme.palette().background.base.color;

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
            check_state_color(true, p.muted, surface),
            surface,
        );
        assert_unchanged_unless_lost(
            &format!("{id:?} Off"),
            p.muted,
            check_state_color(false, p.muted, surface),
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

#[test]
fn focused_erc_severity_letter_is_readable_on_the_accent_selection_collision() {
    use signex_widgets::theme_ext;
    let tokens = collision_tokens();
    let fill = ti(tokens.selection);
    for (name, colour) in [
        ("E", theme_ext::error_color(&tokens)),
        ("W", theme_ext::warning_color(&tokens)),
        ("I", theme_ext::accent(&tokens)),
    ] {
        assert_readable(
            &format!("ERC severity {name}"),
            erc_severity_text(&tokens, true, colour),
            fill,
        );
        assert_eq!(erc_severity_text(&tokens, false, colour), colour);
    }
}

#[test]
fn add_filter_label_never_flips_between_states_on_any_builtin() {
    for &id in ThemeId::BUILTINS {
        let p = PanelPalette::from_tokens(&theme_tokens(id));
        let theme = Signex::id_to_iced_theme(id);
        let resting = add_filter_button_style(&theme, button::Status::Active, p.primary);
        for status in [button::Status::Hovered, button::Status::Pressed] {
            let style = add_filter_button_style(&theme, status, p.primary);
            assert_eq!(style.text_color, resting.text_color, "{id:?} {status:?}");
        }
    }
}

#[test]
fn on_label_reads_on_the_panel_background_of_every_builtin() {
    for &id in ThemeId::BUILTINS {
        let tokens = theme_tokens(id);
        let p = PanelPalette::from_tokens(&tokens);
        let theme = Signex::id_to_iced_theme(id);
        let on = check_state_color(true, p.muted, theme.palette().background.base.color);
        let panel = ti(tokens.panel_bg);
        assert!(
            contrast_ratio(on, panel) >= UNREADABLE_CONTRAST,
            "{id:?}: On is lost on the panel background"
        );
        if is_light(id) {
            assert_readable("On on panel_bg", on, panel);
        }
    }
}

#[test]
fn white_on_accent_segments_are_unchanged_on_every_builtin() {
    // The multi-selection justify and rotation rows pass white text on the
    // raw accent fill.
    for &id in ThemeId::BUILTINS {
        let tokens = theme_tokens(id);
        let accent = ti(tokens.accent);
        let muted = ti(tokens.text_secondary);
        assert_eq!(
            seg_text(true, Color::WHITE, muted, accent),
            Color::WHITE,
            "{id:?}"
        );
        assert_eq!(
            seg_text(false, Color::WHITE, muted, accent),
            muted,
            "{id:?}"
        );
    }
}

#[test]
fn dark_add_filter_label_is_unchanged_in_every_button_state() {
    for id in dark_themes() {
        let p = PanelPalette::from_tokens(&theme_tokens(id));
        let theme = Signex::id_to_iced_theme(id);
        let resting = add_filter_button_style(&theme, button::Status::Active, p.primary);
        let fill = solid(resting.background);
        if contrast_ratio(p.primary, fill) < UNREADABLE_CONTRAST {
            continue; // the corrected case, covered above
        }
        for status in [
            button::Status::Active,
            button::Status::Hovered,
            button::Status::Pressed,
        ] {
            let style = add_filter_button_style(&theme, status, p.primary);
            assert_eq!(style.text_color, p.primary, "{id:?} {status:?}");
        }
    }
}
