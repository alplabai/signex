//! #2513 — a track or mixed-selection drag that bends a track at a 45°
//! corner is logged as an offset move with its corner rule, so a Command
//! Log Replay rebuilds the same bent tracks, whatever the router's rule is
//! by then.

use signex_types::corner::{CornerMode, CornerRule};

use super::*;
use crate::panels::command_log::{CommandLogMsg, LogRow};

/// Every track of the board as `[start x, start y, end x, end y]`, rounded to
/// the nanometre and sorted, ids left out: new legs take new ids.
fn track_lines(r: &Rig) -> Vec<[i64; 4]> {
    let nm = |v: f64| (v * 1e6).round() as i64;
    let mut lines: Vec<[i64; 4]> = (r.board().segments.iter())
        .map(|s| [nm(s.start.x), nm(s.start.y), nm(s.end.x), nm(s.end.y)])
        .collect();
    lines.sort_unstable();
    lines
}

fn newest(r: &Rig) -> LogRow {
    let rows = r.app.ui_state.command_log.log.rows();
    rows.last().expect("a row").clone()
}

/// Drag as `drag` does, check the row it logged, undo it, turn the
/// router's rule to 45° start, and replay the row: the board must come back
/// to what the drag left.
fn assert_drag_replays_alike(r: &mut Rig, drag: impl FnOnce(&mut Rig)) {
    let before = r.live();
    drag(r);
    let bent = track_lines(r);
    let after = r.live();
    assert_ne!(after, before, "the drag moved something");
    let row = newest(r);
    assert_eq!(row.command, "move_selection");
    r.undo();
    assert_eq!(r.live(), before, "one undo step");
    // The drag took the default rule (45° end); a replay must not read this.
    r.app.interaction_state.corner_rule = CornerRule {
        mode: CornerMode::Angle45Start,
        is_flipped: false,
    };

    let _ = (r.app).handle_command_log_message(CommandLogMsg::ReplayRequested(row.seq));

    let ran = newest(r);
    assert!(!ran.outcome.is_failed(), "{:?}", ran.outcome);
    assert_eq!(track_lines(r), bent, "the replay bends the tracks alike");
    r.assert_all_at_legal_angles();
}

/// An unselected track dragged 3 mm up bends the two tracks joined to it;
/// its Replay bends them the same way.
#[test]
fn a_track_drag_with_a_corner_replays_with_the_same_bend() {
    let mut r = rig();
    assert_drag_replays_alike(&mut r, |r| r.drag(p(7.5, 0.0), p(7.5, 3.0)));
    assert_eq!(r.track(DRAGGED), (p(5.0, 3.0), p(10.0, 3.0)));
    assert_eq!(r.track(ON_PAD), (p(0.0, 0.0), p(2.0, 0.0)));
    assert_eq!(r.tracks_between(p(2.0, 0.0), p(5.0, 3.0)), 1);
    assert_eq!(r.track(TO_VIA), (p(13.0, 0.0), p(15.0, 0.0)));
    assert_eq!(r.tracks_between(p(10.0, 3.0), p(13.0, 0.0)), 1);
}

/// A selected track and via dragged together: the track between them moves
/// whole, the tracks outside bend; its Replay does the same.
#[test]
fn a_mixed_drag_with_a_corner_replays_with_the_same_bend() {
    let mut r = rig();
    assert_drag_replays_alike(&mut r, |r| {
        r.click(p(7.5, 0.0), pcb_geom::ClickMode::Replace);
        r.click(p(15.0, 0.0), pcb_geom::ClickMode::Toggle);
        let selection = r.app.interaction_state.pcb_selection.selection().clone();
        assert!(selection.contains(pcb_geom::ItemRef::Track(DRAGGED)));
        assert!(selection.contains(pcb_geom::ItemRef::Via(VIA)));
        r.drag(p(7.5, 0.0), p(7.5, 3.0));
    });
    let via = r.board().vias.iter().find(|v| v.uuid == VIA).unwrap();
    assert_eq!(via.position, p(15.0, 3.0), "the via moved");
    assert_eq!(r.track(DRAGGED), (p(5.0, 3.0), p(10.0, 3.0)));
    assert_eq!(r.track(TO_VIA), (p(10.0, 3.0), p(15.0, 3.0)), "whole");
    assert_eq!(r.track(ON_PAD), (p(0.0, 0.0), p(2.0, 0.0)));
    assert_eq!(r.tracks_between(p(2.0, 0.0), p(5.0, 3.0)), 1);
    assert_eq!(r.track(FROM_VIA), (p(18.0, 0.0), p(20.0, 0.0)));
    assert_eq!(r.tracks_between(p(15.0, 3.0), p(18.0, 0.0)), 1);
}
