use std::sync::Arc;

mod board_text;
mod camera;
mod carry;
mod content;
#[cfg(test)]
mod content_pixel_tests;
#[cfg(test)]
mod content_tests;
mod cull;
mod drag_preview;
#[cfg(test)]
mod edge_parity_tests;
#[cfg(test)]
mod frame_bench_tests;
mod gesture;
mod grid;
mod highlight;
pub mod layer_style;
mod marks;
mod middle_zoom;
#[cfg(test)]
mod middle_zoom_tests;
mod msg;
mod panel_preview;
mod part_ghost;
mod rooms;
mod select;
mod snap_options;
mod tessellate;
mod viewport_resize;

pub use board_text::BoardTextLayer;
pub use camera::BoardCamera;
pub use carry::{
    CarryKind, PasteLoad, PcbCarry, board_after_move, board_without, carried_board, carry_delta,
    followed_arcs, snap_point, via_carry_delta, with_arcs_replaced,
};
pub use content::BoardContentLayer;
pub(crate) use grid::stroke_origin_marker;
pub use highlight::{Highlight, HighlightLayer};
pub use marks::BoardMarks;
pub use msg::{LEAVE_TRACKS_KEY, PcbCanvasMsg};
pub use rooms::{RoomMark, RoomsOverlay};
pub use select::PcbSelectionState;
pub use snap_options::{PcbSnapOptions, SnapDistance, SnapScope};

use crate::scene_generation::next_scene_generation;
use iced::event::Event;
use iced::mouse;
use iced::widget::canvas;
use iced::{Rectangle, Renderer, Theme};
use signex_gfx::scene::{DirtyFlags, Scene};
use signex_gfx::style::ColorSlot;
use signex_renderer::pcb::{PcbRenderer, PcbSnapshot};
use signex_renderer::schematic::ViewRenderer;
use signex_renderer::theme::ResolvedTheme;

use crate::app::{ContextMenuMsg, Message};
use crate::canvas::{Camera, CanvasEvent};
use cull::SceneBounds;

#[derive(Debug, Default)]
pub struct PcbCanvasState {
    panning: bool,
    /// The right press moved the view, so its release opens no menu.
    pan_moved: bool,
    last_pan_pos: Option<iced::Point>,
    /// The middle-button drag zooming the view, while it is held.
    middle_zoom: Option<crate::canvas::middle_zoom::MiddleZoom>,
    /// Ctrl/Cmd held, tracked from `ModifiersChanged` (mouse events carry no
    /// modifiers); keeps a modified scroll zooming under "Two-finger scroll pans".
    zoom_modifier: bool,
    /// Shift held; Shift+wheel pans sideways in "Mouse wheel: Scroll" mode.
    shift_held: bool,
    /// Left-button click / box gesture in flight.
    gesture: gesture::Gesture,
}

pub struct PcbCanvas {
    pub bg_cache: canvas::Cache,
    /// The board tessellated for the CPU mount ([`BoardContentLayer`]),
    /// retained across pans; dropped on every content change.
    content: std::cell::RefCell<Option<content::RetainedBoard>>,
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
    /// Per-primitive world extents of `scene_cache`, for CPU viewport culling.
    /// Built lazily on the first CPU draw of a scene and dropped with it, so
    /// pan/zoom redraws only test rectangles.
    scene_bounds: std::cell::RefCell<Option<Arc<SceneBounds>>>,
    /// Id of the geometry in `scene_cache`, renewed on every invalidation
    /// (`clear_content_cache` / `set_renderer_snapshot`). Handed to the shader
    /// via [`Self::scene_generation`] so the GPU pipeline skips re-uploading
    /// unchanged geometry on pan/zoom. Drawn from one process-wide counter
    /// ([`crate::scene_generation::next_scene_generation`]), so no two PCB canvases ever hold the same
    /// value: every window's canvas shares the one `PcbSurface` pipeline and
    /// its resident generation, and a per-canvas count would let one window
    /// skip its upload and draw another's board (#494). `Cell` for the same
    /// `&self`-in-`view()` reason as `scene_cache`.
    scene_generation: std::cell::Cell<u64>,
    /// The **sole** home of the PCB pan/zoom camera (ADR-0001 §A1: no shadow
    /// copies of widget state). `RefCell` gives interior mutability so
    /// `canvas::Program::update` — which only borrows `&self` — can mutate it,
    /// while `draw` and `view()` read it through the same cell. With no second
    /// copy in the widget `State`, the CPU background/grid, the CPU content
    /// path and the GPU shader program are read off one value and can never
    /// drift out of sync. It holds the view's turn too (#2316): view state
    /// like pan and zoom, never a board edit, never undone, never dirty.
    camera: std::cell::RefCell<BoardCamera>,
    pub pending_fit: std::cell::Cell<Option<Rectangle>>,
    /// The last fit applied and the camera it left (#2345).
    fitted: std::cell::Cell<Option<crate::canvas::FittedView>>,
    /// Bounds as of the last canvas event — the view centre for menu zoom.
    viewport: std::cell::Cell<Rectangle>,
    pub grid_visible: bool,
    /// Effective (live) PCB GPU-render flag. When true, `view()` mounts the
    /// shader stack in place of [`BoardContentLayer`]. Synced from the Preferences toggle —
    /// updated immediately on draft change for live preview, committed/reverted
    /// on Save/Discard. Mirrors `UiState::pcb_gpu_render` (the saved value).
    pub gpu_render: bool,
    /// Per-wheel-notch zoom factor (Preferences "Zoom speed"). Synced from
    /// `UiState` on every update, so canvases created later pick it up.
    pub zoom_step: f32,
    /// "Two-finger scroll pans" preference, synced with `zoom_step`.
    pub trackpad_pan: bool,
    /// Preferences "Mouse wheel", synced with `trackpad_pan`.
    pub wheel_mode: crate::canvas::WheelMode,
    pub canvas_colors: signex_types::theme::CanvasColors,
    pub renderer_snapshot: Option<PcbSnapshot>,
    pub visible_grid_mm: f64,
    /// The shown board's own grid (`PcbSetup::grid_size`, else the PCB
    /// default), the pitch placement snaps to; drawn in place of
    /// [`Self::visible_grid_mm`] so the lines sit on snap points (#2351).
    board_grid_mm: Option<f64>,
    /// Whether the board origin marker is drawn: the shown board's view
    /// setting (`PcbLayerView`), on by default (#2351).
    origin_marker_visible: bool,
    /// Guides, extra grids and the sheet of the shown board (#2374).
    marks: BoardMarks,
    /// The rooms selected on the shown board and the one being drawn
    /// (#2457), drawn standing out.
    rooms_overlay: RoomsOverlay,
    /// Selection and hover copper to paint, set by the app from
    /// [`PcbSelectionState`] (#2191).
    highlight: Highlight,
    /// Box being dragged, in board mm; transient, read by the highlight layer.
    marquee: std::cell::Cell<Option<(signex_types::pcb::Point, signex_types::pcb::Point)>>,
    /// The body and pads of every footprint, set by the app: a press on one
    /// drags it instead of starting a box (#2240, #2358).
    grab: Vec<pcb_geom::Shape>,
    /// A footprint drag is live; Esc clears it and the gesture quits.
    carry: std::cell::Cell<bool>,
    /// A tool, Move, paste or part on the cursor waits for clicks: every
    /// press is a click, never a drag or a box (#2396).
    clicks_only: bool,
    /// The Active Bar took the last right press, so its release is not a
    /// board right-click. Set by `Signex::set_swallow_right_release`.
    swallow_right_release: bool,
    /// The board without the carried footprints, and them at their offset.
    drag_preview: Option<drag_preview::DragPreview>,
    /// What the next click adds, set by the app (#2364), and the share of
    /// its colour it keeps: dimmed, or whole for a route being drawn.
    part_ghost: Option<(Arc<Scene>, f32)>,
    /// The footprint texts drawn by [`BoardTextLayer`], kept while the
    /// camera and the board stay as `text_key` says (#2368).
    text_cache: canvas::Cache,
    text_key: std::cell::Cell<Option<board_text::TextKey>>,
    /// Whether this canvas reports clicks and hovers.
    pub selectable: bool,
}

/// Ends the hold on a right release the Active Bar's press owned.
fn right_release_swallowed() -> canvas::Action<Message> {
    canvas::Action::publish(Message::ActiveBar(
        crate::active_bar::ActiveBarMsg::RightReleaseSwallowed,
    ))
    .and_capture()
}

impl Default for PcbCanvas {
    fn default() -> Self {
        Self::new()
    }
}

impl PcbCanvas {
    pub fn new() -> Self {
        let colors = crate::styles::board_canvas_colors();
        Self {
            bg_cache: canvas::Cache::default(),
            content: std::cell::RefCell::new(None),
            scene_cache: std::cell::RefCell::new(None),
            scene_bounds: std::cell::RefCell::new(None),
            scene_generation: std::cell::Cell::new(next_scene_generation()),
            camera: std::cell::RefCell::new(BoardCamera::default()),
            pending_fit: std::cell::Cell::new(None),
            fitted: std::cell::Cell::new(None),
            viewport: std::cell::Cell::new(Rectangle::with_size(iced::Size::ZERO)),
            grid_visible: true,
            gpu_render: false,
            zoom_step: Camera::zoom_step_factor(Camera::ZOOM_STEP_PERCENT_DEFAULT),
            trackpad_pan: false,
            wheel_mode: Default::default(),
            canvas_colors: colors,
            renderer_snapshot: None,
            visible_grid_mm: 2.54,
            board_grid_mm: None,
            origin_marker_visible: true,
            marks: BoardMarks::default(),
            rooms_overlay: RoomsOverlay::default(),
            highlight: Highlight::default(),
            marquee: std::cell::Cell::new(None),
            grab: Vec::new(),
            carry: std::cell::Cell::new(false),
            clicks_only: false,
            swallow_right_release: false,
            drag_preview: None,
            part_ghost: None,
            text_cache: canvas::Cache::default(),
            text_key: std::cell::Cell::new(None),
            selectable: true,
        }
    }

    pub fn set_highlight(&mut self, highlight: Highlight) {
        self.highlight = highlight;
    }

    pub fn highlight(&self) -> &Highlight {
        &self.highlight
    }

    /// What a press grabs to drag a footprint.
    pub fn set_grab(&mut self, grab: Vec<pcb_geom::Shape>) {
        self.grab = grab;
    }

    /// Make every press a click (a tool or a carry waits for clicks), or
    /// let a press drag a footprint or a box again.
    pub fn set_clicks_only(&mut self, clicks_only: bool) {
        self.clicks_only = clicks_only;
    }

    /// Whether every press is a click.
    pub fn is_clicks_only(&self) -> bool {
        self.clicks_only
    }

    /// Hold back the next right release, or stop holding it back.
    pub fn set_swallow_right_release(&mut self, swallow: bool) {
        self.swallow_right_release = swallow;
    }

    /// Whether the next right release is held back.
    #[cfg(test)]
    pub fn swallows_right_release(&self) -> bool {
        self.swallow_right_release
    }

    /// Whether a press at `at` (board mm) lands on a grabbed footprint.
    pub fn grabs(&self, at: signex_types::pcb::Point, reach_mm: f64) -> bool {
        let (Ok(p), Ok(reach)) = (
            pcb_geom::PointNm::from_mm(at, "press"),
            pcb_geom::mm_to_nm(reach_mm, "reach"),
        ) else {
            return false;
        };
        self.grab.iter().any(|shape| shape.contains(p, reach))
    }

    /// Whether a footprint drag is live on this canvas.
    pub fn is_carrying(&self) -> bool {
        self.carry.get()
    }

    /// Drop a box or a footprint drag in progress (Esc); the gesture sees
    /// it gone and quits.
    pub fn cancel_marquee(&self) {
        self.marquee.set(None);
        self.carry.set(false);
    }

    pub fn highlight_layer(&self) -> HighlightLayer<'_> {
        HighlightLayer(self)
    }

    pub(crate) fn camera_ref(&self) -> std::cell::Ref<'_, BoardCamera> {
        self.camera.borrow()
    }

    pub fn active_renderer_snapshot(&self) -> Option<&PcbSnapshot> {
        self.renderer_snapshot.as_ref()
    }

    /// The board to draw. A drag preview in progress ends: the app starts
    /// it again over the new board if the carry is still live.
    pub fn set_renderer_snapshot(&mut self, renderer_snapshot: Option<PcbSnapshot>) {
        self.drag_preview = None;
        self.replace_snapshot(renderer_snapshot);
    }

    fn replace_snapshot(&mut self, renderer_snapshot: Option<PcbSnapshot>) {
        self.renderer_snapshot = renderer_snapshot;
        // The board changed — the cached GPU scene no longer matches.
        *self.content.get_mut() = None;
        *self.scene_cache.get_mut() = None;
        *self.scene_bounds.get_mut() = None;
        *self.scene_generation.get_mut() = next_scene_generation();
        // The backdrop draws the board area from the outline.
        self.bg_cache.clear();
    }

    /// On-screen grid line spacing in pixels at `scale` px/mm, or `None` when
    /// the grid is toggled off or unusable.
    pub fn grid_step_px(&self, scale: f32) -> Option<f32> {
        self.grid_level(scale).map(|level| level.step_px)
    }

    /// The grid level drawn at `scale` px/mm: the board's own grid when a
    /// board is shown, else the app-wide visible grid.
    fn grid_level(&self, scale: f32) -> Option<grid::GridLevel> {
        if !self.grid_visible {
            return None;
        }
        let pitch_mm = self.board_grid_mm.unwrap_or(self.visible_grid_mm);
        grid::grid_level(pitch_mm as f32, scale)
    }

    /// Draw the shown board's grid (`None`: no board, the app-wide grid).
    pub fn set_board_grid(&mut self, grid_mm: Option<f64>) {
        if self.board_grid_mm != grid_mm {
            self.board_grid_mm = grid_mm;
            self.bg_cache.clear();
        }
    }

    /// Draw the shown board's guides, extra grids and sheet (#2374).
    pub fn set_board_marks(&mut self, marks: BoardMarks) {
        if self.marks != marks {
            self.marks = marks;
            self.bg_cache.clear();
        }
    }

    /// The rooms selected on the shown board and the one being drawn
    /// (#2457); the backdrop repaints when they change.
    pub fn set_rooms_overlay(&mut self, overlay: RoomsOverlay) {
        if self.rooms_overlay != overlay {
            self.rooms_overlay = overlay;
            self.bg_cache.clear();
        }
    }

    pub fn board_marks(&self) -> &BoardMarks {
        &self.marks
    }

    pub fn clear_bg_cache(&mut self) {
        self.bg_cache.clear();
    }

    /// Adopt the app-wide Grid Toggle and Visible Grid Size (`UiState`), the
    /// same values the schematic canvas reads each frame (#682). Invalidates
    /// the background cache only when something actually changed.
    pub fn set_grid(&mut self, visible: bool, visible_grid_mm: f64) {
        if self.grid_visible != visible || self.visible_grid_mm != visible_grid_mm {
            self.grid_visible = visible;
            self.visible_grid_mm = visible_grid_mm;
            self.bg_cache.clear();
        }
    }

    pub fn clear_content_cache(&mut self) {
        *self.content.get_mut() = None;
        // GPU scene shares this content-changed signal (theme + geometry).
        *self.scene_cache.get_mut() = None;
        *self.scene_bounds.get_mut() = None;
        *self.scene_generation.get_mut() = next_scene_generation();
    }

    /// Record the canvas bounds (done on every canvas event; public so tests can
    /// stand in for a drawn frame).
    pub fn set_viewport(&self, bounds: Rectangle) {
        self.viewport.set(bounds);
    }

    /// Zoom one wheel notch about the view centre (View > Zoom In/Out).
    /// Returns whether the camera changed (false at the zoom limits).
    pub fn zoom_about_centre(&self, zoom_in: bool, step: f32) -> bool {
        let bounds = self.viewport.get();
        let centre = iced::Point::new(bounds.width / 2.0, bounds.height / 2.0);
        let notch = if zoom_in { 1.0 } else { -1.0 };
        self.camera
            .borrow_mut()
            .zoom_at(centre, notch, step, bounds)
    }

    /// Scale the view by `factor` about its centre, as a pinch does. Returns
    /// whether the camera changed (false at the zoom limits).
    pub fn zoom_by_factor_about_centre(&self, factor: f32) -> bool {
        let bounds = self.viewport.get();
        let centre = iced::Point::new(bounds.width / 2.0, bounds.height / 2.0);
        self.camera.borrow_mut().zoom_by_factor(centre, factor)
    }

    /// Move the board on screen by `(dx, dy)` canvas pixels, as a
    /// right-button drag does.
    pub fn pan_by(&self, dx: f32, dy: f32) {
        self.camera.borrow_mut().pan(dx, dy);
    }

    /// Recentre the view on world point `(x, y)` without changing zoom or
    /// turn (#525).
    pub fn center_on_world(&self, x: f32, y: f32) {
        let bounds = self.viewport.get();
        self.camera
            .borrow_mut()
            .center_on(iced::Point::new(x, y), bounds);
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

    /// Adopt the board palette, the backdrop's colours included.
    pub fn set_theme_colors(&mut self, colors: signex_types::theme::CanvasColors) {
        self.canvas_colors = colors;
        self.bg_cache.clear();
    }

    /// Whether the board origin marker is drawn.
    pub fn origin_marker_visible(&self) -> bool {
        self.origin_marker_visible
    }

    /// Show or hide the board origin marker (the board view's setting).
    pub fn set_origin_marker_visible(&mut self, visible: bool) {
        if self.origin_marker_visible != visible {
            self.origin_marker_visible = visible;
            self.bg_cache.clear();
        }
    }

    /// The camera's zoom as a percentage, `100.0` at the default scale.
    pub fn zoom_percent(&self) -> f64 {
        self.camera.borrow().zoom_percent()
    }

    /// Current pan/zoom for the GPU path as
    /// `(offset_x_px, offset_y_px, scale_px_per_mm)`, read in `view()` from the
    /// single [`Self::camera`] home to build the shader program. Reads the same
    /// cell the CPU `draw` uses, so the GPU content stays frame-coherent with
    /// the CPU background + grid.
    pub fn live_camera(&self) -> (f32, f32, f32) {
        let camera = self.camera.borrow();
        (camera.offset().x, camera.offset().y, camera.scale())
    }

    /// The board scene's colours: the board palette with a clear board-area
    /// fill, since the backdrop canvas paints the board area with the grid
    /// on it (#2351) and an opaque fill above would hide the grid.
    fn content_theme(&self) -> ResolvedTheme {
        let mut theme = ResolvedTheme::from_canvas_colors(self.canvas_colors);
        theme.set_slot(ColorSlot::BoardArea, [0.0; 4]);
        theme
    }

    /// Build the `signex_gfx` scene for a board snapshot. Shared by the CPU
    /// `draw` path and the GPU [`Self::gpu_scene`] path so both tessellate from
    /// identical instance data.
    fn build_scene(&self, snapshot: &PcbSnapshot) -> Scene {
        let mut scene = Scene::default();
        let theme = self.content_theme();
        PcbRenderer::build_scene(
            snapshot,
            &theme,
            DirtyFlags::LINES | DirtyFlags::CIRCLES | DirtyFlags::POLYGONS | DirtyFlags::OVERLAY,
            &mut scene,
        );
        scene
    }

    /// Build the scene for GPU rendering: the same geometry as the CPU path.
    /// Overlay primitives (active-layer zone highlight, selection highlight)
    /// stay in their own `Scene::overlay_*` fields rather than folding into
    /// the base buckets — `ScenePrimitive::draw` (in `crate::scene_shader`)
    /// composites them in a dedicated pass *after* every base bucket, so they
    /// always render on top, matching the CPU `draw_scene` overlay pass.
    /// Returns `None` when no board snapshot is loaded.
    pub fn gpu_scene(&self) -> Option<Arc<Scene>> {
        // Fast path: hand back the shared cached scene without re-tessellating.
        // `borrow().clone()` is an `Arc` refcount bump (on a miss it clones
        // `None`, which is free); the `borrow()` temporary is released before
        // the rebuild below takes a `borrow_mut()`. See [`Self::scene_cache`].
        if let Some(scene) = self.scene_cache.borrow().clone() {
            return Some(scene);
        }

        let snapshot = self.active_renderer_snapshot()?;
        let scene = Arc::new(self.build_scene(snapshot));
        *self.scene_cache.borrow_mut() = Some(Arc::clone(&scene));
        Some(scene)
    }

    /// Culling extents for the scene [`Self::gpu_scene`] returns, cached with it.
    fn cull_bounds(&self, scene: &Scene) -> Arc<SceneBounds> {
        if let Some(bounds) = self.scene_bounds.borrow().clone() {
            return bounds;
        }
        let bounds = Arc::new(SceneBounds::from_scene(scene));
        *self.scene_bounds.borrow_mut() = Some(Arc::clone(&bounds));
        bounds
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

#[cfg(test)]
fn world_to_screen(camera: &BoardCamera, _bounds: Rectangle, point: [f32; 2]) -> iced::Point {
    camera.world_to_screen(iced::Point::new(point[0], point[1]))
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

    // The board itself: its outline, cutouts, keepouts and holes, so Fit
    // shows the whole board and not only its copper.
    let features = &snapshot.board_features;
    for polygon in std::iter::once(&features.outline)
        .chain(&features.cutouts)
        .chain(&features.keepouts)
    {
        for vertex in polygon {
            include_world_point(&mut bounds, vertex[0], vertex[1]);
        }
    }
    for hole in &features.holes {
        include_world_span(
            &mut bounds,
            hole.center[0],
            hole.center[1],
            (hole.drill_mm * 0.5).max(0.02),
        );
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

impl canvas::Program<Message> for PcbCanvas {
    type State = PcbCanvasState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        // Remember the bounds so menu zoom (no cursor) can find the view centre.
        self.viewport.set(bounds);
        if let Some(action) = self.middle_zoom_event(state, event, bounds, cursor) {
            return Some(action);
        }
        if self.resolve_pending_fit(bounds) {
            return Some(canvas::Action::publish(Message::CanvasEvent(
                CanvasEvent::CursorMoved,
            )));
        }

        // A key released while another window had the focus never reaches
        // this canvas: forget the held modifiers (#2396).
        if let Event::Window(iced::window::Event::Unfocused) = event {
            state.zoom_modifier = false;
            state.shift_held = false;
        }

        if self.selectable
            && let Some(action) = state.gesture.handle(self, event, bounds, cursor)
        {
            return Some(action);
        }

        match event {
            Event::Keyboard(iced::keyboard::Event::ModifiersChanged(mods)) => {
                state.zoom_modifier = mods.command() || mods.control();
                state.shift_held = mods.shift();
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let intent = crate::canvas::wheel_intent(
                    delta,
                    50.0,
                    self.trackpad_pan,
                    self.wheel_mode,
                    crate::canvas::WheelKeys {
                        zoom: state.zoom_modifier,
                        shift: state.shift_held,
                    },
                );
                if let Some(cursor_pos) = cursor.position_in(bounds)
                    && self.camera.borrow_mut().apply_scroll(
                        cursor_pos,
                        intent,
                        self.zoom_step,
                        bounds,
                    )
                {
                    return Some(
                        canvas::Action::publish(Message::CanvasEvent(CanvasEvent::ViewMoved))
                            .and_capture(),
                    );
                }
            }
            Event::Mouse(mouse::Event::PinchGesture { delta }) => {
                if let Some(cursor_pos) = cursor.position_in(bounds)
                    && self
                        .camera
                        .borrow_mut()
                        .zoom_by_factor(cursor_pos, Camera::pinch_factor(*delta))
                {
                    return Some(
                        canvas::Action::publish(Message::CanvasEvent(CanvasEvent::ViewMoved))
                            .and_capture(),
                    );
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(button)) => {
                // A press outside the board belongs to the widget under it:
                // the layer tabs under the canvas open their menu (#2379).
                if *button == mouse::Button::Right
                    && let Some(pos) = cursor.position_in(bounds)
                {
                    state.panning = true;
                    state.pan_moved = false;
                    state.last_pan_pos = Some(pos);
                    // A held-back release was lost: this press ends the hold.
                    if self.swallow_right_release {
                        return Some(right_release_swallowed());
                    }
                    return Some(canvas::Action::capture());
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(button)) => {
                if *button == mouse::Button::Right {
                    let moved = std::mem::take(&mut state.pan_moved);
                    state.panning = false;
                    state.last_pan_pos = None;
                    if self.swallow_right_release {
                        return Some(right_release_swallowed());
                    }
                    // A right click that did not pan opens the board's menu (#2240).
                    if !moved
                        && self.selectable
                        && let Some(pos) = cursor.position_in(bounds)
                    {
                        let show = ContextMenuMsg::Show(bounds.x + pos.x, bounds.y + pos.y);
                        return Some(
                            canvas::Action::publish(Message::ContextMenu(show)).and_capture(),
                        );
                    }
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
                        state.pan_moved |= cursor_pos != last_pan_pos;
                        state.last_pan_pos = Some(cursor_pos);
                        return Some(
                            canvas::Action::publish(Message::CanvasEvent(CanvasEvent::ViewMoved))
                                .and_capture(),
                        );
                    }

                    return Some(canvas::Action::publish(
                        self.pointer_report(cursor_pos, bounds),
                    ));
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
        // Single read of the sole camera home for the backdrop.
        let camera = self.camera.borrow();
        let bg = self.bg_cache.draw(renderer, bounds.size(), |frame| {
            let size = bounds.size();
            let outline = self
                .renderer_snapshot
                .as_ref()
                .map(|s| s.board_features.outline.as_slice());
            let region = grid::board_region(outline);
            let sheet = marks::sheet_box(&self.marks, region);
            grid::draw_board_area(frame, &camera, size, region, sheet, &self.canvas_colors);
            if let Some(level) = self.grid_level(camera.scale()) {
                let paint = grid::grid_paint(&self.canvas_colors, level.fade);
                grid::draw_grid(
                    frame,
                    &camera,
                    size,
                    region,
                    self.marks.origin,
                    level,
                    paint,
                );
                marks::draw_grids(frame, &camera, size, region, &self.marks.grids);
            }
            if let Some(panel) = &self.marks.panel {
                panel_preview::draw_panel(frame, &camera, panel, &self.canvas_colors);
            }
            // #2457: under the copper, which is a layer above this canvas.
            rooms::draw_rooms(
                frame,
                &camera,
                &self.marks.rooms,
                &self.rooms_overlay,
                &self.canvas_colors,
            );
            marks::draw_guides(frame, &camera, size, &self.marks.guides);
            if self.origin_marker_visible {
                let origin = crate::render_config::to_iced(&self.canvas_colors.cursor);
                grid::draw_origin(frame, &camera, size, self.marks.origin, origin);
            }
        });

        // The board is a layer of its own stacked above this canvas: the
        // `shader` on the GPU mount, [`BoardContentLayer`] on the CPU mount
        // (#2321). This canvas draws the workspace, the board area, the grid
        // and the origin marker behind it, so both mounts share them; the
        // board scene leaves its board-area fill clear (`content_theme`).
        vec![bg]
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
        if state.middle_zoom.is_some() {
            return mouse::Interaction::ResizingVertically;
        }
        if self.carry.get() {
            return mouse::Interaction::Move;
        }
        // A tool, Measure, or a Move, paste or part waiting for clicks.
        if cursor.is_over(bounds) {
            crate::canvas::cursor::tool_cursor(self.clicks_only)
        } else {
            mouse::Interaction::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_renderer::pcb::{DrcMarkerInput, PcbSnapshot};
    use signex_types::violation::Severity;

    #[test]
    fn grid_step_px_is_pixels_not_millimetres() {
        let canvas = PcbCanvas::new();
        // Default zoom 3 px/mm, 2.54 mm grid: 7.62 px < 8 px floor, so the
        // 5x level (12.7 mm) is drawn, which is 38.1 px on screen.
        let step = canvas.grid_step_px(3.0).expect("grid is visible");
        assert!((step - 38.1).abs() < 1e-3, "got {step}");
    }

    #[test]
    fn grid_step_px_is_none_when_grid_toggled_off() {
        let mut canvas = PcbCanvas::new();
        canvas.set_grid(false, 2.54);
        assert_eq!(canvas.grid_step_px(3.0), None);
    }

    #[test]
    fn the_shown_boards_grid_wins_over_the_app_wide_grid() {
        let mut canvas = PcbCanvas::new();
        // 0.5 mm board grid at 20 px/mm: 10 px, drawn as is.
        canvas.set_board_grid(Some(0.5));
        let step = canvas.grid_step_px(20.0).expect("grid is visible");
        assert!((step - 10.0).abs() < 1e-4, "got {step}");
        // No board: back to the app-wide 2.54 mm grid (50.8 px).
        canvas.set_board_grid(None);
        let step = canvas.grid_step_px(20.0).expect("grid is visible");
        assert!((step - 50.8).abs() < 1e-3, "got {step}");
    }

    #[test]
    fn the_origin_marker_is_shown_until_hidden() {
        let mut canvas = PcbCanvas::new();
        assert!(canvas.origin_marker_visible());
        canvas.set_origin_marker_visible(false);
        assert!(!canvas.origin_marker_visible());
    }

    /// The backdrop paints the board area under the grid; the board scene
    /// must leave it clear on both mounts, or the grid is hidden.
    #[test]
    fn the_board_scene_leaves_the_board_area_clear() {
        let canvas = PcbCanvas::new();
        assert_eq!(canvas.content_theme().color(ColorSlot::BoardArea), [0.0; 4]);
        let full = ResolvedTheme::from_canvas_colors(canvas.canvas_colors);
        assert_eq!(
            canvas.content_theme().color(ColorSlot::BoardEdge),
            full.color(ColorSlot::BoardEdge)
        );
    }

    #[test]
    fn set_grid_adopts_toggle_and_size() {
        let mut canvas = PcbCanvas::new();
        canvas.set_grid(false, 1.27);
        assert!(!canvas.grid_visible);
        assert_eq!(canvas.visible_grid_mm, 1.27);
    }

    /// Correctness — thread #4 (z-order): `gpu_scene()` must NOT fold overlay
    /// geometry into the base `polygons`/`lines`/`circles` buckets. A DRC
    /// marker (an overlay-only primitive with no base-scene counterpart in
    /// this snapshot) must stay in `overlay_*` so the shader's dedicated
    /// overlay pass — which always composites after every base bucket — is
    /// the only thing that draws it. If the fold regresses, this marker would
    /// show up in `scene.polygons`/`scene.circles` instead, which is exactly
    /// the bug that drew overlays *under* base content.
    #[test]
    fn gpu_scene_keeps_overlays_out_of_the_base_buckets() {
        let mut canvas = PcbCanvas::new();
        let snapshot = PcbSnapshot::default().with_drc_markers(vec![DrcMarkerInput {
            center: [1.0, 1.0],
            radius_mm: 0.3,
            severity: Severity::Error,
            violation_type: None,
        }]);
        canvas.set_renderer_snapshot(Some(snapshot));

        let scene = canvas.gpu_scene().expect("snapshot is set");
        assert!(
            scene.polygons.is_empty(),
            "the DRC marker's overlay polygon must not land in the base bucket"
        );
        assert!(
            scene.top_polygons.is_empty(),
            "the DRC marker's overlay polygon must not land in the top-polygon bucket"
        );
        assert!(
            scene.circles.is_empty(),
            "the DRC marker's overlay circle must not land in the base bucket"
        );
        assert_eq!(scene.overlay_polygons.len(), 1);
        assert_eq!(scene.overlay_circles.len(), 1);
    }

    #[test]
    fn cull_bounds_are_cached_with_the_scene_and_dropped_on_invalidation() {
        let mut canvas = PcbCanvas::new();
        canvas.set_renderer_snapshot(Some(PcbSnapshot::default()));
        let scene = canvas.gpu_scene().expect("snapshot is set");

        let first = canvas.cull_bounds(&scene);
        let second = canvas.cull_bounds(&scene);
        assert!(
            Arc::ptr_eq(&first, &second),
            "pan/zoom must reuse the bounds"
        );

        canvas.clear_content_cache();
        assert!(canvas.scene_bounds.borrow().is_none());
        let scene = canvas.gpu_scene().expect("snapshot is set");
        let rebuilt = canvas.cull_bounds(&scene);
        assert!(!Arc::ptr_eq(&first, &rebuilt));

        canvas.set_renderer_snapshot(Some(PcbSnapshot::default()));
        assert!(canvas.scene_bounds.borrow().is_none());
    }

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

    /// #903 review: every window's PCB canvas uploads through one shared
    /// pipeline, which skips the upload when the generation matches the
    /// resident one. Two canvases must never be at the same generation, or
    /// one window draws the other's board.
    #[test]
    fn two_pcb_canvases_never_share_a_scene_generation() {
        let mut main = PcbCanvas::new();
        let mut undocked = PcbCanvas::new();
        let mut seen = std::collections::HashSet::new();
        assert!(seen.insert(main.scene_generation()));
        assert!(seen.insert(undocked.scene_generation()));
        for _ in 0..4 {
            main.clear_content_cache();
            assert!(seen.insert(main.scene_generation()));
            undocked.set_renderer_snapshot(Some(PcbSnapshot::default()));
            assert!(seen.insert(undocked.scene_generation()));
        }
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

    #[test]
    fn gpu_scene_keeps_an_overlay_polygon_out_of_the_main_bucket() {
        // #4 (fixed, review #308 finding 4): `gpu_scene()` used to fold
        // `overlay_polygons` into the main `polygons` Vec *before* the GPU
        // shader ever saw the scene, so `GPU_SCENE_DRAW_ORDER` (which has no
        // overlay buckets at all) drew that geometry as an ordinary member of
        // the early `Polygons` bucket -- under traces, not on top of them --
        // while the CPU `draw_scene` path walks `CPU_PCB_DRAW_ORDER`, which
        // paints `OverlayPolygons` LAST. `gpu_scene()` now leaves overlay
        // geometry in its own field, and `scene_shader::ScenePrimitive::draw`
        // composites it in a dedicated pass strictly after every base bucket
        // -- matching the CPU path exactly. This pins that CPU==GPU parity by
        // comparing the same distinctly-coloured overlay polygon across both
        // scene builds.
        let mut canvas = PcbCanvas::new();
        let snapshot = PcbSnapshot::default().with_drc_markers(vec![DrcMarkerInput {
            center: [5.0, 5.0],
            radius_mm: 0.5,
            severity: Severity::Error,
            violation_type: None,
        }]);

        // CPU reference: a DRC marker is the one thing an otherwise-empty
        // board contributes to `overlay_polygons`, distinctly coloured by
        // severity (`ColorSlot::DrcError`) and nothing else lands in the
        // plain `polygons` bucket for an empty board.
        let cpu_scene = canvas.build_scene(&snapshot);
        assert_eq!(
            cpu_scene.overlay_polygons.len(),
            1,
            "a DRC marker must emit exactly one overlay polygon"
        );
        assert!(
            cpu_scene.polygons.is_empty() && cpu_scene.top_polygons.is_empty(),
            "an empty board contributes no main-bucket polygons"
        );
        let overlay_fill_color = cpu_scene.overlay_polygons[0].fill_color;

        canvas.set_renderer_snapshot(Some(snapshot));
        let gpu_scene = canvas.gpu_scene().expect("snapshot is set");

        // Parity: the same distinctly-coloured polygon stays in
        // `overlay_polygons` on the GPU path too, never folded into the main
        // `polygons` bucket -- CPU and GPU agree on where it lives.
        assert!(
            gpu_scene.polygons.is_empty(),
            "the overlay polygon must not be folded into the main polygons bucket"
        );
        assert!(
            gpu_scene.top_polygons.is_empty(),
            "the overlay polygon must not be folded into the top-polygon bucket"
        );
        assert_eq!(
            gpu_scene.overlay_polygons.len(),
            1,
            "gpu_scene must keep the overlay polygon in its own bucket"
        );
        assert_eq!(gpu_scene.overlay_polygons[0].fill_color, overlay_fill_color);
    }

    /// Round 9 of #2262: Fit covers the board outline, keepouts and holes,
    /// not only the copper, so a board with a few pads is shown whole.
    #[test]
    fn fit_bounds_cover_the_board_outline_keepouts_and_holes() {
        use signex_renderer::pcb::{BoardFeaturesInput, HoleInput};
        let snapshot = PcbSnapshot {
            board_features: BoardFeaturesInput {
                outline: vec![[0.0, 0.0], [50.0, 0.0], [50.0, 40.0], [0.0, 40.0]],
                cutouts: vec![],
                holes: vec![HoleInput {
                    center: [55.0, 20.0],
                    drill_mm: 4.0,
                }],
                keepouts: vec![vec![[-5.0, 10.0], [5.0, 10.0], [5.0, 45.0]]],
                edge_hidden: false,
            },
            ..PcbSnapshot::default()
        };
        let bounds = renderer_snapshot_bounds(&snapshot).expect("bounds");
        assert_eq!((bounds.x, bounds.y), (-5.0, 0.0));
        assert_eq!((bounds.width, bounds.height), (62.0, 45.0));
    }
}
