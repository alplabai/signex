//! The kinds of board dimension (#2507): aligned, linear horizontal, linear
//! vertical and radial, and the circle a radial dimension measures. A
//! radial dimension names the arc or circle it measures and follows it as
//! it moves or changes size; the board engine finds that circle on the
//! board (`signex_engine::pcb::dimension_refit`). See docs/PCB_TOOLS.md,
//! "Dimension kinds".

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::pcb_dimension::LinearDimension;
use crate::schematic::Point;

/// How far, in mm, a radial dimension's centre or radius may sit off its
/// circle and still count as on it: a file stores nm, so a reload moves a
/// circle's centre by a few nm.
pub const RADIAL_FIT_TOLERANCE_MM: f64 = 1e-4;

/// What a board dimension measures.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DimensionKind {
    /// The straight distance between its two points, its line parallel to
    /// them: the board dimension from before the kinds.
    #[default]
    Aligned,
    /// The distance across, its line horizontal.
    Horizontal,
    /// The distance up or down, its line vertical.
    Vertical,
    /// The radius of an arc or circle: `start` is the centre, `end` the
    /// point on the circle the leader runs to.
    Radial,
}

impl DimensionKind {
    pub const ALL: [Self; 4] = [Self::Aligned, Self::Horizontal, Self::Vertical, Self::Radial];

    /// The direction from `start` to `end` the value is measured along, a
    /// unit vector, and the measured length; `None` when the length is
    /// zero or not a number.
    pub fn measure(self, start: Point, end: Point) -> Option<(Point, f64)> {
        let (dx, dy) = (end.x - start.x, end.y - start.y);
        let (along, length) = match self {
            Self::Aligned | Self::Radial => {
                let length = dx.hypot(dy);
                (Point::new(dx / length, dy / length), length)
            }
            Self::Horizontal => (Point::new(sign(dx), 0.0), dx.abs()),
            Self::Vertical => (Point::new(0.0, sign(dy)), dy.abs()),
        };
        (length.is_finite() && length > 0.0).then_some((along, length))
    }
}

/// 1 or -1: the side of zero `v` is on, zero counted positive.
fn sign(v: f64) -> f64 {
    if v < 0.0 { -1.0 } else { 1.0 }
}

/// An arc or circle on the board a radial dimension can measure.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeasuredCircle {
    /// The free arc's or the board drawing's uuid.
    pub uuid: Uuid,
    pub centre: Point,
    pub radius: f64,
}

impl MeasuredCircle {
    /// How far `at` is from the circle's edge, in mm.
    pub fn edge_distance(&self, at: Point) -> f64 {
        ((at.x - self.centre.x).hypot(at.y - self.centre.y) - self.radius).abs()
    }

    /// The point on the circle in the direction of `toward` from its
    /// centre; to the right of the centre when `toward` is the centre.
    pub fn point_toward(&self, toward: Point) -> Point {
        let (dx, dy) = (toward.x - self.centre.x, toward.y - self.centre.y);
        let length = dx.hypot(dy);
        let (ux, uy) = if length.is_finite() && length > 0.0 {
            (dx / length, dy / length)
        } else {
            (1.0, 0.0)
        };
        Point::new(
            self.centre.x + ux * self.radius,
            self.centre.y + uy * self.radius,
        )
    }
}

impl LinearDimension {
    /// A radial dimension measuring `circle`, its leader from the centre
    /// toward `toward`, otherwise as [`LinearDimension::new`] makes one.
    pub fn radial(id: Uuid, circle: &MeasuredCircle, toward: Point) -> Self {
        let mut dimension = Self::new(id, circle.centre, circle.point_toward(toward), 0.0);
        dimension.kind = DimensionKind::Radial;
        dimension.measures = Some(circle.uuid);
        dimension
    }

    /// This radial dimension put on `circle`, its leader keeping its
    /// direction; `None` for another kind or a dimension already on it
    /// (within [`RADIAL_FIT_TOLERANCE_MM`]).
    pub fn refit_to(&self, circle: &MeasuredCircle) -> Option<LinearDimension> {
        if self.kind != DimensionKind::Radial {
            return None;
        }
        let off_centre = (self.start.x - circle.centre.x).hypot(self.start.y - circle.centre.y);
        let radius = (self.end.x - self.start.x).hypot(self.end.y - self.start.y);
        if off_centre <= RADIAL_FIT_TOLERANCE_MM
            && (radius - circle.radius).abs() <= RADIAL_FIT_TOLERANCE_MM
        {
            return None;
        }
        let toward = Point::new(
            circle.centre.x + (self.end.x - self.start.x),
            circle.centre.y + (self.end.y - self.start.y),
        );
        let mut fitted = self.clone();
        fitted.start = circle.centre;
        fitted.end = circle.point_toward(toward);
        Some(fitted)
    }
}

#[cfg(test)]
#[path = "pcb_dimension_kind_tests.rs"]
mod tests;
