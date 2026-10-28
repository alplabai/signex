//! #1233 — the Properties Body row switches a placed symbol between the normal
//! and the alternate (De Morgan) body through the engine; a switch that would
//! move a wired pin is refused, leaves the sheet alone and reaches the log.

use signex_app::app::{Message, Signex};
use signex_app::diagnostics;
use signex_app::dock::DockMessage;
use signex_app::panels::PanelMsg;
use signex_types::schematic::{
    BodyStyle, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchematicSheet, Symbol,
    Wire,
};
use std::path::PathBuf;
use uuid::Uuid;

const SYMBOL_ID: Uuid = Uuid::from_u128(0x1233);

fn lib_pin(body_style: u32, x: f64) -> LibPin {
    LibPin {
        unit: 1,
        body_style,
        pin: Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: Point::new(x, 0.0),
            rotation: 0.0,
            length: 2.54,
            name: "1".into(),
            number: "1".into(),
            visible: true,
            name_visible: true,
            number_visible: true,
        },
    }
}

/// U1 at the origin with a wire on world (2.54, 0); the normal body has its
/// pin there, the alternate body's pin sits at `alternate_x`.
fn app_with(alternate_x: f64) -> Signex {
    let mut sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    let lib: LibSymbol = serde_json::from_value(serde_json::json!({ "id": "G" })).unwrap();
    sheet.lib_symbols.insert(
        "G".into(),
        LibSymbol {
            pins: vec![lib_pin(1, 2.54), lib_pin(2, alternate_x)],
            ..lib
        },
    );
    sheet.symbols.push(Symbol {
        uuid: SYMBOL_ID,
        lib_id: "G".into(),
        reference: "U1".into(),
        ..Default::default()
    });
    sheet.wires.push(Wire {
        start: Point::new(2.54, 0.0),
        end: Point::new(10.0, 0.0),
        ..Default::default()
    });
    let path = PathBuf::from("body-style.snxsch");
    let (mut app, _boot) = Signex::new();
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet).expect("engine"),
    );
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "body-style".to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: signex_app::app::TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app
}

fn body(app: &Signex) -> BodyStyle {
    app.document_state
        .active_engine()
        .unwrap()
        .document()
        .symbols[0]
        .body_style
}

fn pick(app: &mut Signex, body_style: BodyStyle) {
    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::SymbolBodyStylePicked(SYMBOL_ID, body_style),
    )));
}

#[test]
fn picking_a_body_switches_it_and_back() {
    let mut app = app_with(2.54);
    pick(&mut app, BodyStyle::Alternate);
    assert_eq!(body(&app), BodyStyle::Alternate);
    pick(&mut app, BodyStyle::Normal);
    assert_eq!(body(&app), BodyStyle::Normal);
}

#[test]
fn a_switch_that_would_move_a_wired_pin_is_refused_and_logged() {
    let _ = diagnostics::init_logging();
    let mut app = app_with(-2.54);
    pick(&mut app, BodyStyle::Alternate);
    assert_eq!(body(&app), BodyStyle::Normal, "the sheet must be untouched");
    assert!(
        diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains("body switch refused for U1")),
        "the refusal must reach the Messages panel's log"
    );
}
