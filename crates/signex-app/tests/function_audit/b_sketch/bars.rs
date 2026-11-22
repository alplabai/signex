//! m140, the footprint active bar and its context menu: Move Selection By,
//! Align, Rotate, Flip, Rectangular Pattern, Edit Sketch and Delete.

use signex_app::library::editor::footprint::state::context_menu::SelectionAction;
use signex_app::library::editor::footprint::state::{AlignOp, FootprintContextAction};
use signex_app::library::editor::footprint::{EditorMode, SketchTool};
use signex_app::library::messages::FootprintEditorMsg;

use super::support::{FpRig, near, pads_rig};

const TWO_PADS: [(f64, f64); 2] = [(0.0, 0.0), (4.0, 3.0)];

fn two_selected() -> FpRig {
    let mut rig = pads_rig(&TWO_PADS);
    rig.select_pads(vec![0, 1]);
    rig
}

fn menu_row(rig: &mut FpRig, pick: SelectionAction) {
    rig.send(FootprintEditorMsg::ContextMenuAction(
        FootprintContextAction::OnSelection(pick),
    ));
}

#[test]
fn m140_move_by_moves_the_selection_by_the_typed_offset_and_undo_restores() {
    let mut rig = two_selected();
    let before = rig.positions();

    rig.send(FootprintEditorMsg::MoveByOpen);
    rig.send(FootprintEditorMsg::MoveBySetX("2.5".into()));
    rig.send(FootprintEditorMsg::MoveBySetY("-1".into()));
    rig.send(FootprintEditorMsg::MoveByConfirm);

    let after = rig.positions();
    for (was, now) in before.iter().zip(&after) {
        assert!(near(now.0, was.0 + 2.5) && near(now.1, was.1 - 1.0));
    }
    assert!(rig.editor().state.move_by_modal.is_none());
    assert!(rig.dirty());

    rig.undo();
    assert_eq!(rig.positions(), before);
    assert!(!rig.dirty());
}

#[test]
fn m140_cancel_on_move_by_moves_nothing() {
    let mut rig = two_selected();
    let before = rig.positions();

    rig.send(FootprintEditorMsg::MoveByOpen);
    rig.send(FootprintEditorMsg::MoveBySetX("2.5".into()));
    rig.send(FootprintEditorMsg::MoveByCancel);

    assert_eq!(rig.positions(), before);
    assert!(rig.editor().state.move_by_modal.is_none());
    assert!(!rig.dirty());
}

#[test]
fn m140_a_move_by_of_infinity_is_refused_and_the_dialog_stays() {
    let mut rig = two_selected();
    let before = rig.positions();

    rig.send(FootprintEditorMsg::MoveByOpen);
    rig.send(FootprintEditorMsg::MoveBySetX("inf".into()));
    rig.send(FootprintEditorMsg::MoveBySetY("1".into()));
    rig.send(FootprintEditorMsg::MoveByConfirm);

    assert_eq!(rig.positions(), before);
    assert!(rig.editor().state.move_by_modal.is_some());
    assert!(!rig.dirty());
}

#[test]
fn m140_a_move_by_of_text_moves_nothing() {
    let mut rig = two_selected();
    let before = rig.positions();

    rig.send(FootprintEditorMsg::MoveByOpen);
    rig.send(FootprintEditorMsg::MoveBySetX("abc".into()));
    rig.send(FootprintEditorMsg::MoveBySetY("1".into()));
    rig.send(FootprintEditorMsg::MoveByConfirm);

    assert_eq!(rig.positions(), before);
    assert!(!rig.dirty());
}

#[test]
fn m140_align_left_and_top_is_one_undo_step() {
    let mut rig = two_selected();
    let before = rig.positions();
    let steps = rig.steps();

    rig.send(FootprintEditorMsg::AlignOpen);
    rig.send(FootprintEditorMsg::AlignSetHorizontal(Some(AlignOp::Left)));
    rig.send(FootprintEditorMsg::AlignSetVertical(Some(AlignOp::Top)));
    rig.send(FootprintEditorMsg::AlignConfirm);

    let after = rig.positions();
    assert!(near(after[0].0, after[1].0), "{after:?}");
    assert!(near(after[0].1, after[1].1), "{after:?}");
    assert_eq!(rig.steps(), steps + 1);
    assert!(rig.editor().state.align_modal.is_none());

    rig.undo();
    assert_eq!(rig.positions(), before);
}

#[test]
fn m140_cancel_on_align_moves_nothing() {
    let mut rig = two_selected();
    let before = rig.positions();

    rig.send(FootprintEditorMsg::AlignOpen);
    rig.send(FootprintEditorMsg::AlignSetHorizontal(Some(AlignOp::Left)));
    rig.send(FootprintEditorMsg::AlignCancel);

    assert_eq!(rig.positions(), before);
    assert!(rig.editor().state.align_modal.is_none());
    assert!(!rig.dirty());
}

#[test]
fn m140_align_with_one_pad_selected_is_no_edit() {
    let mut rig = pads_rig(&TWO_PADS);
    rig.select_pads(vec![0]);
    let (before, steps) = (rig.positions(), rig.steps());

    rig.send(FootprintEditorMsg::AlignOpen);
    rig.send(FootprintEditorMsg::AlignSetHorizontal(Some(AlignOp::Right)));
    rig.send(FootprintEditorMsg::AlignConfirm);

    assert_eq!(rig.positions(), before);
    assert_eq!(rig.steps(), steps);
    assert!(!rig.dirty());
}

#[test]
fn m140_the_menu_opens_move_by_and_align() {
    let mut rig = two_selected();

    menu_row(&mut rig, SelectionAction::MoveBy);
    assert!(rig.editor().state.move_by_modal.is_some());
    rig.send(FootprintEditorMsg::MoveByCancel);

    menu_row(&mut rig, SelectionAction::AlignDialog);
    assert!(rig.editor().state.align_modal.is_some());
}

#[test]
fn m140_rotate_turns_the_pad_a_quarter_counter_clockwise_and_undo_restores() {
    let mut rig = pads_rig(&[(0.0, 0.0)]);
    rig.select_pads(vec![0]);
    let before = rig.editor().state.pads[0].rotation_deg;

    menu_row(&mut rig, SelectionAction::Rotate);

    assert!(near(
        rig.editor().state.pads[0].rotation_deg,
        (before + 90.0).rem_euclid(360.0)
    ));
    assert!(rig.dirty());
    rig.undo();
    assert!(near(rig.editor().state.pads[0].rotation_deg, before));
    assert!(!rig.dirty());
}

#[test]
fn m140_rotate_with_nothing_selected_is_no_edit() {
    let mut rig = pads_rig(&[(0.0, 0.0)]);
    rig.select_pads(Vec::new());
    let steps = rig.steps();

    rig.send(FootprintEditorMsg::ActiveBarRotateSelection);

    assert!(near(rig.editor().state.pads[0].rotation_deg, 0.0));
    assert_eq!(rig.steps(), steps);
    assert!(!rig.dirty());
}

#[test]
fn m140_flip_moves_the_pad_to_the_other_side_and_undo_restores() {
    let mut rig = pads_rig(&[(0.0, 0.0)]);
    rig.select_pads(vec![0]);
    let layers = |rig: &FpRig| format!("{:?}", rig.editor().state.pads[0].layers);
    let before = layers(&rig);

    menu_row(&mut rig, SelectionAction::Flip);
    assert_ne!(layers(&rig), before);

    rig.undo();
    assert_eq!(layers(&rig), before);
}

#[test]
fn m140_rectangular_pattern_arms_the_pattern_tool_in_sketch_mode() {
    let mut rig = two_selected();

    menu_row(&mut rig, SelectionAction::RectangularPattern);

    assert_eq!(rig.editor().state.mode, EditorMode::Sketch);
    assert_eq!(rig.editor().state.active_tool, SketchTool::RectPattern);
}

#[test]
fn m140_edit_sketch_switches_to_sketch_mode() {
    let mut rig = two_selected();

    menu_row(&mut rig, SelectionAction::EditSketch);

    assert_eq!(rig.editor().state.mode, EditorMode::Sketch);
}

#[test]
fn m140_delete_removes_the_pad_and_undo_brings_it_back() {
    let mut rig = pads_rig(&TWO_PADS);
    rig.select_pads(vec![0]);

    rig.send(FootprintEditorMsg::DeleteSelected);
    assert_eq!(rig.editor().state.pads.len(), 1);
    assert!(rig.dirty());

    rig.undo();
    assert_eq!(rig.editor().state.pads.len(), 2);
    assert!(!rig.dirty());
}
