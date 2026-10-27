//! #1246 — with the electrical-grid toggle (Shift+E) on, a wire click within
//! the hotspot radius of an off-grid pin end or wire end lands exactly on it;
//! with the toggle off the click snaps to the grid as before.

use signex_app::app::{Message, Signex, TabInfo, TabKind, Tool};
use signex_app::canvas::CanvasEvent;
use signex_app::schematic_runtime::SchematicRenderCache;
use signex_app::toolbar::ToolMessage;
use signex_types::schematic::{
    LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchematicSheet, Symbol, Wire,
};

/// The pin tip, 0.3 mm off the 1.27 mm grid in x and 0.1 mm in y.
const PIN_TIP: (f64, f64) = (12.7 + 0.3, 6.35 + 0.1);

fn one_pin_part() -> LibSymbol {
    LibSymbol {
        id: "R".to_string(),
        reference: String::new(),
        value: String::new(),
        footprint: String::new(),
        datasheet: String::new(),
        description: String::new(),
        keywords: String::new(),
        fp_filters: String::new(),
        in_bom: true,
        on_board: true,
        in_pos_files: true,
        duplicate_pin_numbers_are_jumpers: false,
        graphics: Vec::new(),
        pins: vec![LibPin {
            unit: 0,
            body_style: 1,
            pin: Pin {
                direction: PinDirection::Passive,
                shape_style: PinShapeStyle::Plain,
                position: Point::new(0.0, 0.0),
                rotation: 0.0,
                length: 0.0,
                name: String::new(),
                number: "1".to_string(),
                visible: true,
                name_visible: true,
                number_visible: true,
            },
        }],
        show_pin_numbers: true,
        show_pin_names: true,
        pin_name_offset: 0.0,
    }
}

fn app_with_off_grid_pin_and_wire(hotspots: bool) -> (Signex, std::path::PathBuf) {
    let mut sheet = SchematicSheet::default();
    sheet.lib_symbols.insert("R".to_string(), one_pin_part());
    sheet.symbols.push(Symbol {
        uuid: uuid::Uuid::new_v4(),
        lib_id: "R".to_string(),
        position: Point::new(PIN_TIP.0, PIN_TIP.1),
        ..Default::default()
    });
    let path = std::path::PathBuf::from("hotspot-snap.snxsch");
    let engine = signex_engine::Engine::new(sheet.clone()).expect("engine");
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.snap_enabled = true;
    app.ui_state.grid_size_mm = 1.27;
    app.ui_state.snap_hotspots = hotspots;
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "hotspot-snap".to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path.clone());
    app.interaction_state
        .active_canvas_mut()
        .set_render_cache(Some(SchematicRenderCache::from_sheet(&sheet)));
    let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Wire)));
    (app, path)
}

fn click(app: &mut Signex, x: f64, y: f64) {
    let _ = app.update(Message::CanvasEvent(CanvasEvent::Clicked {
        world_x: x,
        world_y: y,
    }));
}

/// Draw one wire from a grid point to a click near the pin and return it.
fn draw_wire_to_pin(app: &mut Signex, path: &std::path::PathBuf) -> Wire {
    click(app, 0.0, 6.35);
    click(app, PIN_TIP.0 + 0.4, PIN_TIP.1 + 0.3);
    app.document_state.engines[path]
        .document()
        .wires
        .last()
        .expect("a wire was placed")
        .clone()
}

#[test]
fn wire_click_near_an_off_grid_pin_lands_exactly_on_the_pin_end() {
    let (mut app, path) = app_with_off_grid_pin_and_wire(true);
    let wire = draw_wire_to_pin(&mut app, &path);
    assert_eq!((wire.end.x, wire.end.y), PIN_TIP);
}

#[test]
fn toggle_off_snaps_the_same_click_to_the_grid() {
    let (mut app, path) = app_with_off_grid_pin_and_wire(false);
    let wire = draw_wire_to_pin(&mut app, &path);
    assert_ne!((wire.end.x, wire.end.y), PIN_TIP);
    let g = 1.27;
    assert!(((wire.end.x / g).round() * g - wire.end.x).abs() < 1e-9);
}
