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

/// #1473 — the failure reaches the Messages panel, not only the return value.
#[test]
fn a_left_out_pad_is_reported_to_the_messages_panel() {
    use crate::ignore::IgnoreResult;
    use crate::library::editor::footprint::updates::apply_footprint_primitive_edit;
    use crate::library::messages::FootprintEditorMsg;
    use signex_library::primitive::footprint::{Footprint, FootprintFile};

    crate::diagnostics::init_logging().ignore("the logger may already be installed");
    let mut editor = crate::app::FootprintDocument::new(
        std::path::PathBuf::from("courtyard-island.snxfpt"),
        FootprintFile::from_footprint(Footprint::empty("island")),
    );
    for (x, number) in [(0.0, "NEAR1"), (2.0, "NEAR2"), (40.0, "ISLAND_PAD")] {
        let i = editor.state.add_pad_at(x, 0.0);
        editor.state.pads[i].size_mm = (2.0, 2.0);
        editor.state.pads[i].number = number.into();
    }
    editor.dirty = false;

    apply_footprint_primitive_edit(&mut editor, FootprintEditorMsg::RecomputeCourtyardOutline);

    let reported = crate::diagnostics::recent_entries()
        .iter()
        .any(|e| e.message.contains("ISLAND_PAD") && e.message.contains("Courtyard outline"));
    assert!(reported, "the left-out pad must reach the Messages panel");
    assert!(
        !editor.dirty,
        "recomputing the outline must not dirty the file"
    );
}

/// A pad moved after the outline was computed hides the outline.
#[test]
fn the_outline_is_hidden_once_a_pad_moves() {
    let mut s = row(2, 2.0, (2.0, 2.0));
    s.recompute_courtyard_outline().expect("one outline");
    assert!(s.courtyard_outline().is_some());
    s.pads[0].position_mm.0 += 0.5;
    assert!(s.courtyard_outline().is_none());
}
