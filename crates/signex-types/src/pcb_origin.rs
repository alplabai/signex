//! Where a board's coordinates count from (#2384), and the board area of a
//! board that has no outline yet. See `docs/PCB_BOARD_OPTIONS.md`, "Origin".

use serde::{Deserialize, Serialize};

use crate::pcb::Point;

/// The board area of a board with no outline yet, board mm, as (min, max):
/// the canvas draws it, Update PCB stages new parts inside it, and a new
/// board's origin sits at its bottom-left corner.
pub const EMPTY_BOARD_AREA: (Point, Point) =
    (Point { x: 0.0, y: 0.0 }, Point { x: 100.0, y: 80.0 });

/// The bottom-left corner of the board area as the screen shows it: the
/// lowest x and the greatest y (model y grows down the screen) of `outline`,
/// or of [`EMPTY_BOARD_AREA`] when the outline has fewer than three points.
pub fn board_area_bottom_left(outline: &[Point]) -> Point {
    if outline.len() < 3 {
        let (min, max) = EMPTY_BOARD_AREA;
        return Point::new(min.x, max.y);
    }
    outline
        .iter()
        .fold(Point::new(f64::INFINITY, f64::NEG_INFINITY), |corner, p| {
            Point::new(corner.x.min(p.x), corner.y.max(p.y))
        })
}

/// The corners of the board area: the outline's points when it has three
/// or more, else the four corners of [`EMPTY_BOARD_AREA`].
pub fn board_area_corners(outline: &[Point]) -> Vec<Point> {
    if outline.len() >= 3 {
        return outline.to_vec();
    }
    let (min, max) = EMPTY_BOARD_AREA;
    vec![min, Point::new(max.x, min.y), max, Point::new(min.x, max.y)]
}

/// The corner of the board area nearest `at` (Edit ▸ Origin ▸ Snap to
/// Outline Corner, 2026-11-18); the first one on a tie.
pub fn nearest_area_corner(outline: &[Point], at: Point) -> Point {
    let distance = |p: &Point| (p.x - at.x).powi(2) + (p.y - at.y).powi(2);
    board_area_corners(outline)
        .into_iter()
        .fold(None, |best: Option<Point>, p| match best {
            Some(b) if distance(&b) <= distance(&p) => Some(b),
            _ => Some(p),
        })
        .unwrap_or(EMPTY_BOARD_AREA.0)
}

/// The point a board shows as (0, 0), in board mm. Shown coordinates run
/// with X to the right and Y up the screen, the usual board convention;
/// the model's y grows down the screen, so Y is mirrored about the origin.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoardOrigin {
    pub at: Point,
}

/// A board saved without an origin counts from the model's zero.
impl Default for BoardOrigin {
    fn default() -> Self {
        Self::new(Point::ZERO)
    }
}

impl BoardOrigin {
    pub fn new(at: Point) -> Self {
        Self { at }
    }

    /// The origin of a new board: the bottom-left corner of its area.
    pub fn of_new_board(outline: &[Point]) -> Self {
        Self::new(board_area_bottom_left(outline))
    }

    /// A model point as the user reads it.
    pub fn to_user(self, p: Point) -> Point {
        Point::new(self.user_x(p.x), self.user_y(p.y))
    }

    /// A point the user typed, in the model.
    pub fn from_user(self, p: Point) -> Point {
        Point::new(self.board_x(p.x), self.board_y(p.y))
    }

    pub fn user_x(self, x: f64) -> f64 {
        x - self.at.x
    }

    pub fn user_y(self, y: f64) -> f64 {
        self.at.y - y
    }

    pub fn board_x(self, x: f64) -> f64 {
        self.at.x + x
    }

    pub fn board_y(self, y: f64) -> f64 {
        self.at.y - y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-11-18: Snap to Outline Corner takes the outline corner nearest
    /// the pointer, or the board area's without an outline.
    #[test]
    fn the_nearest_corner_is_of_the_outline_else_of_the_area() {
        let outline = [
            Point::new(10.0, 20.0),
            Point::new(60.0, 20.0),
            Point::new(60.0, 50.0),
            Point::new(10.0, 50.0),
        ];
        let near = nearest_area_corner(&outline, Point::new(55.0, 45.0));
        assert_eq!(near, Point::new(60.0, 50.0));
        let empty = nearest_area_corner(&[], Point::new(90.0, 5.0));
        assert_eq!(empty, Point::new(100.0, 0.0));
    }

    #[test]
    fn a_new_board_without_an_outline_counts_from_the_area_bottom_left() {
        let origin = BoardOrigin::of_new_board(&[]);
        assert_eq!(origin.at, Point::new(0.0, 80.0));
    }

    #[test]
    fn the_bottom_left_of_an_outline_is_its_lowest_x_and_greatest_y() {
        let outline = [
            Point::new(10.0, 20.0),
            Point::new(60.0, 20.0),
            Point::new(60.0, 50.0),
            Point::new(10.0, 50.0),
        ];
        assert_eq!(board_area_bottom_left(&outline), Point::new(10.0, 50.0));
    }

    #[test]
    fn shown_y_runs_up_from_the_origin_and_converts_back() {
        let origin = BoardOrigin::new(Point::new(0.0, 80.0));
        // 5 mm right of and 5 mm above the corner.
        let model = origin.from_user(Point::new(5.0, 5.0));
        assert_eq!(model, Point::new(5.0, 75.0));
        assert_eq!(origin.to_user(model), Point::new(5.0, 5.0));
        // The default origin mirrors y about the model's zero.
        assert_eq!(
            BoardOrigin::default().to_user(Point::new(3.0, 4.0)),
            Point::new(3.0, -4.0)
        );
    }
}
