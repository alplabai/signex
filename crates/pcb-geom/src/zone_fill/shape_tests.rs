//! #2578 pour corners and #2579 hatched fill, through `zone_fills`: what
//! the canvas, the fabrication and PDF outputs, the ratsnest and stitching
//! all read.

use signex_types::pcb::{PcbBoard, Point};
use signex_types::zone_pour::POUR_FILL;
use signex_types::zone_pour_shape::{PourCorners, PourFillMode};

use super::super::{Unfilled, zone_fills};
use super::*;
use crate::tool_fixture::{MM, empty_board, zone};

/// A net-1 pour on the top copper over `outline` (mm), nothing else on the
/// board.
fn board_with(outline: &[(f64, f64)]) -> PcbBoard {
    let mut pour = zone(1, 1, POUR_FILL, (0.0, 0.0, 1.0, 1.0));
    pour.outline = outline.iter().map(|&(x, y)| Point::new(x, y)).collect();
    let mut board = empty_board();
    board.zones = vec![pour];
    board
}

const SQUARE: [(f64, f64); 4] = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];

fn fill(board: &PcbBoard) -> Vec<Polygon> {
    zone_fills(board)
        .of(0)
        .expect("the pour is filled")
        .pieces
        .clone()
}

fn twice_area(pieces: &[Polygon]) -> i128 {
    pieces.iter().map(Polygon::twice_area).sum()
}

fn inside(pieces: &[Polygon], x: f64, y: f64) -> bool {
    let p = PointNm::new((x * MM as f64) as i64, (y * MM as f64) as i64);
    pieces.iter().any(|piece| piece.contains_point(p))
}

/// A chamfer of 1 mm cuts a right triangle of 0.5 mm² off each of the four
/// corners; the middle of each edge keeps its copper.
#[test]
fn a_chamfer_cuts_each_convex_corner_back_by_its_size() {
    let mut board = board_with(&SQUARE);
    board.zones[0].corners = PourCorners::Chamfer;
    board.zones[0].corner_size = 1.0;
    let pieces = fill(&board);
    let full = 2 * 100 * i128::from(MM) * i128::from(MM);
    let cut = 4 * i128::from(MM) * i128::from(MM);
    assert_eq!(twice_area(&pieces), full - cut);
    assert!(!inside(&pieces, 0.2, 0.2), "the corner is cut");
    assert!(inside(&pieces, 0.6, 0.6), "inside the chamfer line");
    assert!(inside(&pieces, 5.0, 0.1), "the edge keeps its copper");
}

/// A rounded corner of radius 2 mm leaves the corner point and keeps the
/// copper inside the arc; its area lies between the chamfer's and the
/// square's, near the true (4 - π)·r² taken off each corner.
#[test]
fn a_rounded_corner_follows_an_arc_of_its_radius() {
    let mut board = board_with(&SQUARE);
    board.zones[0].corners = PourCorners::Rounded;
    board.zones[0].corner_size = 2.0;
    let pieces = fill(&board);
    assert!(!inside(&pieces, 0.3, 0.3), "the corner is rounded off");
    assert!(inside(&pieces, 2.0 - 1.3, 2.0 - 1.3), "inside the arc");
    let area_mm2 = twice_area(&pieces) as f64 / 2.0e12;
    let ideal = 100.0 - (4.0 - std::f64::consts::PI) * 4.0;
    assert!((area_mm2 - ideal).abs() < 0.05, "{area_mm2} vs {ideal}");
}

/// Sharp, or a size of 0, fills the outline as drawn.
#[test]
fn sharp_corners_or_a_zero_size_fill_the_outline() {
    let mut board = board_with(&SQUARE);
    let sharp = fill(&board);
    board.zones[0].corners = PourCorners::Rounded;
    assert_eq!(fill(&board), sharp, "no size, no rounding");
    board.zones[0].corner_size = 0.5;
    board.zones[0].corners = PourCorners::Sharp;
    assert_eq!(fill(&board), sharp, "Sharp ignores the size");
}

/// Only convex corners are shaped: the concave corner of an L keeps its
/// copper, and nothing reaches past the drawn outline.
#[test]
fn a_concave_corner_is_left_as_drawn() {
    let l = [
        (0.0, 0.0),
        (10.0, 0.0),
        (10.0, 4.0),
        (4.0, 4.0),
        (4.0, 10.0),
        (0.0, 10.0),
    ];
    let mut board = board_with(&l);
    let sharp = twice_area(&fill(&board));
    board.zones[0].corners = PourCorners::Chamfer;
    board.zones[0].corner_size = 1.0;
    let pieces = fill(&board);
    assert!(inside(&pieces, 3.9, 3.9), "the concave corner keeps copper");
    assert!(!inside(&pieces, 4.2, 4.2), "nothing past the outline");
    // Five convex corners each lose 0.5 mm².
    let five = 5 * i128::from(MM) * i128::from(MM);
    assert_eq!(twice_area(&pieces), sharp - five);
}

/// A corner never takes more than half of an edge: a chamfer larger than
/// the square still leaves a diamond, not a crossed ring.
#[test]
fn a_size_too_large_for_the_edges_takes_half_of_each() {
    let mut board = board_with(&SQUARE);
    board.zones[0].corners = PourCorners::Chamfer;
    board.zones[0].corner_size = 50.0;
    let pieces = fill(&board);
    assert_eq!(pieces.len(), 1);
    assert_eq!(
        twice_area(&pieces),
        2 * 50 * i128::from(MM) * i128::from(MM)
    );
    assert!(inside(&pieces, 5.0, 5.0));
}

/// A hatched pour holds its border and a grid of lines with open gaps: less
/// copper than solid, one connected piece, the gaps open, the lines and the
/// border copper.
#[test]
fn a_hatched_pour_holds_a_grid_inside_a_border() {
    let mut board = board_with(&SQUARE);
    let solid = twice_area(&fill(&board));
    board.zones[0].fill_mode = PourFillMode::Hatched;
    board.zones[0].hatch_width = 0.5;
    board.zones[0].hatch_gap = 1.5;
    let pieces = fill(&board);
    assert_eq!(pieces.len(), 1, "the border joins the lines");
    assert!(twice_area(&pieces) < solid);
    assert!(!pieces[0].holes.is_empty(), "the gaps are holes");
    // Lines at x and y in 0..0.5, 2..2.5, 4..4.5, ...; gaps between.
    assert!(inside(&pieces, 2.25, 5.0), "a vertical line");
    assert!(inside(&pieces, 5.0, 4.25), "a horizontal line");
    assert!(!inside(&pieces, 1.25, 1.25), "a gap");
    assert!(inside(&pieces, 9.8, 7.0), "the border");
}

/// The grid is aligned to the board origin, so a pour moved by a whole
/// pitch holds the same hatch, moved.
#[test]
fn the_hatch_is_aligned_to_the_board_origin() {
    let mut board = board_with(&[(0.3, 0.3), (10.3, 0.3), (10.3, 10.3), (0.3, 10.3)]);
    board.zones[0].fill_mode = PourFillMode::Hatched;
    board.zones[0].hatch_width = 0.5;
    board.zones[0].hatch_gap = 1.5;
    let pieces = fill(&board);
    assert!(inside(&pieces, 2.25, 5.0), "the line at x = 2 stays at 2");
    assert!(!inside(&pieces, 1.25, 1.25), "the gap stays");
}

/// 0 width and gap read as the defaults; a hatch finer than the fill draws
/// leaves the pour unfilled with its reason, instead of a slow fill.
#[test]
fn a_hatch_too_fine_to_draw_leaves_the_pour_unfilled() {
    let mut board = board_with(&SQUARE);
    board.zones[0].fill_mode = PourFillMode::Hatched;
    assert!(zone_fills(&board).of(0).is_some(), "the defaults draw");
    board.zones[0].hatch_width = 0.000_001;
    board.zones[0].hatch_gap = 0.000_001;
    let fills = zone_fills(&board);
    assert!(fills.of(0).is_none());
    assert_eq!(fills.unfilled(0), Some(Unfilled::Hatch));
}

/// The ring shaping alone: a clockwise ring is shaped like a
/// counter-clockwise one.
#[test]
fn a_ring_is_shaped_whichever_way_it_winds() {
    let ccw: Vec<PointNm> = SQUARE
        .iter()
        .map(|&(x, y)| PointNm::new((x * 1e6) as i64, (y * 1e6) as i64))
        .collect();
    let mut cw = ccw.clone();
    cw.reverse();
    let area = |ring: Vec<PointNm>| {
        twice_area(&shaped_area(
            &Polygon::new(ring, Vec::new()),
            PourCorners::Chamfer,
            MM,
        ))
    };
    assert_eq!(area(ccw), area(cw));
}
