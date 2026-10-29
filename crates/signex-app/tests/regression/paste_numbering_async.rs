//! #1263 / #1264 — Next free numbers are read off the UI thread, once, from
//! every sheet of the project; a paste answered after the tool or the sheet
//! moved on is dropped; Paste Array carries its own designator choice.

use signex_app::app::{EditMsg, Message, PasteIntent, Signex};
use signex_app::paste_array::PasteArrayMsg;
use signex_app::paste_prefs::PasteDesignators;
use signex_app::tab_bar::TabMessage;
use signex_types::schematic::{ChildSheet, Point, SchematicSheet};
use uuid::Uuid;

use crate::paste_support::sheet::{
    app_with_sheet, app_with_sheet_at, click, part, populated_sheet, references, select,
};
use crate::paste_support::{
    answer, answer_request, paste, request, send_and_settle, settle, written_text,
};
use crate::placement_session::second_sheet;

fn symbol_count(app: &Signex) -> usize {
    app.document_state
        .active_engine()
        .unwrap()
        .document()
        .symbols
        .len()
}

fn child_ref(filename: &str) -> ChildSheet {
    ChildSheet {
        uuid: Uuid::new_v4(),
        name: filename.to_owned(),
        filename: filename.to_owned(),
        position: Point::new(0.0, 0.0),
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

/// Sheet A (open, R1-R3 and a two-unit U1) whose child sheet on disk holds
/// `child_text`. The child is not open anywhere.
fn app_with_child_on_disk(
    child_name: &str,
    child_text: impl FnOnce() -> String,
) -> (Signex, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(child_name), child_text()).unwrap();
    let mut sheet = populated_sheet();
    sheet.child_sheets.push(child_ref(child_name));
    let app = app_with_sheet_at(dir.path().join("a.snxsch"), sheet);
    (app, dir)
}

fn child_with_r9() -> String {
    let mut child = SchematicSheet {
        uuid: Uuid::new_v4(),
        paper_size: "A4".to_owned(),
        ..Default::default()
    };
    child.symbols.push(part("R9", "lib:r", 1, (10.16, 10.16)));
    signex_types::format::SnxSchematic::new(child)
        .write_string()
        .unwrap()
}

fn paste_one_resistor(app: &mut Signex) -> String {
    select(app, &[("R2", 1)], false);
    let _ = app.update(Message::Edit(EditMsg::Copy));
    paste(app);
    click(app, (80.0, 80.0));
    references(app).pop().unwrap()
}

#[test]
fn next_free_counts_a_project_sheet_that_is_not_open() {
    let (mut app, _dir) = app_with_child_on_disk("child.snxsch", child_with_r9);
    app.ui_state.paste_designators = PasteDesignators::NextFree;
    assert_eq!(paste_one_resistor(&mut app), "R10");
}

#[test]
fn an_unreadable_project_sheet_is_named_in_the_messages_panel() {
    let _ = signex_app::diagnostics::init_logging();
    let (mut app, _dir) = app_with_child_on_disk("unreadable-child-1263.snxsch", || {
        "this is not a schematic".to_owned()
    });
    app.ui_state.paste_designators = PasteDesignators::NextFree;
    paste_one_resistor(&mut app);
    assert!(
        signex_app::diagnostics::recent_entries().iter().any(|e| {
            e.level == signex_app::diagnostics::DiagnosticLevel::Warning
                && e.message.contains("unreadable-child-1263.snxsch")
                && e.message.contains("could not be read")
        }),
        "the sheet that could not be counted is named"
    );
}

#[test]
fn an_edit_made_after_the_numbers_were_read_is_still_counted() {
    let mut app = app_with_sheet(populated_sheet());
    app.ui_state.paste_designators = PasteDesignators::NextFree;
    select(&mut app, &[("R2", 1)], false);
    let _ = app.update(Message::Edit(EditMsg::Copy));
    let task = app.update(Message::Edit(EditMsg::Paste));
    crate::paste_support::settle(&mut app, task, true);
    // A part with a high number lands on the sheet while the paste waits.
    let engine = app.document_state.engines.values_mut().next().unwrap();
    engine
        .execute(signex_engine::Command::PlaceSymbol {
            symbol: part("R20", "lib:r", 1, (60.0, 60.0)),
        })
        .unwrap();
    answer(&mut app, PasteIntent::Paste, None);
    click(&mut app, (80.0, 80.0));
    assert_eq!(references(&app).pop().unwrap(), "R21");
}

#[test]
fn a_stamp_advances_the_numbers_in_memory_click_after_click() {
    let (mut app, _dir) = app_with_child_on_disk("child.snxsch", child_with_r9);
    app.ui_state.paste_designators = PasteDesignators::NextFree;
    select(&mut app, &[("R2", 1)], false);
    send_and_settle(&mut app, EditMsg::RubberStampCopy, PasteIntent::Stamp);
    for x in [60.0, 70.0, 80.0] {
        click(&mut app, (x, 60.0));
    }
    assert_eq!(references(&app)[5..], ["R10", "R11", "R12"]);
}

#[test]
fn a_paste_answered_after_escape_is_dropped() {
    let mut app = app_with_sheet(populated_sheet());
    select(&mut app, &[("R2", 1)], false);
    let _ = app.update(Message::Edit(EditMsg::Copy));
    let stale = request(&app, PasteIntent::Paste);
    let _ = app.update(Message::Edit(EditMsg::Paste));
    let window = app.ui_state.main_window_id;
    let _ = app.update(Message::Input(signex_app::app::InputMsg::EscapePressed {
        window,
    }));
    answer_request(&mut app, stale, None);
    assert!(app.interaction_state.placement.is_none(), "Ctrl+V then Esc");
}

#[test]
fn a_paste_answered_after_a_tab_switch_is_dropped() {
    let mut app = app_with_sheet(populated_sheet());
    select(&mut app, &[("R2", 1)], false);
    let _ = app.update(Message::Edit(EditMsg::Copy));
    let stale = request(&app, PasteIntent::Paste);
    let _ = app.update(Message::Edit(EditMsg::Paste));
    let _other = second_sheet(&mut app);
    let window = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(window);
    let _ = app.update(Message::Tab {
        window_id: window,
        msg: TabMessage::Select(1),
    });
    answer_request(&mut app, stale, None);
    assert!(
        app.interaction_state.placement.is_none(),
        "Ctrl+V then a tab switch"
    );
}

#[test]
fn the_paste_array_dialog_closes_when_the_sheet_changes() {
    let mut app = app_with_sheet(populated_sheet());
    select(&mut app, &[("R2", 1)], false);
    let _ = app.update(Message::Edit(EditMsg::Copy));
    send_and_settle(&mut app, EditMsg::PasteArrayRequested, PasteIntent::Array);
    answer(&mut app, PasteIntent::Array, None);
    assert!(app.ui_state.paste_array.is_some());

    let _other = second_sheet(&mut app);
    let window = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(window);
    let _ = app.update(Message::Tab {
        window_id: window,
        msg: TabMessage::Select(1),
    });
    assert!(app.ui_state.paste_array.is_none());
}

fn array_with_choice(global: PasteDesignators, chosen: PasteDesignators) -> Vec<String> {
    let mut app = app_with_sheet(populated_sheet());
    app.ui_state.paste_designators = global;
    select(&mut app, &[("R2", 1)], false);
    let _ = app.update(Message::Edit(EditMsg::Copy));
    send_and_settle(&mut app, EditMsg::PasteArrayRequested, PasteIntent::Array);
    answer(&mut app, PasteIntent::Array, None);
    let state = app.ui_state.paste_array.as_ref().expect("dialog open");
    assert_eq!(state.designators, global, "starts as the saved preference");
    // Choosing Next free reads the project's numbers; OK waits for them.
    let task = app.update(Message::Edit(EditMsg::PasteArray(
        PasteArrayMsg::DesignatorsChosen(chosen),
    )));
    settle(&mut app, task, true);
    for msg in [
        PasteArrayMsg::CountChanged("2".to_owned()),
        PasteArrayMsg::ApplyPressed,
    ] {
        let _ = app.update(Message::Edit(EditMsg::PasteArray(msg)));
    }
    references(&app).split_off(5)
}

#[test]
fn the_paste_array_dialog_has_its_own_designator_choice() {
    assert_eq!(
        array_with_choice(PasteDesignators::Keep, PasteDesignators::Reset),
        ["R?", "R?"]
    );
    assert_eq!(
        array_with_choice(PasteDesignators::Keep, PasteDesignators::NextFree),
        ["R4", "R5"]
    );
    assert_eq!(
        array_with_choice(PasteDesignators::NextFree, PasteDesignators::Keep),
        ["R2", "R2"]
    );
}

#[test]
fn a_copy_that_cannot_be_serialised_empties_the_system_clipboard() {
    let mut sheet = populated_sheet();
    sheet.symbols[1].rotation = f64::NAN;
    let mut app = app_with_sheet(sheet);
    select(&mut app, &[("R2", 1)], false);
    let task = app.update(Message::Edit(EditMsg::Copy));
    assert_eq!(
        written_text(task).as_deref(),
        Some(""),
        "a stale older fragment must not stay pasteable"
    );
    assert!(
        app.interaction_state.clipboard.is_some(),
        "still pastes here"
    );
}

#[test]
fn a_fifty_copy_array_places_every_copy_and_label() {
    let mut app = app_with_sheet(populated_sheet());
    select(&mut app, &[("R2", 1)], true);
    let _ = app.update(Message::Edit(EditMsg::Copy));
    send_and_settle(&mut app, EditMsg::PasteArrayRequested, PasteIntent::Array);
    answer(&mut app, PasteIntent::Array, None);
    for msg in [
        PasteArrayMsg::CountChanged("50".to_owned()),
        PasteArrayMsg::ApplyPressed,
    ] {
        let _ = app.update(Message::Edit(EditMsg::PasteArray(msg)));
    }
    assert_eq!(symbol_count(&app), 55);
    let doc = app.document_state.active_engine().unwrap().document();
    assert_eq!(doc.labels.len(), 51);
    assert!(doc.labels.iter().all(|l| !l.text.is_empty()));
}

/// The child sheet is open with an unsaved `R15` while the copy on disk
/// still holds `R9`: the open one is the truth.
#[test]
fn an_open_project_sheet_counts_with_its_unsaved_edits() {
    let (mut app, dir) = app_with_child_on_disk("child.snxsch", child_with_r9);
    let mut unsaved = SchematicSheet {
        uuid: Uuid::new_v4(),
        paper_size: "A4".to_owned(),
        ..Default::default()
    };
    unsaved
        .symbols
        .push(part("R15", "lib:r", 1, (10.16, 10.16)));
    app.document_state.engines.insert(
        dir.path().join("child.snxsch"),
        signex_engine::Engine::new(unsaved).expect("engine"),
    );
    app.ui_state.paste_designators = PasteDesignators::NextFree;
    assert_eq!(paste_one_resistor(&mut app), "R16");
}

/// A fragment from another process onto a sheet that lacks its body: the
/// array registers the body, once, and places every copy.
#[test]
fn a_paste_array_of_a_foreign_fragment_brings_its_library_body() {
    let mut source = app_with_sheet(populated_sheet());
    select(&mut source, &[("R2", 1)], false);
    let text = written_text(source.update(Message::Edit(EditMsg::Copy))).unwrap();

    let mut empty = crate::paste_support::sheet::empty_sheet();
    empty.lib_symbols.clear();
    let mut app = app_with_sheet(empty);
    app.document_state.engines.values_mut().for_each(|e| {
        let mut sheet = e.document().clone();
        sheet.lib_symbols.clear();
        e.set_document(sheet);
    });
    send_and_settle(&mut app, EditMsg::PasteArrayRequested, PasteIntent::Array);
    answer(&mut app, PasteIntent::Array, Some(&text));
    for msg in [
        PasteArrayMsg::CountChanged("3".to_owned()),
        PasteArrayMsg::ApplyPressed,
    ] {
        let _ = app.update(Message::Edit(EditMsg::PasteArray(msg)));
    }
    let doc = app.document_state.active_engine().unwrap().document();
    assert_eq!(doc.symbols.len(), 3);
    assert_eq!(doc.lib_symbols.len(), 1);
    assert!(doc.lib_symbols.contains_key("lib:r"));
}

/// A new paste request must not strip the numbers a Next free stamp is
/// still armed with: a click before the new numbers land would reuse R4.
#[test]
fn a_new_request_does_not_strip_an_armed_stamps_project_numbers() {
    let (mut app, _dir) = app_with_child_on_disk("child.snxsch", || {
        let mut child = SchematicSheet {
            uuid: Uuid::new_v4(),
            paper_size: "A4".to_owned(),
            ..Default::default()
        };
        child.symbols.push(part("R4", "lib:r", 1, (10.16, 10.16)));
        signex_types::format::SnxSchematic::new(child)
            .write_string()
            .unwrap()
    });
    app.ui_state.paste_designators = PasteDesignators::NextFree;
    select(&mut app, &[("R2", 1)], false);
    send_and_settle(&mut app, EditMsg::RubberStampCopy, PasteIntent::Stamp);
    // A second request starts, and its numbers have not landed yet.
    let _unsettled = app.update(Message::Edit(EditMsg::Paste));
    click(&mut app, (60.0, 60.0));
    assert_eq!(references(&app).pop().unwrap(), "R5");
}
