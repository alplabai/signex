//! World ↔ screen transform for the schematic canvas.
//!
//! Replaces the old `ScreenTransform` type (deleted in Wave 0). Stores
//! a screen-pixel pan offset and a scalar zoom factor (`scale` = pixels
//! per world millimetre); canvas size is supplied separately to
//! [`Viewport::visible_world_bounds`] / [`super::RenderContext`] when
//! frustum culling needs it. The struct itself is intentionally
//! lightweight so v0.11 callers' `ScreenTransform { ... }` literals
//! continue to work unchanged.
//!
//! Coordinates: world space is millimetres, Y-down (matching
//! `signex_types::schematic::Point`); screen space is iced pixels with
//! origin at the canvas top-left. The mapping is
//!
//! ```text
//! screen.x = world.x * scale + offset_x
//! screen.y = world.y * scale + offset_y
//! ```
//!
//! Field layout matches the v0.11 `ScreenTransform` so existing callers
//! (`signex-app::canvas`, the PCB canvas, panel previews) construct
//! `Viewport` with the same struct-literal shape they used before.

use signex_types::schematic::{Aabb, Point};

/// Helper trait so [`Viewport::world_to_screen`] accepts both a
/// [`Point`] and a `(f64, f64)` pair (v0.11 call shape).
pub trait IntoWorldPoint {
    fn into_world_point(self) -> Point;
}

impl IntoWorldPoint for Point {
    #[inline]
    fn into_world_point(self) -> Point {
        self
    }
}

impl IntoWorldPoint for (f64, f64) {
    #[inline]
    fn into_world_point(self) -> Point {
        Point::new(self.0, self.1)
    }
}

/// World ↔ screen transform — see module doc.
///
/// Field layout matches the v0.11 `ScreenTransform` so existing
/// struct-literal callers (`ScreenTransform { offset_x, offset_y, scale }`)
/// continue to compile against the v0.12 alias. Canvas size is **not**
/// stored on the viewport — primitives that need to know the visible
/// region pass it explicitly (see [`visible_world_bounds`]) or read it
/// from the active iced [`Frame`](iced::widget::canvas::Frame).
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub struct Viewport {
    /// Screen-pixel pan offset on the X axis.
    pub offset_x: f32,
    /// Screen-pixel pan offset on the Y axis.
    pub offset_y: f32,
    /// Scalar zoom — screen pixels per world millimetre. Must be `> 0`.
    /// [`Viewport::new`] clamps non-positive values to `1e-6`; direct
    /// field writes (`viewport.scale = 0.0`) bypass the clamp, so
    /// callers that mutate this field after construction must validate
    /// it themselves. The conversions [`Self::world_to_screen`] /
    /// [`Self::screen_to_world`] don't re-clamp at call time (hot path).
    pub scale: f32,
}

impl Viewport {
    /// Build a viewport. `scale` must be `> 0`; non-positive values clamp
    /// to a small positive epsilon to avoid NaN downstream.
    #[inline]
    pub fn new(offset_x: f32, offset_y: f32, scale: f32) -> Self {
        Self {
            offset_x,
            offset_y,
            scale: if scale > 0.0 { scale } else { 1e-6 },
        }
    }

    /// World mm per screen pixel.
    #[inline]
    pub fn world_per_pixel(&self) -> f64 {
        1.0 / self.scale as f64
    }

    /// Pixels per world millimetre, as `f64` for math precision.
    #[inline]
    pub fn zoom_px_per_mm(&self) -> f64 {
        self.scale as f64
    }

    /// Project a world point onto canvas pixels.
    ///
    /// Accepts either a [`Point`] or a `(x, y)` pair via the
    /// [`IntoWorldPoint`] trait so v0.11 callers (`world_to_screen(x, y)`)
    /// continue to compile without converting their tuples first.
    #[inline]
    pub fn world_to_screen(&self, p: impl IntoWorldPoint) -> iced::Point {
        let p = p.into_world_point();
        iced::Point::new(
            p.x as f32 * self.scale + self.offset_x,
            p.y as f32 * self.scale + self.offset_y,
        )
    }

    /// Inverse of [`Self::world_to_screen`].
    #[inline]
    pub fn screen_to_world(&self, p: iced::Point) -> Point {
        Point::new(
            ((p.x - self.offset_x) / self.scale) as f64,
            ((p.y - self.offset_y) / self.scale) as f64,
        )
    }

    /// World-space rectangle currently visible inside a canvas of the
    /// given pixel size. Used by primitive-level frustum culling so
    /// off-screen items skip tessellation.
    pub fn visible_world_bounds(&self, size: iced::Size) -> Aabb {
        let tl = self.screen_to_world(iced::Point::new(0.0, 0.0));
        let br = self.screen_to_world(iced::Point::new(size.width, size.height));
        Aabb::new(tl.x, tl.y, br.x, br.y)
    }
}

impl Default for Viewport {
    /// Origin at the canvas top-left, zoom 10 px / mm. Cheap default
    /// for tests; production callers always pass real values from the
    /// canvas layout.
    #[inline]
    fn default() -> Self {
        Self {
            offset_x: 0.0,
            offset_y: 0.0,
            scale: 10.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_to_screen_round_trips_within_pixel_precision() {
        let v = Viewport::default();
        let world = Point::new(1.27, -2.54);
        let screen = v.world_to_screen(world);
        let back = v.screen_to_world(screen);
        let tol = v.world_per_pixel();
        assert!(
            (back.x - world.x).abs() < tol,
            "x off by {}",
            back.x - world.x
        );
        assert!(
            (back.y - world.y).abs() < tol,
            "y off by {}",
            back.y - world.y
        );
    }

    #[test]
    fn world_per_pixel_is_inverse_of_zoom() {
        let v = Viewport::new(0.0, 0.0, 5.0);
        assert!((v.world_per_pixel() - 0.2).abs() < 1e-9);
    }

    #[test]
    fn visible_bounds_widen_when_zoom_drops() {
        let size = iced::Size::new(100.0, 100.0);
        let zoomed_in = Viewport::new(50.0, 50.0, 50.0);
        let zoomed_out = Viewport::new(50.0, 50.0, 5.0);
        let in_bounds = zoomed_in.visible_world_bounds(size);
        let out_bounds = zoomed_out.visible_world_bounds(size);
        assert!(out_bounds.width() > in_bounds.width());
        assert!(out_bounds.height() > in_bounds.height());
    }

    #[test]
    fn near_zero_zoom_clamps_without_nan() {
        let v = Viewport::new(0.0, 0.0, 0.0);
        let p = v.world_to_screen(Point::new(1.0, 1.0));
        assert!(p.x.is_finite());
        assert!(p.y.is_finite());
    }
}
