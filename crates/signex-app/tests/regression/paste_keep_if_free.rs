//! #1768: with the default "Designators on paste" preference a pasted part
//! keeps its designator when the project does not use it and otherwise takes
//! the next free number; the original is never renumbered.

use signex_app::app::{EditMsg, PasteIntent, Signex};
use signex_app::paste_prefs::PasteDesignators;

use crate::paste_support::sheet::{
    app_with_sheet, click, empty_sheet, part, populated_sheet, references, select,
};
use crate::paste_support::{paste, send_and_settle, smart_paste};

const ORIGINAL: [&str; 5] = ["R1", "R2", "R3", "U1", "U1"];

fn app_with_r2_and_u1_selected() -> Signex {
    let mut app = app_with_sheet(populated_sheet());
    select(&mut app, &[("R2", 1), ("U1", 1), ("U1", 2)], false);
    let _ = app.update(signex_app::app::Message::Edit(EditMsg::Copy));
    app
}

fn pasted(app: &Signex) -> Vec<String> {
    references(app).split_off(ORIGINAL.len())
}

#[test]
fn the_default_keeps_a_free_designator_and_renumbers_a_taken_one() {
    assert_eq!(
        Signex::new().0.ui_state.paste_designators,
        PasteDesignators::KeepIfFree
    );
}

#[test]
fn paste_of_a_taken_designator_takes_the_next_free_number_and_units_stay_together() {
    let mut app = app_with_r2_and_u1_selected();
    paste(&mut app);
    click(&mut app, (80.0, 80.0));
    assert_eq!(
        references(&app)[..ORIGINAL.len()],
        ORIGINAL,
        "originals keep theirs"
    );
    assert_eq!(pasted(&app), ["R4", "U2", "U2"]);
}

#[test]
fn smart_paste_and_duplicate_follow_the_default() {
    let mut app = app_with_r2_and_u1_selected();
    smart_paste(&mut app);
    assert_eq!(pasted(&app), ["R4", "U2", "U2"], "smart paste");

    let mut app = app_with_r2_and_u1_selected();
    send_and_settle(&mut app, EditMsg::Duplicate, PasteIntent::Duplicate);
    assert_eq!(pasted(&app), ["R4", "U2", "U2"], "duplicate");
}

#[test]
fn a_free_designator_is_kept() {
    let mut app = app_with_sheet(populated_sheet());
    select(&mut app, &[("R2", 1)], false);
    let _ = app.update(signex_app::app::Message::Edit(EditMsg::Cut));
    paste(&mut app);
    click(&mut app, (80.0, 80.0));
    assert_eq!(references(&app), ["R1", "R3", "U1", "U1", "R2"]);
}

#[test]
fn an_unannotated_designator_stays_unannotated() {
    let mut sheet = empty_sheet();
    sheet.symbols = vec![part("R?", "lib:r", 1, (10.16, 10.16))];
    let mut app = app_with_sheet(sheet);
    select(&mut app, &[("R?", 1)], false);
    let _ = app.update(signex_app::app::Message::Edit(EditMsg::Copy));
    paste(&mut app);
    click(&mut app, (80.0, 80.0));
    assert_eq!(references(&app), ["R?", "R?"]);
}
