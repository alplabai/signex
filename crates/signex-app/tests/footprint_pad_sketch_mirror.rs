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
use std::path::PathBuf;

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

/// Two pads that SHARE a pad number, both minted, both synced onto the
/// primitive — i.e. what a shared-designator row / thermal / shield pad
/// set looks like on disk. `designator_override` is the production path
/// that produces it: it stamps one number onto every pad placed after
/// it.
fn footprint_with_two_pads_sharing_a_number() -> Footprint {
    let mut fp = Footprint::empty("t");
    let mut state = FootprintEditorState::from_footprint(&fp);
    state.next_pad_defaults.designator_override = Some("1".to_string());
    for pos in [(0.0, 0.0), (5.0, 0.0)] {
        let idx = state.add_pad_at(pos.0, pos.1);
        state.pads[idx].size_mm = (2.0, 1.0);
        pad_to_sketch::mirror_add_pad_to_sketch(&mut state.pads[idx], &mut fp);
    }
    assert_eq!(state.pads[0].number, state.pads[1].number, "shared number");
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

/// Two pads sharing a number must not share a sketch centre.
///
/// Pad numbers are not unique anywhere in signex — the designator field
/// takes any string, and `next_pad_defaults.designator_override` stamps
/// one number onto every pad placed after it, which is how a
/// shared-designator row / thermal / shield pad set is authored. Each
/// such pad mints its OWN `PadAttr`-bearing centre.
///
/// Relinking them by number alone is last-wins, so on reopen every pad
/// with that number got the SAME `sketch_entity_id` — and then a
/// Pads-mode delete of pad A ran the delete mirror over pad B's centre,
/// B's `PadAttr::owned` ledger and B's whole outline. B stayed in
/// `state.pads` looking healthy while the bake, which resolves copper
/// from the sketch, silently dropped its copper from the export.
#[test]
fn issue142_duplicate_pad_numbers_do_not_alias_one_centre() {
    let fp = footprint_with_two_pads_sharing_a_number();
    let state = FootprintEditorState::from_footprint(&fp);

    let (a, b) = (
        state.pads[0].sketch_entity_id,
        state.pads[1].sketch_entity_id,
    );
    assert!(
        a.is_some() && b.is_some(),
        "both duplicate-numbered pads must relink (positions disambiguate them)"
    );
    assert_ne!(
        a, b,
        "the two pads were aliased onto ONE centre — an edit to either \
         destroys the other's geometry"
    );
}

/// ...and deleting one of them must leave the other's copper intact.
///
/// The concrete failure the aliasing produces. Pad A is deleted in Pads
/// mode; if A and B share a centre, `mirror_delete_pad_from_sketch`
/// takes B's outline and B's `PadAttr` centre with it while B remains
/// in the pad list — copper that silently vanishes from the export.
#[test]
fn issue142_deleting_one_duplicate_numbered_pad_keeps_the_others_copper() {
    let mut fp = footprint_with_two_pads_sharing_a_number();
    let state = FootprintEditorState::from_footprint(&fp);
    let b_centre = state.pads[1].sketch_entity_id.expect("pad B relinked");
    let b_owned: Vec<SketchEntityId> = fp
        .sketch
        .as_ref()
        .unwrap()
        .entities
        .iter()
        .find(|e| e.id == b_centre)
        .and_then(|e| e.pad.as_ref())
        .map(|attr| attr.owned.clone())
        .expect("pad B carries its durable ledger");
    assert!(
        b_owned.len() > 1,
        "pad B owns an outline, not just a centre"
    );

    pad_to_sketch::mirror_delete_pad_from_sketch(&state.pads[0], &mut fp);

    let sketch = fp.sketch.as_ref().unwrap();
    assert!(
        sketch.entities.iter().any(|e| e.id == b_centre),
        "pad B's centre was deleted along with pad A — B's copper is gone from the bake"
    );
    for id in b_owned {
        assert!(
            sketch.entities.iter().any(|e| e.id == id),
            "pad B's owned entity {id:?} was deleted along with pad A"
        );
    }
}

/// The durable ledger has to survive the REAL serialiser, not just
/// `from_footprint`.
///
/// The whole reopen fix rests on `PadAttr::owned` round-tripping
/// through `Footprint`'s serde derive — the same `serde_json` path
/// `local_git::primitives` writes with. The other reopen tests cross
/// the `EditorPad`-volatility boundary but never the serde one, so
/// nothing pinned the field's `#[serde(default,
/// skip_serializing_if = "Vec::is_empty")]` attributes or its presence
/// in the struct at all.
#[test]
fn issue142_owned_ledger_survives_a_real_serde_round_trip() {
    let fp = footprint_with_minted_pad(PadShape::RoundRect { radius_ratio: 0.25 }, (1.0, 1.0));
    let before: Vec<Vec<SketchEntityId>> = fp
        .sketch
        .as_ref()
        .unwrap()
        .entities
        .iter()
        .filter_map(|e| e.pad.as_ref().map(|attr| attr.owned.clone()))
        .collect();
    assert!(
        before.iter().all(|o| o.len() > 1),
        "the mint must record a non-trivial ledger, got {before:?}"
    );

    let json = serde_json::to_string(&fp).expect("Footprint serialises");
    let mut round_tripped: Footprint = serde_json::from_str(&json).expect("Footprint deserialises");

    let after: Vec<Vec<SketchEntityId>> = round_tripped
        .sketch
        .as_ref()
        .unwrap()
        .entities
        .iter()
        .filter_map(|e| e.pad.as_ref().map(|attr| attr.owned.clone()))
        .collect();
    assert_eq!(
        before, after,
        "PadAttr::owned did not survive the serde round trip — every reopened \
         pad owns nothing and both mirrors strand its geometry"
    );

    // And it is still load-bearing after the trip: a delete off the
    // deserialised footprint clears the pad's whole outline.
    let state = FootprintEditorState::from_footprint(&round_tripped);
    pad_to_sketch::mirror_delete_pad_from_sketch(&state.pads[0], &mut round_tripped);
    assert!(
        round_tripped.sketch.as_ref().unwrap().entities.is_empty(),
        "geometry survived a delete on the deserialised footprint"
    );
}

// ---------------------------------------------------------------
// Wave-1 (#142) paste regression, relocated out of `regression.rs`.
// It belongs with the rest of the pad ↔ sketch mirror coverage, and
// `regression.rs` is 6500+ lines against a ~800-line cap — adding to
// it was the regression, moving it out is the fix.
// ---------------------------------------------------------------

/// One app with `count` default pads on a footprint editor — the
/// local twin of `regression.rs`'s `fixture_footprint_with_pads`,
/// carried along with the test that needs it.
fn app_with_footprint_pads(stem: &str, count: usize) -> (signex_app::app::Signex, PathBuf) {
    use signex_app::app::FootprintEditorState as EditorTab;
    use signex_app::library::editor::footprint::state::EditorPad;
    use signex_library::FootprintFile;

    let path = PathBuf::from(format!("{stem}.snxfpt"));
    let file = FootprintFile::from_footprint(Footprint::empty(stem));
    let mut editor = EditorTab::new(path.clone(), file);
    for i in 0..count {
        editor.state.pads.push(EditorPad::new_default(
            (i + 1).to_string(),
            (i as f64 * 2.0, 0.0),
        ));
    }
    let (mut app, _initial_task) = signex_app::app::Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    (app, path)
}
/// A pasted pad must not alias the template's sketch parameters.
///
/// `shape_params` is the third pad-ownership field (alongside
/// `sketch_entity_id` and `corner_entity_ids`) and was cloned wholesale
/// from the template while only the other two were reset. The pasted
/// pad therefore still named the TEMPLATE's parameter names and Arc
/// ids, so editing its corner radius in the Properties panel resolved
/// through `pad.shape_params[key]` and silently resized the ORIGINAL
/// pad. It never self-corrected either: `auto_mint_for_literal_pads`
/// early-returns once the sketch holds any non-construction entity, and
/// `refresh_pads_from_primitive` re-attaches pads by number from the
/// old links, preserving the stale ledger indefinitely.
#[test]
fn v026e_paste_does_not_alias_template_shape_params() {
    use signex_app::app::Message;
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};

    let (mut app, path) = app_with_footprint_pads("v026e-paste-shape-params", 1);
    // Give the single pad a RoundRect shape and mint its sketch
    // geometry, so the footprint's sketch is authored and the pad owns
    // a populated `shape_params` ledger.
    {
        let editor = app.document_state.footprint_editors.get_mut(&path).unwrap();
        editor.state.pads[0].shape = PadShape::RoundRect { radius_ratio: 0.25 };
        editor.with_parts(|state, primitive| {
            pad_to_sketch::mirror_add_pad_to_sketch(&mut state.pads[0], primitive);
        });
        editor.state.selected_pad = Some(0);
    }
    let template = {
        let editor = app.document_state.footprint_editors.get(&path).unwrap();
        assert!(
            !editor.state.pads[0].shape_params.is_empty(),
            "RoundRect mint must populate shape_params"
        );
        editor.state.pads[0].clone()
    };

    for msg in [FootprintEditorMsg::CopyPad, FootprintEditorMsg::PastePad] {
        let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: PrimitiveEdit::Footprint(msg),
        }));
    }

    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    let pasted = editor.state.pads.last().unwrap();
    assert_ne!(
        pasted.sketch_entity_id, template.sketch_entity_id,
        "the paste must own a distinct centre entity"
    );
    assert!(
        pasted.sketch_entity_id.is_some(),
        "an authored sketch will never auto-mint the paste later, so paste must \
         mint it now"
    );
    for (key, value) in &pasted.shape_params {
        assert!(
            template.shape_params.get(key) != Some(value),
            "pasted shape_params[{key}] = {value} still aliases the template — \
             editing the paste would resize the ORIGINAL pad"
        );
    }
}

/// The post-bake refresh must not alias duplicate numbers either.
///
/// `refresh_pads_from_primitive` carries the three volatile link fields
/// across the rebuild through its own number-keyed map — the same
/// last-wins structure, the same aliasing, one function over. Fixing
/// only the reopen path would have left every post-bake refresh
/// handing both duplicate-numbered pads one centre.
#[test]
fn issue142_post_bake_refresh_does_not_alias_duplicate_numbers() {
    let fp = footprint_with_two_pads_sharing_a_number();
    let mut state = FootprintEditorState::from_footprint(&fp);
    let (a, b) = (
        state.pads[0].sketch_entity_id,
        state.pads[1].sketch_entity_id,
    );

    state.refresh_pads_from_primitive(&fp);

    assert_eq!(
        (
            state.pads[0].sketch_entity_id,
            state.pads[1].sketch_entity_id
        ),
        (a, b),
        "the refresh re-pointed a duplicate-numbered pad at the other pad's centre"
    );
    assert_ne!(
        state.pads[0].sketch_entity_id, state.pads[1].sketch_entity_id,
        "the refresh aliased both pads onto ONE centre"
    );
}
