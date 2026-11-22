//! Rows m65 (in-place text edit), m66 (Move Selection), m67 (Find and
//! Replace) and m68 (Find Similar Objects).

use super::*;

// ── m65 In-place text edit ──────────────────────────────────────────────

fn text(app: &mut Signex, message: TextEditMsg) {
    send(app, Message::TextEdit(message));
}

fn request_on_r1(app: &mut Signex, target: TextEditTarget) {
    select_parts(app, &["R1"]);
    text(app, TextEditMsg::Requested(target));
}

#[test]
fn m65_enter_writes_the_typed_value_marks_dirty_and_one_undo_restores_it() {
    let mut app = app();
    request_on_r1(&mut app, TextEditTarget::Value);
    let open = app.interaction_state.editing_text.as_ref().expect("editor");
    assert_eq!(open.kind, SelectedKind::SymbolValField);
    assert_eq!(open.text, "100k", "the editor opens on the current text");

    text(&mut app, TextEditMsg::Changed("220k".into()));
    assert_eq!(symbol(&app, "R1").value, "100k", "typing alone writes nothing");
    assert!(!is_dirty(&app));

    text(&mut app, TextEditMsg::Submit);
    assert_eq!(symbol(&app, "R1").value, "220k");
    assert!(app.interaction_state.editing_text.is_none(), "Enter closes it");
    assert!(is_dirty(&app));

    undo(&mut app);
    assert_eq!(symbol(&app, "R1").value, "100k");
}

#[test]
fn m65_the_designator_edits_in_place() {
    let mut app = app();
    request_on_r1(&mut app, TextEditTarget::Designator);
    assert_eq!(
        app.interaction_state.editing_text.as_ref().map(|e| e.kind),
        Some(SelectedKind::SymbolRefField)
    );
    text(&mut app, TextEditMsg::Changed("R30".into()));
    text(&mut app, TextEditMsg::Submit);
    assert!(doc(&app).symbols.iter().any(|s| s.reference == "R30"));
    undo(&mut app);
    assert!(doc(&app).symbols.iter().any(|s| s.reference == "R1"));
}

#[test]
fn m65_the_footprint_field_edits_in_place() {
    let mut app = app();
    request_on_r1(&mut app, TextEditTarget::FootprintField);
    text(&mut app, TextEditMsg::Changed("R_0805".into()));
    text(&mut app, TextEditMsg::Submit);
    assert_eq!(symbol(&app, "R1").footprint, "R_0805");
}

#[test]
fn m65_a_net_label_edits_in_place_and_leaves_the_other_labels() {
    let mut app = app();
    let id = doc(&app).labels[1].uuid;
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(id, SelectedKind::Label)];
    text(&mut app, TextEditMsg::Requested(TextEditTarget::Text));
    text(&mut app, TextEditMsg::Changed("VBUS".into()));
    text(&mut app, TextEditMsg::Submit);
    assert_eq!(label_texts(&app), ["VOUT", "VBUS", "VOUT"]);
}

#[test]
fn m65_escape_drops_the_typed_text_and_leaves_the_sheet_clean() {
    let mut app = app();
    request_on_r1(&mut app, TextEditTarget::Value);
    text(&mut app, TextEditMsg::Changed("999k".into()));
    escape(&mut app);
    assert!(app.interaction_state.editing_text.is_none());
    assert_eq!(symbol(&app, "R1").value, "100k");
    assert!(!is_dirty(&app));
}

#[test]
fn m65_enter_on_unchanged_text_records_no_undo_step() {
    let mut app = app();
    request_on_r1(&mut app, TextEditTarget::Value);
    text(&mut app, TextEditMsg::Submit);
    assert!(!app.document_state.active_engine().unwrap().can_undo());
    assert!(!is_dirty(&app));
}

#[test]
fn m65_no_editor_opens_with_nothing_to_edit() {
    let mut app = app();
    text(&mut app, TextEditMsg::Requested(TextEditTarget::Value));
    assert!(app.interaction_state.editing_text.is_none());
}

#[test]
fn m65_a_read_only_sheet_opens_no_editor() {
    let mut app = read_only_app();
    request_on_r1(&mut app, TextEditTarget::Value);
    assert!(app.interaction_state.editing_text.is_none());
    assert_eq!(symbol(&app, "R1").value, "100k");
    assert!(!is_dirty(&app));
}

// ── m66 Move Selection ──────────────────────────────────────────────────

#[test]
fn m66_move_shifts_right_and_up_marks_dirty_and_one_undo_restores() {
    let mut app = app();
    let (r1, r2, r3) = (at(&app, "R1"), at(&app, "R2"), at(&app, "R3"));
    open_move(&mut app, "5.08", "2.54");
    mv(&mut app, MoveSelectionMsg::Apply);

    assert!(!app.ui_state.move_selection.open, "Move closes the dialog");
    for (now, was) in [(at(&app, "R1"), r1), (at(&app, "R2"), r2)] {
        assert!(near(now.0, was.0 + 5.08), "positive dX moves right");
        assert!(near(now.1, was.1 - 2.54), "positive dY moves up the sheet");
    }
    assert_eq!(at(&app, "R3"), r3, "an unselected part stays");
    assert!(is_dirty(&app));

    undo(&mut app);
    assert_eq!((at(&app, "R1"), at(&app, "R2")), (r1, r2));
}

#[test]
fn m66_a_negative_dy_moves_down_the_sheet() {
    let mut app = app();
    let was = at(&app, "R1");
    open_move(&mut app, "0", "-2.54");
    mv(&mut app, MoveSelectionMsg::Apply);
    assert!(near(at(&app, "R1").1, was.1 + 2.54));
}

#[test]
fn m66_offsets_are_read_in_the_display_unit() {
    let mut app = app();
    app.ui_state.unit = Unit::Mil;
    let was = at(&app, "R1");
    open_move(&mut app, "100", "0");
    mv(&mut app, MoveSelectionMsg::Apply);
    assert!(near(at(&app, "R1").0, was.0 + 2.54));
}

#[test]
fn m66_the_move_button_closes_the_dialog_window() {
    let mut app = app();
    let windows = app.ui_state.windows.len();
    open_move(&mut app, "5.08", "0");
    assert_eq!(app.ui_state.windows.len(), windows + 1, "the dialog's window");
    mv(&mut app, MoveSelectionMsg::Apply);
    assert_eq!(
        app.ui_state.windows.len(),
        windows,
        "a finished move leaves no dialog window behind"
    );
}

#[test]
fn m66_cancel_moves_nothing_closes_the_window_and_stays_clean() {
    let mut app = app();
    let windows = app.ui_state.windows.len();
    let was = at(&app, "R1");
    open_move(&mut app, "5.08", "2.54");
    mv(&mut app, MoveSelectionMsg::Close);
    assert!(!app.ui_state.move_selection.open);
    assert_eq!(app.ui_state.windows.len(), windows);
    assert_eq!(at(&app, "R1"), was);
    assert!(!is_dirty(&app));
}

#[test]
fn m66_a_non_finite_offset_is_refused_and_the_dialog_stays_open() {
    let mut app = app();
    let was = at(&app, "R1");
    open_move(&mut app, "nan", "2.54");
    mv(&mut app, MoveSelectionMsg::Apply);
    assert!(app.ui_state.move_selection.open, "stays open to be fixed");
    assert_eq!(at(&app, "R1"), was, "neither axis moves");
    assert!(!is_dirty(&app));
}

#[test]
fn m66_letters_in_an_offset_are_refused_rather_than_read_as_zero() {
    let mut app = app();
    let was = at(&app, "R1");
    open_move(&mut app, "abc", "2.54");
    mv(&mut app, MoveSelectionMsg::Apply);
    assert!(app.ui_state.move_selection.open, "stays open to be fixed");
    assert_eq!(at(&app, "R1"), was, "the valid axis does not move alone");
    assert!(!is_dirty(&app));
}

// ── m67 Find and Replace ────────────────────────────────────────────────

#[test]
fn m67_replace_all_takes_whole_words_only_in_one_undo_step() {
    let mut app = app();
    open_replace(&mut app, "VOUT", "VOUT_A");
    assert!(app.ui_state.find_replace.replace_mode);
    assert_eq!(app.ui_state.find_replace.result_count(), 2);

    find(&mut app, FindReplaceMsg::ReplaceAll);
    assert_eq!(label_texts(&app), ["VOUT_A", "VOUT_3V3", "VOUT_A"]);
    assert!(is_dirty(&app));

    undo(&mut app);
    assert_eq!(label_texts(&app), ["VOUT", "VOUT_3V3", "VOUT"]);
}

#[test]
fn m67_replace_current_changes_only_the_current_result() {
    let mut app = app();
    open_replace(&mut app, "VOUT", "VOUT_A");
    find(&mut app, FindReplaceMsg::SelectResult(1));
    assert_eq!(app.ui_state.find_replace.selected_index, Some(1));
    let current = app.ui_state.find_replace.matches[1].hit.item.uuid;

    find(&mut app, FindReplaceMsg::ReplaceCurrent);

    let changed: Vec<_> = doc(&app)
        .labels
        .iter()
        .filter(|l| l.text == "VOUT_A")
        .collect();
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].uuid, current);
    assert_eq!(app.ui_state.find_replace.result_count(), 1, "list refreshed");
}

#[test]
fn m67_a_cleared_include_box_keeps_that_result() {
    let mut app = app();
    open_replace(&mut app, "VOUT", "VOUT_A");
    let kept = app.ui_state.find_replace.matches[0].clone();
    find(&mut app, FindReplaceMsg::ResultIncludeToggled(kept.key()));
    assert_eq!(app.ui_state.find_replace.included_count(), 1);

    find(&mut app, FindReplaceMsg::ReplaceAll);

    let labels = &doc(&app).labels;
    let label = labels.iter().find(|l| l.uuid == kept.hit.item.uuid);
    assert_eq!(label.map(|l| l.text.as_str()), Some("VOUT"));
    assert_eq!(labels.iter().filter(|l| l.text == "VOUT_A").count(), 1);
}

#[test]
fn m67_whole_word_off_also_finds_the_longer_net_name() {
    let mut app = app();
    open_replace(&mut app, "VOUT", "X");
    find(&mut app, FindReplaceMsg::WholeWordToggled);
    assert_eq!(app.ui_state.find_replace.result_count(), 3);
}

#[test]
fn m67_match_case_narrows_the_results() {
    let mut app = app();
    open_replace(&mut app, "vout", "X");
    assert_eq!(app.ui_state.find_replace.result_count(), 2);
    find(&mut app, FindReplaceMsg::MatchCaseToggled);
    assert_eq!(app.ui_state.find_replace.result_count(), 0);
}

#[test]
fn m67_next_and_previous_step_through_results_and_wrap() {
    let mut app = app();
    open_replace(&mut app, "VOUT", "X");
    assert_eq!(app.ui_state.find_replace.selected_index, Some(0));
    find(&mut app, FindReplaceMsg::NextRequested);
    assert_eq!(app.ui_state.find_replace.selected_index, Some(1));
    find(&mut app, FindReplaceMsg::NextRequested);
    assert_eq!(app.ui_state.find_replace.selected_index, Some(0));
    find(&mut app, FindReplaceMsg::PreviousRequested);
    assert_eq!(app.ui_state.find_replace.selected_index, Some(1));
}

#[test]
fn m67_the_header_switch_changes_mode_and_keeps_the_query() {
    let mut app = app();
    open_replace(&mut app, "VOUT", "X");
    find(&mut app, FindReplaceMsg::ModeChanged(false));
    assert!(!app.ui_state.find_replace.replace_mode);
    assert_eq!(app.ui_state.find_replace.query, "VOUT");
}

#[test]
fn m67_an_invalid_regex_lists_nothing_and_replaces_nothing() {
    let mut app = app();
    open_replace(&mut app, "(", "X");
    find(&mut app, FindReplaceMsg::RegexToggled);
    assert!(app.ui_state.find_replace.find_query().is_err());
    assert_eq!(app.ui_state.find_replace.result_count(), 0);

    find(&mut app, FindReplaceMsg::ReplaceAll);
    assert_eq!(label_texts(&app), ["VOUT", "VOUT_3V3", "VOUT"]);
    assert!(!is_dirty(&app));
}

#[test]
fn m67_closing_the_dialog_changes_nothing() {
    let mut app = app();
    open_replace(&mut app, "VOUT", "VOUT_A");
    find(&mut app, FindReplaceMsg::Close);
    assert!(!app.ui_state.find_replace.open);
    assert_eq!(label_texts(&app), ["VOUT", "VOUT_3V3", "VOUT"]);
    assert!(!is_dirty(&app));
}

#[test]
fn m67_project_scope_lists_hits_with_their_sheet() {
    let fx = fixture::fixture().expect("fixture");
    let mut app = fixture::fresh(ThemeId::GraphiteLight);
    settle(
        &mut app,
        Message::File(FileMsg::Opened(Some(fx.sample.clone()))),
    );
    settle(
        &mut app,
        Message::File(FileMsg::Opened(Some(fx.sample.with_extension("snxsch")))),
    );
    settle(&mut app, Message::Overlay(OverlayMsg::OpenReplace));
    settle(
        &mut app,
        Message::FindReplaceMsg(FindReplaceMsg::ScopeChanged(FindScope::Project)),
    );
    settle(
        &mut app,
        Message::FindReplaceMsg(FindReplaceMsg::QueryChanged("GND".into())),
    );

    let state = &app.ui_state.find_replace;
    assert!(!state.scanning, "the scan finished");
    assert!(state.result_count() > 0);
    assert!(state.matches.iter().all(|m| m.sheet.is_some()));
    assert!(
        !app.document_state.has_dirty_documents(),
        "a search writes nothing"
    );
}

// ── m68 Find Similar Objects ────────────────────────────────────────────

fn open_similar(app: &mut Signex) {
    let id = AppCommandId::new("find_similar_objects").expect("command id");
    settle(app, Message::Menu(MenuMessage::Command(id)));
}

#[test]
fn m68_opens_on_the_selected_part_with_only_kind_set_to_same() {
    let mut app = app();
    select_parts(&mut app, &["R1"]);
    open_similar(&mut app);
    let state = app.ui_state.find_similar.as_ref().expect("dialog");
    assert_eq!(state.seed_label, "Symbol R1");
    assert_eq!(
        state.same.iter().copied().collect::<Vec<_>>(),
        [SimilarProperty::Kind]
    );
    assert_eq!(state.sheet_matches, 4, "Select N counts the four parts");
}

#[test]
fn m68_the_seed_criteria_with_value_same_match_the_equal_values() {
    let mut app = app();
    select_parts(&mut app, &["R1"]);
    open_similar(&mut app);
    let mut state = (**app.ui_state.find_similar.as_ref().expect("dialog")).clone();
    state.same.insert(SimilarProperty::Value);
    let found = signex_engine::find_similar(doc(&app), &state.criteria());
    let expected = ["R1", "R2", "C1"].map(|r| symbol(&app, r).uuid);
    assert_eq!(found.len(), 3);
    assert!(found.iter().all(|item| expected.contains(&item.uuid)));

    state.same.insert(SimilarProperty::DesignatorPrefix);
    let found = signex_engine::find_similar(doc(&app), &state.criteria());
    assert_eq!(found.len(), 2, "the C part drops out on prefix R");
}

#[test]
fn m68_escape_closes_and_keeps_the_selection_and_the_sheet() {
    let mut app = app();
    select_parts(&mut app, &["R1"]);
    let selected = app.interaction_state.active_canvas().selected.clone();
    open_similar(&mut app);
    escape(&mut app);
    assert!(app.ui_state.find_similar.is_none());
    assert_eq!(app.interaction_state.active_canvas().selected, selected);
    assert!(!is_dirty(&app));
}

#[test]
fn m68_two_selected_objects_are_no_seed() {
    let mut app = app();
    select_parts(&mut app, &["R1", "R2"]);
    open_similar(&mut app);
    assert!(app.ui_state.find_similar.is_none());
}
