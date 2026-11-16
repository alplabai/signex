//! #2389 — side-by-side tracks: a pair or a bus follows its centre line at
//! fixed offsets, mitred through corners, and refuses a corner it cannot
//! follow.

use super::test_board::{at, nm};
use super::{bus_offsets, offset_path, parallel_paths};
use crate::units::PointNm;

/// The distance from `p` to the infinite line through `a` and `b`.
fn line_distance(p: PointNm, a: PointNm, b: PointNm) -> f64 {
    let ab = b.minus(a);
    let ap = p.minus(a);
    (ab.cross(ap) as f64).abs() / (ab.dot(ab) as f64).sqrt()
}

#[test]
fn a_straight_path_moves_to_its_left_and_right() {
    let path = [at(0.0, 0.0), at(10.0, 0.0)];
    assert_eq!(
        offset_path(&path, nm(0.5)),
        Some(vec![at(0.0, 0.5), at(10.0, 0.5)]),
        "left of a path running +x is +y"
    );
    assert_eq!(
        offset_path(&path, -nm(0.5)),
        Some(vec![at(0.0, -0.5), at(10.0, -0.5)])
    );
}

#[test]
fn a_right_angle_is_mitred_so_both_legs_keep_the_offset() {
    let path = [at(0.0, 0.0), at(10.0, 0.0), at(10.0, 10.0)];
    let left = offset_path(&path, nm(1.0)).expect("a right angle mitres");
    // The inside corner of a left turn sits one offset in on both legs.
    assert_eq!(left[1], at(9.0, 1.0));
    for (i, leg) in path.windows(2).enumerate() {
        let d0 = line_distance(left[i], leg[0], leg[1]);
        let d1 = line_distance(left[i + 1], leg[0], leg[1]);
        assert!((d0 - nm(1.0) as f64).abs() < 2.0, "leg {i} start: {d0}");
        assert!((d1 - nm(1.0) as f64).abs() < 2.0, "leg {i} end: {d1}");
    }
    let right = offset_path(&path, -nm(1.0)).unwrap();
    assert_eq!(right[1], at(11.0, -1.0), "the outside corner");
}

#[test]
fn a_45_degree_bend_keeps_the_offset_on_both_legs() {
    let path = [at(0.0, 0.0), at(10.0, 0.0), at(15.0, 5.0)];
    let left = offset_path(&path, nm(0.4)).expect("mitres");
    for (i, leg) in path.windows(2).enumerate() {
        let d = line_distance(left[i + 1], leg[0], leg[1]);
        assert!((d - nm(0.4) as f64).abs() < 2.0, "leg {i}: {d}");
    }
}

#[test]
fn a_reversal_and_a_point_are_refused() {
    let back = [at(0.0, 0.0), at(10.0, 0.0), at(0.0, 0.1)];
    assert_eq!(offset_path(&back, nm(1.0)), None, "too sharp to mitre");
    assert_eq!(offset_path(&[at(1.0, 1.0), at(1.0, 1.0)], nm(1.0)), None);
    let repeated = [at(0.0, 0.0), at(0.0, 0.0), at(5.0, 0.0)];
    assert_eq!(offset_path(&repeated, nm(1.0)).map(|p| p.len()), Some(2));
}

#[test]
fn bus_offsets_are_centred_and_a_pitch_apart() {
    assert_eq!(bus_offsets(2, nm(0.5)), vec![nm(0.25), -nm(0.25)]);
    assert_eq!(bus_offsets(3, nm(1.0)), vec![nm(1.0), 0, -nm(1.0)]);
    assert_eq!(bus_offsets(1, nm(1.0)), vec![0]);
    let path = [at(0.0, 0.0), at(10.0, 0.0)];
    let members = parallel_paths(&path, &bus_offsets(3, nm(1.0))).unwrap();
    let ys: Vec<i64> = members.iter().map(|m| m[0].y).collect();
    assert_eq!(ys, vec![nm(1.0), 0, -nm(1.0)], "leftmost first");
}
