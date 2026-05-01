//! World ↔ screen transform for the schematic canvas.
//!
//! Replaces the old `ScreenTransform` type (deleted in Wave 0). Carries
//! the canvas size, world-space pan centre, and a world-units-per-pixel
//! zoom, plus the conversions every primitive needs.
//!
//! Coordinates: world space is millimetres, Y-down (matching
//! `signex_types::schematic::Point`); screen space is iced pixels with
//! origin at the canvas top-left.

use signex_types::schematic::{Aabb, Point};

/// World ↔ screen transform — see module doc.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub struct Viewport {
    /// Canvas size in pixels (width, height).
    pub size: iced::Size,
    /// World coordinate that maps to the canvas centre.
    pub centre_world: Point,
    /// Zoom factor — pixels per world millimetre. `> 0`. A value of
    /// `10.0` means each world millimetre paints 10 screen pixels.
    pub zoom_px_per_mm: f64,
}

impl Viewport {
    /// Build a viewport. `zoom_px_per_mm` must be `> 0`; zero or
    /// negative values clamp to a small positive epsilon to avoid NaN
    /// downstream.
    #[inline]
    pub fn new(size: iced::Size, centre_world: Point, zoom_px_per_mm: f64) -> Self {
        Self {
            size,
            centre_world,
            zoom_px_per_mm: zoom_px_per_mm.max(1e-6),
        }
    }

    /// World mm per screen pixel — the inverse of `zoom_px_per_mm`.
    /// Useful for converting hit-test tolerances.
    #[inline]
    pub fn world_per_pixel(&self) -> f64 {
        1.0 / self.zoom_px_per_mm
    }

    /// Project a world point onto canvas pixels. iced uses `f32`
    /// pixel coordinates, so a final cast happens at the boundary;
    /// internal math is `f64` to keep mm→pixel precision honest.
    pub fn world_to_screen(&self, p: Point) -> iced::Point {
        let dx = (p.x - self.centre_world.x) * self.zoom_px_per_mm;
        let dy = (p.y - self.centre_world.y) * self.zoom_px_per_mm;
        let cx = self.size.width as f64 * 0.5 + dx;
        let cy = self.size.height as f64 * 0.5 + dy;
        iced::Point::new(cx as f32, cy as f32)
    }

    /// Inverse of [`Self::world_to_screen`]. Rounds-trip at f32 pixel
    /// precision; callers that need sub-pixel accuracy should keep
    /// world-space coordinates and re-render.
    pub fn screen_to_world(&self, p: iced::Point) -> Point {
        let dx = (p.x as f64 - self.size.width as f64 * 0.5) * self.world_per_pixel();
        let dy = (p.y as f64 - self.size.height as f64 * 0.5) * self.world_per_pixel();
        Point::new(self.centre_world.x + dx, self.centre_world.y + dy)
    }

    /// World-space rectangle currently visible on the canvas. Useful
    /// for primitive-level frustum culling so primitives outside the
    /// viewport skip tessellation.
    pub fn visible_world_bounds(&self) -> Aabb {
        let half_w = self.size.width as f64 * 0.5 * self.world_per_pixel();
        let half_h = self.size.height as f64 * 0.5 * self.world_per_pixel();
        Aabb::new(
            self.centre_world.x - half_w,
            self.centre_world.y - half_h,
            self.centre_world.x + half_w,
            self.centre_world.y + half_h,
        )
    }
}

impl Default for Viewport {
    /// 800×600 canvas, world origin at the centre, zoom = 10 px / mm.
    /// Default exists to make tests cheap; production callers always
    /// pass a real size from `iced::widget::canvas::layout`.
    #[inline]
    fn default() -> Self {
        Self {
            size: iced::Size::new(800.0, 600.0),
            centre_world: Point::ZERO,
            zoom_px_per_mm: 10.0,
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
        // iced::Point holds f32, so the round-trip is bounded by half
        // a world-pixel = world_per_pixel * 0.5 ≈ 0.05 mm at the
        // default 10 px/mm zoom; assert well inside that.
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
        let v = Viewport::new(iced::Size::new(100.0, 100.0), Point::ZERO, 5.0);
        assert!((v.world_per_pixel() - 0.2).abs() < 1e-9);
    }

    #[test]
    fn visible_bounds_widen_when_zoom_drops() {
        let zoomed_in = Viewport::new(iced::Size::new(100.0, 100.0), Point::ZERO, 50.0);
        let zoomed_out = Viewport::new(iced::Size::new(100.0, 100.0), Point::ZERO, 5.0);
        let in_bounds = zoomed_in.visible_world_bounds();
        let out_bounds = zoomed_out.visible_world_bounds();
        assert!(out_bounds.width() > in_bounds.width());
        assert!(out_bounds.height() > in_bounds.height());
    }

    #[test]
    fn near_zero_zoom_clamps_without_nan() {
        let v = Viewport::new(iced::Size::new(10.0, 10.0), Point::ZERO, 0.0);
        let p = v.world_to_screen(Point::new(1.0, 1.0));
        assert!(p.x.is_finite());
        assert!(p.y.is_finite());
    }
}
