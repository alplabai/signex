//! #1264 — rubber-stamp copy keeps the copy on the cursor until Esc, each
//! click its own undo step; Paste Array places a counted, spaced copy of the
//! clipboard as one undo step with stepped label numbers.

use signex_app::app::{EditMsg, InputMsg, Message, PasteIntent, Signex};
use signex_app::paste_array::PasteArrayMsg;
use signex_app::paste_prefs::PasteDesignators;
use signex_types::coord::Unit;

use crate::paste_support::answer;
use crate::paste_support::sheet::{app_with_sheet, click, populated_sheet, references, select};

fn symbol_count(app: &Signex) -> usize {
    app.document_state
        .active_engine()
        .unwrap()
        .document()
        .symbols
        .len()
}

fn escape(app: &mut Signex) {
    let window = app.ui_state.main_window_id;
    let _ = app.update(Message::Input(InputMsg::EscapePressed { window }));
}

fn undo(app: &mut Signex) {
    let _ = app.update(Message::Edit(EditMsg::Undo));
}

#[test]
fn a_rubber_stamp_places_a_copy_per_click_until_escape_with_one_undo_each() {
    let mut app = app_with_sheet(populated_sheet());
    select(&mut app, &[("R1", 1)], false);
    let _ = app.update(Message::Edit(EditMsg::RubberStampCopy));
    assert!(app.interaction_state.placement.is_some(), "stamp armed");

    for (n, at) in [(60.96, 60.96), (76.2, 60.96), (91.44, 60.96)]
        .into_iter()
        .enumerate()
    {
        click(&mut app, at);
        assert_eq!(symbol_count(&app), 6 + n, "one copy per click");
        assert!(
            app.interaction_state.placement.is_some(),
            "the stamp stays on the cursor"
        );
    }

    escape(&mut app);
    assert!(app.interaction_state.placement.is_none());
    click(&mut app, (106.68, 60.96));
    assert_eq!(symbol_count(&app), 8, "nothing is placed after Esc");

    for expected in [7, 6, 5] {
        undo(&mut app);
        assert_eq!(symbol_count(&app), expected, "each click is its own step");
    }
}

#[test]
fn stamped_parts_are_numbered_per_click_by_the_preference() {
    let mut app = app_with_sheet(populated_sheet());
    app.ui_state.paste_designators = PasteDesignators::NextFree;
    select(&mut app, &[("R1", 1)], false);
    let _ = app.update(Message::Edit(EditMsg::RubberStampCopy));
    for x in [60.96, 76.2, 91.44] {
        click(&mut app, (x, 60.96));
    }
    assert_eq!(
        references(&app)[5..],
        ["R4", "R5", "R6"],
        "every stamp takes the next free number"
    );
}

fn open_array(app: &mut Signex) {
    let _ = app.update(Message::Edit(EditMsg::PasteArrayRequested));
    answer(app, PasteIntent::Array, None);
}

fn type_into_dialog(app: &mut Signex, count: &str, dx: &str, dy: &str, step: &str) {
    for msg in [
        PasteArrayMsg::CountChanged(count.to_owned()),
        PasteArrayMsg::XSpacingChanged(dx.to_owned()),
        PasteArrayMsg::YSpacingChanged(dy.to_owned()),
        PasteArrayMsg::StepChanged(step.to_owned()),
    ] {
        let _ = app.update(Message::Edit(EditMsg::PasteArray(msg)));
    }
}

fn app_with_copied_part_and_label() -> Signex {
    let mut app = app_with_sheet(populated_sheet());
    app.ui_state.unit = Unit::Mm;
    select(&mut app, &[("R1", 1)], true);
    let _ = app.update(Message::Edit(EditMsg::Copy));
    app
}

#[test]
fn paste_array_places_counted_spaced_copies_as_one_undo_step() {
    let mut app = app_with_copied_part_and_label();
    app.ui_state.paste_designators = PasteDesignators::NextFree;
    open_array(&mut app);
    assert!(app.ui_state.paste_array.is_some(), "the dialog is open");
    type_into_dialog(&mut app, "3", "10", "5", "2");
    let _ = app.update(Message::Edit(EditMsg::PasteArray(
        PasteArrayMsg::ApplyPressed,
    )));
    assert!(app.ui_state.paste_array.is_none(), "OK closes the dialog");

    let doc = app.document_state.active_engine().unwrap().document();
    let original = doc.symbols[0].position;
    assert_eq!(doc.symbols.len(), 5 + 3);
    for (k, symbol) in doc.symbols[5..].iter().enumerate() {
        let k = (k + 1) as f64;
        assert!((symbol.position.x - (original.x + 10.0 * k)).abs() < 1e-9);
        assert!((symbol.position.y - (original.y + 5.0 * k)).abs() < 1e-9);
    }
    let labels: Vec<_> = doc.labels.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(labels, ["D0", "D2", "D4", "D6"], "k times the step");
    assert_eq!(references(&app)[5..], ["R4", "R5", "R6"]);

    undo(&mut app);
    let doc = app.document_state.active_engine().unwrap().document();
    assert_eq!((doc.symbols.len(), doc.labels.len()), (5, 1), "one undo");
}

#[test]
fn paste_array_reads_the_spacing_in_the_active_unit() {
    let mut app = app_with_copied_part_and_label();
    app.ui_state.unit = Unit::Mil;
    open_array(&mut app);
    type_into_dialog(&mut app, "1", "100", "0", "1");
    let _ = app.update(Message::Edit(EditMsg::PasteArray(
        PasteArrayMsg::ApplyPressed,
    )));
    let doc = app.document_state.active_engine().unwrap().document();
    assert!((doc.symbols[5].position.x - (doc.symbols[0].position.x + 2.54)).abs() < 1e-9);
}

#[test]
fn invalid_paste_array_input_places_nothing_and_says_why() {
    let _ = signex_app::diagnostics::init_logging();
    for (count, dx, reason) in [
        ("0", "10", "Copies must be a whole number"),
        ("1001", "10", "Copies must be a whole number"),
        ("3", "nan", "X spacing must be a number"),
        ("3", "inf", "X spacing must be a number"),
    ] {
        let mut app = app_with_copied_part_and_label();
        open_array(&mut app);
        type_into_dialog(&mut app, count, dx, "0", "1");
        let verdict = app.ui_state.paste_array.as_ref().unwrap().input();
        assert!(verdict.unwrap_err().contains(reason), "{count} {dx}");
        let _ = app.update(Message::Edit(EditMsg::PasteArray(
            PasteArrayMsg::ApplyPressed,
        )));
        assert_eq!(symbol_count(&app), 5, "nothing placed for {count} {dx}");
        assert!(
            app.ui_state.paste_array.is_some(),
            "the dialog stays open to be fixed"
        );
    }
    assert!(
        signex_app::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains("Paste Array: Copies must be")),
        "the refusal reaches the Messages panel"
    );
}

#[test]
fn escape_closes_the_paste_array_dialog() {
    let mut app = app_with_copied_part_and_label();
    open_array(&mut app);
    escape(&mut app);
    assert!(app.ui_state.paste_array.is_none());
    assert_eq!(symbol_count(&app), 5);
}

#[test]
fn paste_array_with_an_empty_clipboard_opens_nothing() {
    let _ = signex_app::diagnostics::init_logging();
    let mut app = app_with_sheet(populated_sheet());
    open_array(&mut app);
    assert!(app.ui_state.paste_array.is_none());
}
