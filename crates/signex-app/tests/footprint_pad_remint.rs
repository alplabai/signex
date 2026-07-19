//! The invariant: exactly ONE owner of per-shape sidecar layout.
//!
//! A transform that changes the pad FRAME (rotate, flip) regenerates
//! the pad's sketch geometry through the mint path, rather than
//! through a second copy of the layout rules. Per-shape layout
//! knowledge lived in three places — the mint functions, the
//! post-solve reverse mirror, and the bbox-corner-only move mirror —
//! and that duplication is why the outline kept drifting out of step
//! with the copper.
//!
//! These tests are written against a PARAMETRIC shape on purpose. The
//! bbox-corner mover is correct for `Rect` by construction, so
//! `Rect`-only coverage cannot see the defect at all: for `Rect` the
//! outline IS the four bbox corners, while a Chamfered / RoundRect /
//! Oval pad also owns anchors and arc centres that the corner mover
//! never touches.

use signex_app::app::{EditMsg, Message, Signex};
use signex_app::library::editor::footprint::state::EditorPad;
use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use signex_sketch::sketch::SketchData;
use std::path::PathBuf;

fn dispatch(app: &mut Signex, path: &PathBuf, msg: FootprintEditorMsg) {
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(msg),
    }));
}

/// The exact repro from issue #390: a Chamfered 2×1 mm pad at the
/// origin with the chamfer on the top-right corner.
fn chamfered_repro_pad() -> EditorPad {
    use signex_library::PadShape;
    use signex_library::primitive::footprint::ChamferedCorners;

    let mut pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    pad.size_mm = (2.0, 1.0);
    pad.shape = PadShape::Chamfered {
        chamfer_ratio: 0.25,
        corners: ChamferedCorners {
            top_right: true,
            ..Default::default()
        },
    };
    pad
}

/// A footprint editor holding one selected pad whose sketch geometry
/// has already been minted, plus the tab wiring `Message::Edit` needs
/// to resolve the active editor.
fn editor_with_minted_pad(stem: &str, mut pad: EditorPad) -> (Signex, PathBuf) {
    use signex_app::app::{FootprintEditorState, TabInfo, TabKind};
    use signex_app::library::editor::footprint::pad_to_sketch::mirror_add_pad_to_sketch;
    use signex_library::{Footprint, FootprintFile};

    let path = PathBuf::from(format!("{stem}.snxfpt"));
    let mut fp = Footprint::empty(stem);
    mirror_add_pad_to_sketch(&mut pad, &mut fp);

    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.pads = vec![pad];
    editor.state.selected_pad = Some(0);

    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: stem.into(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;
    (app, path)
}

/// Every `Point` position in the sketch, sorted, plus the per-kind
/// entity census. Entity IDs are freshly generated on every mint, so
/// two sketches are compared through their geometry rather than their
/// UUIDs. Positions are compared at nanometre resolution — the unit
/// signex coordinates are integral in downstream.
fn geometry_fingerprint(sketch: &SketchData) -> (Vec<(i64, i64)>, [usize; 4]) {
    use signex_sketch::entity::EntityKind;

    let mut points: Vec<(i64, i64)> = Vec::new();
    let mut census = [0usize; 4];
    for e in &sketch.entities {
        match e.kind {
            EntityKind::Point { x, y } => {
                census[0] += 1;
                points.push(((x * 1e6).round() as i64, (y * 1e6).round() as i64));
            }
            EntityKind::Line { .. } => census[1] += 1,
            EntityKind::Arc { .. } => census[2] += 1,
            EntityKind::Circle { .. } => census[3] += 1,
        }
    }
    points.sort_unstable();
    (points, census)
}

/// Position of the pad's centre `Point`.
fn centre_point(sketch: &SketchData, pad: &EditorPad) -> (f64, f64) {
    use signex_sketch::entity::EntityKind;

    sketch
        .entities
        .iter()
        .find(|e| Some(e.id) == pad.sketch_entity_id)
        .and_then(|e| match e.kind {
            EntityKind::Point { x, y } => Some((x, y)),
            _ => None,
        })
        .expect("the pad's centre Point is still a Point")
}

/// Position of the `Point` that a `shape_params` sidecar key names.
fn sidecar_point(sketch: &SketchData, pad: &EditorPad, key: &str) -> (f64, f64) {
    use signex_sketch::entity::EntityKind;

    let raw = pad.shape_params.get(key).unwrap_or_else(|| {
        panic!(
            "pad must carry a `{key}` sidecar binding; it has {:?}",
            pad.shape_params.keys().collect::<Vec<_>>()
        )
    });
    let id = signex_sketch::id::SketchEntityId(
        uuid::Uuid::parse_str(raw).expect("sidecar values are UUID slugs"),
    );
    sketch
        .entities
        .iter()
        .find(|e| e.id == id)
        .and_then(|e| match e.kind {
            EntityKind::Point { x, y } => Some((x, y)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("`{key}` must still resolve to a Point after the transform"))
}

/// THE INVARIANT (a). One `ActiveBarRotateSelection` on the #390 repro
/// must leave the sketch in the state a from-scratch mint of the same
/// pad at the same angle produces.
///
/// The named coordinate: the NE chamfer anchor is minted in the pad
/// frame at (xmax − r, ymin) = (0.75, −0.5). Taken through a 90° frame
/// about the origin that is (0.5, 0.75). Re-placing only the four bbox
/// corners leaves it at (0.75, −0.5) — the corners turn, the chamfer
/// does not, and the outline is a mix of two frames that is neither
/// the old shape nor the new one.
#[test]
fn rotate_leaves_the_sketch_equal_to_a_fresh_mint_at_the_new_angle() {
    use signex_app::library::editor::footprint::pad_to_sketch::mirror_add_pad_to_sketch;
    use signex_library::Footprint;

    let (mut app, path) = editor_with_minted_pad("chamfer-rotate", chamfered_repro_pad());

    dispatch(
        &mut app,
        &path,
        FootprintEditorMsg::ActiveBarRotateSelection,
    );

    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    let pad = &editor.state.pads[0];
    assert_eq!(pad.rotation_deg, 90.0, "pre-condition: the pad turned 90°");
    let sketch = editor
        .primitive()
        .sketch
        .as_ref()
        .expect("the pad minted a sketch");

    let anchor = sidecar_point(sketch, pad, "chamfer_ne_anchor1");
    assert!(
        (anchor.0 - 0.5).abs() < 1e-9 && (anchor.1 - 0.75).abs() < 1e-9,
        "the NE chamfer anchor must land at (0.5, 0.75) — the pad-frame point (0.75, −0.5) \
         taken through the 90° frame. Got {anchor:?}; (0.75, −0.5) means it was never \
         regenerated and the outline mixes the pre- and post-rotate frames"
    );

    // The whole-sketch equality, not just the one anchor: an
    // incremental rotate and a from-scratch mint at 90° have to be
    // indistinguishable, or some part of the geometry was regenerated
    // through rules the mint path does not own.
    let mut fresh_pad = chamfered_repro_pad();
    fresh_pad.rotation_deg = 90.0;
    let mut fresh_fp = Footprint::empty("chamfer-rotate-fresh");
    mirror_add_pad_to_sketch(&mut fresh_pad, &mut fresh_fp);
    let fresh = fresh_fp.sketch.as_ref().expect("fresh mint made a sketch");

    assert_eq!(
        geometry_fingerprint(sketch),
        geometry_fingerprint(fresh),
        "the incrementally-rotated sketch must equal a from-scratch mint of the same pad at \
         90°"
    );
}

/// THE INVARIANT (b). `signex_bake::pad` reads `PadAttr::shape` off the
/// sketch, so that field IS the baked shape. After a flip the editor
/// pad's chamfer corners are mirrored; if the sketch attribute still
/// carries the pre-flip corners then the editor and the bake disagree
/// about the pad and the exported footprint chamfers the wrong corner.
///
/// Asserting the editor value alone cannot see this — it is the two
/// representations AGREEING that matters.
#[test]
fn flip_keeps_the_baked_shape_equal_to_the_editor_shape() {
    use signex_library::PadShape;
    use signex_sketch::attr::PadShape as SkPadShape;

    let (mut app, path) = editor_with_minted_pad("chamfer-flip", chamfered_repro_pad());

    dispatch(&mut app, &path, FootprintEditorMsg::ActiveBarFlipSelection);

    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    let pad = &editor.state.pads[0];
    let PadShape::Chamfered {
        corners: editor_corners,
        ..
    } = &pad.shape
    else {
        panic!(
            "flip must not change the shape variant, got {:?}",
            pad.shape
        );
    };
    assert!(
        editor_corners.top_left && !editor_corners.top_right,
        "pre-condition: the editor pad's chamfer mirrored TOP-RIGHT → TOP-LEFT, got \
         {editor_corners:?}"
    );

    let sketch = editor.primitive().sketch.as_ref().expect("sketch present");
    let attr = sketch
        .entities
        .iter()
        .find(|e| Some(e.id) == pad.sketch_entity_id)
        .and_then(|e| e.pad.as_ref())
        .expect("the pad's centre Point still carries its PadAttr");
    let SkPadShape::Chamfered {
        corners: baked_corners,
        ..
    } = &attr.shape
    else {
        panic!(
            "the sketch attribute must still describe a Chamfered pad, got {:?}",
            attr.shape
        );
    };

    assert_eq!(
        (
            baked_corners.top_left,
            baked_corners.top_right,
            baked_corners.bottom_left,
            baked_corners.bottom_right,
        ),
        (
            editor_corners.top_left,
            editor_corners.top_right,
            editor_corners.bottom_left,
            editor_corners.bottom_right,
        ),
        "the shape `signex_bake::pad` reads off `PadAttr` must equal the shape the editor \
         shows. Baked {baked_corners:?} vs editor {editor_corners:?} — two representations, \
         two answers, and the chamfer is fabricated on the wrong corner"
    );
}

/// THE INVARIANT (c). The rotate now DROPS and re-mints the sidecar,
/// which is a far larger mutation than moving four points. One Ctrl+Z
/// still has to put the sketch back exactly as it was, or the re-mint
/// is a one-way loss of the user's outline.
///
/// NOT A PROOF OF THE FIX, and do not read it as one. This test does
/// not go red when the re-mint is reverted: the behaviour it replaces
/// is a four-point corner move, which is trivially undoable, so it
/// passes either way. It is a FORWARD-LOOKING guard — it fails the day
/// someone makes the re-mint one-way — and it is kept for that.
///
/// Which test guards which mechanism, precisely — the same trap one
/// level down, since (b) was previously listed as a proof of the
/// re-mint and is not one:
///
/// - (a) guards THE RE-MINT on rotate. Reverting the
///   `remint_pad_geometry` call in `updates/active_bar.rs` to
///   `mirror_move_pad_in_sketch` turns it red on its own.
/// - (b) guards `attr::mirror_shape` — the editor-shape → `PadAttr`
///   mirror the flip needs. It does NOT discriminate the re-mint:
///   revert both `remint_pad_geometry` calls in `active_bar.rs` and it
///   stays green. It goes red only when `mirror_shape` is also
///   reverted, which is what the earlier whole-commit revert did.
/// - (d), (e), (g) and (h) each guard the re-mint at one further frame-
///   change site (Properties-panel rotation, the size / shape funnel,
///   the Sketch-mode edge drag, the Sketch-mode corner drag).
/// - (f) guards the sidecar TRANSLATION on the six move sites.
/// - `a_streamed_sketch_edge_drag_keeps_resizing_after_the_first_tick`
///   guards the re-mint at (g) and (h) being an IN-PLACE one. It is red
///   for a plain drop-and-re-mint, which invalidates the id the pointer
///   is dragging.
/// - `sketch_centre_drag_carries_the_chamfer_anchor_with_it` proves
///   nothing — see its own comment.
///
/// Green tests are not proofs; each of the above was confirmed red with
/// its own mechanism reverted and green with it restored.
#[test]
fn one_undo_after_a_rotate_restores_the_prior_sketch_geometry() {
    let (mut app, path) = editor_with_minted_pad("chamfer-rotate-undo", chamfered_repro_pad());

    let before = {
        let editor = app.document_state.footprint_editors.get(&path).unwrap();
        geometry_fingerprint(editor.primitive().sketch.as_ref().expect("sketch present"))
    };

    dispatch(
        &mut app,
        &path,
        FootprintEditorMsg::ActiveBarRotateSelection,
    );
    {
        let editor = app.document_state.footprint_editors.get(&path).unwrap();
        let after = geometry_fingerprint(editor.primitive().sketch.as_ref().unwrap());
        assert_ne!(
            before, after,
            "pre-condition: the rotate actually changed the sketch geometry"
        );
    }

    let _ = app.update(Message::Edit(EditMsg::Undo));

    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    let pad = &editor.state.pads[0];
    assert_eq!(
        pad.rotation_deg, 0.0,
        "a single Ctrl+Z restores the pre-rotate angle"
    );
    assert_eq!(
        geometry_fingerprint(editor.primitive().sketch.as_ref().expect("sketch present")),
        before,
        "a single Ctrl+Z must restore the sketch geometry the re-mint replaced"
    );
    assert!(
        pad.sketch_entity_id.is_some() && pad.corner_entity_ids.is_some(),
        "undo restores the pad's sketch links too, or the pad is orphaned from its outline"
    );
}

/// THE INVARIANT (d), the Properties-panel rotation field. Structurally
/// identical to the active-bar Rotate arm — same frame change, same
/// obligation — and it is the site the branch first regenerated
/// through the bbox-corner mover, so the corners turned into the 90°
/// frame while the chamfer anchor stayed at its 0° position and the
/// outline crossed itself.
///
/// The Rect-shaped sibling of this test in `footprint_pad_rotation.rs`
/// cannot see that: for a `Rect` the outline IS the four bbox corners
/// and the corner mover is correct by construction. Only a parametric
/// shape exposes it.
#[test]
fn properties_panel_rotation_regenerates_the_chamfer_anchor() {
    use signex_app::dock::DockMessage;
    use signex_app::panels::PanelMsg;

    let (mut app, path) = editor_with_minted_pad("chamfer-panel-rotate", chamfered_repro_pad());

    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::FpEditorSetSelectedPadRotation {
            idx: 0,
            value: "90".into(),
        },
    )));

    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    let pad = &editor.state.pads[0];
    assert_eq!(pad.rotation_deg, 90.0, "pre-condition: the panel set 90°");
    let sketch = editor.primitive().sketch.as_ref().expect("sketch present");

    let anchor = sidecar_point(sketch, pad, "chamfer_ne_anchor1");
    assert!(
        (anchor.0 - 0.5).abs() < 1e-9 && (anchor.1 - 0.75).abs() < 1e-9,
        "the NE chamfer anchor must land at (0.5, 0.75). Got {anchor:?}; (0.75, −0.5) means \
         the panel edit re-placed the bbox corners alone and left the anchor in the pre-rotate \
         frame — an outline mixing two frames"
    );
}

/// THE INVARIANT (e), the size / shape funnel. `with_selected_pad`
/// carries `size_mm` and `shape` edits, both of which move the whole
/// outline. Widening a Chamfered pad through the bbox-corner mover
/// pushed the corners out to the new extents and left the chamfer
/// anchors on the old ones.
#[test]
fn resizing_a_chamfered_pad_regenerates_its_chamfer_anchor() {
    use signex_app::dock::DockMessage;
    use signex_app::panels::PanelMsg;

    let (mut app, path) = editor_with_minted_pad("chamfer-resize", chamfered_repro_pad());

    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::FpEditorSetSelectedPadSizeX {
            idx: 0,
            value: "3".into(),
        },
    )));

    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    let pad = &editor.state.pads[0];
    assert_eq!(pad.size_mm, (3.0, 1.0), "pre-condition: the panel set 3 mm");
    let sketch = editor.primitive().sketch.as_ref().expect("sketch present");

    // 3×1 mm at the origin: xmax = 1.5, ymin = −0.5, and the chamfer
    // length r = 0.25 × min(3, 1) = 0.25, so anchor1 = (1.25, −0.5).
    let anchor = sidecar_point(sketch, pad, "chamfer_ne_anchor1");
    assert!(
        (anchor.0 - 1.25).abs() < 1e-9 && (anchor.1 + 0.5).abs() < 1e-9,
        "the NE chamfer anchor must follow the new 3 mm extents to (1.25, −0.5). Got \
         {anchor:?}; (0.75, −0.5) is the pre-resize position, i.e. the corners widened and \
         the chamfer did not"
    );
}

/// THE INVARIANT (f), the TRANSLATION siblings — pad drag, nudge,
/// Move-By, align-to-grid, move-origin-to-grid, align/distribute. Six
/// call sites, all routing through `mirror_move_pad_in_sketch`, and
/// the pad frame's ORIGIN is as much a part of the frame as its angle:
/// every anchor is placed through `local_to_world_mm`, which is
/// centred on `position_mm`.
///
/// The corner mover moved the centre and the four bbox corners and
/// nothing else, so a dragged Chamfered pad left its chamfer anchors
/// behind at the old location entirely. This one is fixed by
/// translating the sidecar rather than re-minting it: a translation
/// moves every owned point by the same delta, and a re-mint on every
/// drag frame would destroy the user's constraints for nothing.
#[test]
fn translating_a_chamfered_pad_carries_its_chamfer_anchor_with_it() {
    let (mut app, path) = editor_with_minted_pad("chamfer-translate", chamfered_repro_pad());

    dispatch(
        &mut app,
        &path,
        FootprintEditorMsg::MovePad {
            idx: 0,
            x_mm: 5.0,
            y_mm: 3.0,
        },
    );

    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    let pad = &editor.state.pads[0];
    assert_eq!(
        pad.position_mm,
        (5.0, 3.0),
        "pre-condition: the pad moved to (5, 3)"
    );
    let sketch = editor.primitive().sketch.as_ref().expect("sketch present");

    let anchor = sidecar_point(sketch, pad, "chamfer_ne_anchor1");
    assert!(
        (anchor.0 - 5.75).abs() < 1e-9 && (anchor.1 - 2.5).abs() < 1e-9,
        "the NE chamfer anchor must ride the (+5, +3) translation to (5.75, 2.5). Got \
         {anchor:?}; (0.75, −0.5) means it never left the origin while the copper moved"
    );
}

/// The `Line` whose two endpoints both sit at world y == `y` — a pad's
/// top or bottom bbox edge, the handle a Sketch-mode edge drag grabs.
fn horizontal_edge_at_y(app: &Signex, path: &PathBuf, y: f64) -> signex_sketch::id::SketchEntityId {
    use signex_sketch::entity::EntityKind;
    use signex_sketch::id::SketchEntityId;

    let editor = app.document_state.footprint_editors.get(path).unwrap();
    let sketch = editor.primitive().sketch.as_ref().expect("sketch present");
    let pos = |id: SketchEntityId| -> Option<(f64, f64)> {
        sketch
            .entities
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| match e.kind {
                EntityKind::Point { x, y } => Some((x, y)),
                _ => None,
            })
    };
    sketch
        .entities
        .iter()
        .find_map(|e| match e.kind {
            EntityKind::Line { start, end } => {
                let (_, sy) = pos(start)?;
                let (_, ey) = pos(end)?;
                ((sy - y).abs() < 1e-9 && (ey - y).abs() < 1e-9).then_some(e.id)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("the pad must mint a horizontal edge at y = {y}"))
}

/// THE INVARIANT (g), the Sketch-mode edge drag. Dragging a pad edge
/// changes both extents and centre — the same frame change the
/// Properties-panel Size field makes — but the handler regenerated only
/// the four bbox corners, through its `target_positions` catch-up loop.
///
/// The discriminating drag is the one that changes `min(w, h)`: a
/// Chamfered pad's chamfer length is `ratio × min(w, h)`, so widening
/// the SHORT axis makes the chamfer longer and the anchors move by
/// something other than the edge delta. A drag along the long axis
/// translates the anchors rigidly and happens to land them where a
/// fresh mint would — which is why this test drags the top edge, not
/// the right one.
///
/// 2×1 mm dragged 0.5 mm up the top edge is 2×1.5 mm centred at
/// (0, −0.25); the chamfer length becomes 0.25 × 1.5 = 0.375, so the NE
/// anchor sits at pad-local (1 − 0.375, −0.75) = world (0.625, −1.0).
#[test]
fn sketch_edge_drag_regenerates_the_chamfer_anchor() {
    use signex_app::library::editor::footprint::pad_to_sketch::mirror_add_pad_to_sketch;
    use signex_library::Footprint;

    let (mut app, path) = editor_with_minted_pad("chamfer-edge-drag", chamfered_repro_pad());
    let edge_id = horizontal_edge_at_y(&app, &path, -0.5);

    dispatch(
        &mut app,
        &path,
        FootprintEditorMsg::SketchMoveLine {
            id: edge_id,
            dx: 0.0,
            dy: -0.5,
        },
    );

    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    let pad = &editor.state.pads[0];
    assert_eq!(
        (pad.size_mm, pad.position_mm),
        ((2.0, 1.5), (0.0, -0.25)),
        "pre-condition: the edge drag resized the pad frame"
    );
    let sketch = editor.primitive().sketch.as_ref().expect("sketch present");

    let anchor = sidecar_point(sketch, pad, "chamfer_ne_anchor1");
    assert!(
        (anchor.0 - 0.625).abs() < 1e-9 && (anchor.1 + 1.0).abs() < 1e-9,
        "the NE chamfer anchor must follow the new 1.5 mm short extent to (0.625, −1.0). Got          {anchor:?}; (0.75, −1.0) is the old chamfer length carried rigidly along — the bbox          corners took the new frame and the chamfer kept the old one"
    );

    // Whole-sketch equality, as in (a): an edge drag and a from-scratch
    // mint of the resulting pad have to be indistinguishable, or some
    // part of the geometry was regenerated through rules the mint path
    // does not own.
    let mut fresh_pad = chamfered_repro_pad();
    fresh_pad.size_mm = (2.0, 1.5);
    fresh_pad.position_mm = (0.0, -0.25);
    let mut fresh_fp = Footprint::empty("chamfer-edge-drag-fresh");
    mirror_add_pad_to_sketch(&mut fresh_pad, &mut fresh_fp);
    let fresh = fresh_fp.sketch.as_ref().expect("fresh mint made a sketch");

    assert_eq!(
        geometry_fingerprint(sketch),
        geometry_fingerprint(fresh),
        "the dragged sketch must equal a from-scratch mint of the resulting 2×1.5 mm pad"
    );
}

/// The other half of (g), and the reason the fix cannot be a plain
/// drop-and-re-mint. A live edge drag streams one `SketchMoveLine` per
/// cursor tick, all carrying the id the pointer latched at press
/// (`drag_tick_line`, `canvas/input/pointer.rs`). Re-minting replaces
/// the outline with fresh UUIDs, so the second tick would address a
/// `Line` that no longer exists and the drag would freeze after the
/// first frame — the pad stuck one tick wide while the cursor keeps
/// moving.
///
/// Two ticks of 0.25 mm must resize exactly as far as one of 0.5 mm.
#[test]
fn a_streamed_sketch_edge_drag_keeps_resizing_after_the_first_tick() {
    let (mut app, path) = editor_with_minted_pad("chamfer-edge-drag-2", chamfered_repro_pad());
    let edge_id = horizontal_edge_at_y(&app, &path, -0.5);

    for _ in 0..2 {
        dispatch(
            &mut app,
            &path,
            FootprintEditorMsg::SketchMoveLine {
                id: edge_id,
                dx: 0.0,
                dy: -0.25,
            },
        );
    }

    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    let pad = &editor.state.pads[0];
    assert_eq!(
        (pad.size_mm, pad.position_mm),
        ((2.0, 1.5), (0.0, -0.25)),
        "two 0.25 mm ticks of one drag must resize as far as one 0.5 mm tick. (2.0, 1.25)          means the second tick addressed an entity the first tick destroyed and the drag          froze one frame in"
    );
}

/// THE INVARIANT (h), the Sketch-mode corner drag — the edge drag's
/// structurally identical sibling in the same file. It recomputes the
/// pad bbox from the four corner Points and rewrites `size_mm` /
/// `position_mm`, which is the same frame change, and it re-placed the
/// same four corners and nothing else.
///
/// Dragged in two ticks, as the pointer streams it, so this also guards
/// the drag continuity the edge-drag test guards: the corner Point's id
/// has to survive the first tick or the second addresses nothing.
///
/// Pulling the NE corner 0.5 mm up gives the same 2×1.5 mm pad at
/// (0, −0.25) the edge-drag test produces, so the NE chamfer anchor
/// lands in the same place: (0.625, −1.0).
#[test]
fn sketch_corner_drag_regenerates_the_chamfer_anchor() {
    let (mut app, path) = editor_with_minted_pad("chamfer-corner-drag", chamfered_repro_pad());
    let ne_corner = {
        let editor = app.document_state.footprint_editors.get(&path).unwrap();
        editor.state.pads[0]
            .corner_entity_ids
            .expect("a Chamfered pad mints bbox corners")[0]
    };

    for _ in 0..2 {
        dispatch(
            &mut app,
            &path,
            FootprintEditorMsg::SketchMovePoint {
                id: ne_corner,
                dx: 0.0,
                dy: -0.25,
            },
        );
    }

    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    let pad = &editor.state.pads[0];
    assert_eq!(
        (pad.size_mm, pad.position_mm),
        ((2.0, 1.5), (0.0, -0.25)),
        "pre-condition: two ticks of the corner drag resized the pad frame. (2.0, 1.25) means \
         the second tick addressed a Point the first tick destroyed"
    );
    let sketch = editor.primitive().sketch.as_ref().expect("sketch present");

    let anchor = sidecar_point(sketch, pad, "chamfer_ne_anchor1");
    assert!(
        (anchor.0 - 0.625).abs() < 1e-9 && (anchor.1 + 1.0).abs() < 1e-9,
        "the NE chamfer anchor must follow the new 1.5 mm short extent to (0.625, −1.0). Got \
         {anchor:?}; (0.75, −1.0) is the old chamfer length, i.e. the bbox corners took the \
         new frame and the chamfer kept the old one"
    );
}

/// The Sketch-mode CENTRE drag — the one translation path that does not
/// route through `mirror_move_pad_in_sketch`, so the sweep flagged it
/// as a candidate sibling of (f). It is NOT one, and this test is NOT a
/// proof of a fix: it passes on the code as it stands, and it passed
/// before this round too.
///
/// The handler translates the centre and the four bbox corners
/// explicitly, but it does so through `apply_sketch_edit_with_warnings`
/// — the SOLVER pass — and the chamfer anchors are constrained to the
/// outline, so the solve carries them. Translating them a second time
/// by hand, which is what the Pads-mode sites need, doubles the delta:
/// it lands the anchor at (10.75, 5.5) instead of (5.75, 2.5).
///
/// Kept as the guard that fact deserves. Anyone who changes this
/// handler to bypass the solver has to make the sidecar ride some
/// other way, and this is what tells them.
#[test]
fn sketch_centre_drag_carries_the_chamfer_anchor_with_it() {
    let (mut app, path) = editor_with_minted_pad("chamfer-centre-drag", chamfered_repro_pad());
    let centre = {
        let editor = app.document_state.footprint_editors.get(&path).unwrap();
        editor.state.pads[0]
            .sketch_entity_id
            .expect("the pad minted a centre Point")
    };

    dispatch(
        &mut app,
        &path,
        FootprintEditorMsg::SketchMovePoint {
            id: centre,
            dx: 5.0,
            dy: 3.0,
        },
    );

    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    let pad = &editor.state.pads[0];
    let sketch = editor.primitive().sketch.as_ref().expect("sketch present");
    // The centre Point, not `pad.position_mm`: this handler adds the
    // drag delta to `position_mm` on top of the value the solve+bake
    // refresh already wrote, so the editor mirror reads double. That
    // double-count is pre-existing and untouched here — the sidecar
    // rides the geometry, so the geometry is what this asserts.
    assert_eq!(
        centre_point(sketch, pad),
        (5.0, 3.0),
        "pre-condition: the centre drag moved the pad's centre Point to (5, 3)"
    );

    let anchor = sidecar_point(sketch, pad, "chamfer_ne_anchor1");
    assert!(
        (anchor.0 - 5.75).abs() < 1e-9 && (anchor.1 - 2.5).abs() < 1e-9,
        "the NE chamfer anchor must ride the (+5, +3) drag to (5.75, 2.5). Got {anchor:?}; \
         (0.75, −0.5) means it never left the origin while the copper moved"
    );
}
