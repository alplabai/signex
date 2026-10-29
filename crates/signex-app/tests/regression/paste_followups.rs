//! #1676 review follow-ups: what Copy and Cut leave on the system clipboard,
//! and Paste Array against a copy that changed under the open dialog.

use signex_app::app::{EditMsg, Message, Signex};
use signex_app::paste_array::PasteArrayMsg;
use signex_app::paste_prefs::PasteDesignators;
use signex_types::schematic::{ChildSheet, Point, SelectedItem, SelectedKind};
use uuid::Uuid;

use crate::paste_support::sheet::{app_with_sheet, part, populated_sheet, references, select};
use crate::paste_support::{reads_clipboard, written_text};

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

#[test]
fn a_keep_mode_array_does_not_read_the_projects_numbers() {
    let mut app = app_with_sheet(populated_sheet());
    app.ui_state.paste_designators = PasteDesignators::Keep;
    select(&mut app, &[("R1", 1)], false);
    let _ = app.update(Message::Edit(EditMsg::Copy));
    let task = app.update(Message::Edit(EditMsg::PasteArrayRequested));
    assert!(
        reads_clipboard(task),
        "the first step is the clipboard read"
    );
}

fn seeded_message(task: iced::Task<Message>) -> Option<Message> {
    crate::library_place_symbol::run(task)
        .into_iter()
        .find(|m| matches!(m, Message::Edit(EditMsg::PasteCountersSeeded { .. })))
}

#[test]
fn choosing_next_free_in_the_dialog_reads_the_numbers_and_ok_waits_for_them() {
    let mut app = app_with_sheet(populated_sheet());
    app.ui_state.paste_designators = PasteDesignators::Keep;
    select(&mut app, &[("R1", 1)], false);
    let _ = app.update(Message::Edit(EditMsg::Copy));
    crate::paste_stamp_array::open_array(&mut app);
    let dialog = |m| Message::Edit(EditMsg::PasteArray(m));

    let task = app.update(dialog(PasteArrayMsg::DesignatorsChosen(
        PasteDesignators::NextFree,
    )));
    let seeded = seeded_message(task).expect("choosing Next free reads the numbers");

    let before = references(&app).len();
    let _ = app.update(dialog(PasteArrayMsg::ApplyPressed));
    assert_eq!(references(&app).len(), before, "OK waits for the numbers");
    assert!(app.ui_state.paste_array.is_some());

    let _ = app.update(seeded);
    let _ = app.update(dialog(PasteArrayMsg::ApplyPressed));
    assert_eq!(references(&app)[before..], ["R4", "R5"]);
}

#[test]
fn choosing_keep_or_reset_in_the_dialog_reads_nothing() {
    let mut app = app_with_sheet(populated_sheet());
    app.ui_state.paste_designators = PasteDesignators::Keep;
    select(&mut app, &[("R1", 1)], false);
    let _ = app.update(Message::Edit(EditMsg::Copy));
    crate::paste_stamp_array::open_array(&mut app);
    for mode in [PasteDesignators::Reset, PasteDesignators::Keep] {
        let task = app.update(Message::Edit(EditMsg::PasteArray(
            PasteArrayMsg::DesignatorsChosen(mode),
        )));
        assert!(seeded_message(task).is_none());
    }
}

fn ghost_references(app: &Signex) -> Vec<String> {
    let session = app.interaction_state.placement.as_ref().expect("armed");
    session
        .ghost
        .fragment()
        .symbols
        .iter()
        .map(|s| s.reference.clone())
        .collect()
}

#[test]
fn the_paste_ghost_shows_the_designators_the_click_will_give() {
    let mut app = app_with_sheet(populated_sheet());
    app.ui_state.paste_designators = PasteDesignators::NextFree;
    select(&mut app, &[("R1", 1)], false);
    let _ = app.update(Message::Edit(EditMsg::Copy));
    crate::paste_support::paste(&mut app);
    assert_eq!(ghost_references(&app), ["R4"], "not the copied R1");

    crate::paste_support::sheet::click(&mut app, (80.0, 80.0));
    assert_eq!(references(&app).last().map(String::as_str), Some("R4"));
}

#[test]
fn a_stamp_ghost_moves_on_to_the_next_number_after_each_click() {
    let mut app = app_with_sheet(populated_sheet());
    app.ui_state.paste_designators = PasteDesignators::NextFree;
    select(&mut app, &[("R1", 1)], false);
    crate::paste_support::send_and_settle(
        &mut app,
        EditMsg::RubberStampCopy,
        signex_app::app::PasteIntent::Stamp,
    );
    assert_eq!(ghost_references(&app), ["R4"]);
    crate::paste_support::sheet::click(&mut app, (80.0, 80.0));
    assert_eq!(ghost_references(&app), ["R5"]);
}

#[test]
fn a_reset_paste_ghost_shows_the_unnumbered_designator() {
    let mut app = app_with_sheet(populated_sheet());
    app.ui_state.paste_designators = PasteDesignators::Reset;
    select(&mut app, &[("R1", 1)], false);
    let _ = app.update(Message::Edit(EditMsg::Copy));
    crate::paste_support::paste(&mut app);
    assert_eq!(ghost_references(&app), ["R?"]);
}

fn app_with_part_at(x: f64) -> Signex {
    let mut sheet = populated_sheet();
    sheet.symbols = vec![part("R1", "lib:r", 1, (x, 10.16))];
    sheet.labels.clear();
    app_with_sheet(sheet)
}

#[test]
fn a_paste_keeps_the_anchor_of_a_copy_made_under_another_grid() {
    let mut app = app_with_part_at(14.0);
    app.ui_state.grid_size_mm = 2.54;
    select(&mut app, &[("R1", 1)], false);
    let text = written_text(app.update(Message::Edit(EditMsg::Copy))).unwrap();
    let anchor = app.interaction_state.clipboard.as_ref().unwrap().origin;

    app.ui_state.grid_size_mm = 1.27;
    crate::paste_support::paste_with_clipboard(&mut app, Some(&text));
    let kept = app.interaction_state.clipboard.as_ref().unwrap().origin;
    assert_eq!(kept, anchor, "the grid changed, the anchor must not");
}

#[test]
fn a_foreign_fragment_replaces_the_copy_and_an_older_text_of_ours_is_read_again() {
    let mut app = app_with_part_at(14.0);
    select(&mut app, &[("R1", 1)], false);
    let ours = written_text(app.update(Message::Edit(EditMsg::Copy))).unwrap();

    let mut other = app_with_part_at(40.0);
    select(&mut other, &[("R1", 1)], false);
    let foreign = written_text(other.update(Message::Edit(EditMsg::Copy))).unwrap();
    let foreign_origin = other.interaction_state.clipboard.as_ref().unwrap().origin;
    let our_origin = app.interaction_state.clipboard.as_ref().unwrap().origin;
    assert_ne!(foreign_origin, our_origin);

    crate::paste_support::paste_with_clipboard(&mut app, Some(&foreign));
    assert_eq!(
        app.interaction_state.clipboard.as_ref().unwrap().origin,
        foreign_origin
    );

    crate::paste_support::paste_with_clipboard(&mut app, Some(&ours));
    assert_eq!(
        app.interaction_state.clipboard.as_ref().unwrap().origin,
        our_origin
    );
}
