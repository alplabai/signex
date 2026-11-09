use crate::polygon_path::polygon_path_and_fill;
use iced::advanced::text as advanced_text;
use iced::alignment;
use iced::widget::canvas;
use iced::{Color, Point};
use signex_gfx::primitive::arc::Arc;
use signex_gfx::primitive::circle::Circle;
use signex_gfx::primitive::line::LineSegment;
use signex_gfx::primitive::ordered::OrderedItem;
use signex_gfx::primitive::polygon::GpuPolygon;
use signex_gfx::primitive::text::{TextHAlign, TextItem, TextRaster, TextVAlign};
use signex_gfx::scene::{CPU_SCHEMATIC_DRAW_ORDER, Scene, SceneBucket};

/// The largest text, in logical pixels, iced draws from the glyph atlas; past
/// it iced draws vector outlines. The one atlas limit: signex reads iced's.
pub(crate) const ATLAS_TEXT_LIMIT_PX: f32 = iced::advanced::graphics::geometry::MAX_ATLAS_TEXT_SIZE;

#[derive(Debug, Clone, Copy)]
pub struct SceneDrawOptions {
    pub scale_px_per_mm: f32,
    /// Device pixels per logical pixel; the stroke floor and the hairline
    /// snap work in device pixels (#1858).
    pub device_px_per_logical_px: f32,
    /// Readability limits in logical pixels — a per-surface view decision, so
    /// each replay supplies its own. The mm→em ratio is *not* here: it is one
    /// rule (`signex_types::font::mm_per_em`) because it encodes
    /// the model's millimetre contract, not a view preference, and a second
    /// copy is how the replays drifted apart.
    pub text_min_px: f32,
    pub text_max_px: f32,
    pub frame_origin: Point, // where the frame starts, for the oversize cull
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

/// Smallest a circle may render, in logical pixels. Shared with
/// [`crate::scene_shader::SchematicSurface`] so the CPU replay and the GPU path
/// floor radii identically.
pub const MIN_RADIUS_PX: f32 = 0.5;

/// Thinnest line the PCB and footprint CPU replays draw, in logical pixels.
/// Shared with [`crate::scene_shader::PcbSurface`] so the canvas and GPU paths
/// floor strokes identically.
pub const MIN_STROKE_PX: f32 = 0.5;

/// Clamp a screen-space stroke width to [`MIN_STROKE_PX`] so hairlines stay visible.
pub fn min_stroke_px(width_px: f32) -> f32 {
    width_px.max(MIN_STROKE_PX)
}

impl SceneDrawOptions {
    /// Pre-snap width in logical pixels a `width_mm` stroke replays at: the
    /// one stroke law, `signex_types::schematic::stroke_px`.
    pub fn stroke_px(self, width_mm: f32) -> f32 {
        signex_types::schematic::stroke_px(
            width_mm,
            self.scale_px_per_mm,
            self.device_px_per_logical_px,
        )
    }

    /// [`Self::stroke_px`] snapped to whole device pixels, for circle and arc
    /// rings: the width a wire of the same mm width gets.
    fn ring_stroke_px(self, width_mm: f32) -> f32 {
        crate::scene_line_snap::snap_stroke_width(
            self.stroke_px(width_mm),
            self.device_px_per_logical_px,
        )
    }

    fn radius_px(self, radius_mm: f32) -> f32 {
        (radius_mm * self.scale_px_per_mm).max(MIN_RADIUS_PX)
    }

    /// How `text` draws at this scale and display (the one rule the GPU
    /// glyph pipeline reads as well).
    pub(crate) fn text_raster(self, text: &TextItem) -> TextRaster {
        let dpr = self.device_px_per_logical_px;
        signex_gfx::primitive::text::text_raster(
            self.text_px(text.size_mm, text.family.as_deref()) * dpr,
            ATLAS_TEXT_LIMIT_PX * dpr,
        )
    }

    #[cfg(test)]
    pub(crate) fn text_px_for_test(self, size_mm: f32) -> f32 {
        self.text_px(size_mm, None)
    }

    pub(crate) fn text_px(self, size_mm: f32, family: Option<&str>) -> f32 {
        signex_gfx::primitive::text::text_px_in(
            size_mm,
            self.scale_px_per_mm,
            signex_gfx::primitive::text::TextSizePolicy::new(self.text_min_px, self.text_max_px),
            family,
        )
    }
}

pub(crate) fn color_from_rgba(rgba: [f32; 4]) -> Color {
    Color::from_rgba(rgba[0], rgba[1], rgba[2], rgba[3])
}

pub(crate) fn draw_dashed_line(
    frame: &mut canvas::Frame,
    p0: Point,
    p1: Point,
    width: f32,
    color: Color,
    (dash, gap): (f32, f32),
) {
    let dx = p1.x - p0.x;
    let dy = p1.y - p0.y;
    let length = (dx * dx + dy * dy).sqrt();
    if length <= 0.0001 {
        return;
    }

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

/// Replay the ordered drawings one item at a time, so list order is the paint
/// order across kinds (#1579). Each item goes through its kind's bucket drawer
/// as a one-element slice, which keeps the per-kind look identical.
fn draw_drawings<F>(
    frame: &mut canvas::Frame,
    drawings: &[OrderedItem],
    world_to_screen: F,
    options: SceneDrawOptions,
) where
    F: Fn([f32; 2]) -> Point + Copy,
{
    use std::slice::from_ref;
    for item in drawings {
        match item {
            OrderedItem::Line(line) => {
                draw_line_bucket(frame, from_ref(line), world_to_screen, options);
            }
            OrderedItem::Circle(circle) => {
                draw_circle_bucket(frame, from_ref(circle), world_to_screen, options);
            }
            OrderedItem::Arc(arc) => {
                draw_arc_bucket(frame, from_ref(arc), world_to_screen, options);
            }
            OrderedItem::Polygon(polygon) => {
                draw_polygon_bucket(frame, from_ref(polygon), world_to_screen, options);
            }
        }
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
        let (p0, p1, width) = line_geometry(line, world_to_screen, options);
        let color = color_from_rgba(line.color);

        if let Some(pattern) = line.pattern_px() {
            draw_dashed_line(frame, p0, p1, width, color, pattern);
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

/// Screen endpoints and stroke width of `line`, solid or dashed, with an
/// axis-aligned hairline snapped to whole pixels so its thickness ignores
/// orientation (#1654; see `scene_line_snap`).
fn line_geometry<F>(
    line: &LineSegment,
    world_to_screen: F,
    options: SceneDrawOptions,
) -> (Point, Point, f32)
where
    F: Fn([f32; 2]) -> Point,
{
    let p0 = world_to_screen(line.p0);
    let p1 = world_to_screen(line.p1);
    crate::scene_line_snap::snap_hairline(
        p0,
        p1,
        options.stroke_px(line.width),
        options.device_px_per_logical_px,
    )
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
        let (center, radius, ring_px) = ring_geometry(
            circle.center,
            circle.radius,
            circle.stroke_width,
            world_to_screen,
            options,
        );
        let path = canvas::Path::circle(center, radius);
        let color = color_from_rgba(circle.color);

        if circle.stroke_width <= 0.0 {
            frame.fill(&path, color);
        } else {
            frame.stroke(
                &path,
                canvas::Stroke::default()
                    .with_width(ring_px)
                    .with_color(color),
            );
        }
    }
}

/// Screen centre, radius and stroke width of a circle or arc ring: the width
/// is snapped to whole device pixels like a wire's (#1888).
fn ring_geometry<F>(
    center: [f32; 2],
    radius_mm: f32,
    width_mm: f32,
    world_to_screen: F,
    options: SceneDrawOptions,
) -> (Point, f32, f32)
where
    F: Fn([f32; 2]) -> Point,
{
    (
        world_to_screen(center),
        options.radius_px(radius_mm),
        options.ring_stroke_px(width_mm),
    )
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

        let (center, radius, ring_px) =
            ring_geometry(arc.center, arc.radius, arc.width, world_to_screen, options);
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
                        .with_width(ring_px)
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
                .with_width(ring_px)
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

        let (path, fill) = polygon_path_and_fill(
            polygon,
            world_to_screen,
            color_from_rgba(polygon.fill_color),
        );
        frame.fill(&path, fill);

        if let Some(stroke_color) = polygon.stroke_color {
            let (stroke_path, width) = polygon_stroke_geometry(polygon, world_to_screen, options);
            frame.stroke(
                &stroke_path,
                canvas::Stroke::default()
                    .with_width(width)
                    .with_color(color_from_rgba(stroke_color))
                    .with_line_join(canvas::LineJoin::Round),
            );
        }
    }
}

/// Outline path and width of `polygon`: each ring (hole rings too) snapped
/// like a wire when it is rectilinear, so a rectangle edge gets the pixel
/// width a wire of the same mm width gets (#1888; see `scene_line_snap`).
fn polygon_stroke_geometry<F>(
    polygon: &GpuPolygon,
    world_to_screen: F,
    options: SceneDrawOptions,
) -> (canvas::Path, f32)
where
    F: Fn([f32; 2]) -> Point + Copy,
{
    let width_px = options.stroke_px(polygon.stroke_width);
    let rings = std::iter::once(&polygon.vertices)
        .chain(polygon.holes.iter())
        .filter(|ring| ring.len() >= 3)
        .map(|ring| {
            let screen: Vec<Point> = ring.iter().map(|v| world_to_screen(*v)).collect();
            crate::scene_line_snap::snap_outline(
                &screen,
                width_px,
                options.device_px_per_logical_px,
            )
        })
        .collect::<Vec<_>>();
    let width = rings.first().map_or(width_px, |(_, w)| *w);
    let path = canvas::Path::new(|builder| {
        for (points, _) in &rings {
            if let Some((first, rest)) = points.split_first() {
                builder.move_to(*first);
                rest.iter().for_each(|p| builder.line_to(*p));
                builder.close();
            }
        }
    });
    (path, width)
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

/// The canvas face for one item. `bold` used to be ignored here while the GPU
/// pipeline honoured it, so the same item rendered at two weights (#645).
/// `italic` maps the same way on both sides; it needs the italic Iosevka
/// faces in `fonts::bundled`, or the request leaves the family (#654).
fn canvas_font(text: &TextItem) -> iced::Font {
    // `family_or` is the same call the GPU pipeline's `attrs_for_item` makes,
    // so an item's own family reaches both replays through one rule.
    let family = text.family_or(crate::render_config::CANVAS_FONT_FAMILY);
    let mut font = if family == crate::render_config::CANVAS_FONT_FAMILY {
        crate::render_config::IOSEVKA
    } else {
        crate::fonts::iced_font_for_family(family)
    };
    if text.bold {
        font.weight = iced::font::Weight::Bold;
    }
    if text.italic {
        font.style = iced::font::Style::Italic;
    }
    font
}

/// Translate-then-rotate placement for a rotated label: the text is drawn at
/// the local origin so its h/v alignment pivots around the anchor. `None` for
/// axis-aligned items, which are placed directly.
fn rotated_text_transform(
    text: &TextItem,
    position: Point,
) -> Option<(iced::Vector, iced::Radians)> {
    (!text.is_axis_aligned()).then(|| {
        (
            iced::Vector::new(position.x, position.y),
            iced::Radians(text.rotation),
        )
    })
}

fn draw_text_bucket<F>(
    frame: &mut canvas::Frame,
    texts: &[TextItem],
    world_to_screen: F,
    options: SceneDrawOptions,
) where
    F: Fn([f32; 2]) -> Point + Copy,
{
    draw_text_items(
        frame,
        text_draw_order(texts, options).into_iter(),
        world_to_screen,
        options,
    );
}

/// `texts` with the atlas-drawn ones first and the outline-drawn ones (rotated,
/// oversize) after, each group in scene order. iced opens a new layer at every
/// switch between the two, cache hit or not (#2128); the GPU mount draws them
/// in this order too.
fn text_draw_order(texts: &[TextItem], options: SceneDrawOptions) -> Vec<&TextItem> {
    let (atlas, outline): (Vec<&TextItem>, Vec<&TextItem>) = texts
        .iter()
        .partition(|text| gpu_stages_text(text, options));
    atlas.into_iter().chain(outline).collect()
}

/// Canvas pass for a scene whose atlas text draws on the GPU: paints only what
/// the glyph atlas cannot, the rotated and the oversize texts
/// ([`cpu_draws_beside_gpu`]), through the same path as the full replay so
/// they match it pixel for pixel.
pub fn draw_canvas_texts<F>(
    frame: &mut canvas::Frame,
    scene: &Scene,
    world_to_screen: F,
    options: SceneDrawOptions,
) where
    F: Fn([f32; 2]) -> Point + Copy,
{
    // Past the atlas ceiling only the canvas draws text (iced turns it into
    // vector outlines), so a GPU-mounted sheet hands it to this overlay too.
    let cpu_texts = scene
        .texts
        .iter()
        .filter(|text| cpu_draws_beside_gpu(text, options));
    draw_text_items(frame, cpu_texts, world_to_screen, options);
}

/// Whether signex's GPU glyph pipeline stages `text`: axis-aligned (the atlas
/// cannot rotate) and within the atlas limit.
pub(crate) fn gpu_stages_text(text: &TextItem, options: SceneDrawOptions) -> bool {
    let dpr = options.device_px_per_logical_px;
    text.gpu_stages(
        options.text_px(text.size_mm, text.family.as_deref()) * dpr,
        ATLAS_TEXT_LIMIT_PX * dpr,
    )
}

/// Whether the canvas draws `text` of a GPU-mounted scene: everything that is
/// drawn at all and not staged on the GPU, so each text is drawn exactly once.
pub(crate) fn cpu_draws_beside_gpu(text: &TextItem, options: SceneDrawOptions) -> bool {
    options.text_raster(text) != TextRaster::Skip && !gpu_stages_text(text, options)
}

fn draw_text_items<'a, F>(
    frame: &mut canvas::Frame,
    texts: impl Iterator<Item = &'a TextItem>,
    world_to_screen: F,
    options: SceneDrawOptions,
) where
    F: Fn([f32; 2]) -> Point + Copy,
{
    let texts: Vec<&TextItem> = texts.filter(|text| !text.content.is_empty()).collect();
    // iced draws oversize text as outlines past its own atlas limit; nothing
    // reaches it that cannot be seen, and what does is bounded, the viewport
    // ahead of the pan margin (#2084). Drawing stays in document order.
    let oversize: Vec<_> = texts
        .iter()
        .map(|text| {
            if options.text_raster(text) != TextRaster::Oversize {
                return None;
            }
            let size_px = options.text_px(text.size_mm, text.family.as_deref());
            let position = world_to_screen(text.position);
            let reach = crate::oversize_text::reach(frame, options, text, position, size_px)?;
            Some((text.content.as_str(), reach))
        })
        .collect();
    let admitted = crate::oversize_text::OversizeBudget::default().admit_all(&oversize);
    for (text, admitted) in texts.into_iter().zip(admitted) {
        let size_px = options.text_px(text.size_mm, text.family.as_deref());
        let position = world_to_screen(text.position);
        let content = match options.text_raster(text) {
            TextRaster::Skip => continue,
            TextRaster::Oversize => match admitted {
                Some(content) => content.to_string(),
                None => continue,
            },
            TextRaster::Bitmap => text.content.clone(),
        };
        let draw_text = canvas::Text {
            content,
            position: Point::ORIGIN,
            color: color_from_rgba(text.color),
            size: iced::Pixels(size_px),
            line_height: iced::widget::text::LineHeight::Relative(
                signex_gfx::primitive::text::LINE_HEIGHT_EM,
            ),
            font: canvas_font(text),
            align_x: to_text_h_align(text.h_align),
            align_y: to_text_v_align(text.v_align),
            ..canvas::Text::default()
        };

        match rotated_text_transform(text, position) {
            None => {
                let mut placed = draw_text;
                placed.position = position;
                frame.fill_text(placed);
            }
            Some((translation, angle)) => frame.with_save(|inner| {
                inner.translate(translation);
                inner.rotate(angle);
                inner.fill_text(draw_text);
            }),
        }
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
            SceneBucket::Drawings => {
                draw_drawings(frame, &scene.drawings, world_to_screen, options);
            }
            SceneBucket::DrawingsFront => {
                draw_drawings(frame, &scene.drawings_front, world_to_screen, options);
            }
            SceneBucket::Lines => draw_line_bucket(frame, &scene.lines, world_to_screen, options),
            SceneBucket::Circles => {
                draw_circle_bucket(frame, &scene.circles, world_to_screen, options);
            }
            SceneBucket::Arcs => draw_arc_bucket(frame, &scene.arcs, world_to_screen, options),
            SceneBucket::Polygons => {
                draw_polygon_bucket(frame, &scene.polygons, world_to_screen, options);
            }
            SceneBucket::TopPolygons => {
                draw_polygon_bucket(frame, &scene.top_polygons, world_to_screen, options);
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
            device_px_per_logical_px: signex_types::schematic::UNSCALED_DEVICE_PX_PER_LOGICAL_PX,
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

#[cfg(test)]
mod canvas_font_tests {
    use super::*;
    use crate::fonts::bundled::BUNDLED_FONTS;

    /// Which bundled face `canvas_font` selects for an item, resolved the
    /// way cosmic-text does: only faces whose style equals the request are
    /// candidates, nearest weight wins. `None` means no bundled face has
    /// that style, so the shaper would leave the family for a system face.
    fn bundled_face_of(item: &TextItem) -> Option<String> {
        let mut db = fontdb::Database::new();
        for font in BUNDLED_FONTS {
            db.load_font_data(font.to_vec());
        }
        let font = canvas_font(item);
        let iced::font::Family::Name(family) = font.family else {
            panic!("the canvas font names its family: {font:?}");
        };
        let style = match font.style {
            iced::font::Style::Normal => fontdb::Style::Normal,
            iced::font::Style::Italic => fontdb::Style::Italic,
            iced::font::Style::Oblique => fontdb::Style::Oblique,
        };
        let weight = match font.weight {
            iced::font::Weight::Normal => fontdb::Weight::NORMAL,
            iced::font::Weight::Bold => fontdb::Weight::BOLD,
            other => panic!("canvas_font only picks Normal or Bold, got {other:?}"),
        };
        let id = db.query(&fontdb::Query {
            families: &[fontdb::Family::Name(family)],
            weight,
            stretch: fontdb::Stretch::Normal,
            style,
        })?;
        let face = db.face(id)?;
        (face.style == style).then(|| face.post_script_name.clone())
    }

    fn bundled_face_for(bold: bool, italic: bool) -> Option<String> {
        bundled_face_of(&TextItem {
            content: "R1".to_string(),
            size_mm: 1.0,
            color: [1.0; 4],
            bold,
            italic,
            ..Default::default()
        })
    }

    #[test]
    fn every_bold_and_italic_combination_selects_its_own_bundled_iosevka_face() {
        assert_eq!(bundled_face_for(false, false).as_deref(), Some("Iosevka"));
        assert_eq!(
            bundled_face_for(true, false).as_deref(),
            Some("Iosevka-Bold")
        );
        assert_eq!(
            bundled_face_for(false, true).as_deref(),
            Some("Iosevka-Italic"),
            "an italic item must draw in a bundled italic Iosevka, not upright \
             and not in a system italic"
        );
        assert_eq!(
            bundled_face_for(true, true).as_deref(),
            Some("Iosevka-Bold-Italic")
        );
    }

    /// #510 end to end, from the document to the face: an installed family
    /// reaches the canvas replay (and, through the shared `family_or`, the
    /// GPU pipeline), and a family this machine lacks falls back to the
    /// bundled Iosevka rather than to the sheet default or a system face.
    #[test]
    fn a_text_items_family_survives_the_scene_and_a_missing_one_falls_back_to_iosevka() {
        use signex_gfx::scene::DirtyFlags;
        use signex_renderer::schematic::SchematicRenderer;
        use signex_renderer::schematic::ViewRenderer;
        use signex_types::schematic::{Label, LabelType, Point as SchPoint, SchematicSheet};

        let label = |text: &str, family: Option<&str>| Label {
            font_family: family.map(str::to_string),
            justify: Default::default(),
            justify_v: Default::default(),
            ..Label::new(text, SchPoint { x: 10.0, y: 10.0 }, LabelType::Net)
        };
        // Roboto is bundled, so it is installed on every machine and the
        // result does not depend on the fonts of the machine running this.
        let sheet = SchematicSheet {
            font_family: Some("Roboto".to_string()),
            labels: vec![
                label("INHERITS", None),
                label("OWN", Some("Roboto")),
                label("MISSING", Some("Surely Not Installed 510")),
                label("DEFAULT", Some("Iosevka")),
            ],
            ..Default::default()
        };

        let transform = crate::schematic_runtime::ScreenTransform {
            offset_x: 0.0,
            offset_y: 0.0,
            scale: 4.0,
            device_px_per_logical_px: signex_types::schematic::UNSCALED_DEVICE_PX_PER_LOGICAL_PX,
        };
        let colors = signex_types::theme::canvas_colors(signex_types::theme::ThemeId::GraphiteDark);
        let bounds = iced::Rectangle::new(iced::Point::ORIGIN, iced::Size::new(800.0, 600.0));
        let snapshot = crate::schematic_runtime::build_renderer_snapshot(
            &sheet,
            &transform,
            &colors,
            bounds,
            None,
            Default::default(),
        );
        let mut scene = Scene::default();
        SchematicRenderer::build_scene(
            &snapshot,
            &signex_renderer::theme::ResolvedTheme::from_canvas_colors(colors),
            DirtyFlags::TEXT,
            &mut scene,
        );

        let face = |content: &str| {
            let item = scene
                .texts
                .iter()
                .find(|item| item.content == content)
                .unwrap_or_else(|| panic!("{content} was emitted"));
            // The one rule both replays share.
            let family = item.family_or(crate::render_config::CANVAS_FONT_FAMILY);
            let iced::font::Family::Name(canvas_family) = canvas_font(item).family else {
                panic!("the canvas font names its family");
            };
            assert_eq!(
                canvas_family, family,
                "{content}: canvas and shared rule agree"
            );
            bundled_face_of(item)
        };
        assert_eq!(face("INHERITS").as_deref(), Some("Roboto-Regular"));
        assert_eq!(face("OWN").as_deref(), Some("Roboto-Regular"));
        assert_eq!(
            face("MISSING").as_deref(),
            Some("Iosevka"),
            "a missing family falls back to the bundled default"
        );
        assert_eq!(face("DEFAULT").as_deref(), Some("Iosevka"));
    }
}

#[cfg(test)]
mod rotated_text_tests;

#[cfg(test)]
mod min_stroke_tests {
    use super::*;

    #[test]
    fn min_stroke_px_floors_hairlines_and_keeps_wide_strokes() {
        assert_eq!(min_stroke_px(0.0), MIN_STROKE_PX);
        assert_eq!(min_stroke_px(0.2), 0.5);
        assert_eq!(min_stroke_px(2.5), 2.5);
    }
}

#[cfg(test)]
mod line_geometry_tests {
    use super::*;

    fn options() -> SceneDrawOptions {
        SceneDrawOptions {
            scale_px_per_mm: 12.8,
            device_px_per_logical_px: signex_types::schematic::UNSCALED_DEVICE_PX_PER_LOGICAL_PX,
            text_min_px: 1.0,
            text_max_px: 100.0,
            frame_origin: Point::ORIGIN,
        }
    }

    fn line(p0: [f32; 2], p1: [f32; 2], style: u32) -> LineSegment {
        LineSegment {
            p0,
            p1,
            width: 0.15,
            color: [0.0, 0.0, 0.5, 1.0],
            style,
            _pad: 0,
        }
    }

    /// The replay draws what `line_geometry` returns: a horizontal and a
    /// vertical 0.15 mm wire come out the same whole-pixel width, solid or
    /// dashed. Without the snap they are 1.92 px each and fall on 1 or 2 rows.
    #[test]
    fn a_horizontal_and_a_vertical_wire_get_the_same_whole_pixel_width() {
        let to_screen = |p: [f32; 2]| Point::new(p[0] * 12.8 + 0.37, p[1] * 12.8 + 0.61);
        for style in [0, LineSegment::STYLE_DASHED] {
            let (h0, h1, hw) =
                line_geometry(&line([1.0, 2.0], [9.0, 2.0], style), to_screen, options());
            let (v0, v1, vw) =
                line_geometry(&line([3.0, 1.0], [3.0, 9.0], style), to_screen, options());
            assert_eq!(hw, 2.0, "style {style}");
            assert_eq!(vw, 2.0, "style {style}");
            assert_eq!(h0.y, h1.y);
            assert_eq!(v0.x, v1.x);
            assert_eq!(h0.y.fract(), 0.0, "an even width sits on a pixel edge");
            assert_eq!(v0.x.fract(), 0.0);
        }
    }

    #[test]
    fn a_diagonal_keeps_its_exact_geometry() {
        let to_screen = |p: [f32; 2]| Point::new(p[0] * 12.8, p[1] * 12.8);
        let (p0, p1, w) = line_geometry(&line([0.0, 0.0], [5.0, 3.0], 0), to_screen, options());
        assert_eq!((p0, p1), (Point::new(0.0, 0.0), Point::new(64.0, 38.4)));
        assert!((w - 1.92).abs() < 1e-4);
    }

    fn rect_polygon(width_mm: f32) -> GpuPolygon {
        GpuPolygon {
            vertices: vec![[1.0, 2.0], [9.0, 2.0], [9.0, 6.0], [1.0, 6.0]],
            holes: Vec::new(),
            fill_color: [0.0; 4],
            stroke_color: Some([0.0, 0.0, 0.5, 1.0]),
            stroke_width: width_mm,
        }
    }

    /// A symbol body outline and a wire of the same 0.15 mm law draw at the
    /// same whole-pixel width at 800 % (3.6 px unsnapped, 3 px on the canvas)
    /// and in a 2x window; rings snap the same way (#1888).
    #[test]
    fn outlines_and_rings_get_the_wire_width_at_every_zoom_and_scale() {
        for dpr in [1.0, 2.0] {
            for scale in [3.2, 6.4, 12.8, 25.6, 51.2, 102.4] {
                let options = SceneDrawOptions {
                    scale_px_per_mm: scale,
                    device_px_per_logical_px: dpr,
                    ..options()
                };
                let to_screen = |p: [f32; 2]| Point::new(p[0] * scale + 0.37, p[1] * scale + 0.61);
                let (_, _, wire) =
                    line_geometry(&line([1.0, 2.0], [9.0, 2.0], 0), to_screen, options);
                let polygon = rect_polygon(0.15);
                let (_, outline) = polygon_stroke_geometry(&polygon, to_screen, options);
                assert_eq!(outline, wire, "outline at {scale} px/mm, {dpr}x");
                let (_, _, ring) = ring_geometry([3.0, 3.0], 2.0, 0.15, to_screen, options);
                assert_eq!(ring, wire, "ring at {scale} px/mm, {dpr}x");
            }
        }
    }
}

#[cfg(test)]
mod oversize_text_tests {
    use super::*;

    fn text_item(size_mm: f32, rotation: f32) -> TextItem {
        TextItem {
            content: "R1".into(),
            size_mm,
            rotation,
            ..Default::default()
        }
    }

    /// #2022 - 40 mm text at 96 px/mm (about 5224 px) is drawn at that true
    /// size, not clamped to the atlas ceiling and not culled; being past the
    /// atlas it is the canvas that draws it beside a GPU-mounted scene.
    #[test]
    fn oversize_text_keeps_its_true_size_and_goes_to_the_canvas_beside_the_gpu() {
        let options = SceneDrawOptions {
            scale_px_per_mm: 96.0,
            device_px_per_logical_px: 1.0,
            text_min_px: 0.0,
            text_max_px: f32::INFINITY,
            frame_origin: Point::ORIGIN,
        };
        let big = text_item(40.0, 0.0);
        let px = options.text_px(big.size_mm, None);
        assert!((px - 5224.0).abs() < 5.0, "true size, got {px}");
        assert_eq!(options.text_raster(&big), TextRaster::Oversize);
        assert!(cpu_draws_beside_gpu(&big, options));
        assert!(
            cpu_draws_beside_gpu(&text_item(1.27, 1.0), options),
            "rotated"
        );
        assert!(
            !cpu_draws_beside_gpu(&text_item(1.27, 0.0), options),
            "ordinary"
        );
    }

    /// One atlas limit: iced's own constant, in logical px, times the display
    /// scale for the GPU staging, decides `Oversize`.
    #[test]
    fn oversize_starts_just_past_the_iced_atlas_limit_at_every_display_scale() {
        use iced::advanced::graphics::geometry::MAX_ATLAS_TEXT_SIZE;
        use signex_gfx::primitive::text::text_raster;
        for dpr in [1.0_f32, 2.0, 3.0] {
            let limit = MAX_ATLAS_TEXT_SIZE * dpr;
            assert_eq!(text_raster(limit, limit), TextRaster::Bitmap, "{dpr}x");
            assert_eq!(
                text_raster(limit + 1.0, limit),
                TextRaster::Oversize,
                "{dpr}x"
            );
        }
        assert_eq!(ATLAS_TEXT_LIMIT_PX, MAX_ATLAS_TEXT_SIZE);
    }
}
