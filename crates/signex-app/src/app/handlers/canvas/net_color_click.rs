//! The net under a point on the active sheet: what a net-colour pen click and
//! the headless `color_net` command both colour.

use super::super::super::*;

/// Is (`px`, `py`) within 0.05 mm of the segment (`ax`, `ay`)-(`bx`, `by`)?
pub(crate) fn point_on_segment(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> bool {
    let dx = bx - ax;
    let dy = by - ay;
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((px - ax) * dx + (py - ay) * dy) / len2).clamp(0.0, 1.0)
    };
    let (cx, cy) = (ax + t * dx, ay + t * dy);
    let (ddx, ddy) = (px - cx, py - cy);
    (ddx * ddx + ddy * ddy).sqrt() < 0.05
}

/// The wire of `sheet` that passes through (`x`, `y`). The test is strict
/// (a wire directly under the pen): the general `hit_test` has a 1.5 mm
/// tolerance, so parallel wires on adjacent grid points could each register.
pub(crate) fn wire_at(
    sheet: &signex_types::schematic::SchematicSheet,
    x: f64,
    y: f64,
) -> Option<uuid::Uuid> {
    sheet.wires.iter().find_map(|w| {
        point_on_segment(x, y, w.start.x, w.start.y, w.end.x, w.end.y).then_some(w.uuid)
    })
}

/// The wires and junctions of the net whose wire passes through (`x`, `y`)
/// on `sheet`; empty when no wire is there. Net membership comes from the
/// connectivity core (`signex_net::flood_net_elements`), the one build_netlist
/// and ERC read.
pub(crate) fn net_elements_in(
    sheet: &signex_types::schematic::SchematicSheet,
    x: f64,
    y: f64,
) -> Vec<uuid::Uuid> {
    wire_at(sheet, x, y)
        .and_then(|wire| signex_net::flood_net_elements(sheet, wire))
        .map(|f| f.wires.into_iter().chain(f.junctions).collect())
        .unwrap_or_default()
}

impl Signex {
    /// The net under (`hit_x`, `hit_y`) on the active sheet, see
    /// [`net_elements_in`].
    pub(crate) fn net_elements_at(&self, hit_x: f64, hit_y: f64) -> Vec<uuid::Uuid> {
        self.active_render_snapshot()
            .map(|snapshot| net_elements_in(snapshot, hit_x, hit_y))
            .unwrap_or_default()
    }
}
