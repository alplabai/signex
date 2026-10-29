//! #171 - dragging a symbol's reference field through `Signex::update`
//! moves the drawn text and its hit box together (both read the stored
//! `TextProp`).

use iced::{Point as ScreenPoint, Rectangle, Size};
use signex_app::app::{Message, Signex, TabInfo, TabKind};
use signex_app::canvas::CanvasEvent;
use signex_app::schematic_runtime::{ScreenTransform, build_renderer_snapshot, hit_test};
use signex_types::schematic::{
    HAlign, Point, SchematicSheet, SelectedItem, SelectedKind, Symbol, TextProp, VAlign,
};
use std::collections::HashMap;
use std::path::PathBuf;

fn symbol(uuid: uuid::Uuid, ref_text: TextProp) -> Symbol {
    Symbol {
        unknown_columns: Default::default(),
        unknown: Default::default(),
        uuid,
        lib_id: "Device:R".to_string(),
        reference: "R1".to_string(),
        value: "10k".to_string(),
        footprint: String::new(),
        datasheet: String::new(),
        position: Point::new(50.0, 50.0),
        rotation: 0.0,
        mirror_x: false,
        mirror_y: false,
        unit: 1,
        body_style: Default::default(),
        is_power: false,
        ref_text: Some(ref_text),
        val_text: None,
        fields_autoplaced: false,
        fields_user_placed: false,
        dnp: false,
        in_bom: true,
        on_board: true,
        exclude_from_sim: false,
        locked: false,
        fields: HashMap::new(),
        custom_properties: Vec::new(),
        pin_uuids: HashMap::new(),
        instances: Vec::new(),
        library_id: None,
        row_id: None,
        library_version: String::new(),
    }
}

fn sheet_with(symbols: Vec<Symbol>) -> SchematicSheet {
    SchematicSheet {
        unknown: Default::default(),
        uuid: uuid::Uuid::new_v4(),
        version: 0,
        generator: String::new(),
        generator_version: String::new(),
        paper_size: "A4".to_string(),
        root_sheet_page: "1".to_string(),
        symbols,
        wires: Vec::new(),
        junctions: Vec::new(),
        labels: Vec::new(),
        child_sheets: Vec::new(),
        no_connects: Vec::new(),
        text_notes: Vec::new(),
        buses: Vec::new(),
        bus_entries: Vec::new(),
        drawings: Vec::new(),
        no_erc_directives: Vec::new(),
        title_block: HashMap::new(),
        lib_symbols: Default::default(),
        net_colors: HashMap::new(),
        drawing_placements: Default::default(),
        frame: Default::default(),
        font_family: None,
    }
}

#[test]
fn dragging_the_reference_field_moves_drawn_text_and_hit_box() {
    let uuid = uuid::Uuid::new_v4();
    let prop = TextProp {
        position: Point::new(70.0, 30.0),
        rotation: 0.0,
        font_size: 2.0,
        justify_h: HAlign::Center,
        justify_v: VAlign::Center,
        hidden: false,
        font_family: None,
    };
    let path = PathBuf::from("field-drag.snxsch");
    let engine = signex_engine::Engine::new(sheet_with(vec![symbol(uuid, prop)])).expect("engine");
    let (mut app, _task) = Signex::new();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "field-drag".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path.clone());
    app.ui_state.snap_enabled = false;
    let field = SelectedItem::new(uuid, SelectedKind::SymbolRefField);
    app.interaction_state.active_canvas_mut().selected = vec![field];

    let _ = app.update(Message::CanvasEvent(CanvasEvent::MoveSelected {
        dx: 10.0,
        dy: 5.0,
        connections: Default::default(),
    }));

    let moved = app.document_state.engines[&path].document();
    let transform = ScreenTransform {
        offset_x: 0.0,
        offset_y: 0.0,
        scale: 1.0,
    };
    let colors = signex_types::theme::canvas_colors(signex_types::theme::ThemeId::CatppuccinMocha);
    let bounds = Rectangle::new(ScreenPoint::ORIGIN, Size::new(800.0, 600.0));
    let snap =
        build_renderer_snapshot(moved, &transform, &colors, bounds, None, Default::default());
    let text = snap
        .reference_value_texts
        .iter()
        .find(|t| t.content == "R1")
        .expect("reference text is drawn");
    assert_eq!(text.position, [80.0, 35.0]);
    assert_eq!(
        hit_test::hit_test(moved, &Default::default(), 80.0, 35.0),
        Some(field)
    );
    assert_ne!(
        hit_test::hit_test(moved, &Default::default(), 70.0, 30.0),
        Some(field)
    );
}

#[test]
fn moved_tall_symbol_is_hit_on_its_library_body() {
    // A 10 x 40 mm IC body: the old fixed 8 x 5.6 mm box missed most of it.
    let uuid = uuid::Uuid::new_v4();
    let prop = TextProp {
        position: Point::new(50.0, 20.0),
        rotation: 0.0,
        font_size: 1.27,
        justify_h: HAlign::Center,
        justify_v: VAlign::Center,
        hidden: true,
        font_family: None,
    };
    let mut sheet = sheet_with(vec![symbol(uuid, prop)]);
    let lib: signex_types::schematic::LibSymbol = serde_json::from_value(serde_json::json!({
        "id": "Device:R",
        "graphics": [{
            "unit": 0,
            "body_style": 1,
            "graphic": {
                "type": "rectangle",
                "start": { "x": -5.0, "y": -20.0 },
                "end": { "x": 5.0, "y": 20.0 }
            }
        }]
    }))
    .expect("lib symbol");
    sheet.lib_symbols.insert(lib.id.clone(), lib);
    let path = PathBuf::from("tall-symbol.snxsch");
    let engine = signex_engine::Engine::new(sheet).expect("engine");
    let (mut app, _task) = Signex::new();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "tall-symbol".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path.clone());
    app.ui_state.snap_enabled = false;
    let body = SelectedItem::new(uuid, SelectedKind::Symbol);
    app.interaction_state.active_canvas_mut().selected = vec![body];

    let _ = app.update(Message::CanvasEvent(CanvasEvent::MoveSelected {
        dx: 10.0,
        dy: 0.0,
        connections: Default::default(),
    }));

    let moved = app.document_state.engines[&path].document();
    // The body now spans x 55..65, y 30..70.
    assert_eq!(
        hit_test::hit_test(moved, &Default::default(), 60.0, 35.0),
        Some(body)
    );
    assert_eq!(
        hit_test::hit_test(moved, &Default::default(), 60.0, 75.0),
        None
    );
    assert_eq!(
        hit_test::hit_test(moved, &Default::default(), 50.0, 50.0),
        None
    );
}
