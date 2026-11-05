//! Grid renderer — dot grid with configurable spacing, visibility, snap.

use iced::advanced::graphics::geometry::Renderer as GeometryRenderer;
use iced::widget::canvas;
use iced::{Color, Point};

use super::camera::Camera;
use crate::render_config::GridStyle;

/// Schematic grid sizes in mm — exact multiples/fractions of 2.54 mm (100 mil).
/// Range: 0.635 mm (¼ grid) → 5.08 mm (2× grid). Default: 1.27 mm (Altium default, 50 mil).
pub const GRID_SIZES_MM: &[f32] = &[0.635, 1.27, 2.54, 5.08];

/// The grid pitch as the exact `f64` millimetre value the user sees.
///
/// The pitch is stored as `f32`, and widening it with `f64::from` turns
/// `1.27` into `1.2699999809…`, which lands snapped points 1–4 nm off the
/// grid in the saved file. Rounding to the nearest whole nanometre recovers
/// the intended decimal (`1.27f32` -> `1.27f64`).
pub fn pitch_mm_exact(mm: f32) -> f64 {
    (f64::from(mm) * 1e6).round() / 1e6
}

/// Human-readable labels for GRID_SIZES_MM (same order).
pub const GRID_SIZE_LABELS: &[&str] = &["0.635 mm", "1.27 mm", "2.54 mm", "5.08 mm"];

/// Human-readable labels for GRID_SIZES_MM when displaying in mils (1 mm ≈ 39.3701 mils exactly).
/// 0.635 mm = 25 mil, 1.27 mm = 50 mil, 2.54 mm = 100 mil, 5.08 mm = 200 mil.
pub const GRID_SIZE_LABELS_MIL: &[&str] = &["25 mil", "50 mil", "100 mil", "200 mil"];

/// Smallest `grid_mm * 5^k` (k = 0..=4) whose on-screen distance at `scale`
/// px/mm is at least `min_px`, or `None` when even the coarsest level is too
/// dense to draw. The result is always an integer multiple of `grid_mm`, so a
/// drawn line never lands off a real grid point (a plain pixel clamp would).
pub fn adaptive_pitch_mm(grid_mm: f32, scale: f32, min_px: f32) -> Option<f32> {
    if grid_mm <= 0.0 || scale <= 0.0 {
        return None;
    }
    let mut pitch = grid_mm;
    for _ in 0..5 {
        if pitch * scale >= min_px {
            return Some(pitch);
        }
        pitch *= 5.0;
    }
    None
}

/// Fixed widths of the grid lines and crosses, in logical px (not a zoom law). A line or arm
/// is kept off the paper edge by half of its width (#1940).
pub(crate) const MINOR_GRID_LINE_PX: f32 = 0.6;
const MAJOR_GRID_LINE_PX: f32 = 0.5;

/// One grid dot, as a filled square with the area of a `radius` circle.
///
/// A dot is at most a few pixels wide, where a square reads the same as a
/// circle. A circle path costs a full lyon tessellation per dot (about 2 µs,
/// over 20 ms for a dense sheet on every pan frame); a rectangle is 4 vertices.
pub(crate) fn fill_grid_dot<R: GeometryRenderer>(
    frame: &mut canvas::Frame<R>,
    centre: Point,
    radius: f32,
    color: Color,
) {
    // At least 1 px, so the square always holds a pixel centre: tiny-skia
    // fills rectangles without anti-aliasing and would drop a smaller one.
    let side = (radius * std::f32::consts::PI.sqrt()).max(1.0);
    frame.fill_rectangle(
        Point::new(centre.x - side / 2.0, centre.y - side / 2.0),
        iced::Size::new(side, side),
        color,
    );
}

/// One small "+" grid glyph, as two filled bars `width` thick reaching `arm`
/// past `centre` (the same footprint as two butt-capped strokes, without the
/// per-glyph stroke tessellation). Each bar's thin axis is centred on a
/// device pixel centre (`device_scale` device px per logical px): tiny-skia
/// fills rectangles without anti-aliasing, and a bar under 1 px thick between
/// two pixel centres would not draw.
pub(crate) fn fill_grid_cross<R: GeometryRenderer>(
    frame: &mut canvas::Frame<R>,
    centre: Point,
    arm: f32,
    width: f32,
    device_scale: f32,
    color: Color,
) {
    let half = width / 2.0;
    let k = sanitised_device_scale(device_scale);
    let on_pixel = |v: f32| ((v * k).floor() + 0.5) / k;
    frame.fill_rectangle(
        Point::new(centre.x - arm, on_pixel(centre.y) - half),
        iced::Size::new(arm * 2.0, width),
        color,
    );
    frame.fill_rectangle(
        Point::new(on_pixel(centre.x) - half, centre.y - arm),
        iced::Size::new(width, arm * 2.0),
        color,
    );
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
///
/// `style` is the caller's effective glyph choice — #630 moved it off a
/// process global that this function used to read from inside `draw`.
#[expect(
    clippy::too_many_arguments,
    reason = "9 arguments: camera + grid metrics + page bounds + colour + glyph style + device scale, all per-frame draw inputs"
)]
pub fn draw_grid<R: GeometryRenderer>(
    frame: &mut canvas::Frame<R>,
    camera: &Camera,
    grid_mm: f32,
    bounds: iced::Rectangle,
    color: Color,
    page_w: f32,
    page_h: f32,
    style: GridStyle,
    device_px_per_logical_px: f32,
) {
    // #1940 - every glyph and line stays inside the paper in this window's
    // device pixels, so none straddles the edge onto the canvas.
    let Some(paper_bounds) = paper_clip(camera, bounds, page_w, page_h, device_px_per_logical_px)
    else {
        return;
    };
    // No `Frame::with_clip` here: iced_wgpu pastes a clipped draft's meshes
    // ahead of the parent's pending buffer, which put the grid under the
    // paper fill (#1993). Every glyph and line stays inside `paper_bounds`.
    draw_grid_within(
        frame,
        camera,
        grid_mm,
        bounds,
        paper_bounds,
        color,
        page_w,
        page_h,
        style,
        device_px_per_logical_px,
    );
}

/// A grid line between two screen points, snapped to whole device pixels by
/// the wires' own hairline snap (#1887, #1902): a thin line becomes one
/// whole device pixel wide on a pixel row, so the grid stays crisp under
/// multisampling too. Logical pixels in and out.
pub(crate) fn snapped_grid_line(
    from: Point,
    to: Point,
    width: f32,
    device_px_per_logical_px: f32,
) -> (Point, Point, f32) {
    let (p0, p1, width) =
        crate::scene_line_snap::snap_hairline(from, to, width, device_px_per_logical_px);
    // Only the line's own row or column snaps: its ends stay where the paper
    // clip put them (whole device pixels already), so no line pokes past the
    // paper edge by the half pixel the hairline snap moves an end.
    if (from.x - to.x).abs() < 1e-3 {
        (Point::new(p0.x, from.y), Point::new(p1.x, to.y), width)
    } else if (from.y - to.y).abs() < 1e-3 {
        (Point::new(from.x, p0.y), Point::new(to.x, p1.y), width)
    } else {
        (p0, p1, width)
    }
}

/// A grid dot's or cross's centre on the middle of a device pixel: the same
/// hairline snap, applied to a zero-length segment of one device pixel.
pub(crate) fn snapped_grid_centre(centre: Point, device_px_per_logical_px: f32) -> Point {
    let one_device_pixel = 1.0 / sanitised_device_scale(device_px_per_logical_px);
    let (snapped, _, _) = crate::scene_line_snap::snap_hairline(
        centre,
        centre,
        one_device_pixel,
        device_px_per_logical_px,
    );
    snapped
}

/// The device px per logical px, or 1 when the value is unusable.
fn sanitised_device_scale(device_px_per_logical_px: f32) -> f32 {
    if device_px_per_logical_px.is_finite() && device_px_per_logical_px > 0.0 {
        device_px_per_logical_px
    } else {
        1.0
    }
}

/// The paper rectangle in screen px, shrunk inward to whole device pixels
/// and cut to the canvas, or `None` when nothing of it is visible. Inward
/// rounding keeps a fractional display scale from re-admitting a half pixel
/// of grid outside the paper (#1888, #1916).
pub(crate) fn paper_clip(
    camera: &Camera,
    bounds: iced::Rectangle,
    page_w: f32,
    page_h: f32,
    device_px_per_logical_px: f32,
) -> Option<iced::Rectangle> {
    let tl = camera.world_to_screen(Point::new(0.0, 0.0), bounds);
    let br = camera.world_to_screen(Point::new(page_w, page_h), bounds);
    let k = sanitised_device_scale(device_px_per_logical_px);
    let left = (tl.x.max(0.0) * k).ceil() / k;
    let top = (tl.y.max(0.0) * k).ceil() / k;
    let right = (br.x.min(bounds.width) * k).floor() / k;
    let bottom = (br.y.min(bounds.height) * k).floor() / k;
    (right > left && bottom > top).then(|| {
        iced::Rectangle::new(
            Point::new(left, top),
            iced::Size::new(right - left, bottom - top),
        )
    })
}

/// Grid stops `k * step` inside `[lo, hi]`. Multiplying (not accumulating)
/// keeps the stops exact, and the range test keeps the last one on the paper
/// instead of rounding the extent up to the next whole step (#1940).
pub(crate) fn axis_stops(lo: f32, hi: f32, step: f32) -> impl Iterator<Item = f32> {
    let eps = 1e-4;
    let (first, last) = if step > 0.0 {
        (
            (lo / step - eps).ceil() as i64,
            (hi / step + eps).floor() as i64,
        )
    } else {
        (1, 0)
    };
    (first..=last).map(move |k| k as f32 * step)
}

#[expect(
    clippy::too_many_arguments,
    reason = "the draw inputs of `draw_grid`, with the paper already clipped to whole device pixels"
)]
fn draw_grid_within<R: GeometryRenderer>(
    frame: &mut canvas::Frame<R>,
    camera: &Camera,
    grid_mm: f32,
    bounds: iced::Rectangle,
    paper_bounds: iced::Rectangle,
    color: Color,
    page_w: f32,
    page_h: f32,
    style: GridStyle,
    device_px_per_logical_px: f32,
) {
    let dpr = device_px_per_logical_px;
    if grid_mm <= 0.0 || camera.scale <= 0.0 {
        return;
    }

    // --- 1. Pick adaptive minor step (5^k * grid_mm), aiming for ≥ 6 px. ---
    const MIN_PX: f32 = 6.0;
    let Some(minor_mm) = adaptive_pitch_mm(grid_mm, camera.scale, MIN_PX) else {
        return;
    };
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

    // Safety cap (adaptive step makes this very rare, but keep it).
    let cols = ((wx_max - wx_min) / minor_mm) as i32 + 2;
    let rows = ((wy_max - wy_min) / minor_mm) as i32 + 2;
    if (cols as i64) * (rows as i64) > 40_000 {
        return;
    }

    // Half the thin stroke width, so a cross arm's end stays inside the paper.
    const GLYPH_EDGE_SLACK: f32 = MINOR_GRID_LINE_PX / 2.0;

    // --- 3. Minor dots. Radius scales with screen step, slightly. ---
    let dot_radius = (minor_screen * 0.06).clamp(0.5, 1.6);

    // Small-cross arm length in screen pixels (Standard-style "+").
    let cross_arm = (minor_screen * 0.18).clamp(1.5, 4.0);
    let minor_stroke = canvas::Stroke::default()
        .with_color(dot_color)
        .with_width(MINOR_GRID_LINE_PX);

    // For Lines style we draw full minor grid lines instead of per-cell
    // glyphs and skip the per-point loop below entirely.
    if matches!(style, GridStyle::Lines) {
        let minor_half = MINOR_GRID_LINE_PX / 2.0;
        let (line_x_left, line_x_right) = (paper_bounds.x, paper_bounds.x + paper_bounds.width);
        let (line_y_top, line_y_bot) = (paper_bounds.y, paper_bounds.y + paper_bounds.height);
        for wx in axis_stops(wx_min, wx_max, minor_mm) {
            let sx = camera.world_to_screen(Point::new(wx, 0.0), bounds).x;
            if sx - minor_half >= line_x_left && sx + minor_half <= line_x_right {
                let (p0, p1, width) = snapped_grid_line(
                    Point::new(sx, line_y_top),
                    Point::new(sx, line_y_bot),
                    MINOR_GRID_LINE_PX,
                    dpr,
                );
                frame.stroke(&canvas::Path::line(p0, p1), minor_stroke.with_width(width));
            }
        }
        for wy in axis_stops(wy_min, wy_max, minor_mm) {
            let sy = camera.world_to_screen(Point::new(0.0, wy), bounds).y;
            if sy - minor_half >= line_y_top && sy + minor_half <= line_y_bot {
                let (p0, p1, width) = snapped_grid_line(
                    Point::new(line_x_left, sy),
                    Point::new(line_x_right, sy),
                    MINOR_GRID_LINE_PX,
                    dpr,
                );
                frame.stroke(&canvas::Path::line(p0, p1), minor_stroke.with_width(width));
            }
        }
    } else {
        for wy in axis_stops(wy_min, wy_max, minor_mm) {
            for wx in axis_stops(wx_min, wx_max, minor_mm) {
                let screen =
                    snapped_grid_centre(camera.world_to_screen(Point::new(wx, wy), bounds), dpr);
                // The whole glyph must sit inside the paper bounds, so no
                // renderer can draw it over the canvas background.
                let reach = match style {
                    GridStyle::SmallCrosses => cross_arm,
                    _ => dot_radius,
                } + GLYPH_EDGE_SLACK;
                if screen.x - reach < paper_bounds.x
                    || screen.x + reach > paper_bounds.x + paper_bounds.width
                    || screen.y - reach < paper_bounds.y
                    || screen.y + reach > paper_bounds.y + paper_bounds.height
                {
                    continue;
                }
                match style {
                    GridStyle::Dots => fill_grid_dot(frame, screen, dot_radius, dot_color),
                    GridStyle::SmallCrosses => fill_grid_cross(
                        frame,
                        screen,
                        cross_arm,
                        MINOR_GRID_LINE_PX,
                        dpr,
                        dot_color,
                    ),
                    // `Lines` took the branch above.
                    GridStyle::Lines => unreachable!(),
                }
            }
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
        .with_width(MAJOR_GRID_LINE_PX);

    let (line_x_left, line_x_right) = (paper_bounds.x, paper_bounds.x + paper_bounds.width);
    let (line_y_top, line_y_bot) = (paper_bounds.y, paper_bounds.y + paper_bounds.height);
    let major_half = MAJOR_GRID_LINE_PX / 2.0;

    for mx in axis_stops(wx_min, wx_max, major_mm) {
        let sx = camera.world_to_screen(Point::new(mx, 0.0), bounds).x;
        if sx - major_half >= line_x_left && sx + major_half <= line_x_right {
            let (p0, p1, width) = snapped_grid_line(
                Point::new(sx, line_y_top),
                Point::new(sx, line_y_bot),
                MAJOR_GRID_LINE_PX,
                dpr,
            );
            frame.stroke(&canvas::Path::line(p0, p1), stroke.with_width(width));
        }
    }

    for my in axis_stops(wy_min, wy_max, major_mm) {
        let sy = camera.world_to_screen(Point::new(0.0, my), bounds).y;
        if sy - major_half >= line_y_top && sy + major_half <= line_y_bot {
            let (p0, p1, width) = snapped_grid_line(
                Point::new(line_x_left, sy),
                Point::new(line_x_right, sy),
                MAJOR_GRID_LINE_PX,
                dpr,
            );
            frame.stroke(&canvas::Path::line(p0, p1), stroke.with_width(width));
        }
    }
}

#[cfg(test)]
mod snap_tests {
    use super::{MAJOR_GRID_LINE_PX, MINOR_GRID_LINE_PX, snapped_grid_centre, snapped_grid_line};
    use iced::Point;

    fn on_device_pixel_middle(coordinate: f32, dpr: f32) -> bool {
        let device = coordinate * dpr;
        ((device - 0.5) - (device - 0.5).round()).abs() < 1e-3
    }

    /// Grid lines land on whole device pixels, one device pixel wide, at 1x
    /// and 2x, wherever the camera puts them (no GPU needed).
    #[test]
    fn grid_lines_are_whole_device_pixels_at_one_and_two_times() {
        for dpr in [1.0_f32, 2.0] {
            for phase in 0..23 {
                let c = 100.0 + phase as f32 * 0.137;
                for width in [MINOR_GRID_LINE_PX, MAJOR_GRID_LINE_PX] {
                    let (p0, p1, w) =
                        snapped_grid_line(Point::new(c, 10.0), Point::new(c, 400.0), width, dpr);
                    assert!(on_device_pixel_middle(p0.x, dpr), "{dpr}x vertical at {c}");
                    assert_eq!(p0.x, p1.x);
                    assert!((w * dpr - 1.0).abs() < 1e-4, "{dpr}x width {w}");
                    let (q0, _, _) =
                        snapped_grid_line(Point::new(10.0, c), Point::new(400.0, c), width, dpr);
                    assert!(
                        on_device_pixel_middle(q0.y, dpr),
                        "{dpr}x horizontal at {c}"
                    );
                }
            }
        }
    }

    /// A dot's centre is the middle of a device pixel in both axes.
    #[test]
    fn grid_dots_sit_on_device_pixel_middles() {
        for dpr in [1.0_f32, 2.0] {
            for phase in 0..23 {
                let at = Point::new(50.0 + phase as f32 * 0.213, 80.0 + phase as f32 * 0.171);
                let snapped = snapped_grid_centre(at, dpr);
                assert!(on_device_pixel_middle(snapped.x, dpr), "{dpr}x x {at:?}");
                assert!(on_device_pixel_middle(snapped.y, dpr), "{dpr}x y {at:?}");
            }
        }
    }
}

#[cfg(test)]
mod pitch_tests {
    use super::{adaptive_pitch_mm, axis_stops, paper_clip, pitch_mm_exact};
    use crate::canvas::camera::Camera;
    use iced::{Point, Rectangle, Size};

    /// #1940 - no grid stop lies outside the sheet, for two sheet sizes.
    #[test]
    fn grid_stops_stay_on_the_sheet() {
        // 297 and 420 are not multiples of 1.27, 2.54 or 5.08, which is
        // what rounded the extent up past the paper edge.
        for (w, h) in [(297.0_f32, 210.0_f32), (420.0, 297.0)] {
            for step in [0.635_f32, 1.27, 2.54, 5.08] {
                for v in axis_stops(0.0, w, step) {
                    assert!((0.0..=w).contains(&v), "x {v} outside {w} (step {step})");
                }
                for v in axis_stops(0.0, h, step) {
                    assert!((0.0..=h).contains(&v), "y {v} outside {h} (step {step})");
                }
                assert!(axis_stops(0.0, w, step).count() >= 2);
            }
        }
    }

    /// #1940 - the clip stays inside the paper's screen rect at a fractional
    /// display scale, and lands on whole device pixels.
    #[test]
    fn paper_clip_is_inside_the_paper_at_fractional_scale() {
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(1000.0, 700.0));
        for dpr in [1.0_f32, 1.25, 1.5, 1.75] {
            for (w, h) in [(297.0_f32, 210.0_f32), (420.0, 297.0)] {
                let cam = Camera {
                    offset: Point::new(13.3, 9.7),
                    scale: 2.37,
                };
                let clip = paper_clip(&cam, bounds, w, h, dpr).expect("visible");
                let tl = cam.world_to_screen(Point::new(0.0, 0.0), bounds);
                let br = cam.world_to_screen(Point::new(w, h), bounds);
                assert!(clip.x >= tl.x.max(0.0) - 1e-3 && clip.y >= tl.y.max(0.0) - 1e-3);
                assert!(clip.x + clip.width <= br.x.min(bounds.width) + 1e-3);
                assert!(clip.y + clip.height <= br.y.min(bounds.height) + 1e-3);
                let on_device = |v: f32| ((v * dpr).round() - v * dpr).abs() < 1e-3;
                assert!(
                    on_device(clip.x) && on_device(clip.x + clip.width),
                    "dpr {dpr}"
                );
                assert!(
                    on_device(clip.y) && on_device(clip.y + clip.height),
                    "dpr {dpr} (y)"
                );
            }
        }
    }

    #[test]
    fn default_pcb_zoom_draws_a_multiple_of_the_grid() {
        // 3.0 px/mm, 2.54 mm grid: 7.62 px < 8, so step to 5 x 2.54.
        let p = adaptive_pitch_mm(2.54, 3.0, 8.0).unwrap();
        assert!((p / 2.54 - 5.0).abs() < 1e-4, "{p}");
    }

    #[test]
    fn dense_enough_grid_keeps_its_own_pitch() {
        assert_eq!(adaptive_pitch_mm(1.27, 10.0, 8.0), Some(1.27));
    }

    #[test]
    fn too_dense_even_at_coarsest_level_hides() {
        assert_eq!(adaptive_pitch_mm(0.635, 0.01, 8.0), None);
        assert_eq!(adaptive_pitch_mm(0.0, 3.0, 8.0), None);
    }

    /// #1057 - a snapped point is an exact multiple of the pitch in nm, for
    /// every preset and the issue's extra grids, at large step counts.
    #[test]
    fn snapped_points_are_exact_nm_multiples() {
        let grids: [(f32, i64); 7] = [
            (0.635, 635_000),
            (1.27, 1_270_000),
            (2.54, 2_540_000),
            (5.08, 5_080_000),
            (0.127, 127_000),
            (0.1, 100_000),
            (0.254, 254_000),
        ];
        for (mm, pitch_nm) in grids {
            let g = pitch_mm_exact(mm);
            for steps in [1_i64, 7, 100, 400, 1000, 5000] {
                let x = steps as f64 * g + 0.3 * g;
                let snapped = (x / g).round() * g;
                assert_eq!(
                    (snapped * 1e6).round() as i64,
                    steps * pitch_nm,
                    "{mm} x {steps}"
                );
            }
        }
    }
}
