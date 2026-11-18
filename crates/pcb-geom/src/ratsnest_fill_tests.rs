//! #2216: the ratsnest joins copper through a pour's fill, not its outline.
//! The board is `two_pads_with`'s: net-1 pads at (0,0) and (20,0).

use super::*;
use signex_types::zone_pour::POUR_FILL;

/// A net-1 pour over both pads, clearance `clearance` mm.
fn pour(clearance: f64) -> signex_types::pcb::Zone {
    let mut z = zone(1, "TopCopper", (-5.0, -5.0, 25.0, 5.0));
    z.fill_type = POUR_FILL.into();
    z.clearance = clearance;
    z
}

fn square_hole(cx: f64, cy: f64, half: f64) -> Vec<Point> {
    vec![
        Point::new(cx - half, cy - half),
        Point::new(cx + half, cy - half),
        Point::new(cx + half, cy + half),
        Point::new(cx - half, cy + half),
    ]
}

#[test]
fn pads_on_the_fill_get_no_ratsnest_line() {
    assert!(ratsnest(&two_pads_with(Vec::new(), Vec::new(), vec![pour(0.2)])).is_empty());
}

#[test]
fn a_pad_in_a_pour_hole_gets_a_ratsnest_line() {
    let mut z = pour(0.2);
    z.holes = vec![square_hole(20.0, 0.0, 2.0)];
    assert_eq!(
        ratsnest(&two_pads_with(Vec::new(), Vec::new(), vec![z])),
        apart()
    );
}

#[test]
fn a_pad_in_a_solid_zones_hole_gets_a_ratsnest_line_too() {
    let mut z = zone(1, "TopCopper", (-5.0, -5.0, 25.0, 5.0));
    z.holes = vec![square_hole(20.0, 0.0, 2.0)];
    assert_eq!(
        ratsnest(&two_pads_with(Vec::new(), Vec::new(), vec![z])),
        apart()
    );
}

#[test]
fn a_pad_inside_the_clearance_band_gets_a_ratsnest_line() {
    // A net-2 track 0.575 mm above pad 2's edge: with a 2 mm clearance the
    // fill keeps clear of the whole pad, with 0.2 mm it reaches it.
    let other = track(9, (18.0, 1.2), (22.0, 1.2), 2, "TopCopper");
    let wide = two_pads_with(vec![other.clone()], Vec::new(), vec![pour(2.0)]);
    assert_eq!(ratsnest(&wide), apart());
    let narrow = two_pads_with(vec![other], Vec::new(), vec![pour(0.2)]);
    assert!(ratsnest(&narrow).is_empty());
}

#[test]
fn pads_on_two_pieces_of_one_pour_get_a_ratsnest_line() {
    let wall = track(9, (10.0, -6.0), (10.0, 6.0), 2, "TopCopper");
    let lines = ratsnest(&two_pads_with(vec![wall], Vec::new(), vec![pour(0.2)]));
    assert_eq!(lines, apart());
}

#[test]
fn the_fills_passed_in_are_the_ones_used() {
    let board = two_pads_with(Vec::new(), Vec::new(), vec![pour(0.2)]);
    assert!(ratsnest_with_fills(&board, &zone_fills(&board)).is_empty());
    assert_eq!(ratsnest_with_fills(&board, &ZoneFills::default()), apart());
}
