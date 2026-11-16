use signex_types::pcb::SOLID_FILL;
use signex_types::zone_keepout::KEEPOUT_FILL;

use super::*;
use crate::tool_fixture::{MM, clearance, empty_board, track, via, zone};
use crate::zone_fill::zone_area;

const DIAMETER: i64 = 600_000;

fn spec() -> StitchSpec {
    StitchSpec {
        net: 1,
        pitch: 2 * MM,
        diameter: DIAMETER,
    }
}

/// A net-1 region over (-5,-5)..(5,5) mm crossed at y = 0 by a net-2 track.
fn board() -> PcbBoard {
    let mut board = empty_board();
    board
        .zones
        .push(zone(10, 1, SOLID_FILL, (-5.0, -5.0, 5.0, 5.0)));
    board.segments.push(track(20, (-10.0, 0.0), (10.0, 0.0), 2));
    board
}

fn area(board: &PcbBoard) -> Vec<Polygon> {
    vec![zone_area(&board.zones[0]).expect("an area")]
}

#[test]
fn vias_fill_the_area_on_the_grid_clear_of_other_nets() {
    let board = board();
    let points = stitch_points(&board, &area(&board), spec(), clearance);
    // x -4..4 and y -4, -2, 2, 4 in 2 mm steps: the y = 0 row is on the
    // net-2 track.
    assert_eq!(points.len(), 20, "{points:?}");
    let other = track_shape_of(&board);
    for p in &points {
        assert_eq!(p.x.rem_euclid(2 * MM), 0, "on the grid");
        assert!(p.x.abs() <= 4 * MM && p.y.abs() <= 4 * MM && p.y != 0);
        let disc = Shape::Disc {
            center: *p,
            radius: DIAMETER / 2,
        };
        assert!(!disc.separation(&other).is_under(clearance(1, 2)));
    }
    assert_eq!(points[0], PointNm::new(-4 * MM, -4 * MM), "rows top first");
}

fn track_shape_of(board: &PcbBoard) -> Shape {
    crate::index::track_shape(&board.segments[0]).expect("a track")
}

#[test]
fn stitching_again_adds_nothing() {
    let mut board = board();
    let points = stitch_points(&board, &area(&board), spec(), clearance);
    for (i, p) in points.iter().enumerate() {
        let mm = p.to_mm();
        board.vias.push(via(100 + i as u128, (mm.x, mm.y), 0.6, 1));
    }
    assert!(stitch_points(&board, &area(&board), spec(), clearance).is_empty());
}

#[test]
fn a_keepout_and_the_board_edge_keep_vias_out() {
    let mut board = board();
    board
        .zones
        .push(zone(11, 0, KEEPOUT_FILL, (0.0, 0.0, 5.0, 5.0)));
    board.outline = vec![
        signex_types::pcb::Point::new(-5.0, -5.0),
        signex_types::pcb::Point::new(3.0, -5.0),
        signex_types::pcb::Point::new(3.0, 5.0),
        signex_types::pcb::Point::new(-5.0, 5.0),
    ];
    let points = stitch_points(&board, &area(&board), spec(), clearance);
    assert!(!points.is_empty());
    for p in &points {
        assert!(
            !(p.x >= -DIAMETER && p.y >= -DIAMETER),
            "{p:?} in the keepout"
        );
        assert!(p.x <= 2 * MM, "{p:?} past the board edge");
    }
}

#[test]
fn a_pitch_under_the_via_size_or_no_net_places_nothing() {
    let board = board();
    let tight = StitchSpec {
        pitch: DIAMETER - 1,
        ..spec()
    };
    assert!(stitch_points(&board, &area(&board), tight, clearance).is_empty());
    let no_net = StitchSpec { net: 0, ..spec() };
    assert!(stitch_points(&board, &area(&board), no_net, clearance).is_empty());
}
