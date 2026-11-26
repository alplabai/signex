//! #1988: a computed column in the BOM dialog shows its result with its
//! unit, an empty cell when it fails, and says so in the dialog's words when
//! the parts of a grouped line differ.

use std::collections::BTreeMap;
use std::path::PathBuf;

use signex_output::{BomColumn, BomRow, ComputedCell, ComputedColumn};
use signex_types::schematic::{SchematicSheet, Symbol};

use crate::app::{BomPreviewMsg, Message, Signex};
use crate::layout_probe::lay_out_texts;

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

/// The dialog over one sheet whose `VCC` is 5 V, with the Power column on.
fn app_with_power() -> Signex {
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        symbols: vec![resistor("R1"), resistor("R2")],
        variables: BTreeMap::from([("VCC".to_string(), "5V".to_string())]),
        ..Default::default()
    };
    let path = PathBuf::from("bom-computed-1988.snxsch");
    let engine = signex_engine::Engine::new(sheet).expect("engine");
    let (mut app, _boot) = Signex::new();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(crate::app::TabInfo {
        title: "bom-computed-1988".to_string(),
        path: path.clone(),
        project_id: None,
        kind: crate::app::TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    let _ = app.handle_bom_preview_open();
    let _ = app.update(Message::BomPreview(BomPreviewMsg::ToggleColumn(power())));
    app
}

fn shown(app: &Signex) -> Vec<String> {
    lay_out_texts(app.view_bom_preview_body(), iced::Size::new(1000.0, 700.0))
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

#[test]
fn the_dialog_shows_a_computed_column_with_its_header_and_unit() {
    let app = app_with_power();
    let texts = shown(&app);
    assert!(texts.iter().any(|t| t == "Power"), "{texts:#?}");
    assert!(texts.iter().any(|t| t == "2.5mW"), "{texts:#?}");
}

#[test]
fn a_line_whose_parts_differ_reads_in_the_dialogs_words() {
    let mut app = app_with_power();
    let preview = app.document_state.bom_preview.as_mut().unwrap();
    preview.table.rows[0]
        .computed
        .insert("Power".into(), ComputedCell::Differs);
    let texts = shown(&app);
    assert!(texts.iter().any(|t| t == "Members differ"), "{texts:#?}");
    assert!(
        !texts
            .iter()
            .any(|t| t == signex_output::bom::MEMBERS_DIFFER),
        "the export's words stay out of the dialog: {texts:#?}"
    );
}

#[test]
fn parts_differ_reads_only_a_computed_cell_that_differs() {
    let mut row = BomRow::default();
    assert!(!super::table::parts_differ(&power(), &row));
    row.computed.insert("Power".into(), ComputedCell::Differs);
    assert!(super::table::parts_differ(&power(), &row));
    assert!(!super::table::parts_differ(&BomColumn::Value, &row));
    row.computed
        .insert("Power".into(), ComputedCell::Value { text: "1W".into() });
    assert!(!super::table::parts_differ(&power(), &row));
}
