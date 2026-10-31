//! #1255 / #1670 — a part's designator and value (and its footprint, which
//! has no canvas text) edit in place: double-click or the F2 / U / V / F
//! commands open the one inline editor, Enter commits as one undo step.

use crate::field_edit_support::{app_with, doc, empty_sheet, label, logged, part, wire};
use signex_app::app::{EditMsg, Message, Signex, TextEditMsg, TextEditTarget};
use signex_app::canvas::CanvasEvent;
use signex_types::schematic::{SelectedItem, SelectedKind};

const R1_AT: (f64, f64) = (20.0, 40.0);
const R2_AT: (f64, f64) = (80.0, 40.0);

fn app() -> Signex {
    let mut sheet = empty_sheet();
    sheet.symbols = vec![part("R1", "10k", R1_AT), part("R2", "22k", R2_AT)];
    sheet.labels = vec![label("CLK", (20.0, 100.0))];
    sheet.wires = vec![wire((20.0, 130.0), (60.0, 130.0), 0.0)];
    app_with(sheet)
}

fn double_click(app: &mut Signex, at: (f64, f64)) {
    let _ = app.update(Message::CanvasEvent(CanvasEvent::DoubleClicked {
        world_x: at.0,
        world_y: at.1,
        screen_x: 0.0,
        screen_y: 0.0,
    }));
}

fn request(app: &mut Signex, target: TextEditTarget) {
    let _ = app.update(Message::TextEdit(TextEditMsg::Requested(target)));
}

fn type_and_submit(app: &mut Signex, text: &str) {
    let _ = app.update(Message::TextEdit(TextEditMsg::Changed(text.into())));
    let _ = app.update(Message::TextEdit(TextEditMsg::Submit));
}

fn editing(app: &Signex) -> Option<(SelectedKind, String)> {
    app.interaction_state
        .editing_text
        .as_ref()
        .map(|e| (e.kind, e.text.clone()))
}

/// Point the cursor at `at` the way a hover does.
fn hover(app: &mut Signex, at: (f64, f64)) {
    app.ui_state.cursor_x = at.0;
    app.ui_state.cursor_y = at.1;
    app.ui_state.cursor_document = app.document_state.active_path.clone();
}

fn select(app: &mut Signex, items: &[(usize, SelectedKind)]) {
    let ids: Vec<_> = items
        .iter()
        .map(|(i, kind)| SelectedItem::new(doc(app).symbols[*i].uuid, *kind))
        .collect();
    app.interaction_state.active_canvas_mut().selected = ids;
}

#[test]
fn double_clicking_a_designator_edits_it_and_enter_is_one_undo_step() {
    let mut app = app();
    double_click(&mut app, (R1_AT.0, R1_AT.1 - 5.0));
    assert_eq!(
        editing(&app),
        Some((SelectedKind::SymbolRefField, "R1".into()))
    );
    type_and_submit(&mut app, "R10");
    assert_eq!(doc(&app).symbols[0].reference, "R10");
    assert!(app.interaction_state.editing_text.is_none());
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(doc(&app).symbols[0].reference, "R1");
}

#[test]
fn double_clicking_a_value_edits_it() {
    let mut app = app();
    double_click(&mut app, (R2_AT.0, R2_AT.1 + 5.0));
    assert_eq!(
        editing(&app),
        Some((SelectedKind::SymbolValField, "22k".into()))
    );
    type_and_submit(&mut app, "1M");
    assert_eq!(doc(&app).symbols[1].value, "1M");
    assert_eq!(doc(&app).symbols[0].value, "10k");
}

#[test]
fn double_clicking_a_label_still_edits_it() {
    let mut app = app();
    double_click(&mut app, (20.0, 100.0));
    assert_eq!(editing(&app), Some((SelectedKind::Label, "CLK".into())));
    type_and_submit(&mut app, "CLK2");
    assert_eq!(doc(&app).labels[0].text, "CLK2");
}

#[test]
fn escape_cancels_and_changes_nothing() {
    let mut app = app();
    double_click(&mut app, (R1_AT.0, R1_AT.1 - 5.0));
    let _ = app.update(Message::TextEdit(TextEditMsg::Changed("ZZZ".into())));
    let _ = app.update(Message::Input(signex_app::app::InputMsg::EscapePressed {
        window: None,
    }));
    assert!(app.interaction_state.editing_text.is_none());
    assert_eq!(doc(&app).symbols[0].reference, "R1");
}

#[test]
fn an_unchanged_text_records_no_undo_step() {
    let mut app = app();
    double_click(&mut app, (R1_AT.0, R1_AT.1 - 5.0));
    let _ = app.update(Message::TextEdit(TextEditMsg::Submit));
    assert!(!app.document_state.active_engine().unwrap().can_undo());
}

#[test]
fn the_value_command_targets_the_part_under_the_cursor_before_the_selection() {
    let mut app = app();
    select(&mut app, &[(0, SelectedKind::Symbol)]);
    hover(&mut app, (R2_AT.0, R2_AT.1 + 5.0));
    request(&mut app, TextEditTarget::Value);
    assert_eq!(
        editing(&app),
        Some((SelectedKind::SymbolValField, "22k".into()))
    );
}

#[test]
fn the_commands_fall_back_to_the_single_selected_part() {
    let mut app = app();
    select(&mut app, &[(0, SelectedKind::Symbol)]);
    request(&mut app, TextEditTarget::Designator);
    assert_eq!(
        editing(&app),
        Some((SelectedKind::SymbolRefField, "R1".into()))
    );
    type_and_submit(&mut app, "U7");
    assert_eq!(doc(&app).symbols[0].reference, "U7");

    // A box select takes the part and its two fields: still one part.
    select(
        &mut app,
        &[
            (0, SelectedKind::Symbol),
            (0, SelectedKind::SymbolRefField),
            (0, SelectedKind::SymbolValField),
        ],
    );
    request(&mut app, TextEditTarget::Value);
    assert_eq!(
        editing(&app),
        Some((SelectedKind::SymbolValField, "10k".into()))
    );
}

#[test]
fn the_footprint_command_opens_the_editor_at_the_part_and_commits_the_footprint() {
    let mut app = app();
    select(&mut app, &[(1, SelectedKind::Symbol)]);
    request(&mut app, TextEditTarget::FootprintField);
    assert_eq!(editing(&app), Some((SelectedKind::Symbol, "R_0603".into())));
    let state = app.interaction_state.editing_text.as_ref().unwrap();
    assert_eq!(
        (state.world_x, state.world_y),
        R2_AT,
        "anchored at the part"
    );
    type_and_submit(&mut app, "R_0402");
    assert_eq!(doc(&app).symbols[1].footprint, "R_0402");
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(doc(&app).symbols[1].footprint, "R_0603");
}

#[test]
fn the_text_command_edits_a_label_a_note_or_a_part_designator() {
    let mut app = app();
    hover(&mut app, (20.0, 100.0));
    request(&mut app, TextEditTarget::Text);
    assert_eq!(editing(&app), Some((SelectedKind::Label, "CLK".into())));
    let _ = app.update(Message::TextEdit(TextEditMsg::Submit));

    app.interaction_state.editing_text = None;
    app.ui_state.cursor_document = None;
    select(&mut app, &[(1, SelectedKind::Symbol)]);
    request(&mut app, TextEditTarget::Text);
    assert_eq!(
        editing(&app),
        Some((SelectedKind::SymbolRefField, "R2".into()))
    );
}

#[test]
fn a_command_with_nothing_suitable_says_so_in_messages_and_opens_nothing() {
    let mut app = app();
    app.interaction_state.active_canvas_mut().selected.clear();
    request(&mut app, TextEditTarget::Value);
    assert!(app.interaction_state.editing_text.is_none());
    assert!(logged(&["Edit value: point at a part"]));

    // A label is not a part: no value to edit.
    let label = doc(&app).labels[0].uuid;
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(label, SelectedKind::Label)];
    request(&mut app, TextEditTarget::Designator);
    assert!(app.interaction_state.editing_text.is_none());
    assert!(logged(&["Edit reference designator: point at a part"]));
}

#[test]
fn two_parts_selected_is_not_a_single_target() {
    let mut app = app();
    select(
        &mut app,
        &[(0, SelectedKind::Symbol), (1, SelectedKind::Symbol)],
    );
    request(&mut app, TextEditTarget::FootprintField);
    assert!(app.interaction_state.editing_text.is_none());
    assert!(logged(&["Edit footprint field: point at a part"]));
}

#[test]
fn an_empty_designator_is_accepted_as_properties_does() {
    let mut app = app();
    select(&mut app, &[(0, SelectedKind::Symbol)]);
    request(&mut app, TextEditTarget::Designator);
    type_and_submit(&mut app, "");
    assert_eq!(doc(&app).symbols[0].reference, "");
}

#[test]
fn a_command_keeps_the_text_typed_in_an_open_editor() {
    let mut app = app();
    select(&mut app, &[(0, SelectedKind::Symbol)]);
    request(&mut app, TextEditTarget::Designator);
    let _ = app.update(Message::TextEdit(TextEditMsg::Changed("R77".into())));
    hover(&mut app, (R2_AT.0, R2_AT.1 + 5.0));
    request(&mut app, TextEditTarget::Value);
    assert_eq!(
        doc(&app).symbols[0].reference,
        "R77",
        "typed text committed"
    );
    assert_eq!(
        editing(&app),
        Some((SelectedKind::SymbolValField, "22k".into()))
    );
}

#[test]
fn undo_closes_an_open_editor_instead_of_leaving_stale_text() {
    let mut app = app();
    select(&mut app, &[(0, SelectedKind::Symbol)]);
    request(&mut app, TextEditTarget::Value);
    type_and_submit(&mut app, "1M");
    request(&mut app, TextEditTarget::Value);
    let _ = app.update(Message::TextEdit(TextEditMsg::Changed("9M".into())));
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert!(app.interaction_state.editing_text.is_none());
    assert_eq!(doc(&app).symbols[0].value, "10k");
    assert!(logged(&["In-place edit discarded", "undone"]));
}

#[test]
fn a_commit_whose_object_is_gone_says_so_in_messages() {
    let mut app = app();
    select(&mut app, &[(0, SelectedKind::Symbol)]);
    request(&mut app, TextEditTarget::Value);
    let _ = app.update(Message::TextEdit(TextEditMsg::Changed("3k3".into())));
    let gone = doc(&app).symbols[0].uuid;
    app.document_state
        .active_engine_mut()
        .unwrap()
        .execute(signex_engine::Command::DeleteSelection {
            items: vec![SelectedItem::new(gone, SelectedKind::Symbol)],
        })
        .unwrap();
    let _ = app.update(Message::TextEdit(TextEditMsg::Submit));
    assert!(logged(&["In-place edit discarded", "no longer exists"]));
}

/// #1799 — a power port's designator is never drawn, so F2 / Edit Designator
/// edits what is drawn: the net name, through the Value edit Properties uses.
#[test]
fn f2_on_a_power_port_edits_its_net_name_in_one_undo_step() {
    let mut sheet = empty_sheet();
    let mut port = part("#PWR01", "+5V", R1_AT);
    port.is_power = true;
    sheet.symbols = vec![port];
    let mut app = app_with(sheet);
    select(&mut app, &[(0, SelectedKind::Symbol)]);
    request(&mut app, TextEditTarget::Designator);
    assert_eq!(
        editing(&app),
        Some((SelectedKind::SymbolValField, "+5V".into()))
    );
    type_and_submit(&mut app, "+3V3");
    assert_eq!(doc(&app).symbols[0].value, "+3V3");
    assert_eq!(doc(&app).symbols[0].reference, "#PWR01");
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(doc(&app).symbols[0].value, "+5V");
    assert!(!app.document_state.active_engine().unwrap().can_undo());
}
