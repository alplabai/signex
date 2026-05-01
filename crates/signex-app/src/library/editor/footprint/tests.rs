//! Footprint editor tests.
//!
//! Coverage focuses on the layer visibility helpers + the pad
//! hit-test surface; the smaller `from_footprint` / `add_pad_at` /
//! `sync_pads_to_primitive` flow is covered in `state.rs` itself.

use super::layers::{FpLayer, LayerVisibility};
use super::state::FootprintEditorState;

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
