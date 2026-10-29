//! #1263 — Copy and Cut put the fragment on the system clipboard as text, a
//! paste reads it back (another Signex process can paste it), and the
//! "Designators on paste" preference decides what pasted parts are called.

use signex_app::app::{EditMsg, Message, Signex};
use signex_app::paste_prefs::PasteDesignators;
use signex_engine::CLIPBOARD_MARKER;

use crate::paste_support::sheet::{
    app_with_sheet, click, empty_sheet, populated_sheet, references, select,
};
use crate::paste_support::{
    paste, paste_with_clipboard, reads_clipboard, smart_paste, written_text,
};

/// The selection that exercises every designator mode: a resistor and the
/// two units of U1.
fn app_with_copied_parts() -> (Signex, String) {
    let mut app = app_with_sheet(populated_sheet());
    select(&mut app, &[("R2", 1), ("U1", 1), ("U1", 2)], false);
    let task = app.update(Message::Edit(EditMsg::Copy));
    let text = written_text(task).expect("Copy writes to the system clipboard");
    (app, text)
}

fn pasted_references(app: &Signex) -> Vec<String> {
    references(app).split_off(5)
}

#[test]
fn copy_writes_the_fragment_to_the_system_clipboard_as_marked_text() {
    let (_app, text) = app_with_copied_parts();
    let (first, body) = text.split_once('\n').unwrap();
    assert_eq!(first, CLIPBOARD_MARKER);
    assert!(
        body.contains("lib:dual"),
        "the library body travels with it"
    );
}

#[test]
fn cut_writes_the_fragment_too_and_removes_the_selection() {
    let mut app = app_with_sheet(populated_sheet());
    select(&mut app, &[("R3", 1)], false);
    let task = app.update(Message::Edit(EditMsg::Cut));
    let text = written_text(task).expect("Cut writes to the system clipboard");
    assert!(text.starts_with(CLIPBOARD_MARKER));
    assert_eq!(references(&app), ["R1", "R2", "U1", "U1"]);
}

#[test]
fn a_paste_request_asks_the_system_clipboard() {
    let (mut app, _text) = app_with_copied_parts();
    for message in [
        EditMsg::Paste,
        EditMsg::SmartPaste,
        EditMsg::PasteArrayRequested,
    ] {
        let task = app.update(Message::Edit(message));
        assert!(reads_clipboard(task));
    }
}

#[test]
fn a_fragment_copied_in_another_process_pastes_with_its_bodies() {
    let (_first, text) = app_with_copied_parts();

    let mut second = app_with_sheet(empty_sheet());
    second.document_state.engines.values_mut().for_each(|e| {
        let mut sheet = e.document().clone();
        sheet.lib_symbols.clear();
        e.set_document(sheet);
    });
    assert!(second.interaction_state.clipboard.is_none());
    paste_with_clipboard(&mut second, Some(&text));
    click(&mut second, (50.8, 50.8));

    let doc = second.document_state.active_engine().unwrap().document();
    assert_eq!(doc.symbols.len(), 3);
    assert!(doc.lib_symbols.contains_key("lib:dual"));
    assert!(doc.lib_symbols.contains_key("lib:r"));
}

#[test]
fn marked_text_that_cannot_be_read_falls_back_with_a_warning() {
    let _ = signex_app::diagnostics::init_logging();
    let (mut app, _text) = app_with_copied_parts();
    paste_with_clipboard(
        &mut app,
        Some(&format!("{CLIPBOARD_MARKER}\nbroken-fragment-1263")),
    );
    click(&mut app, (50.8, 50.8));
    assert_eq!(pasted_references(&app).len(), 3, "the in-app copy was used");
    assert!(
        signex_app::diagnostics::recent_entries().iter().any(|e| {
            e.level == signex_app::diagnostics::DiagnosticLevel::Warning
                && e.message.contains("fragment that could not be read")
        }),
        "the Messages panel says why the clipboard text was not used"
    );
}

#[test]
fn text_that_is_not_a_fragment_falls_back_to_the_in_app_copy() {
    let (mut app, _text) = app_with_copied_parts();
    paste_with_clipboard(&mut app, Some("just some text"));
    click(&mut app, (50.8, 50.8));
    assert_eq!(pasted_references(&app).len(), 3);
}

#[test]
fn the_system_clipboard_wins_over_an_older_in_app_copy() {
    let (mut app, _text) = app_with_copied_parts();
    select(&mut app, &[("R1", 1)], false);
    let newer = written_text(app.update(Message::Edit(EditMsg::Copy))).unwrap();
    // The window's own copy is then replaced by what the clipboard now holds.
    select(&mut app, &[("R3", 1)], false);
    let _ = app.update(Message::Edit(EditMsg::Copy));
    paste_with_clipboard(&mut app, Some(&newer));
    click(&mut app, (50.8, 50.8));
    assert_eq!(pasted_references(&app), ["R1"]);
}

fn paste_with_mode(mode: PasteDesignators) -> Vec<String> {
    let (mut app, _text) = app_with_copied_parts();
    app.ui_state.paste_designators = mode;
    paste(&mut app);
    click(&mut app, (80.0, 80.0));
    pasted_references(&app)
}

#[test]
fn keep_leaves_the_designators_of_the_copy() {
    assert_eq!(paste_with_mode(PasteDesignators::Keep), ["R2", "U1", "U1"]);
}

#[test]
fn reset_gives_unnumbered_designators_and_units_still_share_one() {
    assert_eq!(paste_with_mode(PasteDesignators::Reset), ["R?", "U?", "U?"]);
}

#[test]
fn next_free_takes_the_next_number_and_units_still_share_one() {
    assert_eq!(
        paste_with_mode(PasteDesignators::NextFree),
        ["R4", "U2", "U2"]
    );
}

#[test]
fn smart_paste_and_duplicate_follow_the_preference() {
    for (mode, expected) in [
        (PasteDesignators::Reset, ["R?", "U?", "U?"]),
        (PasteDesignators::NextFree, ["R4", "U2", "U2"]),
    ] {
        let (mut app, _text) = app_with_copied_parts();
        app.ui_state.paste_designators = mode;
        smart_paste(&mut app);
        assert_eq!(pasted_references(&app), expected, "smart paste, {mode:?}");

        let (mut app, _text) = app_with_copied_parts();
        app.ui_state.paste_designators = mode;
        let _ = app.update(Message::Edit(EditMsg::Duplicate));
        assert_eq!(pasted_references(&app), expected, "duplicate, {mode:?}");
    }
}
