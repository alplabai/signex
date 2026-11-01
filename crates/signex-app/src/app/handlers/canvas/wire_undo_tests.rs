//! Undo while the Wire tool is mid-drawing (#1831): the anchor the next click
//! draws from must not outlive the segment that put it there.

use signex_types::schematic::{Point, SchematicSheet, Wire};

use super::super::super::*;

/// An empty grid-snapping sheet with the Wire tool armed.
fn app_with_wire_tool() -> Signex {
    let _ = crate::diagnostics::init_logging();
    let path = std::path::PathBuf::from("wire-undo.snxsch");
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.snap_enabled = true;
    app.ui_state.grid_size_mm = 1.27;
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(SchematicSheet::default()).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: "wire-undo".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app.interaction_state.current_tool = Tool::Wire;
    app
}

fn click(app: &mut Signex, x: f64, y: f64) {
    let _ = app.update(Message::CanvasEvent(crate::canvas::CanvasEvent::Clicked {
        world_x: x,
        world_y: y,
    }));
}

fn wires(app: &Signex) -> Vec<Wire> {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
        .wires
        .clone()
}

fn near(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
}

#[test]
fn undo_mid_wire_resets_the_wire_in_progress() {
    let mut app = app_with_wire_tool();
    click(&mut app, 150.0, 60.96);
    click(&mut app, 165.1, 60.96);
    assert_eq!(wires(&app).len(), 1, "one segment committed");

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert!(wires(&app).is_empty(), "undo removed the segment");
    assert!(
        !app.interaction_state.wire_drawing,
        "the wire in progress must not stay anchored at the removed end"
    );
    assert!(app.interaction_state.wire_points.is_empty());
    assert!(
        !app.interaction_state.canvas.drawing_mode
            && app.interaction_state.canvas.wire_preview.is_empty(),
        "the canvas must not keep a rubber band from the removed end"
    );
    assert_eq!(
        app.interaction_state.current_tool,
        Tool::Wire,
        "the tool stays armed"
    );

    // The next click starts a fresh wire rather than drawing an orphan from
    // (165.1, 60.96).
    click(&mut app, 165.1, 76.2);
    assert!(
        wires(&app).is_empty(),
        "no segment from the removed end: {:?}",
        wires(&app)
    );
    let anchor = app.interaction_state.wire_points.as_slice();
    assert!(
        matches!(anchor, [p] if near(*p, Point::new(165.1, 76.2))),
        "the new wire anchors at the click: {anchor:?}"
    );
}
