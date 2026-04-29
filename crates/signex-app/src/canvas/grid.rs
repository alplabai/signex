//! Grid renderer — dot grid with configurable spacing, visibility, snap.

use iced::widget::canvas;
use iced::{Color, Point};

use super::camera::Camera;

/// Schematic grid sizes in mm — exact multiples/fractions of 2.54 mm (100 mil).
/// Range: 0.635 mm (¼ grid) → 5.08 mm (2× grid). Default: 1.27 mm (Altium default, 50 mil).
pub const GRID_SIZES_MM: &[f32] = &[0.635, 1.27, 2.54, 5.08];

/// Human-readable labels for GRID_SIZES_MM (same order).
pub const GRID_SIZE_LABELS: &[&str] = &["0.635 mm", "1.27 mm", "2.54 mm", "5.08 mm"];

/// Human-readable labels for GRID_SIZES_MM when displaying in mils (1 mm ≈ 39.3701 mils exactly).
/// 0.635 mm = 25 mil, 1.27 mm = 50 mil, 2.54 mm = 100 mil, 5.08 mm = 200 mil.
pub const GRID_SIZE_LABELS_MIL: &[&str] = &["25 mil", "50 mil", "100 mil", "200 mil"];

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
            size_index: 1, // 1.27mm default (50mil) — Altium default
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
///
/// The grid is **zoom-adaptive**: the user's `grid_mm` is the *base* spacing,
/// and the actual minor step is the smallest `grid_mm * 5^k` (k=0..4) whose
/// on-screen distance is ≥ 6 px. Major lines are always 5× the minor step.
/// This keeps the visible dot density roughly constant across zoom levels
/// (no more "very prominent at one zoom, invisible at the next") and the
/// minor → major transitions cross-fade smoothly via alpha rather than
/// snapping on/off.
pub fn draw_grid(
    frame: &mut canvas::Frame,
    camera: &Camera,
    grid_mm: f32,
    bounds: iced::Rectangle,
    color: Color,
    page_w: f32,
    page_h: f32,
) {
    if grid_mm <= 0.0 || camera.scale <= 0.0 {
        return;
    }

    // --- 1. Pick adaptive minor step (5^k * grid_mm), aiming for ≥ 6 px. ---
    const MIN_PX: f32 = 6.0;
    let mut minor_mm = grid_mm;
    for _ in 0..4 {
        if minor_mm * camera.scale >= MIN_PX {
            break;
        }
        minor_mm *= 5.0;
    }
    let minor_screen = minor_mm * camera.scale;
    let major_mm = minor_mm * 5.0;
    let major_screen = major_mm * camera.scale;

    // Cross-fade so the new finer minor level fades in as zoom increases,
    // and the previous (now-major) level naturally takes over its role.
    let minor_alpha = ((minor_screen - MIN_PX) / MIN_PX).clamp(0.0, 1.0);
    if minor_alpha <= 0.0 {
        return;
    }
    let dot_color = Color {
        a: color.a * minor_alpha,
        ..color
    };

    // --- 2. Visible region clipped to page bounds. ---
    let tl = camera.screen_to_world(Point::new(0.0, 0.0), bounds);
    let br = camera.screen_to_world(Point::new(bounds.width, bounds.height), bounds);
    let wx_min = tl.x.max(0.0);
    let wy_min = tl.y.max(0.0);
    let wx_max = br.x.min(page_w);
    let wy_max = br.y.min(page_h);
    if wx_min >= wx_max || wy_min >= wy_max {
        return;
    }

    let start_x = (wx_min / minor_mm).floor() * minor_mm;
    let start_y = (wy_min / minor_mm).floor() * minor_mm;

    // Safety cap (adaptive step makes this very rare, but keep it).
    let cols = ((wx_max - start_x) / minor_mm) as i32 + 1;
    let rows = ((wy_max - start_y) / minor_mm) as i32 + 1;
    if (cols as i64) * (rows as i64) > 40_000 {
        return;
    }

    // --- 3. Minor dots. Radius scales with screen step, slightly. ---
    let dot_radius = (minor_screen * 0.06).clamp(0.5, 1.6);

    let style = signex_render::grid_style();
    // Small-cross arm length in screen pixels (Standard-style "+").
    let cross_arm = (minor_screen * 0.18).clamp(1.5, 4.0);
    let minor_stroke = canvas::Stroke::default()
        .with_color(dot_color)
        .with_width(0.6);

    // For Lines style we draw full minor grid lines instead of per-cell
    // glyphs and skip the per-point loop below entirely.
    if matches!(style, signex_render::GridStyle::Lines) {
        let page_top = camera.world_to_screen(Point::new(0.0, 0.0), bounds).y;
        let page_bot = camera.world_to_screen(Point::new(0.0, page_h), bounds).y;
        let page_left = camera.world_to_screen(Point::new(0.0, 0.0), bounds).x;
        let page_right = camera.world_to_screen(Point::new(page_w, 0.0), bounds).x;
        let line_y_top = page_top.max(0.0);
        let line_y_bot = page_bot.min(bounds.height);
        let line_x_left = page_left.max(0.0);
        let line_x_right = page_right.min(bounds.width);
        let mut wx = start_x;
        while wx <= wx_max + minor_mm * 0.5 {
            if wx >= 0.0 {
                let sx = camera.world_to_screen(Point::new(wx, 0.0), bounds).x;
                if sx >= 0.0 && sx <= bounds.width && line_y_top < line_y_bot {
                    let line = canvas::Path::line(
                        Point::new(sx, line_y_top),
                        Point::new(sx, line_y_bot),
                    );
                    frame.stroke(&line, minor_stroke);
                }
            }
            wx += minor_mm;
        }
        let mut wy = start_y;
        while wy <= wy_max + minor_mm * 0.5 {
            if wy >= 0.0 {
                let sy = camera.world_to_screen(Point::new(0.0, wy), bounds).y;
                if sy >= 0.0 && sy <= bounds.height && line_x_left < line_x_right {
                    let line = canvas::Path::line(
                        Point::new(line_x_left, sy),
                        Point::new(line_x_right, sy),
                    );
                    frame.stroke(&line, minor_stroke);
                }
            }
            wy += minor_mm;
        }
    } else {
        let mut wy = start_y;
        while wy <= wy_max + minor_mm * 0.5 {
            if wy >= 0.0 {
                let mut wx = start_x;
                while wx <= wx_max + minor_mm * 0.5 {
                    if wx >= 0.0 {
                        let screen = camera.world_to_screen(Point::new(wx, wy), bounds);
                        if screen.x >= -dot_radius
                            && screen.x <= bounds.width + dot_radius
                            && screen.y >= -dot_radius
                            && screen.y <= bounds.height + dot_radius
                        {
                            match style {
                                signex_render::GridStyle::Dots => {
                                    let dot = canvas::Path::circle(screen, dot_radius);
                                    frame.fill(&dot, dot_color);
                                }
                                signex_render::GridStyle::SmallCrosses => {
                                    let h = canvas::Path::line(
                                        Point::new(screen.x - cross_arm, screen.y),
                                        Point::new(screen.x + cross_arm, screen.y),
                                    );
                                    let v = canvas::Path::line(
                                        Point::new(screen.x, screen.y - cross_arm),
                                        Point::new(screen.x, screen.y + cross_arm),
                                    );
                                    frame.stroke(&h, minor_stroke);
                                    frame.stroke(&v, minor_stroke);
                                }
                                signex_render::GridStyle::Lines => unreachable!(),
                            }
                        }
                    }
                    wx += minor_mm;
                }
            }
            wy += minor_mm;
        }
    }

    // --- 4. Major lines, also adaptive and softly faded. ---
    // Fade in once major step is comfortably wide; fade out as it grows
    // huge so the *next* level can take over without two heavy line sets.
    let major_alpha_in = ((major_screen - 24.0) / 24.0).clamp(0.0, 1.0);
    let major_alpha_out = ((400.0 - major_screen) / 200.0).clamp(0.0, 1.0);
    let major_alpha = color.a * 0.35 * major_alpha_in * major_alpha_out;
    if major_alpha <= 0.005 {
        return;
    }
    let major_color = Color {
        a: major_alpha,
        ..color
    };
    let stroke = canvas::Stroke::default()
        .with_color(major_color)
        .with_width(0.5);

    let page_top = camera.world_to_screen(Point::new(0.0, 0.0), bounds).y;
    let page_bot = camera.world_to_screen(Point::new(0.0, page_h), bounds).y;
    let page_left = camera.world_to_screen(Point::new(0.0, 0.0), bounds).x;
    let page_right = camera.world_to_screen(Point::new(page_w, 0.0), bounds).x;
    let line_y_top = page_top.max(0.0);
    let line_y_bot = page_bot.min(bounds.height);
    let line_x_left = page_left.max(0.0);
    let line_x_right = page_right.min(bounds.width);

    let mut mx = (wx_min / major_mm).floor() * major_mm;
    while mx <= wx_max {
        let sx = camera.world_to_screen(Point::new(mx, 0.0), bounds).x;
        if sx >= 0.0 && sx <= bounds.width && line_y_top < line_y_bot {
            let line = canvas::Path::line(Point::new(sx, line_y_top), Point::new(sx, line_y_bot));
            frame.stroke(&line, stroke);
        }
        mx += major_mm;
    }

    let mut my = (wy_min / major_mm).floor() * major_mm;
    while my <= wy_max {
        let sy = camera.world_to_screen(Point::new(0.0, my), bounds).y;
        if sy >= 0.0 && sy <= bounds.height && line_x_left < line_x_right {
            let line = canvas::Path::line(Point::new(line_x_left, sy), Point::new(line_x_right, sy));
            frame.stroke(&line, stroke);
        }
        my += major_mm;
    }
}
