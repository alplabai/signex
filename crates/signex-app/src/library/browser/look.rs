//! Library Browser looks, drawn from the Graphite tokens. The grid, sidebar
//! and header ask here instead of hard-coding white overlays.

use iced::{Border, Color, Font};
use signex_types::theme::ThemeTokens;

use super::columns::ColumnKind;
use crate::styles::tokens::{self, Overlay, resolve};

/// Width of the Graphite "Lifecycle" pill column.
pub(super) const LIFECYCLE_PILL_W: f32 = 96.0;

pub(super) use crate::library::editor::kit::{lifecycle_label, lifecycle_tone};

/// The mono face for part numbers and values.
pub(super) fn cell_font(kind: &ColumnKind) -> Font {
    let mono = matches!(
        kind,
        ColumnKind::InternalPn | ColumnKind::Mpn | ColumnKind::Parameter(_)
    );
    if mono {
        crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
    } else {
        Font::default()
    }
}

/// Column-header band fill: sunken on Graphite.
pub(super) fn header_fill(tokens: &ThemeTokens) -> Color {
    resolve(tokens).sunken
}

/// Column-header label ink.
pub(super) fn header_ink(tokens: &ThemeTokens) -> Color {
    resolve(tokens).ink3
}

/// Hover wash on a row or cell.
pub(super) fn hover_fill(tokens: &ThemeTokens) -> Color {
    tokens::overlay(tokens, Overlay::Hover)
}

/// Fill of the active sidebar row.
pub(super) fn active_fill(tokens: &ThemeTokens) -> Color {
    resolve(tokens).accent_soft
}

/// Fill of the selected grid row.
pub(super) fn selected_fill(tokens: &ThemeTokens) -> Color {
    resolve(tokens).accent_soft
}

/// Wash of a deprecated row.
pub(super) fn deprecated_fill(tokens: &ThemeTokens) -> Color {
    resolve(tokens).warn_soft
}

/// Resting fill of a quiet bordered control (the old 4 % white wash).
pub(super) fn control_fill(tokens: &ThemeTokens) -> Color {
    resolve(tokens).field
}

/// Faint resting fill of an inert control (the old 2 % white wash).
pub(super) fn inert_fill(tokens: &ThemeTokens) -> Color {
    resolve(tokens).sunken
}

/// Border of a body row: a 1 px soft line below it.
pub(super) fn row_border(tokens: &ThemeTokens) -> Border {
    Border {
        color: resolve(tokens).line_soft,
        bottom: iced::border::Side::default().width(1.0),
        ..Border::default()
    }
}

/// Hover wash of a sidebar row (the old 4 % white wash).
pub(super) fn row_hover_fill(tokens: &ThemeTokens) -> Color {
    tokens::overlay(tokens, Overlay::Hover)
}

/// Ink on a wash: the theme text.
pub(super) fn ink_over_wash(tokens: &ThemeTokens) -> Color {
    crate::styles::ti(tokens.text)
}

/// Style of a confirm button (Create, rename tick): the theme primary.
pub(super) fn confirm_style(
    tokens: &ThemeTokens,
) -> impl Fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style + 'static {
    crate::styles::primary_button(tokens)
}

/// Style of an editable grid cell: plain text until hovered or
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
pub(super) fn control_radius() -> f32 {
    crate::styles::Radii::GRAPHITE.button
}

/// Height of the column-header row.
pub(super) fn header_height() -> f32 {
    30.0
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
