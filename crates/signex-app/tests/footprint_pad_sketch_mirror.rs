//! Pad ↔ sketch mirror regressions (issue #142 remediation).
//!
//! Split out of `regression.rs` rather than appended to it — that file
//! is past 6500 lines and the repo caps a file at ~800.
//!
//! The wave-1 fix for #142 routed move / delete / paste through one
//! owned-entity set, but proved it only INSIDE the minting session.
//! Every assertion here is about what survives the boundary the
//! session-local tests could not see — a save + reopen — plus the two
//! blast-radius properties the widened owned set put at risk.

use signex_app::library::editor::footprint::pad_to_sketch;
use signex_app::library::editor::footprint::state::FootprintEditorState;
use signex_library::primitive::footprint::{Footprint, PadShape};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::sketch::SketchData;

/// Mint one pad of `shape` at `pos` into a fresh footprint and return
/// the footprint with the editor state already synced onto it — i.e.
/// exactly what would be written to disk.
fn footprint_with_minted_pad(shape: PadShape, pos: (f64, f64)) -> Footprint {
    let mut fp = Footprint::empty("t");
    let mut state = FootprintEditorState::from_footprint(&fp);
    state.add_pad_at(pos.0, pos.1);
    state.pads[0].shape = shape;
    state.pads[0].size_mm = (2.0, 1.0);
    pad_to_sketch::mirror_add_pad_to_sketch(&mut state.pads[0], &mut fp);
    FootprintEditorState::sync_pads_to_primitive(&state, &mut fp);
    fp
}

/// Every `Point` in the sketch, as `(id, x, y)`.
fn points(sketch: &SketchData) -> Vec<(SketchEntityId, f64, f64)> {
    sketch
        .entities
        .iter()
        .filter_map(|e| match e.kind {
            EntityKind::Point { x, y } => Some((e.id, x, y)),
            _ => None,
        })
        .collect()
}

fn point_xy(sketch: &SketchData, id: SketchEntityId) -> Option<(f64, f64)> {
    sketch
        .entities
        .iter()
        .find(|e| e.id == id)
        .and_then(|e| match e.kind {
            EntityKind::Point { x, y } => Some((x, y)),
            _ => None,
        })
}

/// A pad reopened from disk must still own its sketch geometry.
///
/// This is the boundary the wave-1 tests never crossed. All three
/// editor-side ownership fields — `sketch_entity_id`,
/// `corner_entity_ids`, `shape_params` — are session-volatile:
/// `EditorPad::from_pad` sets every one of them to `None` / empty
/// because none has a home on `Pad`. So on reopen the pad owned
/// nothing AND had no link at all, and `mirror_move_pad_in_sketch`
/// early-returned on the missing link before ownership even mattered.
/// The pad's whole RoundRect outline stayed where it was minted while
/// the pad moved away, and the bake resolves copper from the sketch —
/// so the exported footprint had the copper in the old place.
///
/// Two fixes have to hold together for this to pass: the link is
/// rebuilt from the sketch's own `PadAttr.number`, and the owned set
/// is read from the durable `PadAttr::owned` ledger.
#[test]
fn issue142_reopened_pad_still_moves_its_whole_outline() {
    let mut fp = footprint_with_minted_pad(PadShape::RoundRect { radius_ratio: 0.25 }, (1.0, 1.0));
    let before = points(fp.sketch.as_ref().unwrap());
    assert!(
        before.len() > 5,
        "RoundRect must mint anchors + insets beyond the centre, got {}",
        before.len()
    );

    // The save/reopen boundary: rebuild editor state from the
    // primitive alone, exactly as opening the document does.
    let mut state = FootprintEditorState::from_footprint(&fp);
    assert!(
        state.pads[0].sketch_entity_id.is_some(),
        "reopened pad must be relinked to its sketch centre"
    );
    assert!(
        state.pads[0].corner_entity_ids.is_none() && state.pads[0].shape_params.is_empty(),
        "the volatile fields are still empty — the durable ledger is what has to carry this"
    );

    let (dx, dy) = (3.0, -2.0);
    state.pads[0].position_mm = (1.0 + dx, 1.0 + dy);
    pad_to_sketch::mirror_move_pad_in_sketch(&state.pads[0], &mut fp);

    let sketch = fp.sketch.as_ref().unwrap();
    for (id, ox, oy) in before {
        let (nx, ny) = point_xy(sketch, id).expect("point survived the move");
        assert_eq!(
            (nx, ny),
            (ox + dx, oy + dy),
            "point {id:?} was stranded at ({ox}, {oy}) instead of following the pad"
        );
    }
}

/// Deleting a reopened pad must remove it, not leave a ghost.
///
/// Same missing link as above, other mirror: `mirror_delete_pad_from_sketch`
/// early-returned, so the outline AND its `PadAttr`-carrying centre
/// stayed in the sketch. The sketch is the bake's source of truth, so
/// the "deleted" pad came straight back on the next bake.
#[test]
fn issue142_reopened_pad_delete_removes_its_geometry() {
    let mut fp = footprint_with_minted_pad(PadShape::RoundRect { radius_ratio: 0.25 }, (1.0, 1.0));
    let state = FootprintEditorState::from_footprint(&fp);
    pad_to_sketch::mirror_delete_pad_from_sketch(&state.pads[0], &mut fp);

    let sketch = fp.sketch.as_ref().unwrap();
    assert!(
        sketch.entities.iter().all(|e| e.pad.is_none()),
        "the PadAttr centre survived the delete — the pad resurrects on the next bake"
    );
    assert!(
        sketch.entities.is_empty(),
        "the pad's outline survived the delete: {} entities left",
        sketch.entities.len()
    );
}

/// Deleting a pad must not delete user geometry that merely touches it.
///
/// The delete sweep pulls in every Line / Arc that references a dropped
/// Point — it has to, a Line with a dead endpoint is not a Line. But it
/// used to also drop that entity's OTHER endpoint, which for a
/// user-drawn silk Line anchored to a pad corner is the far end sitting
/// out in the user's own drawing. Widening the pad's owned set made
/// that reach three times further. The pad's own anchors and inset
/// arc-centres are already reached by expanding its Arcs forward, so
/// the far side of such an edge is only ever foreign.
#[test]
fn issue142_delete_does_not_eat_user_geometry_sharing_an_anchor() {
    let mut fp = footprint_with_minted_pad(PadShape::RoundRect { radius_ratio: 0.25 }, (0.0, 0.0));
    let state = FootprintEditorState::from_footprint(&fp);
    let sketch = fp.sketch.as_mut().unwrap();
    let plane = sketch.planes[0].id;

    // A pad anchor: the start Point of one of the RoundRect corner Arcs.
    let anchor = sketch
        .entities
        .iter()
        .find_map(|e| match e.kind {
            EntityKind::Arc { start, .. } => Some(start),
            _ => None,
        })
        .expect("RoundRect mints corner Arcs");

    // User silk: a Line from that shared anchor out to a far Point.
    let far = SketchEntityId::new();
    sketch.entities.push(Entity::new(
        far,
        plane,
        EntityKind::Point { x: 50.0, y: 50.0 },
    ));
    let silk = SketchEntityId::new();
    sketch.entities.push(Entity::new(
        silk,
        plane,
        EntityKind::Line {
            start: anchor,
            end: far,
        },
    ));

    pad_to_sketch::mirror_delete_pad_from_sketch(&state.pads[0], &mut fp);

    let sketch = fp.sketch.as_ref().unwrap();
    assert!(
        point_xy(sketch, far).is_some(),
        "the user's far endpoint was deleted along with the pad"
    );
    assert!(
        !sketch.entities.iter().any(|e| e.id == silk),
        "the silk Line lost an endpoint, so it cannot survive"
    );
}

/// A move re-states the bbox corners absolutely, so drift self-heals.
///
/// Routing move through a single centre-derived delta made it purely
/// cumulative: any mismatch between the pad's declared size and its
/// sketch outline became permanent, and repeated moves accumulated
/// rounding in every anchor. The four bbox corners are the only owned
/// Points whose position is fully derivable from `Pad`, so they are
/// re-asserted from `bbox_mm()` after the delta pass.
#[test]
fn issue142_move_repairs_drifted_bbox_corners() {
    let mut fp = Footprint::empty("t");
    let mut state = FootprintEditorState::from_footprint(&fp);
    state.add_pad_at(0.0, 0.0);
    state.pads[0].size_mm = (2.0, 1.0);
    pad_to_sketch::mirror_add_pad_to_sketch(&mut state.pads[0], &mut fp);
    let corners = state.pads[0].corner_entity_ids.expect("Rect mints corners");

    // Shove the `ne` corner off its bbox position.
    {
        let sketch = fp.sketch.as_mut().unwrap();
        if let Some(e) = sketch.entities.iter_mut().find(|e| e.id == corners[0]) {
            e.kind = EntityKind::Point { x: 9.0, y: 9.0 };
        }
    }

    state.pads[0].position_mm = (4.0, 4.0);
    pad_to_sketch::mirror_move_pad_in_sketch(&state.pads[0], &mut fp);

    let sketch = fp.sketch.as_ref().unwrap();
    // bbox of a 2.0 × 1.0 pad centred at (4, 4): ne = (xmax, ymin).
    assert_eq!(
        point_xy(sketch, corners[0]),
        Some((5.0, 3.5)),
        "the drifted corner was carried along by the delta instead of repaired"
    );
}
