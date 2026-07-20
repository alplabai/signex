use super::*;
use signex_library::primitive::footprint::Footprint;
use signex_library::primitive::footprint::PadShape as LibPadShape;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::SketchData;

fn editor_pad(number: &str, x: f64, y: f64) -> EditorPad {
    let mut p = EditorPad::new_default(number.into(), (x, y));
    p.size_mm = (1.0, 0.5);
    p
}

#[test]
fn empty_pads_mint_nothing() {
    let mut fp = Footprint::empty("test");
    let mut pads: Vec<EditorPad> = Vec::new();
    let n = auto_mint_for_literal_pads(&mut pads, &mut fp);
    assert_eq!(n, 0);
    assert!(fp.sketch.is_none() || fp.sketch.as_ref().unwrap().entities.is_empty());
}

#[test]
fn three_pads_mint_three_points_with_pad_attrs() {
    let mut fp = Footprint::empty("test");
    let mut pads = vec![
        editor_pad("1", 0.0, 0.0),
        editor_pad("2", 1.27, 0.0),
        editor_pad("3", 2.54, 0.0),
    ];
    let n = auto_mint_for_literal_pads(&mut pads, &mut fp);
    assert_eq!(n, 3);
    let sketch = fp.sketch.as_ref().unwrap();
    assert_eq!(sketch.planes.len(), 1);
    // v0.16 — per pad: 1 centre Point + 4 corner Points + 4 outline
    // Lines = 9 entities. 3 pads × 9 = 27.
    assert_eq!(sketch.entities.len(), 27);
    let attr_carriers: Vec<&Entity> = sketch.entities.iter().filter(|e| e.pad.is_some()).collect();
    assert_eq!(attr_carriers.len(), 3);
    for entity in attr_carriers {
        assert!(matches!(entity.kind, EntityKind::Point { .. }));
        assert!(!entity.construction);
        let attr = entity.pad.as_ref().unwrap();
        assert!(!attr.number.is_empty());
        assert_eq!(attr.size_x_expr, "1mm");
        assert_eq!(attr.size_y_expr, "0.5mm");
    }
    for pad in &pads {
        assert!(pad.sketch_entity_id.is_some());
        assert!(pad.corner_entity_ids.is_some());
    }
}

#[test]
fn skip_when_sketch_already_has_entities() {
    let mut fp = Footprint::empty("test");
    let mut sketch = SketchData::default();
    let plane = Plane {
        id: PlaneId::new(),
        kind: PlaneKind::BoardTop,
    };
    sketch.planes.push(plane.clone());
    sketch.entities.push(Entity::new(
        SketchEntityId::new(),
        plane.id,
        EntityKind::Point { x: 0.0, y: 0.0 },
    ));
    fp.sketch = Some(sketch);

    let mut pads = vec![editor_pad("1", 0.0, 0.0)];
    let n = auto_mint_for_literal_pads(&mut pads, &mut fp);
    assert_eq!(n, 0, "auto-mint must skip when sketch is already populated");
    assert_eq!(fp.sketch.as_ref().unwrap().entities.len(), 1);
    assert!(
        pads[0].sketch_entity_id.is_none(),
        "skip leaves the link unset"
    );
}

#[test]
fn skip_when_sketch_only_has_construction_entities() {
    let mut fp = Footprint::empty("test");
    let mut sketch = SketchData::default();
    let plane = Plane {
        id: PlaneId::new(),
        kind: PlaneKind::BoardTop,
    };
    let plane_id = plane.id;
    sketch.planes.push(plane);
    let mut construction = Entity::new(
        SketchEntityId::new(),
        plane_id,
        EntityKind::Point { x: 0.0, y: 0.0 },
    );
    construction.construction = true;
    sketch.entities.push(construction);
    fp.sketch = Some(sketch);

    let mut pads = vec![editor_pad("1", 0.0, 0.0)];
    let n = auto_mint_for_literal_pads(&mut pads, &mut fp);
    assert_eq!(n, 1);
    // Pre-existing construction (1) + minted centre (1) + 4 corner
    // Points + 4 outline Lines = 10.
    assert_eq!(fp.sketch.as_ref().unwrap().entities.len(), 10);
    assert!(pads[0].sketch_entity_id.is_some());
    assert!(pads[0].corner_entity_ids.is_some());
}

#[test]
fn mirror_add_pad_links_to_new_sketch_entity() {
    let mut fp = Footprint::empty("test");
    let mut pad = editor_pad("X", 5.0, 5.0);
    assert!(pad.sketch_entity_id.is_none());
    mirror_add_pad_to_sketch(&mut pad, &mut fp);
    let id = pad.sketch_entity_id.expect("mirror should mint id");
    let sketch = fp.sketch.as_ref().unwrap();
    let entity = sketch
        .entities
        .iter()
        .find(|e| e.id == id)
        .expect("entity exists");
    match entity.kind {
        EntityKind::Point { x, y } => assert_eq!((x, y), (5.0, 5.0)),
        _ => panic!("minted entity must be a Point"),
    }
    assert!(entity.pad.is_some(), "Point should carry PadAttr");
}

#[test]
fn mirror_add_pad_with_existing_link_is_noop() {
    let mut fp = Footprint::empty("test");
    let mut pad = editor_pad("X", 0.0, 0.0);
    pad.sketch_entity_id = Some(SketchEntityId::new());
    mirror_add_pad_to_sketch(&mut pad, &mut fp);
    assert!(fp.sketch.is_none() || fp.sketch.as_ref().unwrap().entities.is_empty());
}

#[test]
fn mirror_move_pad_updates_sketch_point() {
    let mut fp = Footprint::empty("test");
    let mut pad = editor_pad("X", 0.0, 0.0);
    mirror_add_pad_to_sketch(&mut pad, &mut fp);
    pad.position_mm = (3.5, 7.25);
    mirror_move_pad_in_sketch(&pad, &mut fp);
    let id = pad.sketch_entity_id.unwrap();
    let entity = fp
        .sketch
        .as_ref()
        .unwrap()
        .entities
        .iter()
        .find(|e| e.id == id)
        .unwrap();
    match entity.kind {
        EntityKind::Point { x, y } => assert_eq!((x, y), (3.5, 7.25)),
        _ => panic!("entity must still be a Point"),
    }
}

#[test]
fn mirror_delete_pad_drops_sketch_entity() {
    let mut fp = Footprint::empty("test");
    let mut pad = editor_pad("X", 0.0, 0.0);
    mirror_add_pad_to_sketch(&mut pad, &mut fp);
    // v0.16 — 1 centre + 4 corners + 4 lines = 9.
    assert_eq!(fp.sketch.as_ref().unwrap().entities.len(), 9);
    mirror_delete_pad_from_sketch(&pad, &mut fp);
    assert_eq!(fp.sketch.as_ref().unwrap().entities.len(), 0);
}

/// Build a footprint whose sketch holds a closed rectangle profile and a
/// centre Point carrying a `SketchProfile` PadAttr seeded from one of the
/// rectangle's Lines — the shape produced by "Make Pad from Profile".
///
/// Returns the pad (linked to the centre) and the four profile-corner ids
/// in sw, se, ne, nw order.
fn footprint_with_profile_pad() -> (Footprint, EditorPad, [SketchEntityId; 4]) {
    use signex_sketch::attr::{CustomPadShape, PadAttr, PadShape};

    let mut fp = Footprint::empty("test");
    let plane_id = PlaneId::new();
    let mut sketch = SketchData::default();
    sketch.planes.push(Plane {
        id: plane_id,
        kind: PlaneKind::BoardTop,
    });

    // Rectangle (0,0)-(2,1): four corner Points, four connecting Lines.
    let corners = [
        (0.0_f64, 0.0_f64),
        (2.0, 0.0),
        (2.0, 1.0),
        (0.0, 1.0),
    ];
    let corner_ids: [SketchEntityId; 4] = std::array::from_fn(|i| {
        let id = SketchEntityId::new();
        sketch.entities.push(Entity::new(
            id,
            plane_id,
            EntityKind::Point {
                x: corners[i].0,
                y: corners[i].1,
            },
        ));
        id
    });
    let mut seed_line = None;
    for i in 0..4 {
        let id = SketchEntityId::new();
        sketch.entities.push(Entity::new(
            id,
            plane_id,
            EntityKind::Line {
                start: corner_ids[i],
                end: corner_ids[(i + 1) % 4],
            },
        ));
        seed_line.get_or_insert(id);
    }

    // Centre Point at the rectangle's centroid, carrying the pad attr.
    let centre_id = SketchEntityId::new();
    let mut centre = Entity::new(centre_id, plane_id, EntityKind::Point { x: 1.0, y: 0.5 });
    centre.pad = Some(PadAttr {
        number: "1".into(),
        shape: PadShape::Custom(CustomPadShape::SketchProfile {
            source: vec![seed_line.expect("seed line minted")],
        }),
        ..PadAttr::default()
    });
    sketch.entities.push(centre);
    fp.sketch = Some(sketch);

    let mut pad = editor_pad("1", 1.0, 0.5);
    pad.sketch_entity_id = Some(centre_id);
    // "Make Pad from Profile" leaves this None — the pad does not own the
    // rectangle's geometry, it only references the seed Line.
    pad.corner_entity_ids = None;

    (fp, pad, corner_ids)
}

/// Moving a `SketchProfile` pad must carry its profile geometry along.
///
/// Regression for the v0.14 bug: `mirror_move_pad_in_sketch` moved only the
/// centre Point and the `corner_entity_ids` bbox outline. A SketchProfile pad
/// has `corner_entity_ids: None`, so the profile stayed at its original
/// coordinates — visibly, the sketch rectangle did not follow the pad. The
/// silent half was worse: `signex_bake` bakes the profile as
/// `world_pts - pad_position`, so the copper resolved back to the ORIGINAL
/// location and the exported footprint had the pad in the wrong place.
#[test]
fn mirror_move_profile_pad_translates_profile_geometry() {
    let (mut fp, mut pad, corner_ids) = footprint_with_profile_pad();

    // Move the pad by (+3.0, +3.0): centroid (1.0, 0.5) -> (4.0, 3.5).
    pad.position_mm = (4.0, 3.5);
    mirror_move_pad_in_sketch(&pad, &mut fp);

    let point_at = |fp: &Footprint, id: SketchEntityId| -> (f64, f64) {
        let entity = fp
            .sketch
            .as_ref()
            .unwrap()
            .entities
            .iter()
            .find(|e| e.id == id)
            .expect("entity present");
        match entity.kind {
            EntityKind::Point { x, y } => (x, y),
            _ => panic!("profile corner must be a Point"),
        }
    };

    // The centre tracks the pad (this already worked).
    assert_eq!(
        point_at(&fp, pad.sketch_entity_id.unwrap()),
        (4.0, 3.5),
        "centre Point must follow the pad"
    );

    // The profile must have travelled by the same delta.
    let expected = [(3.0, 3.0), (5.0, 3.0), (5.0, 4.0), (3.0, 4.0)];
    let actual: Vec<(f64, f64)> = corner_ids.iter().map(|id| point_at(&fp, *id)).collect();
    assert_eq!(
        actual,
        expected.to_vec(),
        "profile geometry must translate with the pad, else the sketch shows \
         the shape at its old position and the bake emits copper at the old \
         location"
    );
}

/// Read a Point's raw x/y, panicking with the id when it isn't one.
fn point_of(fp: &Footprint, id: SketchEntityId) -> (f64, f64) {
    let entity = fp
        .sketch
        .as_ref()
        .expect("sketch present")
        .entities
        .iter()
        .find(|e| e.id == id)
        .unwrap_or_else(|| panic!("entity {id} present"));
    match entity.kind {
        EntityKind::Point { x, y } => (x, y),
        _ => panic!("entity {id} must be a Point"),
    }
}

/// Resolve a `shape_params` UUID-slug sidecar into its entity id.
fn sidecar(pad: &EditorPad, key: &str) -> SketchEntityId {
    let slug = pad
        .shape_params
        .get(key)
        .unwrap_or_else(|| panic!("sidecar {key} bound"));
    SketchEntityId(uuid::Uuid::parse_str(slug).expect("sidecar is a UUID slug"))
}

/// Moving a RoundRect pad must carry its arc anchors and inset
/// arc-centres along.
///
/// Regression: `mirror_move_pad_in_sketch` repositioned only the centre
/// Point and the four `corner_entity_ids`. RoundRect additionally mints
/// 8 edge anchors + 4 inset arc-centres, all NON-construction, so they
/// stayed at the old coordinates and the bake emitted copper from the
/// stranded geometry. Nothing downstream repaired it —
/// `sync_pads_to_primitive` copies attributes only, it never re-mints.
#[test]
fn mirror_move_roundrect_translates_anchors_and_arc_centres() {
    let mut fp = Footprint::empty("test");
    let mut pad = editor_pad("1", 0.0, 0.0);
    pad.shape = LibPadShape::RoundRect { radius_ratio: 0.25 };
    mirror_add_pad_to_sketch(&mut pad, &mut fp);

    // The 4 corner Arcs are the only RoundRect sidecars; each names an
    // inset centre + 2 edge anchors, so the 4 reach all 12 Points.
    let arc_ids: Vec<SketchEntityId> = ["ne", "se", "sw", "nw"]
        .iter()
        .map(|c| sidecar(&pad, &format!("corner_r_{c}_arc")))
        .collect();
    let mut tracked: Vec<SketchEntityId> = Vec::new();
    for arc_id in &arc_ids {
        let entity = fp
            .sketch
            .as_ref()
            .unwrap()
            .entities
            .iter()
            .find(|e| e.id == *arc_id)
            .expect("arc present");
        match entity.kind {
            EntityKind::Arc {
                center, start, end, ..
            } => tracked.extend([center, start, end]),
            _ => panic!("corner_r_*_arc must be an Arc"),
        }
    }
    tracked.sort_by_key(|id| id.0);
    tracked.dedup();
    assert_eq!(tracked.len(), 12, "4 insets + 8 edge anchors");

    let before: Vec<(f64, f64)> = tracked.iter().map(|id| point_of(&fp, *id)).collect();

    pad.position_mm = (3.0, -2.0);
    mirror_move_pad_in_sketch(&pad, &mut fp);

    // Exact equality: the delta is ADDED, never recomputed from the
    // bbox, so the arithmetic is bit-reproducible.
    let expected: Vec<(f64, f64)> = before.iter().map(|(x, y)| (x + 3.0, y - 2.0)).collect();
    let after: Vec<(f64, f64)> = tracked.iter().map(|id| point_of(&fp, *id)).collect();
    assert_eq!(
        after, expected,
        "RoundRect anchors + inset arc-centres must translate with the pad, \
         else the bake emits copper from the stranded geometry"
    );
    assert_eq!(point_of(&fp, pad.sketch_entity_id.unwrap()), (3.0, -2.0));
}

/// Same stranding, Oval's sidecar set: 4 edge anchors + 2 arc-centres.
#[test]
fn mirror_move_oval_translates_anchor_sidecars() {
    let mut fp = Footprint::empty("test");
    let mut pad = editor_pad("1", 0.0, 0.0);
    pad.shape = LibPadShape::Oval;
    mirror_add_pad_to_sketch(&mut pad, &mut fp);

    let tracked: Vec<SketchEntityId> = (0..4)
        .map(|i| sidecar(&pad, &format!("oval_anchor_{i}")))
        .chain((0..2).map(|i| sidecar(&pad, &format!("oval_centre_{i}"))))
        .collect();
    let before: Vec<(f64, f64)> = tracked.iter().map(|id| point_of(&fp, *id)).collect();

    pad.position_mm = (3.0, -2.0);
    mirror_move_pad_in_sketch(&pad, &mut fp);

    let expected: Vec<(f64, f64)> = before.iter().map(|(x, y)| (x + 3.0, y - 2.0)).collect();
    let after: Vec<(f64, f64)> = tracked.iter().map(|id| point_of(&fp, *id)).collect();
    assert_eq!(
        after, expected,
        "Oval anchors + arc-centres must translate with the pad"
    );
}

/// Deleting a pad must drop constraints on ANY entity it owned, not
/// just the ones naming its centre Point.
#[test]
fn mirror_delete_drops_constraints_on_owned_corners() {
    use signex_sketch::constraint::{Constraint, ConstraintKind};
    use signex_sketch::id::ConstraintId;

    let mut fp = Footprint::empty("test");
    let mut pad = editor_pad("1", 0.0, 0.0);
    pad.shape = LibPadShape::Rect;
    mirror_add_pad_to_sketch(&mut pad, &mut fp);

    let corners = pad.corner_entity_ids.expect("corners minted");
    fp.sketch.as_mut().unwrap().constraints.push(Constraint {
        id: ConstraintId::new(),
        // References two owned corners; the centre appears nowhere.
        kind: ConstraintKind::Coincident {
            p1: corners[0],
            p2: corners[1],
        },
    });

    mirror_delete_pad_from_sketch(&pad, &mut fp);

    assert!(
        fp.sketch.as_ref().unwrap().constraints.is_empty(),
        "a constraint on a dropped corner must not survive the pad"
    );
}

/// Deleting a pad must drop the per-corner unlink override parameter.
///
/// `pad_bridge.rs` mints it as `{shared_name}_{corner_suffix}`, i.e.
/// `corner_r_<slug>_ne` — it ends with `_ne`, not the slug, so an
/// `ends_with(&slug)` retain orphaned it in the parameter table.
#[test]
fn mirror_delete_drops_per_corner_unlink_parameter() {
    let mut fp = Footprint::empty("test");
    let mut pad = editor_pad("1", 0.0, 0.0);
    pad.shape = LibPadShape::RoundRect { radius_ratio: 0.25 };
    mirror_add_pad_to_sketch(&mut pad, &mut fp);

    let shared_name = pad
        .shape_params
        .get("corner_r")
        .expect("shared corner_r bound")
        .clone();
    let per_corner = format!("{shared_name}_ne");
    fp.sketch
        .as_mut()
        .unwrap()
        .parameters
        .insert(per_corner.clone(), "0.25mm");
    pad.shape_params
        .insert("corner_r_ne".into(), per_corner.clone());

    mirror_delete_pad_from_sketch(&pad, &mut fp);

    let params = &fp.sketch.as_ref().unwrap().parameters;
    assert!(
        params.get_raw(&per_corner).is_none(),
        "per-corner unlink override {per_corner} must go with the pad"
    );
    assert!(
        params.get_raw(&shared_name).is_none(),
        "shared corner_r must go with the pad"
    );
}

#[test]
fn format_f64_trims_trailing_zeros() {
    use super::attr::format_f64;
    assert_eq!(format_f64(1.0), "1");
    assert_eq!(format_f64(1.5), "1.5");
    assert_eq!(format_f64(0.25), "0.25");
    assert_eq!(format_f64(1.27), "1.27");
    assert_eq!(format_f64(0.0), "0");
}

#[test]
fn shape_change_preserves_corner_positions() {
    use crate::library::editor::footprint::state::FootprintEditorState;

    let mut fp = Footprint::empty("test");
    let mut pad = editor_pad("1", 0.0, 0.0);
    pad.shape = LibPadShape::Rect;
    mirror_add_pad_to_sketch(&mut pad, &mut fp);

    let corner_ids = pad.corner_entity_ids.expect("corners minted");
    let snapshot_corner_pos = |fp: &Footprint| -> Vec<(f64, f64)> {
        corner_ids
            .iter()
            .map(|id| {
                let entity = fp
                    .sketch
                    .as_ref()
                    .unwrap()
                    .entities
                    .iter()
                    .find(|e| e.id == *id)
                    .expect("corner Point present");
                match entity.kind {
                    EntityKind::Point { x, y } => (x, y),
                    _ => panic!("corner must be Point"),
                }
            })
            .collect()
    };

    let before = snapshot_corner_pos(&fp);

    pad.shape = LibPadShape::Oval;
    let mut s = FootprintEditorState::empty();
    s.pads = vec![pad.clone()];
    FootprintEditorState::sync_pads_to_primitive(&s, &mut fp);

    let after = snapshot_corner_pos(&fp);

    assert_eq!(
        before, after,
        "corner positions must remain stable across shape changes"
    );
}

// ─────────────────────────────────────────────────────────────────
// Rotation reaches the SKETCH persistence path.
//
// `pad_attr_from_editor_pad` hardcoded `rotation_expr: None` and
// `sync_pads_to_primitive` never wrote the field, so
// `signex_bake::pad::rotation_deg` mapped `None -> 0.0` while
// `EditorPad::to_pad` wrote the true angle onto the literal `Pad`.
// Two persistence paths, two answers for the same pad.
// ─────────────────────────────────────────────────────────────────

#[test]
fn minted_pad_attr_carries_the_rotation() {
    let mut fp = Footprint::empty("test");
    let mut pad = editor_pad("1", 0.0, 0.0);
    pad.rotation_deg = 45.0;
    mirror_add_pad_to_sketch(&mut pad, &mut fp);

    let id = pad.sketch_entity_id.expect("centre Point minted");
    let attr = fp
        .sketch
        .as_ref()
        .unwrap()
        .entities
        .iter()
        .find(|e| e.id == id)
        .and_then(|e| e.pad.as_ref())
        .expect("PadAttr on the centre Point");
    assert_eq!(
        attr.rotation_expr.as_deref(),
        Some("45deg"),
        "the minted PadAttr must carry the pad's rotation"
    );
}

#[test]
fn rotation_survives_a_bake_round_trip() {
    use signex_sketch::solver::Solver;
    use signex_sketch::solver::residual::ResolvedParams;

    let mut fp = Footprint::empty("test");
    let mut pad = editor_pad("1", 0.0, 0.0);
    pad.rotation_deg = 45.0;
    mirror_add_pad_to_sketch(&mut pad, &mut fp);

    let sketch = fp.sketch.as_ref().unwrap();
    let solve = Solver::default()
        .solve(sketch, &ResolvedParams::new())
        .expect("solve");
    let mut out = Vec::new();
    let mut warnings = Vec::new();
    signex_bake::bake_pads(
        sketch,
        &solve,
        &std::collections::HashMap::new(),
        &mut out,
        &mut warnings,
    )
    .expect("bake_pads ok");

    assert_eq!(out.len(), 1, "one pad baked");
    assert_eq!(
        out[0].rotation, 45.0,
        "the sketch-baked pad must carry the authored rotation, not 0°"
    );
}

#[test]
fn sync_preserves_an_authored_rotation_expression() {
    use crate::library::editor::footprint::state::FootprintEditorState;

    let mut fp = Footprint::empty("test");
    let mut pad = editor_pad("1", 0.0, 0.0);
    mirror_add_pad_to_sketch(&mut pad, &mut fp);
    let id = pad.sketch_entity_id.expect("centre Point minted");

    // The user binds rotation to a parameter. The Pads→Sketch mirror
    // must not clobber it with the editor's literal angle.
    let set_expr = |fp: &mut Footprint, expr: Option<String>| {
        fp.sketch
            .as_mut()
            .unwrap()
            .entities
            .iter_mut()
            .find(|e| e.id == id)
            .and_then(|e| e.pad.as_mut())
            .unwrap()
            .rotation_expr = expr;
    };
    let read_expr = |fp: &Footprint| -> Option<String> {
        fp.sketch
            .as_ref()
            .unwrap()
            .entities
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| e.pad.as_ref())
            .unwrap()
            .rotation_expr
            .clone()
    };

    set_expr(&mut fp, Some("= leg_angle".into()));
    pad.rotation_deg = 90.0;
    let mut s = FootprintEditorState::empty();
    s.pads = vec![pad.clone()];
    FootprintEditorState::sync_pads_to_primitive(&s, &mut fp);
    assert_eq!(
        read_expr(&fp).as_deref(),
        Some("= leg_angle"),
        "an authored `= expr` binding is the user's and must survive the sync"
    );

    // A plain literal, by contrast, is ours to keep current.
    set_expr(&mut fp, Some("0deg".into()));
    FootprintEditorState::sync_pads_to_primitive(&s, &mut fp);
    assert_eq!(
        read_expr(&fp).as_deref(),
        Some("90deg"),
        "a plain literal must track the editor's rotation"
    );
}

// ─────────────────────────────────────────────────────────────────
// Rotation reaches the MINTED GEOMETRY, not just the bbox corners.
//
// `bbox_corner_points` emits rotated corners, but the arc anchors,
// arc centres and chamfer anchors of RoundRect / Oval / Chamfered
// were all still derived straight off the un-rotated `bbox_mm()`.
// The result was an outline whose corners had turned with the pad
// while the edges and arcs joining them had not — geometry that no
// longer closes. The invariant below catches the whole class: every
// Point a shape mints has to sit on or inside the pad's real copper
// quad, which an un-rotated anchor does not at 45°.
// ─────────────────────────────────────────────────────────────────

/// Every `Point` in `fp`'s sketch, expressed in the pad's own frame.
fn minted_points_in_pad_frame(fp: &Footprint, pad: &EditorPad) -> Vec<(f64, f64)> {
    fp.sketch
        .as_ref()
        .expect("mint produced a sketch")
        .entities
        .iter()
        .filter_map(|e| match e.kind {
            EntityKind::Point { x, y } => Some(pad.world_to_local_mm(x, y)),
            _ => None,
        })
        .collect()
}

fn assert_minted_geometry_stays_inside_the_turned_copper(shape: LibPadShape) {
    let mut fp = Footprint::empty("test");
    let mut pad = editor_pad("1", 0.0, 0.0);
    pad.size_mm = (2.0, 1.0);
    pad.shape = shape.clone();
    pad.rotation_deg = 45.0;
    mirror_add_pad_to_sketch(&mut pad, &mut fp);

    let (xmin, ymin, xmax, ymax) = pad.bbox_mm();
    const EPS: f64 = 1e-9;
    let points = minted_points_in_pad_frame(&fp, &pad);
    assert!(
        points.len() > 5,
        "{shape:?} must mint more than the centre + 4 corners; got {}",
        points.len()
    );
    for (lx, ly) in points {
        assert!(
            lx >= xmin - EPS && lx <= xmax + EPS && ly >= ymin - EPS && ly <= ymax + EPS,
            "{shape:?}: minted Point maps to ({lx}, {ly}) in the pad frame, outside the copper \
             [{xmin}, {xmax}] × [{ymin}, {ymax}] — it was derived off the un-rotated bbox while \
             the corners it joins were rotated, so the outline does not close"
        );
    }
}

#[test]
fn rotated_round_rect_mints_geometry_that_closes() {
    assert_minted_geometry_stays_inside_the_turned_copper(LibPadShape::RoundRect {
        radius_ratio: 0.25,
    });
}

#[test]
fn rotated_oval_mints_geometry_that_closes() {
    assert_minted_geometry_stays_inside_the_turned_copper(LibPadShape::Oval);
}

#[test]
fn rotated_chamfered_mints_geometry_that_closes() {
    use signex_library::primitive::footprint::ChamferedCorners;
    assert_minted_geometry_stays_inside_the_turned_copper(LibPadShape::Chamfered {
        chamfer_ratio: 0.25,
        corners: ChamferedCorners {
            top_left: true,
            top_right: true,
            bottom_left: true,
            bottom_right: true,
        },
    });
}

#[test]
fn sync_preserves_a_bare_parameter_binding_with_no_eq_prefix() {
    use crate::library::editor::footprint::state::FootprintEditorState;

    // The `=` prefix is OPTIONAL — `resolve_dim` strips it before
    // parsing and `signex_bake::pad::rotation_deg` does the same — so
    // a bare `leg_angle` is a fully valid authored binding. Keying the
    // data-loss guard on the prefix destroyed exactly these.
    let mut fp = Footprint::empty("test");
    let mut pad = editor_pad("1", 0.0, 0.0);
    mirror_add_pad_to_sketch(&mut pad, &mut fp);
    let id = pad.sketch_entity_id.expect("centre Point minted");
    fp.sketch
        .as_mut()
        .unwrap()
        .entities
        .iter_mut()
        .find(|e| e.id == id)
        .and_then(|e| e.pad.as_mut())
        .unwrap()
        .rotation_expr = Some("leg_angle".into());

    pad.rotation_deg = 90.0;
    let mut s = FootprintEditorState::empty();
    s.pads = vec![pad];
    FootprintEditorState::sync_pads_to_primitive(&s, &mut fp);

    let after = fp
        .sketch
        .as_ref()
        .unwrap()
        .entities
        .iter()
        .find(|e| e.id == id)
        .and_then(|e| e.pad.as_ref())
        .unwrap()
        .rotation_expr
        .clone();
    assert_eq!(
        after.as_deref(),
        Some("leg_angle"),
        "a prefix-less parameter binding is still the user's authored expression and must \
         survive the Pads→Sketch mirror"
    );
}

#[test]
fn sync_overwrites_every_bare_literal_form() {
    use crate::library::editor::footprint::state::FootprintEditorState;

    for literal in ["0deg", "= 0deg", "12.5", "-45deg", "1rad"] {
        let mut fp = Footprint::empty("test");
        let mut pad = editor_pad("1", 0.0, 0.0);
        mirror_add_pad_to_sketch(&mut pad, &mut fp);
        let id = pad.sketch_entity_id.expect("centre Point minted");
        fp.sketch
            .as_mut()
            .unwrap()
            .entities
            .iter_mut()
            .find(|e| e.id == id)
            .and_then(|e| e.pad.as_mut())
            .unwrap()
            .rotation_expr = Some(literal.into());

        pad.rotation_deg = 90.0;
        let mut s = FootprintEditorState::empty();
        s.pads = vec![pad.clone()];
        FootprintEditorState::sync_pads_to_primitive(&s, &mut fp);

        let after = fp
            .sketch
            .as_ref()
            .unwrap()
            .entities
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| e.pad.as_ref())
            .unwrap()
            .rotation_expr
            .clone();
        assert_eq!(
            after.as_deref(),
            Some("90deg"),
            "{literal:?} carries no parameter reference, so the editor owns it and must keep \
             it current"
        );
    }
}

/// The delete sweep drops the pad's whole entity set, so a constraint
/// authored against ANY of those entities is dangling afterwards — not
/// just one authored against the centre. Matching the centre id alone
/// left the rest behind pointing at entities that no longer exist.
///
/// It matters more now that a frame transform re-mints through this
/// path: a user who constrains a chamfer anchor accumulates a stale
/// row on every rotate and flip, not once on a pad delete.
#[test]
fn mirror_delete_pad_drops_constraints_on_the_whole_entity_set() {
    use signex_library::primitive::footprint::ChamferedCorners;
    use signex_sketch::constraint::{Constraint, ConstraintKind};
    use signex_sketch::id::ConstraintId;

    let mut fp = Footprint::empty("test");
    let mut pad = editor_pad("X", 0.0, 0.0);
    pad.size_mm = (2.0, 1.0);
    pad.shape = LibPadShape::Chamfered {
        chamfer_ratio: 0.25,
        corners: ChamferedCorners {
            top_right: true,
            ..Default::default()
        },
    };
    mirror_add_pad_to_sketch(&mut pad, &mut fp);

    // A free Point the user owns, constrained to a chamfer anchor —
    // the anchor is a pad-owned entity that is NOT the centre.
    let anchor_raw = pad
        .shape_params
        .get("chamfer_ne_anchor1")
        .expect("a chamfered pad binds its NE anchor");
    let anchor = SketchEntityId(uuid::Uuid::parse_str(anchor_raw).expect("sidecar is a UUID slug"));
    let plane_id = fp.sketch.as_ref().unwrap().planes[0].id;
    let free = SketchEntityId::new();
    let sketch = fp.sketch.as_mut().unwrap();
    sketch.entities.push(Entity::new(
        free,
        plane_id,
        EntityKind::Point { x: 9.0, y: 9.0 },
    ));
    sketch.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Coincident {
            p1: free,
            p2: anchor,
        },
    });

    mirror_delete_pad_from_sketch(&pad, &mut fp);

    let sketch = fp.sketch.as_ref().unwrap();
    assert!(
        sketch.constraints.is_empty(),
        "a constraint referencing the deleted NE chamfer anchor must be swept with it; \
         {} survived pointing at an entity that no longer exists",
        sketch.constraints.len()
    );
}

/// The owned set must never name an entity the sketch does not have.
///
/// Seeds used to be pushed before the lookup that confirms them, so a
/// stale or foreign UUID in the ledger reached the delete drop set —
/// where ids are stringified and substring-matched against every
/// constraint's `Debug` rendering. A dead id there is not a no-op; it
/// deletes whatever constraint happens to mention it.
#[test]
fn owned_set_excludes_ids_with_no_live_entity() {
    let mut fp = Footprint::empty("test");
    let mut pad = editor_pad("1", 0.0, 0.0);
    mirror_add_pad_to_sketch(&mut pad, &mut fp);
    let sketch = fp.sketch.as_mut().unwrap();

    // A ledger entry naming nothing — a pad written by an older build,
    // or geometry deleted from Sketch mode since.
    let ghost = SketchEntityId::new();
    let centre = pad.sketch_entity_id.unwrap();
    sketch
        .entities
        .iter_mut()
        .find(|e| e.id == centre)
        .and_then(|e| e.pad.as_mut())
        .unwrap()
        .owned
        .push(ghost);

    let owned = ownership::owned_sketch_entities(&pad, sketch);
    assert!(
        !owned.contains(&ghost),
        "a seed naming no live entity must not be reported as owned"
    );
}

/// A profile pad that ALSO owns the loop's Points must take the move
/// delta exactly once.
///
/// `mint_shape_geometry_for`'s catch-all arm sets `corner_entity_ids`
/// for `LibPadShape::Custom`, so a Custom pad minted through
/// `mirror_add_pad_to_sketch` whose `PadAttr` is a `SketchProfile` over
/// that same outline sits in BOTH the traced loop and the owned set.
/// The move translates the profile first and the owned set second, so
/// the shared Points used to take the delta twice and the outline
/// landed at double the offset — copper in the wrong place on the next
/// bake. Correctness was resting on an undocumented "these two sets are
/// disjoint" invariant that this configuration violates.
#[test]
fn mirror_move_profile_pad_owning_its_loop_applies_delta_once() {
    let (mut fp, mut pad, corner_ids) = footprint_with_profile_pad();
    // The one thing that differs from `footprint_with_profile_pad`'s
    // "Make Pad from Profile" origin: the pad also claims the loop's
    // corners, exactly as the Custom mint arm would have written them.
    pad.corner_entity_ids = Some(corner_ids);

    pad.position_mm = (4.0, 3.5); // centroid (1.0, 0.5) + (3.0, 3.0)
    mirror_move_pad_in_sketch(&pad, &mut fp);

    let expected = [(3.0, 3.0), (5.0, 3.0), (5.0, 4.0), (3.0, 4.0)];
    let actual: Vec<(f64, f64)> = corner_ids.iter().map(|id| point_of(&fp, *id)).collect();
    assert_eq!(
        actual,
        expected.to_vec(),
        "the loop's Points took the pad delta more than once"
    );
}
