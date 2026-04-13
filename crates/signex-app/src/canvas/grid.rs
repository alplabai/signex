//! Grid renderer — dot grid with configurable spacing, visibility, snap.

use iced::widget::canvas;
use iced::{Color, Point};

use super::camera::Camera;

/// Schematic grid sizes in mm — exact multiples/fractions of 2.54 mm (100 mil).
/// Range: 0.635 mm (¼ grid) → 5.08 mm (2× grid). Default: 2.54 mm.
pub const GRID_SIZES_MM: &[f32] = &[0.635, 1.27, 2.54, 5.08];

/// Human-readable labels for GRID_SIZES_MM (same order).
pub const GRID_SIZE_LABELS: &[&str] = &["0.635 mm", "1.27 mm", "2.54 mm", "5.08 mm"];

/// Grid state — size, visibility, snap.
#[derive(Debug, Clone)]
pub struct GridState {
    /// Index into GRID_SIZES_MM.
    pub size_index: usize,
    /// Whether snap-to-grid is enabled.
    #[allow(dead_code)]
    pub snap: bool,
}

impl Default for GridState {
    fn default() -> Self {
        Self {
            size_index: 2, // 2.54mm default (100mil)
            snap: true,
        }
    }
}

impl GridState {
    /// Current grid size in mm.
    pub fn size_mm(&self) -> f32 {
        GRID_SIZES_MM[self.size_index]
    }

    /// Cycle to next grid size (wraps around).
    #[allow(dead_code)]
    pub fn cycle_forward(&mut self) {
        self.size_index = (self.size_index + 1) % GRID_SIZES_MM.len();
    }

    /// Cycle to previous grid size (wraps around).
    #[allow(dead_code)]
    pub fn cycle_backward(&mut self) {
        if self.size_index == 0 {
            self.size_index = GRID_SIZES_MM.len() - 1;
        } else {
            self.size_index -= 1;
        }
    }

    /// Snap a world-space coordinate to the nearest grid point.
    #[allow(dead_code)]
    pub fn snap_world(&self, world: Point) -> Point {
        if !self.snap {
            return world;
        }
        let g = self.size_mm();
        Point::new((world.x / g).round() * g, (world.y / g).round() * g)
    }
}

/// Draw the grid dots on a canvas frame.
/// Grid dots are clipped to the page bounds (A4 landscape: 297×210 mm) so
/// we never iterate over off-page area — this keeps both dot count and major-
/// line length proportional to the visible *page* area rather than the full
/// viewport, which prevents the quadratic blowup that occurred at low zoom.
pub fn draw_grid(
    frame: &mut canvas::Frame,
    camera: &Camera,
    grid_mm: f32,
    bounds: iced::Rectangle,
    color: Color,
) {
    // Page bounds in world space (A4 landscape, mm).
    const PAGE_W: f32 = 297.0;
    const PAGE_H: f32 = 210.0;

    let grid_mm = grid_mm;
    let grid_screen = grid_mm * camera.scale;

    // Don't draw grid if dots would be too dense (< 4px apart)
    if grid_screen < 4.0 {
        return;
    }

    // Fade grid when dots are very close together
    let alpha = if grid_screen < 8.0 {
        (grid_screen - 4.0) / 4.0
    } else {
        1.0
    };
    let dot_color = Color {
        a: color.a * alpha,
        ..color
    };

    // World-space bounds visible on screen
    let tl = camera.screen_to_world(Point::new(0.0, 0.0), bounds);
    let br = camera.screen_to_world(Point::new(bounds.width, bounds.height), bounds);

    // Clamp to page bounds — only draw grid inside the page rectangle
    let wx_min = tl.x.max(0.0);
    let wy_min = tl.y.max(0.0);
    let wx_max = br.x.min(PAGE_W);
    let wy_max = br.y.min(PAGE_H);

    // Page not visible at all — nothing to draw
    if wx_min >= wx_max || wy_min >= wy_max {
        return;
    }

    // Snap start positions to grid boundaries
    let start_x = (wx_min / grid_mm).floor() * grid_mm;
    let start_y = (wy_min / grid_mm).floor() * grid_mm;

    // Safety limit — don't render more than ~40 K dots
    let cols = ((wx_max - start_x) / grid_mm) as i32 + 1;
    let rows = ((wy_max - start_y) / grid_mm) as i32 + 1;
    if (cols as i64) * (rows as i64) > 40_000 {
        return;
    }

    // Dot size scales with zoom but has a minimum
    let dot_radius = (camera.scale * 0.3).clamp(0.5, 2.0);

    let mut wy = start_y;
    while wy <= wy_max + grid_mm * 0.5 {
        if wy < 0.0 {
            wy += grid_mm;
            continue;
        }
        let mut wx = start_x;
        while wx <= wx_max + grid_mm * 0.5 {
            if wx < 0.0 {
                wx += grid_mm;
                continue;
            }
            let screen = camera.world_to_screen(Point::new(wx, wy), bounds);

            // Only draw if on-screen
            if screen.x >= -dot_radius
                && screen.x <= bounds.width + dot_radius
                && screen.y >= -dot_radius
                && screen.y <= bounds.height + dot_radius
            {
                let dot = canvas::Path::circle(screen, dot_radius);
                frame.fill(&dot, dot_color);
            }

            wx += grid_mm;
        }
        wy += grid_mm;
    }

    // Major grid lines every 5 grid steps (if visible), also clipped to page
    let major_mm = grid_mm * 5.0;
    let major_screen = major_mm * camera.scale;
    if major_screen >= 30.0 {
        let major_color = Color {
            a: dot_color.a * 0.4,
            ..dot_color
        };
        let stroke = canvas::Stroke::default()
            .with_color(major_color)
            .with_width(0.5);

        // Page edges in screen space — major lines don't extend beyond these
        let page_top = camera.world_to_screen(Point::new(0.0, 0.0), bounds).y;
        let page_bot = camera.world_to_screen(Point::new(0.0, PAGE_H), bounds).y;
        let page_left = camera.world_to_screen(Point::new(0.0, 0.0), bounds).x;
        let page_right = camera.world_to_screen(Point::new(PAGE_W, 0.0), bounds).x;

        // Clamp line extent to both viewport and page
        let line_y_top = page_top.max(0.0);
        let line_y_bot = page_bot.min(bounds.height);
        let line_x_left = page_left.max(0.0);
        let line_x_right = page_right.min(bounds.width);

        // Vertical major lines
        let mut mx = (wx_min / major_mm).floor() * major_mm;
        while mx <= wx_max {
            let sx = camera.world_to_screen(Point::new(mx, 0.0), bounds).x;
            if sx >= 0.0 && sx <= bounds.width && line_y_top < line_y_bot {
                let line = canvas::Path::line(
                    Point::new(sx, line_y_top),
                    Point::new(sx, line_y_bot),
                );
                frame.stroke(&line, stroke);
            }
            mx += major_mm;
        }

        // Horizontal major lines
        let mut my = (wy_min / major_mm).floor() * major_mm;
        while my <= wy_max {
            let sy = camera.world_to_screen(Point::new(0.0, my), bounds).y;
            if sy >= 0.0 && sy <= bounds.height && line_x_left < line_x_right {
                let line = canvas::Path::line(
                    Point::new(line_x_left, sy),
                    Point::new(line_x_right, sy),
                );
                frame.stroke(&line, stroke);
            }
            my += major_mm;
        }
    }
}
