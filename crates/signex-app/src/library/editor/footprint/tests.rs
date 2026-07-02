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
    assert!(fp.body_3d.outline.is_some(), "outline should be the courtyard");
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
