//! Pad rotation is a geometry input, and the active-bar transforms act
//! on the whole selection.
//!
//! The pre-existing coverage in `regression.rs`
//! (`v026g_rotate_selection_increments_rotation_by_90_degrees` and
//! siblings) only ever asserted that the `rotation_deg` FIELD changed
//! on a single-pad selection. That is exactly why three defects
//! survived: nothing checked that rotation reached the geometry, and
//! nothing checked a multi-pad selection. These tests assert GEOMETRY
//! and MULTI-pad.

use signex_app::app::{EditMsg, Message, Signex};
use signex_app::library::editor::footprint::state::EditorPad;
use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use std::path::PathBuf;
use tempfile::TempDir;

/// Fresh app + a footprint editor holding `count` default pads, with
/// the active tab pointed at it so `Message::Edit(EditMsg::Undo)`
/// resolves through `active_footprint_editor_path()`.
fn fixture(stem: &str, count: usize) -> (Signex, PathBuf, TempDir) {
    use signex_app::app::{FootprintEditorState, TabInfo, TabKind};
    use signex_library::{Footprint, FootprintFile};

    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join(format!("{stem}.snxfpt"));
    std::fs::write(&path, b"{}").expect("write .snxfpt placeholder");

    let file = FootprintFile::from_footprint(Footprint::empty(stem));
    let mut editor = FootprintEditorState::new(path.clone(), file);
    for i in 0..count {
        editor.state.pads.push(EditorPad::new_default(
            (i + 1).to_string(),
            (i as f64 * 3.0, 0.0),
        ));
    }

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
    (app, path, tmp)
}

fn dispatch(app: &mut Signex, path: &PathBuf, msg: FootprintEditorMsg) {
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(msg),
    }));
}

/// A 2×1 mm pad turned 90° occupies ±0.5 mm in X and ±1.0 mm in Y.
/// The old `contains_mm` tested the un-rotated box, so it answered
/// exactly backwards on both probes.
#[test]
fn rotated_pad_hit_tests_against_the_turned_copper() {
    let mut pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    pad.size_mm = (2.0, 1.0);
    pad.rotation_deg = 90.0;

    assert!(
        pad.contains_mm(0.0, 0.9),
        "(0, 0.9) is inside the rotated copper (half-height 1.0) and must hit"
    );
    assert!(
        !pad.contains_mm(0.9, 0.0),
        "(0.9, 0) is outside the rotated copper (half-width 0.5) and must MISS \
         — it only looks like a hit against the un-rotated box"
    );
}

/// The auto-fit courtyard is built from the pad extents. Reading the
/// un-rotated box let a turned pad stick out of its own courtyard.
#[test]
fn courtyard_encloses_the_rotated_pad() {
    use signex_app::library::editor::footprint::state::FootprintEditorState as CanvasState;

    let mut state = CanvasState::empty();
    let mut pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    pad.size_mm = (2.0, 1.0);
    pad.rotation_deg = 90.0;
    state.pads.push(pad);
    state.auto_fit_courtyard = true;
    state.recompute_courtyard();

    let court = state.courtyard_mm.expect("auto-fit produced a courtyard");
    assert!(
        court.max_y >= 1.0 && court.min_y <= -1.0,
        "rotated pad reaches ±1.0 mm in Y; courtyard is [{}, {}] and does not contain it",
        court.min_y,
        court.max_y
    );
    assert!(
        court.max_x >= 0.5 && court.min_x <= -0.5,
        "courtyard must still cover the rotated pad in X: [{}, {}]",
        court.min_x,
        court.max_x
    );
}

/// Rotate acted on `selected_pad` alone; pads 1 and 2 never turned.
#[test]
fn rotate_turns_every_pad_in_the_selection() {
    let (mut app, path, _tmp) = fixture("multi-rotate", 3);
    {
        let editor = app.document_state.footprint_editors.get_mut(&path).unwrap();
        editor.state.selected_pad = Some(0);
        editor.state.selected_pads_extra = vec![1, 2];
    }

    dispatch(
        &mut app,
        &path,
        FootprintEditorMsg::ActiveBarRotateSelection,
    );

    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    for (i, pad) in editor.state.pads.iter().enumerate() {
        assert_eq!(
            pad.rotation_deg, 90.0,
            "pad {i} must be rotated — Rotate acts on the whole selection, not just the primary"
        );
    }
}

/// The fab-error case: a partial flip leaves some pads on F.* and some
/// on B.*, which is a board that cannot be built.
#[test]
fn flip_moves_every_selected_pad_to_the_back_side() {
    let (mut app, path, _tmp) = fixture("multi-flip", 3);
    {
        let editor = app.document_state.footprint_editors.get_mut(&path).unwrap();
        editor.state.selected_pad = Some(0);
        editor.state.selected_pads_extra = vec![1, 2];
        for pad in &editor.state.pads {
            assert!(
                pad.layers.iter().all(|l| l.as_str().starts_with("F.")),
                "fixture starts front-side"
            );
        }
    }

    dispatch(&mut app, &path, FootprintEditorMsg::ActiveBarFlipSelection);

    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    for (i, pad) in editor.state.pads.iter().enumerate() {
        assert!(
            pad.layers.iter().all(|l| l.as_str().starts_with("B.")),
            "pad {i} still carries a front-side layer after Flip: {:?} \
             — mixed F./B. across one footprint is a fabrication error",
            pad.layers
                .iter()
                .map(|l| l.as_str().to_string())
                .collect::<Vec<_>>()
        );
    }
}

/// `apply_footprint_primitive_edit` blanket-pushes a snapshot for every
/// mutating message, and Rotate is not on its exemption list — so the
/// arm must NOT push its own. One Ctrl+Z has to reverse the whole
/// multi-pad rotate; two would mean the history got double-stacked.
#[test]
fn one_undo_reverses_the_whole_multi_pad_rotate() {
    let (mut app, path, _tmp) = fixture("multi-rotate-undo", 3);
    {
        let editor = app.document_state.footprint_editors.get_mut(&path).unwrap();
        editor.state.selected_pad = Some(0);
        editor.state.selected_pads_extra = vec![1, 2];
        for pad in editor.state.pads.iter_mut() {
            pad.rotation_deg = 45.0;
        }
    }

    dispatch(
        &mut app,
        &path,
        FootprintEditorMsg::ActiveBarRotateSelection,
    );
    {
        let editor = app.document_state.footprint_editors.get(&path).unwrap();
        assert!(
            editor.state.pads.iter().all(|p| p.rotation_deg == 135.0),
            "pre-condition: all three rotated 45° → 135°"
        );
    }

    let _ = app.update(Message::Edit(EditMsg::Undo));

    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    for (i, pad) in editor.state.pads.iter().enumerate() {
        assert_eq!(
            pad.rotation_deg, 45.0,
            "a SINGLE Ctrl+Z must restore pad {i} to its pre-rotate angle"
        );
    }
}

/// Touching Line is a sibling of rubber-band select and scored pads
/// against the same un-rotated box. A 2×1 mm pad turned 90° occupies
/// ±0.5 mm in X and ±1.0 mm in Y, so the un-rotated box answers
/// backwards on both of these lines.
#[test]
fn touching_line_scores_the_rotated_pad_not_the_unrotated_box() {
    let probe = |x: f64, y: f64| -> bool {
        let (mut app, path, _tmp) = fixture("touching-line-rotated", 1);
        {
            let editor = app.document_state.footprint_editors.get_mut(&path).unwrap();
            let pad = &mut editor.state.pads[0];
            pad.position_mm = (0.0, 0.0);
            pad.size_mm = (2.0, 1.0);
            pad.rotation_deg = 90.0;
            // A short segment straddling the probe point, drawn well
            // clear of the pad centre in the other axis.
            editor.state.touching_line_active = true;
            editor.state.touching_line_first = Some((x - 0.01, y - 0.01));
        }
        dispatch(
            &mut app,
            &path,
            FootprintEditorMsg::TouchingLineCommit {
                x_mm: x + 0.01,
                y_mm: y + 0.01,
            },
        );
        let editor = app.document_state.footprint_editors.get(&path).unwrap();
        editor.state.selected_pad.is_some()
    };

    assert!(
        !probe(0.9, 0.0),
        "a Touching Line at x = 0.9 crosses no copper — the rotated pad only reaches ±0.5 mm \
         in X — yet the un-rotated box selects it"
    );
    assert!(
        probe(0.0, 0.9),
        "a Touching Line at y = 0.9 crosses the real copper (±1.0 mm in Y) and must select \
         the pad; the un-rotated box misses it"
    );
}

/// Flip mirrors the pad's copper to the other side. `signex_bake::pad`
/// consumes the stored fields verbatim with no side-based mirroring of
/// its own, so the stored data IS the geometry and the WHOLE
/// mirror-sensitive set has to move under `x → -x`, not just the angle.
///
/// Mirroring a subset bakes a shape that is neither the front nor the
/// back one: a Chamfered pad flipped with its angle negated but its
/// corner flags left alone keeps the chamfer on the wrong corner and
/// the part will not seat.
#[test]
fn flip_mirrors_every_mirror_sensitive_field_of_every_selected_pad() {
    use signex_library::PadShape;
    use signex_library::primitive::footprint::ChamferedCorners;

    let (mut app, path, _tmp) = fixture("multi-flip-rotation", 3);
    {
        let editor = app.document_state.footprint_editors.get_mut(&path).unwrap();
        editor.state.selected_pad = Some(0);
        editor.state.selected_pads_extra = vec![1, 2];
        for pad in editor.state.pads.iter_mut() {
            pad.rotation_deg = 45.0;
            pad.shape = PadShape::Chamfered {
                chamfer_ratio: 0.25,
                corners: ChamferedCorners {
                    top_right: true,
                    ..Default::default()
                },
            };
            pad.copper_offset_x_mm = Some(0.3);
            pad.copper_offset_y_mm = Some(0.4);
            pad.hole_rotation_deg = Some(30.0);
        }
    }

    dispatch(&mut app, &path, FootprintEditorMsg::ActiveBarFlipSelection);

    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    for (i, pad) in editor.state.pads.iter().enumerate() {
        assert_eq!(
            pad.rotation_deg, 315.0,
            "pad {i}: flipping to the back mirrors the copper, so 45° must become −45° \
             (315°); leaving it at +45° bakes a footprint that cannot be assembled"
        );
        match &pad.shape {
            PadShape::Chamfered { corners, .. } => {
                assert!(
                    corners.top_left && !corners.top_right,
                    "pad {i}: the chamfer was on the TOP-RIGHT corner; mirroring the pad puts \
                     it on the TOP-LEFT. Got {corners:?} — a chamfer left on the un-mirrored \
                     corner means the part will not seat"
                );
                assert!(
                    !corners.bottom_left && !corners.bottom_right,
                    "pad {i}: the bottom corners were both off and must stay off: {corners:?}"
                );
            }
            other => panic!("pad {i}: flip must not change the shape variant, got {other:?}"),
        }
        assert_eq!(
            pad.copper_offset_x_mm,
            Some(-0.3),
            "pad {i}: an X copper offset is measured in the pad frame and reverses under the \
             mirror; leaving it at +0.3 puts the copper on the wrong side of the hole"
        );
        assert_eq!(
            pad.copper_offset_y_mm,
            Some(0.4),
            "pad {i}: the Y offset is along the mirror axis and must NOT change"
        );
        assert_eq!(
            pad.hole_rotation_deg,
            Some(330.0),
            "pad {i}: a slot/rectangular hole turns with the copper — 30° mirrors to −30° \
             (330°), or the slot no longer lines up with the pad it sits in"
        );
    }
}

/// The rotate arm mutated `rotation_deg` and called only
/// `sync_pads_to_primitive`, which writes `fp.pads` + the attribute
/// mirror and never repositions `corner_entity_ids`. The corner
/// `Point`s are moved by `mirror_move_pad_in_sketch` alone, which the
/// two structurally identical align arms in the same file DO call.
///
/// Result: copper rendered at 90° while the sketch construction outline
/// still showed the 0° corners — the exact "derived geometry no longer
/// matches the copper" failure this branch exists to fix, reintroduced
/// by the very button issue #390 is about.
///
/// This drives the real `ActiveBarRotateSelection` message and asserts
/// the sketch `Point` positions; the mint-time closure invariants
/// cannot see this because they only ever run at mint time.
#[test]
fn rotate_moves_the_sketch_outline_corners_to_match_the_turned_copper() {
    let (mut app, path, corner_ids) = sketched_pad_fixture("rotate-outline", (2.0, 1.0));

    dispatch(
        &mut app,
        &path,
        FootprintEditorMsg::ActiveBarRotateSelection,
    );

    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    let pad = &editor.state.pads[0];
    assert_eq!(pad.rotation_deg, 90.0, "pre-condition: the pad turned 90°");
    assert_corners_match_pad(editor, &corner_ids, pad, "after ActiveBarRotateSelection");
}

/// Same defect on the Flip arm — it negates the angle, so
/// `rotated_corners_mm()` moves and the outline has to follow.
#[test]
fn flip_moves_the_sketch_outline_corners_to_match_the_mirrored_copper() {
    let (mut app, path, corner_ids) = sketched_pad_fixture("flip-outline", (2.0, 1.0));
    {
        let editor = app.document_state.footprint_editors.get_mut(&path).unwrap();
        editor.state.pads[0].rotation_deg = 30.0;
    }

    dispatch(&mut app, &path, FootprintEditorMsg::ActiveBarFlipSelection);

    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    let pad = &editor.state.pads[0];
    assert_eq!(
        pad.rotation_deg, 330.0,
        "pre-condition: 30° mirrored to −30°"
    );
    assert_corners_match_pad(editor, &corner_ids, pad, "after ActiveBarFlipSelection");
}

/// The Properties-panel rotation field is the third sibling: it also
/// writes `rotation_deg` and syncs without re-placing the corners.
#[test]
fn properties_panel_rotation_moves_the_sketch_outline_corners() {
    use signex_app::dock::DockMessage;
    use signex_app::panels::PanelMsg;

    let (mut app, path, corner_ids) = sketched_pad_fixture("panel-rotate-outline", (2.0, 1.0));

    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::FpEditorSetSelectedPadRotation {
            idx: 0,
            value: "90".into(),
        },
    )));

    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    let pad = &editor.state.pads[0];
    assert_eq!(pad.rotation_deg, 90.0, "pre-condition: the panel set 90°");
    assert_corners_match_pad(editor, &corner_ids, pad, "after the panel rotation edit");
}

/// A footprint editor holding one selected `Rect` pad at the origin
/// with its sketch outline already minted. Returns the four corner
/// `Point` ids in `[ne, se, sw, nw]` order.
fn sketched_pad_fixture(
    stem: &str,
    size_mm: (f64, f64),
) -> (Signex, PathBuf, [signex_sketch::id::SketchEntityId; 4]) {
    use signex_app::app::{FootprintEditorState, TabInfo, TabKind};
    use signex_app::library::editor::footprint::pad_to_sketch::mirror_add_pad_to_sketch;
    use signex_library::{Footprint, FootprintFile, PadShape};

    let path = PathBuf::from(format!("{stem}.snxfpt"));
    let mut fp = Footprint::empty(stem);
    let mut pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    pad.shape = PadShape::Rect;
    pad.size_mm = size_mm;
    mirror_add_pad_to_sketch(&mut pad, &mut fp);
    let corner_ids = pad
        .corner_entity_ids
        .expect("a Rect pad mints four outline-corner Points");

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
    (app, path, corner_ids)
}

/// Every outline-corner `Point` must sit exactly where the pad's real
/// copper corner is. Anything else is a construction outline that
/// disagrees with the copper it outlines.
fn assert_corners_match_pad(
    editor: &signex_app::app::FootprintEditorState,
    corner_ids: &[signex_sketch::id::SketchEntityId; 4],
    pad: &EditorPad,
    when: &str,
) {
    use signex_sketch::entity::EntityKind;

    let sketch = editor
        .primitive()
        .sketch
        .as_ref()
        .expect("the fixture minted a sketch");
    let expected = pad.rotated_corners_mm();
    for (i, id) in corner_ids.iter().enumerate() {
        let got = sketch
            .entities
            .iter()
            .find(|e| e.id == *id)
            .and_then(|e| match e.kind {
                EntityKind::Point { x, y } => Some((x, y)),
                _ => None,
            })
            .expect("corner Point still present");
        let (ex, ey) = expected[i];
        assert!(
            (got.0 - ex).abs() < 1e-9 && (got.1 - ey).abs() < 1e-9,
            "{when}: outline corner {i} sits at {got:?} but the pad's copper corner is at \
             ({ex}, {ey}). The construction outline is stranded at the PRE-transform \
             geometry while the copper renders transformed"
        );
    }
}

/// The v0.27 sketch-line-edge-drag → pad-resize propagation
/// classified the dragged line against the un-rotated `bbox_mm()`.
/// Once the outline is minted rotated, a turned pad's edges are
/// diagonal (or, at 90°, axis-aligned but with W/H swapped relative
/// to the un-rotated box) and the classification rejects every one of
/// them — the propagation silently no-ops and the user sees the line
/// move while the copper underneath does nothing.
#[test]
fn sketch_edge_drag_resizes_a_rotated_pad() {
    use signex_app::app::{FootprintEditorState, TabInfo, TabKind};
    use signex_app::library::editor::footprint::pad_to_sketch::mirror_add_pad_to_sketch;
    use signex_library::{Footprint, FootprintFile, PadShape};
    use signex_sketch::entity::EntityKind;

    let path = PathBuf::from("rotated-line-drag-resize.snxfpt");
    let mut fp = Footprint::empty("rotated-line-drag");
    let mut pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    pad.shape = PadShape::Rect;
    pad.size_mm = (2.0, 1.0);
    pad.rotation_deg = 90.0;
    mirror_add_pad_to_sketch(&mut pad, &mut fp);

    // At +90° the pad-frame top edge (y = ymin = −0.5, x ∈ [−1, 1])
    // maps to the world segment x = 0.5, y ∈ [−1, 1] — a VERTICAL
    // world line. Find it by its constant world x.
    let sketch = fp.sketch.as_ref().expect("mirror minted a sketch");
    let pos_of = |id: signex_sketch::id::SketchEntityId| -> Option<(f64, f64)> {
        sketch.entities.iter().find(|e| e.id == id).and_then(|e| {
            if let EntityKind::Point { x, y } = e.kind {
                Some((x, y))
            } else {
                None
            }
        })
    };
    let edge_id = sketch
        .entities
        .iter()
        .find_map(|e| match e.kind {
            EntityKind::Line { start, end } => {
                let (sx, _) = pos_of(start)?;
                let (ex, _) = pos_of(end)?;
                ((sx - 0.5).abs() < 1e-6 && (ex - 0.5).abs() < 1e-6).then_some(e.id)
            }
            _ => None,
        })
        .expect("the rotated Rect pad mints its top edge as the world line x = 0.5");

    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.pads = vec![pad];
    editor.state.selected_pad = Some(0);
    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: "rotated-line-drag".into(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;

    // Drag that edge 0.2 mm in −X (world). In the pad frame that is
    // +0.2 in Y on the top edge: ymin −0.5 → −0.3, so H 1.0 → 0.8 and
    // the local centre moves to y = +0.1, i.e. world (−0.1, 0).
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchMoveLine {
            id: edge_id,
            dx: -0.2,
            dy: 0.0,
        }),
    }));

    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    let after = &editor.state.pads[0];
    assert!(
        (after.size_mm.1 - 0.8).abs() < 1e-6,
        "dragging the rotated pad's top edge inward by 0.2mm must shrink H 1.0 → 0.8; got {} \
         (unchanged means the propagation rejected the edge and silently no-opped)",
        after.size_mm.1
    );
    assert!(
        (after.size_mm.0 - 2.0).abs() < 1e-6,
        "the perpendicular extent must not change; got {}",
        after.size_mm.0
    );
    assert!(
        (after.position_mm.0 + 0.1).abs() < 1e-6 && after.position_mm.1.abs() < 1e-6,
        "the new centre is the pad-frame midpoint taken back to world = (−0.1, 0); got {:?}",
        after.position_mm
    );
}
