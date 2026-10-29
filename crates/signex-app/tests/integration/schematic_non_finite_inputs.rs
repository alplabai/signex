//! #678 — a schematic property input must refuse `nan` / `inf`: they parse
//! under `f64::from_str`, and `inf >= 0.0` even passes the "non-negative
//! stroke width" check, so the value used to land in the document.

use signex_app::app::{Message, Signex, TabInfo, TabKind};
use signex_app::dock::DockMessage;
use signex_app::panels::PanelMsg;
use signex_types::schematic::{ChildSheet, FillType, Point, SchematicSheet};
use std::path::PathBuf;

const STARTING_WIDTH: f64 = 0.12;

fn fixture() -> (Signex, PathBuf, uuid::Uuid) {
    let sheet_uuid = uuid::Uuid::new_v4();
    let sheet = SchematicSheet {
        child_sheets: vec![ChildSheet {
            uuid: sheet_uuid,
            name: "Power".to_string(),
            filename: "power.snxsch".to_string(),
            position: Point::new(20.0, 20.0),
            size: (30.0, 30.0),
            stroke_width: STARTING_WIDTH,
            fill: FillType::None,
            stroke_color: None,
            fill_color: None,
            fields_autoplaced: false,
            pins: Vec::new(),
            instances: Vec::new(),
        }],
        ..Default::default()
    };
    let path = PathBuf::from("non-finite-inputs.snxsch");
    let engine = signex_engine::Engine::new(sheet).expect("engine");
    let (mut app, _initial_task) = Signex::new();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "non-finite-inputs".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path.clone());
    (app, path, sheet_uuid)
}

fn panel(app: &mut Signex, msg: PanelMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
}

fn stroke_width(app: &Signex, path: &PathBuf) -> f64 {
    app.document_state.engines[path].document().child_sheets[0].stroke_width
}

#[test]
fn child_sheet_stroke_width_refuses_non_finite_input() {
    for bad in ["inf", "infinity", "+inf", "nan", "NaN", "1e999"] {
        let (mut app, path, id) = fixture();
        panel(
            &mut app,
            PanelMsg::ChildSheetStrokeWidthTyping(id, bad.into()),
        );
        panel(&mut app, PanelMsg::CommitChildSheetStrokeWidth(id));
        assert_eq!(
            stroke_width(&app, &path),
            STARTING_WIDTH,
            "{bad:?} reached the stroke width"
        );
    }
}

#[test]
fn child_sheet_stroke_width_still_accepts_a_finite_value() {
    let (mut app, path, id) = fixture();
    panel(
        &mut app,
        PanelMsg::ChildSheetStrokeWidthTyping(id, "0.5".into()),
    );
    panel(&mut app, PanelMsg::CommitChildSheetStrokeWidth(id));
    assert_eq!(stroke_width(&app, &path), 0.5);
}
