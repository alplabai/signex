//! m129 Library picker and m131 Primitive picker.

use signex_app::app::{EditMsg, Message};
use signex_app::canvas::CanvasEvent;
use signex_app::library::messages::{
    BrowserEditMsg, LibraryMessage, PickerMsg, PrimitivePickerMsg,
};
use signex_app::library::state::{EditorAddress, PrimitivePickerTarget};
use signex_library::{PrimitiveKind, PrimitiveRef, RowId};
use uuid::Uuid;

use super::support::{LibRig, TABLE, fresh_app, library_rig, open_sheet, read_only_rig};
use crate::fixture;
use crate::settle_support::settle;

fn picker(r: &mut LibRig, msg: PickerMsg) {
    r.run(LibraryMessage::Picker(msg));
}

fn listed(r: &LibRig) -> usize {
    r.app
        .library
        .picker
        .as_ref()
        .expect("the picker is open")
        .rows
        .visible()
        .count()
}

fn open_primitive_picker(r: &mut LibRig, kind: PrimitiveKind, row_id: RowId) {
    let address = EditorAddress::new(r.root.clone(), TABLE.to_string(), row_id);
    r.run(LibraryMessage::OpenPrimitivePicker {
        kind,
        target: PrimitivePickerTarget::BrowserRow(address),
    });
}

fn primitive(r: &mut LibRig, msg: PrimitivePickerMsg) {
    r.run(LibraryMessage::PrimitivePicker(msg));
}

// ---- m129 ----

#[test]
fn the_picker_lists_every_row_and_the_filter_narrows_it() {
    let mut r = library_rig(3);
    r.run(LibraryMessage::OpenPicker);
    assert_eq!(listed(&r), 3);

    picker(&mut r, PickerMsg::FilterChanged("R-0001".to_string()));

    assert_eq!(listed(&r), 1);
}

#[test]
fn a_filter_that_hides_the_selected_row_clears_the_selection_and_place_does_nothing() {
    let mut r = library_rig(2);
    r.run(LibraryMessage::OpenPicker);
    let id = r.rows[0].as_uuid();
    picker(&mut r, PickerMsg::SelectComponent(id));
    assert!(
        r.app
            .library
            .picker
            .as_ref()
            .expect("open")
            .selected
            .is_some()
    );

    picker(&mut r, PickerMsg::FilterChanged("no-such-part".to_string()));
    assert!(
        r.app
            .library
            .picker
            .as_ref()
            .expect("open")
            .selected
            .is_none()
    );

    picker(&mut r, PickerMsg::PlaceSelected);
    assert!(r.app.library.picker.is_some(), "the picker stays open");
    assert!(r.app.document_state.error_notice.is_none());
}

#[test]
fn cancel_closes_the_picker_and_changes_nothing() {
    let mut r = library_rig(1);
    let before = r.files();
    r.run(LibraryMessage::OpenPicker);

    r.run(LibraryMessage::ClosePicker);

    assert!(r.app.library.picker.is_none());
    assert_eq!(r.files(), before);
}

#[test]
fn place_with_no_sheet_in_front_says_so() {
    let mut r = library_rig(1);
    r.run(LibraryMessage::OpenPicker);
    let id = r.rows[0].as_uuid();
    picker(&mut r, PickerMsg::SelectComponent(id));

    picker(&mut r, PickerMsg::PlaceSelected);

    assert!(r.app.document_state.error_notice.is_some());
}

#[test]
fn place_arms_the_part_a_click_places_it_and_undo_removes_it() {
    let fx = fixture::fixture().expect("fixture");
    let mut app = fresh_app();
    let (sheet, _tmp) = open_sheet(&mut app);
    fixture::mount_stock(&mut app, &fx).expect("mount stock");
    let (_, resistor) = app
        .library
        .all_components()
        .into_iter()
        .find(|(_, c)| c.internal_pn.as_str() == "STOCK-R")
        .expect("STOCK-R");
    settle(&mut app, Message::Library(LibraryMessage::OpenPicker));
    settle(
        &mut app,
        Message::Library(LibraryMessage::Picker(PickerMsg::SelectComponent(
            resistor.row_id,
        ))),
    );

    settle(
        &mut app,
        Message::Library(LibraryMessage::Picker(PickerMsg::PlaceSelected)),
    );
    assert!(app.library.picker.is_none(), "the picker closes");
    assert!(app.document_state.error_notice.is_none());
    assert!(
        app.document_state.engines[&sheet]
            .document()
            .symbols
            .is_empty(),
        "nothing lands before the click"
    );

    let _ = app.update(Message::CanvasEvent(CanvasEvent::Clicked {
        world_x: 25.4,
        world_y: 38.1,
    }));
    let placed = &app.document_state.engines[&sheet].document().symbols;
    assert_eq!(placed.len(), 1);
    assert_eq!(placed[0].row_id, Some(resistor.row_id));
    assert!(app.document_state.is_document_dirty(&sheet));

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert!(
        app.document_state.engines[&sheet]
            .document()
            .symbols
            .is_empty()
    );
    assert!(!app.document_state.is_document_dirty(&sheet));
}

// ---- m131 ----

#[test]
fn picking_a_symbol_binds_it_to_the_row_and_closes() {
    let mut r = library_rig(1);
    let id = r.rows[0];
    let picked = PrimitiveRef::new(r.library_id(), Uuid::now_v7());
    open_primitive_picker(&mut r, PrimitiveKind::Symbol, id);

    primitive(&mut r, PrimitivePickerMsg::Pick(picked));

    assert!(r.app.library.primitive_picker.is_none());
    assert_eq!(r.row(id).expect("row").symbol_ref, picked);
}

#[test]
fn picking_a_footprint_binds_it_to_the_row() {
    let mut r = library_rig(1);
    let id = r.rows[0];
    let picked = PrimitiveRef::new(r.library_id(), Uuid::now_v7());
    open_primitive_picker(&mut r, PrimitiveKind::Footprint, id);

    primitive(&mut r, PrimitivePickerMsg::Pick(picked));

    assert_eq!(r.row(id).expect("row").footprint_ref, Some(picked));
}

#[test]
fn the_primitive_filter_is_kept_and_cancel_binds_nothing() {
    let mut r = library_rig(1);
    let id = r.rows[0];
    let bound = r.row(id).expect("row").symbol_ref;
    let before = r.files();
    open_primitive_picker(&mut r, PrimitiveKind::Symbol, id);

    primitive(&mut r, PrimitivePickerMsg::SetFilter("res".to_string()));
    assert_eq!(
        r.app
            .library
            .primitive_picker
            .as_ref()
            .expect("open")
            .filter,
        "res"
    );
    primitive(&mut r, PrimitivePickerMsg::Cancel);

    assert!(r.app.library.primitive_picker.is_none());
    assert_eq!(r.row(id).expect("row").symbol_ref, bound);
    assert_eq!(r.files(), before);
}

#[test]
fn a_pick_from_the_edit_row_modal_waits_for_its_save() {
    let mut r = library_rig(1);
    let id = r.rows[0];
    let bound = r.row(id).expect("row").symbol_ref;
    let picked = PrimitiveRef::new(r.library_id(), Uuid::now_v7());
    r.open_modal(id);
    r.edit(BrowserEditMsg::OpenSymbolPicker);
    assert!(r.app.library.primitive_picker.is_some());

    primitive(&mut r, PrimitivePickerMsg::Pick(picked));

    assert_eq!(r.modal().draft.symbol_ref, picked);
    assert_eq!(r.row(id).expect("row").symbol_ref, bound, "not saved yet");
}

#[test]
fn a_read_only_library_opens_no_primitive_picker() {
    let mut r = read_only_rig(1);
    let id = r.rows[0];

    open_primitive_picker(&mut r, PrimitiveKind::Symbol, id);

    assert!(r.app.library.primitive_picker.is_none());
    assert!(r.app.document_state.error_notice.is_some());
}
