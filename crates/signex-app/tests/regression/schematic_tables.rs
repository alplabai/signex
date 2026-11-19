//! #1300: schematic tables. A placed table takes cell edits as single undo
//! steps.

use signex_app::app::{EditMsg, Message, Signex, TablesMsg};
use signex_types::schematic::{Point, SchDrawing, SchTable};

use crate::paste_support::sheet::{app_with_sheet, empty_sheet};

fn table(app: &Signex) -> SchTable {
    app.document_state
        .active_engine()
        .unwrap()
        .document()
        .drawings
        .iter()
        .find_map(|d| match d {
            SchDrawing::Table(t) => Some(t.clone()),
            _ => None,
        })
        .expect("a table")
}

#[test]
fn a_cell_edit_is_one_undo_step() {
    let mut sheet = empty_sheet();
    let id = uuid::Uuid::from_u128(77);
    sheet.drawings.push(SchDrawing::Table(SchTable::new_typed(
        id,
        Point::new(20.32, 20.32),
    )));
    let mut app = app_with_sheet(sheet);
    let _ = app.update(Message::Edit(EditMsg::Tables(TablesMsg::CellEdited {
        table: id,
        row: 0,
        column: 0,
        text: "0x00".into(),
    })));
    assert_eq!(table(&app).cell(0, 0), "0x00");
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(table(&app).cell(0, 0), "");
}
