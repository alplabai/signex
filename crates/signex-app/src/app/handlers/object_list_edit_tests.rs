//! #2953: an Object List cell edited in place, through `Signex::update` as
//! the panel sends it: Enter is one undo step and the same edit Properties
//! makes, Esc and an unchanged draft write nothing, and a cell that cannot
//! be edited here zooms instead.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use signex_types::schematic::{Label, LabelType, Point, SchematicSheet, Symbol};
use uuid::Uuid;

use crate::app::handlers::editing_commands::net_color_driver::add_sheet_with;
use crate::app::{Message, Signex};
use crate::panels::object_list::cell_edit::{CellEditMsg, ObjectColumn};
use crate::panels::object_list::model::sheet_rows;
use crate::panels::{ObjectListMsg, ObjectRef, PanelMsg};

const PART: u128 = 1;
const LABEL: u128 = 2;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn sheet() -> SchematicSheet {
    SchematicSheet {
        paper_size: "A4".into(),
        symbols: vec![Symbol {
            uuid: id(PART),
            lib_id: "Device:R".into(),
            reference: "R1".into(),
            value: "10k".into(),
            position: Point::new(10.0, 10.0),
            ..Default::default()
        }],
        labels: vec![Label {
            uuid: id(LABEL),
            ..Label::new("NET", Point::new(40.0, 10.0), LabelType::Net)
        }],
        ..Default::default()
    }
}

/// The sheet open and in view, its rows listed.
fn app_with(dir: &Path) -> (Signex, PathBuf) {
    let (mut app, _task) = Signex::new();
    let path = dir.join("main.snxsch");
    add_sheet_with(&mut app, &path, sheet());
    let rows = sheet_rows(&sheet(), &path, &|_: Uuid| None);
    app.ui_state.object_list.source.rows = Arc::new(rows);
    (app, path)
}

fn doc(app: &Signex) -> &SchematicSheet {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
}

fn can_undo(app: &Signex) -> bool {
    app.document_state
        .active_engine()
        .is_some_and(|e| e.can_undo())
}

fn send(app: &mut Signex, msg: CellEditMsg) {
    let _ = app.update(Message::Dock(crate::dock::DockMessage::Panel(
        PanelMsg::ObjectList(ObjectListMsg::CellEdit(msg)),
    )));
}

fn target(path: &Path, n: u128) -> ObjectRef {
    let rows = sheet_rows(&sheet(), path, &|_: Uuid| None);
    rows.into_iter()
        .find(|r| r.target.item.uuid == id(n))
        .expect("row")
        .target
}

/// Double-click the cell, type `text`, press Enter.
fn edit(app: &mut Signex, path: &Path, n: u128, column: ObjectColumn, text: &str) {
    send(
        app,
        CellEditMsg::Opened {
            target: target(path, n),
            column,
        },
    );
    send(app, CellEditMsg::DraftChanged(text.into()));
    send(app, CellEditMsg::Committed);
}

fn undo(app: &mut Signex) {
    let _ = app.update(Message::Edit(crate::app::EditMsg::Undo));
}

#[test]
fn a_cell_opens_with_its_stored_text_and_enter_is_one_undo_step() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with(dir.path());
    send(
        &mut app,
        CellEditMsg::Opened {
            target: target(&path, PART),
            column: ObjectColumn::Value,
        },
    );
    let open = app.ui_state.object_list.editing.clone().expect("open");
    assert_eq!(
        (open.original.as_str(), open.draft.as_str()),
        ("10k", "10k")
    );
    send(&mut app, CellEditMsg::DraftChanged("4k7".into()));
    send(&mut app, CellEditMsg::Committed);
    assert!(
        app.ui_state.object_list.editing.is_none(),
        "Enter closes it"
    );
    assert_eq!(doc(&app).symbols[0].value, "4k7");

    edit(&mut app, &path, PART, ObjectColumn::Name, "R9");
    edit(&mut app, &path, LABEL, ObjectColumn::Name, "SDA");
    assert_eq!(doc(&app).symbols[0].reference, "R9");
    assert_eq!(doc(&app).labels[0].text, "SDA");

    undo(&mut app);
    assert_eq!(doc(&app).labels[0].text, "NET");
    undo(&mut app);
    assert_eq!(doc(&app).symbols[0].reference, "R1");
    undo(&mut app);
    assert_eq!(doc(&app).symbols[0].value, "10k");
    assert!(!can_undo(&app), "one step each");
}

/// The cell sends the edit the Value row of Properties sends: the same
/// document after, and one undo step either way.
#[test]
fn a_cell_edit_is_the_edit_properties_makes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut by_cell, path) = app_with(dir.path());
    edit(&mut by_cell, &path, PART, ObjectColumn::Value, "4k7");

    let other = tempfile::tempdir().expect("tempdir");
    let (mut by_row, _) = app_with(other.path());
    let _ = by_row.update(Message::Dock(crate::dock::DockMessage::Panel(
        PanelMsg::EditSymbolValue(id(PART), "4k7".into()),
    )));
    assert_eq!(doc(&by_cell).symbols, doc(&by_row).symbols);
    for app in [&mut by_cell, &mut by_row] {
        undo(app);
        assert_eq!(doc(app).symbols[0].value, "10k");
        assert!(!can_undo(app));
    }
}

#[test]
fn esc_and_an_unchanged_draft_write_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with(dir.path());
    send(
        &mut app,
        CellEditMsg::Opened {
            target: target(&path, PART),
            column: ObjectColumn::Value,
        },
    );
    send(&mut app, CellEditMsg::DraftChanged("1M".into()));
    let _ = app.handle_escape_pressed(None);
    assert!(app.ui_state.object_list.editing.is_none());
    assert_eq!(doc(&app).symbols[0].value, "10k");

    edit(&mut app, &path, PART, ObjectColumn::Value, "10k");
    assert!(!can_undo(&app), "nothing changed, no step");
}

/// A cell with no single text, or of a sheet not in view, does not open: it
/// zooms to the object as the row's double click does.
#[test]
fn a_cell_that_cannot_be_edited_here_does_not_open() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with(dir.path());
    send(
        &mut app,
        CellEditMsg::Opened {
            target: target(&path, LABEL),
            column: ObjectColumn::Value,
        },
    );
    assert!(app.ui_state.object_list.editing.is_none());

    let elsewhere = ObjectRef {
        sheet: dir.path().join("other.snxsch"),
        ..target(&path, PART)
    };
    send(
        &mut app,
        CellEditMsg::Opened {
            target: elsewhere,
            column: ObjectColumn::Value,
        },
    );
    assert!(app.ui_state.object_list.editing.is_none());
    assert_eq!(doc(&app).symbols[0].value, "10k");
}
