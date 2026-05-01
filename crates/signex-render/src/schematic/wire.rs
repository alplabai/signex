//! Wire primitive — straight strokes between two endpoints.
//!
//! Wires render as a single line from `wire.start` to `wire.end` using
//! the `theme.wire` colour and `wire.stroke_width` (or the schematic
//! default ~0.15 mm when `0.0`). See `docs/RENDERING_RULES.md` and
//! `crates/signex-types/src/schematic.rs::Wire` for the domain shape.
//!
//! Filled in by Wave 2 sub-agent 1.

use iced::widget::canvas::Frame;
use signex_types::schematic::Wire;

use super::RenderContext;

/// Render a single wire into the content layer's frame.
pub fn draw_wire(_frame: &mut Frame, _wire: &Wire, _ctx: &RenderContext<'_>) {
    todo!("Wave 2 sub-agent 1 fills this in against signex-types::Wire")
}

#[cfg(test)]
mod tests {
    // Sub-agent populates: render_smoke, edge case (e.g. zero-length wire,
    // user stroke override).
}
