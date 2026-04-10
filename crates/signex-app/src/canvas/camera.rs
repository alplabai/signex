//! Camera system — Altium-style pan/zoom with cursor-centered scaling.
//!
//! World coordinates are in mm (KiCad internal units).
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
            scale: 3.0, // ~3 pixels per mm → reasonable default zoom
        }
    }
}

impl Camera {
    pub const MIN_SCALE: f32 = 0.05;
    pub const MAX_SCALE: f32 = 200.0;
    pub const ZOOM_FACTOR: f32 = 1.1;

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
    /// `scroll_y` > 0 = zoom in, < 0 = zoom out.
    pub fn zoom_at(&mut self, screen_pos: Point, scroll_y: f32, _bounds: Rectangle) {
        let factor = if scroll_y > 0.0 {
            Self::ZOOM_FACTOR
        } else {
            1.0 / Self::ZOOM_FACTOR
        };

        let new_scale = (self.scale * factor).clamp(Self::MIN_SCALE, Self::MAX_SCALE);
        let actual_factor = new_scale / self.scale;

        // Adjust offset so the point under the cursor stays fixed
        self.offset.x = screen_pos.x - (screen_pos.x - self.offset.x) * actual_factor;
        self.offset.y = screen_pos.y - (screen_pos.y - self.offset.y) * actual_factor;
        self.scale = new_scale;
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
        // 3.0 scale = 100% (default)
        (self.scale as f64 / 3.0) * 100.0
    }
}
