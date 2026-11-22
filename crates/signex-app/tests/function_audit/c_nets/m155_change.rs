//! Mockup row 155, Change Component: picking a part only previews it; the
//! confirm swaps it in one undo step and keeps the designator and position.

use signex_app::app::{EditMsg, Message, Signex};
use signex_app::library::state::PickerMode;
use signex_app::library::{LibraryMessage, PlacedPartAction, PlacedPartMsg};
use signex_library::primitive::{Symbol, SymbolPin};
use signex_types::schematic::{SelectedItem, SelectedKind};

use super::library_place_symbol::{
    Fixture, app_with_libraries, doc, fixture, fixture_with_symbol, place,
};
use crate::settle_support::settle;

/// The replacement part: pin 1 stays, the shared pin 3 is numbered 9.
fn replacement() -> Symbol {
    let mut s = Symbol::empty("RENUMBERED");
    s.designator = "U?".into();
    s.part_count = 2;
    for (number, name, part, y) in [("1", "P1", 1u8, 0.0), ("9", "P3", 0, 10.16)] {
        let mut p = SymbolPin::new(number, name);
        p.position = [0.0, y];
        p.part_number = part;
        s.pins.push(p);
    }
    s
}

/// A placed, selected part with a second library open. The first library's
/// fixture is returned too: it owns the temp folder the part was read from.
fn placed() -> (Signex, Fixture, Fixture) {
    let _ = signex_app::diagnostics::init_logging();
    let old = fixture();
    let new = fixture_with_symbol("lib-new", replacement());
    let mut app = app_with_libraries(&[&old, &new]);
    place(&mut app, &old);
    let id = doc(&app).symbols[0].uuid;
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(id, SelectedKind::Symbol)];
    (app, old, new)
}

fn send(app: &mut Signex, msg: PlacedPartMsg) {
    settle(app, Message::Library(LibraryMessage::PlacedPart(msg)));
}

fn open_picker(app: &mut Signex) {
    send(
        app,
        PlacedPartMsg::ActionRequested(PlacedPartAction::ChangeComponent),
    );
    let picker = app.library.picker.as_ref().expect("the picker opens");
    assert!(matches!(picker.mode, PickerMode::ChangeComponent(_)));
}

/// Pick `new`'s row and continue to the preview.
fn pick(app: &mut Signex, new: &Fixture) {
    let row = app
        .library
        .open_libraries
        .iter()
        .find(|l| l.library_id == new.library_id)
        .and_then(|l| {
            l.cached_components
                .first()
                .map(|c| (l.root.clone(), c.clone()))
        });
    assert!(row.is_some(), "the replacement is listed");
    app.library.picker.as_mut().expect("the picker").selected = row;
    send(app, PlacedPartMsg::ChangePartPicked);
    let picker = app.library.picker.as_ref().expect("the picker stays open");
    assert!(
        matches!(picker.mode, PickerMode::ChangePreview(_)),
        "{:?}",
        picker.mode
    );
}

#[test]
fn m155_the_preview_changes_nothing_on_the_sheet() {
    let (mut app, _old, new) = placed();
    let before = doc(&app).clone();
    open_picker(&mut app);
    pick(&mut app, &new);
    assert_eq!(doc(&app).symbols, before.symbols);
}

#[test]
fn m155_confirm_swaps_the_part_keeps_designator_and_place_in_one_undo_step() {
    let (mut app, _old, new) = placed();
    let before = doc(&app).symbols[0].clone();
    let steps = app.document_state.active_engine().unwrap().undo_position();
    open_picker(&mut app);
    pick(&mut app, &new);
    send(&mut app, PlacedPartMsg::ChangeConfirmed);

    assert!(app.library.picker.is_none(), "the dialog closes");
    let after = &doc(&app).symbols[0];
    assert_ne!(after.lib_id, before.lib_id);
    assert_eq!(after.library_id, Some(new.library_id));
    assert_eq!(after.reference, before.reference, "the designator stays");
    assert_eq!(after.position, before.position, "the place stays");
    assert_eq!(after.rotation, before.rotation, "the rotation stays");
    assert_eq!(
        app.document_state.active_engine().unwrap().undo_position(),
        steps + 1
    );
    let path = app.document_state.active_path.clone().unwrap();
    assert!(app.document_state.is_document_dirty(&path));

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(doc(&app).symbols[0].lib_id, before.lib_id);
    assert_eq!(doc(&app).symbols[0].row_id, before.row_id);
}

#[test]
fn m155_closing_the_picker_leaves_the_part() {
    let (mut app, _old, new) = placed();
    let before = doc(&app).symbols.clone();
    let steps = app.document_state.active_engine().unwrap().undo_position();
    open_picker(&mut app);
    pick(&mut app, &new);
    let _ = app.update(Message::Library(LibraryMessage::ClosePicker));
    assert!(app.library.picker.is_none());
    send(&mut app, PlacedPartMsg::ChangeConfirmed);
    assert_eq!(doc(&app).symbols, before);
    assert_eq!(
        app.document_state.active_engine().unwrap().undo_position(),
        steps
    );
}

#[test]
fn m155_backing_out_of_the_preview_returns_to_the_part_list() {
    let (mut app, _old, new) = placed();
    let before = doc(&app).symbols.clone();
    open_picker(&mut app);
    pick(&mut app, &new);
    send(&mut app, PlacedPartMsg::ChangePreviewBackedOut);
    let picker = app.library.picker.as_ref().expect("the picker stays");
    assert!(matches!(picker.mode, PickerMode::ChangeComponent(_)));
    assert_eq!(doc(&app).symbols, before);
}

#[test]
fn m155_a_locked_part_is_refused_and_no_picker_opens() {
    let (mut app, _old, _new) = placed();
    let id = doc(&app).symbols[0].uuid;
    app.document_state
        .active_engine_mut()
        .unwrap()
        .execute(signex_engine::Command::SetSymbolLocked {
            symbol_id: id,
            locked: true,
        })
        .unwrap();
    send(
        &mut app,
        PlacedPartMsg::ActionRequested(PlacedPartAction::ChangeComponent),
    );
    assert!(app.library.picker.is_none());
}
