//! #1988: a computed column ticked in the BOM dialog is evaluated at once,
//! and each failed cell is one Messages line naming its line.

use std::collections::BTreeMap;
use std::path::PathBuf;

use signex_output::{BomColumn, ComputedCell, ComputedColumn};
use signex_types::schematic::{SchematicSheet, Symbol};

use crate::app::{BomPreviewMsg, Message, Signex};

fn resistor(reference: &str) -> Symbol {
    Symbol {
        uuid: uuid::Uuid::now_v7(),
        lib_id: "Device:R".to_string(),
        reference: reference.to_string(),
        value: "10kΩ".to_string(),
        footprint: "R_0603".to_string(),
        ..Default::default()
    }
}

fn power() -> BomColumn {
    BomColumn::Computed(ComputedColumn::new("Power", "= VCC^2 / Value").unwrap())
}

/// The dialog over one sheet holding RB61 and RB62, with its own `VCC` when
/// `vcc` is given.
fn app_with_dialog(vcc: Option<&str>) -> Signex {
    let _ = crate::diagnostics::init_logging();
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        symbols: vec![resistor("RB61"), resistor("RB62")],
        variables: vcc
            .map(|v| BTreeMap::from([("VCC".to_string(), v.to_string())]))
            .unwrap_or_default(),
        ..Default::default()
    };
    let path = PathBuf::from("bom-computed-b6.snxsch");
    let engine = signex_engine::Engine::new(sheet).expect("engine");
    let (mut app, _boot) = Signex::new();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(crate::app::TabInfo {
        title: "bom-computed-b6".to_string(),
        path: path.clone(),
        project_id: None,
        kind: crate::app::TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    let _ = app.handle_bom_preview_open();
    app
}

fn power_cell(app: &Signex) -> Option<ComputedCell> {
    let preview = app
        .document_state
        .bom_preview
        .as_ref()
        .expect("preview open");
    preview.table.rows[0].computed.get("Power").cloned()
}

#[test]
fn ticking_a_computed_column_evaluates_it() {
    let mut app = app_with_dialog(Some("5V"));
    assert_eq!(power_cell(&app), None);
    let _ = app.update(Message::BomPreview(BomPreviewMsg::ToggleColumn(power())));
    assert_eq!(
        power_cell(&app),
        Some(ComputedCell::Value {
            text: "2.5mW".into()
        })
    );
    let preview = app.document_state.bom_preview.as_ref().unwrap();
    assert_eq!(preview.options.columns.last(), Some(&power()));
    // Untick: the column leaves the shown and exported ones.
    let _ = app.update(Message::BomPreview(BomPreviewMsg::ToggleColumn(power())));
    let preview = app.document_state.bom_preview.as_ref().unwrap();
    assert!(!preview.options.columns.contains(&power()));
}

#[test]
fn a_cell_that_fails_is_one_messages_line_naming_its_line() {
    let needle = "BOM line 1 (RB61, RB62): column Power for RB61: unknown parameter 'VCC'";
    let before = crate::diagnostics::logged_count(needle);
    let mut app = app_with_dialog(None);
    let _ = app.update(Message::BomPreview(BomPreviewMsg::ToggleColumn(power())));
    assert!(matches!(power_cell(&app), Some(ComputedCell::Error { .. })));
    assert_eq!(crate::diagnostics::logged_count(needle), before + 1);
}
