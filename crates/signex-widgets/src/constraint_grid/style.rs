//! Colours, metrics and chrome text of the constraint grid and the class
//! matrix. Colours come from theme tokens; every size derives from the
//! text size, so the grid scales with the UI font.

use iced::Color;
use signex_types::theme::ThemeTokens;

use crate::Label;
use crate::theme_ext::to_color;

/// Colours and the text size of the grid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridStyle {
    pub text_size: f32,
    /// A value set on its own row, and row names.
    pub ink: Color,
    /// An inherited or default value.
    pub muted_ink: Color,
    /// Header and group-header fill.
    pub header_fill: Color,
    /// Cell and row rules.
    pub line: Color,
    /// Selected-cell ring and the found-column marker.
    pub accent: Color,
    /// Fill behind the selected row and the found column.
    pub accent_soft: Color,
    /// Text-input fill of the inline editor.
    pub field_fill: Color,
    /// Edge of an editor whose text does not parse.
    pub error: Color,
    /// Fill of an inert cell (the matrix's mirror half).
    pub inert_fill: Color,
}

impl GridStyle {
    /// The grid's colours from `tokens`, with the extended chrome tokens
    /// where the theme sets them and the base tokens otherwise.
    pub fn from_tokens(tokens: &ThemeTokens, text_size: f32) -> Self {
        let ext = &tokens.extended;
        let pick = |token: Option<signex_types::theme::Color>, base: signex_types::theme::Color| {
            to_color(&token.unwrap_or(base))
        };
        Self {
            text_size,
            ink: to_color(&tokens.text),
            muted_ink: pick(ext.ink3, tokens.text_secondary),
            header_fill: pick(ext.sunken, tokens.toolbar_bg),
            line: pick(ext.line_soft, tokens.border),
            accent: pick(ext.accent_text, tokens.accent),
            accent_soft: pick(ext.accent_soft, tokens.selection),
            field_fill: pick(ext.field, tokens.panel_bg),
            error: to_color(&tokens.error),
            inert_fill: pick(ext.sunken, tokens.hover),
        }
    }

    pub fn metrics(&self) -> GridMetrics {
        GridMetrics::for_text(self.text_size)
    }
}

/// Sizes of the grid in px, all derived from the text size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridMetrics {
    pub row_height: f32,
    /// Width of the object-name column.
    pub name_width: f32,
    /// Width of one value column.
    pub cell_width: f32,
    /// Width of a collapsed column group.
    pub collapsed_width: f32,
    /// Indent per hierarchy level.
    pub indent: f32,
    /// Inner horizontal padding of a cell.
    pub pad: f32,
    /// Side of the reset and chevron glyphs.
    pub glyph: f32,
}

/// Row height in em of the text size.
const ROW_EM: f32 = 2.0;
/// Object-name column width in em.
const NAME_EM: f32 = 15.0;
/// Value column width in em.
const CELL_EM: f32 = 7.5;
/// Collapsed group width in em.
const COLLAPSED_EM: f32 = 2.25;
/// Indent per level in em.
const INDENT_EM: f32 = 1.25;
/// Cell padding in em.
const PAD_EM: f32 = 0.5;

impl GridMetrics {
    pub fn for_text(text_size: f32) -> Self {
        Self {
            row_height: text_size * ROW_EM,
            name_width: text_size * NAME_EM,
            cell_width: text_size * CELL_EM,
            collapsed_width: text_size * COLLAPSED_EM,
            indent: text_size * INDENT_EM,
            pad: text_size * PAD_EM,
            glyph: crate::chevron::side(text_size),
        }
    }
}

/// The grid's own words, from the caller's message catalog.
#[derive(Debug, Clone)]
pub struct GridText<'a> {
    /// Header of the object-name column ("Object").
    pub object_header: Label<'a>,
    /// Tooltip line of a value set on its row ("Set on this row").
    pub set_here: Label<'a>,
    /// Tooltip line before the source row's name ("Inherited from").
    pub inherited_from: Label<'a>,
    /// Tooltip line of a column default ("Built-in default").
    pub built_in: Label<'a>,
    /// Tooltip of the reset glyph ("Reset to the inherited value").
    pub reset: Label<'a>,
    /// Placeholder of the find box ("Find a constraint").
    pub find_placeholder: Label<'a>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_scale_with_the_text() {
        let small = GridMetrics::for_text(11.0);
        let large = GridMetrics::for_text(14.0);
        assert!(large.row_height > small.row_height);
        assert!(large.cell_width > small.cell_width);
        assert_eq!(small.row_height, 11.0 * ROW_EM);
        assert!(small.collapsed_width < small.cell_width);
        assert!(small.name_width > small.cell_width);
    }
}
