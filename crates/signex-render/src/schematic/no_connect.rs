//! No-connect primitive — small "X" marker at an unconnected pin.
//!
//! Two short crossing strokes centred on `nc.position`. Stroke colour
//! `theme.no_connect`. See `signex-types::schematic::NoConnect`.
//!
//! Filled in by Wave 2 sub-agent 5.

use iced::widget::canvas::Frame;
use signex_types::schematic::NoConnect;

use super::RenderContext;

/// Render a single no-connect marker into the content layer's frame.
pub fn draw_no_connect(_frame: &mut Frame, _nc: &NoConnect, _ctx: &RenderContext<'_>) {
    todo!("Wave 2 sub-agent 5 fills this in against signex-types::NoConnect")
}

#[cfg(test)]
mod tests {
    // Sub-agent populates: render_smoke, plus an edge case
    // (e.g. several markers at adjacent positions).
}
