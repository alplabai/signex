//! The turn of a 2D view on screen (#2316): a value object every
//! screen/world transform of a turned view reads, so no caller works out its
//! own sine and cosine. It also says whether the view is seen from below
//! (#2394): mirrored left to right before the turn.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: elementary plane geometry.
//!
//! Screen and world both have y pointing down, so a positive angle turns the
//! drawing clockwise as the user sees it. A multiple of 90 degrees gets exact
//! 0 / 1 / -1 factors, so a quarter turn moves no point by a rounding error.

/// A clockwise turn of the view, in degrees within `[0, 360)`, of the
/// drawing as seen from above or, when mirrored, from below.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewRotation {
    degrees: f32,
    cos: f32,
    sin: f32,
    /// World x is negated before the turn: the view from below.
    mirrored: bool,
}

impl Default for ViewRotation {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl ViewRotation {
    /// No turn, seen from above.
    pub const IDENTITY: Self = Self {
        degrees: 0.0,
        cos: 1.0,
        sin: 0.0,
        mirrored: false,
    };

    /// A turn of `degrees` clockwise, folded into `[0, 360)`, seen from
    /// above. A non-finite angle is no turn.
    pub fn from_degrees(degrees: f32) -> Self {
        if !degrees.is_finite() {
            return Self::IDENTITY;
        }
        // `+ 0.0` turns -0 into 0; a value just under 360 can round up to it.
        let folded = degrees.rem_euclid(360.0) + 0.0;
        let degrees = if folded >= 360.0 { 0.0 } else { folded };
        let (sin, cos) = if degrees == 0.0 {
            (0.0, 1.0)
        } else if degrees == 90.0 {
            (1.0, 0.0)
        } else if degrees == 180.0 {
            (0.0, -1.0)
        } else if degrees == 270.0 {
            (-1.0, 0.0)
        } else {
            degrees.to_radians().sin_cos()
        };
        Self {
            degrees,
            cos,
            sin,
            mirrored: false,
        }
    }

    /// This turn seen from below (`true`) or from above.
    pub fn with_mirror(self, mirrored: bool) -> Self {
        Self { mirrored, ..self }
    }

    /// The angle, in `[0, 360)`.
    pub fn degrees(self) -> f32 {
        self.degrees
    }

    /// Whether the view is seen from below: mirrored left to right.
    pub fn is_mirrored(self) -> bool {
        self.mirrored
    }

    /// No turn and no mirror: world and screen axes agree.
    pub fn is_identity(self) -> bool {
        self.degrees == 0.0 && !self.mirrored
    }

    /// `[cos, sin]` of the turn. The mirror is not in them: see
    /// [`Self::is_mirrored`].
    pub fn factors(self) -> [f32; 2] {
        [self.cos, self.sin]
    }

    /// This turn followed by `degrees` more, mirrored as it was.
    pub fn turned_by(self, degrees: f32) -> Self {
        Self::from_degrees(self.degrees + degrees).with_mirror(self.mirrored)
    }

    /// `v` turned by this rotation (world direction to screen direction),
    /// mirrored first when seen from below.
    pub fn apply(self, v: [f32; 2]) -> [f32; 2] {
        let x = if self.mirrored { -v[0] } else { v[0] };
        [
            x * self.cos - v[1] * self.sin,
            x * self.sin + v[1] * self.cos,
        ]
    }

    /// `v` turned back (screen direction to world direction).
    pub fn undo(self, v: [f32; 2]) -> [f32; 2] {
        let x = v[0] * self.cos + v[1] * self.sin;
        let y = -v[0] * self.sin + v[1] * self.cos;
        [if self.mirrored { -x } else { x }, y]
    }

    /// World mm to screen px: `apply(world) * scale + offset`, the mapping of
    /// a turned view. The GPU camera ([`crate::camera::CameraUniform::ortho_turned`])
    /// builds the same matrix.
    pub fn world_to_screen(self, world: [f32; 2], offset_px: [f32; 2], scale: f32) -> [f32; 2] {
        let [x, y] = self.apply(world);
        [x * scale + offset_px[0], y * scale + offset_px[1]]
    }

    /// Screen px to world mm, the inverse of [`Self::world_to_screen`].
    pub fn screen_to_world(self, screen: [f32; 2], offset_px: [f32; 2], scale: f32) -> [f32; 2] {
        self.undo([
            (screen[0] - offset_px[0]) / scale,
            (screen[1] - offset_px[1]) / scale,
        ])
    }

    /// The world box `(min, max)` around what a `size`-px view shows, or
    /// `None` while the scale or offset is not usable.
    pub fn visible_world_box(
        self,
        size: [f32; 2],
        offset_px: [f32; 2],
        scale: f32,
    ) -> Option<([f32; 2], [f32; 2])> {
        let usable = scale.is_finite() && scale > 0.0 && offset_px.iter().all(|v| v.is_finite());
        if !usable {
            return None;
        }
        let corners = [[0.0, 0.0], [size[0], 0.0], [0.0, size[1]], size]
            .map(|c| self.screen_to_world(c, offset_px, scale));
        let (mut min, mut max) = (corners[0], corners[0]);
        for c in &corners[1..] {
            min = [min[0].min(c[0]), min[1].min(c[1])];
            max = [max[0].max(c[0]), max[1].max(c[1])];
        }
        Some((min, max))
    }

    /// Width and height of the axis-aligned box around a `size` rectangle
    /// turned by this rotation.
    pub fn bounding_size(self, size: [f32; 2]) -> [f32; 2] {
        let (c, s) = (self.cos.abs(), self.sin.abs());
        [size[0] * c + size[1] * s, size[0] * s + size[1] * c]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angles_fold_into_one_turn() {
        assert_eq!(ViewRotation::from_degrees(-90.0).degrees(), 270.0);
        assert_eq!(ViewRotation::from_degrees(360.0).degrees(), 0.0);
        assert_eq!(ViewRotation::from_degrees(450.0).degrees(), 90.0);
        assert_eq!(ViewRotation::from_degrees(-0.0).degrees(), 0.0);
        assert!(ViewRotation::from_degrees(f32::NAN).is_identity());
        assert!(ViewRotation::from_degrees(f32::INFINITY).is_identity());
    }

    #[test]
    fn quarter_turns_are_exact() {
        let r = ViewRotation::from_degrees(90.0);
        // Clockwise on a y-down screen: +x goes to +y.
        assert_eq!(r.apply([1.0, 0.0]), [0.0, 1.0]);
        assert_eq!(r.apply([0.0, 1.0]), [-1.0, 0.0]);
        assert_eq!(
            ViewRotation::from_degrees(180.0).apply([3.0, 4.0]),
            [-3.0, -4.0]
        );
    }

    #[test]
    fn undo_inverts_apply_at_any_angle() {
        for deg in [0.0, 37.0, 90.0, 180.0, 233.5, 359.0] {
            let r = ViewRotation::from_degrees(deg);
            let back = r.undo(r.apply([12.5, -7.25]));
            assert!((back[0] - 12.5).abs() < 1e-4 && (back[1] + 7.25).abs() < 1e-4);
        }
    }

    #[test]
    fn turning_adds_and_folds() {
        let r = ViewRotation::from_degrees(300.0).turned_by(90.0);
        assert_eq!(r.degrees(), 30.0);
    }

    #[test]
    fn the_bounding_size_swaps_at_a_quarter_turn() {
        let r = ViewRotation::from_degrees(90.0);
        assert_eq!(r.bounding_size([40.0, 10.0]), [10.0, 40.0]);
        let r = ViewRotation::from_degrees(45.0).bounding_size([10.0, 10.0]);
        assert!((r[0] - 10.0 * 2f32.sqrt()).abs() < 1e-4);
    }

    /// #2394: seen from below, +x goes left on screen; undo inverts it at
    /// any angle, and a further turn keeps the mirror.
    #[test]
    fn a_mirrored_view_flips_x_before_the_turn() {
        let below = ViewRotation::IDENTITY.with_mirror(true);
        assert!(!below.is_identity() && below.is_mirrored());
        assert_eq!(below.apply([3.0, 4.0]), [-3.0, 4.0]);
        for deg in [0.0, 37.0, 90.0, 233.5] {
            let r = ViewRotation::from_degrees(deg).with_mirror(true);
            let back = r.undo(r.apply([12.5, -7.25]));
            assert!((back[0] - 12.5).abs() < 1e-4 && (back[1] + 7.25).abs() < 1e-4);
        }
        assert!(below.turned_by(90.0).is_mirrored());
    }
}
