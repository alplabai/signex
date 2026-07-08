//! Global 2D rotation utilities for Signex geometry.
//!
//! This module is intentionally domain-agnostic: callers provide object pose,
//! geometry center, rotation space, and pivot. The same API can be reused by
//! schematic, symbol editor, PCB, or any future object model.
//!
//! Rotation order conventions:
//! - Local rotation: `M_new = M_old * R_local`
//! - World rotation: `M_new = R_world * M_old`
//!
//! Where `R_*` may include pivot translation as `T(p) * R * T(-p)`.

use std::f64::consts::{PI, TAU};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec2d {
    pub x: f64,
    pub y: f64,
}

impl Vec2d {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Pose2d {
    /// Object-space origin in world coordinates.
    pub origin: Vec2d,
    /// Counter-clockwise angle in radians.
    pub rotation_rad: f64,
}

impl Pose2d {
    pub const IDENTITY: Self = Self {
        origin: Vec2d::ZERO,
        rotation_rad: 0.0,
    };

    pub const fn new(origin: Vec2d, rotation_rad: f64) -> Self {
        Self {
            origin,
            rotation_rad,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationSpace {
    /// Rotate in object-local space (right-multiply model matrix).
    Local,
    /// Rotate in world space (left-multiply model matrix).
    World,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RotationPivot {
    /// Use object geometry center (`geometry_center_local`) as pivot.
    GeometryCenter,
    /// Use world origin `(0, 0)` as pivot.
    WorldOrigin,
    /// Use explicit world-space pivot.
    WorldPoint(Vec2d),
    /// Use explicit local-space pivot.
    LocalPoint(Vec2d),
}

/// Adapter trait for per-object rotation integration.
///
/// This keeps the rotation math centralized while allowing each object type
/// to map its own pose/anchor conventions when callers integrate it.
pub trait Rotatable2d {
    fn pose(&self) -> Pose2d;
    fn geometry_center_local(&self) -> Vec2d;
    fn set_pose(&mut self, pose: Pose2d);
}

/// Rotate any object implementing [`Rotatable2d`].
pub fn rotate_object<T: Rotatable2d>(
    object: &mut T,
    space: RotationSpace,
    pivot: RotationPivot,
    delta_rad: f64,
) {
    let next = rotate_pose(
        object.pose(),
        object.geometry_center_local(),
        space,
        pivot,
        delta_rad,
    );
    object.set_pose(next);
}

/// Rotate a pose using local or world semantics.
///
/// - `geometry_center_local`: object geometry center in local coordinates.
///   This is required for anchor-aware rotations and `GeometryCenter` pivot mode.
#[must_use]
pub fn rotate_pose(
    pose: Pose2d,
    geometry_center_local: Vec2d,
    space: RotationSpace,
    pivot: RotationPivot,
    delta_rad: f64,
) -> Pose2d {
    if delta_rad.abs() <= f64::EPSILON {
        return pose;
    }

    let model = pose_to_matrix(pose);

    let rotated = match space {
        RotationSpace::Local => {
            let pivot_local = resolve_local_pivot(pose, geometry_center_local, pivot);
            let delta = translation_matrix(pivot_local)
                .mul(rotation_matrix(delta_rad))
                .mul(translation_matrix(Vec2d::new(
                    -pivot_local.x,
                    -pivot_local.y,
                )));
            model.mul(delta)
        }
        RotationSpace::World => {
            let pivot_world = resolve_world_pivot(pose, geometry_center_local, pivot);
            let delta = translation_matrix(pivot_world)
                .mul(rotation_matrix(delta_rad))
                .mul(translation_matrix(Vec2d::new(
                    -pivot_world.x,
                    -pivot_world.y,
                )));
            delta.mul(model)
        }
    };

    pose_from_matrix(rotated)
}

/// Transform a local-space point to world-space with the given pose.
#[must_use]
pub fn transform_local_point(pose: Pose2d, local: Vec2d) -> Vec2d {
    pose_to_matrix(pose).transform_point(local)
}

/// Transform a world-space point back into object local coordinates.
#[must_use]
pub fn inverse_transform_world_point(pose: Pose2d, world: Vec2d) -> Vec2d {
    inverse_pose_matrix(pose).transform_point(world)
}

/// Compute object geometry center in world coordinates.
#[must_use]
pub fn geometry_center_world(pose: Pose2d, geometry_center_local: Vec2d) -> Vec2d {
    transform_local_point(pose, geometry_center_local)
}

/// Normalize angle into `(-PI, PI]` range.
#[must_use]
pub fn normalize_angle_rad(angle: f64) -> f64 {
    let mut wrapped = angle % TAU;
    if wrapped <= -PI {
        wrapped += TAU;
    } else if wrapped > PI {
        wrapped -= TAU;
    }
    wrapped
}

fn resolve_local_pivot(pose: Pose2d, geometry_center_local: Vec2d, pivot: RotationPivot) -> Vec2d {
    match pivot {
        RotationPivot::GeometryCenter => geometry_center_local,
        RotationPivot::LocalPoint(p) => p,
        RotationPivot::WorldOrigin => inverse_transform_world_point(pose, Vec2d::ZERO),
        RotationPivot::WorldPoint(p) => inverse_transform_world_point(pose, p),
    }
}

fn resolve_world_pivot(pose: Pose2d, geometry_center_local: Vec2d, pivot: RotationPivot) -> Vec2d {
    match pivot {
        RotationPivot::GeometryCenter => geometry_center_world(pose, geometry_center_local),
        RotationPivot::WorldOrigin => Vec2d::ZERO,
        RotationPivot::WorldPoint(p) => p,
        RotationPivot::LocalPoint(p) => transform_local_point(pose, p),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Mat3 {
    m: [[f64; 3]; 3],
}

impl Mat3 {
    fn mul(self, rhs: Self) -> Self {
        let mut out = [[0.0; 3]; 3];
        for (r, row) in out.iter_mut().enumerate() {
            for (c, cell) in row.iter_mut().enumerate() {
                *cell = self.m[r][0] * rhs.m[0][c]
                    + self.m[r][1] * rhs.m[1][c]
                    + self.m[r][2] * rhs.m[2][c];
            }
        }
        Self { m: out }
    }

    fn transform_point(self, p: Vec2d) -> Vec2d {
        Vec2d::new(
            self.m[0][0] * p.x + self.m[0][1] * p.y + self.m[0][2],
            self.m[1][0] * p.x + self.m[1][1] * p.y + self.m[1][2],
        )
    }
}

fn translation_matrix(v: Vec2d) -> Mat3 {
    Mat3 {
        m: [[1.0, 0.0, v.x], [0.0, 1.0, v.y], [0.0, 0.0, 1.0]],
    }
}

fn rotation_matrix(angle_rad: f64) -> Mat3 {
    let c = angle_rad.cos();
    let s = angle_rad.sin();
    Mat3 {
        m: [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]],
    }
}

fn pose_to_matrix(pose: Pose2d) -> Mat3 {
    translation_matrix(pose.origin).mul(rotation_matrix(pose.rotation_rad))
}

fn inverse_pose_matrix(pose: Pose2d) -> Mat3 {
    let c = pose.rotation_rad.cos();
    let s = pose.rotation_rad.sin();

    // Inverse of rigid transform T*R is R^T * T^-1.
    Mat3 {
        m: [
            [c, s, -(c * pose.origin.x + s * pose.origin.y)],
            [-s, c, -(-s * pose.origin.x + c * pose.origin.y)],
            [0.0, 0.0, 1.0],
        ],
    }
}

fn pose_from_matrix(m: Mat3) -> Pose2d {
    let rotation = normalize_angle_rad(m.m[1][0].atan2(m.m[0][0]));
    Pose2d::new(Vec2d::new(m.m[0][2], m.m[1][2]), rotation)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-9;

    fn rad(deg: f64) -> f64 {
        deg.to_radians()
    }

    fn assert_close(a: f64, b: f64) {
        assert!(
            (a - b).abs() <= EPS,
            "expected {a} ~= {b} (diff={})",
            (a - b).abs()
        );
    }

    fn assert_vec_close(a: Vec2d, b: Vec2d) {
        assert_close(a.x, b.x);
        assert_close(a.y, b.y);
    }

    #[test]
    fn local_rotation_about_geometry_center_keeps_center_fixed() {
        let pose = Pose2d::new(Vec2d::new(10.0, 4.0), rad(30.0));
        let center_local = Vec2d::new(3.0, -1.5);

        let before = geometry_center_world(pose, center_local);
        let after_pose = rotate_pose(
            pose,
            center_local,
            RotationSpace::Local,
            RotationPivot::GeometryCenter,
            rad(90.0),
        );
        let after = geometry_center_world(after_pose, center_local);

        assert_vec_close(before, after);
        assert_close(
            after_pose.rotation_rad,
            normalize_angle_rad(pose.rotation_rad + rad(90.0)),
        );
    }

    #[test]
    fn world_rotation_about_world_origin_orbits_origin_point() {
        let pose = Pose2d::new(Vec2d::new(2.0, 0.0), 0.0);

        let out = rotate_pose(
            pose,
            Vec2d::ZERO,
            RotationSpace::World,
            RotationPivot::WorldOrigin,
            rad(90.0),
        );

        assert_vec_close(out.origin, Vec2d::new(0.0, 2.0));
        assert_close(out.rotation_rad, rad(90.0));
    }

    #[test]
    fn local_rotation_about_local_origin_keeps_origin_fixed() {
        let pose = Pose2d::new(Vec2d::new(2.0, 0.0), 0.0);

        let out = rotate_pose(
            pose,
            Vec2d::ZERO,
            RotationSpace::Local,
            RotationPivot::LocalPoint(Vec2d::ZERO),
            rad(90.0),
        );

        assert_vec_close(out.origin, pose.origin);
        assert_close(out.rotation_rad, rad(90.0));
    }

    #[test]
    fn local_and_world_orders_differ_for_same_angle() {
        let pose = Pose2d::new(Vec2d::new(2.0, 0.0), 0.0);

        let local = rotate_pose(
            pose,
            Vec2d::ZERO,
            RotationSpace::Local,
            RotationPivot::LocalPoint(Vec2d::ZERO),
            rad(90.0),
        );
        let world = rotate_pose(
            pose,
            Vec2d::ZERO,
            RotationSpace::World,
            RotationPivot::WorldOrigin,
            rad(90.0),
        );

        assert_vec_close(local.origin, Vec2d::new(2.0, 0.0));
        assert_vec_close(world.origin, Vec2d::new(0.0, 2.0));
    }

    #[test]
    fn world_rotation_with_local_pivot_keeps_local_anchor_world_position() {
        let pose = Pose2d::new(Vec2d::new(4.0, 1.0), rad(20.0));
        let anchor_local = Vec2d::new(1.0, -2.0);

        let before_anchor_world = transform_local_point(pose, anchor_local);
        let out = rotate_pose(
            pose,
            Vec2d::ZERO,
            RotationSpace::World,
            RotationPivot::LocalPoint(anchor_local),
            rad(47.0),
        );
        let after_anchor_world = transform_local_point(out, anchor_local);

        assert_vec_close(before_anchor_world, after_anchor_world);
    }

    #[test]
    fn local_rotation_with_world_pivot_keeps_that_world_point_fixed() {
        let pose = Pose2d::new(Vec2d::new(-3.0, 5.0), rad(-35.0));
        let world_pivot = Vec2d::new(2.5, -1.75);

        let local_point_at_pivot = inverse_transform_world_point(pose, world_pivot);
        let out = rotate_pose(
            pose,
            Vec2d::ZERO,
            RotationSpace::Local,
            RotationPivot::WorldPoint(world_pivot),
            rad(30.0),
        );

        let transformed = transform_local_point(out, local_point_at_pivot);
        assert_vec_close(transformed, world_pivot);
    }

    #[test]
    fn normalize_angle_wraps_to_minus_pi_plus_pi() {
        assert_close(normalize_angle_rad(TAU + 0.25), 0.25);
        assert_close(normalize_angle_rad(-TAU - 0.25), -0.25);
        assert_close(normalize_angle_rad(PI), PI);
        assert_close(normalize_angle_rad(-PI), PI);
    }

    #[derive(Debug, Clone, Copy)]
    struct DummyObject {
        pose: Pose2d,
        center_local: Vec2d,
    }

    impl Rotatable2d for DummyObject {
        fn pose(&self) -> Pose2d {
            self.pose
        }

        fn geometry_center_local(&self) -> Vec2d {
            self.center_local
        }

        fn set_pose(&mut self, pose: Pose2d) {
            self.pose = pose;
        }
    }

    #[test]
    fn rotate_object_trait_matches_rotate_pose_result() {
        let mut object = DummyObject {
            pose: Pose2d::new(Vec2d::new(8.0, -3.0), rad(10.0)),
            center_local: Vec2d::new(2.0, 1.0),
        };

        let expected = rotate_pose(
            object.pose,
            object.center_local,
            RotationSpace::World,
            RotationPivot::GeometryCenter,
            rad(15.0),
        );

        rotate_object(
            &mut object,
            RotationSpace::World,
            RotationPivot::GeometryCenter,
            rad(15.0),
        );

        assert_vec_close(object.pose.origin, expected.origin);
        assert_close(object.pose.rotation_rad, expected.rotation_rad);
    }
}
