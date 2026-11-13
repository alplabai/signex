//! Tessellating the board scene for the CPU mount (#2321).
//!
//! The visible primitives are listed in the draw order, cut into contiguous
//! runs of about equal cost and tessellated on worker threads, one frame per
//! run. The frames are drawn in run order, so the triangles reach the GPU in
//! the same order as from one frame, only in more meshes. Rules:
//! `docs/RENDERING_RULES.md`, "pcb-cpu-content".

use iced::Rectangle;
use iced::advanced::graphics::geometry;
use iced::widget::canvas;
use signex_gfx::primitive::circle::Circle as GfxCircle;
use signex_gfx::primitive::line::LineSegment;
use signex_gfx::primitive::polygon::GpuPolygon;
use signex_gfx::scene::{CPU_PCB_DRAW_ORDER, Scene, SceneBucket};

use super::BoardCamera;
use super::cull::{SceneBounds, WorldRect, visible};
use crate::polygon_path::polygon_path_and_fill;
use crate::renderer_scene_canvas::{
    MIN_RADIUS_PX, color_from_rgba, draw_dashed_line, min_stroke_px,
};

/// Below this much work one frame on the calling thread is faster than
/// starting workers.
const PARALLEL_MIN_COST: usize = 2_000;
/// Most worker frames per build: past this the split costs more than it saves.
const MAX_RUNS: usize = 8;

/// One primitive of the scene, as drawn.
#[derive(Clone, Copy)]
pub(super) enum Item<'a> {
    Line(&'a LineSegment),
    Circle(&'a GfxCircle),
    Polygon(&'a GpuPolygon),
}

impl Item<'_> {
    /// Rough tessellation work, for cutting the list into even runs.
    fn cost(self) -> usize {
        match self {
            Self::Line(_) => 1,
            Self::Circle(_) => 6,
            Self::Polygon(p) => 4 + p.vertices.len() + p.holes.iter().map(Vec::len).sum::<usize>(),
        }
    }
}

/// The primitives of `scene` that touch `view`, in `CPU_PCB_DRAW_ORDER`
/// (the order the GPU `scene_shader` draws too; the `scene::order` parity
/// test diffs the two), and whether every primitive made it.
pub(super) fn visible_items<'a>(
    scene: &'a Scene,
    extents: &'a SceneBounds,
    view: Option<WorldRect>,
) -> (Vec<Item<'a>>, bool) {
    let mut items = Vec::with_capacity(scene.lines.len() + scene.circles.len());
    let mut total = 0;
    for &bucket in CPU_PCB_DRAW_ORDER {
        match bucket {
            SceneBucket::Lines => {
                total += scene.lines.len();
                items.extend(visible(&scene.lines, &extents.lines, view).map(Item::Line));
            }
            SceneBucket::Circles => {
                total += scene.circles.len();
                items.extend(visible(&scene.circles, &extents.circles, view).map(Item::Circle));
            }
            SceneBucket::Polygons => {
                total += scene.polygons.len();
                items.extend(visible(&scene.polygons, &extents.polygons, view).map(Item::Polygon));
            }
            SceneBucket::TopPolygons => {
                total += scene.top_polygons.len();
                items.extend(
                    visible(&scene.top_polygons, &extents.top_polygons, view).map(Item::Polygon),
                );
            }
            SceneBucket::OverlayLines => {
                total += scene.overlay_lines.len();
                items.extend(
                    visible(&scene.overlay_lines, &extents.overlay_lines, view).map(Item::Line),
                );
            }
            SceneBucket::OverlayCircles => {
                total += scene.overlay_circles.len();
                items.extend(
                    visible(&scene.overlay_circles, &extents.overlay_circles, view)
                        .map(Item::Circle),
                );
            }
            SceneBucket::OverlayPolygons => {
                total += scene.overlay_polygons.len();
                items.extend(
                    visible(&scene.overlay_polygons, &extents.overlay_polygons, view)
                        .map(Item::Polygon),
                );
            }
            // The PCB CPU path emits no arc, drawings, text, or ERC buckets.
            // Handled for exhaustiveness so adding a Scene bucket forces a
            // decision here.
            SceneBucket::Arcs
            | SceneBucket::Drawings
            | SceneBucket::DrawingsFront
            | SceneBucket::Texts
            | SceneBucket::ErcMarkerLines
            | SceneBucket::ErcMarkerCircles
            | SceneBucket::ErcMarkerPolygons => {}
        }
    }
    let complete = items.len() == total;
    (items, complete)
}

/// Tessellate `items` in frames clipped to `clip`, in draw order, one frame
/// per run. `workers` caps the runs; they are cut so each has about the same
/// work, and all but the first go to scoped threads.
///
/// Threads need the renderer's frame to be `Send`, which is what the bound
/// says. Should a renderer's frame stop being `Send`, the fallback is
/// `paint_in_order` over the same runs: the same frames and triangles in
/// the same order, on this thread.
pub(super) fn tessellate<R>(
    renderer: &R,
    clip: Rectangle,
    items: &[Item<'_>],
    camera: &BoardCamera,
    workers: usize,
) -> Vec<R::Geometry>
where
    R: geometry::Renderer,
    R::Frame: Send,
{
    let runs = runs(items, workers);
    let Some((first_run, rest_runs)) = runs.split_first() else {
        return Vec::new();
    };
    if rest_runs.is_empty() {
        return paint_in_order(renderer, clip, &runs, camera);
    }
    // Frames come from the renderer, which stays on this thread.
    let mut frames: Vec<canvas::Frame<R>> = runs
        .iter()
        .map(|_| canvas::Frame::<R>::with_bounds(renderer, clip))
        .collect();
    let Some((first_frame, rest_frames)) = frames.split_first_mut() else {
        return Vec::new();
    };
    std::thread::scope(|scope| {
        let workers: Vec<_> = rest_runs
            .iter()
            .zip(rest_frames.iter_mut())
            .map(|(&run, frame)| scope.spawn(move || paint(frame, run, camera)))
            .collect();
        paint(first_frame, first_run, camera);
        for worker in workers {
            if let Err(panic) = worker.join() {
                std::panic::resume_unwind(panic);
            }
        }
    });
    frames
        .into_iter()
        .map(|frame| frame.into_geometry())
        .collect()
}

/// `runs` painted one after the other on this thread, one frame each.
fn paint_in_order<R: geometry::Renderer>(
    renderer: &R,
    clip: Rectangle,
    runs: &[&[Item<'_>]],
    camera: &BoardCamera,
) -> Vec<R::Geometry> {
    runs.iter()
        .map(|run| {
            let mut frame = canvas::Frame::<R>::with_bounds(renderer, clip);
            paint(&mut frame, run, camera);
            frame.into_geometry()
        })
        .collect()
}

/// How many workers a build may use on this machine.
/// Read once: on Linux the query reads affinity and cgroup state.
pub(super) fn available_workers() -> usize {
    static WORKERS: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *WORKERS
        .get_or_init(|| std::thread::available_parallelism().map_or(1, |n| n.get().min(MAX_RUNS)))
}

/// `items` cut into at most `workers` contiguous runs of about equal cost;
/// one run when the work is too small to share.
fn runs<'s, 'a>(items: &'s [Item<'a>], workers: usize) -> Vec<&'s [Item<'a>]> {
    let total: usize = items.iter().map(|item| item.cost()).sum();
    let count = if total < PARALLEL_MIN_COST {
        1
    } else {
        workers.clamp(1, MAX_RUNS)
    };
    let share = total.div_ceil(count).max(1);
    let mut runs = Vec::with_capacity(count);
    let (mut start, mut acc) = (0, 0);
    for (index, item) in items.iter().enumerate() {
        acc += item.cost();
        if acc >= share && runs.len() + 1 < count {
            runs.push(&items[start..=index]);
            (start, acc) = (index + 1, 0);
        }
    }
    runs.push(&items[start..]);
    runs
}

fn paint<R: geometry::Renderer>(
    frame: &mut canvas::Frame<R>,
    items: &[Item<'_>],
    camera: &BoardCamera,
) {
    for item in items {
        match *item {
            Item::Line(line) => draw_line(frame, line, camera),
            Item::Circle(circle) => draw_circle(frame, circle, camera),
            Item::Polygon(polygon) => draw_polygon(frame, polygon, camera),
        }
    }
}

fn to_screen(camera: &BoardCamera, point: [f32; 2]) -> iced::Point {
    camera.world_to_screen(iced::Point::new(point[0], point[1]))
}

fn draw_line<R: geometry::Renderer>(
    frame: &mut canvas::Frame<R>,
    line: &LineSegment,
    camera: &BoardCamera,
) {
    let p0 = to_screen(camera, line.p0);
    let p1 = to_screen(camera, line.p1);
    let width = min_stroke_px(line.width * camera.scale());
    let color = color_from_rgba(line.color);
    if line.is_dashed() {
        draw_dashed_line(frame, p0, p1, width, color, (8.0, 5.0));
    } else {
        frame.stroke(
            &canvas::Path::line(p0, p1),
            canvas::Stroke::default()
                .with_width(width)
                .with_color(color),
        );
    }
}

fn draw_circle<R: geometry::Renderer>(
    frame: &mut canvas::Frame<R>,
    circle: &GfxCircle,
    camera: &BoardCamera,
) {
    let center = to_screen(camera, circle.center);
    let radius = (circle.radius * camera.scale()).max(MIN_RADIUS_PX);
    let color = color_from_rgba(circle.color);
    let path = canvas::Path::circle(center, radius);
    if circle.is_filled() {
        frame.fill(&path, color);
    } else {
        frame.stroke(
            &path,
            canvas::Stroke::default()
                .with_width(min_stroke_px(circle.stroke_width * camera.scale()))
                .with_color(color),
        );
    }
}

fn draw_polygon<R: geometry::Renderer>(
    frame: &mut canvas::Frame<R>,
    polygon: &GpuPolygon,
    camera: &BoardCamera,
) {
    if polygon.vertices.len() < 3 {
        return;
    }
    let (path, fill) = polygon_path_and_fill(
        polygon,
        |vertex| to_screen(camera, vertex),
        color_from_rgba(polygon.fill_color),
    );
    frame.fill(&path, fill);
    if let Some(stroke_color) = polygon.stroke_color {
        frame.stroke(
            &path,
            canvas::Stroke::default()
                .with_width(min_stroke_px(polygon.stroke_width * camera.scale()))
                .with_color(color_from_rgba(stroke_color)),
        );
    }
}
