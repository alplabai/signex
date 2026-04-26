//! Footprint editor state + round-trip tests.

use super::layers::{FpLayer, LayerVisibility};
use super::state::{CourtyardRect, FootprintEditorState};

const SAMPLE_FOOTPRINT: &str = r#"
(footprint "Resistor_SMD:R_0603"
    (layer "F.Cu")
    (pad "1" smd rect (at -0.825 0) (size 0.8 0.8) (layers "F.Cu" "F.Mask" "F.Paste"))
    (pad "2" smd rect (at 0.825 0) (size 0.8 0.8) (layers "F.Cu" "F.Mask" "F.Paste"))
    (fp_line (start -1.5 0.7) (end 1.5 0.7) (layer "F.SilkS") (stroke (width 0.12)))
)"#;

#[test]
fn empty_sexpr_yields_empty_state() {
    let s = FootprintEditorState::from_sexpr("");
    assert!(s.pads.is_empty());
    assert!(s.graphics.is_empty());
    assert!(s.courtyard_mm.is_none());
    assert!(s.auto_fit_courtyard);
}

#[test]
fn malformed_sexpr_falls_back_to_empty() {
    let s = FootprintEditorState::from_sexpr("(definitely not a valid footprint");
    assert!(s.pads.is_empty());
}

#[test]
fn parses_two_smd_pads_from_sample() {
    let s = FootprintEditorState::from_sexpr(SAMPLE_FOOTPRINT);
    assert_eq!(s.pads.len(), 2);
    assert_eq!(s.pads[0].number, "1");
    assert_eq!(s.pads[1].number, "2");
    // Pad-1 sits at -0.825 mm in X.
    assert!((s.pads[0].position_mm.0 - -0.825).abs() < 1e-9);
    // F.SilkS line preserved as a graphic.
    assert!(s.graphics.iter().any(|g| g.layer == FpLayer::FSilks));
}

#[test]
fn auto_fit_courtyard_wraps_pads_with_slack() {
    let s = FootprintEditorState::from_sexpr(SAMPLE_FOOTPRINT);
    let c = s.courtyard_mm.expect("courtyard");
    // The two pads span X ∈ [-1.225, 1.225] (centre ± 0.4) and Y
    // ∈ [-0.4, 0.4]. The courtyard slack is 0.25 mm.
    assert!(c.min_x < -1.4);
    assert!(c.max_x > 1.4);
    assert!(c.min_y < -0.6);
    assert!(c.max_y > 0.6);
}

#[test]
fn next_pad_number_increments_max_int() {
    let mut s = FootprintEditorState::empty();
    assert_eq!(s.next_pad_number(), "1");
    s.add_pad_at(0.0, 0.0);
    assert_eq!(s.next_pad_number(), "2");
    s.add_pad_at(1.0, 0.0);
    assert_eq!(s.next_pad_number(), "3");
}

#[test]
fn next_pad_number_skips_non_integer_pads() {
    let mut s = FootprintEditorState::empty();
    s.add_pad_at(0.0, 0.0); // "1"
    // Manually rename to a non-integer; the next number should still
    // come from the integer pad currently at "1".
    s.pads[0].number = "GND".to_string();
    assert_eq!(s.next_pad_number(), "1");
}

#[test]
fn add_pad_recomputes_courtyard() {
    let mut s = FootprintEditorState::empty();
    assert!(s.courtyard_mm.is_none());
    s.add_pad_at(0.0, 0.0);
    let c1 = s.courtyard_mm.unwrap();
    s.add_pad_at(5.0, 0.0);
    let c2 = s.courtyard_mm.unwrap();
    assert!(c2.max_x > c1.max_x, "courtyard should grow with second pad");
}

#[test]
fn move_pad_updates_courtyard() {
    let mut s = FootprintEditorState::empty();
    let idx = s.add_pad_at(0.0, 0.0);
    let c1 = s.courtyard_mm.unwrap();
    s.move_pad(idx, 10.0, 0.0);
    let c2 = s.courtyard_mm.unwrap();
    assert!(c2.max_x > c1.max_x, "courtyard right edge should follow pad");
    assert!(c2.min_x > c1.min_x, "courtyard left edge should also shift right");
}

#[test]
fn delete_pad_clears_selection_and_shrinks_courtyard() {
    let mut s = FootprintEditorState::empty();
    let i0 = s.add_pad_at(0.0, 0.0);
    let i1 = s.add_pad_at(5.0, 0.0);
    assert_eq!(s.selected_pad, Some(i1));
    let c1 = s.courtyard_mm.unwrap();
    s.delete_pad(i1);
    assert_eq!(s.pads.len(), 1);
    assert!(s.selected_pad.is_none());
    let c2 = s.courtyard_mm.unwrap();
    assert!(c2.max_x < c1.max_x, "courtyard right edge should shrink");
    // i0 is still valid.
    assert_eq!(i0, 0);
}

#[test]
fn delete_out_of_range_is_noop() {
    let mut s = FootprintEditorState::empty();
    s.delete_pad(99);
    assert!(s.pads.is_empty());
    s.add_pad_at(0.0, 0.0);
    s.delete_pad(99);
    assert_eq!(s.pads.len(), 1);
}

#[test]
fn pad_at_returns_topmost() {
    let mut s = FootprintEditorState::empty();
    s.add_pad_at(0.0, 0.0); // i=0 (1×1 mm centred at origin)
    s.add_pad_at(0.0, 0.0); // i=1 (overlaps)
    // The later-added pad wins.
    assert_eq!(s.pad_at(0.0, 0.0), Some(1));
}

#[test]
fn pad_at_skips_hidden_layers() {
    let mut s = FootprintEditorState::empty();
    s.add_pad_at(0.0, 0.0); // SMD on F.Cu
    s.layer_visibility = LayerVisibility {
        f_cu: false,
        ..Default::default()
    };
    assert_eq!(s.pad_at(0.0, 0.0), None);
}

#[test]
fn pad_at_misses_outside_bbox() {
    let mut s = FootprintEditorState::empty();
    s.add_pad_at(0.0, 0.0);
    assert!(s.pad_at(2.0, 2.0).is_none());
}

#[test]
fn toggle_auto_fit_is_idempotent_pair() {
    let mut s = FootprintEditorState::from_sexpr(SAMPLE_FOOTPRINT);
    let before = s.courtyard_mm;
    s.toggle_auto_fit();
    s.toggle_auto_fit();
    assert_eq!(s.auto_fit_courtyard, true);
    let after = s.courtyard_mm;
    // Auto-fit re-runs on second toggle, so values should be equal.
    let (b, a) = (before.unwrap(), after.unwrap());
    let approx_eq = |x: f64, y: f64| (x - y).abs() < 1e-9;
    assert!(approx_eq(b.min_x, a.min_x));
    assert!(approx_eq(b.max_x, a.max_x));
}

#[test]
fn round_trip_through_sexpr_preserves_pads() {
    let s = FootprintEditorState::from_sexpr(SAMPLE_FOOTPRINT);
    let out = s.to_sexpr();
    let s2 = FootprintEditorState::from_sexpr(&out);
    assert_eq!(s2.pads.len(), s.pads.len());
    assert_eq!(s2.pads[0].number, s.pads[0].number);
    assert_eq!(s2.pads[1].number, s.pads[1].number);
    assert!((s2.pads[0].position_mm.0 - s.pads[0].position_mm.0).abs() < 1e-3);
    assert!((s2.pads[0].size_mm.0 - s.pads[0].size_mm.0).abs() < 1e-3);
}

#[test]
fn round_trip_preserves_silk_graphic() {
    let s = FootprintEditorState::from_sexpr(SAMPLE_FOOTPRINT);
    let out = s.to_sexpr();
    let s2 = FootprintEditorState::from_sexpr(&out);
    assert!(s2.graphics.iter().any(|g| g.layer == FpLayer::FSilks));
}

#[test]
fn round_trip_writes_courtyard_as_edge_cuts_lines() {
    let mut s = FootprintEditorState::empty();
    s.add_pad_at(0.0, 0.0);
    let out = s.to_sexpr();
    // 4 fp_lines forming the courtyard rectangle.
    let count = out.matches("Edge.Cuts").count();
    assert_eq!(count, 4, "expected 4 Edge.Cuts strokes for courtyard");
}

#[test]
fn empty_state_serializes_without_panic() {
    let s = FootprintEditorState::empty();
    let out = s.to_sexpr();
    assert!(out.starts_with("(footprint"));
    assert!(out.ends_with(')'));
}

#[test]
fn layer_visibility_toggles_individually() {
    let mut v = LayerVisibility::default();
    assert!(v.get(FpLayer::FCu));
    v.toggle(FpLayer::FCu);
    assert!(!v.get(FpLayer::FCu));
    v.toggle(FpLayer::BCu);
    assert!(v.get(FpLayer::BCu));
}

#[test]
fn fplayer_round_trips_through_standard_name() {
    for layer in FpLayer::ORDER {
        let n = layer.standard_name();
        let parsed = FpLayer::from_standard_name(n);
        assert_eq!(parsed, Some(*layer));
    }
}

#[test]
fn auto_fit_off_freezes_courtyard() {
    let mut s = FootprintEditorState::empty();
    s.add_pad_at(0.0, 0.0);
    let c = s.courtyard_mm.unwrap();
    s.toggle_auto_fit(); // turn off
    s.add_pad_at(20.0, 0.0);
    let c2 = s.courtyard_mm.unwrap();
    assert!(approx_eq(c2.min_x, c.min_x));
    assert!(approx_eq(c2.max_x, c.max_x));
}

fn approx_eq(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn courtyard_rect_min_max_invariant() {
    let mut s = FootprintEditorState::empty();
    s.add_pad_at(-2.0, -3.0);
    s.add_pad_at(4.0, 5.0);
    let CourtyardRect {
        min_x,
        min_y,
        max_x,
        max_y,
    } = s.courtyard_mm.unwrap();
    assert!(min_x < max_x);
    assert!(min_y < max_y);
}
