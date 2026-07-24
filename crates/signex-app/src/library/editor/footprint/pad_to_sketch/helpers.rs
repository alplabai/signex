//! Mint primitives shared across `mint_*_pad_geometry` functions.
//!
//! Each `push_*` helper allocates a fresh `SketchEntityId`, builds the
//! matching `Entity`, and pushes it onto `sketch.entities`. The
//! `_construction` variants set `entity.construction = true` so the
//! bake skips them. Together these collapse ~30 near-identical
//! 3-line blocks across the mint pipeline into single calls.

use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::PlaneId;
use signex_sketch::sketch::SketchData;

use super::super::state::EditorPad;

/// Push a non-construction `Point` entity at `(x, y)` and return its
/// fresh ID.
pub(super) fn push_point(
    sketch: &mut SketchData,
    plane_id: PlaneId,
    x: f64,
    y: f64,
) -> SketchEntityId {
    let id = SketchEntityId::new();
    sketch
        .entities
        .push(Entity::new(id, plane_id, EntityKind::Point { x, y }));
    id
}

/// Push a `Point` entity flagged `construction = true` and return its
/// fresh ID.
pub(super) fn push_construction_point(
    sketch: &mut SketchData,
    plane_id: PlaneId,
    x: f64,
    y: f64,
) -> SketchEntityId {
    let id = SketchEntityId::new();
    let mut entity = Entity::new(id, plane_id, EntityKind::Point { x, y });
    entity.construction = true;
    sketch.entities.push(entity);
    id
}

/// Push a non-construction `Line` entity referencing `(start, end)`
/// and return its fresh ID.
pub(super) fn push_line(
    sketch: &mut SketchData,
    plane_id: PlaneId,
    start: SketchEntityId,
    end: SketchEntityId,
) -> SketchEntityId {
    let id = SketchEntityId::new();
    sketch
        .entities
        .push(Entity::new(id, plane_id, EntityKind::Line { start, end }));
    id
}

/// Push a `Line` flagged `construction = true` and return its fresh ID.
pub(super) fn push_construction_line(
    sketch: &mut SketchData,
    plane_id: PlaneId,
    start: SketchEntityId,
    end: SketchEntityId,
) {
    let mut entity = Entity::new(
        SketchEntityId::new(),
        plane_id,
        EntityKind::Line { start, end },
    );
    entity.construction = true;
    sketch.entities.push(entity);
}

/// Push a non-construction `Arc` entity (CCW sweep) and return its
/// fresh ID.
pub(super) fn push_arc_ccw(
    sketch: &mut SketchData,
    plane_id: PlaneId,
    center: SketchEntityId,
    start: SketchEntityId,
    end: SketchEntityId,
) -> SketchEntityId {
    let id = SketchEntityId::new();
    sketch.entities.push(Entity::new(
        id,
        plane_id,
        EntityKind::Arc {
            center,
            start,
            end,
            sweep_ccw: true,
        },
    ));
    id
}

/// Mint the four bbox corner Points (`[ne, se, sw, nw]`) at the pad's
/// bbox extents. Used as the spine of every rectangular mint variant
/// (Rect / RoundRect / Oval / Chamfered).
pub(super) fn bbox_corner_points(
    sketch: &mut SketchData,
    plane_id: PlaneId,
    pad: &EditorPad,
) -> [SketchEntityId; 4] {
    let c = pad.rotated_corners_mm(); // [ne, se, sw, nw]
    std::array::from_fn(|i| push_point(sketch, plane_id, c[i].0, c[i].1))
}

/// Set an existing Point entity's coordinates by ID. Returns `true`
/// when the entity was found (and is a Point); `false` otherwise.
/// Shared between the move-mirror path and the post-solve mirrors.
pub(super) fn set_point_xy(sketch: &mut SketchData, id: SketchEntityId, x: f64, y: f64) -> bool {
    if let Some(entity) = sketch.entities.iter_mut().find(|e| e.id == id) {
        if let EntityKind::Point { x: ex, y: ey } = &mut entity.kind {
            *ex = x;
            *ey = y;
            return true;
        }
    }
    false
}

/// Bind a canonical shape-parameter key (e.g. `"corner_r"`,
/// `"diameter"`) to a freshly-named sketch parameter at value `expr`,
/// recording the binding on `pad.shape_params`. Returns the generated
/// parameter name (`"<key>_<centre-uuid-slug>"`).
pub(super) fn bind_shape_param(
    sketch: &mut SketchData,
    pad: &mut EditorPad,
    key: &str,
    centre_id: SketchEntityId,
    value_mm: f64,
) -> String {
    use super::attr::format_f64;
    let slug = super::attr::id_slug(centre_id);
    let name = format!("{key}_{slug}");
    sketch
        .parameters
        .insert(name.clone(), format!("{}mm", format_f64(value_mm)));
    pad.shape_params.insert(key.into(), name.clone());
    name
}
