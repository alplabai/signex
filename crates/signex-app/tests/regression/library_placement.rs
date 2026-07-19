//! Sketch-tool gestures and live numeric placement input (typed distance/angle, Tab-cycling, Escape).

use signex_app::app::{Message, Signex};

use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

// ─────────────────────────────────────────────────────────────────
// v0.24 Track C — Tangent Arc sketch sub-tool
//
// Drives the dispatcher via Signex::update(Message::Library(...))
// against a FootprintEditorState parked in document_state.footprint
// _editors so the dispatcher's existing routing keeps the test
// realistic. Tool-based gesture only — never a click-and-drag mode
// (per feedback_no_canvas_gestures.md / the user's spec for v0.24
// Track C).
// ─────────────────────────────────────────────────────────────────

#[test]
fn tangent_arc_tool_first_click_sets_pending() {
    use signex_app::app::FootprintEditorState;
    use signex_app::library::editor::footprint::state::{SketchTool, ToolPending};
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    use signex_library::{Footprint, FootprintFile};
    use signex_sketch::SketchData;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

    let path = PathBuf::from("test-tangent-arc-c1.snxfpt");
    let mut fp = Footprint::empty("test");
    // Provide a plane so the dispatcher doesn't have to mint one
    // (keeps the state setup focused on the tool gesture itself).
    fp.sketch = Some(SketchData {
        planes: vec![Plane {
            id: PlaneId::new(),
            kind: PlaneKind::BoardTop,
        }],
        ..SketchData::default()
    });
    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.active_tool = SketchTool::TangentArc;

    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);

    // First click — dispatcher mints a Point at the click position
    // (no snap target supplied) and stashes it as TangentArcFirst.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchToolClick {
            x_mm: 0.0,
            y_mm: 0.0,
            snap_id: None,
        }),
    }));

    let editor = app
        .document_state
        .footprint_editors
        .get(&path)
        .expect("footprint editor still registered");

    // tool_pending must transition to TangentArcFirst.
    assert!(
        matches!(
            editor.state.tool_pending,
            ToolPending::TangentArcFirst { .. }
        ),
        "tool_pending = {:?}, expected TangentArcFirst",
        editor.state.tool_pending
    );

    // The Point at the first click must be in the sketch — it's
    // referenced by `first` for the second click to resolve against.
    let sketch = editor
        .file
        .footprints
        .first()
        .and_then(|f| f.sketch.as_ref())
        .expect("sketch present");
    assert!(
        sketch
            .entities
            .iter()
            .any(|e| matches!(e.kind, signex_sketch::entity::EntityKind::Point { x, y } if x == 0.0 && y == 0.0)),
        "first-click Point not minted"
    );
}

#[test]
fn tangent_arc_tool_second_click_mints_arc_and_tangent_constraint() {
    use signex_app::app::FootprintEditorState;
    use signex_app::library::editor::footprint::state::{SketchTool, ToolPending};
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    use signex_library::{Footprint, FootprintFile};
    use signex_sketch::SketchData;
    use signex_sketch::constraint::ConstraintKind;
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::id::SketchEntityId;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

    let path = PathBuf::from("test-tangent-arc-c2.snxfpt");
    let mut fp = Footprint::empty("test");
    let plane_id = PlaneId::new();

    // Pre-seed: Point A at (0, 0), Point B at (5, 0), Line A→B.
    // The Line ends at B, so a TangentArc click at B should find
    // this Line and emit a TangentLineArc constraint linking it to
    // the new Arc.
    let a_id = SketchEntityId::new();
    let b_id = SketchEntityId::new();
    let line_id = SketchEntityId::new();
    let pt_a = Entity::new(a_id, plane_id, EntityKind::Point { x: 0.0, y: 0.0 });
    let pt_b = Entity::new(b_id, plane_id, EntityKind::Point { x: 5.0, y: 0.0 });
    let line = Entity::new(
        line_id,
        plane_id,
        EntityKind::Line {
            start: a_id,
            end: b_id,
        },
    );

    fp.sketch = Some(SketchData {
        planes: vec![Plane {
            id: plane_id,
            kind: PlaneKind::BoardTop,
        }],
        entities: vec![pt_a, pt_b, line],
        ..SketchData::default()
    });

    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.active_tool = SketchTool::TangentArc;
    // Click 1 already happened — pre-seed the pending state with
    // first = B (the Line's end). The next click is click 2.
    editor.state.tool_pending = ToolPending::TangentArcFirst { first: b_id };

    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);

    // Snapshot pre-click counts so we can assert deltas without
    // relying on absolute totals (the FROM_FOOTPRINT path may
    // implicitly reorder/auto-mint pad-backed Points in future).
    let (entities_before, constraints_before) = {
        let editor = app.document_state.footprint_editors.get(&path).unwrap();
        let sketch = editor.file.footprints[0].sketch.as_ref().unwrap();
        (sketch.entities.len(), sketch.constraints.len())
    };

    // Click 2 — pick a point off the line so the tangent circle has
    // a non-degenerate radius. (3, 4) is 5 mm from B and 1.41 mm off
    // the line, well above the perpendicular-cursor degeneracy
    // threshold the dispatcher uses.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchToolClick {
            x_mm: 3.0,
            y_mm: 4.0,
            snap_id: None,
        }),
    }));

    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    let sketch = editor.file.footprints[0].sketch.as_ref().unwrap();

    // Tool reset to Idle after commit.
    assert!(
        matches!(editor.state.tool_pending, ToolPending::Idle),
        "tool_pending = {:?}, expected Idle after click 2",
        editor.state.tool_pending
    );

    // The dispatcher mints two new entities on click 2: the second
    // endpoint Point (at the click) and the centre Point of the
    // tangent circle, plus the Arc itself — three new entities.
    // We assert the Arc is present + at least one new entity, since
    // the centre minting is the dispatcher's choice.
    assert!(
        sketch.entities.len() >= entities_before + 2,
        "expected at least the second endpoint + centre + arc to be minted (entities: {} → {})",
        entities_before,
        sketch.entities.len()
    );
    let arc_count = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Arc { .. }))
        .count();
    assert_eq!(arc_count, 1, "expected exactly one Arc entity to be minted");

    // The Arc's start endpoint must be the pre-stashed `first`
    // (b_id), proving the click chained off the previous Line.
    let arc = sketch
        .entities
        .iter()
        .find(|e| matches!(e.kind, EntityKind::Arc { .. }))
        .unwrap();
    let (arc_start, arc_id) = match arc.kind {
        EntityKind::Arc { start, .. } => (start, arc.id),
        _ => unreachable!(),
    };
    assert_eq!(arc_start, b_id, "Arc.start must be the first-click Point");

    // The TangentLineArc constraint must reference the pre-existing
    // Line + the freshly minted Arc.
    assert!(
        sketch.constraints.len() > constraints_before,
        "expected a new constraint to be added"
    );
    assert!(
        sketch.constraints.iter().any(|c| matches!(
            c.kind,
            ConstraintKind::TangentLineArc { line, arc } if line == line_id && arc == arc_id
        )),
        "expected a TangentLineArc {{ line, arc }} constraint linking the seed Line to the new Arc"
    );
}

// ─────────────────────────────────────────────────────────────────
// v0.24 Track D — live numeric placement input
// ─────────────────────────────────────────────────────────────────

/// v0.24 Track D — Line tool's second click honours the typed
/// `placement_input` length. With a buffer of "10" set against the
/// `LineLength` kind, a click that lands at `(20, 0)` must place the
/// line's second endpoint at exactly `(10, 0)` along the cursor's
/// azimuth from the first endpoint at the origin — not `(20, 0)`.
///
/// Drives the dispatcher via `Message::Library(PrimitiveEditorEvent
/// { ... })` so the integration matches what the canvas + bootstrap
/// keyboard handler emit.
#[test]
fn placement_input_line_length_pins_second_click_at_exact_distance() {
    use signex_app::app::FootprintEditorState;
    use signex_app::library::editor::footprint::state::{
        EditorMode, PlacementInput, PlacementInputKind, SketchTool,
    };
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    use signex_library::{Footprint, FootprintFile};
    use signex_sketch::entity::EntityKind;

    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("track-d.snxfpt");
    fs::write(&path, b"{}").expect("write .snxfpt placeholder");

    // Empty `Signex` + a fresh footprint editor state pre-populated in
    // `document_state.footprint_editors` so the dispatcher's
    // path-keyed lookup resolves.
    let (mut app, _initial_task) = Signex::new();
    let fp = Footprint::empty("track-d-fixture");
    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.mode = EditorMode::Sketch;
    editor.state.active_tool = SketchTool::Line;
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);

    // First click → first endpoint at (0, 0). The dispatcher mints
    // a Point entity and sets `tool_pending = LineFirst { first }`.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchToolClick {
            x_mm: 0.0,
            y_mm: 0.0,
            snap_id: None,
        }),
    }));

    // Pin the placement input: user types "10" while the gesture is
    // mid-flight. With `LineLength` kind, the next click commits the
    // line at exactly 10 mm from the first endpoint.
    {
        let editor = app
            .document_state
            .footprint_editors
            .get_mut(&path)
            .expect("editor present after first click");
        editor.state.placement_input = Some(PlacementInput {
            buffer: "10".into(),
            kind: PlacementInputKind::LineLength,
        });
    }

    // Second click — cursor at (20, 0). Without placement_input the
    // line's end would land at (20, 0); with the buffer pinned, it
    // must land at (10, 0).
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchToolClick {
            x_mm: 20.0,
            y_mm: 0.0,
            snap_id: None,
        }),
    }));

    let editor = app
        .document_state
        .footprint_editors
        .get(&path)
        .expect("editor present after second click");
    let sketch = editor
        .primitive()
        .sketch
        .as_ref()
        .expect("sketch present after the click pair");

    // Find the Line entity + resolve its `end` Point's coords.
    let line = sketch
        .entities
        .iter()
        .find(|e| matches!(e.kind, EntityKind::Line { .. }))
        .expect("line entity emitted by the second click");
    let (start_id, end_id) = match line.kind {
        EntityKind::Line { start, end } => (start, end),
        _ => unreachable!(),
    };
    let start_pt = sketch
        .entities
        .iter()
        .find(|e| e.id == start_id)
        .and_then(|e| match e.kind {
            EntityKind::Point { x, y } => Some((x, y)),
            _ => None,
        })
        .expect("line start endpoint resolves to a Point");
    let end_pt = sketch
        .entities
        .iter()
        .find(|e| e.id == end_id)
        .and_then(|e| match e.kind {
            EntityKind::Point { x, y } => Some((x, y)),
            _ => None,
        })
        .expect("line end endpoint resolves to a Point");

    assert!(
        (start_pt.0 - 0.0).abs() < 1e-9 && (start_pt.1 - 0.0).abs() < 1e-9,
        "first endpoint should remain at the origin; got {:?}",
        start_pt
    );
    assert!(
        (end_pt.0 - 10.0).abs() < 1e-9,
        "second endpoint x should be 10 mm (typed length), not the cursor's 20 mm; got {}",
        end_pt.0
    );
    assert!(
        (end_pt.1 - 0.0).abs() < 1e-9,
        "second endpoint y should be 0 (cursor azimuth); got {}",
        end_pt.1
    );
}

/// v0.24 Track D — `state.placement_input` clears to `None` once the
/// click that consumed it commits. The user has to type again before
/// the next gesture step to keep the chain explicit.
#[test]
fn placement_input_clears_after_commit() {
    use signex_app::app::FootprintEditorState;
    use signex_app::library::editor::footprint::state::{
        EditorMode, PlacementInput, PlacementInputKind, SketchTool,
    };
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    use signex_library::{Footprint, FootprintFile};

    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("track-d-clear.snxfpt");
    fs::write(&path, b"{}").expect("write .snxfpt placeholder");

    let (mut app, _initial_task) = Signex::new();
    let fp = Footprint::empty("track-d-fixture");
    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.mode = EditorMode::Sketch;
    editor.state.active_tool = SketchTool::Line;
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);

    // First click — drops the first endpoint.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchToolClick {
            x_mm: 0.0,
            y_mm: 0.0,
            snap_id: None,
        }),
    }));

    // Type "10" before the second click.
    {
        let editor = app
            .document_state
            .footprint_editors
            .get_mut(&path)
            .expect("editor present after first click");
        editor.state.placement_input = Some(PlacementInput {
            buffer: "10".into(),
            kind: PlacementInputKind::LineLength,
        });
    }

    // Second click — commits, must consume + clear the buffer.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchToolClick {
            x_mm: 20.0,
            y_mm: 0.0,
            snap_id: None,
        }),
    }));

    let editor = app
        .document_state
        .footprint_editors
        .get(&path)
        .expect("editor present after second click");
    assert!(
        editor.state.placement_input.is_none(),
        "placement_input must clear after the click that consumed it; \
         leaked buffer = {:?}",
        editor.state.placement_input.as_ref().map(|p| &p.buffer)
    );
}

/// v0.24 Track D — typed character path. The user types '5' then
/// '.', then '2' against an active Line tool with first click
/// landed; the dispatcher's char-append handler must validate
/// (single decimal point) and grow `buffer = "5.2"` keyed off
/// `LineLength`.
#[test]
fn placement_input_char_append_validates_decimal_point() {
    use signex_app::app::FootprintEditorState;
    use signex_app::library::editor::footprint::state::{
        EditorMode, PlacementInputKind, SketchTool,
    };
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    use signex_library::{Footprint, FootprintFile};

    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("track-d-buffer.snxfpt");
    fs::write(&path, b"{}").expect("write .snxfpt placeholder");

    let (mut app, _initial_task) = Signex::new();
    let fp = Footprint::empty("track-d-fixture");
    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.mode = EditorMode::Sketch;
    editor.state.active_tool = SketchTool::Line;
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);

    // First click — anchors the gesture so the dispatcher accepts
    // numeric input on subsequent keypresses.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchToolClick {
            x_mm: 0.0,
            y_mm: 0.0,
            snap_id: None,
        }),
    }));

    for ch in ['5', '.', '2'] {
        let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchPlacementInputChar(ch)),
        }));
    }
    // Second decimal — must be rejected.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchPlacementInputChar('.')),
    }));

    let editor = app
        .document_state
        .footprint_editors
        .get(&path)
        .expect("editor present after keypress sequence");
    let input = editor
        .state
        .placement_input
        .as_ref()
        .expect("buffer minted by the first digit press");
    assert_eq!(input.buffer, "5.2");
    assert_eq!(input.kind, PlacementInputKind::LineLength);
}

/// v0.24 Track D — Escape clears the buffer immediately; subsequent
/// click commits at the cursor with no override, as if no buffer
/// had been typed.
#[test]
fn placement_input_escape_clears_buffer() {
    use signex_app::app::FootprintEditorState;
    use signex_app::library::editor::footprint::state::{
        EditorMode, PlacementInput, PlacementInputKind, SketchTool,
    };
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    use signex_library::{Footprint, FootprintFile};

    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("track-d-escape.snxfpt");
    fs::write(&path, b"{}").expect("write .snxfpt placeholder");

    let (mut app, _initial_task) = Signex::new();
    let fp = Footprint::empty("track-d-fixture");
    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.mode = EditorMode::Sketch;
    editor.state.active_tool = SketchTool::Line;
    editor.state.placement_input = Some(PlacementInput {
        buffer: "42".into(),
        kind: PlacementInputKind::LineLength,
    });
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);

    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchPlacementInputEscape),
    }));

    let editor = app
        .document_state
        .footprint_editors
        .get(&path)
        .expect("editor present after Esc");
    assert!(
        editor.state.placement_input.is_none(),
        "Esc must clear placement_input; leaked = {:?}",
        editor.state.placement_input.as_ref().map(|p| &p.buffer)
    );
}

// ─────────────────────────────────────────────────────────────────
// v0.25 polish #2 — Offset placement-input
//
// Typing a digit while the Offset tool is active sets a buffer
// (PlacementInputKind::OffsetDistance) that overrides the cursor
// distance on the commit click. The buffer clears on commit so the
// next Offset gesture starts fresh.
// ─────────────────────────────────────────────────────────────────

#[test]
fn v025_offset_placement_input_pins_typed_distance_over_cursor() {
    use signex_app::app::FootprintEditorState;
    use signex_app::library::editor::footprint::state::{
        EditorMode, PlacementInput, PlacementInputKind, SketchTool,
    };
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    use signex_library::{Footprint, FootprintFile};
    use signex_sketch::SketchData;
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::id::SketchEntityId;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
    let path = PathBuf::from("v025-offset-placement.snxfpt");
    let plane_id = PlaneId::new();
    // Pre-seed: source Line from (0, 0) to (10, 0). Offset tool will
    // emit a parallel Line at the typed perpendicular distance.
    let start_id = SketchEntityId::new();
    let end_id = SketchEntityId::new();
    let line_id = SketchEntityId::new();
    let pt_start = Entity::new(start_id, plane_id, EntityKind::Point { x: 0.0, y: 0.0 });
    let pt_end = Entity::new(end_id, plane_id, EntityKind::Point { x: 10.0, y: 0.0 });
    let line = Entity::new(
        line_id,
        plane_id,
        EntityKind::Line {
            start: start_id,
            end: end_id,
        },
    );
    let mut fp = Footprint::empty("v025-offset");
    fp.sketch = Some(SketchData {
        planes: vec![Plane {
            id: plane_id,
            kind: PlaneKind::BoardTop,
        }],
        entities: vec![pt_start, pt_end, line],
        ..SketchData::default()
    });
    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.mode = EditorMode::Sketch;
    editor.state.active_tool = SketchTool::Offset;
    // Offset tool needs the source curve in `selected_sketch`.
    editor.state.selected_sketch = Some(line_id);
    editor.state.placement_input = Some(PlacementInput {
        buffer: "2".into(),
        kind: PlacementInputKind::OffsetDistance,
    });
    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    // Click on the +y side of the source Line at a wildly large y so
    // the cursor distance (100mm) clearly differs from the typed one
    // (2mm). The Offset dispatcher picks the side from the cross
    // product, then uses the typed distance for the perpendicular
    // offset magnitude.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchToolClick {
            x_mm: 5.0,
            y_mm: 100.0,
            snap_id: None,
        }),
    }));
    let editor = app
        .document_state
        .footprint_editors
        .get(&path)
        .expect("editor still registered after offset click");
    let sketch = editor.primitive().sketch.as_ref().expect("sketch present");
    // Find the new Line — there are now two Line entities; the offset
    // Line is the one whose endpoints differ from (start_id, end_id).
    let new_line = sketch
        .entities
        .iter()
        .find(|e| matches!(e.kind, EntityKind::Line { start, end } if start != start_id && end != end_id))
        .expect("offset Line emitted");
    let (new_start_id, _new_end_id) = match new_line.kind {
        EntityKind::Line { start, end } => (start, end),
        _ => unreachable!(),
    };
    let new_start = sketch
        .entities
        .iter()
        .find(|e| e.id == new_start_id)
        .and_then(|e| match e.kind {
            EntityKind::Point { x, y } => Some((x, y)),
            _ => None,
        })
        .expect("offset Line start resolves to a Point");
    // Source Line is on y=0, click at (5, 100) is on the +y side, so
    // perpendicular sign is +1 → new_start.y = 0 + 2 = 2.0. The cursor
    // y of 100 must NOT have leaked through.
    assert!(
        (new_start.0 - 0.0).abs() < 1e-9,
        "offset Line start.x stays at 0 (parallel to source); got {}",
        new_start.0
    );
    assert!(
        (new_start.1 - 2.0).abs() < 1e-9,
        "offset distance = typed 2mm, NOT cursor's 100mm; got start.y = {}",
        new_start.1
    );
    // Buffer must clear so the next Offset click doesn't reuse 2mm.
    assert!(
        editor.state.placement_input.is_none(),
        "placement_input must clear after the commit click consumes it"
    );
}

/// v0.27 — dragging a pad-outline edge in Sketch mode must propagate
/// through to `pad.size_mm` / `pad.position_mm`. Pre-fix: the sketch
/// outline visibly resized but the literal pad bbox never updated,
/// so the rendered pad copper underneath the moving line stayed put.
#[test]
fn v027_sketch_line_drag_resizes_rect_pad_bbox() {
    use signex_app::app::{FootprintEditorState, TabInfo, TabKind};
    use signex_app::library::editor::footprint::pad_to_sketch::mirror_add_pad_to_sketch;
    use signex_app::library::editor::footprint::state::EditorPad;
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    use signex_library::{Footprint, FootprintFile, PadShape};
    use signex_sketch::entity::EntityKind;
    let path = PathBuf::from("v027-sketch-line-drag-resize.snxfpt");
    let mut fp = Footprint::empty("v027-line-drag");
    let mut pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    pad.shape = PadShape::Rect;
    pad.size_mm = (2.0, 1.0); // bbox: (-1, -0.5, 1, 0.5)
    mirror_add_pad_to_sketch(&mut pad, &mut fp);
    // Locate the top construction line. Rect mints 4 corner Points
    // + 4 connecting lines; the top one runs along y = ymin.
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
    let top_line_id = sketch
        .entities
        .iter()
        .find_map(|e| match e.kind {
            EntityKind::Line { start, end } => {
                let (_, sy) = pos_of(start)?;
                let (_, ey) = pos_of(end)?;
                if (sy + 0.5).abs() < 1e-6 && (ey + 0.5).abs() < 1e-6 {
                    Some(e.id)
                } else {
                    None
                }
            }
            _ => None,
        })
        .expect("Rect pad mints a top construction line at y=ymin");
    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.pads = vec![pad];
    editor.state.selected_pad = Some(0);
    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: "v027-line-drag".into(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;
    // Drag the top edge DOWN by 0.2 mm. New bbox y range:
    // [-0.5 + 0.2, 0.5] = [-0.3, 0.5]. Height 1.0 → 0.8, centre y
    // 0.0 → +0.1. Width and centre x must stay unchanged.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchMoveLine {
            id: top_line_id,
            dx: 0.0,
            dy: 0.2,
        }),
    }));
    let editor = app
        .document_state
        .footprint_editors
        .get(&path)
        .expect("editor still registered");
    let pad_after = &editor.state.pads[0];
    assert!(
        (pad_after.size_mm.1 - 0.8).abs() < 1e-6,
        "top-edge drag down by 0.2mm must shrink height 1.0 → 0.8; got {}",
        pad_after.size_mm.1
    );
    assert!(
        (pad_after.size_mm.0 - 2.0).abs() < 1e-6,
        "horizontal-line drag must not affect width; got {}",
        pad_after.size_mm.0
    );
    assert!(
        (pad_after.position_mm.1 - 0.1).abs() < 1e-6,
        "centre y must shift to (-0.3 + 0.5)/2 = 0.1; got {}",
        pad_after.position_mm.1
    );
    assert!(
        (pad_after.position_mm.0 - 0.0).abs() < 1e-6,
        "horizontal-line drag must not move centre x; got {}",
        pad_after.position_mm.0
    );
}

// ─────────────────────────────────────────────────────────────────
// v0.14-footprint — #23 TAB-pause click suppression + #24 Line
// length/angle dimension entry. (#25 is render-only: the dimension
// pills in draw_sketch.rs read exactly the `placement_input` /
// `placement_input_other` state these tests assert, so verifying the
// state plumbing covers the data the pills display.)
// ─────────────────────────────────────────────────────────────────

/// v0.14-footprint #23 — while `placement_paused` is set, a sketch
/// commit click must be dropped before it can advance `tool_pending`
/// or mint geometry. Reproduces the "TAB shows paused but the Rounded
/// Rectangle still commits" bug: the canvas-layer gate leaked the
/// commit click, so the authoritative gate now lives at the top of
/// the sketch-click dispatcher arm.
#[test]
fn placement_paused_suppresses_rounded_rect_commit_click() {
    use signex_app::app::FootprintEditorState;
    use signex_app::library::editor::footprint::state::{EditorMode, SketchTool, ToolPending};
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    use signex_library::{Footprint, FootprintFile};
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("pause-rrect.snxfpt");
    fs::write(&path, b"{}").expect("write .snxfpt placeholder");
    let (mut app, _initial_task) = Signex::new();
    let fp = Footprint::empty("pause-rrect-fixture");
    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.mode = EditorMode::Sketch;
    editor.state.active_tool = SketchTool::RoundedRectangle;
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    // First click (NOT paused) → anchors the first corner.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchToolClick {
            x_mm: 0.0,
            y_mm: 0.0,
            snap_id: None,
        }),
    }));
    // Snapshot the post-first-click state: pending = RoundedRectangleFirst
    // with exactly the one anchor Point minted.
    let (count_before, pending_was_first) = {
        let editor = app
            .document_state
            .footprint_editors
            .get(&path)
            .expect("editor present after first click");
        let n = editor
            .primitive()
            .sketch
            .as_ref()
            .map(|s| s.entities.len())
            .unwrap_or(0);
        (
            n,
            matches!(
                editor.state.tool_pending,
                ToolPending::RoundedRectangleFirst { .. }
            ),
        )
    };
    assert!(
        pending_was_first,
        "sanity: first click must arm RoundedRectangleFirst"
    );
    assert_eq!(
        count_before, 1,
        "sanity: first click mints exactly one anchor Point"
    );
    // TAB-pause, then the commit click. With the fix the click is
    // dropped: no opposite corner, no rounded-rect geometry, gesture
    // stays armed at RoundedRectangleFirst.
    {
        let editor = app
            .document_state
            .footprint_editors
            .get_mut(&path)
            .expect("editor present");
        editor.state.placement_paused = true;
    }
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchToolClick {
            x_mm: 12.0,
            y_mm: 8.0,
            snap_id: None,
        }),
    }));
    let editor = app
        .document_state
        .footprint_editors
        .get(&path)
        .expect("editor present after paused click");
    let count_after = editor
        .primitive()
        .sketch
        .as_ref()
        .map(|s| s.entities.len())
        .unwrap_or(0);
    assert_eq!(
        count_after, count_before,
        "paused commit click must mint no geometry; entity count changed {count_before} -> {count_after}"
    );
    assert!(
        matches!(
            editor.state.tool_pending,
            ToolPending::RoundedRectangleFirst { .. }
        ),
        "paused commit click must NOT advance tool_pending; got {:?}",
        editor.state.tool_pending
    );
}

/// v0.14-footprint #24 — Tab toggles the focused Line dimension field
/// between length and angle, stashing the inactive field in
/// `placement_input_other`. Each field keeps its own typed digits
/// across the round-trip (length "10" survives length→angle→length).
#[test]
fn placement_input_tab_swaps_line_length_and_angle() {
    use signex_app::app::FootprintEditorState;
    use signex_app::library::editor::footprint::state::{
        EditorMode, PlacementInput, PlacementInputKind, SketchTool, ToolPending,
    };
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    use signex_library::{Footprint, FootprintFile};
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("line-tab.snxfpt");
    fs::write(&path, b"{}").expect("write .snxfpt placeholder");
    let (mut app, _initial_task) = Signex::new();
    let fp = Footprint::empty("line-tab-fixture");
    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.mode = EditorMode::Sketch;
    editor.state.active_tool = SketchTool::Line;
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    // Anchor the first endpoint → tool_pending = LineFirst.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchToolClick {
            x_mm: 0.0,
            y_mm: 0.0,
            snap_id: None,
        }),
    }));
    // Focus = length "10", nothing stashed yet.
    {
        let editor = app.document_state.footprint_editors.get_mut(&path).unwrap();
        assert!(
            matches!(editor.state.tool_pending, ToolPending::LineFirst { .. }),
            "sanity: first click arms LineFirst"
        );
        editor.state.placement_input = Some(PlacementInput {
            buffer: "10".into(),
            kind: PlacementInputKind::LineLength,
        });
        editor.state.placement_input_others.clear();
    }
    // First Tab — focus moves to a fresh empty angle field; length
    // "10" is stashed.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchPlacementInputTab),
    }));
    {
        let editor = app.document_state.footprint_editors.get(&path).unwrap();
        let focused = editor
            .state
            .placement_input
            .as_ref()
            .expect("focused field present after Tab");
        assert_eq!(
            focused.kind,
            PlacementInputKind::LineAngle,
            "Tab focuses the angle field"
        );
        assert_eq!(focused.buffer, "", "fresh angle field starts empty");
        assert_eq!(
            editor.state.placement_input_others.len(),
            1,
            "exactly one field parked while editing the other"
        );
        let stashed = &editor.state.placement_input_others[0];
        assert_eq!(stashed.kind, PlacementInputKind::LineLength);
        assert_eq!(stashed.buffer, "10", "stashed length keeps its digits");
    }
    // Type "90" into the angle field, then Tab back to length.
    for ch in ['9', '0'] {
        let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchPlacementInputChar(ch)),
        }));
    }
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchPlacementInputTab),
    }));
    {
        let editor = app.document_state.footprint_editors.get(&path).unwrap();
        let focused = editor
            .state
            .placement_input
            .as_ref()
            .expect("focused field present after second Tab");
        assert_eq!(
            focused.kind,
            PlacementInputKind::LineLength,
            "second Tab restores length focus"
        );
        assert_eq!(
            focused.buffer, "10",
            "length digits survived the length→angle→length round-trip"
        );
        assert_eq!(
            editor.state.placement_input_others.len(),
            1,
            "exactly one field parked after the round-trip"
        );
        let stashed = &editor.state.placement_input_others[0];
        assert_eq!(stashed.kind, PlacementInputKind::LineAngle);
        assert_eq!(
            stashed.buffer, "90",
            "angle digits typed while focused are retained"
        );
    }
}

/// v0.14-footprint #24 — with BOTH a typed length and a typed angle
/// pinned (in either placement slot), the Line second click commits
/// the endpoint at `first + (len @ angle°)`, ignoring the cursor's own
/// azimuth and distance. 10 mm @ 90° from the origin lands the
/// endpoint at (0, 10) even though the cursor sits at (20, 0).
#[test]
fn placement_input_line_length_and_angle_commit_at_polar_offset() {
    use signex_app::app::FootprintEditorState;
    use signex_app::library::editor::footprint::state::{
        EditorMode, PlacementInput, PlacementInputKind, SketchTool,
    };
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    use signex_library::{Footprint, FootprintFile};
    use signex_sketch::entity::EntityKind;
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("line-polar.snxfpt");
    fs::write(&path, b"{}").expect("write .snxfpt placeholder");
    let (mut app, _initial_task) = Signex::new();
    let fp = Footprint::empty("line-polar-fixture");
    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.mode = EditorMode::Sketch;
    editor.state.active_tool = SketchTool::Line;
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    // First click → first endpoint at the origin.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchToolClick {
            x_mm: 0.0,
            y_mm: 0.0,
            snap_id: None,
        }),
    }));
    // Pin length = 10 (focused) and angle = 90 (stashed). The commit
    // reads each field from whichever slot holds it.
    {
        let editor = app.document_state.footprint_editors.get_mut(&path).unwrap();
        editor.state.placement_input = Some(PlacementInput {
            buffer: "10".into(),
            kind: PlacementInputKind::LineLength,
        });
        editor.state.placement_input_others = vec![PlacementInput {
            buffer: "90".into(),
            kind: PlacementInputKind::LineAngle,
        }];
    }
    // Second click — cursor at (20, 0): azimuth 0°, distance 20. Both
    // are overridden by the typed 10 mm @ 90°.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchToolClick {
            x_mm: 20.0,
            y_mm: 0.0,
            snap_id: None,
        }),
    }));
    let editor = app
        .document_state
        .footprint_editors
        .get(&path)
        .expect("editor present after second click");
    let sketch = editor
        .primitive()
        .sketch
        .as_ref()
        .expect("sketch present after the click pair");
    let line = sketch
        .entities
        .iter()
        .find(|e| matches!(e.kind, EntityKind::Line { .. }))
        .expect("line entity emitted by the second click");
    let end_id = match line.kind {
        EntityKind::Line { end, .. } => end,
        _ => unreachable!(),
    };
    let end_pt = sketch
        .entities
        .iter()
        .find(|e| e.id == end_id)
        .and_then(|e| match e.kind {
            EntityKind::Point { x, y } => Some((x, y)),
            _ => None,
        })
        .expect("line end endpoint resolves to a Point");
    assert!(
        end_pt.0.abs() < 1e-9,
        "endpoint x should be 0 (10 mm @ 90°), not the cursor's 20; got {}",
        end_pt.0
    );
    assert!(
        (end_pt.1 - 10.0).abs() < 1e-9,
        "endpoint y should be 10 mm (10 @ 90°); got {}",
        end_pt.1
    );
}

/// v0.14-footprint re-verify — Circle still honours a typed radius:
/// click the centre, type "4", then a click at the cursor's 10 mm
/// must commit a circle of radius 4 (the typed value), not 10.
#[test]
fn placement_input_circle_radius_pins_typed_radius() {
    use signex_app::app::FootprintEditorState;
    use signex_app::library::editor::footprint::state::{
        EditorMode, PlacementInput, PlacementInputKind, SketchTool,
    };
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    use signex_library::{Footprint, FootprintFile};
    use signex_sketch::entity::EntityKind;
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("circle-r.snxfpt");
    fs::write(&path, b"{}").expect("write .snxfpt placeholder");
    let (mut app, _initial_task) = Signex::new();
    let fp = Footprint::empty("circle-r-fixture");
    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.mode = EditorMode::Sketch;
    editor.state.active_tool = SketchTool::Circle;
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    // Click 1 → centre at the origin.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchToolClick {
            x_mm: 0.0,
            y_mm: 0.0,
            snap_id: None,
        }),
    }));
    // Pin radius = 4.
    {
        let editor = app.document_state.footprint_editors.get_mut(&path).unwrap();
        editor.state.placement_input = Some(PlacementInput {
            buffer: "4".into(),
            kind: PlacementInputKind::CircleRadius,
        });
    }
    // Click 2 — cursor at (10, 0); radius pinned to 4.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchToolClick {
            x_mm: 10.0,
            y_mm: 0.0,
            snap_id: None,
        }),
    }));
    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    let sketch = editor.primitive().sketch.as_ref().expect("sketch present");
    let circle = sketch
        .entities
        .iter()
        .find(|e| matches!(e.kind, EntityKind::Circle { .. }))
        .expect("circle minted by the second click");
    if let EntityKind::Circle { radius, .. } = circle.kind {
        assert!(
            (radius - 4.0).abs() < 1e-9,
            "circle radius should be the typed 4 mm, not the cursor's 10; got {radius}"
        );
    } else {
        unreachable!()
    }
}

/// v0.14-footprint — Rectangle accepts typed width/height during
/// placement. With width 6 and height 4 pinned (one in each slot), the
/// second click commits a 6×4 box anchored at the first corner,
/// ignoring the cursor's 10×10 position.
#[test]
fn placement_input_rectangle_commits_typed_width_height() {
    use signex_app::app::FootprintEditorState;
    use signex_app::library::editor::footprint::state::{
        EditorMode, PlacementInput, PlacementInputKind, SketchTool,
    };
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    use signex_library::{Footprint, FootprintFile};
    use signex_sketch::entity::EntityKind;
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("rect-wh.snxfpt");
    fs::write(&path, b"{}").expect("write .snxfpt placeholder");
    let (mut app, _initial_task) = Signex::new();
    let fp = Footprint::empty("rect-wh-fixture");
    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.mode = EditorMode::Sketch;
    editor.state.active_tool = SketchTool::Rectangle;
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    // Click 1 → first corner at the origin.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchToolClick {
            x_mm: 0.0,
            y_mm: 0.0,
            snap_id: None,
        }),
    }));
    // Pin width = 6 (focused) and height = 4 (parked).
    {
        let editor = app.document_state.footprint_editors.get_mut(&path).unwrap();
        editor.state.placement_input = Some(PlacementInput {
            buffer: "6".into(),
            kind: PlacementInputKind::RectWidth,
        });
        editor.state.placement_input_others = vec![PlacementInput {
            buffer: "4".into(),
            kind: PlacementInputKind::RectHeight,
        }];
    }
    // Click 2 — cursor at (10, 10): quadrant +x/+y, so the box grows
    // to (6, 4), not (10, 10).
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchToolClick {
            x_mm: 10.0,
            y_mm: 10.0,
            snap_id: None,
        }),
    }));
    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    let sketch = editor.primitive().sketch.as_ref().expect("sketch present");
    let pts: Vec<(f64, f64)> = sketch
        .entities
        .iter()
        .filter_map(|e| match e.kind {
            EntityKind::Point { x, y } => Some((x, y)),
            _ => None,
        })
        .collect();
    assert!(!pts.is_empty(), "rectangle minted corner points");
    let min_x = pts.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
    let max_x = pts.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
    let min_y = pts.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let max_y = pts.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
    assert!(
        (min_x - 0.0).abs() < 1e-9 && (max_x - 6.0).abs() < 1e-9,
        "box width should be the typed 6 mm; x span = [{min_x}, {max_x}]"
    );
    assert!(
        (min_y - 0.0).abs() < 1e-9 && (max_y - 4.0).abs() < 1e-9,
        "box height should be the typed 4 mm; y span = [{min_y}, {max_y}]"
    );
}

/// v0.14-footprint — Tab cycles the Rounded-Rectangle's THREE dimension
/// fields (width → height → radius → width…), parking the inactive
/// ones in `placement_input_others` and preserving each field's digits
/// across a full round-trip.
#[test]
fn placement_input_tab_cycles_rounded_rect_three_fields() {
    use signex_app::app::FootprintEditorState;
    use signex_app::library::editor::footprint::state::{
        EditorMode, PlacementInput, PlacementInputKind, SketchTool, ToolPending,
    };
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    use signex_library::{Footprint, FootprintFile};
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("rrect-tab.snxfpt");
    fs::write(&path, b"{}").expect("write .snxfpt placeholder");
    let (mut app, _initial_task) = Signex::new();
    let fp = Footprint::empty("rrect-tab-fixture");
    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.mode = EditorMode::Sketch;
    editor.state.active_tool = SketchTool::RoundedRectangle;
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    // Anchor the first corner → tool_pending = RoundedRectangleFirst.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchToolClick {
            x_mm: 0.0,
            y_mm: 0.0,
            snap_id: None,
        }),
    }));
    {
        let editor = app.document_state.footprint_editors.get_mut(&path).unwrap();
        assert!(
            matches!(
                editor.state.tool_pending,
                ToolPending::RoundedRectangleFirst { .. }
            ),
            "sanity: first click arms RoundedRectangleFirst"
        );
        // Focus = width "5", nothing parked yet (as if the user typed
        // a width before any Tab).
        editor.state.placement_input = Some(PlacementInput {
            buffer: "5".into(),
            kind: PlacementInputKind::RectWidth,
        });
        editor.state.placement_input_others.clear();
    }
    let tab = |app: &mut Signex| {
        let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchPlacementInputTab),
        }));
    };
    let focused_kind = |app: &Signex| {
        app.document_state.footprint_editors[&path]
            .state
            .placement_input
            .as_ref()
            .map(|p| p.kind)
    };
    tab(&mut app);
    assert_eq!(
        focused_kind(&app),
        Some(PlacementInputKind::RectHeight),
        "first Tab → height"
    );
    tab(&mut app);
    assert_eq!(
        focused_kind(&app),
        Some(PlacementInputKind::RRectRadius),
        "second Tab → radius"
    );
    tab(&mut app);
    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    let focused = editor.state.placement_input.as_ref().unwrap();
    assert_eq!(
        focused.kind,
        PlacementInputKind::RectWidth,
        "third Tab wraps back to width"
    );
    assert_eq!(
        focused.buffer, "5",
        "width digits survived the w→h→r→w round-trip"
    );
    assert_eq!(
        editor.state.placement_input_others.len(),
        2,
        "the other two fields stay parked"
    );
}

/// v0.14-footprint — Rounded-Rectangle commit honours typed width,
/// height AND corner radius. Width 8 / height 5 / radius 1.5 pinned
/// across the slots; the committed box spans 8×5 and each corner arc
/// has radius 1.5, regardless of the cursor's position.
#[test]
fn placement_input_rounded_rect_commits_typed_size_and_radius() {
    use signex_app::app::FootprintEditorState;
    use signex_app::library::editor::footprint::state::{
        EditorMode, PlacementInput, PlacementInputKind, SketchTool,
    };
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    use signex_library::{Footprint, FootprintFile};
    use signex_sketch::entity::EntityKind;
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("rrect-commit.snxfpt");
    fs::write(&path, b"{}").expect("write .snxfpt placeholder");
    let (mut app, _initial_task) = Signex::new();
    let fp = Footprint::empty("rrect-commit-fixture");
    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.mode = EditorMode::Sketch;
    editor.state.active_tool = SketchTool::RoundedRectangle;
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    // Click 1 → first corner at the origin.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchToolClick {
            x_mm: 0.0,
            y_mm: 0.0,
            snap_id: None,
        }),
    }));
    // Pin width=8 (focused), height=5 + radius=1.5 (parked).
    {
        let editor = app.document_state.footprint_editors.get_mut(&path).unwrap();
        editor.state.placement_input = Some(PlacementInput {
            buffer: "8".into(),
            kind: PlacementInputKind::RectWidth,
        });
        editor.state.placement_input_others = vec![
            PlacementInput {
                buffer: "5".into(),
                kind: PlacementInputKind::RectHeight,
            },
            PlacementInput {
                buffer: "1.5".into(),
                kind: PlacementInputKind::RRectRadius,
            },
        ];
    }
    // Click 2 — cursor far away at (20, 20); typed dims win.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchToolClick {
            x_mm: 20.0,
            y_mm: 20.0,
            snap_id: None,
        }),
    }));
    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    let sketch = editor.primitive().sketch.as_ref().expect("sketch present");
    let resolve = |id| {
        sketch
            .entities
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| match e.kind {
                EntityKind::Point { x, y } => Some((x, y)),
                _ => None,
            })
    };
    // Outer bounding box of all corner points = width × height.
    let pts: Vec<(f64, f64)> = sketch
        .entities
        .iter()
        .filter_map(|e| match e.kind {
            EntityKind::Point { x, y } => Some((x, y)),
            _ => None,
        })
        .collect();
    let max_x = pts.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
    let max_y = pts.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
    let min_x = pts.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
    let min_y = pts.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    assert!(
        (max_x - min_x - 8.0).abs() < 1e-9,
        "rounded-rect width should be the typed 8 mm; got {}",
        max_x - min_x
    );
    assert!(
        (max_y - min_y - 5.0).abs() < 1e-9,
        "rounded-rect height should be the typed 5 mm; got {}",
        max_y - min_y
    );
    // Each corner arc has radius = |centre, start| = typed 1.5 mm.
    let arc = sketch
        .entities
        .iter()
        .find_map(|e| match e.kind {
            EntityKind::Arc { center, start, .. } => Some((center, start)),
            _ => None,
        })
        .expect("rounded-rect mints corner arcs");
    let (cx, cy) = resolve(arc.0).expect("arc centre resolves");
    let (sx, sy) = resolve(arc.1).expect("arc start resolves");
    let arc_r = ((sx - cx).powi(2) + (sy - cy).powi(2)).sqrt();
    assert!(
        (arc_r - 1.5).abs() < 1e-9,
        "corner radius should be the typed 1.5 mm; got {arc_r}"
    );
}
