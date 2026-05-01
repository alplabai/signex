//! Bus entry primitive — short angled stub joining a wire to a bus.
//!
//! A bus entry is anchored at `entry.position` and extends by
//! `entry.size` in `(dx, dy)` so the wire endpoint sits on the bus
//! line. Renders with the `theme.bus` colour. See
//! `signex-types::schematic::BusEntry`.
//!
//! Filled in by Wave 2 sub-agent 3.

use iced::widget::canvas::Frame;
use signex_types::schematic::BusEntry;

use super::RenderContext;

/// Render a single bus entry into the content layer's frame.
pub fn draw_bus_entry(_frame: &mut Frame, _entry: &BusEntry, _ctx: &RenderContext<'_>) {
    todo!("Wave 2 sub-agent 3 fills this in against signex-types::BusEntry")
}

#[cfg(test)]
mod tests {
    // Sub-agent populates: render_smoke, plus an edge case
    // (e.g. negative size components for the four diagonal orientations).
}
