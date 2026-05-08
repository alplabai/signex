//! Sketch hit-test helpers — find the nearest sketch entity (or
//! Point only) under a click for the Select tool + auto-Coincident
//! snap behaviour.

use signex_sketch::entity::EntityKind;
use signex_sketch::id::SketchEntityId;

use super::FootprintCanvasState;
use super::geometry::screen_dist_to_segment_sq;

/// v0.13.2 — Snap radius in screen pixels. A click within this
/// distance of an existing sketch Point's screen position resolves
/// to that Point (auto-Coincident).
const SKETCH_SNAP_RADIUS_PX: f32 = 8.0;

/// v0.13.3 — Hit-test Lines / Arcs / Circles (everything that isn't
/// a Point — Points are caught by `sketch_snap`). Returns the
/// nearest entity within `SKETCH_SNAP_RADIUS_PX`. Used by the
/// Select tool so the user can grab line / arc / circle entities,
/// not just Points.
pub(super) fn sketch_hit_other(
    sketch: Option<&signex_sketch::SketchData>,
    cstate: &FootprintCanvasState,
    click_world: (f64, f64),
) -> Option<SketchEntityId> {
    let sketch = sketch?;
    let click_screen = cstate.world_to_screen(click_world);
    let radius_sq = SKETCH_SNAP_RADIUS_PX * SKETCH_SNAP_RADIUS_PX;
    let mut best: Option<(f32, SketchEntityId)> = None;

    let resolve_pt = |id: SketchEntityId| -> Option<(f64, f64)> {
        sketch
            .entities
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| match e.kind {
                EntityKind::Point { x, y } => Some((x, y)),
                _ => None,
            })
    };

    for entity in &sketch.entities {
        let world_dist_sq = match entity.kind {
            EntityKind::Line { start, end } => {
                let s = resolve_pt(start);
                let e = resolve_pt(end);
                match (s, e) {
                    (Some(s), Some(e)) => {
                        let p0 = cstate.world_to_screen(s);
                        let p1 = cstate.world_to_screen(e);
                        screen_dist_to_segment_sq(click_screen, p0, p1)
                    }
                    _ => continue,
                }
            }
            EntityKind::Circle { center, radius } => {
                let c = match resolve_pt(center) {
                    Some(c) => c,
                    None => continue,
                };
                let centre = cstate.world_to_screen(c);
                let dx = click_screen.x - centre.x;
                let dy = click_screen.y - centre.y;
                let dist = (dx * dx + dy * dy).sqrt();
                let r_screen = (radius as f32) * cstate.scale;
                let edge_dist = (dist - r_screen).abs();
                edge_dist * edge_dist
            }
            EntityKind::Arc { center, .. } => {
                let c = match resolve_pt(center) {
                    Some(c) => c,
                    None => continue,
                };
                let centre = cstate.world_to_screen(c);
                let dx = click_screen.x - centre.x;
                let dy = click_screen.y - centre.y;
                dx * dx + dy * dy
            }
            EntityKind::Point { .. } => continue,
        };
        if world_dist_sq <= radius_sq {
            match best {
                Some((b, _)) if b <= world_dist_sq => {}
                _ => best = Some((world_dist_sq, entity.id)),
            }
        }
    }
    best.map(|(_, id)| id)
}

/// Find the sketch Point whose screen position is within
/// `SKETCH_SNAP_RADIUS_PX` of the given world-mm click. Returns the
/// nearest-snap Point's `SketchEntityId`, or `None` if no Point is
/// in range. Used by the canvas to drive auto-Coincident behaviour
/// in multi-click drawing tools.
pub(super) fn sketch_snap(
    sketch: Option<&signex_sketch::SketchData>,
    cstate: &FootprintCanvasState,
    click_world: (f64, f64),
) -> Option<SketchEntityId> {
    let sketch = sketch?;
    let click_screen = cstate.world_to_screen(click_world);
    let radius_sq = SKETCH_SNAP_RADIUS_PX * SKETCH_SNAP_RADIUS_PX;
    let mut best: Option<(f32, SketchEntityId)> = None;
    for entity in &sketch.entities {
        if let EntityKind::Point { x, y } = entity.kind {
            let p = cstate.world_to_screen((x, y));
            let dx = p.x - click_screen.x;
            let dy = p.y - click_screen.y;
            let dist_sq = dx * dx + dy * dy;
            if dist_sq <= radius_sq {
                match best {
                    Some((b, _)) if b <= dist_sq => {}
                    _ => best = Some((dist_sq, entity.id)),
                }
            }
        }
    }
    best.map(|(_, id)| id)
}
