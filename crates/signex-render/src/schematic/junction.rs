//! Junction primitive — filled disc at a wire intersection.
//!
//! Drawn as a single filled circle at `junction.position`. Diameter
//! comes from `junction.diameter` when non-zero, else the schematic
//! default scaled with the active grid step. Colour is `theme.junction`.
//! See `signex-types::schematic::Junction`.
//!
//! Filled in by Wave 2 sub-agent 4.

use iced::widget::canvas::Frame;
use signex_types::schematic::Junction;

use super::RenderContext;

/// Render a single junction into the content layer's frame.
pub fn draw_junction(_frame: &mut Frame, _junction: &Junction, _ctx: &RenderContext<'_>) {
    todo!("Wave 2 sub-agent 4 fills this in against signex-types::Junction")
}

#[cfg(test)]
mod tests {
    // Sub-agent populates: render_smoke, plus an edge case
    // (e.g. user-overridden diameter).
}
