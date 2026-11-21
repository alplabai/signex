//! Caner 2026-11-21: while a route is drawn, the routed net's line leaves
//! from the route's end to the nearest pad its start does not reach.

use super::*;
use crate::ratsnest::ratsnest;
use crate::tool_fixture::{MM, empty_board, pad_part, track};
use crate::zone_fill::zone_fills;

fn at(x: i64, y: i64) -> PointNm {
    PointNm::new(x * MM, y * MM)
}

/// Net 1 pads at (0, 0), (10, 0) and (20, 0); net 2 pads at (0, 10) and
/// (5, 10).
fn board() -> PcbBoard {
    let mut board = empty_board();
    board.footprints = vec![
        pad_part(10, (0.0, 0.0), 1.0, 1),
        pad_part(20, (10.0, 0.0), 1.0, 1),
        pad_part(30, (20.0, 0.0), 1.0, 1),
        pad_part(40, (0.0, 10.0), 1.0, 2),
        pad_part(50, (5.0, 10.0), 1.0, 2),
    ];
    board
}

#[test]
fn the_routed_line_leaves_the_route_end_for_the_nearest_open_pad() {
    let board = board();
    let base = ratsnest(&board);
    let rats = route_rats(&board, &zone_fills(&board), 1, at(0, 0));
    let lines = rats.lines(&base, at(7, 3));
    let routed: Vec<_> = lines.iter().filter(|l| l.net == 1).collect();
    assert!(
        routed.iter().any(|l| l.from == at(7, 3) && l.to == at(10, 0)),
        "{routed:?}"
    );
    assert!(
        routed.iter().all(|l| l.from != at(0, 0) && l.to != at(0, 0)),
        "no line leaves the start pad: {routed:?}"
    );
    assert!(
        routed.iter().any(|l| l.from == at(10, 0) && l.to == at(20, 0)),
        "the net's other lines stay"
    );
    let others = |ls: &[RatLine]| ls.iter().filter(|l| l.net == 2).copied().collect::<Vec<_>>();
    assert_eq!(others(&lines), others(&base), "other nets are unchanged");
}

/// Copper already joined to the start counts as the start: the line goes
/// to the nearest pad off it, not back to a pad the track reaches.
#[test]
fn pads_the_start_copper_reaches_are_not_targets() {
    let mut board = board();
    board.segments = vec![track(90, (0.0, 0.0), (10.0, 0.0), 1)];
    let rats = route_rats(&board, &zone_fills(&board), 1, at(0, 0));
    let line = rats.line_from(at(11, 2)).expect("one pad is open");
    assert_eq!(line.to, at(20, 0));
    assert!(route_rats(&board, &zone_fills(&board), 0, at(0, 0)).line_from(at(1, 1)).is_none());
}
