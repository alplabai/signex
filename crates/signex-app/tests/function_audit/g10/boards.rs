//! m81 Update PCB, m82 Back Annotate, m83 Re-Annotate Board.

use signex_app::app::{BackAnnotateMsg, BoardReannotateMsg, Message, Signex, UpdatePcbMsg};
use signex_engine::pcb::designators::{BottomSideOrder, ReannotateOrder};
use signex_engine::pcb::eco::{BackKey, EcoKey};
use signex_engine::{Command, TextTarget};
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use super::{
    BoardRig, Rig, board_of, empty_board, footprint, loaded_symbol, menu, open_board, part, rig,
    sheet, show, undo,
};
use crate::settle_support::settle;

const PART: Uuid = Uuid::from_u128(0x8101);
const FP: Uuid = Uuid::from_u128(0x8102);

/// The sheet's R1 is `sheet_value`; the board's footprint, linked to it, is
/// `board_ref` with `board_value`.
fn linked_project(sheet_value: &str, board_ref: &str, board_value: &str) -> Rig {
    let mut board = empty_board();
    board.footprints = vec![footprint(FP, board_ref, board_value, (10.0, 10.0), PART)];
    rig(
        sheet(vec![part(PART, "R1", sheet_value, 0.0)], Vec::new()),
        &[],
        Some(board),
    )
}

fn board_value(rig: &Rig) -> String {
    board_of(&rig.app, &rig.board).footprints[0].value.clone()
}

/// An edit of the sheet behind the dialog's back: its list is stale now.
fn edit_sheet_value(rig: &mut Rig) {
    rig.app
        .document_state
        .engines
        .get_mut(&rig.root)
        .expect("root engine")
        .execute(Command::UpdateText {
            target: TextTarget::SymbolValue(PART),
            value: "47k".into(),
        })
        .expect("the edit applies");
}

// ---- m81 Update PCB ----

fn up(app: &mut Signex, msg: UpdatePcbMsg) {
    settle(app, Message::from(msg));
}

fn open_update(rig: &mut Rig) {
    menu(&mut rig.app, "update_pcb_from_schematic");
    let state = rig.app.ui_state.update_pcb.as_ref().expect("dialog open");
    assert!(state.is_ready(), "the list is ready: {:?}", state.failure());
}

fn value_row_on(app: &Signex) -> bool {
    app.ui_state
        .update_pcb
        .as_ref()
        .and_then(|state| state.list())
        .and_then(|list| list.row(&EcoKey::SetValue(FP)))
        .expect("the value row is listed")
        .on
}

#[test]
fn m81_validate_then_execute_carries_the_value_to_the_board_and_undo_restores_it() {
    let mut rig = linked_project("22k", "R1", "10k");
    open_update(&mut rig);
    assert!(value_row_on(&rig.app));
    assert_eq!(board_value(&rig), "10k", "the list changes nothing");

    up(&mut rig.app, UpdatePcbMsg::ValidateRequested);
    let state = rig.app.ui_state.update_pcb.as_ref().expect("dialog open");
    assert_eq!(state.validation(), Some(&Ok(())));

    up(&mut rig.app, UpdatePcbMsg::ExecuteRequested);

    assert!(rig.app.ui_state.update_pcb.is_none(), "Execute closes it");
    assert_eq!(board_value(&rig), "22k");
    assert!(rig.app.document_state.is_document_dirty(&rig.board));

    show(&mut rig.app, &rig.board.clone());
    undo(&mut rig.app);

    assert_eq!(board_value(&rig), "10k");
}

#[test]
fn m81_close_leaves_the_board_as_it_was() {
    let mut rig = linked_project("22k", "R1", "10k");
    open_update(&mut rig);

    up(&mut rig.app, UpdatePcbMsg::CloseRequested);

    assert!(rig.app.ui_state.update_pcb.is_none());
    assert_eq!(board_value(&rig), "10k");
    assert!(!rig.app.document_state.is_document_dirty(&rig.board));
}

#[test]
fn m81_an_unticked_row_is_not_executed() {
    let mut rig = linked_project("22k", "R1", "10k");
    open_update(&mut rig);

    up(
        &mut rig.app,
        UpdatePcbMsg::RowToggled {
            key: EcoKey::SetValue(FP),
            on: false,
        },
    );
    assert!(!value_row_on(&rig.app));
    up(&mut rig.app, UpdatePcbMsg::ExecuteRequested);

    assert_eq!(board_value(&rig), "10k");
}

#[test]
fn m81_a_list_gone_stale_is_refused_by_validate_and_execute() {
    let mut rig = linked_project("22k", "R1", "10k");
    open_update(&mut rig);
    edit_sheet_value(&mut rig);
    assert!(rig.app.update_pcb_stale());
    assert!(rig.app.update_pcb_execute_block().is_some());

    up(&mut rig.app, UpdatePcbMsg::ValidateRequested);
    let state = rig.app.ui_state.update_pcb.as_ref().expect("dialog open");
    assert!(matches!(state.validation(), Some(Err(_))));

    up(&mut rig.app, UpdatePcbMsg::ExecuteRequested);

    assert!(rig.app.ui_state.update_pcb.is_some(), "the dialog stays");
    assert_eq!(board_value(&rig), "10k");
}

#[test]
fn m81_the_filter_field_lists_only_matching_rows_and_clear_lists_all() {
    let mut rig = linked_project("22k", "R1", "10k");
    open_update(&mut rig);

    up(
        &mut rig.app,
        UpdatePcbMsg::FilterQueryChanged("no such part".to_string()),
    );
    let state = rig.app.ui_state.update_pcb.as_ref().expect("dialog open");
    assert!(state.shown_rows().is_empty());

    up(&mut rig.app, UpdatePcbMsg::FilterCleared);
    let state = rig.app.ui_state.update_pcb.as_ref().expect("dialog open");
    assert!(!state.shown_rows().is_empty());
}

#[test]
fn m81_a_project_with_no_board_offers_create_pcb_instead_of_a_list() {
    let mut rig = rig(
        sheet(vec![part(PART, "R1", "22k", 0.0)], Vec::new()),
        &[],
        None,
    );

    menu(&mut rig.app, "update_pcb_from_schematic");

    let state = rig.app.ui_state.update_pcb.as_ref().expect("dialog open");
    assert!(state.lacks_board());
    assert!(state.list().is_none());
}

// ---- m82 Back Annotate ----

fn back(app: &mut Signex, msg: BackAnnotateMsg) {
    settle(app, Message::from(msg));
}

fn open_back(rig: &mut Rig) {
    menu(&mut rig.app, "back_annotate_schematics");
    let state = rig
        .app
        .ui_state
        .back_annotate
        .as_ref()
        .expect("dialog open");
    assert!(state.is_ready(), "the list is ready: {:?}", state.failure());
}

fn on_sheet(rig: &Rig) -> (String, String) {
    let symbol = loaded_symbol(&rig.app, &rig.root, PART).expect("the part");
    (symbol.reference.clone(), symbol.value.clone())
}

fn pair(reference: &str, value: &str) -> (String, String) {
    (reference.to_string(), value.to_string())
}

#[test]
fn m82_execute_carries_designator_and_value_to_the_sheet_and_one_undo_restores_them() {
    let mut rig = linked_project("10k", "R7", "22k");
    let sheet_file = std::fs::read(&rig.root).expect("sheet file");
    open_back(&mut rig);
    assert_eq!(
        on_sheet(&rig),
        pair("R1", "10k"),
        "the list changes nothing"
    );

    back(&mut rig.app, BackAnnotateMsg::ValidateRequested);
    let state = rig
        .app
        .ui_state
        .back_annotate
        .as_ref()
        .expect("dialog open");
    assert_eq!(state.validation(), Some(&Ok(())));

    back(&mut rig.app, BackAnnotateMsg::ExecuteRequested);

    assert!(
        rig.app.ui_state.back_annotate.is_none(),
        "Execute closes it"
    );
    assert_eq!(on_sheet(&rig), pair("R7", "22k"));
    assert!(rig.app.document_state.is_document_dirty(&rig.root));
    assert_eq!(
        std::fs::read(&rig.root).expect("sheet file"),
        sheet_file,
        "nothing is saved until the user saves"
    );

    show(&mut rig.app, &rig.root.clone());
    undo(&mut rig.app);

    assert_eq!(on_sheet(&rig), pair("R1", "10k"));
}

#[test]
fn m82_close_leaves_the_sheet_as_it_was() {
    let mut rig = linked_project("10k", "R7", "22k");
    open_back(&mut rig);

    back(&mut rig.app, BackAnnotateMsg::CloseRequested);

    assert!(rig.app.ui_state.back_annotate.is_none());
    assert_eq!(on_sheet(&rig), pair("R1", "10k"));
    assert!(!rig.app.document_state.is_document_dirty(&rig.root));
}

#[test]
fn m82_an_unticked_row_stays_out_of_the_run() {
    let mut rig = linked_project("10k", "R7", "22k");
    open_back(&mut rig);

    back(
        &mut rig.app,
        BackAnnotateMsg::RowToggled {
            key: BackKey::Designator(FP),
            on: false,
        },
    );
    back(&mut rig.app, BackAnnotateMsg::ExecuteRequested);

    assert_eq!(on_sheet(&rig), pair("R1", "22k"));
}

#[test]
fn m82_a_list_gone_stale_is_refused_until_refresh() {
    let mut rig = linked_project("10k", "R7", "22k");
    open_back(&mut rig);
    edit_sheet_value(&mut rig);
    assert!(rig.app.back_annotate_stale());

    back(&mut rig.app, BackAnnotateMsg::ExecuteRequested);

    assert_eq!(
        on_sheet(&rig),
        pair("R1", "47k"),
        "a stale list does not run"
    );
    let state = rig.app.ui_state.back_annotate.as_ref().expect("still open");
    assert!(matches!(state.validation(), Some(Err(_))));

    back(&mut rig.app, BackAnnotateMsg::RefreshRequested);

    assert!(!rig.app.back_annotate_stale());
}

// ---- m83 Re-Annotate Board ----

const LEFT: Uuid = Uuid::from_u128(0x8301);
const RIGHT: Uuid = Uuid::from_u128(0x8302);
const LOCKED: Uuid = Uuid::from_u128(0x8303);

/// R2 at (10, 10) left of R1 at (30, 5), and a locked R9 at (50, 10).
fn board_out_of_order() -> PcbBoard {
    let mut board = empty_board();
    let mut locked = footprint(LOCKED, "R9", "10k", (50.0, 10.0), Uuid::from_u128(0x8313));
    locked.locked = true;
    board.footprints = vec![
        footprint(LEFT, "R2", "10k", (10.0, 10.0), Uuid::from_u128(0x8311)),
        footprint(RIGHT, "R1", "10k", (30.0, 5.0), Uuid::from_u128(0x8312)),
        locked,
    ];
    board
}

fn reannotate(app: &mut Signex, msg: BoardReannotateMsg) {
    settle(app, Message::from(msg));
}

fn designator(rig: &BoardRig, id: Uuid) -> String {
    board_of(&rig.app, &rig.path)
        .footprints
        .iter()
        .find(|fp| fp.uuid == id)
        .expect("footprint")
        .reference
        .clone()
}

fn open_reannotate() -> BoardRig {
    let mut rig = open_board(board_out_of_order());
    menu(&mut rig.app, "reannotate_board");
    assert!(rig.app.ui_state.board_reannotate.is_some(), "dialog open");
    rig
}

#[test]
fn m83_execute_numbers_by_position_keeps_the_locked_part_and_undo_restores() {
    let mut rig = open_reannotate();
    let state = rig.app.ui_state.board_reannotate.as_ref().expect("open");
    assert!(state.can_execute());
    assert_eq!(state.plan.renames.len(), 2);
    assert_eq!(designator(&rig, LEFT), "R2", "the plan changes nothing");

    reannotate(&mut rig.app, BoardReannotateMsg::ExecuteRequested);

    assert!(
        rig.app.ui_state.board_reannotate.is_none(),
        "Execute closes it"
    );
    assert_eq!(designator(&rig, LEFT), "R1");
    assert_eq!(designator(&rig, RIGHT), "R2");
    assert_eq!(designator(&rig, LOCKED), "R9");
    assert!(rig.app.document_state.is_document_dirty(&rig.path));

    undo(&mut rig.app);

    assert_eq!(designator(&rig, LEFT), "R2");
    assert_eq!(designator(&rig, RIGHT), "R1");
}

#[test]
fn m83_the_order_choice_remakes_the_preview() {
    let mut rig = open_reannotate();

    reannotate(
        &mut rig.app,
        BoardReannotateMsg::OrderPicked(ReannotateOrder::YThenX),
    );

    let state = rig.app.ui_state.board_reannotate.as_ref().expect("open");
    assert_eq!(state.options.order, ReannotateOrder::YThenX);
    assert!(
        state.plan.renames.is_empty(),
        "by Y, R1 at y = 5 is already first"
    );
    assert!(!state.can_execute());
}

#[test]
fn m83_with_nothing_to_rename_execute_changes_nothing() {
    let mut rig = open_reannotate();
    reannotate(
        &mut rig.app,
        BoardReannotateMsg::OrderPicked(ReannotateOrder::YThenX),
    );

    reannotate(&mut rig.app, BoardReannotateMsg::ExecuteRequested);

    assert!(
        rig.app.ui_state.board_reannotate.is_some(),
        "the dialog stays"
    );
    assert_eq!(designator(&rig, LEFT), "R2");
    assert!(!rig.app.document_state.is_document_dirty(&rig.path));
}

#[test]
fn m83_the_bottom_side_and_show_in_order_controls_update_the_dialog() {
    let mut rig = open_reannotate();

    reannotate(
        &mut rig.app,
        BoardReannotateMsg::BottomPicked(BottomSideOrder::WithTop),
    );
    reannotate(&mut rig.app, BoardReannotateMsg::InOrderShown(true));

    let state = rig.app.ui_state.board_reannotate.as_ref().expect("open");
    assert_eq!(state.options.bottom, BottomSideOrder::WithTop);
    assert!(state.show_in_order);
}

#[test]
fn m83_cancel_leaves_the_board_as_it_was() {
    let mut rig = open_reannotate();

    reannotate(&mut rig.app, BoardReannotateMsg::CancelRequested);

    assert!(rig.app.ui_state.board_reannotate.is_none());
    assert_eq!(designator(&rig, LEFT), "R2");
    assert!(!rig.app.document_state.is_document_dirty(&rig.path));
}

#[test]
fn m83_the_next_opening_keeps_the_order_picked_before_cancel() {
    let mut rig = open_reannotate();
    reannotate(
        &mut rig.app,
        BoardReannotateMsg::OrderPicked(ReannotateOrder::YThenX),
    );
    reannotate(&mut rig.app, BoardReannotateMsg::CancelRequested);

    menu(&mut rig.app, "reannotate_board");

    let state = rig.app.ui_state.board_reannotate.as_ref().expect("open");
    assert_eq!(state.options.order, ReannotateOrder::YThenX);
    assert_eq!(designator(&rig, LEFT), "R2", "the board is unchanged");
}
