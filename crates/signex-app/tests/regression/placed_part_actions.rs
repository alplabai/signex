//! #1301 Open Datasheet, #1262 Edit Symbol and #1260 Change Component on a
//! placed part: each action, from the context menu, Properties and the
//! command ids, ends in the model or in Messages, never in a dead click.

use crate::library_place_symbol::{
    Fixture, app_with_libraries, app_with_library, doc, fixture, fixture_with_symbol, place,
};
use crate::library_symbol_drift::{check_drift, select_first};
use crate::settle_support::{settle, settle_task};
use signex_app::app::contracts::{ContextAction, ContextMenuMsg};
use signex_app::app::{EditMsg, FileMsg, LoadedProject, Message, Signex, TabKind};
use signex_app::dock::DockMessage;
use signex_app::library::placed_part::DatasheetLocation;
use signex_app::library::state::PickerMode;
use signex_app::library::{LibraryMessage, PlacedPartAction, PlacedPartMsg};
use signex_app::panels::PanelMsg;
use signex_library::primitive::{Symbol, SymbolPin};
use signex_library::{DatasheetDoc, DatasheetRef};
use signex_types::project::{ProjectData, SheetEntry};
use signex_types::schematic::{Point, SymbolTransform, Wire};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use uuid::Uuid;

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

fn record_open(location: &DatasheetLocation, _part: &str) -> Result<(), String> {
    OPENED.lock().unwrap().push(location.clone());
    Ok(())
}

fn refuse_open(_: &DatasheetLocation, _part: &str) -> Result<(), String> {
    Err("Refuse-opener advice for zq-open-fail".into())
}

fn opened_urls() -> Vec<String> {
    OPENED
        .lock()
        .unwrap()
        .iter()
        .filter_map(|l| match l {
            DatasheetLocation::Url(u) => Some(u.clone()),
            DatasheetLocation::Pinned { .. } => None,
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
    editor.mark_dirty();
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
    change_setup_in("lib-new", reference, replacement, wired)
}

fn change_setup_in(
    library: &str,
    reference: &str,
    replacement: Symbol,
    wired: &[&str],
) -> (Signex, Fixture) {
    change_setup_from(fixture(), library, reference, replacement, wired)
}

/// [`change_setup_in`] for a placed part that comes from the `old` library.
fn change_setup_from(
    old: Fixture,
    library: &str,
    reference: &str,
    replacement: Symbol,
    wired: &[&str],
) -> (Signex, Fixture) {
    ensure_logger();
    let new = fixture_with_symbol(library, replacement);
    let mut app = app_with_libraries(&[&old, &new]);
    place(&mut app, &old);
    rename(&mut app, reference);
    wire_pins(&mut app, wired);
    select_first(&mut app);
    (app, new)
}

/// Select `f`'s row in the open Change Component picker.
fn select_row(app: &mut Signex, f: &Fixture) {
    let selected = app
        .library
        .open_libraries
        .iter()
        .find(|l| l.library_id == f.library_id)
        .and_then(|l| {
            l.cached_components
                .first()
                .map(|c| (l.root.clone(), c.clone()))
        });
    assert!(selected.is_some(), "the row is listed");
    app.library
        .picker
        .as_mut()
        .expect("the picker is open")
        .selected = selected;
}

/// Pick `new`'s row in the open Change Component picker and continue.
fn pick_part(app: &mut Signex, new: &Fixture) {
    select_row(app, new);
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
    let pin_uuid = Uuid::from_u128(0x77);
    set_symbol(&mut app, |s| {
        s.pin_uuids.insert("3".into(), pin_uuid);
    });
    request(&mut app, PlacedPartAction::ChangeComponent);
    pick_part(&mut app, &new);

    send(&mut app, PlacedPartMsg::ChangeConfirmed);

    // The wires still meet the mapped pins (1 -> 1, 3 -> 9), the value is the
    // new part's and the pin uuid followed pin 3 to its new number.
    let tips: Vec<Point> = {
        let sheet = doc(&app);
        let sym = &sheet.symbols[0];
        let x = SymbolTransform::from_symbol(sym);
        sheet.lib_symbols[&sym.lib_id]
            .pins
            .iter()
            .filter(|p| p.unit == 0 || p.unit == sym.unit)
            .map(|p| x.apply(p.pin.position))
            .collect()
    };
    assert_eq!(tips.len(), 2);
    for tip in tips {
        assert!(
            doc(&app).wires.iter().any(|w| w.start == tip),
            "a wire no longer meets a pin at {tip:?}"
        );
    }
    assert_eq!(doc(&app).symbols[0].value, "TWO-1");
    assert_eq!(doc(&app).symbols[0].pin_uuids.get("9"), Some(&pin_uuid));

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
    select_row(&mut app, &new);
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

fn set_symbol(app: &mut Signex, edit: impl FnOnce(&mut signex_types::schematic::Symbol)) {
    let mut d = doc(app).clone();
    edit(&mut d.symbols[0]);
    app.document_state
        .active_engine_mut()
        .unwrap()
        .set_document(d);
}

/// A second unit (unit 2) of the placed part, same designator and drawing.
fn add_second_unit(app: &mut Signex) {
    let mut d = doc(app).clone();
    let mut unit2 = d.symbols[0].clone();
    unit2.uuid = Uuid::now_v7();
    unit2.unit = 2;
    unit2.position = Point::new(80.0, 40.0);
    d.symbols.push(unit2);
    app.document_state
        .active_engine_mut()
        .unwrap()
        .set_document(d);
}

/// A replacement that draws unit 1 only (no shared pins).
fn unit_one_only() -> Symbol {
    let mut s = Symbol::empty("ONE-UNIT");
    s.designator = "U?".into();
    for (number, y) in [("1", 0.0), ("2", 5.08)] {
        let mut p = SymbolPin::new(number, format!("P{number}"));
        p.position = [0.0, y];
        p.part_number = 1;
        s.pins.push(p);
    }
    s
}

#[test]
fn every_unit_of_the_part_on_the_sheet_is_swapped_in_one_undo_step() {
    let (mut app, new) = change_setup("ZM1", renumbered(), &[]);
    add_second_unit(&mut app);
    let before = symbol_lib_id(&app);
    request(&mut app, PlacedPartAction::ChangeComponent);
    pick_part(&mut app, &new);

    send(&mut app, PlacedPartMsg::ChangeConfirmed);

    let after: Vec<(String, Option<Uuid>)> = doc(&app)
        .symbols
        .iter()
        .map(|s| (s.lib_id.clone(), s.row_id))
        .collect();
    assert_eq!(after.len(), 2);
    assert_eq!(after[0], after[1], "both units moved to the new part");
    assert_ne!(after[0].0, before);

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert!(doc(&app).symbols.iter().all(|s| s.lib_id == before));
}

#[test]
fn units_on_another_sheet_of_the_project_refuse_the_change() {
    let (mut app, _new) = change_setup("ZM2", renumbered(), &[]);
    let other_sheet = join_project_with_a_second_sheet(&mut app);
    // The other sheet holds another unit of ZM2 on the same drawing.
    let mut unit = doc(&app).symbols[0].clone();
    unit.uuid = Uuid::now_v7();
    unit.unit = 2;
    let mut sheet = doc(&app).clone();
    sheet.symbols = vec![unit];
    app.document_state.engines.insert(
        other_sheet,
        signex_engine::Engine::new(sheet).expect("engine"),
    );

    request(&mut app, PlacedPartAction::ChangeComponent);

    assert!(logged(&[
        "Change Component works on one sheet; units of ZM2 are also on",
        "sheet-two.snxsch"
    ]));
    assert!(app.library.picker.is_none());
}

#[test]
fn a_new_part_that_lacks_a_placed_unit_is_refused_before_the_preview() {
    let (mut app, new) = change_setup("ZM3", unit_one_only(), &[]);
    add_second_unit(&mut app);
    let before = symbol_lib_id(&app);
    request(&mut app, PlacedPartAction::ChangeComponent);

    pick_part(&mut app, &new);

    assert!(logged(&["Pick a part that has unit 2", "ZM3"]));
    assert!(matches!(
        app.library.picker.as_ref().unwrap().mode,
        PickerMode::ChangeComponent(_)
    ));
    assert_eq!(symbol_lib_id(&app), before);
}

#[test]
fn a_sheet_that_changed_after_the_preview_shows_the_fresh_preview_instead_of_committing() {
    // The new part has no pin 3. Unwired, that costs nothing: a clean preview.
    let (mut app, new) = change_setup("ZS1", lacking_pin_3(), &["1"]);
    let before = symbol_lib_id(&app);
    request(&mut app, PlacedPartAction::ChangeComponent);
    pick_part(&mut app, &new);
    assert!(preview_of(&app).preview.unmatched_connected.is_empty());

    // A wire lands on pin 3 while the preview is open.
    wire_pins(&mut app, &["3"]);
    send(&mut app, PlacedPartMsg::ChangeConfirmed);

    assert_eq!(symbol_lib_id(&app), before, "nothing was committed");
    assert_eq!(
        preview_of(&app).preview.unmatched_connected,
        ["3".to_string()],
        "the preview was shown again with the new wire in it"
    );
    assert!(logged(&["Review the changed pins", "ZS1"]));

    // Confirming what is now on screen goes ahead.
    send(&mut app, PlacedPartMsg::ChangeConfirmed);
    assert_ne!(symbol_lib_id(&app), before);
}

#[test]
fn a_part_read_for_a_row_that_is_no_longer_picked_is_dropped() {
    let (mut app, new) = change_setup("ZR1", renumbered(), &[]);
    request(&mut app, PlacedPartAction::ChangeComponent);
    select_row(&mut app, &new);
    let task = app.update(Message::Library(LibraryMessage::PlacedPart(
        PlacedPartMsg::ChangePartPicked,
    )));
    // The user picks another row while the read of the first is in flight.
    let old_library = app.library.open_libraries[0].library_id;
    let other = app
        .library
        .open_libraries
        .iter()
        .find(|l| l.library_id == old_library)
        .and_then(|l| {
            l.cached_components
                .first()
                .map(|c| (l.root.clone(), c.clone()))
        });
    app.library.picker.as_mut().unwrap().selected = other;

    settle_task(&mut app, task);

    assert!(matches!(
        app.library.picker.as_ref().unwrap().mode,
        PickerMode::ChangeComponent(_)
    ));
}

#[test]
fn the_part_data_of_the_new_row_replaces_the_old_and_user_parameters_stay() {
    let (mut app, new) = change_setup("ZP1", renumbered(), &[]);
    set_symbol(&mut app, |s| {
        s.footprint = "OLD-FOOTPRINT".into();
        s.fields.insert("MPN".into(), "OLD-MPN".into());
        s.fields.insert("Tolerance".into(), "1%".into());
    });
    request(&mut app, PlacedPartAction::ChangeComponent);
    pick_part(&mut app, &new);
    let shown = preview_of(&app).preview;
    assert!(shown.replaced.iter().any(|r| r == "Footprint"));
    assert!(shown.kept.iter().any(|k| k == "Tolerance"));

    send(&mut app, PlacedPartMsg::ChangeConfirmed);

    let sym = &doc(&app).symbols[0];
    assert_eq!(sym.footprint, "", "the old part's footprint is gone");
    assert_eq!(sym.fields.get("MPN").map(String::as_str), Some("TWO-1"));
    assert_eq!(
        sym.fields.get("Manufacturer").map(String::as_str),
        Some("Acme")
    );
    assert_eq!(sym.fields.get("Tolerance").map(String::as_str), Some("1%"));
    assert_eq!(sym.reference, "ZP1");
}

#[test]
fn a_change_that_loses_connections_says_which_pins_in_messages() {
    let (mut app, new) = change_setup("ZW1", lacking_pin_3(), &["1", "3"]);
    request(&mut app, PlacedPartAction::ChangeComponent);
    pick_part(&mut app, &new);

    send(&mut app, PlacedPartMsg::ChangeConfirmed);

    assert!(logged(&["Check the wires of ZW1", "dangling 3"]));
}

// ---- the Properties panel buttons reach the same flows ----

fn panel_action(app: &mut Signex, action: PlacedPartAction) {
    settle(
        app,
        Message::Dock(DockMessage::Panel(PanelMsg::PlacedPartAction(action))),
    );
}

#[test]
fn the_properties_buttons_reach_all_three_flows() {
    let f = fixture();
    let path = symbol_file(&f);
    let mut app = placed_part(&f, "ZB1");

    panel_action(&mut app, PlacedPartAction::OpenDatasheet);
    assert!(logged(&["Add a datasheet to the library row of ZB1"]));

    panel_action(&mut app, PlacedPartAction::EditSymbol);
    assert_eq!(symbol_editor_paths(&app), vec![path]);

    // The symbol editor tab is active now; go back to the sheet.
    app.document_state.active_tab = 0;
    let (mut app2, _new) = change_setup("ZB2", renumbered(), &[]);
    panel_action(&mut app2, PlacedPartAction::ChangeComponent);
    assert!(matches!(
        app2.library.picker.as_ref().unwrap().mode,
        PickerMode::ChangeComponent(_)
    ));
}

/// A project owning the active sheet and `sheet-two.snxsch`; returns the
/// second sheet's path.
fn join_project_with_a_second_sheet(app: &mut Signex) -> PathBuf {
    let sheet_entry = |filename: &str| SheetEntry {
        name: filename.to_string(),
        filename: filename.to_string(),
        symbols_count: 0,
        wires_count: 0,
        labels_count: 0,
        unknown: Default::default(),
    };
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: PathBuf::from("proj.snxprj"),
        data: ProjectData {
            description: None,
            name: "proj".into(),
            dir: String::new(),
            schematic_root: Some(SHEET.into()),
            pcb_file: None,
            sheets: vec![sheet_entry(SHEET), sheet_entry("sheet-two.snxsch")],
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
            net_classes: Vec::new(),
            rules: None,
            annotation: None,
            parameters: Default::default(),
            unknown: Default::default(),
        },
        pending_libraries: HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    PathBuf::from("sheet-two.snxsch")
}

#[test]
fn a_row_that_binds_a_footprint_does_not_put_a_third_footprint_format_on_the_symbol() {
    // The netlist, the BOM and the hover card print `Symbol.footprint`
    // verbatim, so it must stay what a fresh placement leaves: empty.
    let (mut app, new) = change_setup_in("lib-new-fp", "ZF1", renumbered(), &[]);
    set_symbol(&mut app, |s| s.footprint = "R_0603".into());
    request(&mut app, PlacedPartAction::ChangeComponent);
    pick_part(&mut app, &new);

    send(&mut app, PlacedPartMsg::ChangeConfirmed);

    assert_eq!(doc(&app).symbols[0].footprint, "");
}

#[test]
fn a_sibling_unit_on_a_sheet_that_is_not_open_refuses_the_change() {
    // The project declares a second page nobody opened, so no engine holds
    // it: units of the part could be there, and a swap would split the part.
    let (mut app, _new) = change_setup("ZM4", renumbered(), &[]);
    join_project_with_a_second_sheet(&mut app);

    request(&mut app, PlacedPartAction::ChangeComponent);

    assert!(logged(&[
        "Open every sheet of the project to change ZM4",
        "sheet-two.snxsch"
    ]));
    assert!(app.library.picker.is_none());
}

#[test]
fn a_locked_sibling_unit_refuses_the_change_before_the_picker_opens() {
    let (mut app, _new) = change_setup("ZL1", renumbered(), &[]);
    add_second_unit(&mut app);
    let mut d = doc(&app).clone();
    d.symbols[1].locked = true;
    app.document_state
        .active_engine_mut()
        .unwrap()
        .set_document(d);
    select_first(&mut app);

    request(&mut app, PlacedPartAction::ChangeComponent);

    assert!(logged(&["Unlock ZL1", "a unit of it is locked"]));
    assert!(app.library.picker.is_none());
}

#[test]
fn a_child_sheet_that_is_neither_open_nor_a_declared_page_refuses_the_change() {
    // A loose root sheet whose sheet symbol points at a file nobody opened:
    // units of the part could sit in it.
    let (mut app, _new) = change_setup("ZH1", renumbered(), &[]);
    let mut d = doc(&app).clone();
    d.child_sheets.push(signex_types::schematic::ChildSheet {
        uuid: Uuid::now_v7(),
        name: "Child".into(),
        filename: "child.snxsch".into(),
        position: Point::new(0.0, 0.0),
        size: (10.0, 10.0),
        stroke_width: 0.0,
        fill: Default::default(),
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    });
    app.document_state
        .active_engine_mut()
        .unwrap()
        .set_document(d);

    // Child references resolve against the sheet's folder, so the sheet
    // needs a real one.
    let old = app.document_state.active_path.clone().unwrap();
    let absolute = PathBuf::from("/proj").join(&old);
    let engine = app.document_state.engines.remove(&old).unwrap();
    app.document_state.engines.insert(absolute.clone(), engine);
    app.document_state.tabs[0].path = absolute.clone();
    app.document_state.active_path = Some(absolute);

    request(&mut app, PlacedPartAction::ChangeComponent);

    assert!(logged(&[
        "Open every sheet of the project to change ZH1",
        "child.snxsch"
    ]));
    assert!(app.library.picker.is_none());
}

/// A one-unit symbol: pins on unit 1 only, so it cannot have sibling units.
fn single_unit_old() -> Fixture {
    fixture_with_symbol("lib-old-single", unit_one_only())
}

#[test]
fn a_single_unit_part_changes_even_with_an_unopened_page_in_the_project() {
    let (mut app, _new) = change_setup_from(single_unit_old(), "lib-new", "ZU1", renumbered(), &[]);
    join_project_with_a_second_sheet(&mut app);

    request(&mut app, PlacedPartAction::ChangeComponent);

    assert!(
        matches!(
            app.library.picker.as_ref().map(|p| &p.mode),
            Some(PickerMode::ChangeComponent(_))
        ),
        "a resistor-like part needs no other sheet open"
    );
}

#[test]
fn a_sibling_that_appears_on_another_open_sheet_after_the_preview_refuses_the_confirm() {
    let (mut app, new) = change_setup("ZK1", renumbered(), &[]);
    let other_sheet = join_project_with_a_second_sheet(&mut app);
    let mut empty = doc(&app).clone();
    empty.symbols.clear();
    app.document_state.engines.insert(
        other_sheet.clone(),
        signex_engine::Engine::new(empty).expect("engine"),
    );
    let before = symbol_lib_id(&app);
    request(&mut app, PlacedPartAction::ChangeComponent);
    pick_part(&mut app, &new);
    assert!(
        matches!(
            app.library.picker.as_ref().map(|p| &p.mode),
            Some(PickerMode::ChangePreview(_))
        ),
        "no sibling yet: the preview is shown"
    );

    // Unit 2 of the part shows up on the other open sheet.
    let mut unit2 = doc(&app).symbols[0].clone();
    unit2.uuid = Uuid::now_v7();
    unit2.unit = 2;
    let mut d = app.document_state.engines[&other_sheet].document().clone();
    d.symbols.push(unit2);
    app.document_state
        .engines
        .get_mut(&other_sheet)
        .unwrap()
        .set_document(d);
    send(&mut app, PlacedPartMsg::ChangeConfirmed);

    assert!(logged(&["units of ZK1 are also on", "sheet-two.snxsch"]));
    assert_eq!(symbol_lib_id(&app), before, "nothing was changed");
    assert!(app.library.picker.is_none());
}

#[test]
fn a_duplicate_designator_with_the_same_unit_on_another_open_sheet_does_not_refuse() {
    // ZD9 unit 1 here and ZD9 unit 1 there: a duplicate designator, which is
    // another part, not a sibling unit.
    let (mut app, _new) = change_setup("ZD9", renumbered(), &[]);
    let other_sheet = join_project_with_a_second_sheet(&mut app);
    let mut there = doc(&app).clone();
    there.symbols[0].uuid = Uuid::now_v7();
    app.document_state.engines.insert(
        other_sheet,
        signex_engine::Engine::new(there).expect("engine"),
    );

    request(&mut app, PlacedPartAction::ChangeComponent);

    assert!(matches!(
        app.library.picker.as_ref().map(|p| &p.mode),
        Some(PickerMode::ChangeComponent(_))
    ));
}

#[test]
fn a_drawing_that_is_not_embedded_counts_as_possibly_multi_unit() {
    // Nothing says the part is single-unit, so the unopened page matters.
    let (mut app, _new) = change_setup_from(single_unit_old(), "lib-new", "ZN1", renumbered(), &[]);
    let mut d = doc(&app).clone();
    d.lib_symbols = Default::default();
    app.document_state
        .active_engine_mut()
        .unwrap()
        .set_document(d);
    join_project_with_a_second_sheet(&mut app);

    request(&mut app, PlacedPartAction::ChangeComponent);

    assert!(logged(&["Open every sheet of the project to change ZN1"]));
    assert!(app.library.picker.is_none());
}

#[test]
fn an_unannotated_part_skips_the_cross_sheet_check() {
    let (mut app, _new) = change_setup("U?", renumbered(), &[]);
    join_project_with_a_second_sheet(&mut app);

    request(&mut app, PlacedPartAction::ChangeComponent);

    assert!(
        matches!(
            app.library.picker.as_ref().map(|p| &p.mode),
            Some(PickerMode::ChangeComponent(_))
        ),
        "a U? part is a part of its own"
    );
}

// ---- #813 Edit Symbol on a part from the stock library ----

#[test]
fn edit_symbol_on_a_stock_part_edits_a_copy_in_the_picked_library() {
    use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
    use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
    use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};

    let f = fixture();
    let stock_file = symbol_file(&f);
    let stock_before = std::fs::read(&stock_file).unwrap();
    let mut app = placed_part(&f, "ZE813");
    // The fixture library plays the stock library.
    app.library.stock_library_root = Some(f.snxlib.clone());
    let mine_dir = tempfile::tempdir().unwrap();
    let mine = mine_dir.path().join("Mine.snxlib");
    LocalGitAdapter::init(
        &mine,
        SnxlibManifest {
            format: FORMAT_TOKEN.into(),
            library_id: Uuid::now_v7(),
            library: LibrarySection {
                name: "Mine".into(),
                description: None,
            },
            mode: LibraryMode::default(),
            workflow: WorkflowConfig::default(),
            users: UsersConfig::default(),
            classes: Vec::new(),
        },
        LibraryInitOptions {
            enable_git: false,
            use_lfs: false,
        },
    )
    .unwrap();
    app.library.open_library(mine.clone()).unwrap();

    request(&mut app, PlacedPartAction::EditSymbol);

    assert!(
        symbol_editor_paths(&app).is_empty(),
        "the stock file is not opened"
    );
    assert!(app.library.stock_copy.is_some(), "the dialog asks where");

    settle(
        &mut app,
        Message::Library(LibraryMessage::StockCopy(
            signex_app::library::stock_copy::StockCopyMsg::TargetPicked(mine.clone()),
        )),
    );

    let editors = symbol_editor_paths(&app);
    assert_eq!(editors.len(), 1);
    assert!(editors[0].starts_with(mine_dir.path()), "{editors:?}");
    assert_eq!(std::fs::read(&stock_file).unwrap(), stock_before);
}
