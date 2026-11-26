//! #2953: which Object List cells edit which stored text, and the cell
//! editor as the panel draws it and as a double click opens it.

use std::path::PathBuf;
use std::sync::Arc;

use signex_engine::TextTarget;
use uuid::Uuid;

use super::super::model::{
    ObjectKind, ObjectListState, ObjectListStatus, ObjectListView, SheetLook, sheet_rows,
};
use super::super::model_tests::power_sheet;
use super::super::{ObjectListMsg, PanelContext, PanelMsg, view};
use super::*;
use crate::layout_probe::{lay_out_inputs, lay_out_texts, messages_on_clicks};

const WINDOW: iced::Size = iced::Size::new(900.0, 2400.0);

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn power_path() -> PathBuf {
    PathBuf::from("proj").join("Power.snxsch")
}

fn rows() -> Vec<ObjectRow> {
    sheet_rows(
        &power_sheet(),
        &power_path(),
        &SheetLook::default(),
        &|_: Uuid| None,
    )
}

fn row(kind: ObjectKind, n: u128) -> ObjectRow {
    rows()
        .into_iter()
        .find(|r| r.kind == kind && r.target.item.uuid == id(n))
        .expect("row")
}

/// The panel listing the Power sheet, the sheet in view, with `editing`.
fn context(editing: Option<CellEditing>) -> PanelContext {
    let mut state = ObjectListState::default();
    state.source.rows = Arc::new(rows());
    state.editing = editing;
    let (app, _boot) = crate::app::Signex::new();
    let mut ctx = app.document_state.panel_ctx.clone();
    ctx.object_list = ObjectListView::build(
        &state,
        &state.source,
        ObjectListStatus::Ready,
        Some(power_path()),
        &[],
        signex_i18n::I18n::en_us_ref(),
    );
    ctx
}

#[test]
fn each_kind_edits_the_text_it_shows() {
    let part = row(ObjectKind::Part, 1);
    assert_eq!(
        editable_text(&part, ObjectColumn::Name),
        Some(TextTarget::SymbolReference(id(1)))
    );
    assert_eq!(
        editable_text(&part, ObjectColumn::Value),
        Some(TextTarget::SymbolValue(id(1)))
    );
    // A power port symbol's name is its value.
    let port = row(ObjectKind::PowerPort, 3);
    assert_eq!(
        editable_text(&port, ObjectColumn::Name),
        Some(TextTarget::SymbolValue(id(3)))
    );
    assert_eq!(editable_text(&port, ObjectColumn::Value), None);
    for (kind, n) in [(ObjectKind::Label, 4), (ObjectKind::Port, 5)] {
        let label = row(kind, n);
        assert_eq!(
            editable_text(&label, ObjectColumn::Name),
            Some(TextTarget::Label(id(n)))
        );
        assert_eq!(editable_text(&label, ObjectColumn::Value), None);
    }
    let note = row(ObjectKind::Text, 9);
    assert_eq!(
        editable_text(&note, ObjectColumn::Value),
        Some(TextTarget::TextNote(id(9)))
    );
    assert_eq!(editable_text(&note, ObjectColumn::Name), None);
    // No single text: a sheet symbol, an entry, a wire, a junction.
    for (kind, n) in [
        (ObjectKind::SheetSymbol, 6),
        (ObjectKind::SheetEntry, 7),
        (ObjectKind::Wire, 8),
        (ObjectKind::Junction, 10),
    ] {
        let r = row(kind, n);
        for column in [ObjectColumn::Name, ObjectColumn::Value] {
            assert_eq!(editable_text(&r, column), None, "{kind:?} {column:?}");
        }
    }
}

#[test]
fn an_editor_is_the_cell_of_its_object_and_column_only() {
    let part = row(ObjectKind::Part, 1);
    let editing = CellEditing {
        target: part.target.clone(),
        column: ObjectColumn::Value,
        text: TextTarget::SymbolValue(id(1)),
        original: "INA226".into(),
        draft: "INA2".into(),
    };
    assert!(editing.is_cell(&part, ObjectColumn::Value));
    assert!(!editing.is_cell(&part, ObjectColumn::Name));
    assert!(!editing.is_cell(&row(ObjectKind::Part, 2), ObjectColumn::Value));
}

/// While a cell is open its text box holds the draft in place of the value.
#[test]
fn an_open_cell_draws_its_draft_in_a_text_box() {
    let part = row(ObjectKind::Part, 1);
    let ctx = context(Some(CellEditing {
        target: part.target.clone(),
        column: ObjectColumn::Value,
        text: TextTarget::SymbolValue(id(1)),
        original: "INA226".into(),
        draft: "INA2".into(),
    }));
    let inputs: Vec<String> = lay_out_inputs(view(&ctx), WINDOW)
        .into_iter()
        .map(|t| t.text)
        .collect();
    assert!(inputs.iter().any(|t| t == "INA2"), "{inputs:?}");
    let texts: Vec<String> = lay_out_texts(view(&ctx), WINDOW)
        .into_iter()
        .map(|t| t.text)
        .collect();
    assert!(!texts.iter().any(|t| t == "INA226"), "{texts:?}");
    assert!(
        texts.iter().any(|t| t == "U3"),
        "the Name cell stays: {texts:?}"
    );
}

/// A double click on an editable cell opens it; on a cell with no single
/// text it zooms to the object, as before.
#[test]
fn a_double_click_opens_an_editable_cell_and_zooms_from_any_other() {
    let ctx = context(None);
    let pressed = messages_on_clicks(view(&ctx), WINDOW, &["INA226", "INA226"]);
    let opened = pressed.iter().find_map(|m| match m {
        PanelMsg::ObjectList(ObjectListMsg::CellEdit(CellEditMsg::Opened { target, column })) => {
            Some((target.item.uuid, *column))
        }
        _ => None,
    });
    assert_eq!(opened, Some((id(1), ObjectColumn::Value)), "{pressed:?}");
    assert!(
        pressed
            .iter()
            .all(|m| !matches!(m, PanelMsg::ObjectList(ObjectListMsg::RowDoubleClicked(_)))),
        "{pressed:?}"
    );
    // A click still selects the row.
    assert!(
        matches!(
            pressed.first(),
            Some(PanelMsg::ObjectList(ObjectListMsg::RowClicked(_)))
        ),
        "{pressed:?}"
    );

    let pressed = messages_on_clicks(view(&ctx), WINDOW, &["input.snxsch", "input.snxsch"]);
    assert!(
        pressed
            .iter()
            .any(|m| matches!(m, PanelMsg::ObjectList(ObjectListMsg::RowDoubleClicked(_)))),
        "{pressed:?}"
    );
}
