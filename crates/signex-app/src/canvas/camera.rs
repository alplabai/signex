//! Camera system — Altium-style pan/zoom with cursor-centered scaling.
//!
//! World coordinates are in mm (Standard internal units).
//! Screen coordinates are in pixels.

use iced::{Point, Rectangle};

/// Camera state for pan and zoom.
#[derive(Debug, Clone)]
pub struct Camera {
    /// Offset of the world origin in screen pixels.
    /// Positive X = world moved right, positive Y = world moved down.
    pub offset: Point,
    /// Pixels per mm. Higher = zoomed in.
    pub scale: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            // Start centered roughly on an A4 sheet
            offset: Point::new(50.0, 50.0),
            scale: signex_types::schematic::SCHEMATIC_ZOOM_100_SCALE,
        }
    }
}

impl Camera {
    // 100% = `SCHEMATIC_ZOOM_100_SCALE`, so 25%..1600% = x0.25..x16.0.
    pub const MIN_SCALE: f32 = signex_types::schematic::SCHEMATIC_ZOOM_100_SCALE * 0.25;
    pub const MAX_SCALE: f32 = signex_types::schematic::SCHEMATIC_ZOOM_100_SCALE * 16.0;
    /// Per-wheel-notch zoom step, as a percent, bounds and default. The
    /// "Zoom speed" preference is stored in this unit; 10 gives the
    /// historical x1.10 notch.
    pub const ZOOM_STEP_PERCENT_MIN: u8 = 5;
    pub const ZOOM_STEP_PERCENT_MAX: u8 = 50;
    pub const ZOOM_STEP_PERCENT_DEFAULT: u8 = 10;
    /// Largest scroll magnitude (in notches) one event may apply, so a
    /// fast trackpad flick cannot jump the view across the whole range.
    const MAX_SCROLL_NOTCHES: f32 = 4.0;

    /// Clamp a stored "Zoom speed" percent into its valid range.
    pub fn clamp_zoom_step_percent(percent: i64) -> u8 {
        percent.clamp(
            Self::ZOOM_STEP_PERCENT_MIN as i64,
            Self::ZOOM_STEP_PERCENT_MAX as i64,
        ) as u8
    }

    /// Scale factor of one wheel notch for a "Zoom speed" percent
    /// (`10` -> `1.10`). Out-of-range input is clamped.
    pub fn zoom_step_factor(percent: u8) -> f32 {
        1.0 + Self::clamp_zoom_step_percent(percent as i64) as f32 / 100.0
    }

    /// Scale multiplier for `scroll_y` notches at per-notch factor `step`:
    /// `step^scroll_y`, so a mouse notch (`1.0`) is exactly one step and a
    /// trackpad's fractional deltas give a proportional, smooth zoom.
    /// `scroll_y` is clamped to +-4 notches per event; NaN means "no zoom"
    /// (`1.0`) so a bad device delta can never poison the camera.
    pub fn zoom_factor_for_scroll(step: f32, scroll_y: f32) -> f32 {
        if scroll_y.is_nan() {
            return 1.0;
        }
        step.powf(scroll_y.clamp(-Self::MAX_SCROLL_NOTCHES, Self::MAX_SCROLL_NOTCHES))
    }

    /// Convert world coordinates (mm) to screen coordinates (pixels).
    pub fn world_to_screen(&self, world: Point, _bounds: Rectangle) -> Point {
        Point::new(
            world.x * self.scale + self.offset.x,
            world.y * self.scale + self.offset.y,
        )
    }

    /// Convert screen coordinates (pixels) to world coordinates (mm).
    pub fn screen_to_world(&self, screen: Point, _bounds: Rectangle) -> Point {
        Point::new(
            (screen.x - self.offset.x) / self.scale,
            (screen.y - self.offset.y) / self.scale,
        )
    }

    /// Pan by a screen-space delta (pixels).
    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.offset.x += dx;
        self.offset.y += dy;
    }

    /// Zoom centered on a screen-space point.
    /// `scroll_y` > 0 = zoom in, < 0 = zoom out, in wheel notches; `step`
    /// is the per-notch factor (see [`Self::zoom_step_factor`]).
    pub fn zoom_at(
        &mut self,
        screen_pos: Point,
        scroll_y: f32,
        step: f32,
        _bounds: Rectangle,
    ) -> bool {
        if scroll_y == 0.0 {
            return false;
        }

        let factor = Self::zoom_factor_for_scroll(step, scroll_y);
        if !factor.is_finite() || factor == 1.0 {
            return false;
        }

        let new_scale = (self.scale * factor).clamp(Self::MIN_SCALE, Self::MAX_SCALE);
        if (new_scale - self.scale).abs() < f32::EPSILON {
            return false;
        }

        let actual_factor = new_scale / self.scale;

        // Adjust offset so the point under the cursor stays fixed
        self.offset.x = screen_pos.x - (screen_pos.x - self.offset.x) * actual_factor;
        self.offset.y = screen_pos.y - (screen_pos.y - self.offset.y) * actual_factor;
        self.scale = new_scale;
        true
    }

    /// Pan so `world` sits at the viewport centre, keeping the zoom (#525).
    pub fn center_on(&mut self, world: Point, viewport: Rectangle) {
        self.offset.x = viewport.width / 2.0 - world.x * self.scale;
        self.offset.y = viewport.height / 2.0 - world.y * self.scale;
    }

    /// Fit a world-space rectangle into the viewport with some padding.
    pub fn fit_rect(&mut self, world_rect: Rectangle, viewport: Rectangle) {
        let padding = 40.0; // screen pixels
        let available_w = viewport.width - padding * 2.0;
        let available_h = viewport.height - padding * 2.0;

        if world_rect.width <= 0.0 || world_rect.height <= 0.0 {
            return;
        }

        let scale_x = available_w / world_rect.width;
        let scale_y = available_h / world_rect.height;
        self.scale = scale_x.min(scale_y).clamp(Self::MIN_SCALE, Self::MAX_SCALE);

        // Center the rect
        let world_center_x = world_rect.x + world_rect.width / 2.0;
        let world_center_y = world_rect.y + world_rect.height / 2.0;
        self.offset.x = viewport.width / 2.0 - world_center_x * self.scale;
        self.offset.y = viewport.height / 2.0 - world_center_y * self.scale;
    }

    /// Current zoom percentage for display.
    pub fn zoom_percent(&self) -> f64 {
        (self.scale as f64 / signex_types::schematic::SCHEMATIC_ZOOM_100_SCALE as f64) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOUNDS: Rectangle = Rectangle {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
    };

    fn mid_camera() -> Camera {
        Camera {
            offset: Point::new(30.0, 20.0),
            scale: Camera::MIN_SCALE * 4.0,
        }
    }

    #[test]
    fn one_notch_multiplies_the_scale_by_the_step_and_pins_the_cursor_point() {
        // Arrange
        let mut camera = mid_camera();
        let cursor = Point::new(400.0, 250.0);
        let before = camera.screen_to_world(cursor, BOUNDS);
        let scale_before = camera.scale;

        // Act
        let changed = camera.zoom_at(cursor, 1.0, 1.25, BOUNDS);

        // Assert
        assert!(changed);
        assert!((camera.scale / scale_before - 1.25).abs() < 1e-4);
        let after = camera.screen_to_world(cursor, BOUNDS);
        assert!((after.x - before.x).abs() < 1e-3 && (after.y - before.y).abs() < 1e-3);
    }

    #[test]
    fn a_notch_out_divides_by_the_step() {
        let mut camera = mid_camera();
        let scale_before = camera.scale;

        camera.zoom_at(Point::new(10.0, 10.0), -1.0, 1.25, BOUNDS);

        assert!((scale_before / camera.scale - 1.25).abs() < 1e-4);
    }

    #[test]
    fn a_fractional_scroll_gives_a_proportional_step() {
        let mut camera = mid_camera();
        let scale_before = camera.scale;

        camera.zoom_at(Point::new(10.0, 10.0), 0.5, 1.21, BOUNDS);

        // 1.21^0.5 = 1.1
        assert!((camera.scale / scale_before - 1.1).abs() < 1e-4);
    }

    #[test]
    fn scroll_magnitude_is_capped_at_four_notches() {
        let mut camera = mid_camera();
        let scale_before = camera.scale;

        camera.zoom_at(Point::new(10.0, 10.0), 100.0, 1.1, BOUNDS);

        assert!((camera.scale / scale_before - 1.1_f32.powi(4)).abs() < 1e-3);
    }

    #[test]
    fn zoom_stops_at_the_scale_limits() {
        let mut camera = mid_camera();
        camera.scale = Camera::MAX_SCALE;
        assert!(!camera.zoom_at(Point::ORIGIN, 1.0, 1.5, BOUNDS));
        assert_eq!(camera.scale, Camera::MAX_SCALE);

        camera.scale = Camera::MIN_SCALE;
        assert!(!camera.zoom_at(Point::ORIGIN, -1.0, 1.5, BOUNDS));
        assert_eq!(camera.scale, Camera::MIN_SCALE);

        camera.scale = Camera::MAX_SCALE * 0.9;
        assert!(camera.zoom_at(Point::ORIGIN, 4.0, 1.5, BOUNDS));
        assert_eq!(camera.scale, Camera::MAX_SCALE);
    }

    #[test]
    fn a_nan_scroll_leaves_the_camera_untouched() {
        let mut camera = mid_camera();
        let (offset, scale) = (camera.offset, camera.scale);

        let changed = camera.zoom_at(Point::new(10.0, 10.0), f32::NAN, 1.1, BOUNDS);

        assert!(!changed);
        assert_eq!((camera.offset, camera.scale), (offset, scale));
    }

    #[test]
    fn an_infinite_scroll_clamps_to_four_notches_and_stays_finite() {
        for sign in [1.0_f32, -1.0] {
            let mut camera = mid_camera();
            let scale_before = camera.scale;

            camera.zoom_at(Point::new(10.0, 10.0), sign * f32::INFINITY, 1.1, BOUNDS);

            assert!(camera.scale.is_finite() && camera.offset.x.is_finite());
            assert!((camera.scale / scale_before - 1.1_f32.powf(4.0 * sign)).abs() < 1e-3);
        }
    }

    #[test]
    fn step_percent_maps_to_a_factor_and_is_clamped() {
        assert!((Camera::zoom_step_factor(10) - 1.10).abs() < 1e-6);
        assert!((Camera::zoom_step_factor(0) - 1.05).abs() < 1e-6);
        assert!((Camera::zoom_step_factor(200) - 1.50).abs() < 1e-6);
        assert_eq!(Camera::clamp_zoom_step_percent(-3), 5);
    }

    /// #1228 — iced hands the canvas its cursor and bounds in logical px
    /// already divided by OS scale x interface scale, so pick, drag and
    /// zoom pivots need no scale of their own. With the interface scale at
    /// 1.5 the same physical cursor lands on the same world point as at 1.0
    /// once both are expressed in their own logical px, and zooming about
    /// it keeps that point under the cursor.
    #[test]
    fn pick_and_zoom_pivot_hold_at_a_non_unit_interface_scale() {
        let os_scale = 1.25_f32;
        let physical_cursor = Point::new(900.0, 600.0);
        for ui_scale in [1.0_f32, 1.5, 2.0] {
            let total = os_scale * ui_scale;
            let logical = Point::new(physical_cursor.x / total, physical_cursor.y / total);
            let mut cam = mid_camera();
            let before = cam.screen_to_world(logical, BOUNDS);
            let back = cam.world_to_screen(before, BOUNDS);
            assert!((back.x - logical.x).abs() < 1e-3 && (back.y - logical.y).abs() < 1e-3);

            assert!(cam.zoom_at(logical, 1.0, 1.1, BOUNDS));
            let after = cam.screen_to_world(logical, BOUNDS);
            assert!(
                (after.x - before.x).abs() < 1e-3 && (after.y - before.y).abs() < 1e-3,
                "ui_scale {ui_scale}: {before:?} moved to {after:?}"
            );
        }
    }
}
