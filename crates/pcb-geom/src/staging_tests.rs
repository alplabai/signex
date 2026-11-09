use super::*;

fn rect(x0: i64, y0: i64, x1: i64, y1: i64) -> RectNm {
    RectNm::new(PointNm::new(x0, y0), PointNm::new(x1, y1))
}

fn placed(extent: &RectNm, origin: PointNm) -> RectNm {
    RectNm::new(origin.plus(extent.min), origin.plus(extent.max))
}

fn gap_between(a: &RectNm, b: &RectNm) -> i64 {
    let dx = (b.min.x - a.max.x).max(a.min.x - b.max.x);
    let dy = (b.min.y - a.max.y).max(a.min.y - b.max.y);
    dx.max(dy)
}

#[test]
fn staged_footprints_sit_right_of_the_board_on_the_grid_and_never_overlap() {
    let board = rect(0, 0, 50_000_000, 30_000_000);
    let extents: Vec<RectNm> = (0..40)
        .map(|i| {
            let w = 1_000_000 + (i % 7) * 900_000;
            let h = 800_000 + (i % 5) * 1_100_000;
            rect(-w / 2, -h / 3, w / 2 + 123, h)
        })
        .collect();
    let staging = Staging::DEFAULT;
    let origins = stage(Some(board), &extents, staging);
    assert_eq!(origins.len(), extents.len());
    let boxes: Vec<RectNm> = extents
        .iter()
        .zip(&origins)
        .map(|(e, o)| placed(e, *o))
        .collect();
    for (i, (b, o)) in boxes.iter().zip(&origins).enumerate() {
        assert_eq!(o.x % staging.grid, 0, "origin {i} off the grid");
        assert_eq!(o.y % staging.grid, 0, "origin {i} off the grid");
        assert!(
            b.min.x >= board.max.x + staging.gap,
            "footprint {i} on the board"
        );
        for (j, other) in boxes.iter().enumerate().skip(i + 1) {
            assert!(
                gap_between(b, other) >= staging.gap,
                "footprints {i} and {j} closer than the gap"
            );
        }
    }
    assert!(
        boxes.iter().any(|b| b.min.y > boxes[0].max.y),
        "forty footprints wrap into more than one row"
    );
}

#[test]
fn staging_is_deterministic_and_keeps_the_order() {
    let extents = vec![rect(-5, -5, 5, 5); 6];
    let board = Some(rect(0, 0, 100, 100));
    let staging = Staging {
        grid: 10,
        gap: 3,
        row_width: 1_000,
    };
    let a = stage(board, &extents, staging);
    assert_eq!(a, stage(board, &extents, staging));
    assert!(
        a.windows(2).all(|w| w[0].x < w[1].x),
        "one row, left to right"
    );
}

#[test]
fn with_no_outline_staging_starts_at_the_origin() {
    let origins = stage(None, &[rect(0, 0, 10, 10)], Staging::DEFAULT);
    assert_eq!(origins, vec![PointNm::ORIGIN]);
}

#[test]
fn snapping_rounds_up_for_negative_values_too() {
    assert_eq!(snap_up(-15, 10), -10);
    assert_eq!(snap_up(-10, 10), -10);
    assert_eq!(snap_up(1, 10), 10);
    assert_eq!(snap_up(0, 10), 0);
}
