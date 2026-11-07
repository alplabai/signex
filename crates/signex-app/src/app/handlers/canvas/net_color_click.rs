//! The net under a point on the active sheet: what a net-colour pen click and
//! the headless `color_net` command both colour.

use super::super::super::*;

/// Is (`px`, `py`) within 0.05 mm of the segment (`ax`, `ay`)-(`bx`, `by`)?
fn point_on_segment(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> bool {
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

impl Signex {
    /// The wires and junctions of the net whose wire passes through
    /// (`hit_x`, `hit_y`); empty when no wire is there. The test is strict
    /// (a wire directly under the pen): the general `hit_test` has a 1.5 mm
    /// tolerance, so parallel wires on adjacent grid points could each
    /// register. Net membership comes from the connectivity core
    /// (`signex_net::flood_net_elements`), the one build_netlist and ERC read.
    pub(crate) fn net_elements_at(&self, hit_x: f64, hit_y: f64) -> Vec<uuid::Uuid> {
        let Some(snapshot) = self.active_render_snapshot() else {
            return Vec::new();
        };
        let hit = snapshot.wires.iter().find_map(|w| {
            point_on_segment(hit_x, hit_y, w.start.x, w.start.y, w.end.x, w.end.y).then_some(w.uuid)
        });
        hit.and_then(|wire| signex_net::flood_net_elements(snapshot, wire))
            .map(|f| f.wires.into_iter().chain(f.junctions).collect())
            .unwrap_or_default()
    }
}
