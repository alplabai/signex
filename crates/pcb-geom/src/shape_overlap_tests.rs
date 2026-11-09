use super::*;
use crate::rect::RectNm;

const MM: i64 = 1_000_000;

fn p(x: i64, y: i64) -> PointNm {
    PointNm::new(x * MM, y * MM)
}

fn rect(cx: i64, cy: i64, half: i64, rotation_deg: f64) -> Shape {
    Shape::Rect {
        center: p(cx, cy),
        half_w: half * MM,
        half_h: half * MM,
        rotation_deg,
    }
}

#[test]
fn a_track_across_a_rect_overlaps_without_an_end_inside() {
    let pad = rect(0, 0, 1, 0.0);
    let across = Shape::Capsule {
        a: p(-5, 0),
        b: p(5, 0),
        radius: MM / 8,
    };
    assert!(across.overlaps(&pad) && pad.overlaps(&across));
}

#[test]
fn a_track_beside_a_rect_does_not() {
    let pad = rect(0, 0, 1, 0.0);
    let beside = Shape::Capsule {
        a: p(-5, 3),
        b: p(5, 3),
        radius: MM / 8,
    };
    assert!(!beside.overlaps(&pad));
}

#[test]
fn rects_overlap_off_centre_and_not_when_apart() {
    assert!(rect(0, 0, 1, 0.0).overlaps(&rect(1, 1, 1, 0.0)));
    assert!(!rect(0, 0, 1, 0.0).overlaps(&rect(3, 0, 1, 0.0)));
    // A rotated square's corner reaches 1.41 mm; the other's edge is at 2.
    assert!(rect(0, 0, 1, 45.0).overlaps(&rect(2, 0, 1, 0.0)));
    assert!(!rect(0, 0, 1, 45.0).overlaps(&rect(3, 0, 1, 0.0)));
}

#[test]
fn a_rect_swallowed_by_a_bigger_one_overlaps() {
    assert!(rect(0, 0, 5, 0.0).overlaps(&rect(0, 0, 1, 0.0)));
}

#[test]
fn discs_overlap_by_radius_sum() {
    let d = |x| Shape::Disc {
        center: p(x, 0),
        radius: MM,
    };
    assert!(d(0).overlaps(&d(2)));
    assert!(!d(0).overlaps(&d(3)));
}

#[test]
fn a_stadium_overlaps_a_disc_on_its_long_axis_only() {
    let oval = Shape::Capsule {
        a: p(-2, 0),
        b: p(2, 0),
        radius: MM,
    };
    let at = |x, y| Shape::Disc {
        center: p(x, y),
        radius: MM / 2,
    };
    assert!(oval.overlaps(&at(3, 0)));
    assert!(!oval.overlaps(&at(0, 3)));
}

#[test]
fn polygons_meet_on_crossing_touching_and_containing_but_not_a_1_nm_gap() {
    let sq = |x0: i64, y0: i64, x1: i64, y1: i64| {
        vec![
            PointNm::new(x0, y0),
            PointNm::new(x1, y0),
            PointNm::new(x1, y1),
            PointNm::new(x0, y1),
        ]
    };
    let a = sq(0, 0, 10, 10);
    assert!(polygons_meet(&a, &sq(5, 5, 15, 15))); // edges cross
    assert!(polygons_meet(&a, &sq(10, 0, 20, 10))); // shared edge
    assert!(polygons_meet(&a, &sq(10, 10, 20, 20))); // one corner
    assert!(!polygons_meet(&a, &sq(11, 0, 20, 10)));
    assert!(!polygons_meet(&a, &sq(10 + 1, 10 + 1, 20, 20)));
    assert!(polygons_meet(&a, &sq(2, 2, 4, 4))); // b inside a
    assert!(polygons_meet(&sq(2, 2, 4, 4), &a)); // a inside b
    assert!(polygons_meet(&a, &a[..2])); // a 2-point ring is its segment, on a's edge
    assert!(!polygons_meet(
        &a,
        &[PointNm::new(50, 50), PointNm::new(60, 50)]
    ));
    // A U-shape: a square in its notch touches no copper of it.
    let u = vec![
        PointNm::new(0, 0),
        PointNm::new(30, 0),
        PointNm::new(30, 30),
        PointNm::new(20, 30),
        PointNm::new(20, 10),
        PointNm::new(10, 10),
        PointNm::new(10, 30),
        PointNm::new(0, 30),
    ];
    assert!(!polygons_meet(&u, &sq(12, 12, 18, 18)));
    assert!(polygons_meet(&u, &sq(12, 8, 18, 18)));
}

#[test]

fn a_zero_size_pad_is_a_point_not_the_whole_plane() {
    let pad = Shape::Rect {
        center: PointNm::new(0, 0),
        half_w: 0,
        half_h: 0,
        rotation_deg: 0.0,
    };
    let far_track = Shape::Capsule {
        a: PointNm::new(5_000_000, -5_000_000),
        b: PointNm::new(5_000_000, 5_000_000),
        radius: 100_000,
    };
    assert!(!pad.overlaps(&far_track) && !far_track.overlaps(&pad));
    assert!(!pad.overlaps(&rect(5, 0, 1, 0.0)));
    let through = Shape::Capsule {
        a: p(-1, 0),
        b: p(1, 0),
        radius: 0,
    };
    assert!(pad.overlaps(&through) && through.overlaps(&pad));
    assert!(pad.overlaps(&rect(0, 0, 1, 30.0)));
}

#[test]
fn a_zero_width_pad_is_a_segment_and_stops_at_its_ends() {
    let sliver = Shape::Rect {
        center: p(0, 0),
        half_w: 0,
        half_h: MM,
        rotation_deg: 0.0,
    };
    let beyond_end = Shape::Disc {
        center: p(0, 3),
        radius: MM / 2,
    };
    assert!(!sliver.overlaps(&beyond_end));
    let at_end = Shape::Disc {
        center: p(0, 2),
        radius: MM,
    };
    assert!(sliver.overlaps(&at_end));
    let across = Shape::Capsule {
        a: p(-1, 0),
        b: p(1, 0),
        radius: 0,
    };
    assert!(sliver.overlaps(&across));
}

#[test]
fn a_polygon_with_no_area_is_a_segment_or_point_for_both_polygon_tests() {
    let sq = vec![p(0, 0), p(10, 0), p(10, 10), p(0, 10)];
    let line = vec![p(-5, 5), p(5, 5), p(15, 5)]; // crosses sq
    let far_line = vec![p(20, 5), p(30, 5), p(40, 5)];
    let dot = vec![p(5, 5), p(5, 5), p(5, 5)]; // inside sq
    let far_dot = vec![p(50, 50)];
    assert!(polygons_meet(&sq, &line) && polygons_meet(&line, &sq));
    assert!(!polygons_meet(&sq, &far_line) && !polygons_meet(&far_line, &sq));
    assert!(polygons_meet(&sq, &dot) && polygons_meet(&dot, &sq));
    assert!(!polygons_meet(&sq, &far_dot) && !polygons_meet(&far_dot, &sq));
    assert!(!polygons_meet(&line, &far_line));
    assert!(polygons_meet(&line, &[p(5, 0), p(5, 10)]));
    assert!(!polygons_meet(&sq, &[]));
    // Against a shape: a pad 5 mm from a collinear ring is not met; one on it is.
    let pad = rect(0, 0, 1, 0.0);
    assert!(!pad.overlaps_polygon(&[p(6, 0), p(8, 0), p(10, 0)]));
    assert!(!pad.overlaps_polygon(&[p(6, 0), p(10, 0)]));
    assert!(pad.overlaps_polygon(&[p(-3, 0), p(0, 0), p(3, 0)]));
    assert!(pad.overlaps_polygon(&[p(1, 0)]));
    assert!(!pad.overlaps_polygon(&[]));
}

fn round_rect(rotation_deg: f64) -> Shape {
    Shape::RoundRect {
        center: p(0, 0),
        half_w: 2 * MM,
        half_h: MM,
        radius: MM / 2,
        rotation_deg,
    }
}

#[test]
fn round_rect_hit_overlap_and_window_agree_near_the_corner_arc() {
    use crate::index::{BoardIndex, IndexedItem, ItemRef, RectSelect};
    use uuid::Uuid;

    for rotation in [0.0, 30.0, 45.0] {
        let pad = round_rect(rotation);
        let index = BoardIndex::from_items(vec![
            IndexedItem::new(ItemRef::Via(Uuid::from_u128(1)), pad).unwrap(),
        ]);
        let step = MM / 25;
        let mut seen = (false, false);
        let mut x = -3 * MM + 7;
        while x <= 3 * MM {
            let mut y = -3 * MM + 7;
            while y <= 3 * MM {
                let at = PointNm::new(x, y);
                let hit = pad.contains(at, 0);
                let probe = Shape::Disc {
                    center: at,
                    radius: 0,
                };
                let window = RectNm::new(at, at);
                let ctx = format!("rotation {rotation}, point {x},{y}");
                assert_eq!(pad.overlaps(&probe), hit, "overlap {ctx}");
                assert_eq!(probe.overlaps(&pad), hit, "overlap rev {ctx}");
                assert_eq!(pad.touches_rect(&window), hit, "touches {ctx}");
                assert_eq!(
                    !index.hit_rect(&window, RectSelect::Touching).is_empty(),
                    hit,
                    "window {ctx}"
                );
                seen = (seen.0 | hit, seen.1 | !hit);
                y += step;
            }
            x += step;
        }
        assert_eq!(seen, (true, true));
        // The window that holds the whole pad selects it; one that cuts a
        // corner arc does not.
        let bounds = pad.aabb();
        assert!(!index.hit_rect(&bounds, RectSelect::Inside).is_empty());
        let cut = RectNm::new(bounds.min, PointNm::new(bounds.max.x - 1_000, bounds.max.y));
        assert!(index.hit_rect(&cut, RectSelect::Inside).is_empty());
    }
}

#[test]
fn a_probe_just_inside_the_arc_between_chord_vertices_overlaps() {
    // Inner corner (1.5, 0.5) mm, radius 0.5 mm; 5.625 degrees is midway
    // between two 11.25 degree outline points, where a chord sags ~1.2 um.
    let t = 5.625_f64.to_radians();
    let reach = (MM / 2 - 1_000) as f64;
    let at = PointNm::new(
        3 * MM / 2 + (reach * t.cos()).round() as i64,
        MM / 2 + (reach * t.sin()).round() as i64,
    );
    let pad = round_rect(0.0);
    let probe = Shape::Disc {
        center: at,
        radius: 0,
    };
    assert!(pad.contains(at, 0) && pad.overlaps(&probe));
}

#[test]
fn a_ratio_half_square_round_rect_turned_45_is_its_disc_to_the_nm() {
    let pad = Shape::RoundRect {
        center: p(0, 0),
        half_w: MM / 2,
        half_h: MM / 2,
        radius: MM / 2,
        rotation_deg: 45.0,
    };
    assert_eq!(pad.aabb(), RectNm::around(p(0, 0), MM / 2, MM / 2));
}
