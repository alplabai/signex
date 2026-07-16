use iced::advanced::text as advanced_text;
use iced::alignment;
use iced::widget::canvas;
use iced::{Color, Point};
use signex_gfx::primitive::arc::Arc;
use signex_gfx::primitive::circle::Circle;
use signex_gfx::primitive::line::LineSegment;
use signex_gfx::primitive::polygon::GpuPolygon;
use signex_gfx::primitive::text::{TextHAlign, TextItem, TextVAlign};
use signex_gfx::scene::Scene;

#[derive(Debug, Clone, Copy)]
pub struct SceneDrawOptions {
    pub scale_px_per_mm: f32,
    pub min_stroke_px: f32,
    pub text_mm_per_em: f32,
    pub text_min_px: f32,
    pub text_max_px: f32,
}

impl SceneDrawOptions {
    fn stroke_px(self, width_mm: f32) -> f32 {
        (width_mm * self.scale_px_per_mm).max(self.min_stroke_px)
    }

    fn radius_px(self, radius_mm: f32) -> f32 {
        (radius_mm * self.scale_px_per_mm).max(0.5)
    }

    fn text_px(self, size_mm: f32) -> f32 {
        let em_mm = size_mm.max(0.1) / self.text_mm_per_em.max(0.01);
        (em_mm * self.scale_px_per_mm).clamp(self.text_min_px, self.text_max_px)
    }
}

fn color_from_rgba(rgba: [f32; 4]) -> Color {
    Color::from_rgba(rgba[0], rgba[1], rgba[2], rgba[3])
}

fn draw_dashed_line(frame: &mut canvas::Frame, p0: Point, p1: Point, width: f32, color: Color) {
    let dx = p1.x - p0.x;
    let dy = p1.y - p0.y;
    let length = (dx * dx + dy * dy).sqrt();
    if length <= 0.0001 {
        return;
    }

    let dash = 8.0;
    let gap = 5.0;
    let ux = dx / length;
    let uy = dy / length;
    let mut dist = 0.0;

    while dist < length {
        let seg_end = (dist + dash).min(length);
        let sp = Point::new(p0.x + ux * dist, p0.y + uy * dist);
        let ep = Point::new(p0.x + ux * seg_end, p0.y + uy * seg_end);
        let path = canvas::Path::line(sp, ep);
        frame.stroke(
            &path,
            canvas::Stroke::default()
                .with_width(width)
                .with_color(color),
        );
        dist += dash + gap;
    }
}

fn draw_line_bucket<F>(
    frame: &mut canvas::Frame,
    lines: &[LineSegment],
    world_to_screen: F,
    options: SceneDrawOptions,
) where
    F: Fn([f32; 2]) -> Point + Copy,
{
    for line in lines {
        let p0 = world_to_screen(line.p0);
        let p1 = world_to_screen(line.p1);
        let width = options.stroke_px(line.width);
        let color = color_from_rgba(line.color);

        if (line.style & 1) == 1 {
            draw_dashed_line(frame, p0, p1, width, color);
        } else {
            let path = canvas::Path::line(p0, p1);
            frame.stroke(
                &path,
                canvas::Stroke::default()
                    .with_width(width)
                    .with_color(color)
                    .with_line_cap(canvas::LineCap::Round),
            );
        }
    }
}

fn draw_circle_bucket<F>(
    frame: &mut canvas::Frame,
    circles: &[Circle],
    world_to_screen: F,
    options: SceneDrawOptions,
) where
    F: Fn([f32; 2]) -> Point + Copy,
{
    for circle in circles {
        let center = world_to_screen(circle.center);
        let radius = options.radius_px(circle.radius);
        let path = canvas::Path::circle(center, radius);
        let color = color_from_rgba(circle.color);

        if circle.stroke_width <= 0.0 {
            frame.fill(&path, color);
        } else {
            frame.stroke(
                &path,
                canvas::Stroke::default()
                    .with_width(options.stroke_px(circle.stroke_width))
                    .with_color(color),
            );
        }
    }
}

fn draw_arc_bucket<F>(
    frame: &mut canvas::Frame,
    arcs: &[Arc],
    world_to_screen: F,
    options: SceneDrawOptions,
) where
    F: Fn([f32; 2]) -> Point + Copy,
{
    for arc in arcs {
        if !arc.start_angle.is_finite() || !arc.end_angle.is_finite() {
            continue;
        }

        let center = world_to_screen(arc.center);
        let radius = options.radius_px(arc.radius);
        // canvas::path::Arc operates in screen space (y-down). Arc angles are
        // stored as world-space radians (y-up) using this codebase's
        // CCW-wraparound sweep convention (see
        // `signex_gfx::primitive::arc::ccw_wrapped_sweep_rad`'s doc
        // comment — same rule as `arc.wgsl`'s `sdf_arc` and
        // `hit_test.rs`'s `Arc` arm): `start..end` always sweeps
        // counter-clockwise from `start`, wrapping through a full turn
        // when `end < start`, never the signed `end - start` delta.
        //
        // iced/lyon's `builder.arc` does NOT know this convention — it
        // draws `end_angle - start_angle` as a raw signed sweep with no
        // wraparound (iced_graphics 0.14's `Builder::ellipse`). Feeding
        // it the negated world angles directly (as this used to do)
        // only happened to match for already-non-wrapped arcs
        // (`start <= end`, where the raw difference already equals the
        // CCW-wraparound sweep); any arc stored in wrapped form
        // (`end < start`, e.g. after a 0°-crossing rotation, or a
        // placement drag before the endpoint-swap fix in
        // `symbol::updates::apply_symbol_primitive_edit`'s `AddArc`
        // handler) drew as the wrong complement — the opposite arc
        // from the one hit-test and the GPU shader respond on.
        //
        // The fix: derive `end_angle` for the builder from the
        // wraparound sweep instead of trusting the stored `end_angle`
        // directly. `start_angle` still negates exactly as before
        // (screen_angle = -world_angle correctly places the start
        // point); `end_angle` is `start_angle - sweep` — subtracting
        // because negating a CCW (increasing) world angle produces a
        // DECREASING screen angle, so a positive CCW world sweep
        // becomes a negative delta in screen-angle space. For
        // already-non-wrapped arcs this reduces to exactly `-arc.
        // end_angle` (the previous, correct behaviour is unchanged);
        // only wrapped arcs draw differently now — correctly.
        let start_angle = -arc.start_angle;
        let sweep =
            signex_gfx::primitive::arc::ccw_wrapped_sweep_rad(arc.start_angle, arc.end_angle);
        // Defensive belt: `SymbolFile::from_toml_str` migrates a
        // stored full-turn Arc (a raw span that's an exact multiple
        // of 360°) into a Circle on load, but an in-memory Arc that
        // bypasses that load-time migration — e.g. a Properties-panel
        // start_deg/end_deg edit typed directly against an
        // already-loaded graphic — can still reach this draw arm with
        // a zero CCW-wraparound sweep and a genuinely different raw
        // pair. A 360° arc IS a circle; draw that instead of nothing.
        let raw_span = arc.end_angle - arc.start_angle;
        if is_full_turn_arc(sweep, raw_span) {
            frame.stroke(
                &canvas::Path::circle(center, radius),
                canvas::Stroke::default()
                    .with_width(options.stroke_px(arc.width))
                    .with_color(color_from_rgba(arc.color)),
            );
            continue;
        }
        let end_angle = start_angle - sweep;
        let path = canvas::Path::new(|builder| {
            builder.arc(canvas::path::Arc {
                center,
                radius,
                start_angle: iced::Radians(start_angle),
                end_angle: iced::Radians(end_angle),
            });
        });
        frame.stroke(
            &path,
            canvas::Stroke::default()
                .with_width(options.stroke_px(arc.width))
                .with_color(color_from_rgba(arc.color)),
        );
    }
}

/// `true` when a `signex_gfx::primitive::arc::Arc`'s CCW-wraparound
/// `sweep` (from `ccw_wrapped_sweep_rad`) has collapsed to (near)
/// zero despite its raw stored angles genuinely differing —
/// `draw_arc_bucket`'s full-turn defensive belt for an Arc that
/// bypassed `SymbolFile::from_toml_str`'s load-time full-turn-to-
/// Circle migration (e.g. an in-memory Properties-panel edit).
/// Excludes a genuine zero-sweep degenerate point-arc (`raw_span`
/// also ~0), which stays invisible on purpose.
fn is_full_turn_arc(sweep: f32, raw_span: f32) -> bool {
    sweep.abs() < 1e-4 && raw_span.abs() > 1e-4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_turn_arc_is_detected_when_sweep_collapses_to_zero() {
        // A 0 -> 360° stored pair: CCW-wraparound sweep is exactly
        // zero, but the raw span (a full turn, in radians) is not.
        let sweep = signex_gfx::primitive::arc::ccw_wrapped_sweep_rad(0.0, std::f32::consts::TAU);
        assert!(is_full_turn_arc(sweep, std::f32::consts::TAU));
    }

    #[test]
    fn genuine_zero_sweep_point_arc_is_not_a_full_turn() {
        // start == end: both the sweep and the raw span are exactly
        // zero — a real degenerate point-arc, not a full circle.
        assert!(!is_full_turn_arc(0.0, 0.0));
    }

    #[test]
    fn ordinary_arc_is_not_a_full_turn() {
        let sweep = signex_gfx::primitive::arc::ccw_wrapped_sweep_rad(0.0, 90f32.to_radians());
        assert!(!is_full_turn_arc(sweep, 90f32.to_radians()));
    }
}

fn draw_polygon_bucket<F>(
    frame: &mut canvas::Frame,
    polygons: &[GpuPolygon],
    world_to_screen: F,
    options: SceneDrawOptions,
) where
    F: Fn([f32; 2]) -> Point + Copy,
{
    for polygon in polygons {
        if polygon.vertices.len() < 3 {
            continue;
        }

        let points: Vec<Point> = polygon
            .vertices
            .iter()
            .map(|vertex| world_to_screen(*vertex))
            .collect();

        let path = canvas::Path::new(|builder| {
            builder.move_to(points[0]);
            for point in &points[1..] {
                builder.line_to(*point);
            }
            builder.close();
        });

        frame.fill(&path, color_from_rgba(polygon.fill_color));

        if let Some(stroke_color) = polygon.stroke_color {
            frame.stroke(
                &path,
                canvas::Stroke::default()
                    .with_width(options.stroke_px(polygon.stroke_width))
                    .with_color(color_from_rgba(stroke_color))
                    .with_line_join(canvas::LineJoin::Round),
            );
        }
    }
}

fn to_text_h_align(align: TextHAlign) -> advanced_text::Alignment {
    match align {
        TextHAlign::Left => advanced_text::Alignment::Left,
        TextHAlign::Center => advanced_text::Alignment::Center,
        TextHAlign::Right => advanced_text::Alignment::Right,
    }
}

fn to_text_v_align(align: TextVAlign) -> alignment::Vertical {
    match align {
        TextVAlign::Top => alignment::Vertical::Top,
        TextVAlign::Center => alignment::Vertical::Center,
        TextVAlign::Bottom => alignment::Vertical::Bottom,
    }
}

fn draw_text_bucket<F>(
    frame: &mut canvas::Frame,
    texts: &[TextItem],
    world_to_screen: F,
    options: SceneDrawOptions,
) where
    F: Fn([f32; 2]) -> Point + Copy,
{
    for text in texts {
        if text.content.is_empty() {
            continue;
        }

        let position = world_to_screen(text.position);
        let draw_text = canvas::Text {
            content: text.content.clone(),
            position: Point::ORIGIN,
            color: color_from_rgba(text.color),
            size: iced::Pixels(options.text_px(text.size_mm)),
            font: crate::render_config::IOSEVKA,
            align_x: to_text_h_align(text.h_align),
            align_y: to_text_v_align(text.v_align),
            ..canvas::Text::default()
        };

        if text.rotation.abs() < f32::EPSILON {
            let mut placed = draw_text;
            placed.position = position;
            frame.fill_text(placed);
            continue;
        }

        frame.with_save(|inner| {
            inner.translate(iced::Vector::new(position.x, position.y));
            inner.rotate(iced::Radians(text.rotation));
            inner.fill_text(draw_text);
        });
    }
}

pub fn draw_scene_with_world_to_screen<F>(
    frame: &mut canvas::Frame,
    scene: &Scene,
    world_to_screen: F,
    options: SceneDrawOptions,
) where
    F: Fn([f32; 2]) -> Point + Copy,
{
    draw_line_bucket(frame, &scene.lines, world_to_screen, options);
    draw_circle_bucket(frame, &scene.circles, world_to_screen, options);
    draw_arc_bucket(frame, &scene.arcs, world_to_screen, options);
    draw_polygon_bucket(frame, &scene.polygons, world_to_screen, options);
    draw_text_bucket(frame, &scene.texts, world_to_screen, options);

    draw_line_bucket(frame, &scene.overlay_lines, world_to_screen, options);
    draw_circle_bucket(frame, &scene.overlay_circles, world_to_screen, options);
    draw_polygon_bucket(frame, &scene.overlay_polygons, world_to_screen, options);

    draw_line_bucket(frame, &scene.erc_marker_lines, world_to_screen, options);
    draw_circle_bucket(frame, &scene.erc_marker_circles, world_to_screen, options);
    draw_polygon_bucket(frame, &scene.erc_marker_polygons, world_to_screen, options);
}
