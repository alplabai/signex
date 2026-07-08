//! Anchor-aware 2D transform for Signex geometry.
//!
//! This module provides [`Transform2D`], a transform that stores the pivot/anchor
//! point explicitly in world space — the "B-type compensated" model.
//!
//! # Coordinate model
//!
//! ```text
//! origin_world = pivot_world + rotate(local_offset, rotation_rad)
//! ```
//!
//! - `pivot_world`: anchor point in world space. **Stays fixed** during rotation.
//! - `local_offset`: offset from pivot to object origin in LOCAL (pre-rotation) space.
//!   Convention: `local_offset = -(anchor_frac * size)` when pivot is at an anchor.
//!
//! # Why B-type (compensated)?
//!
//! In the A-type model the anchor world position is re-derived each frame from
//! `(position, anchor_frac, size)`. When `anchor_frac` changes on a rotated object
//! the derived pivot jumps to a different world location, causing a visible snap.
//!
//! In the B-type model `pivot_world` is stored directly. Rotation always turns
//! the object around that stable world coordinate.  Changing the anchor fraction
//! (which point of the object the pivot sits on) is done via
//! [`Transform2D::set_pivot_to_anchor`], which moves `pivot_world` to the new
//! anchor position without moving the object.

use crate::rotation2d::{Vec2d, normalize_angle_rad};

/// 2D transform with an explicit pivot/anchor point stored in world space.
///
/// See the [module docs](self) for the full coordinate model.
#[derive(Debug, Clone, PartialEq)]
pub struct Transform2D {
    /// Pivot/anchor point in world space. Stays fixed during [`Self::rotate`].
    pub pivot_world: Vec2d,
    /// Offset from pivot to object origin in LOCAL (pre-rotation) space.
    pub local_offset: Vec2d,
    /// Object bounding-box size.
    pub size: Vec2d,
    /// Rotation angle in radians, counter-clockwise.
    pub rotation_rad: f64,
}

impl Transform2D {
    /// Construct from raw components.
    pub const fn new(
        pivot_world: Vec2d,
        local_offset: Vec2d,
        size: Vec2d,
        rotation_rad: f64,
    ) -> Self {
        Self {
            pivot_world,
            local_offset,
            size,
            rotation_rad,
        }
    }

    /// Construct from the object's world-space origin and a fractional anchor.
    ///
    /// `anchor_frac` is in `[0, 1]²`:
    /// - `(0, 0)` = bottom-left corner
    /// - `(0.5, 0.5)` = center
    /// - `(1, 1)` = top-right corner  (Y-up convention)
    ///
    /// The pivot is placed at the anchor point.
    pub fn from_origin_anchor(
        origin_world: Vec2d,
        anchor_frac: Vec2d,
        size: Vec2d,
        rotation_rad: f64,
    ) -> Self {
        let local_offset = Vec2d::new(-anchor_frac.x * size.x, -anchor_frac.y * size.y);
        // pivot_world = origin_world - rotate(local_offset, rotation_rad)
        //             = origin_world + rotate(anchor_frac * size, rotation_rad)
        let rotated = rotate_vec(local_offset, rotation_rad);
        let pivot_world = Vec2d::new(origin_world.x - rotated.x, origin_world.y - rotated.y);
        Self {
            pivot_world,
            local_offset,
            size,
            rotation_rad,
        }
    }

    /// World-space position of the object origin.
    ///
    /// ```text
    /// origin_world = pivot_world + rotate(local_offset, rotation_rad)
    /// ```
    #[must_use]
    pub fn origin_world(&self) -> Vec2d {
        let rotated = rotate_vec(self.local_offset, self.rotation_rad);
        Vec2d::new(
            self.pivot_world.x + rotated.x,
            self.pivot_world.y + rotated.y,
        )
    }

    /// World-space position of any fractional anchor point within the object.
    ///
    /// `anchor_frac = (0, 0)` returns the object origin; `(1, 1)` returns the
    /// opposite corner (top-right, Y-up).
    #[must_use]
    pub fn anchor_world_at(&self, anchor_frac: Vec2d) -> Vec2d {
        // Local vector from pivot to anchor point:
        //   local = local_offset + anchor_frac * size
        let local = Vec2d::new(
            self.local_offset.x + anchor_frac.x * self.size.x,
            self.local_offset.y + anchor_frac.y * self.size.y,
        );
        let rotated = rotate_vec(local, self.rotation_rad);
        Vec2d::new(
            self.pivot_world.x + rotated.x,
            self.pivot_world.y + rotated.y,
        )
    }

    /// 3×3 row-major homogeneous transform matrix.
    ///
    /// Maps a local-space point `p` (relative to the object origin) to world space:
    ///
    /// ```text
    /// [wx, wy, 1] = [lx, ly, 1] * M^T
    ///              (or equivalently: world = M * [lx, ly, 1]^T)
    /// ```
    ///
    /// Matrix: `T(pivot_world) · R(rotation_rad) · T(local_offset)`
    ///
    /// ```text
    /// [ c  -s  (c·lo_x − s·lo_y + pw_x) ]
    /// [ s   c  (s·lo_x + c·lo_y + pw_y) ]
    /// [ 0   0   1                        ]
    /// ```
    #[must_use]
    pub fn to_matrix(&self) -> [[f64; 3]; 3] {
        let c = self.rotation_rad.cos();
        let s = self.rotation_rad.sin();
        let lx = self.local_offset.x;
        let ly = self.local_offset.y;
        let px = self.pivot_world.x;
        let py = self.pivot_world.y;
        [
            [c, -s, c * lx - s * ly + px],
            [s, c, s * lx + c * ly + py],
            [0.0, 0.0, 1.0],
        ]
    }

    /// Transform a local-space point to world space using this transform.
    #[must_use]
    pub fn transform_point(&self, local: Vec2d) -> Vec2d {
        let m = self.to_matrix();
        Vec2d::new(
            m[0][0] * local.x + m[0][1] * local.y + m[0][2],
            m[1][0] * local.x + m[1][1] * local.y + m[1][2],
        )
    }

    /// Rotate the object by `delta_rad` counter-clockwise around `pivot_world`.
    ///
    /// `pivot_world` is **unchanged**. The object origin orbits around it.
    pub fn rotate(&mut self, delta_rad: f64) {
        self.rotation_rad = normalize_angle_rad(self.rotation_rad + delta_rad);
    }

    /// Translate the entire transform: both `pivot_world` and the derived
    /// `origin_world` shift by `delta`.
    pub fn translate(&mut self, delta: Vec2d) {
        self.pivot_world.x += delta.x;
        self.pivot_world.y += delta.y;
    }

    /// Move `pivot_world` to a new anchor fraction inside the object without
    /// visually moving the object (B-type compensated behavior).
    ///
    /// After this call:
    /// - `pivot_world` is at the new `anchor_frac` position on the (unchanged) object.
    /// - `origin_world()` returns the same value as before.
    /// - `local_offset = -(anchor_frac * size)`.
    pub fn set_pivot_to_anchor(&mut self, anchor_frac: Vec2d) {
        let old_origin = self.origin_world();
        self.local_offset = Vec2d::new(-anchor_frac.x * self.size.x, -anchor_frac.y * self.size.y);
        // Recompute pivot_world so that origin_world stays at old_origin:
        //   origin_world = pivot_world + rotate(local_offset, rotation_rad)
        //   => pivot_world = origin_world - rotate(local_offset, rotation_rad)
        let rotated = rotate_vec(self.local_offset, self.rotation_rad);
        self.pivot_world = Vec2d::new(old_origin.x - rotated.x, old_origin.y - rotated.y);
    }
}

// ── Helper ────────────────────────────────────────────────────────────────────

/// Rotate a 2D vector by `angle_rad` counter-clockwise.
#[must_use]
pub fn rotate_vec(v: Vec2d, angle_rad: f64) -> Vec2d {
    if angle_rad.abs() <= f64::EPSILON {
        return v;
    }
    let c = angle_rad.cos();
    let s = angle_rad.sin();
    Vec2d::new(c * v.x - s * v.y, s * v.x + c * v.y)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};

    const EPSILON: f64 = 1e-10;

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < EPSILON
    }

    fn vec_eq(a: Vec2d, b: Vec2d) -> bool {
        approx(a.x, b.x) && approx(a.y, b.y)
    }

    // ── from_origin_anchor ────────────────────────────────────────────────────

    #[test]
    fn from_origin_anchor_places_pivot_at_anchor_no_rotation() {
        // Size 120×80, center anchor, no rotation, origin at (100, 200).
        let t = Transform2D::from_origin_anchor(
            Vec2d::new(100.0, 200.0),
            Vec2d::new(0.5, 0.5),
            Vec2d::new(120.0, 80.0),
            0.0,
        );
        // Pivot should be at center of the bounding box.
        assert!(
            approx(t.pivot_world.x, 160.0),
            "pivot x: {}",
            t.pivot_world.x
        );
        assert!(
            approx(t.pivot_world.y, 240.0),
            "pivot y: {}",
            t.pivot_world.y
        );
        // origin_world round-trips back.
        assert!(vec_eq(t.origin_world(), Vec2d::new(100.0, 200.0)));
    }

    #[test]
    fn from_origin_anchor_corner_anchor_no_rotation() {
        // Bottom-left anchor: pivot_world = origin_world.
        let t = Transform2D::from_origin_anchor(
            Vec2d::new(50.0, 30.0),
            Vec2d::new(0.0, 0.0),
            Vec2d::new(100.0, 60.0),
            0.0,
        );
        assert!(vec_eq(t.pivot_world, Vec2d::new(50.0, 30.0)));
        assert!(vec_eq(t.origin_world(), Vec2d::new(50.0, 30.0)));
    }

    #[test]
    fn from_origin_anchor_center_with_rotation() {
        // Center anchor, 90° CCW rotation.  origin_world must still round-trip.
        let origin = Vec2d::new(0.0, 0.0);
        let t = Transform2D::from_origin_anchor(
            origin,
            Vec2d::new(0.5, 0.5),
            Vec2d::new(4.0, 2.0),
            FRAC_PI_2,
        );
        assert!(vec_eq(t.origin_world(), origin));
    }

    // ── rotate ────────────────────────────────────────────────────────────────

    #[test]
    fn rotate_keeps_pivot_world_fixed() {
        let mut t = Transform2D::from_origin_anchor(
            Vec2d::new(100.0, 200.0),
            Vec2d::new(0.5, 0.5),
            Vec2d::new(120.0, 80.0),
            0.0,
        );
        let pivot_before = t.pivot_world;
        t.rotate(PI / 4.0);
        assert!(vec_eq(t.pivot_world, pivot_before));
    }

    #[test]
    fn rotate_90_moves_origin_around_pivot() {
        // Object at (-2, -1) with center anchor at (0, 0).
        // local_offset = (-2, -1).  After 90° CCW: rotate((-2,-1), 90°) = (1, -2).
        let mut t = Transform2D::from_origin_anchor(
            Vec2d::new(-2.0, -1.0),
            Vec2d::new(0.5, 0.5),
            Vec2d::new(4.0, 2.0),
            0.0,
        );
        assert!(vec_eq(t.pivot_world, Vec2d::ZERO));
        t.rotate(FRAC_PI_2);
        let origin = t.origin_world();
        // rotate((-2, -1), 90°) = (1, -2)
        assert!(approx(origin.x, 1.0), "x: {}", origin.x);
        assert!(approx(origin.y, -2.0), "y: {}", origin.y);
    }

    #[test]
    fn rotate_full_circle_returns_to_original() {
        let mut t = Transform2D::from_origin_anchor(
            Vec2d::new(37.0, -14.0),
            Vec2d::new(0.3, 0.7),
            Vec2d::new(80.0, 60.0),
            0.0,
        );
        let origin_before = t.origin_world();
        t.rotate(PI);
        t.rotate(PI); // total 2π
        assert!(vec_eq(t.origin_world(), origin_before));
    }

    // ── set_pivot_to_anchor ────────────────────────────────────────────────────

    #[test]
    fn set_pivot_to_anchor_keeps_origin_fixed_no_rotation() {
        let mut t = Transform2D::from_origin_anchor(
            Vec2d::new(100.0, 200.0),
            Vec2d::new(0.5, 0.5),
            Vec2d::new(120.0, 80.0),
            0.0,
        );
        let origin_before = t.origin_world();
        t.set_pivot_to_anchor(Vec2d::new(0.0, 0.0));
        assert!(
            vec_eq(t.origin_world(), origin_before),
            "origin shifted: {:?}",
            t.origin_world()
        );
        // New pivot should be at the object origin (bottom-left corner).
        assert!(vec_eq(t.pivot_world, origin_before));
    }

    #[test]
    fn set_pivot_to_anchor_keeps_origin_fixed_with_rotation() {
        let mut t = Transform2D::from_origin_anchor(
            Vec2d::new(0.0, 0.0),
            Vec2d::new(0.5, 0.5),
            Vec2d::new(4.0, 2.0),
            FRAC_PI_2,
        );
        let origin_before = t.origin_world();
        t.set_pivot_to_anchor(Vec2d::new(0.0, 0.0));
        assert!(vec_eq(t.origin_world(), origin_before));
    }

    #[test]
    fn set_pivot_to_anchor_updates_local_offset() {
        let mut t = Transform2D::from_origin_anchor(
            Vec2d::new(0.0, 0.0),
            Vec2d::new(0.5, 0.5),
            Vec2d::new(4.0, 2.0),
            0.0,
        );
        t.set_pivot_to_anchor(Vec2d::new(1.0, 1.0));
        // local_offset should be -(1*4, 1*2) = (-4, -2)
        assert!(approx(t.local_offset.x, -4.0));
        assert!(approx(t.local_offset.y, -2.0));
    }

    // ── anchor_world_at ───────────────────────────────────────────────────────

    #[test]
    fn anchor_world_at_corners_no_rotation() {
        // Origin at (0, 0), size (4, 2), bottom-left anchor, no rotation.
        let t = Transform2D::from_origin_anchor(
            Vec2d::new(0.0, 0.0),
            Vec2d::new(0.0, 0.0),
            Vec2d::new(4.0, 2.0),
            0.0,
        );
        assert!(vec_eq(
            t.anchor_world_at(Vec2d::new(0.0, 0.0)),
            Vec2d::new(0.0, 0.0)
        ));
        assert!(vec_eq(
            t.anchor_world_at(Vec2d::new(1.0, 1.0)),
            Vec2d::new(4.0, 2.0)
        ));
        assert!(vec_eq(
            t.anchor_world_at(Vec2d::new(0.5, 0.5)),
            Vec2d::new(2.0, 1.0)
        ));
    }

    #[test]
    fn anchor_world_at_center_equals_pivot_when_center_anchor() {
        let t = Transform2D::from_origin_anchor(
            Vec2d::new(100.0, 200.0),
            Vec2d::new(0.5, 0.5),
            Vec2d::new(120.0, 80.0),
            FRAC_PI_4,
        );
        let center = t.anchor_world_at(Vec2d::new(0.5, 0.5));
        assert!(vec_eq(center, t.pivot_world));
    }

    // ── to_matrix / transform_point ───────────────────────────────────────────

    #[test]
    fn transform_point_origin_matches_origin_world() {
        let t = Transform2D::from_origin_anchor(
            Vec2d::new(100.0, 200.0),
            Vec2d::new(0.5, 0.5),
            Vec2d::new(120.0, 80.0),
            PI / 6.0,
        );
        let via_matrix = t.transform_point(Vec2d::ZERO);
        assert!(vec_eq(via_matrix, t.origin_world()));
    }

    #[test]
    fn transform_point_size_corner_matches_anchor_world_at_one() {
        let t = Transform2D::from_origin_anchor(
            Vec2d::new(0.0, 0.0),
            Vec2d::new(0.0, 0.0),
            Vec2d::new(4.0, 2.0),
            FRAC_PI_4,
        );
        // Top-right corner in local space = (size.x, size.y)
        let via_matrix = t.transform_point(Vec2d::new(t.size.x, t.size.y));
        let via_anchor = t.anchor_world_at(Vec2d::new(1.0, 1.0));
        assert!(vec_eq(via_matrix, via_anchor));
    }

    // ── translate ─────────────────────────────────────────────────────────────

    #[test]
    fn translate_shifts_pivot_and_origin_equally() {
        let mut t = Transform2D::from_origin_anchor(
            Vec2d::new(100.0, 200.0),
            Vec2d::new(0.5, 0.5),
            Vec2d::new(120.0, 80.0),
            0.0,
        );
        let origin_before = t.origin_world();
        let pivot_before = t.pivot_world;
        let delta = Vec2d::new(10.0, -20.0);
        t.translate(delta);
        assert!(vec_eq(
            t.pivot_world,
            Vec2d::new(pivot_before.x + delta.x, pivot_before.y + delta.y)
        ));
        assert!(vec_eq(
            t.origin_world(),
            Vec2d::new(origin_before.x + delta.x, origin_before.y + delta.y)
        ));
    }

    // ── B-behavior end-to-end ─────────────────────────────────────────────────

    #[test]
    fn b_behavior_anchor_change_does_not_move_object() {
        // Construct: origin (300, 200), center anchor, 45° rotation.
        let mut t = Transform2D::from_origin_anchor(
            Vec2d::new(300.0, 200.0),
            Vec2d::new(0.5, 0.5),
            Vec2d::new(120.0, 80.0),
            FRAC_PI_4,
        );
        let origin_before = t.origin_world();
        let center_before = t.anchor_world_at(Vec2d::new(0.5, 0.5));

        // B behavior: change anchor to bottom-left — object must not jump.
        t.set_pivot_to_anchor(Vec2d::new(0.0, 0.0));

        assert!(
            vec_eq(t.origin_world(), origin_before),
            "origin jumped: before={:?} after={:?}",
            origin_before,
            t.origin_world(),
        );
        assert!(
            vec_eq(t.anchor_world_at(Vec2d::new(0.5, 0.5)), center_before),
            "center jumped: before={:?} after={:?}",
            center_before,
            t.anchor_world_at(Vec2d::new(0.5, 0.5)),
        );
    }

    #[test]
    fn b_behavior_rotate_then_change_anchor_does_not_jump() {
        // Simulate a drag interaction: rotate around pivot, then re-anchor.
        let mut t = Transform2D::from_origin_anchor(
            Vec2d::new(0.0, 0.0),
            Vec2d::new(0.5, 0.5),
            Vec2d::new(60.0, 40.0),
            0.0,
        );
        t.rotate(FRAC_PI_2); // 90° CCW
        let origin_after_rot = t.origin_world();

        // Now change pivot to top-right corner — object must not jump.
        t.set_pivot_to_anchor(Vec2d::new(1.0, 1.0));
        assert!(
            vec_eq(t.origin_world(), origin_after_rot),
            "jumped after rotate+anchor: {:?}",
            t.origin_world(),
        );
    }

    // ── rotate_vec ────────────────────────────────────────────────────────────

    #[test]
    fn rotate_vec_zero_angle_is_identity() {
        let v = Vec2d::new(3.0, -7.0);
        assert!(vec_eq(rotate_vec(v, 0.0), v));
    }

    #[test]
    fn rotate_vec_90_ccw() {
        // rotate((1, 0), 90°) = (0, 1)
        let v = rotate_vec(Vec2d::new(1.0, 0.0), FRAC_PI_2);
        assert!(approx(v.x, 0.0), "x: {}", v.x);
        assert!(approx(v.y, 1.0), "y: {}", v.y);
    }

    #[test]
    fn rotate_vec_180() {
        // rotate((3, 4), 180°) = (-3, -4)
        let v = rotate_vec(Vec2d::new(3.0, 4.0), PI);
        assert!(approx(v.x, -3.0), "x: {}", v.x);
        assert!(approx(v.y, -4.0), "y: {}", v.y);
    }
}
