//! Grid renderer — dot grid with configurable spacing, visibility, snap.

use iced::widget::canvas;
use iced::{Color, Point};

use super::camera::Camera;

/// Schematic grid sizes in mm (matching KiCad/Altium defaults).
pub const GRID_SIZES_MM: &[f32] = &[0.635, 1.27, 2.54, 5.08, 10.16];

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
pub fn draw_grid(
    frame: &mut canvas::Frame,
    camera: &Camera,
    grid: &GridState,
    bounds: iced::Rectangle,
    color: Color,
) {
    let grid_mm = grid.size_mm();
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

    // Find world-space bounds of the visible area
    let tl = camera.screen_to_world(Point::new(0.0, 0.0), bounds);
    let br = camera.screen_to_world(Point::new(bounds.width, bounds.height), bounds);

    // Snap to grid boundaries
    let start_x = (tl.x / grid_mm).floor() * grid_mm;
    let start_y = (tl.y / grid_mm).floor() * grid_mm;
    let end_x = (br.x / grid_mm).ceil() * grid_mm;
    let end_y = (br.y / grid_mm).ceil() * grid_mm;

    // Safety limit — don't render more than ~100K dots
    let cols = ((end_x - start_x) / grid_mm) as i32 + 1;
    let rows = ((end_y - start_y) / grid_mm) as i32 + 1;
    if (cols as i64) * (rows as i64) > 100_000 {
        return;
    }

    // Dot size scales with zoom but has a minimum
    let dot_radius = (camera.scale * 0.3).clamp(0.5, 2.0);

    let mut wy = start_y;
    while wy <= end_y {
        let mut wx = start_x;
        while wx <= end_x {
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

    // Major grid lines every 5 grid steps (if visible)
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

        // Vertical major lines
        let mut mx = (start_x / major_mm).floor() * major_mm;
        while mx <= end_x {
            let sx = camera.world_to_screen(Point::new(mx, 0.0), bounds).x;
            if sx >= 0.0 && sx <= bounds.width {
                let line = canvas::Path::line(Point::new(sx, 0.0), Point::new(sx, bounds.height));
                frame.stroke(&line, stroke);
            }
            mx += major_mm;
        }

        // Horizontal major lines
        let mut my = (start_y / major_mm).floor() * major_mm;
        while my <= end_y {
            let sy = camera.world_to_screen(Point::new(0.0, my), bounds).y;
            if sy >= 0.0 && sy <= bounds.height {
                let line = canvas::Path::line(Point::new(0.0, sy), Point::new(bounds.width, sy));
                frame.stroke(&line, stroke);
            }
            my += major_mm;
        }
    }
}
