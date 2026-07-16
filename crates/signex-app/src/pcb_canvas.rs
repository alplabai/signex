use std::sync::Arc;

use iced::event::Event;
use iced::mouse;
use iced::widget::canvas;
use iced::{Color, Rectangle, Renderer, Theme};
use signex_gfx::primitive::circle::Circle as GfxCircle;
use signex_gfx::primitive::line::LineSegment;
use signex_gfx::primitive::polygon::GpuPolygon;
use signex_gfx::scene::{CPU_PCB_DRAW_ORDER, DirtyFlags, Scene, SceneBucket};
use signex_renderer::pcb::{PcbRenderer, PcbSnapshot};
use signex_renderer::schematic::ViewRenderer;
use signex_renderer::theme::ResolvedTheme;

use crate::app::Message;
use crate::canvas::{Camera, CanvasEvent};

#[derive(Debug, Default)]
pub struct PcbCanvasState {
    panning: bool,
    last_pan_pos: Option<iced::Point>,
    pub pending_fit: Option<Rectangle>,
}

pub struct PcbCanvas {
    pub bg_cache: canvas::Cache,
    pub content_cache: canvas::Cache,
    pub content_cache_camera: std::cell::Cell<(f32, f32, f32)>,
    /// Cached GPU scene for the `gpu_render` path. The tessellated geometry is
    /// camera-independent — pan/zoom is applied by the shader's ortho
    /// projection at draw time, not baked into the instance data — so the scene
    /// survives across pan/zoom frames and is rebuilt only when the board
    /// content or theme changes. Invalidated by [`Self::clear_content_cache`]
    /// (the single content-changed signal every mutation site already calls)
    /// and [`Self::set_renderer_snapshot`]. `RefCell` because `gpu_scene` is
    /// called from `view()`, which holds `&self`. Stored behind an `Arc` so the
    /// cache hand-off and the shader primitive share the geometry by refcount
    /// bump — no per-frame deep copy.
    scene_cache: std::cell::RefCell<Option<Arc<Scene>>>,
    /// Monotonic id of the geometry in `scene_cache`, bumped on every
    /// invalidation (`clear_content_cache` / `set_renderer_snapshot`). Handed to
    /// the shader via [`Self::scene_generation`] so the GPU pipeline skips
    /// re-uploading unchanged geometry on pan/zoom. `Cell` for the same
    /// `&self`-in-`view()` reason as `scene_cache`.
    scene_generation: std::cell::Cell<u64>,
    /// The **sole** home of the PCB pan/zoom camera (ADR-0001 §A1: no shadow
    /// copies of widget state). `RefCell` gives interior mutability so
    /// `canvas::Program::update` — which only borrows `&self` — can mutate it,
    /// while `draw` and `view()` read it through the same cell. With no second
    /// copy in the widget `State`, the CPU background/grid, the CPU content
    /// path and the GPU shader program are read off one value and can never
    /// drift out of sync.
    camera: std::cell::RefCell<Camera>,
    pub pending_fit: std::cell::Cell<Option<Rectangle>>,
    pub grid_visible: bool,
    /// Effective (live) PCB GPU-render flag. When true, `draw` skips the CPU
    /// content tessellation (a `shader` stacked above draws it on the GPU) and
    /// `view()` mounts the shader stack. Synced from the Preferences toggle —
    /// updated immediately on draft change for live preview, committed/reverted
    /// on Save/Discard. Mirrors `UiState::pcb_gpu_render` (the saved value).
    pub gpu_render: bool,
    pub theme_bg: Color,
    pub theme_grid: Color,
    pub canvas_colors: signex_types::theme::CanvasColors,
    pub renderer_snapshot: Option<PcbSnapshot>,
    pub visible_grid_mm: f64,
}

impl PcbCanvas {
    pub fn new() -> Self {
        let colors = signex_types::theme::canvas_colors(signex_types::theme::ThemeId::Signex);
        Self {
            bg_cache: canvas::Cache::default(),
            content_cache: canvas::Cache::default(),
            content_cache_camera: std::cell::Cell::new((0.0, 0.0, 1.0)),
            scene_cache: std::cell::RefCell::new(None),
            scene_generation: std::cell::Cell::new(0),
            camera: std::cell::RefCell::new(Camera::default()),
            pending_fit: std::cell::Cell::new(None),
            grid_visible: true,
            gpu_render: false,
            theme_bg: crate::render_config::to_iced(&colors.background),
            theme_grid: crate::render_config::to_iced(&colors.grid),
            canvas_colors: colors,
            renderer_snapshot: None,
            visible_grid_mm: 2.54,
        }
    }

    pub fn active_renderer_snapshot(&self) -> Option<&PcbSnapshot> {
        self.renderer_snapshot.as_ref()
    }

    pub fn set_renderer_snapshot(&mut self, renderer_snapshot: Option<PcbSnapshot>) {
        self.renderer_snapshot = renderer_snapshot;
        // The board changed — the cached GPU scene no longer matches.
        *self.scene_cache.get_mut() = None;
        *self.scene_generation.get_mut() += 1;
    }

    pub fn clear_bg_cache(&mut self) {
        self.bg_cache.clear();
    }

    pub fn clear_content_cache(&mut self) {
        self.content_cache.clear();
        // GPU scene shares this content-changed signal (theme + geometry).
        *self.scene_cache.get_mut() = None;
        *self.scene_generation.get_mut() += 1;
    }

    pub fn fit_to_board(&mut self) {
        if let Some(snapshot) = self.active_renderer_snapshot()
            && let Some(bounds) = renderer_snapshot_bounds(snapshot)
        {
            self.pending_fit.set(Some(Rectangle::new(
                iced::Point::new(bounds.x, bounds.y),
                iced::Size::new(bounds.width, bounds.height),
            )));
        }
    }

    pub fn set_theme_colors(&mut self, bg: Color, grid: Color) {
        self.theme_bg = bg;
        self.theme_grid = grid;
        self.bg_cache.clear();
    }

    /// Current pan/zoom for the GPU path as
    /// `(offset_x_px, offset_y_px, scale_px_per_mm)`, read in `view()` from the
    /// single [`Self::camera`] home to build the shader program. Reads the same
    /// cell the CPU `draw` uses, so the GPU content stays frame-coherent with
    /// the CPU background + grid.
    pub fn live_camera(&self) -> (f32, f32, f32) {
        let camera = self.camera.borrow();
        (camera.offset.x, camera.offset.y, camera.scale)
    }

    /// Build the `signex_gfx` scene for a board snapshot. Shared by the CPU
    /// `draw` path and the GPU [`Self::gpu_scene`] path so both tessellate from
    /// identical instance data.
    fn build_scene(&self, snapshot: &PcbSnapshot) -> Scene {
        let mut scene = Scene::default();
        let theme = ResolvedTheme::from_canvas_colors(self.canvas_colors);
        PcbRenderer::build_scene(
            snapshot,
            &theme,
            DirtyFlags::LINES | DirtyFlags::CIRCLES | DirtyFlags::POLYGONS | DirtyFlags::OVERLAY,
            &mut scene,
        );
        scene
    }

    /// Build the scene for GPU rendering: the same geometry as the CPU path,
    /// but the overlay primitives are folded into the main instance buffers so
    /// the single shader pass draws them last (on top of the content of the
    /// same kind). Returns `None` when no board snapshot is loaded.
    pub fn gpu_scene(&self) -> Option<Arc<Scene>> {
        // Fast path: hand back the shared cached scene without re-tessellating.
        // `borrow().clone()` is an `Arc` refcount bump (on a miss it clones
        // `None`, which is free); the `borrow()` temporary is released before
        // the rebuild below takes a `borrow_mut()`. See [`Self::scene_cache`].
        if let Some(scene) = self.scene_cache.borrow().clone() {
            return Some(scene);
        }

        let snapshot = self.active_renderer_snapshot()?;
        let mut scene = self.build_scene(snapshot);

        let overlay_lines = std::mem::take(&mut scene.overlay_lines);
        let overlay_circles = std::mem::take(&mut scene.overlay_circles);
        let overlay_polygons = std::mem::take(&mut scene.overlay_polygons);
        scene.lines.extend(overlay_lines);
        scene.circles.extend(overlay_circles);
        scene.polygons.extend(overlay_polygons);

        let scene = Arc::new(scene);
        *self.scene_cache.borrow_mut() = Some(Arc::clone(&scene));
        Some(scene)
    }

    /// Generation id of the scene [`Self::gpu_scene`] currently returns. Passed
    /// to the shader so the GPU pipeline can skip re-uploading unchanged
    /// geometry on pan/zoom. Read in `view()` right after `gpu_scene()`, so it
    /// reflects the same geometry that call returned (a rebuild bumps this at
    /// its invalidation site, never mid-`gpu_scene`).
    pub fn scene_generation(&self) -> u64 {
        self.scene_generation.get()
    }
}

fn color_from_rgba(rgba: [f32; 4]) -> Color {
    Color::from_rgba(rgba[0], rgba[1], rgba[2], rgba[3])
}

fn world_to_screen(camera: &Camera, bounds: Rectangle, point: [f32; 2]) -> iced::Point {
    camera.world_to_screen(iced::Point::new(point[0], point[1]), bounds)
}

fn include_world_point(bounds: &mut Option<(f32, f32, f32, f32)>, x: f32, y: f32) {
    if let Some((min_x, min_y, max_x, max_y)) = bounds.as_mut() {
        *min_x = (*min_x).min(x);
        *min_y = (*min_y).min(y);
        *max_x = (*max_x).max(x);
        *max_y = (*max_y).max(y);
    } else {
        *bounds = Some((x, y, x, y));
    }
}

fn include_world_span(bounds: &mut Option<(f32, f32, f32, f32)>, x: f32, y: f32, radius: f32) {
    let r = radius.max(0.0);
    include_world_point(bounds, x - r, y - r);
    include_world_point(bounds, x + r, y + r);
}

fn renderer_snapshot_bounds(snapshot: &PcbSnapshot) -> Option<Rectangle> {
    let mut bounds: Option<(f32, f32, f32, f32)> = None;

    for trace in &snapshot.traces {
        let half_w = (trace.width_mm * 0.5).max(0.02);
        include_world_span(&mut bounds, trace.p0[0], trace.p0[1], half_w);
        include_world_span(&mut bounds, trace.p1[0], trace.p1[1], half_w);
    }

    for via in &snapshot.vias {
        include_world_span(
            &mut bounds,
            via.center[0],
            via.center[1],
            (via.diameter_mm * 0.5).max(0.02),
        );
    }

    for pad in &snapshot.pads {
        include_world_span(
            &mut bounds,
            pad.center[0],
            pad.center[1],
            (pad.size_mm[0].max(pad.size_mm[1]) * 0.5).max(0.02),
        );
    }

    for zone in &snapshot.zones {
        for vertex in &zone.vertices {
            include_world_point(&mut bounds, vertex[0], vertex[1]);
        }
    }

    for rule_area in &snapshot.rule_areas {
        for vertex in &rule_area.vertices {
            include_world_point(&mut bounds, vertex[0], vertex[1]);
        }
    }

    for line in &snapshot.ratsnest_lines {
        include_world_point(&mut bounds, line.p0[0], line.p0[1]);
        include_world_point(&mut bounds, line.p1[0], line.p1[1]);
    }

    for marker in &snapshot.drc_markers {
        include_world_span(
            &mut bounds,
            marker.center[0],
            marker.center[1],
            marker.radius_mm.max(0.02),
        );
    }

    bounds.map(|(min_x, min_y, max_x, max_y)| {
        Rectangle::new(
            iced::Point::new(min_x, min_y),
            iced::Size::new((max_x - min_x).max(0.1), (max_y - min_y).max(0.1)),
        )
    })
}

fn draw_dashed_line(
    frame: &mut canvas::Frame,
    p0: iced::Point,
    p1: iced::Point,
    width: f32,
    color: Color,
) {
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
        let sp = iced::Point::new(p0.x + ux * dist, p0.y + uy * dist);
        let ep = iced::Point::new(p0.x + ux * seg_end, p0.y + uy * seg_end);
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

fn draw_lines(
    frame: &mut canvas::Frame,
    lines: &[LineSegment],
    camera: &Camera,
    bounds: Rectangle,
) {
    for line in lines {
        let p0 = world_to_screen(camera, bounds, line.p0);
        let p1 = world_to_screen(camera, bounds, line.p1);
        let width = (line.width * camera.scale).max(0.5);
        let color = color_from_rgba(line.color);

        if line.is_dashed() {
            draw_dashed_line(frame, p0, p1, width, color);
        } else {
            let path = canvas::Path::line(p0, p1);
            frame.stroke(
                &path,
                canvas::Stroke::default()
                    .with_width(width)
                    .with_color(color),
            );
        }
    }
}

fn draw_circles(
    frame: &mut canvas::Frame,
    circles: &[GfxCircle],
    camera: &Camera,
    bounds: Rectangle,
) {
    for circle in circles {
        let center = world_to_screen(camera, bounds, circle.center);
        let radius = (circle.radius * camera.scale).max(0.5);
        let stroke_width = (circle.stroke_width * camera.scale).max(0.5);
        let color = color_from_rgba(circle.color);
        let path = canvas::Path::circle(center, radius);

        if circle.is_filled() {
            frame.fill(&path, color);
        } else {
            frame.stroke(
                &path,
                canvas::Stroke::default()
                    .with_width(stroke_width)
                    .with_color(color),
            );
        }
    }
}

fn draw_polygons(
    frame: &mut canvas::Frame,
    polygons: &[GpuPolygon],
    camera: &Camera,
    bounds: Rectangle,
) {
    for polygon in polygons {
        if polygon.vertices.len() < 3 {
            continue;
        }

        let points: Vec<iced::Point> = polygon
            .vertices
            .iter()
            .map(|vertex| world_to_screen(camera, bounds, *vertex))
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
                    .with_width((polygon.stroke_width * camera.scale).max(0.5))
                    .with_color(color_from_rgba(stroke_color)),
            );
        }
    }
}

fn draw_scene(frame: &mut canvas::Frame, scene: &Scene, camera: &Camera, bounds: Rectangle) {
    // Walk the shared `CPU_PCB_DRAW_ORDER` so this CPU path and the GPU
    // `scene_shader` cannot silently drift apart; the `scene::order` parity test
    // diffs the two orders. Main geometry first, then the overlay pass.
    for &bucket in CPU_PCB_DRAW_ORDER {
        match bucket {
            SceneBucket::Lines => draw_lines(frame, &scene.lines, camera, bounds),
            SceneBucket::Circles => draw_circles(frame, &scene.circles, camera, bounds),
            SceneBucket::Polygons => draw_polygons(frame, &scene.polygons, camera, bounds),
            SceneBucket::OverlayLines => draw_lines(frame, &scene.overlay_lines, camera, bounds),
            SceneBucket::OverlayCircles => {
                draw_circles(frame, &scene.overlay_circles, camera, bounds)
            }
            SceneBucket::OverlayPolygons => {
                draw_polygons(frame, &scene.overlay_polygons, camera, bounds)
            }
            // The PCB CPU path emits no arc, text, or ERC buckets. Handled for
            // exhaustiveness so adding a Scene bucket forces a decision here.
            SceneBucket::Arcs
            | SceneBucket::Texts
            | SceneBucket::ErcMarkerLines
            | SceneBucket::ErcMarkerCircles
            | SceneBucket::ErcMarkerPolygons => {}
        }
    }
}

impl canvas::Program<Message> for PcbCanvas {
    type State = PcbCanvasState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        if let Some(target) = self.pending_fit.take() {
            state.pending_fit = Some(target);
        }

        if let Some(target) = state.pending_fit.take() {
            self.camera.borrow_mut().fit_rect(target, bounds);
            return Some(canvas::Action::publish(Message::CanvasEvent(
                CanvasEvent::CursorMoved,
            )));
        }

        match event {
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let scroll_y = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => *y,
                    mouse::ScrollDelta::Pixels { y, .. } => *y / 50.0,
                };
                if let Some(cursor_pos) = cursor.position_in(bounds)
                    && self
                        .camera
                        .borrow_mut()
                        .zoom_at(cursor_pos, scroll_y, bounds)
                {
                    return Some(
                        canvas::Action::publish(Message::CanvasEvent(CanvasEvent::CursorMoved))
                            .and_capture(),
                    );
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(button)) => {
                if matches!(button, mouse::Button::Right | mouse::Button::Middle) {
                    state.panning = true;
                    state.last_pan_pos = cursor.position_in(bounds);
                    return Some(canvas::Action::capture());
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(button)) => {
                if matches!(button, mouse::Button::Right | mouse::Button::Middle) {
                    state.panning = false;
                    state.last_pan_pos = None;
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let Some(cursor_pos) = cursor.position_in(bounds) {
                    if state.panning
                        && let Some(last_pan_pos) = state.last_pan_pos
                    {
                        self.camera
                            .borrow_mut()
                            .pan(cursor_pos.x - last_pan_pos.x, cursor_pos.y - last_pan_pos.y);
                        state.last_pan_pos = Some(cursor_pos);
                        return Some(
                            canvas::Action::publish(Message::CanvasEvent(CanvasEvent::CursorMoved))
                                .and_capture(),
                        );
                    }

                    let (world, zoom_pct) = {
                        let camera = self.camera.borrow();
                        (
                            camera.screen_to_world(cursor_pos, bounds),
                            camera.zoom_percent(),
                        )
                    };
                    return Some(canvas::Action::publish(Message::CanvasEvent(
                        CanvasEvent::CursorAt {
                            x: world.x,
                            y: world.y,
                            zoom_pct,
                        },
                    )));
                }
            }
            _ => {}
        }

        None
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        // Single read of the sole camera home; both the background/grid cache
        // and the content cache below project from this one value.
        let camera = self.camera.borrow();
        let bg = self.bg_cache.draw(renderer, bounds.size(), |frame| {
            frame.fill_rectangle(iced::Point::ORIGIN, bounds.size(), self.theme_bg);

            if self.grid_visible {
                let step = (self.visible_grid_mm as f32 * camera.scale).max(8.0);
                let mut x = camera.offset.x.rem_euclid(step) - step;
                while x <= bounds.width + step {
                    let path = canvas::Path::line(
                        iced::Point::new(x, 0.0),
                        iced::Point::new(x, bounds.height),
                    );
                    frame.stroke(
                        &path,
                        canvas::Stroke::default()
                            .with_color(Color {
                                a: 0.18,
                                ..self.theme_grid
                            })
                            .with_width(0.5),
                    );
                    x += step;
                }

                let mut y = camera.offset.y.rem_euclid(step) - step;
                while y <= bounds.height + step {
                    let path = canvas::Path::line(
                        iced::Point::new(0.0, y),
                        iced::Point::new(bounds.width, y),
                    );
                    frame.stroke(
                        &path,
                        canvas::Stroke::default()
                            .with_color(Color {
                                a: 0.18,
                                ..self.theme_grid
                            })
                            .with_width(0.5),
                    );
                    y += step;
                }
            }
        });

        // GPU mode: a `shader` stacked above this canvas draws the board
        // content on the GPU (see `Self::gpu_render` / `Self::gpu_scene`). The
        // CPU canvas then contributes only the background + grid (`bg`), which
        // sits *behind* the shader — iced's shader primitive composites over
        // whatever is already there and never clears its own region.
        if self.gpu_render {
            return vec![bg];
        }

        let (cached_offset_x, cached_offset_y, cached_scale) = self.content_cache_camera.get();
        let camera_matches_cache = (cached_offset_x - camera.offset.x).abs() < 0.01
            && (cached_offset_y - camera.offset.y).abs() < 0.01
            && (cached_scale - camera.scale).abs() < 0.0001;
        if !camera_matches_cache {
            self.content_cache.clear();
        }
        let content = self.content_cache.draw(renderer, bounds.size(), |frame| {
            self.content_cache_camera
                .set((camera.offset.x, camera.offset.y, camera.scale));
            if let Some(snapshot) = self.active_renderer_snapshot() {
                let scene = self.build_scene(snapshot);
                draw_scene(frame, &scene, &camera, bounds);
            }
        });

        vec![bg, content]
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.panning {
            return mouse::Interaction::Grabbing;
        }
        if cursor.is_over(bounds) {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_renderer::pcb::PcbSnapshot;

    #[test]
    fn gpu_scene_is_none_without_a_snapshot() {
        let canvas = PcbCanvas::new();
        assert!(canvas.gpu_scene().is_none());
        // A missing snapshot must not populate the cache.
        assert!(canvas.scene_cache.borrow().is_none());
    }

    #[test]
    fn gpu_scene_builds_once_then_serves_from_cache() {
        let mut canvas = PcbCanvas::new();
        canvas.set_renderer_snapshot(Some(PcbSnapshot::default()));

        // First call tessellates and fills the cache.
        assert!(canvas.gpu_scene().is_some());
        assert!(canvas.scene_cache.borrow().is_some());

        // Subsequent calls are served from the cache (still populated).
        assert!(canvas.gpu_scene().is_some());
        assert!(canvas.scene_cache.borrow().is_some());
    }

    #[test]
    fn clear_content_cache_invalidates_the_gpu_scene() {
        let mut canvas = PcbCanvas::new();
        canvas.set_renderer_snapshot(Some(PcbSnapshot::default()));
        let _ = canvas.gpu_scene();
        assert!(canvas.scene_cache.borrow().is_some());

        canvas.clear_content_cache();
        assert!(canvas.scene_cache.borrow().is_none());
    }

    #[test]
    fn setting_a_new_snapshot_invalidates_the_gpu_scene() {
        let mut canvas = PcbCanvas::new();
        canvas.set_renderer_snapshot(Some(PcbSnapshot::default()));
        let _ = canvas.gpu_scene();
        assert!(canvas.scene_cache.borrow().is_some());

        canvas.set_renderer_snapshot(Some(PcbSnapshot::default()));
        assert!(canvas.scene_cache.borrow().is_none());
    }

    #[test]
    fn scene_generation_bumps_on_every_invalidation() {
        let mut canvas = PcbCanvas::new();
        let g0 = canvas.scene_generation();

        canvas.set_renderer_snapshot(Some(PcbSnapshot::default()));
        let g1 = canvas.scene_generation();
        assert!(g1 > g0, "snapshot swap must bump the generation");

        canvas.clear_content_cache();
        let g2 = canvas.scene_generation();
        assert!(g2 > g1, "content/theme change must bump the generation");
    }

    #[test]
    fn gpu_scene_keeps_a_stable_generation_across_cache_hits() {
        let mut canvas = PcbCanvas::new();
        canvas.set_renderer_snapshot(Some(PcbSnapshot::default()));

        let gen_at_build = canvas.scene_generation();
        let _ = canvas.gpu_scene(); // miss: builds + caches
        // A pan/zoom re-reads gpu_scene without invalidating: same generation,
        // so the shader will skip the geometry upload.
        let _ = canvas.gpu_scene(); // hit
        assert_eq!(canvas.scene_generation(), gen_at_build);
    }

    #[test]
    fn live_camera_reflects_the_single_source_after_a_mutation() {
        // #5 regression: `view()` (GPU shader) and the CPU `draw` both read the
        // one `camera` cell. Mutating it the way `Program::update` does must be
        // visible through `live_camera()` — there is no separate shadow that
        // could go stale.
        let canvas = PcbCanvas::new();
        let (x0, y0, s0) = canvas.live_camera();

        canvas.camera.borrow_mut().pan(12.0, -7.0);

        let (x1, y1, s1) = canvas.live_camera();
        assert_eq!(x1, x0 + 12.0);
        assert_eq!(y1, y0 - 7.0);
        assert_eq!(s1, s0, "pan must not change scale");
    }
}
