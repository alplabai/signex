//! m75 Annotate Schematics, m76 Reset confirm, m77 Re-prefix to standard.

use std::collections::HashMap;
use std::sync::Arc;

use signex_app::app::{AnnotateMsg, Message, Signex};
use signex_app::library::reprefix_lookup::{ReprefixLookup, RowFacts};
use signex_engine::AnnotateMode;
use signex_types::designator::DesignatorStandard;
use signex_types::project::AnnotationSettings;
use signex_types::schematic::{AnnotateOrder, SchematicSheet, Symbol};
use uuid::Uuid;

use super::{Rig, loaded_symbol, part, placement, reference, rig, sheet, undo};
use crate::settle_support::settle;

const ON_ROOT: Uuid = Uuid::from_u128(0x7501);
const ON_CHILD: Uuid = Uuid::from_u128(0x7502);
const PLACED: Uuid = Uuid::from_u128(0x7503);
const LIBRARY: Uuid = Uuid::from_u128(0x7701);
const ROW: Uuid = Uuid::from_u128(0x7702);

fn send(app: &mut Signex, msg: AnnotateMsg) {
    settle(app, Message::Annotate(msg));
}

/// Root holds a resistor and places `child.snxsch`, closed, with a capacitor.
fn two_sheets(root_ref: &str, child_ref: &str) -> (Rig, std::path::PathBuf) {
    let mut on_child = part(ON_CHILD, child_ref, "100n", 0.0);
    on_child.lib_id = "Device:C".to_string();
    let root = sheet(
        vec![part(ON_ROOT, root_ref, "10k", 0.0)],
        vec![placement(PLACED, "Child", "child.snxsch", 40.0)],
    );
    let rig = rig(
        root,
        &[("child.snxsch", sheet(vec![on_child], Vec::new()))],
        None,
    );
    let child = rig.dir.join("child.snxsch");
    (rig, child)
}

fn proposed(app: &Signex, id: Uuid) -> Option<String> {
    app.ui_state
        .annotate_preview
        .entries
        .iter()
        .find(|entry| entry.uuid == id)
        .map(|entry| entry.proposed.clone())
}

#[test]
fn m75_the_list_shows_the_new_designators_and_accept_numbers_both_sheets_in_memory() {
    let (mut rig, child) = two_sheets("R?", "C?");
    let child_file = std::fs::read(&child).expect("child file");
    send(&mut rig.app, AnnotateMsg::OpenDialog);
    assert!(rig.app.ui_state.annotate_dialog_open);
    assert_eq!(proposed(&rig.app, ON_ROOT).as_deref(), Some("R1"));
    assert_eq!(proposed(&rig.app, ON_CHILD).as_deref(), Some("C1"));
    assert_eq!(
        reference(&rig.app, &rig.root, ON_ROOT),
        "R?",
        "only a preview"
    );

    send(&mut rig.app, AnnotateMsg::Run(AnnotateMode::Incremental));

    assert_eq!(reference(&rig.app, &rig.root, ON_ROOT), "R1");
    assert_eq!(reference(&rig.app, &child, ON_CHILD), "C1");
    let unsaved = rig.app.document_state.dirty_document_paths();
    assert!(
        unsaved.contains(&rig.root),
        "the open sheet is marked unsaved"
    );
    assert!(
        unsaved.contains(&child),
        "the closed sheet is marked unsaved"
    );
    assert_eq!(
        std::fs::read(&child).expect("child file"),
        child_file,
        "Annotate writes no sheet file"
    );
}

#[test]
fn m75_one_undo_restores_the_open_and_the_closed_sheet() {
    let (mut rig, child) = two_sheets("R?", "C?");
    send(&mut rig.app, AnnotateMsg::OpenDialog);
    send(&mut rig.app, AnnotateMsg::Run(AnnotateMode::Incremental));
    send(&mut rig.app, AnnotateMsg::CloseDialog);

    undo(&mut rig.app);

    assert_eq!(reference(&rig.app, &rig.root, ON_ROOT), "R?");
    let parked = loaded_symbol(&rig.app, &child, ON_CHILD).map(|s| s.reference.clone());
    assert!(
        parked.is_none() || parked.as_deref() == Some("C?"),
        "the closed sheet is back as read: {parked:?}"
    );
    assert!(!rig.app.document_state.is_document_dirty(&child));
}

#[test]
fn m75_close_without_accept_changes_no_designator_and_marks_nothing_unsaved() {
    let (mut rig, child) = two_sheets("R?", "C?");
    send(&mut rig.app, AnnotateMsg::OpenDialog);
    send(&mut rig.app, AnnotateMsg::FixedOptionsToggled);
    send(&mut rig.app, AnnotateMsg::ListFilterEdited("R".to_string()));
    assert!(rig.app.ui_state.annotate_session.fixed_options_open);
    assert_eq!(rig.app.ui_state.annotate_session.filter, "R");

    send(&mut rig.app, AnnotateMsg::CloseDialog);

    assert!(!rig.app.ui_state.annotate_dialog_open);
    assert_eq!(reference(&rig.app, &rig.root, ON_ROOT), "R?");
    assert!(loaded_symbol(&rig.app, &child, ON_CHILD).is_none());
    assert!(rig.app.document_state.dirty_document_paths().is_empty());
    assert!(
        rig.app.ui_state.annotate_session.filter.is_empty(),
        "the filter belongs to the closed dialog"
    );
}

#[test]
fn m75_a_locked_part_keeps_its_designator_through_reset_and_renumber() {
    let (mut rig, child) = two_sheets("R5", "C7");
    send(&mut rig.app, AnnotateMsg::OpenDialog);
    send(&mut rig.app, AnnotateMsg::ToggleLock(ON_ROOT));
    assert_eq!(
        proposed(&rig.app, ON_ROOT).as_deref(),
        Some("R5"),
        "the list shows the lock"
    );

    send(
        &mut rig.app,
        AnnotateMsg::Run(AnnotateMode::ResetAndRenumber),
    );

    assert_eq!(reference(&rig.app, &rig.root, ON_ROOT), "R5");
    assert_eq!(reference(&rig.app, &child, ON_CHILD), "C1");
}

#[test]
fn m75_all_off_locks_every_listed_part_and_all_on_frees_them() {
    let (mut rig, _child) = two_sheets("R?", "C?");
    send(&mut rig.app, AnnotateMsg::OpenDialog);

    send(&mut rig.app, AnnotateMsg::SetAllLocked(true));
    assert!(rig.app.ui_state.annotate_locked.contains(&ON_ROOT));
    assert!(rig.app.ui_state.annotate_locked.contains(&ON_CHILD));

    send(&mut rig.app, AnnotateMsg::SetAllLocked(false));
    assert!(rig.app.ui_state.annotate_locked.is_empty());
}

#[test]
fn m75_an_unticked_sheet_is_left_out_of_the_run() {
    let (mut rig, child) = two_sheets("R?", "C?");
    send(&mut rig.app, AnnotateMsg::OpenDialog);
    send(&mut rig.app, AnnotateMsg::SheetTickToggled(child.clone()));
    assert!(rig.app.ui_state.annotate_session.unticked.contains(&child));

    send(&mut rig.app, AnnotateMsg::Run(AnnotateMode::Incremental));

    assert_eq!(reference(&rig.app, &rig.root, ON_ROOT), "R1");
    let on_child = loaded_symbol(&rig.app, &child, ON_CHILD).map(|s| s.reference.clone());
    assert!(
        on_child.is_none() || on_child.as_deref() == Some("C?"),
        "the unticked sheet keeps its designator: {on_child:?}"
    );
}

#[test]
fn m75_with_every_sheet_unticked_accept_changes_nothing() {
    let (mut rig, _child) = two_sheets("R?", "C?");
    send(&mut rig.app, AnnotateMsg::OpenDialog);
    send(&mut rig.app, AnnotateMsg::AllSheetsTicked(false));

    send(&mut rig.app, AnnotateMsg::Run(AnnotateMode::Incremental));

    assert_eq!(reference(&rig.app, &rig.root, ON_ROOT), "R?");
    assert!(rig.app.document_state.dirty_document_paths().is_empty());
}

#[test]
fn m75_the_order_of_processing_changes_which_part_gets_the_first_number() {
    const LEFT_LOW: Uuid = Uuid::from_u128(0x7511);
    const RIGHT_HIGH: Uuid = Uuid::from_u128(0x7512);
    let mut left = part(LEFT_LOW, "R?", "1k", 0.0);
    left.position.y = 20.0;
    let right = part(RIGHT_HIGH, "R?", "2k", 20.0);
    let mut rig = rig(sheet(vec![left, right], Vec::new()), &[], None);
    send(&mut rig.app, AnnotateMsg::OpenDialog);

    send(
        &mut rig.app,
        AnnotateMsg::OrderChanged(AnnotateOrder::AcrossThenDown),
    );
    assert_eq!(
        proposed(&rig.app, RIGHT_HIGH).as_deref(),
        Some("R1"),
        "rows from the top: the part at y = 0 is first"
    );

    send(
        &mut rig.app,
        AnnotateMsg::OrderChanged(AnnotateOrder::UpThenAcross),
    );
    assert_eq!(rig.app.ui_state.annotate_order, AnnotateOrder::UpThenAcross);
    assert_eq!(
        proposed(&rig.app, LEFT_LOW).as_deref(),
        Some("R1"),
        "columns from the left: the part at x = 0 is first"
    );
}

#[test]
fn m76_cancel_on_the_reset_confirm_changes_nothing_and_keeps_the_dialog() {
    let (mut rig, _child) = two_sheets("R5", "C7");
    send(&mut rig.app, AnnotateMsg::OpenDialog);
    send(
        &mut rig.app,
        AnnotateMsg::ResetPressed(AnnotateMode::ResetOnly),
    );
    assert!(rig.app.ui_state.annotate_reset_confirm);
    assert_eq!(
        rig.app.ui_state.annotate_session.reset_mode,
        AnnotateMode::ResetOnly
    );

    send(&mut rig.app, AnnotateMsg::CloseResetConfirm);

    assert!(!rig.app.ui_state.annotate_reset_confirm);
    assert!(rig.app.ui_state.annotate_dialog_open);
    assert_eq!(reference(&rig.app, &rig.root, ON_ROOT), "R5");
    assert!(rig.app.document_state.dirty_document_paths().is_empty());
}

#[test]
fn m76_reset_all_clears_every_number_and_one_undo_restores_them() {
    let (mut rig, child) = two_sheets("R5", "C7");
    let child_file = std::fs::read(&child).expect("child file");
    send(&mut rig.app, AnnotateMsg::OpenDialog);
    send(
        &mut rig.app,
        AnnotateMsg::ResetPressed(AnnotateMode::ResetOnly),
    );
    let confirmed = rig.app.ui_state.annotate_session.reset_mode;

    send(&mut rig.app, AnnotateMsg::Run(confirmed));

    assert!(
        !rig.app.ui_state.annotate_reset_confirm,
        "the confirm closes"
    );
    assert_eq!(reference(&rig.app, &rig.root, ON_ROOT), "R?");
    assert_eq!(reference(&rig.app, &child, ON_CHILD), "C?");
    assert_eq!(std::fs::read(&child).expect("child file"), child_file);

    undo(&mut rig.app);

    assert_eq!(reference(&rig.app, &rig.root, ON_ROOT), "R5");
    let parked = loaded_symbol(&rig.app, &child, ON_CHILD).map(|s| s.reference.clone());
    assert!(
        parked.is_none() || parked.as_deref() == Some("C7"),
        "the closed sheet is back as read: {parked:?}"
    );
}

#[test]
fn m76_reset_and_renumber_numbers_every_sheet_from_one() {
    let (mut rig, child) = two_sheets("R5", "C7");
    send(&mut rig.app, AnnotateMsg::OpenDialog);
    send(&mut rig.app, AnnotateMsg::OpenResetConfirm);
    assert_eq!(
        rig.app.ui_state.annotate_session.reset_mode,
        AnnotateMode::ResetAndRenumber,
        "the menu's Reset is Reset and Renumber"
    );
    let confirmed = rig.app.ui_state.annotate_session.reset_mode;

    send(&mut rig.app, AnnotateMsg::Run(confirmed));

    assert_eq!(reference(&rig.app, &rig.root, ON_ROOT), "R1");
    assert_eq!(reference(&rig.app, &child, ON_CHILD), "C1");
}

#[test]
fn m76_reset_and_renumber_is_refused_whole_when_a_sheet_is_read_only() {
    let (mut rig, child) = two_sheets("R5", "C7");
    let mut newer = sheet(vec![part(ON_CHILD, "C7", "100n", 0.0)], Vec::new());
    newer.unknown.newer_minor_source = Some("snxsch/v1.99".into());
    let engine = signex_engine::Engine::new_with_path(newer, Some(child.clone())).expect("engine");
    rig.app.document_state.engines.insert(child.clone(), engine);
    send(&mut rig.app, AnnotateMsg::OpenDialog);

    send(
        &mut rig.app,
        AnnotateMsg::Run(AnnotateMode::ResetAndRenumber),
    );

    assert_eq!(reference(&rig.app, &rig.root, ON_ROOT), "R5");
    assert_eq!(reference(&rig.app, &child, ON_CHILD), "C7");
    assert!(
        rig.app.document_state.error_notice.is_some(),
        "the refusal names the sheet"
    );
}

/// A library-linked part drawn from a symbol whose designator is `R?`.
fn linked(id: Uuid, reference: &str, x: f64) -> Symbol {
    Symbol {
        lib_id: format!("{LIBRARY}:{ROW}"),
        library_id: Some(LIBRARY),
        row_id: Some(ROW),
        ..part(id, reference, "10k", x)
    }
}

fn reprefix_sheet(symbols: Vec<Symbol>) -> SchematicSheet {
    let mut drawn = sheet(symbols, Vec::new());
    let lib_id = format!("{LIBRARY}:{ROW}");
    let mut body = signex_library::primitive::Symbol::empty("R");
    body.designator = "R?".into();
    drawn
        .lib_symbols
        .insert(lib_id.clone(), body.to_lib_symbol(lib_id));
    drawn
}

fn use_din_standard(rig: &mut Rig) {
    let project = rig
        .app
        .document_state
        .project_by_id_mut(rig.project)
        .expect("project");
    project.data.annotation = Some(AnnotationSettings {
        designator_standard: Some(DesignatorStandard::Din40719_2),
        ..Default::default()
    });
}

/// Press Re-prefix and hand the planner the row's answer: prefix `Q`.
fn press_reprefix(rig: &mut Rig) {
    send(&mut rig.app, AnnotateMsg::OpenDialog);
    send(&mut rig.app, AnnotateMsg::ReprefixPressed);
    let facts = RowFacts {
        internal_pn: "RES-0603".to_string(),
        symbol: (LIBRARY, ROW),
        class: "generic".to_string(),
        prefix_override: Some("Q".to_string()),
    };
    let lookup = ReprefixLookup {
        generation: rig.app.ui_state.reprefix_generation,
        project: Some(rig.prj.clone()),
        libraries: HashMap::from([(LIBRARY, HashMap::from([(ROW, facts)]))]),
        problems: Vec::new(),
    };
    send(
        &mut rig.app,
        AnnotateMsg::ReprefixRowsResolved {
            generation: lookup.generation,
            result: Arc::new(Ok(lookup)),
        },
    );
}

#[test]
fn m77_confirm_takes_the_standard_prefix_and_one_undo_restores_it() {
    let mut rig = rig(reprefix_sheet(vec![linked(ON_ROOT, "R1", 0.0)]), &[], None);
    use_din_standard(&mut rig);
    press_reprefix(&mut rig);
    assert!(
        rig.app.ui_state.reprefix_confirm.is_some(),
        "the list shows"
    );
    assert_eq!(
        reference(&rig.app, &rig.root, ON_ROOT),
        "R1",
        "nothing changes before the confirm"
    );

    send(&mut rig.app, AnnotateMsg::ReprefixConfirmed);

    assert!(rig.app.ui_state.reprefix_confirm.is_none());
    let renamed = reference(&rig.app, &rig.root, ON_ROOT);
    assert!(renamed.starts_with('Q'), "{renamed}");
    assert!(rig.app.document_state.is_document_dirty(&rig.root));

    undo(&mut rig.app);

    assert_eq!(reference(&rig.app, &rig.root, ON_ROOT), "R1");
}

#[test]
fn m77_cancel_leaves_every_designator() {
    let mut rig = rig(reprefix_sheet(vec![linked(ON_ROOT, "R1", 0.0)]), &[], None);
    use_din_standard(&mut rig);
    press_reprefix(&mut rig);

    send(&mut rig.app, AnnotateMsg::ReprefixCancelled);

    assert!(rig.app.ui_state.reprefix_confirm.is_none());
    assert_eq!(reference(&rig.app, &rig.root, ON_ROOT), "R1");
    assert!(!rig.app.document_state.is_document_dirty(&rig.root));
}

#[test]
fn m77_a_hand_edited_prefix_is_left_alone() {
    const TYPED: Uuid = Uuid::from_u128(0x7721);
    let parts = vec![linked(ON_ROOT, "R1", 0.0), linked(TYPED, "X9", 20.0)];
    let mut rig = rig(reprefix_sheet(parts), &[], None);
    use_din_standard(&mut rig);
    press_reprefix(&mut rig);

    send(&mut rig.app, AnnotateMsg::ReprefixConfirmed);

    assert!(reference(&rig.app, &rig.root, ON_ROOT).starts_with('Q'));
    assert_eq!(reference(&rig.app, &rig.root, TYPED), "X9");
}

#[test]
fn m77_a_project_with_no_standard_gets_no_confirm() {
    let mut rig = rig(reprefix_sheet(vec![linked(ON_ROOT, "R1", 0.0)]), &[], None);
    send(&mut rig.app, AnnotateMsg::OpenDialog);

    send(&mut rig.app, AnnotateMsg::ReprefixPressed);

    assert!(rig.app.ui_state.reprefix_confirm.is_none());
    assert_eq!(reference(&rig.app, &rig.root, ON_ROOT), "R1");
}
