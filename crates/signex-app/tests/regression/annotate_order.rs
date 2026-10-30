//! #961 — the Annotate dialog's "Order of Processing" must drive the
//! numbering. `ui_state.annotate_order` was stored and drawn but never read by
//! the pass, so every choice produced the row-major default.

use signex_app::app::{AnnotateMsg, AnnotateOrder, Message, Signex};
use signex_types::schematic::{Point, SchematicSheet, Symbol};
use std::path::PathBuf;

fn resistor_at(x: f64, y: f64) -> Symbol {
    Symbol {
        uuid: uuid::Uuid::now_v7(),
        lib_id: "Device:R".to_string(),
        reference: "R?".to_string(),
        value: "10k".to_string(),
        position: Point::new(x, y),
        ..Default::default()
    }
}

/// A 2x2 grid in `[top-left, top-right, bottom-left, bottom-right]` order
/// (schematic y grows down), annotated after picking `order` in the dialog.
fn annotate_grid_with(order: AnnotateOrder) -> Vec<String> {
    let path = PathBuf::from("annotate-order.snxsch");
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        symbols: vec![
            resistor_at(0.0, 0.0),
            resistor_at(10.0, 0.0),
            resistor_at(0.0, 10.0),
            resistor_at(10.0, 10.0),
        ],
        ..Default::default()
    };
    let engine = signex_engine::Engine::new(sheet).expect("engine");
    let (mut app, _initial_task) = Signex::new();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "annotate-order".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: signex_app::app::TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);

    let _ = app.update(Message::Annotate(AnnotateMsg::OrderChanged(order)));
    crate::settle_support::settle(
        &mut app,
        Message::Annotate(AnnotateMsg::Run(
            signex_engine::AnnotateMode::ResetAndRenumber,
        )),
    );

    app.document_state
        .active_engine()
        .expect("engine")
        .document()
        .symbols
        .iter()
        .map(|s| s.reference.clone())
        .collect()
}

#[test]
fn the_dialog_default_still_numbers_rows_top_to_bottom() {
    let (app, _t) = Signex::new();
    assert_eq!(app.ui_state.annotate_order, AnnotateOrder::AcrossThenDown);
    assert_eq!(
        annotate_grid_with(AnnotateOrder::AcrossThenDown),
        ["R1", "R2", "R3", "R4"]
    );
}

#[test]
fn across_then_up_picked_in_the_dialog_numbers_the_bottom_row_first() {
    assert_eq!(
        annotate_grid_with(AnnotateOrder::AcrossThenUp),
        ["R3", "R4", "R1", "R2"]
    );
}

#[test]
fn up_then_across_picked_in_the_dialog_numbers_columns_bottom_to_top() {
    assert_eq!(
        annotate_grid_with(AnnotateOrder::UpThenAcross),
        ["R2", "R4", "R1", "R3"]
    );
}

#[test]
fn down_then_across_picked_in_the_dialog_numbers_columns_top_to_bottom() {
    assert_eq!(
        annotate_grid_with(AnnotateOrder::DownThenAcross),
        ["R1", "R3", "R2", "R4"]
    );
}
