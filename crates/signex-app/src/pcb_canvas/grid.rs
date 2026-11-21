//! The board backdrop behind the PCB canvas (#2351): a mid-grey workspace,
//! the board area in the dark board colour, the grid on the board area only,
//! and the origin marker. Grid lines sit on multiples of the pitch from the
//! board origin, where placement snap steps from, so on a turned view
//! (#2316) they turn with the board. Every fifth line from the origin is a
//! brighter major line; when the fine lines get too close they fade out and
//! the grid steps to the next coarser level, so a zoomed-out view never
//! turns into noise. Colours come from the board palette.

use iced::widget::canvas::path::Builder;
use iced::widget::canvas::{Frame, Path, Stroke};
use iced::{Color, Point, Size};
use signex_types::theme::CanvasColors;

use super::camera::BoardCamera;

/// Narrowest on-screen pitch of the fine grid; a denser grid steps up a level.
pub(super) const GRID_MIN_PX: f32 = 8.0;
/// Fine lines fade in from [`GRID_MIN_PX`] up to this pitch, so a level step
/// does not pop.
pub(super) const GRID_FADE_PX: f32 = 24.0;
/// Every `MAJOR_EVERY`th line is a major line; also the factor between levels.
pub(super) const MAJOR_EVERY: i64 = 5;
/// Coarsest level tried: `grid * 5^11`, far past any zoom-out.
const MAX_LEVELS: u32 = 12;
/// Most lines one axis draws; a guard, the pitch floor keeps it far lower.
const MAX_LINES_PER_AXIS: i64 = 4096;

/// The dark board region of a board with no outline yet, in board mm: the
/// model's empty board area (`signex_types::pcb_origin::EMPTY_BOARD_AREA`).
pub(super) const EMPTY_BOARD_AREA: [[f32; 2]; 4] = {
    let (lo, hi) = signex_types::pcb_origin::EMPTY_BOARD_AREA;
    let (x0, y0, x1, y1) = (lo.x as f32, lo.y as f32, hi.x as f32, hi.y as f32);
    [[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
};

const GRID_LINE_PX: f32 = 1.0;
/// Radius of the origin marker's circle; the X spans the same radius.
const ORIGIN_RADIUS_PX: f32 = 7.0;

/// The grid level drawn at a zoom: the fine pitch in board mm and on screen,
/// and how far the fine lines have faded in (0 = hidden, 1 = full).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct GridLevel {
    pub pitch_mm: f32,
    pub step_px: f32,
    pub fade: f32,
}

/// The level for a `grid_mm` pitch at `scale` px/mm: the smallest
/// `grid_mm * 5^k` at least [`GRID_MIN_PX`] apart on screen, so every drawn
/// line is on a real grid point. `None` for an unusable pitch or scale.
pub(super) fn grid_level(grid_mm: f32, scale: f32) -> Option<GridLevel> {
    if !(grid_mm.is_finite() && grid_mm > 0.0 && scale.is_finite() && scale > 0.0) {
        return None;
    }
    let mut pitch_mm = grid_mm;
    for _ in 0..MAX_LEVELS {
        let step_px = pitch_mm * scale;
        if step_px >= GRID_MIN_PX {
            let fade = ((step_px - GRID_MIN_PX) / (GRID_FADE_PX - GRID_MIN_PX)).clamp(0.0, 1.0);
            return Some(GridLevel {
                pitch_mm,
                step_px,
                fade,
            });
        }
        pitch_mm *= MAJOR_EVERY as f32;
    }
    None
}

/// The two grid line colours at a fade.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct GridPaint {
    pub fine: Color,
    pub major: Color,
}

/// Fine lines take the palette's fine colour, faded by `fade`. Major lines
/// go from the fine colour (as the level is about to step, when they become
/// the next level's fine lines) to the palette's major colour.
pub(super) fn grid_paint(colors: &CanvasColors, fade: f32) -> GridPaint {
    let fine = crate::render_config::to_iced(&colors.board_grid_color());
    let major = crate::render_config::to_iced(&colors.board_grid_major_color());
    let t = fade.clamp(0.0, 1.0);
    let mix = |a: f32, b: f32| a + (b - a) * t;
    GridPaint {
        fine: Color {
            a: fine.a * t,
            ..fine
        },
        major: Color {
            r: mix(fine.r, major.r),
            g: mix(fine.g, major.g),
            b: mix(fine.b, major.b),
            a: mix(fine.a, major.a),
        },
    }
}

/// The dark board region: the outline when it is a polygon, else the
/// default area of a board with no outline yet.
pub(super) fn board_region(outline: Option<&[[f32; 2]]>) -> &[[f32; 2]] {
    match outline {
        Some(outline) if outline.len() >= 3 => outline,
        _ => &EMPTY_BOARD_AREA,
    }
}

/// Fill the frame with the workspace, `sheet` (#2374) with the sheet colour
/// and `region` with the board area.
pub(super) fn draw_board_area(
    frame: &mut Frame,
    camera: &BoardCamera,
    size: Size,
    region: &[[f32; 2]],
    sheet: ([f32; 2], [f32; 2]),
    colors: &CanvasColors,
) {
    let workspace = crate::render_config::to_iced(&colors.board_workspace_color());
    frame.fill_rectangle(Point::ORIGIN, size, workspace);
    let sheet_fill = crate::render_config::to_iced(&colors.board_sheet_color());
    let sheet_edge = crate::render_config::to_iced(&colors.board_grid_major_color());
    super::marks::draw_sheet(frame, camera, sheet, sheet_fill, sheet_edge);
    let Some((first, rest)) = region.split_first() else {
        return;
    };
    let screen = |p: &[f32; 2]| camera.world_to_screen(Point::new(p[0], p[1]));
    let area = Path::new(|b| {
        b.move_to(screen(first));
        for p in rest {
            b.line_to(screen(p));
        }
        b.close();
    });
    frame.fill(
        &area,
        crate::render_config::to_iced(&colors.board_area_color()),
    );
}

/// Stroke the grid at `level` inside `region` (board mm), counted from the
/// board origin `origin` (#2384): one path for the fine lines and one for
/// the major lines.
pub(super) fn draw_grid(
    frame: &mut Frame,
    camera: &BoardCamera,
    size: Size,
    region: &[[f32; 2]],
    origin: [f32; 2],
    level: GridLevel,
    paint: GridPaint,
) {
    let Some(view) = camera.visible_world_bounds(size) else {
        return;
    };
    // One pass over the lines feeds both paths.
    let draw_fine = paint.fine.a > 0.0;
    let mut fine = Builder::new();
    let mut major = Builder::new();
    let mut scratch = Vec::with_capacity(8);
    each_segment_from(
        origin,
        region,
        view,
        level.pitch_mm,
        &mut scratch,
        |i, from, to| {
            let path = if i.rem_euclid(MAJOR_EVERY) == 0 {
                &mut major
            } else if draw_fine {
                &mut fine
            } else {
                return;
            };
            path.move_to(camera.world_to_screen(from));
            path.line_to(camera.world_to_screen(to));
        },
    );
    let stroke = |color: Color| Stroke::default().with_color(color).with_width(GRID_LINE_PX);
    if draw_fine {
        frame.stroke(&fine.build(), stroke(paint.fine));
    }
    frame.stroke(&major.build(), stroke(paint.major));
}

/// Mark the board origin `origin` (board mm) with a circle and an X through it.
pub(super) fn draw_origin(
    frame: &mut Frame,
    camera: &BoardCamera,
    size: Size,
    origin: [f32; 2],
    color: Color,
) {
    let at = camera.world_to_screen(Point::new(origin[0], origin[1]));
    let reach = ORIGIN_RADIUS_PX + GRID_LINE_PX;
    if at.x < -reach || at.y < -reach || at.x > size.width + reach || at.y > size.height + reach {
        return;
    }
    stroke_origin_marker(frame, at, color);
}

/// The board origin marker at canvas pixel `at`: a circle with an X through
/// it. The board sketch draws the same one.
pub(crate) fn stroke_origin_marker(frame: &mut Frame, at: Point, color: Color) {
    let arm = ORIGIN_RADIUS_PX * std::f32::consts::FRAC_1_SQRT_2;
    let marker = Path::new(|b| {
        b.circle(at, ORIGIN_RADIUS_PX);
        b.move_to(Point::new(at.x - arm, at.y - arm));
        b.line_to(Point::new(at.x + arm, at.y + arm));
        b.move_to(Point::new(at.x - arm, at.y + arm));
        b.line_to(Point::new(at.x + arm, at.y - arm));
    });
    frame.stroke(
        &marker,
        Stroke::default().with_color(color).with_width(GRID_LINE_PX),
    );
}

/// [`each_segment`] for a grid through `origin` (board mm): `region`, `view`
/// and the segments are all in board mm, and `index` counts from `origin`.
pub(super) fn each_segment_from(
    origin: [f32; 2],
    region: &[[f32; 2]],
    view: ([f32; 2], [f32; 2]),
    pitch_mm: f32,
    scratch: &mut Vec<f32>,
    mut segment: impl FnMut(i64, Point, Point),
) {
    let [ox, oy] = origin;
    let shifted: Vec<[f32; 2]> = region.iter().map(|p| [p[0] - ox, p[1] - oy]).collect();
    let view = (
        [view.0[0] - ox, view.0[1] - oy],
        [view.1[0] - ox, view.1[1] - oy],
    );
    let back = |p: Point| Point::new(p.x + ox, p.y + oy);
    each_segment(&shifted, view, pitch_mm, scratch, |i, from, to| {
        segment(i, back(from), back(to));
    });
}

/// Call `segment(index, from, to)` for every piece of a grid line through
/// (0, 0) inside `region` and the visible `view` box. `index` counts pitches
/// from (0, 0) along the line's axis; `scratch` holds the crossings.
pub(super) fn each_segment(
    region: &[[f32; 2]],
    view: ([f32; 2], [f32; 2]),
    pitch_mm: f32,
    scratch: &mut Vec<f32>,
    mut segment: impl FnMut(i64, Point, Point),
) {
    if region.len() < 3 || pitch_mm.is_nan() || pitch_mm <= 0.0 {
        return;
    }
    let (mut lo, mut hi) = (region[0], region[0]);
    for p in region {
        lo = [lo[0].min(p[0]), lo[1].min(p[1])];
        hi = [hi[0].max(p[0]), hi[1].max(p[1])];
    }
    for axis in 0..2 {
        let other = 1 - axis;
        let from = lo[axis].max(view.0[axis]);
        let to = hi[axis].min(view.1[axis]);
        if from > to {
            continue;
        }
        let first = (from / pitch_mm).ceil() as i64;
        let last = ((to / pitch_mm).floor() as i64).min(first + MAX_LINES_PER_AXIS);
        for i in first..=last {
            let c = i as f32 * pitch_mm;
            crossings(region, axis, c, scratch);
            for pair in scratch.chunks_exact(2) {
                let a = pair[0].max(view.0[other]);
                let b = pair[1].min(view.1[other]);
                if a >= b {
                    continue;
                }
                let point = |t: f32| {
                    if axis == 0 {
                        Point::new(c, t)
                    } else {
                        Point::new(t, c)
                    }
                };
                segment(i, point(a), point(b));
            }
        }
    }
}

/// Where the line `p[axis] == c` crosses the edges of `region`, sorted
/// along the other axis; consecutive pairs bound the inside (even-odd).
fn crossings(region: &[[f32; 2]], axis: usize, c: f32, out: &mut Vec<f32>) {
    out.clear();
    let other = 1 - axis;
    let mut prev = region[region.len() - 1];
    for &p in region {
        if (prev[axis] <= c) != (p[axis] <= c) {
            let t = (c - prev[axis]) / (p[axis] - prev[axis]);
            out.push(prev[other] + t * (p[other] - prev[other]));
        }
        prev = p;
    }
    out.sort_by(f32::total_cmp);
}

#[cfg(test)]
#[path = "grid_tests.rs"]
mod tests;
