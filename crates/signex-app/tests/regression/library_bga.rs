//! BGA row/column numbering (skip-letters, start-row, start-col).

use signex_app::app::{Message, Signex};

use std::path::PathBuf;

// ─────────────────────────────────────────────────────────────────
// v0.25 polish — BGA numbering config (PanelMsg path)
//
// The three setters (`FpEditorSetBgaSkipLetters` / `StartRow` /
// `StartCol`) route via `Message::Dock(DockMessage::Panel(_))` and
// resolve the active footprint editor through `tabs[active_tab].kind
// == TabKind::FootprintEditor(path)`. The fixture below stands up a
// minimal sketch with a single Linear array carrying a `BgaRowCol`
// scheme so the field mutates land somewhere the test can read back.
// ─────────────────────────────────────────────────────────────────

/// Build a footprint editor with one Linear array + BgaRowCol numbering,
/// plant it as the active tab, and return the array's id so the test
/// can target it by id (the dispatcher matches arrays by id).
fn fixture_footprint_with_bga_array(stem: &str) -> (Signex, signex_sketch::array::ArrayId) {
    use signex_app::app::{FootprintEditorState, TabInfo, TabKind};
    use signex_library::{Footprint, FootprintFile};
    use signex_sketch::SketchData;
    use signex_sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::id::SketchEntityId;
    use signex_sketch::parameter::ParameterTable;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
    let plane_id = PlaneId::new();
    let pt_id = SketchEntityId::new();
    let pt = Entity::new(pt_id, plane_id, EntityKind::Point { x: 0.0, y: 0.0 });
    let array_id = ArrayId::new();
    let array = Array {
        id: array_id,
        kind: ArrayKind::Linear {
            source: pt_id,
            count_expr: "4".into(),
            dx_expr: "1mm".into(),
            dy_expr: "0mm".into(),
        },
        numbering: NumberingScheme::BgaRowCol {
            skip_letters: true,
            start_row: 'A',
            start_col: 1,
        },
    };
    let mut fp = Footprint::empty(stem);
    fp.sketch = Some(SketchData {
        planes: vec![Plane {
            id: plane_id,
            kind: PlaneKind::BoardTop,
        }],
        entities: vec![pt],
        constraints: Vec::new(),
        arrays: vec![array],
        parameters: ParameterTable::default(),
    });
    let path = PathBuf::from(format!("{stem}.snxfpt"));
    let file = FootprintFile::from_footprint(fp);
    let editor = FootprintEditorState::new(path.clone(), file);
    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: stem.to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;
    (app, array_id)
}

/// Read back the current BgaRowCol triple from the active footprint
/// editor's first array. Panics if the array isn't BgaRowCol — that
/// would indicate the test setup got clobbered.
fn read_bga_config(app: &Signex) -> (bool, char, u32) {
    use signex_sketch::array::NumberingScheme;
    let editor = app
        .document_state
        .tabs
        .first()
        .and_then(|t| match &t.kind {
            signex_app::app::TabKind::FootprintEditor(p) => {
                app.document_state.footprint_editors.get(p)
            }
            _ => None,
        })
        .expect("active tab is a footprint editor");
    let primitive = editor
        .file
        .footprints
        .first()
        .expect("footprint primitive present");
    let sketch = primitive.sketch.as_ref().expect("sketch present");
    let array = sketch.arrays.first().expect("array present");
    match &array.numbering {
        NumberingScheme::BgaRowCol {
            skip_letters,
            start_row,
            start_col,
        } => (*skip_letters, *start_row, *start_col),
        other => panic!("expected BgaRowCol numbering, got {other:?}"),
    }
}

#[test]
fn v025_bga_set_skip_letters_round_trips_bool() {
    use signex_app::dock::DockMessage;
    use signex_app::panels::PanelMsg;
    let (mut app, array_id) = fixture_footprint_with_bga_array("v025-bga-skip");
    assert_eq!(
        read_bga_config(&app).0,
        true,
        "fixture seeds skip_letters=true"
    );
    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::FpEditorSetBgaSkipLetters {
            array_id,
            value: false,
        },
    )));
    assert_eq!(
        read_bga_config(&app).0,
        false,
        "FpEditorSetBgaSkipLetters must round-trip the bool into the array"
    );
}

#[test]
fn v025_bga_set_start_row_uppercases_lowercase_input() {
    use signex_app::dock::DockMessage;
    use signex_app::panels::PanelMsg;
    let (mut app, array_id) = fixture_footprint_with_bga_array("v025-bga-row-lower");
    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::FpEditorSetBgaStartRow {
            array_id,
            value: "h".to_string(),
        },
    )));
    assert_eq!(
        read_bga_config(&app).1,
        'H',
        "lowercase input must be uppercased before storage"
    );
}

#[test]
fn v025_bga_set_start_row_rejects_non_alphabetic_input() {
    use signex_app::dock::DockMessage;
    use signex_app::panels::PanelMsg;
    let (mut app, array_id) = fixture_footprint_with_bga_array("v025-bga-row-digit");
    let before = read_bga_config(&app).1;
    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::FpEditorSetBgaStartRow {
            array_id,
            value: "9".to_string(),
        },
    )));
    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::FpEditorSetBgaStartRow {
            array_id,
            value: "".to_string(),
        },
    )));
    assert_eq!(
        read_bga_config(&app).1,
        before,
        "non-alphabetic / empty inputs must leave start_row unchanged"
    );
}

#[test]
fn v025_bga_set_start_col_parses_valid_integer() {
    use signex_app::dock::DockMessage;
    use signex_app::panels::PanelMsg;
    let (mut app, array_id) = fixture_footprint_with_bga_array("v025-bga-col-ok");
    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::FpEditorSetBgaStartCol {
            array_id,
            value: "  17  ".to_string(),
        },
    )));
    assert_eq!(
        read_bga_config(&app).2,
        17,
        "trimmed valid integer must parse and round-trip into start_col"
    );
}

#[test]
fn v025_bga_set_start_col_rejects_non_numeric_input() {
    use signex_app::dock::DockMessage;
    use signex_app::panels::PanelMsg;
    let (mut app, array_id) = fixture_footprint_with_bga_array("v025-bga-col-bad");
    let before = read_bga_config(&app).2;
    for v in ["", "abc", "-5", "1.5"] {
        let _ = app.update(Message::Dock(DockMessage::Panel(
            PanelMsg::FpEditorSetBgaStartCol {
                array_id,
                value: v.to_string(),
            },
        )));
    }
    assert_eq!(
        read_bga_config(&app).2,
        before,
        "non-numeric / negative / decimal inputs must leave start_col unchanged"
    );
}
