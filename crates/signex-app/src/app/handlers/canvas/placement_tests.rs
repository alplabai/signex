//! What a canvas click places, per tool (#1930): pinned before the placement
//! commits moved out of `handle_canvas_clicked`, and the reference the
//! plain-value `place_*_at` methods are compared against.

use signex_types::schematic::{
    FillType, HAlign, LabelType, Point, SCHEMATIC_PT_TO_MM, SCHEMATIC_TEXT_MM, SchDrawing,
    SchematicSheet, VAlign,
};

use super::super::super::*;
use crate::panels::{PrePlacementData, PrePlacementKind};

/// An off-grid click; it snaps to grid point (8, 16).
const OFF_GRID: (f64, f64) = (10.3, 20.2);

/// Grid point (`i`, `j`) of the 1.27 mm grid, as the snap computes it.
fn grid(i: f64, j: f64) -> Point {
    let g = crate::canvas::grid::pitch_mm_exact(1.27);
    Point::new(i * g, j * g)
}

/// An empty grid-snapping sheet with `tool` armed.
fn app_with_tool(tool: Tool) -> Signex {
    let path = std::path::PathBuf::from("placement.snxsch");
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.snap_enabled = true;
    app.ui_state.grid_size_mm = 1.27;
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(SchematicSheet::default()).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: "placement".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app.interaction_state.current_tool = tool;
    app
}

fn click(app: &mut Signex, (x, y): (f64, f64)) {
    let _task = app.update(Message::CanvasEvent(crate::canvas::CanvasEvent::Clicked {
        world_x: x,
        world_y: y,
    }));
}

fn doc(app: &Signex) -> &SchematicSheet {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
}

/// The sheet as a value with every uuid removed, so placements made in two
/// apps compare equal when they placed the same things.
fn sheet_shape(app: &Signex) -> serde_json::Value {
    fn strip(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                map.retain(|key, _| !key.ends_with("uuid") && !key.ends_with("uuids"));
                map.values_mut().for_each(strip);
            }
            serde_json::Value::Array(items) => items.iter_mut().for_each(strip),
            _ => {}
        }
    }
    let mut value = serde_json::to_value(doc(app)).expect("sheet serialises");
    strip(&mut value);
    value
}

/// Two apps set up by `setup`: one placed by a click at `OFF_GRID`, the other
/// by `place` with the snapped point. Their sheets must match.
fn assert_method_matches_click(
    tool: Tool,
    setup: impl Fn(&mut Signex),
    place: impl FnOnce(&mut Signex, Point),
) {
    let mut clicked = app_with_tool(tool);
    setup(&mut clicked);
    click(&mut clicked, OFF_GRID);
    let mut direct = app_with_tool(tool);
    setup(&mut direct);
    place(&mut direct, on_grid());
    assert_ne!(
        sheet_shape(&direct),
        sheet_shape(&app_with_tool(tool)),
        "placed nothing"
    );
    assert_eq!(sheet_shape(&direct), sheet_shape(&clicked));
}

fn on_grid() -> Point {
    grid(8.0, 16.0)
}

fn form(kind: PrePlacementKind, text: &str) -> PrePlacementData {
    PrePlacementData::new("tool".into(), kind, text.into(), 0.0, 0.0)
}

#[test]
fn a_label_click_places_a_net_label_with_the_defaults() {
    let mut app = app_with_tool(Tool::Label);
    click(&mut app, OFF_GRID);
    let label = &doc(&app).labels[0];
    assert_eq!(label.label_type, LabelType::Net);
    assert_eq!(label.text, "NET");
    assert_eq!(label.position, on_grid());
    assert_eq!(label.rotation, 0.0);
    assert_eq!(label.font_size, SCHEMATIC_TEXT_MM);
    assert_eq!(
        (label.justify, label.justify_v),
        (HAlign::Left, VAlign::Bottom)
    );
}

#[test]
fn a_pending_port_kind_picks_the_label_type_and_its_default_text() {
    for (kind, shape, text) in [
        (LabelType::Global, "input", "PORT"),
        (LabelType::Hierarchical, "output", "SHEET"),
        (LabelType::Net, "", "NET"),
    ] {
        let mut app = app_with_tool(Tool::Label);
        app.interaction_state.pending_port = Some((kind, shape.to_string()));
        click(&mut app, OFF_GRID);
        let label = &doc(&app).labels[0];
        assert_eq!((label.label_type, label.shape.as_str()), (kind, shape));
        assert_eq!(label.text, text);
    }
}

#[test]
fn the_pre_placement_form_sets_text_rotation_size_and_justification() {
    let mut app = app_with_tool(Tool::Label);
    let mut pre = form(PrePlacementKind::NetLabel, " D0 ");
    pre.rotation = 90.0;
    pre.font_size_pt = 12;
    pre.justify_h = HAlign::Right;
    pre.justify_v = VAlign::Top;
    app.document_state.panel_ctx.pre_placement = Some(pre);
    click(&mut app, OFF_GRID);
    let label = &doc(&app).labels[0];
    assert_eq!(label.text, "D0");
    assert_eq!(label.rotation, 90.0);
    assert_eq!(label.font_size, 12.0 * SCHEMATIC_PT_TO_MM);
    assert_eq!(
        (label.justify, label.justify_v),
        (HAlign::Right, VAlign::Top)
    );
    // The repeat bookkeeping steps the form's text for the next click.
    let next = app.document_state.panel_ctx.pre_placement.as_ref().unwrap();
    assert_eq!(next.label_text, "D1");
}

#[test]
fn a_text_click_places_the_form_text_and_returns_to_select() {
    let mut app = app_with_tool(Tool::Text);
    app.document_state.panel_ctx.pre_placement = Some(form(PrePlacementKind::TextNote, "Note"));
    click(&mut app, OFF_GRID);
    let note = &doc(&app).text_notes[0];
    assert_eq!((note.text.as_str(), note.position), ("Note", on_grid()));
    assert_eq!(app.interaction_state.current_tool, Tool::Select);
}

#[test]
fn a_power_click_places_the_pending_power_port() {
    let mut app = app_with_tool(Tool::Component);
    app.interaction_state.pending_power = Some(("GND".into(), "power:GND".into()));
    click(&mut app, OFF_GRID);
    let symbol = doc(&app)
        .symbols
        .iter()
        .find(|s| s.is_power)
        .expect("a power port");
    assert_eq!(
        (symbol.value.as_str(), symbol.lib_id.as_str()),
        ("GND", "power:GND")
    );
    assert_eq!(symbol.position, on_grid());
}

#[test]
fn point_tools_place_at_the_snapped_click() {
    let mut app = app_with_tool(Tool::NoConnect);
    click(&mut app, OFF_GRID);
    assert_eq!(doc(&app).no_connects[0].position, on_grid());

    let mut app = app_with_tool(Tool::Junction);
    click(&mut app, OFF_GRID);
    click(&mut app, OFF_GRID);
    assert_eq!(
        doc(&app).junctions.len(),
        1,
        "a second dot on the point is not stacked"
    );
    assert_eq!(doc(&app).junctions[0].position, on_grid());

    let mut app = app_with_tool(Tool::NoErcDirective);
    click(&mut app, OFF_GRID);
    assert_eq!(doc(&app).no_erc_directives[0].position, on_grid());
}

#[test]
fn wire_and_bus_tools_place_a_segment_on_the_second_click() {
    let mut app = app_with_tool(Tool::Wire);
    click(&mut app, OFF_GRID);
    click(&mut app, (30.4, 20.2));
    let wire = &doc(&app).wires[0];
    assert_eq!((wire.start, wire.end), (on_grid(), grid(24.0, 16.0)));

    let mut app = app_with_tool(Tool::Bus);
    click(&mut app, OFF_GRID);
    click(&mut app, (30.4, 20.2));
    let bus = &doc(&app).buses[0];
    assert_eq!((bus.start, bus.end), (on_grid(), grid(24.0, 16.0)));

    let mut app = app_with_tool(Tool::BusEntry);
    click(&mut app, OFF_GRID);
    let entry = &doc(&app).bus_entries[0];
    assert_eq!(entry.position, on_grid());
    assert_eq!(entry.size, app.interaction_state.bus_entry_size);
}

#[test]
fn shape_tools_place_a_drawing_with_the_form_width() {
    let mut app = app_with_tool(Tool::Line);
    click(&mut app, OFF_GRID);
    click(&mut app, (30.4, 20.2));
    match &doc(&app).drawings[0] {
        SchDrawing::Line {
            start, end, width, ..
        } => {
            assert_eq!((*start, *end, *width), (on_grid(), grid(24.0, 16.0), 0.15));
        }
        other => panic!("{other:?}"),
    }

    let mut app = app_with_tool(Tool::Rectangle);
    let mut pre = form(PrePlacementKind::Rectangle, "");
    pre.shape_width_mm = 0.4;
    pre.shape_fill = FillType::Outline;
    app.document_state.panel_ctx.pre_placement = Some(pre);
    click(&mut app, OFF_GRID);
    click(&mut app, (30.4, 30.4));
    match &doc(&app).drawings[0] {
        SchDrawing::Rect {
            start,
            end,
            width,
            fill,
            ..
        } => {
            assert_eq!((*start, *end), (on_grid(), grid(24.0, 24.0)));
            assert_eq!((*width, *fill), (0.4, FillType::Outline));
        }
        other => panic!("{other:?}"),
    }

    let mut app = app_with_tool(Tool::Circle);
    click(&mut app, OFF_GRID);
    click(&mut app, (15.2, 20.2));
    match &doc(&app).drawings[0] {
        SchDrawing::Circle { center, radius, .. } => {
            assert_eq!(*center, on_grid());
            assert!((radius - grid(4.0, 0.0).x).abs() < 1e-9, "{radius}");
        }
        other => panic!("{other:?}"),
    }

    let mut app = app_with_tool(Tool::Arc);
    for at in [OFF_GRID, (15.2, 25.3), (20.3, 20.2)] {
        click(&mut app, at);
    }
    match &doc(&app).drawings[0] {
        SchDrawing::Arc {
            start, mid, end, ..
        } => {
            assert_eq!(*start, on_grid());
            assert_eq!(*mid, grid(12.0, 20.0));
            assert_eq!(*end, grid(16.0, 16.0));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn place_label_at_matches_a_label_click() {
    let setup = |app: &mut Signex| {
        app.interaction_state.pending_port = Some((LabelType::Global, "input".into()));
        let mut pre = form(PrePlacementKind::GlobalPort, "CLK");
        pre.rotation = 180.0;
        app.document_state.panel_ctx.pre_placement = Some(pre);
    };
    assert_method_matches_click(Tool::Label, setup, |app, at| {
        let spec = app.label_spec_from_form();
        assert!(app.place_label_at(at, spec));
    });
}

#[test]
fn place_text_at_matches_a_text_click() {
    let setup = |app: &mut Signex| {
        app.document_state.panel_ctx.pre_placement = Some(form(PrePlacementKind::TextNote, "Note"));
    };
    assert_method_matches_click(Tool::Text, setup, |app, at| {
        assert!(app.place_text_at(at, "Note".into()));
    });
}
