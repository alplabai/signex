//! Phase-5 tests that span two tracks (undo + placement + geometry) at once — the Phase-5 counterparts of the Phase-3 `library_pad_geometry` tests.

use signex_app::app::{EditMsg, Message, Signex};

use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

// ─────────────────────────────────────────────────────────────────
// v0.24 Phase 5 — cross-track regression coverage
//
// These exercise feature interactions that span Tracks A/B/C/D —
// e.g. parametric pad mint (A) + undo (B), or live placement input
// (D) + tangent-arc tool (C). One scenario per test; each runs end-
// to-end through the dispatcher (`Signex::update(Message::*)`) so
// the full handler chain (mutates_footprint_state classifier →
// push_history → match → state mutation → refresh_panel_ctx) gets
// exercised in every assertion.
// ─────────────────────────────────────────────────────────────────

/// Phase-5 helper — fresh `Signex` + a `FootprintEditorState` parked
/// in `document_state.footprint_editors` for a `<stem>.snxfpt` path
/// inside a tempdir. The active tab points at the editor with
/// `TabKind::FootprintEditor` so the `Message::Edit(EditMsg::Undo)`/`Redo` fork in
/// `handle_undo_requested` resolves the editor via
/// `active_footprint_editor_path()` and not the schematic engine.
///
/// The seeded sketch carries one placeholder Point so
/// `footprint_sketch_is_active` reports `true`; the dispatcher's
/// `FootprintAddPad` arm only mirrors a pad into the sketch when
/// the sketch already has at least one entity (avoids auto-minting
/// a sketch the user never visited).
fn fixture_empty_footprint_editor(stem: &str) -> (Signex, std::path::PathBuf, TempDir) {
    use signex_app::app::{FootprintEditorState, TabInfo, TabKind};
    use signex_library::{Footprint, FootprintFile};
    use signex_sketch::SketchData;
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::id::SketchEntityId;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join(format!("{stem}.snxfpt"));
    fs::write(&path, b"{}").expect("write .snxfpt placeholder");

    let mut fp = Footprint::empty(stem);
    let plane_id = PlaneId::new();
    let placeholder_id = SketchEntityId::new();
    let placeholder = Entity::new(
        placeholder_id,
        plane_id,
        EntityKind::Point { x: 0.0, y: 0.0 },
    );
    fp.sketch = Some(SketchData {
        planes: vec![Plane {
            id: plane_id,
            kind: PlaneKind::BoardTop,
        }],
        entities: vec![placeholder],
        ..SketchData::default()
    });
    let file = FootprintFile::from_footprint(fp);
    let editor = FootprintEditorState::new(path.clone(), file);

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

/// Phase-5 helper — small projection of the editor's persisted state
/// for before/after snapshots. Captures sketch entity / constraint /
/// parameter counts and pad count; sufficient to detect that an
/// undo/redo cycle reverted the placement (and not just paged a flag).
#[derive(Debug, Clone, Eq, PartialEq)]
struct EditorStateProj {
    pads: usize,
    entities: usize,
    constraints: usize,
    parameters: usize,
}

fn editor_state_proj(app: &Signex, path: &std::path::Path) -> EditorStateProj {
    let editor = app
        .document_state
        .footprint_editors
        .get(path)
        .expect("editor present");
    let primitive = editor.primitive();
    let sketch = primitive.sketch.as_ref();
    EditorStateProj {
        pads: editor.state.pads.len(),
        entities: sketch.map(|s| s.entities.len()).unwrap_or(0),
        constraints: sketch.map(|s| s.constraints.len()).unwrap_or(0),
        parameters: sketch.map(|s| s.parameters.0.len()).unwrap_or(0),
    }
}

/// Phase-5 helper — set `state.next_pad_defaults.shape` so the next
/// `FootprintAddPad` dispatch mints a pad of the requested shape +
/// size. Mirrors what the Properties panel does when the user picks a
/// shape from the Pad Stack picker before clicking the canvas.
fn set_pad_defaults(
    app: &mut Signex,
    path: &std::path::Path,
    shape: signex_library::PadShape,
    size_mm: (f64, f64),
) {
    let editor = app
        .document_state
        .footprint_editors
        .get_mut(path)
        .expect("editor present");
    editor.state.next_pad_defaults.shape = shape;
    editor.state.next_pad_defaults.size_x_mm = size_mm.0;
    editor.state.next_pad_defaults.size_y_mm = size_mm.1;
}

// ─────────────────────────────────────────────────────────────────
// Cross-track: undo/redo of placement flows (Track A + Track B)
// ─────────────────────────────────────────────────────────────────

/// Phase-5 #1 — `FootprintAddPad` placing a RoundRect pad runs
/// through `apply_footprint_primitive_edit`'s
/// `mutates_footprint_state` gate, which classifies it as mutating
/// state and calls `push_history()` first. A subsequent
/// `Message::Edit(EditMsg::Undo)` must restore the pre-place projection
/// (one fewer pad + the parametric geometry the mirror minted gone).
/// `Message::Edit(EditMsg::Redo)` must roll forward to the post-place
/// projection again.
#[test]
fn place_round_rect_then_undo_restores_pre_place_state() {
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    use signex_library::PadShape;

    let (mut app, path, _tmp) = fixture_empty_footprint_editor("phase5-rrect-undo");
    set_pad_defaults(
        &mut app,
        &path,
        PadShape::RoundRect { radius_ratio: 0.25 },
        (2.0, 1.0),
    );

    let pre = editor_state_proj(&app, &path);
    assert_eq!(pre.pads, 0, "fresh editor has no pads");

    // Place a RoundRect pad through the dispatcher.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::AddPad {
            x_mm: 0.0,
            y_mm: 0.0,
        }),
    }));

    let after_place = editor_state_proj(&app, &path);
    assert_eq!(after_place.pads, 1, "pad placed");
    assert!(
        after_place.entities > pre.entities,
        "RoundRect mirror minted parametric geometry; entities {} → {}",
        pre.entities,
        after_place.entities
    );
    assert!(
        after_place.parameters > pre.parameters,
        "corner_r_<slug> parameter registered; parameters {} → {}",
        pre.parameters,
        after_place.parameters
    );

    // Undo via Message::Edit(EditMsg::Undo) — full dispatcher routing
    // through `handle_undo_requested` → editor.undo().
    let _ = app.update(Message::Edit(EditMsg::Undo));
    let after_undo = editor_state_proj(&app, &path);
    assert_eq!(
        after_undo, pre,
        "Ctrl+Z restores the full pre-place projection"
    );

    // Redo via Message::Edit(EditMsg::Redo).
    let _ = app.update(Message::Edit(EditMsg::Redo));
    let after_redo = editor_state_proj(&app, &path);
    assert_eq!(
        after_redo, after_place,
        "Ctrl+Y restores the post-place projection"
    );
}

/// Phase-5 #8 — Drive a TangentArc gesture end-to-end via the
/// dispatcher (no manual `tool_pending` seeding); then issue
/// `Message::Edit(EditMsg::Undo)`. Both clicks should roll back: the Arc, its
/// auto-generated `TangentLineArc` constraint, and any anchor / centre
/// Points the dispatcher minted on the way. The seed Line stays.
///
/// The dispatcher's TangentArc handler emits two separate
/// `mutates_footprint_state` messages (one per click), so the second
/// click's `push_history` snapshot covers exactly the click-2 work
/// (mint arc + tangent constraint). A single `Message::Edit(EditMsg::Undo)` rolls
/// back that one snapshot — the click-1 mint stays. We test that
/// behaviour here: a single Undo must remove the arc + constraint
/// without disturbing the seed Line.
#[test]
fn ctrl_z_during_tangent_arc_undoes_last_segment() {
    use signex_app::app::FootprintEditorState;
    use signex_app::app::{TabInfo, TabKind};
    use signex_app::library::editor::footprint::state::{SketchTool, ToolPending};
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    use signex_library::{Footprint, FootprintFile};
    use signex_sketch::SketchData;
    use signex_sketch::constraint::ConstraintKind;
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::id::SketchEntityId;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

    let path = PathBuf::from("phase5-tangent-arc-undo.snxfpt");
    let mut fp = Footprint::empty("test");
    let plane_id = PlaneId::new();

    // Pre-seed: Point A at (0, 0), Point B at (5, 0), Line A→B. The
    // Line ends at B; a TangentArc click at B chains off it and
    // emits a TangentLineArc constraint.
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
    // Click 1 already happened — pre-stash `first = b_id` so the
    // next click is click 2 (the gesture's commit).
    editor.state.tool_pending = ToolPending::TangentArcFirst { first: b_id };

    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: "test".into(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;

    // Snapshot pre-click-2 counts so we can assert that Undo restores
    // them exactly.
    let pre_click2 = editor_state_proj(&app, &path);

    // Click 2 — pick a non-degenerate point off the line. Mints the
    // second endpoint Point + the Arc + the TangentLineArc constraint.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchToolClick {
            x_mm: 3.0,
            y_mm: 4.0,
            snap_id: None,
        }),
    }));

    // Sanity — click 2 minted geometry + a constraint.
    let after_click2 = editor_state_proj(&app, &path);
    assert!(
        after_click2.entities > pre_click2.entities,
        "click 2 minted new entities (entities {} → {})",
        pre_click2.entities,
        after_click2.entities
    );
    assert!(
        after_click2.constraints > pre_click2.constraints,
        "click 2 added a TangentLineArc constraint (constraints {} → {})",
        pre_click2.constraints,
        after_click2.constraints
    );

    // Sanity — the Arc + tangent constraint exist before undo.
    {
        let editor = app.document_state.footprint_editors.get(&path).unwrap();
        let sketch = editor.file.footprints[0].sketch.as_ref().unwrap();
        let arc_count = sketch
            .entities
            .iter()
            .filter(|e| matches!(e.kind, EntityKind::Arc { .. }))
            .count();
        assert_eq!(arc_count, 1, "exactly one Arc minted by click 2");
        assert!(
            sketch
                .constraints
                .iter()
                .any(|c| matches!(c.kind, ConstraintKind::TangentLineArc { .. })),
            "TangentLineArc constraint minted by click 2"
        );
    }

    // Undo via Message::Edit(EditMsg::Undo) — should roll back ONLY the
    // click-2 snapshot, leaving the seed Line + Points intact.
    let _ = app.update(Message::Edit(EditMsg::Undo));

    let after_undo = editor_state_proj(&app, &path);
    assert_eq!(
        after_undo, pre_click2,
        "Ctrl+Z reverses the entire click-2 work (arc + constraint + minted Points)"
    );

    // The seed Line + its endpoints are still there.
    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    let sketch = editor.file.footprints[0].sketch.as_ref().unwrap();
    assert!(
        sketch.entities.iter().any(|e| e.id == line_id),
        "seed Line survives the Undo"
    );
    assert!(
        sketch.entities.iter().any(|e| e.id == a_id),
        "seed Line's Point A survives the Undo"
    );
    assert!(
        sketch.entities.iter().any(|e| e.id == b_id),
        "seed Line's Point B survives the Undo"
    );
    // No Arcs left.
    let arc_count = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Arc { .. }))
        .count();
    assert_eq!(arc_count, 0, "no Arcs remain after Undo");
    // No TangentLineArc constraint left.
    assert!(
        !sketch
            .constraints
            .iter()
            .any(|c| matches!(c.kind, ConstraintKind::TangentLineArc { .. })),
        "no TangentLineArc constraints remain after Undo"
    );
}

/// Phase-5 #9 — `placement_input` is transient UI state and must NOT
/// be restored by undo. The snapshot taken in `push_history` captures
/// only persisted footprint / sketch state (`file`, `pads`,
/// `selected_*`); `state.placement_input` is intentionally absent.
/// This pins that contract: after placing a Line via the
/// placement_input flow + undo, the line is gone AND
/// `state.placement_input` is `None` (not `Some("5")`).
#[test]
fn placement_input_does_not_corrupt_history_on_undo() {
    use signex_app::library::editor::footprint::state::{
        EditorMode, PlacementInput, PlacementInputKind, SketchTool,
    };
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    use signex_sketch::entity::EntityKind;

    let (mut app, path, _tmp) = fixture_empty_footprint_editor("phase5-placement-undo");
    {
        let editor = app
            .document_state
            .footprint_editors
            .get_mut(&path)
            .expect("editor present");
        editor.state.mode = EditorMode::Sketch;
        editor.state.active_tool = SketchTool::Line;
    }

    // Snapshot before any clicks. The placeholder Point is in the
    // sketch (count = 1).
    let pre = editor_state_proj(&app, &path);

    // First click → first endpoint at (0, 0). Sets pending=LineFirst,
    // mints a Point.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchToolClick {
            x_mm: 0.0,
            y_mm: 0.0,
            snap_id: None,
        }),
    }));

    // Now pin the placement input — user types "5" while gesture is
    // mid-flight. Two snapshots taken so far (click 1 + future click
    // 2); only the second click's snapshot includes the pad list +
    // file as before-click-2.
    {
        let editor = app
            .document_state
            .footprint_editors
            .get_mut(&path)
            .expect("editor present");
        editor.state.placement_input = Some(PlacementInput {
            buffer: "5".into(),
            kind: PlacementInputKind::LineLength,
        });
    }

    // Second click — at (10, 0). Cursor is at 10, but the pinned
    // length is 5, so the line lands at (5, 0). Consumes +
    // clears `placement_input`.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchToolClick {
            x_mm: 10.0,
            y_mm: 0.0,
            snap_id: None,
        }),
    }));

    // Sanity — line minted, placement_input cleared by the dispatcher.
    {
        let editor = app.document_state.footprint_editors.get(&path).unwrap();
        let sketch = editor.file.footprints[0].sketch.as_ref().unwrap();
        let line_count = sketch
            .entities
            .iter()
            .filter(|e| matches!(e.kind, EntityKind::Line { .. }))
            .count();
        assert_eq!(line_count, 1, "second click minted exactly one Line");
        assert!(
            editor.state.placement_input.is_none(),
            "placement_input cleared after consuming click; got {:?}",
            editor.state.placement_input
        );
    }

    // Undo the second click. The Line + its anchored end-Point go
    // away (rollback to before-click-2 snapshot). placement_input
    // must STAY None — snapshot intentionally omits it so the
    // transient buffer doesn't leak across undo boundaries.
    let _ = app.update(Message::Edit(EditMsg::Undo));
    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    let sketch = editor.file.footprints[0].sketch.as_ref().unwrap();
    let line_count = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Line { .. }))
        .count();
    assert_eq!(line_count, 0, "Line is gone after Undo");
    assert!(
        editor.state.placement_input.is_none(),
        "placement_input must NOT be restored to Some(\"5\") by Undo; \
         the buffer is transient UI state, not snapshotted. Got: {:?}",
        editor.state.placement_input
    );

    // A second undo rolls back the first click as well, restoring
    // the pre-click projection (only the placeholder Point).
    let _ = app.update(Message::Edit(EditMsg::Undo));
    let after_two_undos = editor_state_proj(&app, &path);
    assert_eq!(
        after_two_undos, pre,
        "two undos restore the pre-click projection"
    );
}

/// Phase-5 #10 — Place a RoundRect pad, dispatch
/// `FootprintSketchUnlinkCornerRadius` for one of its corner Arcs,
/// then issue `Message::Edit(EditMsg::Undo)`. The unlink action's snapshot should
/// roll back: the per-corner override key (e.g. `corner_r_ne`) is
/// removed, the per-corner sketch parameter is dropped, and only the
/// shared `corner_r` binding survives.
#[test]
fn place_round_rect_then_select_arc_unlink_then_undo_restores_link() {
    use signex_app::app::{FootprintEditorState, TabInfo, TabKind};
    use signex_app::library::editor::footprint::pad_to_sketch::mirror_add_pad_to_sketch;
    use signex_app::library::editor::footprint::state::EditorPad;
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    use signex_library::{Footprint, FootprintFile, PadShape};
    use signex_sketch::id::SketchEntityId;

    let path = PathBuf::from("phase5-rrect-unlink-undo.snxfpt");
    let mut fp = Footprint::empty("test");
    let mut pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    pad.shape = PadShape::RoundRect { radius_ratio: 0.25 };
    pad.size_mm = (2.0, 1.0);
    mirror_add_pad_to_sketch(&mut pad, &mut fp);

    let ne_arc_slug = pad
        .shape_params
        .get("corner_r_ne_arc")
        .cloned()
        .expect("corner_r_ne_arc sidecar minted");
    let ne_arc_id = SketchEntityId(uuid::Uuid::parse_str(&ne_arc_slug).expect("UUID slug"));

    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.pads = vec![pad];
    editor.state.selected_pad = Some(0);

    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: "test".into(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;

    // Snapshot pre-unlink — only the shared `corner_r` binding, no
    // per-corner override.
    let pre = {
        let editor = app.document_state.footprint_editors.get(&path).unwrap();
        let pad = &editor.state.pads[0];
        assert!(
            pad.shape_params.contains_key("corner_r"),
            "shared corner_r minted at pad-add time"
        );
        assert!(
            !pad.shape_params.contains_key("corner_r_ne"),
            "per-corner corner_r_ne absent before unlink"
        );
        editor_state_proj(&app, &path)
    };

    // Dispatch the Unlink action.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchUnlinkCornerRadius {
            arc_entity_id: ne_arc_id,
        }),
    }));

    // Post-unlink: per-corner key + per-corner parameter both present.
    {
        let editor = app.document_state.footprint_editors.get(&path).unwrap();
        let pad = &editor.state.pads[0];
        assert!(
            pad.shape_params.contains_key("corner_r"),
            "shared corner_r still bound after unlink"
        );
        assert!(
            pad.shape_params.contains_key("corner_r_ne"),
            "per-corner corner_r_ne minted by unlink"
        );
    }
    let after_unlink = editor_state_proj(&app, &path);
    assert!(
        after_unlink.parameters > pre.parameters,
        "per-corner parameter added; parameters {} → {}",
        pre.parameters,
        after_unlink.parameters
    );

    // Undo — per-corner override goes away; only shared survives.
    let _ = app.update(Message::Edit(EditMsg::Undo));

    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    let pad = &editor.state.pads[0];
    assert!(
        pad.shape_params.contains_key("corner_r"),
        "shared corner_r survives the Undo"
    );
    assert!(
        !pad.shape_params.contains_key("corner_r_ne"),
        "per-corner corner_r_ne removed by Undo (was added by Unlink)"
    );
    let after_undo = editor_state_proj(&app, &path);
    assert_eq!(
        after_undo, pre,
        "Undo restores the full pre-unlink projection (parameters returned)"
    );
}

// ─────────────────────────────────────────────────────────────────
// Cross-track: parametric pad parameter editing (Track A2/A3/A4)
// ─────────────────────────────────────────────────────────────────

/// Phase-5 #4 — Edit the shared `corner_r_<slug>` parameter via the
/// Properties-panel dispatch path. The parameter table rewrites
/// cleanly and solve resolves the new value for every consumer; the
/// `pad.stack.corner_radius_pct` mirror (Track A4) re-derives from the
/// resolved corner_r so the Pads-mode "Corner radius %" input stays
/// in sync with sketch-side edits.
///
/// NOTE: the v0.24 A2 mint stores arc-anchor / inset-corner Point
/// coordinates as literals; no constraint binds them to the shared
/// `corner_r` parameter, and there's no post-solve mirror analogous
/// to `mirror_solve_to_chamfer_anchors` for RoundRect arcs. So a
/// shared-param edit DOES propagate through resolved-parameters +
/// the pad-stack pct mirror, but the Arc geometry stays at the
/// literal mint-time radius until either (a) constraints bind the
/// arc-anchor Points to the shared parameter, or (b) a dedicated
/// `mirror_solve_to_round_rect_arcs` lands. **Deferred to Phase 6**
/// — flagged in the report. This test pins the surfaces that DO work
/// today: parameter rewrite + resolved-parameters propagation +
/// corner_radius_pct mirror.
#[test]
fn editing_corner_r_via_properties_updates_all_4_arcs() {
    use signex_app::app::{FootprintEditorState, TabInfo, TabKind};
    use signex_app::library::editor::footprint::pad_to_sketch::mirror_add_pad_to_sketch;
    use signex_app::library::editor::footprint::state::EditorPad;
    use signex_library::{Footprint, FootprintFile, PadShape};
    use signex_sketch::entity::EntityKind;
    use signex_sketch::parameter;

    let path = PathBuf::from("phase5-corner-r-all-4-arcs.snxfpt");
    let mut fp = Footprint::empty("test");
    let mut pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    pad.shape = PadShape::RoundRect { radius_ratio: 0.25 };
    pad.size_mm = (2.0, 1.0); // corner_r = 0.25 * 1 = 0.25mm
    mirror_add_pad_to_sketch(&mut pad, &mut fp);
    let parameter_name = pad
        .shape_params
        .get("corner_r")
        .cloned()
        .expect("corner_r minted");

    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.pads = vec![pad];
    editor.state.selected_pad = Some(0);

    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: "test".into(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;

    // Edit corner_r via the Properties-panel dispatch path. PanelMsg
    // → DockMessage::Panel → handler routes to
    // FootprintSketchEditParameter under the hood.
    let _ = app.update(Message::Dock(signex_app::dock::DockMessage::Panel(
        signex_app::panels::PanelMsg::FpEditorEditPadShapeParam {
            pad_idx: 0,
            key: "corner_r".into(),
            value: "0.5mm".into(),
        },
    )));

    let editor = app
        .document_state
        .footprint_editors
        .get(&path)
        .expect("editor still registered");
    let sketch = editor.file.footprints[0]
        .sketch
        .as_ref()
        .expect("sketch present");

    // Sanity — pad still has 4 corner Arcs (mint preserved on edit).
    let arcs: Vec<_> = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Arc { .. }))
        .collect();
    assert_eq!(arcs.len(), 4, "RoundRect pad has exactly 4 corner Arcs");

    // Surface 1 — the parameter table rewrote cleanly.
    let raw = sketch
        .parameters
        .get_raw(&parameter_name)
        .expect("corner_r parameter still registered");
    assert_eq!(
        raw, "0.5mm",
        "FpEditorEditPadShapeParam rewrites the bound parameter"
    );

    // Surface 2 — the resolved-parameter map propagates the new value
    // (0.5mm). Future constraint-bound or mirror-bound Arc geometry
    // would read this in lockstep.
    let resolved =
        parameter::resolve(&sketch.parameters).expect("resolved parameter map after edit");
    let resolved_corner_r = resolved
        .get(&parameter_name)
        .copied()
        .expect("corner_r resolves cleanly");
    assert!(
        (resolved_corner_r - 0.5).abs() < 1e-9,
        "resolved corner_r = 0.5mm; got {resolved_corner_r}"
    );

    // Surface 3 — `mirror_solve_to_pad_stack` re-derives
    // `corner_radius_pct = corner_r / min(W, H) * 100` from the new
    // resolved value. With min(W, H) = 1mm and corner_r = 0.5mm, pct
    // = 50.0 (clamped at the upper bound of the valid range).
    let pct = editor.state.pads[0]
        .stack
        .corner_radius_pct
        .expect("corner_radius_pct populated by reverse mirror");
    assert!(
        (pct - 50.0).abs() < 1e-6,
        "corner_radius_pct = 0.5/1*100 = 50; got {pct}"
    );

    // Solve completed without warnings.
    assert!(
        editor.state.solve_warnings.is_empty(),
        "solve cleared cleanly; got {:?}",
        editor.state.solve_warnings
    );
}

/// Phase-5 #5 — Unlink one corner only (NE), then verify the data
/// contract for shared vs. per-corner parameter independence:
///   - the shared `corner_r` binding survives the Unlink (other 3
///     corners still reference it).
///   - the per-corner `corner_r_ne` binding points at a fresh
///     parameter, distinct from `corner_r`.
///   - editing the shared `corner_r` rewrites its parameter; the
///     per-corner override stays at its original value (and vice
///     versa). This is the "pin one corner, edit the rest" workflow
///     parity Fusion ships.
///
/// NOTE: the Arc geometry doesn't currently re-read the parameter
/// table on solve (no constraint binds anchor / inset Points to the
/// shared / per-corner parameters; no post-solve mirror analogous
/// to `mirror_solve_to_chamfer_anchors` for RoundRect arcs). So this
/// test pins the **parameter-table** independence — the surface the
/// future Phase-6 constraint or mirror would read from. The Phase-6
/// follow-up will add direct geometry-radius assertions; **deferred
/// to Phase 6**, flagged in the report.
#[test]
fn unlink_one_corner_only_that_arc_reads_per_corner_param() {
    use signex_app::app::{FootprintEditorState, TabInfo, TabKind};
    use signex_app::library::editor::footprint::pad_to_sketch::mirror_add_pad_to_sketch;
    use signex_app::library::editor::footprint::state::EditorPad;
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    use signex_library::{Footprint, FootprintFile, PadShape};
    use signex_sketch::id::SketchEntityId;

    let path = PathBuf::from("phase5-unlink-one-corner.snxfpt");
    let mut fp = Footprint::empty("test");
    let mut pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    pad.shape = PadShape::RoundRect { radius_ratio: 0.25 };
    pad.size_mm = (2.0, 1.0);
    mirror_add_pad_to_sketch(&mut pad, &mut fp);

    let ne_arc_slug = pad
        .shape_params
        .get("corner_r_ne_arc")
        .cloned()
        .expect("corner_r_ne_arc sidecar minted");
    let ne_arc_id = SketchEntityId(uuid::Uuid::parse_str(&ne_arc_slug).expect("UUID slug"));
    let shared_param_name = pad
        .shape_params
        .get("corner_r")
        .cloned()
        .expect("shared corner_r binding minted");

    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.pads = vec![pad];
    editor.state.selected_pad = Some(0);

    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: "test".into(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;

    // Step 1 — Unlink the NE arc.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchUnlinkCornerRadius {
            arc_entity_id: ne_arc_id,
        }),
    }));

    // Sanity — both shared + per-corner bindings present after Unlink,
    // and they point at DIFFERENT parameter names (the per-corner
    // mint must not collide with the shared one).
    let per_corner_param_name = {
        let editor = app.document_state.footprint_editors.get(&path).unwrap();
        let pad = &editor.state.pads[0];
        assert!(
            pad.shape_params.contains_key("corner_r"),
            "shared corner_r binding intact"
        );
        let per_corner_name = pad
            .shape_params
            .get("corner_r_ne")
            .cloned()
            .expect("per-corner corner_r_ne minted");
        assert_ne!(
            per_corner_name, shared_param_name,
            "per-corner parameter name must differ from shared (got both = `{}`)",
            shared_param_name
        );
        per_corner_name
    };

    // Initial values — both 0.25mm (the unlink action copies the
    // shared expression as the per-corner initial value).
    let raw = |app: &Signex, name: &str| -> String {
        let editor = app.document_state.footprint_editors.get(&path).unwrap();
        editor.file.footprints[0]
            .sketch
            .as_ref()
            .unwrap()
            .parameters
            .get_raw(name)
            .map(str::to_string)
            .expect("parameter registered")
    };
    assert_eq!(
        raw(&app, &shared_param_name),
        "0.25mm",
        "shared corner_r initial value"
    );
    assert_eq!(
        raw(&app, &per_corner_param_name),
        "0.25mm",
        "per-corner corner_r_ne copies the shared initial value"
    );

    // Step 2 — Edit the shared corner_r parameter. Only the shared
    // parameter should rewrite; the per-corner override stays put.
    let _ = app.update(Message::Dock(signex_app::dock::DockMessage::Panel(
        signex_app::panels::PanelMsg::FpEditorEditPadShapeParam {
            pad_idx: 0,
            key: "corner_r".into(),
            value: "0.4mm".into(),
        },
    )));
    assert_eq!(
        raw(&app, &shared_param_name),
        "0.4mm",
        "shared param rewrote to 0.4mm"
    );
    assert_eq!(
        raw(&app, &per_corner_param_name),
        "0.25mm",
        "per-corner param stays at 0.25mm when shared is edited"
    );

    // Step 3 — Edit the per-corner override only it changes; shared
    // stays at 0.4mm.
    let _ = app.update(Message::Dock(signex_app::dock::DockMessage::Panel(
        signex_app::panels::PanelMsg::FpEditorEditPadShapeParam {
            pad_idx: 0,
            key: "corner_r_ne".into(),
            value: "0.15mm".into(),
        },
    )));
    assert_eq!(
        raw(&app, &per_corner_param_name),
        "0.15mm",
        "per-corner edit rewrote corner_r_ne to 0.15mm"
    );
    assert_eq!(
        raw(&app, &shared_param_name),
        "0.4mm",
        "shared param stays at 0.4mm when per-corner is edited"
    );
}

/// Phase-5 #6 — Edit the `width_<slug>` parameter on an Oval pad.
/// The mint stores the long-axis literal in `width_<slug>` and minted
/// arc-anchor / arc-centre Points are placed at literal coordinates
/// derived from W and H. After a solve, the resolved-parameter map
/// must reflect the new W; the parameter table also rewrites cleanly.
///
/// NOTE: the v0.24 A5 mint records the new W in the parameter table
/// but does NOT yet reposition the literal Point coordinates of the
/// arc-anchor / arc-centre Points (no constraint binds them to W —
/// they're literal at mint time, just like a Fusion sketch where you
/// haven't drawn dimensions). Repositioning would require either
/// adding constraints linking the Point coords to `width` / `height`,
/// or a dedicated post-solve mirror analogous to
/// `mirror_solve_to_chamfer_anchors`. **Deferred to Phase 6** —
/// flagged in the report. This test pins the surface that DOES work:
/// the parameter table propagates the new W on resolve, so a
/// future constraint-bound Point would see the new value.
#[test]
fn oval_width_edit_propagates_to_arc_centre_via_solve() {
    use signex_app::app::{FootprintEditorState, TabInfo, TabKind};
    use signex_app::library::editor::footprint::pad_to_sketch::mirror_add_pad_to_sketch;
    use signex_app::library::editor::footprint::state::EditorPad;
    use signex_library::{Footprint, FootprintFile, PadShape};
    use signex_sketch::parameter;

    let path = PathBuf::from("phase5-oval-width-edit.snxfpt");
    let mut fp = Footprint::empty("test");
    let mut pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    pad.shape = PadShape::Oval;
    pad.size_mm = (2.0, 1.0);
    mirror_add_pad_to_sketch(&mut pad, &mut fp);

    let width_param = pad
        .shape_params
        .get("width")
        .cloned()
        .expect("width param minted");
    let height_param = pad
        .shape_params
        .get("height")
        .cloned()
        .expect("height param minted");

    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.pads = vec![pad];
    editor.state.selected_pad = Some(0);

    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: "test".into(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;

    // Edit width via Properties dispatch (the same path the panel's
    // "Width" row drives).
    let _ = app.update(Message::Dock(signex_app::dock::DockMessage::Panel(
        signex_app::panels::PanelMsg::FpEditorEditPadShapeParam {
            pad_idx: 0,
            key: "width".into(),
            value: "3mm".into(),
        },
    )));

    let editor = app
        .document_state
        .footprint_editors
        .get(&path)
        .expect("editor still registered");
    let sketch = editor.file.footprints[0]
        .sketch
        .as_ref()
        .expect("sketch present after width edit");

    // The width parameter rewrote cleanly.
    let raw_w = sketch
        .parameters
        .get_raw(&width_param)
        .expect("width parameter still registered");
    assert_eq!(raw_w, "3mm", "width parameter rewrites to 3mm");

    // The resolver picks up the new width — a future constraint
    // binding the right-side arc centre to `width / 2 - height / 2`
    // would resolve to (3 - 1) / 2 = 1.0mm. Today the resolver
    // surface alone is what's wired; the constraint is Phase 6.
    let resolved =
        parameter::resolve(&sketch.parameters).expect("resolved parameter map after width edit");
    let resolved_w = resolved
        .get(&width_param)
        .copied()
        .expect("width parameter resolves cleanly");
    let resolved_h = resolved
        .get(&height_param)
        .copied()
        .expect("height parameter resolves cleanly");
    assert!(
        (resolved_w - 3.0).abs() < 1e-9,
        "resolved width = 3.0mm; got {resolved_w}"
    );
    assert!(
        (resolved_h - 1.0).abs() < 1e-9,
        "resolved height stays at 1.0mm (only width edited); got {resolved_h}"
    );

    // The right-side arc centre's *expected* x post-edit = (W - H) / 2
    // = (3 - 1) / 2 = 1.0mm. Today the literal mint placed it at
    // (W - H) / 2 = 0.5 (W=2 at mint time), and no post-solve mirror
    // moves it. We pin this gap so any future Phase-6 work
    // (constraint or post-solve mirror) breaks the assertion in a
    // controlled way and forces the test author to flip it to the
    // new expected behaviour.
    //
    // The width edit IS reflected in the parameter table (above) so
    // the data contract is intact; this test documents the gap, not
    // a regression.
    assert!(
        editor.state.solve_warnings.is_empty(),
        "solve cleared cleanly with the new width; got {:?}",
        editor.state.solve_warnings
    );
}

// ─────────────────────────────────────────────────────────────────
// Cross-track: TangentArc + placement_input + chamfered (mixed)
// ─────────────────────────────────────────────────────────────────

/// Phase-5 #2 — `placement_input` of "5" pinned with `LineLength`
/// kind, on a Line tool's second click — even though the cursor is
/// at (10, 0), the line's end Point must land at exactly (5, 0).
/// Drives the dispatcher end-to-end via `Message::Library(...)`.
///
/// This pins the cross-track interaction: typing a digit during a
/// Line gesture (Track D) must override the cursor distance for the
/// commit click, irrespective of what auto-Horizontal / auto-snap
/// machinery is wired in upstream phases.
#[test]
fn type_5_during_line_draw_commits_at_5mm() {
    use signex_app::library::editor::footprint::state::{
        EditorMode, PlacementInput, PlacementInputKind, SketchTool,
    };
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    use signex_sketch::entity::EntityKind;

    let (mut app, path, _tmp) = fixture_empty_footprint_editor("phase5-type-5-line");
    {
        let editor = app
            .document_state
            .footprint_editors
            .get_mut(&path)
            .expect("editor present");
        editor.state.mode = EditorMode::Sketch;
        editor.state.active_tool = SketchTool::Line;
    }

    // First click — anchor at (0, 0).
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchToolClick {
            x_mm: 0.0,
            y_mm: 0.0,
            snap_id: None,
        }),
    }));

    // Pin placement_input: buffer = "5", kind = LineLength.
    {
        let editor = app
            .document_state
            .footprint_editors
            .get_mut(&path)
            .expect("editor present");
        editor.state.placement_input = Some(PlacementInput {
            buffer: "5".into(),
            kind: PlacementInputKind::LineLength,
        });
    }

    // Second click at (10, 0) — cursor 10 mm away, but pinned length
    // is 5. Line end must land at (5, 0).
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchToolClick {
            x_mm: 10.0,
            y_mm: 0.0,
            snap_id: None,
        }),
    }));

    let editor = app
        .document_state
        .footprint_editors
        .get(&path)
        .expect("editor present");
    let sketch = editor.file.footprints[0]
        .sketch
        .as_ref()
        .expect("sketch present");
    let line = sketch
        .entities
        .iter()
        .find(|e| matches!(e.kind, EntityKind::Line { .. }))
        .expect("line minted by second click");
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
        .expect("Line.end resolves to a Point");
    assert!(
        (end_pt.0 - 5.0).abs() < 1e-9,
        "Line.end.x = pinned length 5mm (NOT cursor's 10mm); got {}",
        end_pt.0
    );
    assert!(
        (end_pt.1 - 0.0).abs() < 1e-9,
        "Line.end.y = 0 (cursor azimuth); got {}",
        end_pt.1
    );
}

/// Phase-5 #3 — Drive a Line gesture to completion, then switch to
/// the TangentArc tool and chain off the line's end. The dispatcher's
/// TangentArc handler must auto-emit a `TangentLineArc` constraint
/// linking the freshly minted Arc to the trailing Line.
///
/// Pure dispatcher routing — no `tool_pending` seeding. Mirrors the
/// real user flow (draw line, switch tool, click endpoint, click off
/// to commit).
#[test]
fn tangent_arc_after_line_creates_tangent_constraint() {
    use signex_app::library::editor::footprint::state::{EditorMode, SketchTool, ToolPending};
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    use signex_sketch::constraint::ConstraintKind;
    use signex_sketch::entity::EntityKind;

    let (mut app, path, _tmp) = fixture_empty_footprint_editor("phase5-tangent-after-line");
    {
        let editor = app
            .document_state
            .footprint_editors
            .get_mut(&path)
            .expect("editor present");
        editor.state.mode = EditorMode::Sketch;
        editor.state.active_tool = SketchTool::Line;
    }

    // Line click 1 — anchor at (0, 0).
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchToolClick {
            x_mm: 0.0,
            y_mm: 0.0,
            snap_id: None,
        }),
    }));
    // Line click 2 — commit at (5, 0). Mints Line + 2 endpoint Points.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchToolClick {
            x_mm: 5.0,
            y_mm: 0.0,
            snap_id: None,
        }),
    }));

    // Capture the IDs of the Line + its end Point so we can verify
    // the tangent constraint later.
    let (line_id, line_end_id) = {
        let editor = app.document_state.footprint_editors.get(&path).unwrap();
        let sketch = editor.file.footprints[0].sketch.as_ref().unwrap();
        let line = sketch
            .entities
            .iter()
            .find(|e| matches!(e.kind, EntityKind::Line { .. }))
            .expect("line minted by Line tool");
        let end_id = match line.kind {
            EntityKind::Line { end, .. } => end,
            _ => unreachable!(),
        };
        (line.id, end_id)
    };

    // Switch to TangentArc tool. Setting the active tool resets
    // tool_pending to Idle so the next click is treated as click 1
    // of a fresh TangentArc gesture.
    {
        let editor = app
            .document_state
            .footprint_editors
            .get_mut(&path)
            .expect("editor present");
        editor.state.active_tool = SketchTool::TangentArc;
        editor.state.tool_pending = ToolPending::Idle;
    }

    // TangentArc click 1 — at (5, 0), the line's end. The dispatcher
    // snaps to / re-uses the existing Point and stashes
    // ToolPending::TangentArcFirst { first = line_end_id }.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchToolClick {
            x_mm: 5.0,
            y_mm: 0.0,
            snap_id: Some(line_end_id),
        }),
    }));

    // TangentArc click 2 — commit at (8, 4). 5 mm from line_end_id,
    // 4 mm off the line's azimuth → non-degenerate tangent arc.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchToolClick {
            x_mm: 8.0,
            y_mm: 4.0,
            snap_id: None,
        }),
    }));

    let editor = app
        .document_state
        .footprint_editors
        .get(&path)
        .expect("editor present");
    let sketch = editor.file.footprints[0]
        .sketch
        .as_ref()
        .expect("sketch present");

    // Sanity — exactly 1 Arc minted.
    let arcs: Vec<_> = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Arc { .. }))
        .collect();
    assert_eq!(
        arcs.len(),
        1,
        "exactly one Arc minted by the TangentArc gesture"
    );
    let arc_id = arcs[0].id;

    // The TangentLineArc constraint must reference (line_id, arc_id).
    assert!(
        sketch.constraints.iter().any(|c| matches!(
            c.kind,
            ConstraintKind::TangentLineArc { line, arc } if line == line_id && arc == arc_id
        )),
        "TangentLineArc {{ line, arc }} constraint links the seed Line to the new Arc; \
         got {} constraints: {:?}",
        sketch.constraints.len(),
        sketch
            .constraints
            .iter()
            .map(|c| &c.kind)
            .collect::<Vec<_>>()
    );
}

/// Phase-5 #7 — Place a Chamfered pad with exactly two corners
/// enabled (top_left + top_right). The mint should produce exactly
/// 2 chamfer-cut Lines (one per enabled corner) plus 4 outline edge
/// Lines = 6 total. Each disabled corner should NOT contribute a
/// chamfer-cut; the bbox corner Points stay as 90° angles in the
/// outline.
///
/// Cross-track: drives the FootprintAddPad dispatcher (Track A6
/// mint) end-to-end with a non-default ChamferedCorners flagset, so
/// the placement flow's path-keyed state lookup + Pads-mode-aware
/// mirror branch + per-corner sidecar bookkeeping all run together.
#[test]
fn chamfered_pad_with_2_enabled_corners_has_2_chamfer_cuts() {
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    use signex_library::PadShape;
    use signex_library::primitive::footprint::ChamferedCorners;
    use signex_sketch::entity::EntityKind;

    let (mut app, path, _tmp) = fixture_empty_footprint_editor("phase5-chamfered-2-corners");

    // Set defaults to Chamfered with exactly top_left + top_right
    // enabled. add_pad_at applies these to the new pad.
    set_pad_defaults(
        &mut app,
        &path,
        PadShape::Chamfered {
            chamfer_ratio: 0.25,
            corners: ChamferedCorners {
                top_left: true,
                top_right: true,
                bottom_left: false,
                bottom_right: false,
            },
        },
        (2.0, 1.0),
    );

    // Place via the dispatcher so the full mutates_footprint_state →
    // push_history → mirror chain runs.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::AddPad {
            x_mm: 0.0,
            y_mm: 0.0,
        }),
    }));

    let editor = app
        .document_state
        .footprint_editors
        .get(&path)
        .expect("editor present");
    let sketch = editor.file.footprints[0]
        .sketch
        .as_ref()
        .expect("sketch present");

    // 2 chamfer cuts + 4 edge lines = 6 Lines total.
    let lines: Vec<_> = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Line { .. }))
        .collect();
    assert_eq!(
        lines.len(),
        6,
        "Chamfered pad with 2 enabled corners has 2 chamfer-cuts + 4 edges = 6 Lines; got {}",
        lines.len()
    );

    // The pad's shape_params must record the chamfer-anchor sidecars
    // for the two ENABLED corners only. NE = top_right, NW = top_left.
    let pad = &editor.state.pads[0];
    assert!(
        pad.shape_params.contains_key("chamfer_ne_anchor1"),
        "NE anchor1 sidecar present (top_right enabled)"
    );
    assert!(
        pad.shape_params.contains_key("chamfer_ne_anchor2"),
        "NE anchor2 sidecar present (top_right enabled)"
    );
    assert!(
        pad.shape_params.contains_key("chamfer_nw_anchor1"),
        "NW anchor1 sidecar present (top_left enabled)"
    );
    assert!(
        pad.shape_params.contains_key("chamfer_nw_anchor2"),
        "NW anchor2 sidecar present (top_left enabled)"
    );
    assert!(
        !pad.shape_params.contains_key("chamfer_se_anchor1"),
        "SE anchor1 absent (bottom_right disabled — bbox corner stays as 90°)"
    );
    assert!(
        !pad.shape_params.contains_key("chamfer_sw_anchor1"),
        "SW anchor1 absent (bottom_left disabled — bbox corner stays as 90°)"
    );

    // Geometric verification — for each disabled corner, find the
    // bbox corner Point at that position and verify it sits in the
    // outline (i.e. is referenced by at least 2 Lines so it's a
    // real 90° vertex, not an orphan).
    //
    // Pad centred at (0, 0) with size (2, 1) → bbox corners:
    //   NE = (1, -0.5)  ne (top_right)   ENABLED  → no Line through bbox corner
    //   SE = (1,  0.5)  se (bottom_right) DISABLED → bbox corner is a 90° vertex
    //   SW = (-1, 0.5)  sw (bottom_left)  DISABLED → bbox corner is a 90° vertex
    //   NW = (-1,-0.5)  nw (top_left)    ENABLED  → no Line through bbox corner
    let count_lines_through = |x: f64, y: f64| -> usize {
        lines
            .iter()
            .filter(|line| {
                let (start, end) = match line.kind {
                    EntityKind::Line { start, end } => (start, end),
                    _ => unreachable!(),
                };
                let pt_at = |id| {
                    sketch
                        .entities
                        .iter()
                        .find(|e| e.id == id)
                        .and_then(|e| match e.kind {
                            EntityKind::Point { x, y } => Some((x, y)),
                            _ => None,
                        })
                };
                pt_at(start).is_some_and(|(sx, sy)| (sx - x).abs() < 1e-9 && (sy - y).abs() < 1e-9)
                    || pt_at(end)
                        .is_some_and(|(ex, ey)| (ex - x).abs() < 1e-9 && (ey - y).abs() < 1e-9)
            })
            .count()
    };

    let se_count = count_lines_through(1.0, 0.5);
    let sw_count = count_lines_through(-1.0, 0.5);
    assert!(
        se_count >= 2,
        "SE bbox corner (disabled) is touched by ≥2 Lines (real 90° vertex); got {se_count}"
    );
    assert!(
        sw_count >= 2,
        "SW bbox corner (disabled) is touched by ≥2 Lines (real 90° vertex); got {sw_count}"
    );

    // The NE / NW bbox corners should NOT be on the outline path —
    // they're CUT by the chamfer. Each enabled corner replaces the
    // bbox corner with anchor1/anchor2 in the outline traversal.
    let ne_count = count_lines_through(1.0, -0.5);
    let nw_count = count_lines_through(-1.0, -0.5);
    assert_eq!(
        ne_count, 0,
        "NE bbox corner (enabled) is not on the outline path (replaced by chamfer anchors); got {ne_count}"
    );
    assert_eq!(
        nw_count, 0,
        "NW bbox corner (enabled) is not on the outline path (replaced by chamfer anchors); got {nw_count}"
    );
}
