//! Library Browser looks that differ on Graphite themes.
//!
//! Every helper returns the legacy value for a non-Graphite theme, so the
//! legacy themes keep their exact look. The grid, sidebar and header ask
//! here instead of hard-coding white overlays.

use iced::{Border, Color, Font};
use signex_types::theme::ThemeTokens;

use super::columns::ColumnKind;
use crate::styles::is_graphite;
use crate::styles::tokens::{self, Overlay, resolve};

/// Width of the Graphite "Lifecycle" pill column.
pub(super) const LIFECYCLE_PILL_W: f32 = 96.0;

pub(super) use crate::library::editor::kit::{lifecycle_label, lifecycle_tone};

/// The mono face for part numbers and values (Graphite only; elsewhere the
/// default face, so nothing changes).
pub(super) fn cell_font(kind: &ColumnKind, tokens: &ThemeTokens) -> Font {
    let mono = matches!(
        kind,
        ColumnKind::InternalPn | ColumnKind::Mpn | ColumnKind::Parameter(_)
    );
    if mono && is_graphite(tokens) {
        crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
    } else {
        Font::default()
    }
}

/// Column-header band fill: sunken on Graphite.
pub(super) fn header_fill(tokens: &ThemeTokens) -> Color {
    if is_graphite(tokens) {
        resolve(tokens).sunken
    } else {
        Color::from_rgba(1.0, 1.0, 1.0, 0.04)
    }
}

/// Column-header label ink.
pub(super) fn header_ink(tokens: &ThemeTokens) -> Color {
    if is_graphite(tokens) {
        resolve(tokens).ink3
    } else {
        Color::from_rgba(1.0, 1.0, 1.0, 0.65)
    }
}

/// Hover wash on a row or cell.
pub(super) fn hover_fill(tokens: &ThemeTokens) -> Color {
    if is_graphite(tokens) {
        tokens::overlay(tokens, Overlay::Hover)
    } else {
        Color::from_rgba(1.0, 1.0, 1.0, 0.06)
    }
}

/// Fill of the active sidebar row.
pub(super) fn active_fill(tokens: &ThemeTokens) -> Color {
    if is_graphite(tokens) {
        resolve(tokens).accent_soft
    } else {
        Color::from_rgba(1.0, 1.0, 1.0, 0.10)
    }
}

/// Fill of the selected grid row.
pub(super) fn selected_fill(tokens: &ThemeTokens) -> Color {
    if is_graphite(tokens) {
        resolve(tokens).accent_soft
    } else {
        Color::from_rgba(0.30, 0.55, 0.85, 0.25)
    }
}

/// Wash of a deprecated row.
pub(super) fn deprecated_fill(tokens: &ThemeTokens) -> Color {
    if is_graphite(tokens) {
        resolve(tokens).warn_soft
    } else {
        Color::from_rgba(0.96, 0.80, 0.10, 0.10)
    }
}

/// Resting fill of a quiet bordered control (the old 4 % white wash).
pub(super) fn control_fill(tokens: &ThemeTokens) -> Color {
    if is_graphite(tokens) {
        resolve(tokens).field
    } else {
        Color::from_rgba(1.0, 1.0, 1.0, 0.04)
    }
}

/// Faint resting fill of an inert control (the old 2 % white wash).
pub(super) fn inert_fill(tokens: &ThemeTokens) -> Color {
    if is_graphite(tokens) {
        resolve(tokens).sunken
    } else {
        Color::from_rgba(1.0, 1.0, 1.0, 0.02)
    }
}

/// Border of a body row: a 1 px soft line below it on Graphite, none else.
pub(super) fn row_border(tokens: &ThemeTokens) -> Border {
    if !is_graphite(tokens) {
        return Border::default();
    }
    Border {
        color: resolve(tokens).line_soft,
        bottom: iced::border::Side::default().width(1.0),
        ..Border::default()
    }
}

/// Hover wash of a sidebar row (the old 4 % white wash).
pub(super) fn row_hover_fill(tokens: &ThemeTokens) -> Color {
    if is_graphite(tokens) {
        tokens::overlay(tokens, Overlay::Hover)
    } else {
        Color::from_rgba(1.0, 1.0, 1.0, 0.04)
    }
}

/// Ink on a wash: the theme text on Graphite, white on the legacy dark
/// themes that the washes were tuned for.
pub(super) fn ink_over_wash(tokens: &ThemeTokens) -> Color {
    if is_graphite(tokens) {
        crate::styles::ti(tokens.text)
    } else {
        Color::WHITE
    }
}

/// Style of a confirm button (Create, rename tick): the theme primary on
/// Graphite, the legacy blue fill elsewhere with `legacy_radius`.
pub(super) fn confirm_style(
    tokens: &ThemeTokens,
    legacy_radius: f32,
) -> impl Fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style + 'static {
    let graphite = is_graphite(tokens);
    let primary = crate::styles::primary_button(tokens);
    move |theme, status| {
        if graphite {
            return primary(theme, status);
        }
        iced::widget::button::Style {
            background: Some(iced::Background::Color(Color::from_rgb(0.18, 0.36, 0.58))),
            text_color: Color::WHITE,
            border: Border {
                radius: legacy_radius.into(),
                ..Border::default()
            },
            ..iced::widget::button::Style::default()
        }
    }
}

/// Style of an editable grid cell on Graphite: plain text until hovered or
/// focused, then the field frame. Typing, focus and commit are unchanged.
pub(super) fn plain_cell_input(
    tokens: &ThemeTokens,
) -> impl Fn(&iced::Theme, iced::widget::text_input::Status) -> iced::widget::text_input::Style + 'static
{
    use iced::widget::text_input::{Status, Style};
    let r = resolve(tokens);
    let ink = crate::styles::ti(tokens.text);
    let accent = crate::styles::ti(tokens.accent);
    let radius = crate::styles::Radii::GRAPHITE.control;
    move |_, status| {
        let (fill, line, width) = match status {
            Status::Focused { .. } => (r.field, accent, 1.0),
            Status::Hovered => (r.field, r.line_soft, 1.0),
            _ => (Color::TRANSPARENT, Color::TRANSPARENT, 0.0),
        };
        Style {
            background: iced::Background::Color(fill),
            border: Border {
                width,
                color: line,
                radius: radius.into(),
                ..Border::default()
            },
            placeholder: r.ink4,
            value: ink,
            selection: Color { a: 0.4, ..accent },
        }
    }
}

/// Corner radius of a small bordered control.
pub(super) fn control_radius(tokens: &ThemeTokens) -> f32 {
    if is_graphite(tokens) {
        crate::styles::Radii::GRAPHITE.button
    } else {
        3.0
    }
}

/// Height of the column-header row: 30 px on Graphite, the legacy height
/// elsewhere.
pub(super) fn header_height(tokens: &ThemeTokens) -> f32 {
    if is_graphite(tokens) {
        30.0
    } else {
        super::window::BROWSER_HEADER_HEIGHT
    }
}

/// Text-input style for a field that sits inside a framed container: no
/// fill or border of its own, so the container's Graphite field look shows.
pub(super) fn bare_input(
    tokens: &ThemeTokens,
) -> impl Fn(&iced::Theme, iced::widget::text_input::Status) -> iced::widget::text_input::Style + 'static
{
    use iced::widget::text_input::Style;
    let r = resolve(tokens);
    let ink = crate::styles::ti(tokens.text);
    let accent = crate::styles::ti(tokens.accent);
    move |_, _| Style {
        background: iced::Background::Color(Color::TRANSPARENT),
        border: Border::default(),
        placeholder: r.ink4,
        value: ink,
        selection: Color { a: 0.4, ..accent },
    }
}
