//! #1061 — a net named differently on two sheets is reported by an ERC run
//! driven through `Message::Erc`, filed under the sheet of the winning name.
//!
//! Real temp dir: `root.snxsch` (open, active) puts a Global `I2C_SDA` on the
//! wire of sheet pin `SDA`; `child.snxsch` (on disk only) calls its side
//! Hierarchical `SDA`. Each sheet alone carries one name.

use signex_app::app::{ErcMsg, Message, Signex};
use signex_types::schematic::{
    ChildSheet, FillType, HAlign, Label, LabelType, Point, SchematicSheet, SheetPin, VAlign, Wire,
};
use tempfile::TempDir;

fn named_wire(text: &str, label_type: LabelType) -> SchematicSheet {
    let mut sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    sheet.wires.push(Wire {
        start: Point::new(0.0, 0.0),
        end: Point::new(10.0, 0.0),
        ..Default::default()
    });
    sheet.labels.push(Label {
        unknown_columns: Default::default(),
        uuid: uuid::Uuid::new_v4(),
        text: text.to_string(),
        position: Point::new(0.0, 0.0),
        rotation: 0.0,
        label_type,
        shape: String::new(),
        font_size: 1.27,
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
    });
    sheet
}

fn write_sheet(path: &std::path::Path, sheet: SchematicSheet) {
    let text = signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .expect("serialise sheet");
    std::fs::write(path, text).expect("write sheet");
}

#[test]
fn an_erc_run_reports_a_net_named_differently_on_two_sheets() {
    let tmp = TempDir::new().expect("tempdir");
    let root_path = tmp.path().join("root.snxsch");
    write_sheet(
        &tmp.path().join("child.snxsch"),
        named_wire("SDA", LabelType::Hierarchical),
    );
    let mut root = named_wire("I2C_SDA", LabelType::Global);
    root.child_sheets.push(ChildSheet {
        uuid: uuid::Uuid::new_v4(),
        name: "charger".to_string(),
        filename: "child.snxsch".to_string(),
        position: Point::new(0.0, 0.0),
        size: (10.0, 10.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: vec![SheetPin {
            uuid: uuid::Uuid::new_v4(),
            name: "SDA".to_string(),
            direction: String::new(),
            position: Point::new(0.0, 0.0),
            rotation: 0.0,
            auto_generated: false,
            user_moved: false,
        }],
        instances: Vec::new(),
    });
    write_sheet(&root_path, root.clone());

    let (mut app, _task) = Signex::new();
    let engine =
        signex_engine::Engine::new_with_path(root, Some(root_path.clone())).expect("engine");
    app.document_state.engines.insert(root_path.clone(), engine);
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "root".to_string(),
        path: root_path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: signex_app::app::TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(root_path.clone());

    crate::settle_support::settle(&mut app, Message::Erc(ErcMsg::Run));

    let found: Vec<&signex_erc::Violation> = app.ui_state.erc_violations_by_path[&root_path]
        .iter()
        .filter(|v| v.rule == signex_erc::RuleKind::NetMultipleNames)
        .collect();
    assert_eq!(found.len(), 1, "{found:?}");
    let m = &found[0].message;
    assert!(m.contains("'I2C_SDA' (global) on sheet"), "{m}");
    assert!(m.contains("'SDA' (hierarchical) on sheet charger"), "{m}");
}
