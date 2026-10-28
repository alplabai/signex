//! #1301 Open Datasheet, #1262 Edit Symbol and #1260 Change Component on a
//! placed part: each action, from the context menu, Properties and the
//! command ids, ends in the model or in Messages, never in a dead click.

use crate::library_place_symbol::{
    Fixture, app_with_libraries, app_with_library, doc, fixture, fixture_with_symbol, place,
};
use crate::library_symbol_drift::{check_drift, select_first};
use crate::settle_support::{settle, settle_task};
use signex_app::app::contracts::{ContextAction, ContextMenuMsg};
use signex_app::app::{EditMsg, FileMsg, Message, Signex, TabKind};
use signex_app::library::placed_part::DatasheetLocation;
use signex_app::library::state::PickerMode;
use signex_app::library::{LibraryMessage, PlacedPartAction, PlacedPartMsg};
use signex_library::primitive::{Symbol, SymbolPin};
use signex_library::{DatasheetDoc, DatasheetRef};
use signex_types::schematic::{Point, SymbolTransform, Wire};
use std::path::PathBuf;
use std::sync::Mutex;

const SHEET: &str = "place-364.snxsch";

fn ensure_logger() {
    let _ = signex_app::diagnostics::init_logging();
}

/// A Messages-panel line containing every needle.
fn logged(needles: &[&str]) -> bool {
    signex_app::diagnostics::recent_entries()
        .iter()
        .any(|e| needles.iter().all(|n| e.message.contains(n)))
}

fn request(app: &mut Signex, action: PlacedPartAction) {
    settle(
        app,
        Message::Library(LibraryMessage::PlacedPart(PlacedPartMsg::ActionRequested(
            action,
        ))),
    );
}

fn send(app: &mut Signex, msg: PlacedPartMsg) {
    settle(app, Message::Library(LibraryMessage::PlacedPart(msg)));
}

/// Give the placed symbol a designator no other test logs, so a Messages
/// line can be told from the shared ring's other traffic.
fn rename(app: &mut Signex, reference: &str) {
    let (id, value, footprint) = {
        let s = &doc(app).symbols[0];
        (s.uuid, s.value.clone(), s.footprint.clone())
    };
    app.document_state
        .active_engine_mut()
        .unwrap()
        .execute(signex_engine::Command::UpdateSymbolFields {
            symbol_id: id,
            reference: reference.into(),
            value,
            footprint,
        })
        .unwrap();
}

/// A placed, selected part named `reference` from the fixture's library.
fn placed_part(f: &Fixture, reference: &str) -> Signex {
    ensure_logger();
    let mut app = app_with_library(f);
    place(&mut app, f);
    rename(&mut app, reference);
    select_first(&mut app);
    app
}

fn set_documents(app: &mut Signex, docs: Vec<DatasheetDoc>) {
    let lib = &mut app.library.open_libraries[0];
    lib.tables.get_mut("ICs").unwrap()[0].set_documents(docs);
}

static OPENED: Mutex<Vec<DatasheetLocation>> = Mutex::new(Vec::new());

fn record_open(location: &DatasheetLocation) -> Result<(), String> {
    OPENED.lock().unwrap().push(location.clone());
    Ok(())
}

fn refuse_open(_: &DatasheetLocation) -> Result<(), String> {
    Err("Refuse-opener advice for zq-open-fail".into())
}

fn opened_urls() -> Vec<String> {
    OPENED
        .lock()
        .unwrap()
        .iter()
        .filter_map(|l| match l {
            DatasheetLocation::Url(u) => Some(u.clone()),
            DatasheetLocation::File(_) => None,
        })
        .collect()
}

// ---- #1301 Open Datasheet ----

#[test]
fn a_part_placed_without_a_library_row_says_how_to_get_a_datasheet() {
    let f = fixture();
    let mut app = placed_part(&f, "ZD1");
    let mut d = doc(&app).clone();
    d.symbols[0].library_id = None;
    d.symbols[0].row_id = None;
    app.document_state
        .active_engine_mut()
        .unwrap()
        .set_document(d);
    select_first(&mut app);

    request(&mut app, PlacedPartAction::OpenDatasheet);

    assert!(logged(&["Place ZD1 from a library part", "no library row"]));
    assert!(app.library.picker.is_none());
}

#[test]
fn a_row_without_a_datasheet_says_how_to_add_one() {
    let f = fixture();
    let mut app = placed_part(&f, "ZD2");

    request(&mut app, PlacedPartAction::OpenDatasheet);

    assert!(logged(&["Add a datasheet to the library row of ZD2"]));
}

#[test]
fn a_part_whose_library_is_closed_says_to_open_it() {
    let f = fixture();
    let mut app = placed_part(&f, "ZD3");
    app.library.open_libraries.clear();

    request(&mut app, PlacedPartAction::OpenDatasheet);

    assert!(logged(&["Open the library of ZD3"]));
}

#[test]
fn one_document_opens_at_once_through_the_opener_task() {
    let f = fixture();
    let mut app = placed_part(&f, "ZD4");
    set_documents(
        &mut app,
        vec![DatasheetDoc::new(
            "",
            DatasheetRef::url("https://one.example/zd4.pdf"),
        )],
    );
    app.library.datasheet_opener = record_open;

    request(&mut app, PlacedPartAction::OpenDatasheet);

    assert!(app.library.picker.is_none(), "one document needs no choice");
    assert!(opened_urls().contains(&"https://one.example/zd4.pdf".to_string()));
}

#[test]
fn several_documents_ask_which_one_and_open_only_that() {
    let f = fixture();
    let mut app = placed_part(&f, "ZD5");
    set_documents(
        &mut app,
        vec![
            DatasheetDoc::new(
                "Datasheet",
                DatasheetRef::url("https://two.example/zd5.pdf"),
            ),
            DatasheetDoc::new(
                "Errata",
                DatasheetRef::url("https://two.example/zd5-errata.pdf"),
            ),
        ],
    );
    app.library.datasheet_opener = record_open;

    request(&mut app, PlacedPartAction::OpenDatasheet);

    let labels: Vec<String> = match &app.library.picker.as_ref().expect("a chooser").mode {
        PickerMode::Datasheets(choice) => choice.targets.iter().map(|t| t.label.clone()).collect(),
        other => panic!("expected the datasheet list, got {other:?}"),
    };
    assert_eq!(labels, ["Datasheet", "Errata"]);
    assert!(
        !opened_urls().iter().any(|u| u.contains("zd5")),
        "nothing opened yet"
    );

    send(&mut app, PlacedPartMsg::DatasheetPicked(1));

    assert!(app.library.picker.is_none());
    let opened = opened_urls();
    assert!(opened.contains(&"https://two.example/zd5-errata.pdf".to_string()));
    assert!(!opened.contains(&"https://two.example/zd5.pdf".to_string()));
}

#[test]
fn a_launcher_failure_reaches_messages() {
    let f = fixture();
    let mut app = placed_part(&f, "ZD6");
    set_documents(
        &mut app,
        vec![DatasheetDoc::new(
            "",
            DatasheetRef::url("https://x.example/zd6.pdf"),
        )],
    );
    app.library.datasheet_opener = refuse_open;

    request(&mut app, PlacedPartAction::OpenDatasheet);

    assert!(logged(&["Refuse-opener advice for zq-open-fail"]));
}

#[test]
fn the_context_menu_action_and_the_command_id_reach_the_datasheet_flow() {
    let f = fixture();
    let mut app = placed_part(&f, "ZD7");

    settle(
        &mut app,
        Message::ContextMenu(ContextMenuMsg::Action(ContextAction::PlacedPart(
            PlacedPartAction::OpenDatasheet,
        ))),
    );

    assert!(logged(&["Add a datasheet to the library row of ZD7"]));
}

#[test]
fn an_action_without_one_selected_placed_part_says_so() {
    let f = fixture();
    let mut app = placed_part(&f, "ZD8");
    app.interaction_state.active_canvas_mut().selected.clear();

    request(&mut app, PlacedPartAction::EditSymbol);

    assert!(logged(&["Select one placed part first"]));
}

// ---- #1262 Edit Symbol ----

/// The fixture names its symbol file `two-unit.snxsym`; a real library
/// names it by the symbol's uuid, which is where Edit Symbol looks.
fn symbol_file(f: &Fixture) -> PathBuf {
    let dir = f.snxlib.parent().unwrap().join("symbols");
    let path = dir.join(format!("{}.snxsym", f.symbol_id));
    std::fs::copy(dir.join("two-unit.snxsym"), &path).unwrap();
    path
}

fn symbol_editor_paths(app: &Signex) -> Vec<PathBuf> {
    app.document_state
        .tabs
        .iter()
        .filter(|t| matches!(t.kind, TabKind::SymbolEditor(_)))
        .map(|t| t.path.clone())
        .collect()
}

#[test]
fn edit_symbol_opens_the_source_symbol_in_the_symbol_editor() {
    let f = fixture();
    let path = symbol_file(&f);
    let mut app = placed_part(&f, "ZE1");

    request(&mut app, PlacedPartAction::EditSymbol);

    assert_eq!(symbol_editor_paths(&app), vec![path]);
}

#[test]
fn edit_symbol_through_the_context_menu_opens_the_same_tab() {
    let f = fixture();
    let path = symbol_file(&f);
    let mut app = placed_part(&f, "ZE2");

    settle(
        &mut app,
        Message::ContextMenu(ContextMenuMsg::Action(ContextAction::PlacedPart(
            PlacedPartAction::EditSymbol,
        ))),
    );

    assert_eq!(symbol_editor_paths(&app), vec![path]);
}

#[test]
fn edit_symbol_with_its_library_closed_says_to_open_it() {
    let f = fixture();
    symbol_file(&f);
    let mut app = placed_part(&f, "ZE3");
    app.library.open_libraries.clear();

    request(&mut app, PlacedPartAction::EditSymbol);

    assert!(logged(&["Open the library of ZE3 to edit its symbol"]));
    assert!(symbol_editor_paths(&app).is_empty());
}

#[test]
fn saving_the_edited_symbol_makes_the_placed_part_drift_so_the_update_flow_offers_it() {
    let f = fixture();
    let path = symbol_file(&f);
    let mut app = placed_part(&f, "ZE4");
    let sheet = PathBuf::from(SHEET);
    let lib_id = doc(&app).symbols[0].lib_id.clone();
    check_drift(&mut app);
    assert!(!app.library.drift.is_drifted(&sheet, &lib_id));

    request(&mut app, PlacedPartAction::EditSymbol);
    let editor = app.document_state.symbol_editors.get_mut(&path).unwrap();
    let mut pin = SymbolPin::new("4", "P4");
    pin.position = [0.0, 15.24];
    pin.part_number = 1;
    editor.file.symbols[0].pins.push(pin);
    editor.dirty = true;
    settle(&mut app, Message::File(FileMsg::Save));

    // No explicit recheck: the save itself raised it.
    assert!(
        app.library.drift.is_drifted(&sheet, &lib_id),
        "the saved symbol differs from the sheet's copy, which the update flow offers to refresh"
    );
}

// ---- #1260 Change Component ----

/// A symbol shaped like the fixture's, for the replacement part: pin "1"
/// stays, the shared pin "3" is numbered "9" but still named "P3".
fn renumbered() -> Symbol {
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

/// A replacement that lacks the old shared pin "3" / "P3" altogether.
fn lacking_pin_3() -> Symbol {
    let mut s = renumbered();
    s.pins.retain(|p| p.number == "1");
    s
}

fn wire_pins(app: &mut Signex, numbers: &[&str]) {
    for number in numbers {
        let tip = {
            let sheet = doc(app);
            let sym = &sheet.symbols[0];
            let lib = &sheet.lib_symbols[&sym.lib_id];
            let pin = lib.pins.iter().find(|p| p.pin.number == *number).unwrap();
            SymbolTransform::from_symbol(sym).apply(pin.pin.position)
        };
        app.document_state
            .active_engine_mut()
            .unwrap()
            .execute(signex_engine::Command::PlaceWireSegment {
                wire: Wire {
                    start: tip,
                    end: Point::new(tip.x - 10.0, tip.y),
                    ..Default::default()
                },
            })
            .unwrap();
    }
}

/// A sheet holding the fixture's part `reference`, its pins `wired`, and the
/// replacement library open next to it.
fn change_setup(reference: &str, replacement: Symbol, wired: &[&str]) -> (Signex, Fixture) {
    ensure_logger();
    let old = fixture();
    let new = fixture_with_symbol("lib-new", replacement);
    let mut app = app_with_libraries(&[&old, &new]);
    place(&mut app, &old);
    rename(&mut app, reference);
    wire_pins(&mut app, wired);
    select_first(&mut app);
    (app, new)
}

/// Pick `new`'s row in the open Change Component picker and continue.
fn pick_part(app: &mut Signex, new: &Fixture) {
    let picker = app.library.picker.as_mut().expect("the picker is open");
    picker.selected = app
        .library
        .open_libraries
        .iter()
        .find(|l| l.library_id == new.library_id)
        .and_then(|l| {
            l.cached_components
                .first()
                .map(|c| (l.root.clone(), c.clone()))
        });
    assert!(picker.selected.is_some(), "the replacement row is listed");
    send(app, PlacedPartMsg::ChangePartPicked);
}

fn symbol_lib_id(app: &Signex) -> String {
    doc(app).symbols[0].lib_id.clone()
}

fn preview_of(app: &Signex) -> signex_app::library::state::ChangePreview {
    match &app
        .library
        .picker
        .as_ref()
        .expect("the picker stays open")
        .mode
    {
        PickerMode::ChangePreview(p) => (**p).clone(),
        other => panic!("expected the pin preview, got {other:?}"),
    }
}

#[test]
fn change_component_opens_the_part_picker_for_the_selected_part() {
    let (mut app, _new) = change_setup("ZC1", renumbered(), &[]);

    request(&mut app, PlacedPartAction::ChangeComponent);

    let picker = app.library.picker.as_ref().expect("a picker");
    assert!(matches!(picker.mode, PickerMode::ChangeComponent(_)));
}

#[test]
fn a_locked_part_is_not_offered_a_new_component() {
    let (mut app, _new) = change_setup("ZC2", renumbered(), &[]);
    let id = doc(&app).symbols[0].uuid;
    app.document_state
        .active_engine_mut()
        .unwrap()
        .execute(signex_engine::Command::SetSymbolLocked {
            symbol_id: id,
            locked: true,
        })
        .unwrap();

    request(&mut app, PlacedPartAction::ChangeComponent);

    assert!(logged(&["Unlock ZC2"]));
    assert!(app.library.picker.is_none());
}

#[test]
fn the_preview_maps_by_number_then_name_and_changes_nothing_yet() {
    let (mut app, new) = change_setup("ZC3", renumbered(), &["1", "3"]);
    let before = symbol_lib_id(&app);
    request(&mut app, PlacedPartAction::ChangeComponent);

    pick_part(&mut app, &new);

    let p = preview_of(&app);
    let pairs: Vec<(String, String)> = p
        .preview
        .mapping
        .pairs
        .iter()
        .map(|x| (x.old.clone(), x.new.clone()))
        .collect();
    assert_eq!(pairs, [("1".into(), "1".into()), ("3".into(), "9".into())]);
    assert!(p.preview.is_clean(), "{:?}", p.preview);
    assert_eq!(
        symbol_lib_id(&app),
        before,
        "nothing is committed by the preview"
    );
}

#[test]
fn connected_pins_with_no_match_are_listed_before_anything_is_committed() {
    let (mut app, new) = change_setup("ZC4", lacking_pin_3(), &["1", "3"]);
    let before = symbol_lib_id(&app);
    request(&mut app, PlacedPartAction::ChangeComponent);

    pick_part(&mut app, &new);

    let p = preview_of(&app);
    assert_eq!(p.preview.unmatched_connected, ["3".to_string()]);
    assert_eq!(symbol_lib_id(&app), before);
}

#[test]
fn confirming_swaps_the_part_keeps_the_designator_and_is_one_undo_step() {
    let (mut app, new) = change_setup("ZC5", renumbered(), &["1", "3"]);
    let before = symbol_lib_id(&app);
    let old_row = doc(&app).symbols[0].row_id;
    request(&mut app, PlacedPartAction::ChangeComponent);
    pick_part(&mut app, &new);

    send(&mut app, PlacedPartMsg::ChangeConfirmed);

    let sym = &doc(&app).symbols[0];
    assert_ne!(sym.lib_id, before);
    assert_ne!(sym.row_id, old_row);
    assert_eq!(sym.library_id, Some(new.library_id));
    assert_eq!(sym.reference, "ZC5", "the designator stays");
    assert!(doc(&app).lib_symbols.contains_key(&sym.lib_id));
    assert!(app.library.picker.is_none(), "the modal closes");

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(
        symbol_lib_id(&app),
        before,
        "one undo restores the old part"
    );
    assert_eq!(doc(&app).symbols[0].row_id, old_row);
}

#[test]
fn backing_out_of_the_preview_returns_to_the_part_list() {
    let (mut app, new) = change_setup("ZC6", renumbered(), &[]);
    request(&mut app, PlacedPartAction::ChangeComponent);
    pick_part(&mut app, &new);

    send(&mut app, PlacedPartMsg::ChangePreviewBackedOut);

    assert!(matches!(
        app.library.picker.as_ref().unwrap().mode,
        PickerMode::ChangeComponent(_)
    ));
}

#[test]
fn a_part_read_that_finishes_after_the_picker_closed_changes_nothing() {
    let (mut app, new) = change_setup("ZC7", renumbered(), &[]);
    let before = symbol_lib_id(&app);
    request(&mut app, PlacedPartAction::ChangeComponent);
    let picker = app.library.picker.as_mut().unwrap();
    picker.selected = app
        .library
        .open_libraries
        .iter()
        .find(|l| l.library_id == new.library_id)
        .and_then(|l| {
            l.cached_components
                .first()
                .map(|c| (l.root.clone(), c.clone()))
        });
    let task = app.update(Message::Library(LibraryMessage::PlacedPart(
        PlacedPartMsg::ChangePartPicked,
    )));
    // The user dismisses the picker while the read is in flight.
    let _ = app.update(Message::Library(LibraryMessage::ClosePicker));
    settle_task(&mut app, task);

    assert!(app.library.picker.is_none(), "no preview is conjured");
    send(&mut app, PlacedPartMsg::ChangeConfirmed);
    assert_eq!(symbol_lib_id(&app), before);
}

#[test]
fn confirming_after_the_part_was_deleted_is_refused_and_logged() {
    let (mut app, new) = change_setup("ZC8", renumbered(), &[]);
    request(&mut app, PlacedPartAction::ChangeComponent);
    pick_part(&mut app, &new);
    // The sheet's symbol is deleted while the preview is open.
    let id = doc(&app).symbols[0].uuid;
    app.document_state
        .active_engine_mut()
        .unwrap()
        .execute(signex_engine::Command::DeleteSelection {
            items: vec![signex_types::schematic::SelectedItem::new(
                id,
                signex_types::schematic::SelectedKind::Symbol,
            )],
        })
        .unwrap();

    send(&mut app, PlacedPartMsg::ChangeConfirmed);

    assert!(logged(&["ZC8", "was not changed"]));
    assert!(doc(&app).symbols.is_empty());
    assert!(app.library.picker.is_none());
}
