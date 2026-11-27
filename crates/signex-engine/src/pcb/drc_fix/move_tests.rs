//! #2940: the fixes that move an item clear: a track or via too near other
//! copper, a via hole too near another hole, a board string too near a pad.

use serde_json::json;
use signex_drc::{Limit, ViolationKind};
use uuid::Uuid;

use super::tests::{
    assert_clean_fix, assert_left, board, fix, fix_every, part, rules, smd_pad, track, via,
};
use super::*;

/// Tracks of nets A and B 0.1 mm apart under the 0.2 mm clearance: the
/// first moves 0.101 mm away, the least that clears it on the stored nm.
#[test]
fn a_track_too_near_another_net_moves_the_least_that_clears_it() {
    let before = board(json!({"segments": [
        track(11, (0.0, 0.0), (10.0, 0.0), 1),
        track(12, (0.0, 0.35), (10.0, 0.35), 2),
    ]}));
    let rules = rules(&before, Vec::new());
    let run = fix_every(&before, &rules, FixRule::Clearance);
    assert_clean_fix(&before, &rules, &run);
    let moved = &run.board.segments[0];
    assert_eq!((moved.start.y, moved.end.y), (-0.101, -0.101));
    assert_eq!(run.board.segments[1].start.y, 0.35, "the other stays");
    assert_eq!(run.fixed[0].changed, [Uuid::from_u128(11).to_string()]);
}

/// The track joined to a moved track's end follows it, so the net stays
/// in one piece.
#[test]
fn the_track_joined_to_a_moved_track_follows_its_end() {
    let before = board(json!({"segments": [
        track(11, (0.0, 0.0), (10.0, 0.0), 1),
        track(13, (10.0, 0.0), (10.0, -5.0), 1),
        track(12, (0.0, 0.35), (9.0, 0.35), 2),
    ]}));
    let rules = rules(&before, Vec::new());
    let run = fix_every(&before, &rules, FixRule::Clearance);
    assert_clean_fix(&before, &rules, &run);
    let joined = &run.board.segments[1];
    assert_eq!(joined.start, run.board.segments[0].end, "still joined");
    assert_eq!(joined.end.y, -5.0, "its far end stays");
    let changed = &run.fixed[0].changed;
    assert!(
        changed.contains(&Uuid::from_u128(13).to_string()),
        "{changed:?}"
    );
}

/// Moving A down would come too near C below it, so B moves up instead;
/// with B locked the finding is left, saying the fix makes a new one.
#[test]
fn a_move_that_makes_a_new_finding_is_not_made() {
    let segments = |b_locked: bool| {
        let mut b = track(12, (0.0, 0.35), (10.0, 0.35), 2);
        b["locked"] = json!(b_locked);
        json!({"segments": [
            track(11, (0.0, 0.0), (10.0, 0.0), 1),
            b,
            track(14, (0.0, -0.45), (10.0, -0.45), 3),
        ]})
    };
    let before = board(segments(false));
    let rules = rules(&before, Vec::new());
    let run = fix_every(&before, &rules, FixRule::Clearance);
    assert_clean_fix(&before, &rules, &run);
    assert_eq!(run.fixed[0].changed, [Uuid::from_u128(12).to_string()]);
    assert_eq!(run.board.segments[0].start.y, 0.0, "A stays");

    let before = board(segments(true));
    let run = fix_every(&before, &rules, FixRule::Clearance);
    assert_left(&before, &run, ViolationKind::Clearance, "new_violation");
}

#[test]
fn a_finding_between_two_locked_tracks_is_left() {
    let (mut a, mut b) = (
        track(11, (0.0, 0.0), (10.0, 0.0), 1),
        track(12, (0.0, 0.35), (10.0, 0.35), 2),
    );
    a["locked"] = json!(true);
    b["locked"] = json!(true);
    let before = board(json!({"segments": [a, b]}));
    let rules = rules(&before, Vec::new());
    let run = fix_every(&before, &rules, FixRule::Clearance);
    assert_left(&before, &run, ViolationKind::Clearance, "locked");
}

/// One move clears two findings; the second says which fix cleared it.
#[test]
fn a_finding_cleared_by_an_earlier_fix_names_it() {
    let before = board(json!({"segments": [
        track(11, (0.0, 0.0), (10.0, 0.0), 1),
        track(12, (0.0, 0.35), (4.0, 0.35), 2),
        track(13, (6.0, 0.35), (10.0, 0.35), 3),
    ]}));
    let rules = rules(&before, Vec::new());
    let run = fix(&before, &rules, FixScope::Every(None));
    assert_clean_fix(&before, &rules, &run);
    assert_eq!(run.fixed.len(), 2);
    assert_eq!(run.fixed[1].cleared_by.as_ref(), Some(&run.fixed[0].key));
    assert!(run.fixed[1].changed.is_empty());
}

/// Two 0.4 mm vias of two nets, 0.7 mm apart: clear of each other's copper,
/// their holes 0.4 mm apart.
fn near_vias(locked: bool) -> serde_json::Value {
    let mut vias = [via(21, (5.0, 5.0), 1), via(22, (5.7, 5.0), 2)];
    for v in &mut vias {
        v["diameter"] = json!(0.4);
        v["locked"] = json!(locked);
    }
    json!({"vias": vias})
}

/// Holes 0.4 mm apart under a 0.5 mm rule: the first via moves away the
/// least that clears it.
#[test]
fn a_via_too_near_another_hole_moves_clear() {
    let before = board(near_vias(false));
    let rules = rules(&before, vec![Limit::MinHoleToHole(500_000)]);
    let run = fix_every(&before, &rules, FixRule::HoleToHole);
    assert_clean_fix(&before, &rules, &run);
    assert_eq!(run.board.vias[0].position.x, 5.0 - 0.101);
    assert_eq!(run.board.vias[1].position.x, 5.7, "the other stays");
}

#[test]
fn two_locked_vias_too_near_each_other_are_left() {
    let before = board(near_vias(true));
    let rules = rules(&before, vec![Limit::MinHoleToHole(500_000)]);
    let run = fix_every(&before, &rules, FixRule::HoleToHole);
    assert_left(&before, &run, ViolationKind::HoleToHole, "locked");
}

/// A board string 1 mm high over the top 0.1 mm of a pad moves off it,
/// straight up, past the rule.
#[test]
fn a_board_string_on_a_pad_moves_clear_of_it() {
    let label = json!({
        "uuid": Uuid::from_u128(41), "text": "R", "position": {"x": 0.0, "y": 0.9},
        "layer": "TopSilkscreen", "font_size": 1.0, "rotation": 0.0
    });
    let before = board(json!({
        "footprints": [part(31, (0.0, 0.0), smd_pad(32))],
        "texts": [label],
    }));
    let rules = rules(&before, vec![Limit::MinSilkToPad(200_000)]);
    assert_eq!(
        signex_drc::run(&before, &rules).count(ViolationKind::SilkToPad),
        1
    );
    let run = fix_every(&before, &rules, FixRule::SilkToPad);
    assert_clean_fix(&before, &rules, &run);
    let moved = run.board.texts[0].position;
    assert!(moved.x == 0.0 && moved.y > 1.2, "straight up: {moved:?}");
    // The least move: a 0.21 mm rule still finds it.
    let stricter = rules_with(&run.board, 210_000);
    assert_eq!(
        signex_drc::run(&run.board, &stricter).count(ViolationKind::SilkToPad),
        1
    );
}

fn rules_with(board: &signex_types::pcb::PcbBoard, silk_to_pad: i64) -> signex_drc::DrcRules {
    rules(board, vec![Limit::MinSilkToPad(silk_to_pad)])
}

/// A part's own silkscreen over its pad is the footprint's to change: the
/// finding is left.
#[test]
fn a_parts_own_silkscreen_is_left() {
    let mut fp = part(31, (0.0, 0.0), smd_pad(32));
    fp["graphics"] = json!([{
        "graphic_type": "line", "layer": "TopSilkscreen", "width": 0.15,
        "start": {"x": -1.0, "y": 0.0}, "end": {"x": 1.0, "y": 0.0}
    }]);
    let before = board(json!({"footprints": [fp]}));
    let rules = rules(&before, vec![Limit::MinSilkToPad(200_000)]);
    let run = fix_every(&before, &rules, FixRule::SilkToPad);
    assert_left(&before, &run, ViolationKind::SilkToPad, "cannot_change");
}
