//! Minimal 3D-body mint helpers. No wgpu — the CPU preview3d pane reads
//! `Footprint::body_3d` directly, so populating it is immediately visible.
//! Real interactive 3D manipulation stays deferred (v2.x,
//! docs/internal/docs/PCB_3D_RENDER_PLAN.md).

use signex_library::primitive::footprint::{Body3D, BodyShape, Footprint};

/// "3D Body" — extrude the courtyard outline into a solid box. Height and
/// colours come from `Body3D::default()` (no magic constants).
pub fn mint_box_from_courtyard(fp: &mut Footprint) {
    let mut body = Body3D::default(); // Extrude, grey, h = 1.0mm, z = 0
    body.shape = BodyShape::Extrude;
    body.outline = Some(fp.courtyard.clone());
    fp.body_3d = body;
}

/// "Extruded 3D Body" — extrude with no explicit outline so preview3d /
/// bake fall back to the fab outline (preview3d.rs:290 handles `None`).
pub fn mint_extruded_from_fab(fp: &mut Footprint) {
    let mut body = Body3D::default();
    body.shape = BodyShape::Extrude;
    body.outline = None;
    fp.body_3d = body;
}
