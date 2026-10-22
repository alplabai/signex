//! Pad outline geometry: a pad shape placed at a pose and turned with it.
//!
//! Derived at read time from the footprint's persisted rotation, so no
//! file format carries a per-pad angle.

use crate::pcb::PadShape;
use crate::rotation2d::{Pose2d, Rotatable2d, Vec2d};

/// Smallest half-extent, so a degenerate pad still draws.
const MIN_HALF_EXTENT_MM: f64 = 0.01;
/// Fewest outline vertices for a round shape.
const MIN_ELLIPSE_SEGMENTS: usize = 8;

/// A pad's footprint on the board: shape and size in the pad's own frame,
/// placed by `pose` (origin = pad centre, angle = footprint rotation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PadOutline {
    pub shape: PadShape,
    pub size_mm: Vec2d,
    pub pose: Pose2d,
}

impl PadOutline {
    pub const fn new(shape: PadShape, size_mm: Vec2d, pose: Pose2d) -> Self {
        Self {
            shape,
            size_mm,
            pose,
        }
    }

    /// Outline vertices in world space, counter-clockwise from the pad's +x axis.
    pub fn vertices(&self, ellipse_segments: usize) -> Vec<Vec2d> {
        let half_w = (self.size_mm.x * 0.5).max(MIN_HALF_EXTENT_MM);
        let half_h = (self.size_mm.y * 0.5).max(MIN_HALF_EXTENT_MM);

        let local: Vec<Vec2d> = match self.shape {
            PadShape::Circle => {
                let r = half_w.min(half_h);
                ellipse(r, r, ellipse_segments)
            }
            PadShape::Oval => ellipse(half_w, half_h, ellipse_segments),
            // Library-only shapes have no parameters on a board pad yet.
            PadShape::Rect | PadShape::RoundRect | PadShape::Trapezoid | PadShape::Custom => {
                vec![
                    Vec2d::new(-half_w, -half_h),
                    Vec2d::new(half_w, -half_h),
                    Vec2d::new(half_w, half_h),
                    Vec2d::new(-half_w, half_h),
                ]
            }
        };

        let (sin, cos) = self.pose.rotation_rad.sin_cos();
        local
            .into_iter()
            .map(|p| {
                Vec2d::new(
                    self.pose.origin.x + p.x * cos - p.y * sin,
                    self.pose.origin.y + p.x * sin + p.y * cos,
                )
            })
            .collect()
    }
}

fn ellipse(radius_x: f64, radius_y: f64, segments: usize) -> Vec<Vec2d> {
    let count = segments.max(MIN_ELLIPSE_SEGMENTS);
    (0..count)
        .map(|i| {
            let t = (i as f64 / count as f64) * std::f64::consts::TAU;
            Vec2d::new(radius_x * t.cos(), radius_y * t.sin())
        })
        .collect()
}

impl Rotatable2d for PadOutline {
    fn pose(&self) -> Pose2d {
        self.pose
    }

    // The outline is centred on its own origin.
    fn geometry_center_local(&self) -> Vec2d {
        Vec2d::ZERO
    }

    fn set_pose(&mut self, pose: Pose2d) {
        self.pose = pose;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rotation2d::{RotationPivot, RotationSpace, rotate_object};
    use std::f64::consts::FRAC_PI_2;

    fn rect() -> PadOutline {
        PadOutline::new(
            PadShape::Rect,
            Vec2d::new(2.0, 1.0),
            Pose2d::new(Vec2d::new(5.0, 3.0), 0.0),
        )
    }

    #[test]
    fn quarter_turn_swaps_the_extent_about_the_centre() {
        let mut outline = rect();
        rotate_object(
            &mut outline,
            RotationSpace::Local,
            RotationPivot::GeometryCenter,
            FRAC_PI_2,
        );
        let v = outline.vertices(18);
        let width = v.iter().map(|p| p.x).fold(f64::MIN, f64::max)
            - v.iter().map(|p| p.x).fold(f64::MAX, f64::min);
        let height = v.iter().map(|p| p.y).fold(f64::MIN, f64::max)
            - v.iter().map(|p| p.y).fold(f64::MAX, f64::min);
        assert!((width - 1.0).abs() < 1e-9 && (height - 2.0).abs() < 1e-9);
    }

    #[test]
    fn unrotated_outline_is_axis_aligned_on_the_pose_origin() {
        let v = rect().vertices(18);
        assert_eq!(v[0], Vec2d::new(4.0, 2.5));
        assert_eq!(v[2], Vec2d::new(6.0, 3.5));
    }
}
