//! Bus primitive — thicker strokes between two endpoints.
//!
//! Buses share their geometry with [`super::wire`] but render with a
//! heavier stroke width and the `theme.bus` colour. See
//! `docs/RENDERING_RULES.md` and `signex-types::schematic::Bus`.
//!
//! Filled in by Wave 2 sub-agent 2.

use iced::widget::canvas::Frame;
use signex_types::schematic::Bus;

use super::RenderContext;

/// Render a single bus into the content layer's frame.
pub fn draw_bus(_frame: &mut Frame, _bus: &Bus, _ctx: &RenderContext<'_>) {
    todo!("Wave 2 sub-agent 2 fills this in against signex-types::Bus")
}

#[cfg(test)]
mod tests {
    // Sub-agent populates: render_smoke, plus an edge case
    // (e.g. zero-length bus, vertical vs horizontal).
}
