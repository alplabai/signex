//! Free text-note rendering + shared text mechanics for the schematic
//! primitives that paint glyphs (labels, fields, drawing text).
//!
//! Text is anchored at `note.position` with rotation `note.rotation`
//! and size `note.font_size` (mm). The renderer converts mm font
//! sizes to iced em-square pixels via `crate::SCHEMATIC_TEXT_EM_MM`.
//! See `signex-types::schematic::TextNote` and the canvas-text helpers
//! in `crate::lib`.
//!
//! Filled in by Wave 2 sub-agent 6. Sub-modules `label`, `field_style`,
//! `symbol` (for body text), and `drawing` (for `Graphic::Text` /
//! `TextBox`) all depend on the helpers exported here.

use iced::widget::canvas::Frame;
use signex_types::schematic::TextNote;

use super::RenderContext;

/// Render a single free text note into the content layer's frame.
pub fn draw_text_note(_frame: &mut Frame, _note: &TextNote, _ctx: &RenderContext<'_>) {
    todo!("Wave 2 sub-agent 6 fills this in against signex-types::TextNote")
}

#[cfg(test)]
mod tests {
    // Sub-agent populates: render_smoke, plus edge cases (rotated text,
    // hidden text, multi-line).
}
