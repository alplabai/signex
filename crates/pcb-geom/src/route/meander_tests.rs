//! #2389 — the length-tuning accordion: each bump adds twice its height,
//! the bumps sit in the middle of the track, and they must fit.

use super::test_board::{at, nm};
use super::{accordion, accordion_room};
use crate::units::PointNm;

fn length(corners: &[PointNm]) -> i64 {
    corners
        .windows(2)
        .map(|leg| (leg[0].distance_sq(leg[1]) as f64).sqrt().round() as i64)
        .sum()
}

#[test]
fn each_bump_adds_twice_its_height() {
    let (a, b) = (at(0.0, 0.0), at(20.0, 0.0));
    let corners = accordion(a, b, 2, nm(3.0), nm(1.0), true).expect("two bumps fit");
    assert_eq!(corners.len(), 2 + 4 * 2);
    assert_eq!((corners[0], corners[9]), (a, b));
    assert_eq!(length(&corners), nm(20.0) + 2 * 2 * nm(3.0));
    // Centred: 20 mm less three pitches, halved, before the first rise.
    assert_eq!(corners[1], at(8.5, 0.0));
    assert_eq!(corners[2], at(8.5, 3.0), "left of a track running +x");
    let right = accordion(a, b, 2, nm(3.0), nm(1.0), false).unwrap();
    assert_eq!(right[2], at(8.5, -3.0));
}

#[test]
fn bumps_that_do_not_fit_are_refused() {
    assert_eq!(accordion_room(nm(20.0), nm(1.0)), 9);
    let (a, b) = (at(0.0, 0.0), at(20.0, 0.0));
    assert!(accordion(a, b, 9, nm(1.0), nm(1.0), true).is_some());
    assert_eq!(accordion(a, b, 10, nm(1.0), nm(1.0), true), None);
    assert_eq!(accordion(a, a, 1, nm(1.0), nm(1.0), true), None);
}
