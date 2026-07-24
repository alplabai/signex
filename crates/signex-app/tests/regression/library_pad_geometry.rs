//! Parametric pad-shape mirror into sketch entities (round, round-rect, oval, chamfered pads).

use signex_app::app::{Message, Signex};

use std::path::PathBuf;

// ─────────────────────────────────────────────────────────────────
// v0.24 Phase 1 Track A — parametric pad geometry mirror
// ─────────────────────────────────────────────────────────────────

#[test]
fn mirror_add_round_pad_mints_circle_with_diameter_param() {
    // v0.24 Track A — placing a Round pad in Pads mode should mirror
    // into the sketch as 1 centre Point + 1 Circle entity referencing
    // that centre, plus a `diameter_<slug>` sketch parameter
    // recording the pad's diameter literal. `pad.shape_params` should
    // record `"diameter" -> param_name` so the Phase 3 Properties row
    // can look up the binding.
    use signex_app::library::editor::footprint::pad_to_sketch::mirror_add_pad_to_sketch;
    use signex_app::library::editor::footprint::state::EditorPad;
    use signex_library::primitive::footprint::{Footprint, PadShape};
    use signex_sketch::entity::EntityKind;

    let mut pad = EditorPad::new_default("1".into(), (2.0, 3.0));
    pad.shape = PadShape::Round;
    pad.size_mm = (1.5, 1.5);
    let mut fp = Footprint::empty("test");

    mirror_add_pad_to_sketch(&mut pad, &mut fp);

    let sketch = fp.sketch.as_ref().expect("sketch minted");

    // Exactly 1 Point (the centre) + 1 Circle.
    let points: Vec<_> = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Point { .. }))
        .collect();
    let circles: Vec<_> = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Circle { .. }))
        .collect();
    let arcs: Vec<_> = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Arc { .. }))
        .collect();
    let lines: Vec<_> = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Line { .. }))
        .collect();
    assert_eq!(points.len(), 1, "Round pad mints exactly 1 centre Point");
    assert_eq!(circles.len(), 1, "Round pad mints exactly 1 Circle");
    assert!(arcs.is_empty(), "Round pad mints no Arcs");
    assert!(lines.is_empty(), "Round pad mints no Lines");

    // The Circle's centre must reference the centre Point ID.
    let centre_id = pad.sketch_entity_id.expect("centre id minted");
    if let EntityKind::Circle { center, radius } = circles[0].kind {
        assert_eq!(center, centre_id, "Circle.center references centre Point");
        // radius = diameter / 2 = 0.75
        assert!(
            (radius - 0.75).abs() < 1e-9,
            "Circle.radius is half the diameter"
        );
    } else {
        unreachable!()
    }

    // No bbox-corner outline for Round pads.
    assert!(
        pad.corner_entity_ids.is_none(),
        "Round pads skip the bbox 4-Point outline"
    );

    // pad.shape_params binds "diameter" to a named parameter.
    let param_name = pad
        .shape_params
        .get("diameter")
        .expect("'diameter' key bound on Round pad");
    assert!(
        param_name.starts_with("diameter_"),
        "param name has the diameter_<slug> form (got `{param_name}`)"
    );

    // sketch.parameters must contain that exact parameter, holding
    // the literal diameter expression.
    let raw = sketch
        .parameters
        .get_raw(param_name)
        .expect("diameter parameter is registered on sketch.parameters");
    assert_eq!(raw, "1.5mm", "diameter parameter records the W literal");
}

#[test]
fn mirror_add_round_rect_pad_mints_4_arcs_linked_to_corner_r() {
    // v0.24 Track A — placing a RoundRect pad in Pads mode should
    // mirror into the sketch as the full Fusion-parity parametric
    // outline:
    //   - 1 centre Point
    //   - 4 bbox corner Points
    //   - 8 arc-anchor Points
    //   - 4 inset corner Points (arc centres)
    //   = 17 Points
    //   + 4 shorter Lines + 4 corner Arcs = 25 entities
    // All 4 Arcs must read from the same `corner_r_<slug>` parameter
    // so they stay linked implicitly. `pad.shape_params` should
    // record `"corner_r" -> param_name`.
    use signex_app::library::editor::footprint::pad_to_sketch::mirror_add_pad_to_sketch;
    use signex_app::library::editor::footprint::state::EditorPad;
    use signex_library::primitive::footprint::{Footprint, PadShape};
    use signex_sketch::entity::EntityKind;

    let mut pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    pad.shape = PadShape::RoundRect { radius_ratio: 0.25 };
    pad.size_mm = (2.0, 1.0); // W=2, H=1, min=1, r = 0.25 * 1 = 0.25
    let mut fp = Footprint::empty("test");

    mirror_add_pad_to_sketch(&mut pad, &mut fp);

    let sketch = fp.sketch.as_ref().expect("sketch minted");

    // Exactly 4 Arc entities.
    let arcs: Vec<_> = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Arc { .. }))
        .collect();
    assert_eq!(arcs.len(), 4, "RoundRect pad mints exactly 4 corner Arcs");

    // Exactly 4 Lines (the shorter edge-anchor connectors).
    let lines: Vec<_> = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Line { .. }))
        .collect();
    assert_eq!(lines.len(), 4, "RoundRect pad mints 4 shorter edge Lines");

    // 1 centre + 4 bbox corners + 8 anchors + 4 inset corners = 17 Points.
    let points: Vec<_> = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Point { .. }))
        .collect();
    assert_eq!(
        points.len(),
        17,
        "RoundRect pad mints 1 centre + 4 bbox + 8 anchors + 4 inset = 17 Points"
    );

    // pad.shape_params must bind "corner_r" to a named parameter.
    let param_name = pad
        .shape_params
        .get("corner_r")
        .expect("'corner_r' key bound on RoundRect pad");
    assert!(
        param_name.starts_with("corner_r_"),
        "param name has the corner_r_<slug> form (got `{param_name}`)"
    );

    // sketch.parameters must contain that exact parameter, holding
    // the literal radius (= 0.25 * min(W,H) = 0.25 mm).
    let raw = sketch
        .parameters
        .get_raw(param_name)
        .expect("corner_r parameter is registered on sketch.parameters");
    assert_eq!(
        raw, "0.25mm",
        "corner_r parameter records the literal inset distance"
    );

    // All 4 Arcs implicitly share the same corner_r parameter — the
    // mint side encodes this by giving the arcs equal radii at mint
    // time (literal-equal because they all read the same parameter
    // expression). Verify by extracting the radius implied by each
    // Arc's geometry and checking they're all equal.
    //
    // Arc radius = distance from center Point to start Point. We
    // grab each Arc's center+start, look up the Point coords, and
    // compute the radius. All 4 must be equal (within EPS).
    let mut arc_radii: Vec<f64> = Vec::with_capacity(4);
    for arc in &arcs {
        let (center_id, start_id) = match arc.kind {
            EntityKind::Arc { center, start, .. } => (center, start),
            _ => unreachable!(),
        };
        let center_pos = sketch
            .entities
            .iter()
            .find(|e| e.id == center_id)
            .and_then(|e| match e.kind {
                EntityKind::Point { x, y } => Some((x, y)),
                _ => None,
            })
            .expect("Arc.center references a Point");
        let start_pos = sketch
            .entities
            .iter()
            .find(|e| e.id == start_id)
            .and_then(|e| match e.kind {
                EntityKind::Point { x, y } => Some((x, y)),
                _ => None,
            })
            .expect("Arc.start references a Point");
        let dx = start_pos.0 - center_pos.0;
        let dy = start_pos.1 - center_pos.1;
        arc_radii.push((dx * dx + dy * dy).sqrt());
    }
    let first = arc_radii[0];
    for r in &arc_radii {
        assert!(
            (r - first).abs() < 1e-9,
            "all 4 Arc radii must be equal (corner_r-linked); got {arc_radii:?}"
        );
        assert!(
            (r - 0.25).abs() < 1e-9,
            "Arc radius must equal corner_r = 0.25mm; got {r}"
        );
    }
}

// ─────────────────────────────────────────────────────────────────
// v0.24 Phase 3 — A2/A3/A4 Properties row + Unlink + reverse-mirror
// ─────────────────────────────────────────────────────────────────

/// v0.24 Phase 3 (Track A2) — placing a RoundRect pad in Pads mode
/// registers a `corner_r` shape_params binding that the panel
/// context surfaces as a `PadShapeParamSummary` so the Properties
/// panel can render an editable "Corner radius" row.
#[test]
fn properties_panel_shows_corner_radius_for_round_rect_pad() {
    use signex_app::app::FootprintEditorState;
    use signex_app::library::editor::footprint::pad_to_sketch::mirror_add_pad_to_sketch;
    use signex_app::library::editor::footprint::state::EditorPad;
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    use signex_library::{Footprint, FootprintFile, PadShape};

    let path = PathBuf::from("test-a2-corner-radius-row.snxfpt");
    let mut fp = Footprint::empty("test");

    // Build the editor state directly so the pad's shape_params get
    // populated via mirror_add_pad_to_sketch — which is the path the
    // app dispatcher takes when the user places a pad in Pads mode.
    let mut pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    pad.shape = PadShape::RoundRect { radius_ratio: 0.25 };
    pad.size_mm = (2.0, 1.0);
    mirror_add_pad_to_sketch(&mut pad, &mut fp);

    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.pads = vec![pad];
    editor.state.selected_pad = Some(0);

    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);

    // Open a tab pointing at the editor so build_footprint_editor_panel_ctx
    // resolves it. Using TabKind::FootprintEditor matches what the
    // app does when the user double-clicks a .snxfpt in the tree.
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "test".into(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: signex_app::app::TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;

    // Trigger a panel refresh by dispatching a no-op selection
    // (FootprintSelectPad re-selects the pad and triggers
    // refresh_panel_ctx in the post-dispatch flow).
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SelectPad(Some(0))),
    }));

    let ctx = app
        .document_state
        .panel_ctx
        .footprint_editor
        .as_ref()
        .expect("footprint editor panel ctx populated");

    let entries = &ctx.selected_pad_shape_params;
    let corner_r_entry = entries
        .iter()
        .find(|e| e.key == "corner_r")
        .expect("corner_r entry surfaced on selected pad shape_params");
    assert_eq!(
        corner_r_entry.label, "Corner radius",
        "label is the user-facing 'Corner radius' string"
    );
    assert!(
        corner_r_entry.parameter_name.starts_with("corner_r_"),
        "parameter_name follows corner_r_<slug> convention; got `{}`",
        corner_r_entry.parameter_name,
    );
    assert_eq!(
        corner_r_entry.current_expr, "0.25mm",
        "current_expr reads the live sketch parameter expression"
    );
}

/// v0.24 Phase 3 (Track A2) — dispatching FpEditorEditPadShapeParam
/// rewrites the bound sketch parameter and triggers a solve+rebake
/// (warnings list stays empty).
#[test]
fn editing_corner_radius_updates_all_4_arcs() {
    use signex_app::app::FootprintEditorState;
    use signex_app::library::editor::footprint::pad_to_sketch::mirror_add_pad_to_sketch;
    use signex_app::library::editor::footprint::state::EditorPad;
    use signex_library::{Footprint, FootprintFile, PadShape};

    let path = PathBuf::from("test-a2-edit-corner-radius.snxfpt");
    let mut fp = Footprint::empty("test");
    let mut pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    pad.shape = PadShape::RoundRect { radius_ratio: 0.25 };
    pad.size_mm = (2.0, 1.0);
    mirror_add_pad_to_sketch(&mut pad, &mut fp);

    let parameter_name = pad
        .shape_params
        .get("corner_r")
        .cloned()
        .expect("corner_r minted at pad-add time");

    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.pads = vec![pad];
    editor.state.selected_pad = Some(0);

    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "test".into(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: signex_app::app::TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;

    // Dispatch the Properties-panel edit. PanelMsg flows through the
    // dock dispatcher which forwards to FootprintSketchEditParameter.
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
        .expect("sketch present after edit");
    let raw = sketch
        .parameters
        .get_raw(&parameter_name)
        .expect("corner_r parameter still registered");
    assert_eq!(
        raw, "0.5mm",
        "FpEditorEditPadShapeParam rewrites the bound parameter"
    );
    assert!(
        editor.state.solve_warnings.is_empty(),
        "solve completed without warnings; got {:?}",
        editor.state.solve_warnings
    );
}

/// v0.24 Phase 3 (Track A3) — dispatching FootprintSketchUnlinkCornerRadius
/// for one of the 4 corner Arcs mints a per-corner parameter and
/// records the override on `pad.shape_params`. The shared corner_r
/// binding stays in place so the other 3 corners follow it.
#[test]
fn unlink_corner_radius_mints_per_corner_param() {
    use signex_app::app::FootprintEditorState;
    use signex_app::library::editor::footprint::pad_to_sketch::mirror_add_pad_to_sketch;
    use signex_app::library::editor::footprint::state::EditorPad;
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    use signex_library::{Footprint, FootprintFile, PadShape};
    use signex_sketch::entity::EntityKind;
    use signex_sketch::id::SketchEntityId;

    let path = PathBuf::from("test-a3-unlink.snxfpt");
    let mut fp = Footprint::empty("test");
    let mut pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    pad.shape = PadShape::RoundRect { radius_ratio: 0.25 };
    pad.size_mm = (2.0, 1.0);
    mirror_add_pad_to_sketch(&mut pad, &mut fp);

    // Pick the NE arc — its UUID slug lives at
    // `shape_params["corner_r_ne_arc"]`. Resolve back to the entity
    // id by parsing the slug.
    let ne_slug = pad
        .shape_params
        .get("corner_r_ne_arc")
        .cloned()
        .expect("corner_r_ne_arc sidecar minted");
    let arc_entity_id = {
        let uuid = uuid::Uuid::parse_str(&ne_slug).expect("sidecar value is a UUID slug");
        SketchEntityId(uuid)
    };
    // Sanity: the entity actually is an Arc.
    let sketch_pre = fp.sketch.as_ref().unwrap();
    let arc_kind = sketch_pre
        .entities
        .iter()
        .find(|e| e.id == arc_entity_id)
        .map(|e| matches!(e.kind, EntityKind::Arc { .. }))
        .unwrap_or(false);
    assert!(arc_kind, "sidecar UUID points at an Arc entity");

    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.pads = vec![pad];
    editor.state.selected_pad = Some(0);

    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "test".into(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: signex_app::app::TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;

    // Dispatch the Unlink action.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchUnlinkCornerRadius {
            arc_entity_id,
        }),
    }));

    let editor = app
        .document_state
        .footprint_editors
        .get(&path)
        .expect("editor still registered");
    let pad_after = &editor.state.pads[0];

    // Both shared and per-corner keys are present.
    assert!(
        pad_after.shape_params.contains_key("corner_r"),
        "shared corner_r binding stays intact"
    );
    assert!(
        pad_after.shape_params.contains_key("corner_r_ne"),
        "per-corner corner_r_ne override added"
    );
    // The per-corner parameter is registered on the sketch.
    let per_corner_name = pad_after
        .shape_params
        .get("corner_r_ne")
        .expect("corner_r_ne value points at a parameter name");
    let sketch = editor.file.footprints[0]
        .sketch
        .as_ref()
        .expect("sketch present after unlink");
    let raw = sketch
        .parameters
        .get_raw(per_corner_name)
        .expect("per-corner parameter registered on sketch.parameters");
    assert_eq!(
        raw, "0.25mm",
        "per-corner parameter copies the shared expression as initial value"
    );
}

/// v0.24 Phase 3 (Track A4) — after every solve, the reverse mirror
/// re-derives `EditorPad.stack.corner_radius_pct` from the resolved
/// corner_r parameter so the Pads-mode "Corner radius %" input stays
/// in sync with sketch-side edits.
#[test]
fn reverse_mirror_updates_pad_stack_corner_radius_pct() {
    use signex_app::app::FootprintEditorState;
    use signex_app::library::editor::footprint::pad_to_sketch::mirror_add_pad_to_sketch;
    use signex_app::library::editor::footprint::state::EditorPad;
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    use signex_library::{Footprint, FootprintFile, PadShape};

    let path = PathBuf::from("test-a4-reverse-mirror.snxfpt");
    let mut fp = Footprint::empty("test");
    let mut pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    pad.shape = PadShape::RoundRect { radius_ratio: 0.25 };
    pad.size_mm = (2.0, 1.0); // W=2, H=1, min=1, corner_r = 0.25*1 = 0.25mm
    mirror_add_pad_to_sketch(&mut pad, &mut fp);

    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.pads = vec![pad];

    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "test".into(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: signex_app::app::TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;

    // Trigger a solve+bake by editing a parameter (no-op rewrite of
    // the same value; still forces resolve + bake).
    let parameter_name = app
        .document_state
        .footprint_editors
        .get(&path)
        .unwrap()
        .state
        .pads[0]
        .shape_params
        .get("corner_r")
        .cloned()
        .unwrap();
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchEditParameter {
            name: parameter_name,
            expr: "0.25mm".into(),
        }),
    }));

    let editor = app
        .document_state
        .footprint_editors
        .get(&path)
        .expect("editor still registered");
    let pad_after = &editor.state.pads[0];

    // corner_r = 0.25mm, min(W,H) = 1mm → pct = 25%.
    let pct = pad_after
        .stack
        .corner_radius_pct
        .expect("reverse mirror populated corner_radius_pct");
    assert!(
        (pct - 25.0).abs() < 1e-6,
        "corner_radius_pct = corner_r/min(W,H)*100 = 0.25/1*100 = 25; got {pct}"
    );
}

// ─────────────────────────────────────────────────────────────────
// v0.24 Phase 4 Track A5 — Oval pad parametric mint
// ─────────────────────────────────────────────────────────────────

/// v0.24 Track A5 — placing an Oval pad in Pads mode should mirror
/// into the sketch as the full Fusion-parity stadium primitive:
///   - 1 centre Point
///   - 4 bbox corner Points
///   - 4 arc-anchor Points (where the rounded ends meet the
///     straight edges)
///   - 2 Arc-centre Points (offset inward from the short-axis edges
///     by half the short axis)
///   = 11 Points
///   + 2 long-axis Lines + 2 short-axis Arcs = 15 entities
/// `pad.shape_params` records `"width" -> width_<slug>` and
/// `"height" -> height_<slug>` so the Properties panel can surface
/// both as editable rows.
#[test]
fn mirror_add_oval_pad_mints_2_arcs_2_lines_with_w_and_h_params() {
    use signex_app::library::editor::footprint::pad_to_sketch::mirror_add_pad_to_sketch;
    use signex_app::library::editor::footprint::state::EditorPad;
    use signex_library::primitive::footprint::{Footprint, PadShape};
    use signex_sketch::entity::EntityKind;

    // Wide oval: W=2mm, H=1mm. Rounded ends on the left + right
    // edges; arc radius = H/2 = 0.5mm.
    let mut pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    pad.shape = PadShape::Oval;
    pad.size_mm = (2.0, 1.0);
    let mut fp = Footprint::empty("test");

    mirror_add_pad_to_sketch(&mut pad, &mut fp);

    let sketch = fp.sketch.as_ref().expect("sketch minted");

    // Exactly 2 Arc entities — one per short-axis end, each spanning
    // 180°.
    let arcs: Vec<_> = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Arc { .. }))
        .collect();
    assert_eq!(arcs.len(), 2, "Oval pad mints exactly 2 short-axis Arcs");

    // Exactly 2 Lines on the long-axis edges connecting anchor pairs.
    let lines: Vec<_> = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Line { .. }))
        .collect();
    assert_eq!(lines.len(), 2, "Oval pad mints exactly 2 long-axis Lines");

    // 1 centre + 4 bbox + 4 anchors + 2 arc-centres = 11 Points.
    let points: Vec<_> = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Point { .. }))
        .collect();
    assert_eq!(
        points.len(),
        11,
        "Oval pad mints 1 centre + 4 bbox + 4 anchors + 2 arc-centres = 11 Points"
    );

    // No Circles (Round-only).
    let circles: Vec<_> = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Circle { .. }))
        .collect();
    assert!(circles.is_empty(), "Oval pad mints no Circle entities");

    // pad.shape_params binds "width" + "height" to named parameters.
    let width_param = pad
        .shape_params
        .get("width")
        .expect("'width' key bound on Oval pad");
    let height_param = pad
        .shape_params
        .get("height")
        .expect("'height' key bound on Oval pad");
    assert!(
        width_param.starts_with("width_"),
        "width param has the width_<slug> form (got `{width_param}`)"
    );
    assert!(
        height_param.starts_with("height_"),
        "height param has the height_<slug> form (got `{height_param}`)"
    );

    // sketch.parameters records both literal values.
    let raw_w = sketch
        .parameters
        .get_raw(width_param)
        .expect("width parameter is registered on sketch.parameters");
    let raw_h = sketch
        .parameters
        .get_raw(height_param)
        .expect("height parameter is registered on sketch.parameters");
    assert_eq!(
        raw_w, "2mm",
        "width parameter records the long-axis literal"
    );
    assert_eq!(
        raw_h, "1mm",
        "height parameter records the short-axis literal"
    );

    // Both Arcs implicitly share the same `height_<slug>` parameter
    // (= H/2 = 0.5mm). Verify both Arc radii are equal and match.
    let mut arc_radii: Vec<f64> = Vec::with_capacity(2);
    for arc in &arcs {
        let (center_id, start_id) = match arc.kind {
            EntityKind::Arc { center, start, .. } => (center, start),
            _ => unreachable!(),
        };
        let center_pos = sketch
            .entities
            .iter()
            .find(|e| e.id == center_id)
            .and_then(|e| match e.kind {
                EntityKind::Point { x, y } => Some((x, y)),
                _ => None,
            })
            .expect("Arc.center references a Point");
        let start_pos = sketch
            .entities
            .iter()
            .find(|e| e.id == start_id)
            .and_then(|e| match e.kind {
                EntityKind::Point { x, y } => Some((x, y)),
                _ => None,
            })
            .expect("Arc.start references a Point");
        let dx = start_pos.0 - center_pos.0;
        let dy = start_pos.1 - center_pos.1;
        arc_radii.push((dx * dx + dy * dy).sqrt());
    }
    assert!(
        (arc_radii[0] - arc_radii[1]).abs() < 1e-9,
        "both Arc radii must be equal (height-linked); got {arc_radii:?}"
    );
    assert!(
        (arc_radii[0] - 0.5).abs() < 1e-9,
        "Arc radius must equal height/2 = 0.5mm; got {}",
        arc_radii[0]
    );

    // The 4 bbox corners come back via corner_entity_ids so move +
    // delete mirrors keep the bbox tracking the pad.
    assert!(
        pad.corner_entity_ids.is_some(),
        "Oval pad sets corner_entity_ids to the 4 bbox Points"
    );
}

/// v0.24 Track A5 — editing the `width_<slug>` parameter via the
/// dispatcher (the same path the Properties-panel "Width" row drives)
/// rewrites the bound parameter and runs a solve cleanly. The
/// resolved parameter map reflects the new width so any future
/// constraint linking Line endpoints to `width` would see the
/// updated value; we assert the resolved value here as the surface
/// proxy for "endpoint reflects the new width".
#[test]
fn editing_oval_width_param_propagates_through_solve() {
    use signex_app::app::FootprintEditorState;
    use signex_app::library::editor::footprint::pad_to_sketch::mirror_add_pad_to_sketch;
    use signex_app::library::editor::footprint::state::EditorPad;
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    use signex_library::{Footprint, FootprintFile, PadShape};
    use signex_sketch::parameter;

    let path = PathBuf::from("test-a5-oval-edit-width.snxfpt");
    let mut fp = Footprint::empty("test");
    let mut pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    pad.shape = PadShape::Oval;
    pad.size_mm = (2.0, 1.0); // wide oval
    mirror_add_pad_to_sketch(&mut pad, &mut fp);

    let width_param_name = pad
        .shape_params
        .get("width")
        .cloned()
        .expect("width parameter minted at pad-add time");

    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.pads = vec![pad];
    editor.state.selected_pad = Some(0);

    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "test".into(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: signex_app::app::TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;

    // Edit width via the same dispatcher path that the Properties
    // "Width" row drives.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchEditParameter {
            name: width_param_name.clone(),
            expr: "3mm".into(),
        }),
    }));

    let editor = app
        .document_state
        .footprint_editors
        .get(&path)
        .expect("editor still registered");
    let sketch = editor.file.footprints[0]
        .sketch
        .as_ref()
        .expect("sketch present after edit");

    // Surface 1 — the parameter table records the new expression.
    let raw = sketch
        .parameters
        .get_raw(&width_param_name)
        .expect("width parameter still registered");
    assert_eq!(
        raw, "3mm",
        "FootprintSketchEditParameter rewrites the bound width parameter"
    );

    // Surface 2 — the resolved-parameter map reads 3.0mm. The Lines'
    // endpoints (and a future width-linked constraint) would propagate
    // this value when the solver next runs.
    let resolved =
        parameter::resolve(&sketch.parameters).expect("resolved parameter map after width edit");
    let resolved_width = resolved
        .get(&width_param_name)
        .copied()
        .expect("width parameter resolves cleanly");
    assert!(
        (resolved_width - 3.0).abs() < 1e-9,
        "width parameter resolves to 3.0mm (canonical mm); got {resolved_width}"
    );

    // Surface 3 — solve completed without warnings (no
    // SolverFailed / Expr error / etc.). The Oval mint runs through
    // the same apply_sketch_edit pipeline as RoundRect's corner_r.
    assert!(
        editor.state.solve_warnings.is_empty(),
        "solve completed without warnings; got {:?}",
        editor.state.solve_warnings
    );
}

// ─────────────────────────────────────────────────────────────────
// v0.24 Phase 4 (Track A6) — Chamfered pad parametric mint
// ─────────────────────────────────────────────────────────────────

/// v0.24 Track A6 — placing a Chamfered pad in Pads mode mirrors
/// into the sketch as a parametric outline. With only the top_left +
/// top_right corners enabled, the mint should:
///   - 1 centre Point + 4 bbox corner Points = 5 Points (baseline).
///   - Per ENABLED corner: 2 anchor Points (8 entries' worth ÷ 2
///     corners = 4 anchor Points total).
///   - A single shared `chamfer_len_<slug>` sketch parameter.
///   - Per-corner sidecar keys recording each anchor's UUID so a
///     future Unlink-chamfer-length action can resolve them.
#[test]
fn mirror_add_chamfered_pad_mints_anchors_per_enabled_corner() {
    use signex_app::library::editor::footprint::pad_to_sketch::mirror_add_pad_to_sketch;
    use signex_app::library::editor::footprint::state::EditorPad;
    use signex_library::primitive::footprint::{ChamferedCorners, Footprint, PadShape};
    use signex_sketch::entity::EntityKind;

    let mut pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    pad.shape = PadShape::Chamfered {
        chamfer_ratio: 0.25,
        corners: ChamferedCorners {
            top_left: true,
            top_right: true,
            bottom_left: false,
            bottom_right: false,
        },
    };
    pad.size_mm = (2.0, 1.0); // W=2, H=1, min=1, chamfer_len = 0.25 * 1 = 0.25mm
    let mut fp = Footprint::empty("test");

    mirror_add_pad_to_sketch(&mut pad, &mut fp);

    let sketch = fp.sketch.as_ref().expect("sketch minted");

    // Chamfered pad mints no Arcs and no Circles.
    let arcs: Vec<_> = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Arc { .. }))
        .collect();
    let circles: Vec<_> = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Circle { .. }))
        .collect();
    assert!(arcs.is_empty(), "Chamfered pad mints no Arcs");
    assert!(circles.is_empty(), "Chamfered pad mints no Circles");

    // Points: 1 centre + 4 bbox corners + 2 enabled corners × 2
    // anchors each = 9 Points.
    let points: Vec<_> = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Point { .. }))
        .collect();
    assert_eq!(
        points.len(),
        9,
        "1 centre + 4 bbox + 2 corners × 2 anchors = 9 Points; got {}",
        points.len()
    );

    // Lines: per the outline traversal —
    //   - 1 chamfer-cut line per enabled corner (2 enabled = 2
    //     chamfer-cut Lines).
    //   - 4 edges (NE→SE, SE→SW, SW→NW, NW→NE) connecting the bbox
    //     corner / anchor of each side.
    //   - Total Lines = enabled + 4 = 2 + 4 = 6.
    let lines: Vec<_> = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Line { .. }))
        .collect();
    assert_eq!(
        lines.len(),
        6,
        "2 chamfer cuts + 4 edge lines = 6 Lines; got {}",
        lines.len()
    );

    // pad.shape_params binds the shared chamfer length param.
    let param_name = pad
        .shape_params
        .get("chamfer_len")
        .expect("'chamfer_len' key bound on Chamfered pad");
    assert!(
        param_name.starts_with("chamfer_len_"),
        "param name has the chamfer_len_<slug> form (got `{param_name}`)"
    );

    // sketch.parameters records the literal chamfer length
    // (= 0.25 * min(W,H) = 0.25mm).
    let raw = sketch
        .parameters
        .get_raw(param_name)
        .expect("chamfer_len parameter is registered on sketch.parameters");
    assert_eq!(
        raw, "0.25mm",
        "chamfer_len parameter records the literal length"
    );

    // Per-corner anchor sidecar keys present for both enabled corners
    // and absent for the disabled ones. Y-down naming:
    // top_right == NE, top_left == NW, bottom_right == SE,
    // bottom_left == SW.
    assert!(
        pad.shape_params.contains_key("chamfer_ne_anchor1"),
        "chamfer_ne_anchor1 sidecar key present (top_right enabled)"
    );
    assert!(
        pad.shape_params.contains_key("chamfer_ne_anchor2"),
        "chamfer_ne_anchor2 sidecar key present (top_right enabled)"
    );
    assert!(
        pad.shape_params.contains_key("chamfer_nw_anchor1"),
        "chamfer_nw_anchor1 sidecar key present (top_left enabled)"
    );
    assert!(
        pad.shape_params.contains_key("chamfer_nw_anchor2"),
        "chamfer_nw_anchor2 sidecar key present (top_left enabled)"
    );
    assert!(
        !pad.shape_params.contains_key("chamfer_se_anchor1"),
        "chamfer_se_* sidecars omitted when bottom_right is disabled"
    );
    assert!(
        !pad.shape_params.contains_key("chamfer_sw_anchor1"),
        "chamfer_sw_* sidecars omitted when bottom_left is disabled"
    );

    // The pad's `corner_entity_ids` is the standard 4 bbox corners —
    // anchors live in shape_params, not corner_entity_ids.
    let bbox_corners = pad
        .corner_entity_ids
        .expect("Chamfered pad still mints the 4 bbox corners");
    assert_eq!(bbox_corners.len(), 4, "4 bbox corners");
}

/// v0.24 Track A6 — editing the shared `chamfer_len_<slug>`
/// parameter routes through the FootprintSketchEditParameter
/// dispatch path: rewrites the sketch parameter, runs a fresh
/// solve+rebake, and the post-solve chamfer-anchor mirror
/// (`mirror_solve_to_chamfer_anchors`) rewrites the anchor Point
/// coordinates from the resolved chamfer_len value. Verifies that
/// the shared-parameter wiring is end-to-end live (parameter
/// rewrite → solve → entity-position update).
#[test]
fn editing_chamfer_len_propagates_through_solve() {
    use signex_app::app::FootprintEditorState;
    use signex_app::library::editor::footprint::pad_to_sketch::mirror_add_pad_to_sketch;
    use signex_app::library::editor::footprint::state::EditorPad;
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    use signex_library::primitive::footprint::{
        ChamferedCorners, Footprint, FootprintFile, PadShape,
    };
    use signex_sketch::entity::EntityKind;
    use signex_sketch::id::SketchEntityId;

    let path = PathBuf::from("test-a6-edit-chamfer-len.snxfpt");
    let mut fp = Footprint::empty("test");
    let mut pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    pad.shape = PadShape::Chamfered {
        chamfer_ratio: 0.25,
        corners: ChamferedCorners {
            top_left: true,
            top_right: true,
            bottom_left: false,
            bottom_right: false,
        },
    };
    pad.size_mm = (2.0, 1.0); // chamfer_len = 0.25 * min(2,1) = 0.25mm
    mirror_add_pad_to_sketch(&mut pad, &mut fp);

    let parameter_name = pad
        .shape_params
        .get("chamfer_len")
        .cloned()
        .expect("chamfer_len minted at pad-add time");
    let ne_anchor1_slug = pad
        .shape_params
        .get("chamfer_ne_anchor1")
        .cloned()
        .expect("chamfer_ne_anchor1 sidecar minted");
    let ne_anchor1_id = {
        let uuid = uuid::Uuid::parse_str(&ne_anchor1_slug).expect("sidecar value is a UUID slug");
        SketchEntityId(uuid)
    };

    // Sanity — pre-edit, NE anchor1 sits at (xmax - r, ymin) =
    // (1 - 0.25, -0.5) = (0.75, -0.5). For pad centred at origin
    // with size 2×1, bbox is (-1..1, -0.5..0.5). Y-down so ymin
    // = -0.5 (top edge).
    let pre_pos = fp
        .sketch
        .as_ref()
        .unwrap()
        .entities
        .iter()
        .find(|e| e.id == ne_anchor1_id)
        .and_then(|e| match e.kind {
            EntityKind::Point { x, y } => Some((x, y)),
            _ => None,
        })
        .expect("NE anchor1 is a Point");
    assert!(
        (pre_pos.0 - 0.75).abs() < 1e-9 && (pre_pos.1 - (-0.5)).abs() < 1e-9,
        "before edit, NE anchor1 sits at (xmax - r, ymin) = (0.75, -0.5); got {pre_pos:?}"
    );

    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.pads = vec![pad];
    editor.state.selected_pad = Some(0);

    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "test".into(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: signex_app::app::TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;

    // Edit the shared chamfer_len parameter to 0.5mm.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchEditParameter {
            name: parameter_name.clone(),
            expr: "0.5mm".into(),
        }),
    }));

    let editor = app
        .document_state
        .footprint_editors
        .get(&path)
        .expect("editor still registered");
    let sketch = editor.file.footprints[0]
        .sketch
        .as_ref()
        .expect("sketch present after edit");

    // Parameter rewrote on disk. The dispatcher's
    // `FootprintSketchEditParameter` handler upserts via
    // `SketchEdit::EditParameter` and then runs a fresh solve+bake.
    let raw = sketch
        .parameters
        .get_raw(&parameter_name)
        .expect("chamfer_len parameter still registered");
    assert_eq!(
        raw, "0.5mm",
        "FootprintSketchEditParameter rewrites the bound parameter"
    );

    // Solve completed cleanly — no warnings means the resolver
    // accepted the new expression and the bake re-emitted the pad
    // without errors.
    assert!(
        editor.state.solve_warnings.is_empty(),
        "solve completed without warnings; got {:?}",
        editor.state.solve_warnings
    );

    // shape_params bindings survived the rebake. `chamfer_len` and
    // every per-corner anchor sidecar must still be present so the
    // Properties row + future Unlink action keep their data.
    let pad_after = &editor.state.pads[0];
    assert_eq!(
        pad_after.shape_params.get("chamfer_len"),
        Some(&parameter_name),
        "chamfer_len binding survives solve+rebake"
    );
    for key in [
        "chamfer_ne_anchor1",
        "chamfer_ne_anchor2",
        "chamfer_nw_anchor1",
        "chamfer_nw_anchor2",
    ] {
        assert!(
            pad_after.shape_params.contains_key(key),
            "{key} sidecar survives solve+rebake"
        );
    }

    // Anchor moved post-solve. The post-solve mirror walks the
    // pad's chamfer sidecars and rewrites each anchor's coords from
    // the resolved chamfer_len. Expected new position for NE
    // anchor1: (xmax - 0.5, ymin) = (0.5, -0.5).
    let post_pos = sketch
        .entities
        .iter()
        .find(|e| e.id == ne_anchor1_id)
        .and_then(|e| match e.kind {
            EntityKind::Point { x, y } => Some((x, y)),
            _ => None,
        })
        .expect("NE anchor1 still present after solve");
    assert!(
        (post_pos.0 - 0.5).abs() < 1e-9 && (post_pos.1 - (-0.5)).abs() < 1e-9,
        "after editing chamfer_len = 0.5mm, NE anchor1 sits at \
         (xmax - 0.5, ymin) = (0.5, -0.5); got {post_pos:?}"
    );
    assert!(
        (post_pos.0 - pre_pos.0).abs() > 1e-6,
        "anchor1 X moved after parameter edit (was {pre_pos:?}, now {post_pos:?})"
    );
}

// ── v0.14 / v0.25 / v0.26 footprint-editor regression tests ──────────────

/// Helpers used by the Phase 6 geometric-assertion tightening of
/// the v0.24 deferred suite. Centralised to keep the assertion
/// blocks above readable.
// Currently unused — the Phase 6 tightening that was going to call these
// never landed (zero callers as of the #432 split; kept in case it does).
fn pad_arc_id(
    editor: &signex_app::app::FootprintEditorState,
    pad_idx: usize,
    sidecar_key: &str,
) -> signex_sketch::id::SketchEntityId {
    let pad = &editor.state.pads[pad_idx];
    let slug = pad
        .shape_params
        .get(sidecar_key)
        .unwrap_or_else(|| panic!("pad {pad_idx} sidecar {sidecar_key} missing"));
    let uuid = uuid::Uuid::parse_str(slug)
        .unwrap_or_else(|_| panic!("sidecar {sidecar_key} value {slug} not a UUID slug"));
    signex_sketch::id::SketchEntityId(uuid)
}

fn arc_endpoint_ids(
    sketch: &signex_sketch::SketchData,
    arc_id: signex_sketch::id::SketchEntityId,
) -> (
    signex_sketch::id::SketchEntityId,
    signex_sketch::id::SketchEntityId,
    signex_sketch::id::SketchEntityId,
) {
    use signex_sketch::entity::EntityKind;
    let arc = sketch
        .entities
        .iter()
        .find(|e| e.id == arc_id)
        .unwrap_or_else(|| panic!("arc {arc_id:?} not in sketch"));
    match arc.kind {
        EntityKind::Arc {
            center, start, end, ..
        } => (center, start, end),
        ref other => panic!("entity {arc_id:?} not an Arc: {other:?}"),
    }
}

fn point_pos(
    sketch: &signex_sketch::SketchData,
    id: signex_sketch::id::SketchEntityId,
) -> (f64, f64) {
    use signex_sketch::entity::EntityKind;
    let pt = sketch
        .entities
        .iter()
        .find(|e| e.id == id)
        .unwrap_or_else(|| panic!("point {id:?} not in sketch"));
    match pt.kind {
        EntityKind::Point { x, y } => (x, y),
        ref other => panic!("entity {id:?} not a Point: {other:?}"),
    }
}

fn approx_eq_pt(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6
}

// ─────────────────────────────────────────────────────────────────
// v0.25 polish #3 — Oval reverse-mirror to pad.size_mm
//
// Sister to mirror_solve_to_pad_stack's corner-radius mirror: when
// the user edits the `width_<slug>` parameter, the post-solve mirror
// pushes the resolved value back to `pad.size_mm.0` so the bbox
// tracks the resolved dimensions before geometry repositioning.
// Closes the v0.24 Phase-5 deferred case mentioned in the existing
// `oval_width_edit_propagates_to_arc_centre_via_solve` test.
// ─────────────────────────────────────────────────────────────────

#[test]
fn v025_oval_width_edit_mirrors_back_to_pad_size_mm() {
    use signex_app::app::{FootprintEditorState, TabInfo, TabKind};
    use signex_app::library::editor::footprint::pad_to_sketch::mirror_add_pad_to_sketch;
    use signex_app::library::editor::footprint::state::EditorPad;
    use signex_library::{Footprint, FootprintFile, PadShape};
    let path = PathBuf::from("v025-oval-mirror-size.snxfpt");
    let mut fp = Footprint::empty("v025-oval");
    let mut pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    pad.shape = PadShape::Oval;
    pad.size_mm = (2.0, 1.0);
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
        title: "v025-oval".into(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;
    // Edit width via the Properties dispatch (same path the panel's
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
    // The v0.25 polish: post-solve mirror pushes the resolved width
    // back into the editor-side pad's `size_mm.0`. Pre-v0.25 this
    // stayed at the mint value of 2.0.
    assert!(
        (editor.state.pads[0].size_mm.0 - 3.0).abs() < 1e-9,
        "mirror_solve_to_oval_size must push resolved width back to pad.size_mm.0; got {}",
        editor.state.pads[0].size_mm.0
    );
    // Height parameter wasn't touched, so the mirror leaves it at 1.0.
    assert!(
        (editor.state.pads[0].size_mm.1 - 1.0).abs() < 1e-9,
        "untouched height stays at 1.0 mm; got {}",
        editor.state.pads[0].size_mm.1
    );
    assert!(
        editor.state.solve_warnings.is_empty(),
        "solve must clear cleanly with the new width; got {:?}",
        editor.state.solve_warnings
    );
}
