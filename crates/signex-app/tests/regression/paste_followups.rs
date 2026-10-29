//! #1676 review follow-ups: what Copy and Cut leave on the system clipboard,
//! and Paste Array against a copy that changed under the open dialog.

use signex_app::app::{EditMsg, Message, Signex};
use signex_app::paste_array::PasteArrayMsg;
use signex_types::schematic::{ChildSheet, Point, SelectedItem, SelectedKind};
use uuid::Uuid;

use crate::paste_support::sheet::{app_with_sheet, part, populated_sheet, references, select};
use crate::paste_support::written_text;

fn child_sheet() -> ChildSheet {
    ChildSheet {
        uuid: Uuid::new_v4(),
        name: "child".to_owned(),
        filename: "child.snxsch".to_owned(),
        position: Point::new(60.0, 60.0),
        size: (10.0, 10.0),
        stroke_width: 0.0,
        fill: Default::default(),
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    }
}

fn app_with_child_sheet() -> (Signex, Uuid) {
    let mut sheet = populated_sheet();
    let child = child_sheet();
    let id = child.uuid;
    sheet.child_sheets.push(child);
    (app_with_sheet(sheet), id)
}

fn select_child(app: &mut Signex, id: Uuid) {
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(id, SelectedKind::ChildSheet)];
}

#[test]
fn a_copy_that_carries_nothing_empties_the_system_clipboard() {
    let (mut app, child) = app_with_child_sheet();
    select(&mut app, &[("R1", 1)], false);
    let first = written_text(app.update(Message::Edit(EditMsg::Copy)));
    assert!(
        first.is_some_and(|t| !t.is_empty()),
        "a real copy is written"
    );

    select_child(&mut app, child);
    let task = app.update(Message::Edit(EditMsg::Copy));
    assert_eq!(
        written_text(task).as_deref(),
        Some(""),
        "the older fragment must not stay on the system clipboard"
    );
    assert!(app.interaction_state.clipboard.is_none());
}

#[test]
fn a_copy_with_nothing_selected_leaves_both_clipboards_alone() {
    let (mut app, _child) = app_with_child_sheet();
    select(&mut app, &[("R1", 1)], false);
    let _ = app.update(Message::Edit(EditMsg::Copy));
    app.interaction_state.active_canvas_mut().selected.clear();
    let task = app.update(Message::Edit(EditMsg::Copy));
    assert_eq!(written_text(task), None);
    assert!(app.interaction_state.clipboard.is_some());
}

#[test]
fn a_cut_whose_delete_fails_writes_nothing_and_keeps_the_earlier_copy() {
    let mut app = app_with_sheet(populated_sheet());
    select(&mut app, &[("R1", 1)], false);
    let _ = app.update(Message::Edit(EditMsg::Copy));
    let earlier = app.interaction_state.clipboard.clone().unwrap().origin;

    // A newer-minor sheet is read-only, so the delete is refused.
    let path = app.document_state.active_path.clone().unwrap();
    let engine = app.document_state.engines.get_mut(&path).unwrap();
    let mut sheet = engine.document().clone();
    sheet.unknown.newer_minor_source = Some("snxsch/1.9".to_owned());
    engine.set_document(sheet);

    select(&mut app, &[("R3", 1)], false);
    let task = app.update(Message::Edit(EditMsg::Cut));
    assert_eq!(written_text(task), None, "no system clipboard write");
    assert_eq!(references(&app).len(), 5, "nothing was deleted");
    let kept = app.interaction_state.clipboard.as_ref().unwrap();
    assert_eq!(kept.origin, earlier, "the earlier copy is still in charge");
    assert_eq!(app.interaction_state.active_canvas().selected.len(), 1);
}

#[test]
fn paste_array_checks_the_item_cap_against_the_copy_it_places() {
    let mut app = app_with_sheet(populated_sheet());
    select(&mut app, &[("R1", 1)], false);
    let _ = app.update(Message::Edit(EditMsg::Copy));
    crate::paste_stamp_array::open_array(&mut app);
    assert!(app.ui_state.paste_array.is_some());

    // Another copy, of 101 items, replaces the one the dialog was opened for.
    let mut sheet = populated_sheet();
    sheet.symbols = (0..101)
        .map(|n| part(&format!("C{n}"), "lib:r", 1, (10.16 * f64::from(n), 80.0)))
        .collect();
    let mut big = app_with_sheet(sheet);
    let items: Vec<_> = big
        .document_state
        .active_engine()
        .unwrap()
        .document()
        .symbols
        .iter()
        .map(|s| SelectedItem::new(s.uuid, SelectedKind::Symbol))
        .collect();
    big.interaction_state.active_canvas_mut().selected = items;
    let _ = big.update(Message::Edit(EditMsg::Copy));
    app.interaction_state.clipboard = big.interaction_state.clipboard.clone();

    let before = references(&app).len();
    for msg in [
        PasteArrayMsg::CountChanged("1000".to_owned()),
        PasteArrayMsg::ApplyPressed,
    ] {
        let _ = app.update(Message::Edit(EditMsg::PasteArray(msg)));
    }
    assert_eq!(references(&app).len(), before, "nothing was placed");
    assert!(app.ui_state.paste_array.is_some(), "the dialog stays open");
}
