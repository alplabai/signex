use iced::advanced::text as advanced_text;
use iced::alignment;
use iced::widget::canvas;
use iced::{Color, Point};
use signex_gfx::primitive::arc::Arc;
use signex_gfx::primitive::circle::Circle;
use signex_gfx::primitive::line::LineSegment;
use signex_gfx::primitive::polygon::GpuPolygon;
use signex_gfx::primitive::text::{TextHAlign, TextItem, TextVAlign};
use signex_gfx::scene::{CPU_SCHEMATIC_DRAW_ORDER, Scene, SceneBucket};

#[derive(Debug, Clone, Copy)]
pub struct SceneDrawOptions {
    pub scale_px_per_mm: f32,
    pub min_stroke_px: f32,
    /// Readability limits in logical pixels — a per-surface view decision, so
    /// each replay supplies its own. The mm→em ratio is *not* here: it is one
    /// constant (`signex_gfx::primitive::text::MM_PER_EM`) because it encodes
    /// the model's millimetre contract, not a view preference, and a second
    /// copy is how the replays drifted apart.
    pub text_min_px: f32,
    pub text_max_px: f32,
}

/// Does this world→screen map flip Y?
///
/// Probed rather than declared. Only arcs care, and they care absolutely: a
/// flipping map reflects the plane, so a world angle becomes its negation on
/// screen and a positive world sweep runs backwards. A non-flipping map is a
/// positive similarity and preserves both.
///
/// The two surfaces sharing this replay disagree — the Symbol Editor maps
/// `oy - point[1] * scale` and the schematic maps `y * scale + offset_y` —
/// which is exactly why this is derived from the mapping the caller already
/// passes in rather than taken as a flag beside it. A flag can be set to
/// contradict the transform; that is the bug this fixes (#645), and it went
/// unnoticed for months. A probe cannot disagree with the function it probes.
fn world_is_y_up<F>(world_to_screen: F) -> bool
where
    F: Fn([f32; 2]) -> Point,
{
    let origin = world_to_screen([0.0, 0.0]);
    let up = world_to_screen([0.0, 1.0]);
    // Screen Y grows downward. If advancing world +Y moved us *up* the
    // screen, the map reflects.
    up.y < origin.y
}

/// How an [`signex_gfx::primitive::arc::Arc`] maps onto the screen-space
/// angles `canvas::path::Arc` wants.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ArcScreenSpan {
    /// The arc spans a whole number of turns: draw a circle. Its collapsed
    /// CCW-wraparound sweep would otherwise be a zero-length point.
    FullTurn,
    Span {
        start: f32,
        end: f32,
    },
}

/// Map an arc's world angles onto the screen angles lyon will sweep between.
///
/// Arc angles are world-space radians measured from +X, and the sweep is
/// always this codebase's CCW-wraparound rule — `(end - start).rem_euclid(TAU)`
/// (`signex_gfx::primitive::arc::ccw_wrapped_sweep_rad`, the same rule
/// `arc.wgsl` and the symbol hit-test use) — never the signed difference.
/// lyon's `builder.arc` does not know that convention: it draws
/// `end_angle - start_angle` as a raw signed sweep, so the end angle handed to
/// it has to be derived from the wrapped sweep rather than passed through.
///
/// `world_is_y_up` decides the sign, and getting it wrong mirrors the arc
/// about the horizontal line through its own centre — right radius, right
/// sweep magnitude, wrong bulge direction, endpoints detached from the points
/// the user clicked.
///
/// This used to negate unconditionally. The negation arrived in `19a3d4a1`
/// ("Negate arc angles in scene canvas renderer for screen-space y-flip") on
/// the stated premise that "Arc primitives store world-space radians (y-up
/// convention)" — true of the Symbol Editor, whose map flips Y, and false of
/// the schematic, whose map does not. Both share this replay, so the fix for
/// one silently mirrored the other (#645).
/// [`arc_screen_span`] with the handedness probed from the caller's own
/// world→screen map, the way `draw_arc_bucket` does it.
///
/// For anything drawing an arc outside the scene replay — the in-progress
/// placement preview — so that a preview and the arc it is about to commit
/// cannot disagree about which way the curve bulges.
pub fn arc_screen_span_for<F>(start_angle: f32, end_angle: f32, world_to_screen: F) -> ArcScreenSpan
where
    F: Fn([f32; 2]) -> Point,
{
    arc_screen_span(start_angle, end_angle, world_is_y_up(world_to_screen))
}

pub fn arc_screen_span(start_angle: f32, end_angle: f32, world_is_y_up: bool) -> ArcScreenSpan {
    if signex_gfx::primitive::arc::arc_is_full_turn_rad(start_angle, end_angle) {
        return ArcScreenSpan::FullTurn;
    }

    let sweep = signex_gfx::primitive::arc::ccw_wrapped_sweep_rad(start_angle, end_angle);

    if world_is_y_up {
        // Reflection: screen angle is the negated world angle, so an
        // increasing world sweep runs backwards through screen angles.
        let start = -start_angle;
        ArcScreenSpan::Span {
            start,
            end: start - sweep,
        }
    } else {
        // Positive similarity: angles and sweep direction both survive.
        ArcScreenSpan::Span {
            start: start_angle,
            end: start_angle + sweep,
        }
    }
}

impl SceneDrawOptions {
    fn stroke_px(self, width_mm: f32) -> f32 {
        (width_mm * self.scale_px_per_mm).max(self.min_stroke_px)
    }

    fn radius_px(self, radius_mm: f32) -> f32 {
        (radius_mm * self.scale_px_per_mm).max(0.5)
    }

    fn text_px(self, size_mm: f32) -> f32 {
        signex_gfx::primitive::text::text_px(
            size_mm,
            self.scale_px_per_mm,
            signex_gfx::primitive::text::TextSizePolicy::new(self.text_min_px, self.text_max_px),
        )
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
    // Probed once per bucket from the caller's own mapping, so it cannot
    // contradict it. Two extra calls per frame's arcs.
    let y_up = world_is_y_up(world_to_screen);

    for arc in arcs {
        if !arc.start_angle.is_finite() || !arc.end_angle.is_finite() {
            continue;
        }

        let center = world_to_screen(arc.center);
        let radius = options.radius_px(arc.radius);
        // `canvas::path::Arc` operates in screen space and lyon's
        // `builder.arc` draws `end - start` as a raw signed sweep with no
        // wraparound (iced_graphics 0.14's `Builder::ellipse`), so both the
        // wraparound convention and this surface's Y handedness have to be
        // resolved before it sees an angle. `arc_screen_span` owns that.
        let (start_angle, end_angle) = match arc_screen_span(arc.start_angle, arc.end_angle, y_up) {
            // A full-turn Arc (raw span a nonzero whole number of turns)
            // is a circle, not the degenerate zero-sweep point its
            // collapsed CCW-wraparound sweep would otherwise draw. Reaches
            // an in-memory Arc that bypassed the load-time full-turn-to-
            // Circle migration — e.g. a Properties-panel start_deg/end_deg
            // edit typed against an already-loaded graphic.
            ArcScreenSpan::FullTurn => {
                frame.stroke(
                    &canvas::Path::circle(center, radius),
                    canvas::Stroke::default()
                        .with_width(options.stroke_px(arc.width))
                        .with_color(color_from_rgba(arc.color)),
                );
                continue;
            }
            ArcScreenSpan::Span { start, end } => (start, end),
        };
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
    // Walk the shared `CPU_SCHEMATIC_DRAW_ORDER` so this replay and the GPU
    // `scene_shader` cannot silently drift apart — the `scene::order` parity
    // tests diff the orders. This sequence used to be hardcoded here, which is
    // why `scene::order`'s promise held for the PCB and quietly did not cover
    // the surface #199 is about to move onto the GPU (#645).
    for &bucket in CPU_SCHEMATIC_DRAW_ORDER {
        match bucket {
            SceneBucket::Lines => draw_line_bucket(frame, &scene.lines, world_to_screen, options),
            SceneBucket::Circles => {
                draw_circle_bucket(frame, &scene.circles, world_to_screen, options);
            }
            SceneBucket::Arcs => draw_arc_bucket(frame, &scene.arcs, world_to_screen, options),
            SceneBucket::Polygons => {
                draw_polygon_bucket(frame, &scene.polygons, world_to_screen, options);
            }
            SceneBucket::Texts => draw_text_bucket(frame, &scene.texts, world_to_screen, options),
            SceneBucket::OverlayLines => {
                draw_line_bucket(frame, &scene.overlay_lines, world_to_screen, options);
            }
            SceneBucket::OverlayCircles => {
                draw_circle_bucket(frame, &scene.overlay_circles, world_to_screen, options);
            }
            SceneBucket::OverlayPolygons => {
                draw_polygon_bucket(frame, &scene.overlay_polygons, world_to_screen, options);
            }
            SceneBucket::ErcMarkerLines => {
                draw_line_bucket(frame, &scene.erc_marker_lines, world_to_screen, options);
            }
            SceneBucket::ErcMarkerCircles => {
                draw_circle_bucket(frame, &scene.erc_marker_circles, world_to_screen, options);
            }
            SceneBucket::ErcMarkerPolygons => {
                draw_polygon_bucket(frame, &scene.erc_marker_polygons, world_to_screen, options);
            }
        }
    }
}

#[cfg(test)]
mod arc_span_tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};

    fn span(start: f32, end: f32, y_up: bool) -> (f32, f32) {
        match arc_screen_span(start, end, y_up) {
            ArcScreenSpan::Span { start, end } => (start, end),
            ArcScreenSpan::FullTurn => panic!("expected a partial span"),
        }
    }

    /// Where the drawn arc's own midpoint lands, as a screen angle: lyon
    /// sweeps linearly from `start` to `end`, so it is simply the mean.
    fn drawn_mid(start: f32, end: f32) -> f32 {
        (start + end) / 2.0
    }

    /// The bug this fixes, stated as the property that was violated.
    ///
    /// A `SchDrawing::Arc` is authored as three clicked points, and the stored
    /// angle pair is chosen so the CCW-wrapped span from `start` to `end`
    /// contains the middle one (`schematic_runtime::arc_sweeps_through_mid`).
    /// The schematic's world→screen map does not flip Y, so it preserves
    /// angles — meaning the arc drawn on screen must bulge toward the same
    /// side as the world midpoint. Under the old unconditional negation it
    /// bulged to the opposite side.
    #[test]
    fn a_schematic_arc_bulges_toward_the_point_the_user_clicked() {
        // Clicked start (10,0), mid (7.07,7.07), end (0,10) about (0,0):
        // world angles 0, 45, 90 degrees, so the stored pair is (0, 90).
        let (start, end) = span(0.0, FRAC_PI_2, false);
        let world_mid = FRAC_PI_4;

        assert!(
            (drawn_mid(start, end) - world_mid).abs() < 1e-5,
            "the drawn arc must pass through the clicked midpoint at \
             {world_mid} rad, but its midpoint is at {} rad — an arc mirrored \
             about the horizontal through its own centre",
            drawn_mid(start, end)
        );
    }

    /// The Symbol Editor's map *does* flip Y, so there the negation is
    /// correct and must survive: its screen midpoint is the reflection of the
    /// world one. This is the half that `19a3d4a1` got right.
    #[test]
    fn a_symbol_editor_arc_reflects_because_its_canvas_flips_y() {
        let (start, end) = span(0.0, FRAC_PI_2, true);
        assert!((drawn_mid(start, end) - -FRAC_PI_4).abs() < 1e-5);
    }

    /// The wrapped case the sweep convention exists for: 330° → 30° is a 60°
    /// arc across zero, not a 300° one. Both handedness values must take the
    /// short way round, in opposite directions.
    #[test]
    fn a_wrapped_arc_sweeps_the_short_way_in_both_frames() {
        let (start, end) = span(TAU - PI / 6.0, PI / 6.0, false);
        assert!((end - start - PI / 3.0).abs() < 1e-5, "60 degrees forward");

        let (start, end) = span(TAU - PI / 6.0, PI / 6.0, true);
        assert!((end - start + PI / 3.0).abs() < 1e-5, "60 degrees backward");
    }

    /// A whole-turn span is a circle on both surfaces — its wrapped sweep
    /// collapses to zero, which would otherwise draw nothing at all.
    #[test]
    fn a_full_turn_is_a_circle_regardless_of_handedness() {
        assert_eq!(arc_screen_span(0.0, TAU, false), ArcScreenSpan::FullTurn);
        assert_eq!(arc_screen_span(0.0, TAU, true), ArcScreenSpan::FullTurn);
    }

    /// The schematic's real transform, probed. If someone gives
    /// `ScreenTransform` a Y flip, this fails — which is the point: the
    /// handedness is derived from the map, so the two cannot drift apart.
    #[test]
    fn the_schematic_transform_reads_as_y_down() {
        let transform = crate::schematic_runtime::ScreenTransform {
            offset_x: 17.0,
            offset_y: 23.0,
            scale: 3.0,
        };
        let map = |p: [f32; 2]| transform.world_to_screen((p[0] as f64, p[1] as f64));
        assert!(
            !world_is_y_up(map),
            "the schematic maps `y * scale + offset_y`, so advancing world +Y \
             must move *down* the screen"
        );
    }

    /// The Symbol Editor's real map, in the form its canvas uses.
    #[test]
    fn the_symbol_editor_transform_reads_as_y_up() {
        let (ox, oy, scale) = (17.0_f32, 23.0_f32, 3.0_f32);
        let map = |p: [f32; 2]| Point::new(ox + p[0] * scale, oy - p[1] * scale);
        assert!(
            world_is_y_up(map),
            "the Symbol Editor maps `oy - y * scale`, so advancing world +Y \
             must move *up* the screen"
        );
    }

    /// A degenerate map must not be read as a reflection — `<` and not `<=`
    /// is what keeps a zero-scale frame on the non-flipping branch, where the
    /// angles pass through untouched.
    #[test]
    fn a_degenerate_map_is_not_treated_as_flipped() {
        assert!(!world_is_y_up(|_| Point::new(0.0, 0.0)));
    }

    /// Sweep magnitude is a property of the data, not of the surface — only
    /// its direction may differ.
    #[test]
    fn both_frames_sweep_the_same_magnitude() {
        for (a, b) in [(0.0, FRAC_PI_2), (TAU - PI / 6.0, PI / 6.0), (1.0, 2.5)] {
            let (ds, de) = span(a, b, false);
            let (us, ue) = span(a, b, true);
            assert!(((de - ds).abs() - (ue - us).abs()).abs() < 1e-5);
        }
    }
}
