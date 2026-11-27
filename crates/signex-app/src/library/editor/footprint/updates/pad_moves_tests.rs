//! #3013: moving and aligning given pads, the edit Move Selection By, Align
//! Selection and the `footprint_move_pads` / `footprint_align_pads`
//! commands share.

use super::apply_footprint_primitive_edit;
use super::pad_moves::{align_pads_on_axes, move_pads, pads_needed};
use crate::app::FootprintDocument;
use crate::library::editor::footprint::state::{AlignModal, AlignOp, MoveByModal};
use crate::library::messages::FootprintEditorMsg;

/// An editor with pads at `positions`, none selected, clean and with no
/// history.
fn editor_with(positions: &[(f64, f64)]) -> FootprintDocument {
    let file = signex_library::FootprintFile::from_footprint(
        signex_library::primitive::footprint::Footprint::empty("Test"),
    );
    let mut e = FootprintDocument::new(std::path::PathBuf::from("t.snxfpt"), file);
    for &(x, y) in positions {
        e.state.add_pad_at(x, y);
    }
    // `add_pad_at` selects the pad it adds; the fixture starts with none.
    e.state.selected_pad = None;
    e.state.selected_pads_extra.clear();
    e.set_dirty(false);
    e.history.undo_steps_mut().clear();
    e
}

fn centres(e: &FootprintDocument) -> Vec<(f64, f64)> {
    e.state.pads.iter().map(|p| p.position_mm).collect()
}

fn select_all(e: &mut FootprintDocument) {
    e.state.selected_pad = Some(0);
    e.state.selected_pads_extra = (1..e.state.pads.len()).collect();
}

#[test]
fn moving_named_pads_moves_only_them_in_one_undo_step() {
    let mut e = editor_with(&[(0.0, 0.0), (2.0, 1.0), (5.0, 5.0)]);
    assert!(move_pads(&mut e, &[2, 0, 0], 1.5, -0.5));
    assert_eq!(centres(&e), [(1.5, -0.5), (2.0, 1.0), (6.5, 4.5)]);
    assert!(e.is_dirty());
    assert_eq!(e.history.undo_len(), 1);
    assert_eq!(e.state.selected_pad, None, "the selection is not touched");
    assert!(e.undo());
    assert_eq!(centres(&e), [(0.0, 0.0), (2.0, 1.0), (5.0, 5.0)]);
}

#[test]
fn moving_no_pad_records_nothing() {
    let mut e = editor_with(&[(0.0, 0.0)]);
    assert!(!move_pads(&mut e, &[], 1.0, 1.0));
    assert!(!move_pads(&mut e, &[7], 1.0, 1.0));
    assert_eq!(centres(&e), [(0.0, 0.0)]);
    assert!(!e.is_dirty());
    assert_eq!(e.history.undo_len(), 0);
}

/// Move Selection By's Move on a selection and the command's message on the
/// same pads leave the same footprint and one step each.
#[test]
fn the_dialog_and_the_message_move_alike() {
    let positions = [(0.0, 0.0), (2.0, 1.0), (5.0, 5.0)];
    let mut dialog = editor_with(&positions);
    select_all(&mut dialog);
    dialog.state.move_by_modal = Some(MoveByModal {
        dx_buf: "0.5".into(),
        dy_buf: "-2".into(),
    });
    apply_footprint_primitive_edit(&mut dialog, FootprintEditorMsg::MoveByConfirm);

    let mut message = editor_with(&positions);
    let msg = FootprintEditorMsg::PadsMoveRequested {
        pads: vec![0, 1, 2],
        dx: 0.5,
        dy: -2.0,
    };
    apply_footprint_primitive_edit(&mut message, msg);

    assert_eq!(centres(&dialog), centres(&message));
    assert_eq!(centres(&message)[2], (5.5, 3.0));
    assert_eq!(dialog.history.undo_len(), 1);
    assert_eq!(message.history.undo_len(), 1, "one step, not two");
    assert!(message.is_dirty());
    assert!(dialog.state.move_by_modal.is_none());
}

#[test]
fn align_skips_an_axis_the_pads_are_too_few_for() {
    assert_eq!(pads_needed(AlignOp::Left), 2);
    assert_eq!(pads_needed(AlignOp::DistributeV), 3);
    let mut e = editor_with(&[(0.0, 0.0), (4.0, 6.0)]);
    // Two pads: Space (distribute) is skipped, Top runs.
    assert!(align_pads_on_axes(
        &mut e,
        &[0, 1],
        Some(AlignOp::DistributeH),
        Some(AlignOp::Top)
    ));
    assert_eq!(centres(&e), [(0.0, 0.0), (4.0, 0.0)]);
    assert_eq!(e.history.undo_len(), 1);
    // One pad: nothing to align, nothing recorded.
    let mut one = editor_with(&[(0.0, 0.0), (4.0, 6.0)]);
    assert!(!align_pads_on_axes(
        &mut one,
        &[1],
        Some(AlignOp::Left),
        None
    ));
    assert!(!align_pads_on_axes(&mut one, &[0, 1], None, None));
    assert!(!one.is_dirty());
    assert_eq!(one.history.undo_len(), 0);
}

/// Align Selection's Align on a selection and the command's message on the
/// same pads leave the same footprint and one step each.
#[test]
fn the_dialog_and_the_message_align_alike() {
    let positions = [(0.0, 0.0), (4.0, 6.0), (10.0, 2.0)];
    let mut dialog = editor_with(&positions);
    select_all(&mut dialog);
    dialog.state.align_modal = Some(AlignModal {
        horizontal: Some(AlignOp::DistributeH),
        vertical: Some(AlignOp::CenterV),
    });
    apply_footprint_primitive_edit(&mut dialog, FootprintEditorMsg::AlignConfirm);

    let mut message = editor_with(&positions);
    let msg = FootprintEditorMsg::PadsAlignRequested {
        pads: vec![0, 1, 2],
        horizontal: Some(AlignOp::DistributeH),
        vertical: Some(AlignOp::CenterV),
    };
    apply_footprint_primitive_edit(&mut message, msg);

    assert_eq!(centres(&dialog), centres(&message));
    assert_eq!(
        centres(&message),
        [(0.0, 8.0 / 3.0), (5.0, 8.0 / 3.0), (10.0, 8.0 / 3.0)]
    );
    assert_eq!(dialog.history.undo_len(), 1);
    assert_eq!(message.history.undo_len(), 1, "one step, not two");
    assert!(message.state.selected_pad.is_none(), "no selection is made");
}
