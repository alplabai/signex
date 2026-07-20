//! Footprint editor tests.
//!
//! Coverage focuses on the layer visibility helpers + the pad
//! hit-test surface; the smaller `from_footprint` / `add_pad_at` /
//! `sync_pads_to_primitive` flow is covered in `state.rs` itself.

use super::layers::{FpLayer, LayerVisibility};
use super::state::{FootprintEditorState, MoveByModal};

#[test]
fn empty_state_has_no_pads_or_courtyard() {
    let s = FootprintEditorState::empty();
    assert!(s.pads.is_empty());
    assert!(s.courtyard_mm.is_none());
}

#[test]
fn add_two_pads_then_hit_test() {
    let mut s = FootprintEditorState::empty();
    s.add_pad_at(0.0, 0.0);
    s.add_pad_at(2.5, 0.0);
    assert_eq!(s.pads.len(), 2);
    let hit = s.pad_at(2.5, 0.0);
    assert_eq!(hit, Some(1));
    let miss = s.pad_at(20.0, 20.0);
    assert!(miss.is_none());
}

#[test]
fn delete_pad_clears_selection() {
    let mut s = FootprintEditorState::empty();
    s.add_pad_at(0.0, 0.0);
    s.selected_pad = Some(0);
    s.delete_pad(0);
    assert!(s.pads.is_empty());
    assert!(s.selected_pad.is_none());
}

#[test]
fn auto_fit_courtyard_tracks_pads() {
    let mut s = FootprintEditorState::empty();
    // Auto-fit defaults off (v0.26-I) — courtyard is authored
    // explicitly. Enable it the way the active-bar toggle does
    // before asserting it tracks the pad bbox.
    s.toggle_auto_fit();
    s.add_pad_at(-2.0, -1.0);
    s.add_pad_at(2.0, 1.0);
    let c = s
        .courtyard_mm
        .expect("auto-fit should have produced a rect");
    assert!(c.min_x < -2.0);
    assert!(c.max_x > 2.0);
}

#[test]
fn layer_visibility_default_only_front_on() {
    let v = LayerVisibility::default();
    assert!(v.get(FpLayer::FCu));
    assert!(!v.get(FpLayer::BCu));
    assert!(v.get(FpLayer::FFab));
    assert!(!v.get(FpLayer::BFab));
}

// v0.14 — "Move Selection by X, Y…" nudges the whole selection by one
// grid step. `nudge_pads` is the geometry the dispatcher calls; assert
// it translates exactly the selected pads and leaves the rest put.
#[test]
fn nudge_pads_translates_selection_by_delta() {
    let mut s = FootprintEditorState::empty();
    s.add_pad_at(0.0, 0.0); // idx 0 — selected
    s.add_pad_at(2.0, 1.0); // idx 1 — selected
    s.add_pad_at(5.0, 5.0); // idx 2 — NOT selected, must stay put

    // Default grid step is 1.0 mm; nudge the first two pads by +1 / +1.
    let step = s.snap_options.grid_step_mm;
    let moved = s.nudge_pads(&[0, 1], step, step);

    assert_eq!(moved, vec![0, 1]);
    assert_eq!(s.pads[0].position_mm, (1.0, 1.0));
    assert_eq!(s.pads[1].position_mm, (3.0, 2.0));
    // Unselected pad is untouched.
    assert_eq!(s.pads[2].position_mm, (5.0, 5.0));
}

// 3D Body mint populates body_3d as an Extrude body whose outline is the
// courtyard, so the CPU preview shows a solid immediately.
#[test]
fn mint_body3d_extrudes_courtyard() {
    use signex_library::primitive::footprint::BodyShape;
    let mut fp = signex_library::primitive::footprint::Footprint::empty("TestFp");
    // give the footprint a non-empty courtyard (2x2mm square) so the box
    // has an outline to copy.
    fp.courtyard = signex_library::primitive::footprint::Polygon::new(vec![
        [-1.0, -1.0],
        [1.0, -1.0],
        [1.0, 1.0],
        [-1.0, 1.0],
    ]);
    assert!(fp.body_3d.outline.is_none());
    crate::library::editor::footprint::body3d_mint::mint_box_from_courtyard(&mut fp);
    assert_eq!(fp.body_3d.shape, BodyShape::Extrude);
    assert!(
        fp.body_3d.outline.is_some(),
        "outline should be the courtyard"
    );
    assert!(fp.body_3d.height_mm > 0.0);
}

// Out-of-range indices are skipped (no panic) and excluded from the
// returned moved-list — the dispatcher relies on this to mirror only
// the pads that actually moved into the sketch.
#[test]
fn nudge_pads_skips_out_of_range_indices() {
    let mut s = FootprintEditorState::empty();
    s.add_pad_at(0.0, 0.0); // idx 0
    let moved = s.nudge_pads(&[0, 99], 0.5, -0.5);
    assert_eq!(moved, vec![0]);
    assert_eq!(s.pads[0].position_mm, (0.5, -0.5));
}

// Empty selection is a clean no-op: nothing moves, nothing returned.
#[test]
fn nudge_pads_empty_selection_is_noop() {
    let mut s = FootprintEditorState::empty();
    s.add_pad_at(0.0, 0.0);
    let moved = s.nudge_pads(&[], 1.0, 1.0);
    assert!(moved.is_empty());
    assert_eq!(s.pads[0].position_mm, (0.0, 0.0));
}

// Confirming the Move-By modal nudges the selection by the typed mm delta
// (not one grid step) and closes the modal.
#[test]
fn move_by_modal_nudges_by_typed_delta() {
    let mut s = FootprintEditorState::empty();
    s.add_pad_at(0.0, 0.0); // idx 0
    s.selected_pad = Some(0);
    s.move_by_modal = Some(MoveByModal {
        dx_buf: "2.5".into(),
        dy_buf: "-1.0".into(),
    });
    let (dx, dy) = s.move_by_modal.as_ref().unwrap().parsed().unwrap();
    let moved = s.nudge_pads(&[0], dx, dy);
    assert_eq!(moved, vec![0]);
    assert_eq!(s.pads[0].position_mm, (2.5, -1.0));
}

// v0.14 — Placing a text frame appends a silk Text carrying a
// Some(frame) box (item ③ bounding-box Text Frame place tool).
#[test]
fn place_text_frame_sets_frame_box() {
    use signex_library::primitive::footprint::FpGraphicKind;
    let mut fp = signex_library::primitive::footprint::Footprint::empty("FrameTool");
    crate::library::editor::footprint::text_frame::add_text_frame(&mut fp, 0.0, 0.0, 4.0, 2.0);
    match &fp.silk_f.last().unwrap().kind {
        FpGraphicKind::Text { frame, .. } => assert_eq!(*frame, Some((4.0, 2.0))),
        _ => panic!("expected Text"),
    }
}

// Task 6 — applying a footprint filter preset replaces the active
// selection-filter set with exactly the preset's kinds.
#[test]
fn apply_filter_preset_sets_state_filter() {
    use crate::library::editor::footprint::state::selection_filter::SelectionFilterKind as K;
    let mut s = FootprintEditorState::empty();
    s.selection_filter.set_all(true);
    let preset = crate::active_bar::FootprintFilterPreset {
        name: "pads".into(),
        kinds: vec![K::Pads],
    };
    crate::library::editor::footprint::filter_presets::apply_preset(&mut s, &preset);
    assert!(s.selection_filter.get(K::Pads));
    assert!(!s.selection_filter.get(K::Tracks));
}

// Issue #375 — the Place / Move active-bar button's left-click must
// arm PadsTool::Select (a footprint has no separate move tool; pad
// movement is drag-under-Select — see `active_bar_dropdowns.rs`'s
// `place_entries`), not fall through to the `ActiveBarStub` no-op.
//
// Pinned by POSITION, not by tooltip text: `dropdown_trigger_items`
// puts Filter, Snap, Place, Select, Align, Body3d, Text, Shapes in
// that exact order (the same order `dropdown_x_offset` documents and
// depends on for dropdown placement), so the Place/Move button is
// always index 2. A `tooltip.contains("Move")` assertion would
// couple this test to prose the tooltip-wording fix itself rewrites.
#[test]
fn place_move_button_left_click_arms_select_tool() {
    use crate::library::editor::footprint::state::PadsTool;
    use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    use signex_widgets::active_bar::ActiveBarItem;

    let ActiveBarItem::Button(place_btn) = place_move_button(default_editor()) else {
        panic!("index 2 should be the Place/Move button");
    };

    match place_btn.on_press {
        Some(LibraryMessage::PrimitiveEditorEvent {
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SetPadsTool(PadsTool::Select)),
            ..
        }) => {}
        other => panic!("expected left-click to arm PadsTool::Select, got {other:?}"),
    }
}

fn default_editor() -> crate::app::FootprintEditorState {
    let file = signex_library::FootprintFile::from_footprint(
        signex_library::primitive::footprint::Footprint::empty("Test"),
    );
    crate::app::FootprintEditorState::new(std::path::PathBuf::from("t.snxfpt"), file)
}

fn place_move_button(
    editor: crate::app::FootprintEditorState,
) -> signex_widgets::active_bar::ActiveBarItem<crate::library::messages::LibraryMessage> {
    use crate::library::editor::footprint::unified_active_bar::bar_items;
    use signex_types::theme::{ThemeId, theme_tokens};

    let tid = ThemeId::CatppuccinMocha;
    let tokens = theme_tokens(tid);
    bar_items(&editor, tid, &tokens).remove(2)
}

// ---------------------------------------------------------------------
// #146 — footprint context-menu / active-bar selection + dirty/history
// hygiene. These drive the real dispatcher
// (`apply_footprint_primitive_edit`) against the app-wrapper editor so
// the `dirty` flag and the undo `history` stack are observable.
// ---------------------------------------------------------------------

/// A wrapper editor pre-seeded with `n` pads at distinct positions and
/// reset to a clean state, so a test asserts the *dispatcher* — not the
/// fixture — is what dirties the document / stacks history.
fn editor_with_pads(n: usize) -> crate::app::FootprintEditorState {
    let mut e = default_editor();
    for i in 0..n {
        e.state.add_pad_at(i as f64 * 2.0, 0.0);
    }
    e.dirty = false;
    e.history.clear();
    e
}

#[test]
fn issue_146_rotate_with_no_selection_stays_clean() {
    use crate::library::editor::footprint::updates::apply_footprint_primitive_edit;
    use crate::library::messages::FootprintEditorMsg;
    let mut e = editor_with_pads(2);
    e.state.selected_pad = None;
    apply_footprint_primitive_edit(&mut e, FootprintEditorMsg::ActiveBarRotateSelection);
    assert!(!e.dirty, "no-op rotate must not dirty the document");
    assert!(
        e.history.is_empty(),
        "no-op rotate must not stack undo history"
    );
}

#[test]
fn issue_146_rotate_with_selection_dirties_and_snapshots_once() {
    use crate::library::editor::footprint::updates::apply_footprint_primitive_edit;
    use crate::library::messages::FootprintEditorMsg;
    let mut e = editor_with_pads(2);
    e.state.selected_pad = Some(0);
    apply_footprint_primitive_edit(&mut e, FootprintEditorMsg::ActiveBarRotateSelection);
    assert!(e.dirty, "a real rotate dirties the document");
    assert_eq!(
        e.history.len(),
        1,
        "exactly one undo snapshot per real rotate (no double-push)"
    );
}

#[test]
fn issue_146_align_to_grid_with_no_selection_stays_clean() {
    use crate::library::editor::footprint::updates::apply_footprint_primitive_edit;
    use crate::library::messages::FootprintEditorMsg;
    let mut e = editor_with_pads(1);
    e.state.selected_pad = None;
    apply_footprint_primitive_edit(&mut e, FootprintEditorMsg::ActiveBarAlignSelectionToGrid);
    assert!(!e.dirty);
    assert!(e.history.is_empty());
}

#[test]
fn issue_146_context_click_on_new_pad_clears_stale_extras() {
    use crate::library::editor::footprint::state::FootprintContextTarget;
    use crate::library::editor::footprint::updates::apply_footprint_primitive_edit;
    use crate::library::messages::FootprintEditorMsg;
    let mut e = editor_with_pads(3);
    e.state.selected_pad = Some(0);
    e.state.selected_pads_extra = vec![1, 2];
    // Right-click a different pad: the primary selection changes, so the
    // stale multi-select extras must be dropped.
    apply_footprint_primitive_edit(
        &mut e,
        FootprintEditorMsg::ShowContextMenu {
            x: 0.0,
            y: 0.0,
            target: FootprintContextTarget::Pad(1),
        },
    );
    assert_eq!(e.state.selected_pad, Some(1));
    assert!(
        e.state.selected_pads_extra.is_empty(),
        "a primary-selection change via right-click drops stale extras"
    );
}

#[test]
fn issue_146_context_select_all_fills_extras_like_active_bar() {
    use crate::library::editor::footprint::state::FootprintContextAction;
    use crate::library::editor::footprint::updates::apply_footprint_primitive_edit;
    use crate::library::messages::FootprintEditorMsg;
    let mut e = editor_with_pads(3);
    apply_footprint_primitive_edit(
        &mut e,
        FootprintEditorMsg::ContextMenuAction(FootprintContextAction::SelectAllPads),
    );
    // Same set the active-bar Select All produces: pad 0 primary, rest
    // in the extras.
    assert_eq!(e.state.selected_pad, Some(0));
    assert_eq!(e.state.selected_pads_extra, vec![1, 2]);
}

// ---------------------------------------------------------------------
// #370 — "Align…" dialog. These drive the real dispatcher
// (`apply_footprint_primitive_edit`) against the app-wrapper editor so
// `dirty`, the undo `history` stack, and the `align_modal` slot are all
// observable — matching the #146 hygiene tests above.
// ---------------------------------------------------------------------

/// A wrapper editor pre-seeded with pads at the given world-mm centres,
/// the whole set selected (pad 0 primary + the rest as extras), reset to
/// a clean dirty/history baseline so a test asserts the *dispatcher* is
/// what moves pads / stacks history.
fn editor_with_positions(positions: &[(f64, f64)]) -> crate::app::FootprintEditorState {
    let mut e = default_editor();
    for &(x, y) in positions {
        e.state.add_pad_at(x, y);
    }
    if !positions.is_empty() {
        e.state.selected_pad = Some(0);
        e.state.selected_pads_extra = (1..positions.len()).collect();
    }
    e.dirty = false;
    e.history.clear();
    e
}

#[test]
fn align_confirm_horizontal_matches_direct_align_pads() {
    use crate::library::editor::footprint::state::{AlignModal, AlignOp};
    use crate::library::editor::footprint::updates::apply_footprint_primitive_edit;
    use crate::library::messages::FootprintEditorMsg;

    let positions = [(0.0, 0.0), (4.0, 6.0), (10.0, 2.0)];

    // Confirm a horizontal-only op through the dialog...
    let mut dialog = editor_with_positions(&positions);
    dialog.state.align_modal = Some(AlignModal {
        horizontal: Some(AlignOp::Left),
        vertical: None,
    });
    apply_footprint_primitive_edit(&mut dialog, FootprintEditorMsg::AlignConfirm);

    // ...and dispatch the corresponding concrete dropdown row directly.
    let mut direct = editor_with_positions(&positions);
    apply_footprint_primitive_edit(&mut direct, FootprintEditorMsg::AlignPads(AlignOp::Left));

    let dialog_pos: Vec<_> = dialog.state.pads.iter().map(|p| p.position_mm).collect();
    let direct_pos: Vec<_> = direct.state.pads.iter().map(|p| p.position_mm).collect();
    assert_eq!(
        dialog_pos, direct_pos,
        "dialog confirm must apply the exact same transform as AlignPads"
    );
    assert!(
        dialog.state.align_modal.is_none(),
        "confirm closes the modal"
    );
}

#[test]
fn align_confirm_both_axes_is_one_undo_step() {
    use crate::library::editor::footprint::state::{AlignModal, AlignOp};
    use crate::library::editor::footprint::updates::apply_footprint_primitive_edit;
    use crate::library::messages::FootprintEditorMsg;

    let positions = [(0.0, 0.0), (4.0, 6.0), (10.0, 2.0)];
    let mut e = editor_with_positions(&positions);
    let before: Vec<_> = e.state.pads.iter().map(|p| p.position_mm).collect();

    // Distribute horizontally (keeps extremes, re-spaces middle) AND
    // centre vertically — a non-degenerate two-axis move.
    e.state.align_modal = Some(AlignModal {
        horizontal: Some(AlignOp::DistributeH),
        vertical: Some(AlignOp::CenterV),
    });
    apply_footprint_primitive_edit(&mut e, FootprintEditorMsg::AlignConfirm);

    assert!(e.dirty, "a real two-axis confirm dirties the document");
    assert_eq!(
        e.history.len(),
        1,
        "both axes under a single snapshot = exactly one undo step"
    );
    let after: Vec<_> = e.state.pads.iter().map(|p| p.position_mm).collect();
    assert_ne!(after, before, "both axes actually moved pads");

    // One undo fully restores the pre-confirm geometry.
    assert!(e.undo(), "the single snapshot is undoable");
    let restored: Vec<_> = e.state.pads.iter().map(|p| p.position_mm).collect();
    assert_eq!(restored, before, "one undo restores every pad centre");
    assert!(
        e.history.is_empty(),
        "one undo empties the one-entry history — proving it was one step"
    );
}

#[test]
fn align_confirm_both_axes_matches_two_sequential_align_pads() {
    use crate::library::editor::footprint::state::{AlignModal, AlignOp};
    use crate::library::editor::footprint::updates::apply_footprint_primitive_edit;
    use crate::library::messages::FootprintEditorMsg;

    // The dialog's headline claim: applying a horizontal AND a vertical op
    // in one confirm lands EXACTLY where picking the two concrete dropdown
    // rows one at a time would. H touches only X, V only Y, so the two are
    // axis-independent and the results must be bit-identical (no rounding
    // divergence — each partial delta is exactly 0.0 on the untouched axis).
    let positions = [(0.0, 0.0), (4.0, 6.0), (10.0, 2.0)];

    // Both axes at once through the dialog...
    let mut dialog = editor_with_positions(&positions);
    dialog.state.align_modal = Some(AlignModal {
        horizontal: Some(AlignOp::DistributeH),
        vertical: Some(AlignOp::CenterV),
    });
    apply_footprint_primitive_edit(&mut dialog, FootprintEditorMsg::AlignConfirm);

    // ...vs the two concrete dropdown rows, one at a time.
    let mut sequential = editor_with_positions(&positions);
    apply_footprint_primitive_edit(
        &mut sequential,
        FootprintEditorMsg::AlignPads(AlignOp::DistributeH),
    );
    apply_footprint_primitive_edit(
        &mut sequential,
        FootprintEditorMsg::AlignPads(AlignOp::CenterV),
    );

    let dialog_pos: Vec<_> = dialog.state.pads.iter().map(|p| p.position_mm).collect();
    let sequential_pos: Vec<_> = sequential.state.pads.iter().map(|p| p.position_mm).collect();
    assert_eq!(
        dialog_pos, sequential_pos,
        "a both-axes dialog confirm must land bit-identically to two AlignPads picks (H then V)"
    );
}

#[test]
fn align_confirm_neither_axis_is_clean_noop() {
    use crate::library::editor::footprint::state::AlignModal;
    use crate::library::editor::footprint::updates::apply_footprint_primitive_edit;
    use crate::library::messages::FootprintEditorMsg;

    let positions = [(0.0, 0.0), (4.0, 6.0)];
    let mut e = editor_with_positions(&positions);
    e.state.align_modal = Some(AlignModal::default()); // both axes None

    apply_footprint_primitive_edit(&mut e, FootprintEditorMsg::AlignConfirm);

    assert!(
        !e.dirty,
        "confirming with neither axis chosen must not dirty"
    );
    assert!(
        e.history.is_empty(),
        "neither-axis confirm pushes no history entry"
    );
    assert!(
        e.state.align_modal.is_none(),
        "confirm still closes the modal even on a no-op"
    );
}

#[test]
fn align_confirm_below_size_gate_pushes_no_history() {
    use crate::library::editor::footprint::state::{AlignModal, AlignOp};
    use crate::library::editor::footprint::updates::apply_footprint_primitive_edit;
    use crate::library::messages::FootprintEditorMsg;

    // DistributeH needs ≥3 pads; with only 2 selected the chosen op
    // can't apply, so the confirm must stay a clean no-op.
    let positions = [(0.0, 0.0), (4.0, 0.0)];
    let mut e = editor_with_positions(&positions);
    e.state.align_modal = Some(AlignModal {
        horizontal: Some(AlignOp::DistributeH),
        vertical: None,
    });

    apply_footprint_primitive_edit(&mut e, FootprintEditorMsg::AlignConfirm);

    assert!(!e.dirty, "a confirm that can't apply must not dirty");
    assert!(
        e.history.is_empty(),
        "a below-gate confirm pushes no history"
    );
    assert!(
        e.state.align_modal.is_none(),
        "confirm still closes the modal"
    );
}

#[test]
fn align_cancel_closes_modal_and_keeps_selection() {
    use crate::library::editor::footprint::state::{AlignModal, AlignOp};
    use crate::library::editor::footprint::updates::apply_footprint_primitive_edit;
    use crate::library::messages::FootprintEditorMsg;

    let positions = [(0.0, 0.0), (4.0, 6.0)];
    let mut e = editor_with_positions(&positions);
    e.state.align_modal = Some(AlignModal {
        horizontal: Some(AlignOp::Left),
        vertical: None,
    });
    let selected_pad = e.state.selected_pad;
    let extras = e.state.selected_pads_extra.clone();
    let positions_before: Vec<_> = e.state.pads.iter().map(|p| p.position_mm).collect();

    apply_footprint_primitive_edit(&mut e, FootprintEditorMsg::AlignCancel);

    assert!(e.state.align_modal.is_none(), "cancel closes the modal");
    assert_eq!(
        e.state.selected_pad, selected_pad,
        "cancel leaves the primary selection untouched"
    );
    assert_eq!(
        e.state.selected_pads_extra, extras,
        "cancel leaves the multi-selection untouched"
    );
    let positions_after: Vec<_> = e.state.pads.iter().map(|p| p.position_mm).collect();
    assert_eq!(positions_after, positions_before, "cancel moves no pads");
    assert!(!e.dirty, "cancel does not dirty");
    assert!(e.history.is_empty(), "cancel pushes no history");
}
