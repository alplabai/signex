//! Drawing primitives — sheet-level decorations (lines, rectangles,
//! circles, arcs, polylines) authored in the schematic editor's
//! drawing tool. See `signex-types::schematic::SchDrawing`.
//!
//! Draws each variant with stroke width and optional fill from the
//! domain enum, falling back to `theme.outline` (or the matching theme
//! token) when `stroke_color` is `None`.
//!
//! Filled in by Wave 2 sub-agent 7.

use iced::widget::canvas::Frame;
use signex_types::schematic::SchDrawing;

use super::RenderContext;

/// Render a single drawing primitive into the content layer's frame.
pub fn draw_drawing(_frame: &mut Frame, _drawing: &SchDrawing, _ctx: &RenderContext<'_>) {
    todo!("Wave 2 sub-agent 7 fills this in against signex-types::SchDrawing")
}

#[cfg(test)]
mod tests {
    // Sub-agent populates: render_smoke for each variant, plus an edge
    // case per variant (filled rect, three-point arc, polyline with
    // collinear points).
}
