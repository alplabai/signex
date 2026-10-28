//! #1473 — the outline covers abutting pads or says which it left out.

use super::FootprintEditorState;
use super::courtyard::CourtyardOutlineError;

fn row(count: usize, pitch: f64, size: (f64, f64)) -> FootprintEditorState {
    let mut s = FootprintEditorState::empty();
    for i in 0..count {
        s.add_pad_at(i as f64 * pitch, 0.0);
    }
    for (i, pad) in s.pads.iter_mut().enumerate() {
        pad.size_mm = size;
        pad.number = format!("P{}", i + 1);
    }
    s
}

#[test]
fn abutting_pads_yield_one_outline_covering_all() {
    // 2 mm pads on a 2 mm pitch share edges.
    let mut s = row(4, 2.0, (2.0, 2.0));
    s.recompute_courtyard_outline().expect("one outline");
    let outline = s.courtyard_outline_mm.as_ref().expect("an outline");
    let (min_x, max_x) = outline.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
        (lo.min(p.0), hi.max(p.0))
    });
    // Pads span x in [-1, 7]; the outline reaches past both ends.
    assert!(min_x < -1.0 && max_x > 7.0, "x {min_x}..{max_x}");
}

#[test]
fn a_separate_island_is_reported_not_swallowed() {
    let mut s = row(2, 2.0, (2.0, 2.0));
    s.add_pad_at(40.0, 0.0);
    s.pads[2].size_mm = (2.0, 2.0);
    s.pads[2].number = "FAR".into();
    let err = s.recompute_courtyard_outline().unwrap_err();
    assert!(s.courtyard_outline_mm.is_none());
    match err {
        CourtyardOutlineError::PadsLeftOut(pads) => assert_eq!(pads, vec!["FAR".to_string()]),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn no_pads_is_an_error() {
    let mut s = FootprintEditorState::empty();
    assert_eq!(
        s.recompute_courtyard_outline(),
        Err(CourtyardOutlineError::NoPads)
    );
}
