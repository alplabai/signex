//! Schematic/PCB canvas — wgpu rendering with Altium-style pan/zoom/grid.
//!
//! Uses `iced::widget::canvas::Program`. Layers, back to front:
//! - background (`bg_cache`): cached, cleared on theme/grid/viewport change;
//! - content (`content_cache`): cached, cleared when the document or camera
//!   changes (pan frames replay a camera-independent scene);
//! - autofocus dim: drawn over the content when autofocus is active;
//! - selection overlay (`overlay_cache`): cached, cleared on input events;
//! - HUD / previews (cursor, wire and placement previews): uncached, redrawn
//!   every frame.

mod camera;
mod drag_layers;
pub(crate) mod drag_preview;
mod draw;
mod erc_marker;
pub mod grid;
mod input;
mod move_pick;
mod shape_preview;
mod shift_selection;

use shift_selection::shift_snapshot_for_selection;

use iced::event::Event;
use iced::mouse;
use iced::widget::canvas;
use iced::{Color, Rectangle, Renderer, Theme};

pub use camera::Camera;
pub use erc_marker::{ErcMarker, ErcMarkerSeverity};
pub use move_pick::MovePick;
pub use shape_preview::ShapePreviewKind;

use crate::schematic_runtime::SchematicSheetExt as _;

use crate::app::{ContextMenuMsg, Message};
use crate::toolbar::ToolMessage;

// ─── Canvas State (per-canvas mutable state) ──────────────────

#[derive(Debug, Default)]
pub struct CanvasState {
    /// Is the user currently panning (right-click or middle-click drag)?
    panning: bool,
    /// Whether actual pan movement occurred (to distinguish click from drag).
    pan_moved: bool,
    /// Last cursor position during a pan (in screen pixels).
    last_pan_pos: Option<iced::Point>,
    /// Whether Ctrl is currently held (for multi-select toggle).
    pub ctrl_held: bool,
    /// Whether Shift is currently held (for multi-select add).
    pub shift_held: bool,
    /// Drag-to-select: start position in world coordinates.
    select_drag_start: Option<(f64, f64)>,
    /// Drag-to-select: current end position in world coordinates.
    select_drag_end: Option<(f64, f64)>,
    // ─── Drag-to-move state ───
    /// True when the initial left-click landed on an already-selected item.
    click_on_selected: bool,
    /// World position where the move-drag started.
    move_origin: Option<(f64, f64)>,
    /// True once the threshold is exceeded and we're actively moving.
    move_dragging: bool,
    /// Current world position during a move-drag (for preview offset).
    move_current: Option<(f64, f64)>,
    /// Set while a click-click move floats (see `move_pick::Float`).
    move_float: Option<move_pick::Float>,
    /// The rubber-banded drag ghost, filled from `update`, read by `draw`.
    ghost: drag_preview::GhostState,
    // ─── Double-click detection ───
    /// Timestamp of last left-click (for double-click detection).
    last_click_time: Option<std::time::Instant>,
    /// Screen position of last left-click (double-click distance is compared
    /// in pixels, so zooming between presses cannot skew it).
    last_click_screen: Option<(f32, f32)>,
}

// ─── CanvasSlot (the Program) ────────────────────────────

/// The canvas program that handles input and rendering.
/// Holds references to app state needed for drawing (theme colors, etc).
pub struct CanvasSlot {
    pub bg_cache: canvas::Cache,
    pub content_cache: canvas::Cache,
    pub overlay_cache: canvas::Cache,
    /// Camera state when content_cache was last built — used to compute offset delta.
    pub content_cache_camera: std::cell::Cell<(f32, f32, f32)>, // (offset_x, offset_y, scale)
    /// Camera-independent content scene: pan frames replay it through the live
    /// transform instead of rebuilding snapshot + scene (#171 item 6).
    /// Dropped with `content_cache` by [`Self::clear_content_cache`].
    content_scene: std::cell::RefCell<crate::schematic_runtime::ContentSceneCache>,
    /// The static layer and lifted items of a move-drag in flight (#1335);
    /// dropped when the drag ends, is cancelled, or the document changes.
    drag_layers: std::cell::RefCell<drag_layers::DragLayers>,
    /// The **sole** home of the schematic pan/zoom camera (ADR-0001 §A1: no
    /// shadow copies of widget state). `RefCell` gives interior mutability so
    /// `canvas::Program::update` — which only borrows `&self` — can mutate it,
    /// while `draw` and `view()` read it through the same cell.
    ///
    /// #632 — this replaces a pair: `CanvasState::camera` (the `Program::State`
    /// copy that `update` and `draw` worked on) plus a
    /// `live_camera: Cell<(f32, f32, f32)>` republished from `draw` every
    /// frame so `view` could place world-anchored overlays. `view(&self)`
    /// cannot reach a `Program::State`, so the camera round-tripped through
    /// that `Cell` and the two could disagree for a frame. One home, read by
    /// the background, the content, the overlays and `view`, cannot drift.
    /// Mirrors [`crate::pcb_canvas::PcbCanvas::camera`], which already had it.
    camera: std::cell::RefCell<Camera>,
    /// Render-facing cache of the currently visible schematic.
    /// The app updates this from the active engine or active tab cache.
    pub render_cache: Option<crate::schematic_runtime::SchematicRenderCache>,
    /// Currently selected items — drives selection overlay rendering.
    pub selected: Vec<signex_types::schematic::SelectedItem>,
    /// Pending fit target to transfer to CanvasState.
    /// Uses Cell so canvas::Program::update (&self) can consume it.
    pub pending_fit: std::cell::Cell<Option<Rectangle>>,
    /// Bounds of the canvas as of the last canvas event — lets menu zoom (which has no
    /// cursor and no event bounds) find the view centre.
    viewport: std::cell::Cell<Rectangle>,
    /// Wire-in-progress points for rubber-band preview.
    pub wire_preview: Vec<signex_types::schematic::Point>,
    /// Whether currently in wire/bus drawing mode.
    pub drawing_mode: bool,
    /// Current tool name for preview display.
    pub tool_preview: Option<String>,
    /// Ghost label preview for port/label placement (follows cursor).
    pub ghost_label: Option<signex_types::schematic::Label>,
    /// Ghost power-port / symbol preview for placement (follows cursor).
    pub ghost_symbol: Option<signex_types::schematic::Symbol>,
    /// Ghost text-note preview for placement (follows cursor).
    pub ghost_text: Option<signex_types::schematic::TextNote>,
    /// Ghost bus entry (its orientation and size) for placement; the bus end
    /// follows the cursor and snaps onto a bus.
    pub ghost_bus_entry: Option<(f64, f64)>,
    /// When true, placement is paused (TAB pressed → pre-placement form
    /// open). The ghost freezes and canvas clicks don't place — the user
    /// interacts with the Properties panel until they confirm with OK.
    pub placement_paused: bool,
    /// Active paper width in mm (world units).
    pub paper_width_mm: f32,
    /// Active paper height in mm (world units).
    pub paper_height_mm: f32,
    /// Border, zone markers and title block for the paper, lowered by the
    /// app with the same lowering the PDF uses (#1079). Drawn in sheet mm.
    pub sheet_frame: Vec<signex_output::template::FramePrimitive>,
    /// Designators of the placement this tab was opened through (#1151), by
    /// symbol uuid, for the symbols whose designator there differs from their
    /// `reference`. Empty for a tab with no recorded placement. Set by the app
    /// from the engine; drawn in place of `reference`.
    pub designators: std::collections::HashMap<uuid::Uuid, String>,
    /// ERC violations to highlight on the canvas — Altium-style marker
    /// dots + primary-item halos. Synced from `ui_state.erc_violations`
    /// after each ERC run so the overlay renders without the canvas
    /// reaching back into app state.
    pub erc_markers: Vec<ErcMarker>,
    /// Armed net-color from the Active Bar palette. `Some(c)` with a
    /// non-zero alpha means the next wire click floods that colour
    /// onto the whole connected net; alpha 0 signals "clear one".
    /// Drives the pen cursor drawn over the canvas.
    pub pending_net_color: Option<signex_types::theme::Color>,
    /// In-flight lasso polygon in world space. Synced from
    /// `ui_state.lasso_polygon` so the overlay draw can render the
    /// committed vertices + rubber-band to the cursor without
    /// reaching into app state.
    pub lasso_polygon: Option<Vec<signex_types::schematic::Point>>,
    /// In-flight 3-click arc (start, mid) while Tool::Arc is active.
    /// Mirrors `interaction_state.arc_points` for the preview draw.
    pub arc_points: Vec<signex_types::schematic::Point>,
    /// In-flight polyline vertices while Tool::Polyline is active.
    /// Mirrors `interaction_state.polyline_points`.
    pub polyline_points: Vec<signex_types::schematic::Point>,
    /// When the BringToFrontOf / SendToBackOf picker is armed, show
    /// the gray-X placement cursor so the user knows the next click
    /// is a reference pick, not a selection. Synced from
    /// `ui_state.reorder_picker.is_some()` at the top of each view().
    pub reorder_picker_armed: bool,
    /// Armed Active Bar Move / Move Selection; mirrors `ui_state.move_pick`.
    pub move_pick: Option<MovePick>,
    /// Bumped to cancel any floating move started under an older value.
    pub move_epoch: u64,
    /// Two-click shape anchor + which shape is being drawn. Used by
    /// the rubber-band preview for Line / Rectangle / Circle.
    pub shape_anchor: Option<(signex_types::schematic::Point, ShapePreviewKind)>,
    /// Test-only: how often the drawn content was invalidated.
    #[cfg(test)]
    pub content_cache_clears: u32,
    /// Test-only: how often the overlay (selection, markers) was invalidated.
    #[cfg(test)]
    pub overlay_cache_clears: u32,
    /// Test-only: how often the grid background was invalidated.
    #[cfg(test)]
    pub bg_cache_clears: u32,
}

impl Default for CanvasSlot {
    fn default() -> Self {
        Self::new()
    }
}

impl CanvasSlot {
    pub fn active_render_cache(&self) -> Option<&crate::schematic_runtime::SchematicRenderCache> {
        self.render_cache.as_ref()
    }

    pub fn active_snapshot(&self) -> Option<&crate::schematic_runtime::SchematicRenderSnapshot> {
        self.render_cache.as_ref().map(|cache| cache.snapshot())
    }

    pub fn new() -> Self {
        Self {
            bg_cache: canvas::Cache::default(),
            content_cache: canvas::Cache::default(),
            overlay_cache: canvas::Cache::default(),
            content_cache_camera: std::cell::Cell::new((0.0, 0.0, 1.0)),
            content_scene: std::cell::RefCell::new(Default::default()),
            drag_layers: std::cell::RefCell::new(Default::default()),
            camera: std::cell::RefCell::new(Camera::default()),
            render_cache: None,
            selected: Vec::new(),
            pending_fit: std::cell::Cell::new(None),
            viewport: std::cell::Cell::new(Rectangle::with_size(iced::Size::ZERO)),
            wire_preview: Vec::new(),
            drawing_mode: false,
            tool_preview: None,
            ghost_label: None,
            ghost_symbol: None,
            ghost_text: None,
            ghost_bus_entry: None,
            placement_paused: false,
            paper_width_mm: 297.0,
            paper_height_mm: 210.0,
            sheet_frame: Vec::new(),
            designators: std::collections::HashMap::new(),
            erc_markers: Vec::new(),
            pending_net_color: None,
            lasso_polygon: None,
            arc_points: Vec::new(),
            polyline_points: Vec::new(),
            reorder_picker_armed: false,
            move_pick: None,
            move_epoch: 0,
            shape_anchor: None,
            #[cfg(test)]
            content_cache_clears: 0,
            #[cfg(test)]
            overlay_cache_clears: 0,
            #[cfg(test)]
            bg_cache_clears: 0,
        }
    }

    pub fn clear_overlay_cache(&mut self) {
        #[cfg(test)]
        {
            self.overlay_cache_clears += 1;
        }
        self.overlay_cache.clear();
    }

    pub fn clear_bg_cache(&mut self) {
        #[cfg(test)]
        {
            self.bg_cache_clears += 1;
        }
        self.bg_cache.clear();
    }

    pub fn clear_content_cache(&mut self) {
        #[cfg(test)]
        {
            self.content_cache_clears += 1;
        }
        self.content_cache.clear();
        self.content_scene.get_mut().invalidate();
        self.drag_layers.get_mut().clear();
    }

    /// Current pan/zoom as `(offset_x_px, offset_y_px, scale_px_per_mm)`,
    /// read in `view()` from the single [`Self::camera`] home to place
    /// world-anchored overlays (the inline text editor, measurements).
    /// Same shape as [`crate::pcb_canvas::PcbCanvas::live_camera`].
    pub fn live_camera(&self) -> (f32, f32, f32) {
        let camera = self.camera.borrow();
        (camera.offset.x, camera.offset.y, camera.scale)
    }

    /// Read-only borrow of the camera: for the `draw` path, held for one draw
    /// call, and for reads in `update` once its [`Self::camera_mut`] borrow
    /// has ended (the drag ghost's step). Never overlapped with `camera_mut`.
    pub(in crate::canvas) fn camera(&self) -> std::cell::Ref<'_, Camera> {
        self.camera.borrow()
    }

    /// Mutable borrow for the `update` path — pan, zoom and fit write here.
    pub(in crate::canvas) fn camera_mut(&self) -> std::cell::RefMut<'_, Camera> {
        self.camera.borrow_mut()
    }

    pub fn set_render_cache(
        &mut self,
        render_cache: Option<crate::schematic_runtime::SchematicRenderCache>,
    ) {
        self.render_cache = render_cache;
        self.content_scene.get_mut().invalidate();
    }

    #[cfg(test)]
    pub(crate) fn content_scene_for_test(
        &mut self,
    ) -> &mut crate::schematic_runtime::ContentSceneCache {
        self.content_scene.get_mut()
    }

    /// Record the canvas bounds (done on every canvas event; public so tests can
    /// stand in for a drawn frame).
    pub fn set_viewport(&self, bounds: Rectangle) {
        self.viewport.set(bounds);
    }

    /// World-space point (mm) under the centre of the last-recorded canvas
    /// viewport. Used to drop a part that was picked from a list rather than
    /// aimed at the canvas.
    pub fn view_centre_world(&self) -> (f64, f64) {
        let bounds = self.viewport.get();
        let centre = iced::Point::new(bounds.width / 2.0, bounds.height / 2.0);
        let world = self.camera.borrow().screen_to_world(centre, bounds);
        (f64::from(world.x), f64::from(world.y))
    }

    /// View > Zoom In/Out: one `step` notch about the centre; false at the limits.
    pub fn zoom_about_centre(&self, zoom_in: bool, step: f32) -> bool {
        let bounds = self.viewport.get();
        let centre = iced::Point::new(bounds.width / 2.0, bounds.height / 2.0);
        let notch = if zoom_in { 1.0 } else { -1.0 };
        self.camera_mut().zoom_at(centre, notch, step, bounds)
    }

    /// Recentre the view on world point `(x, y)` without changing zoom
    /// (View > Center View At Cursor, #525).
    pub fn center_on_world(&self, x: f32, y: f32) {
        let bounds = self.viewport.get();
        self.camera_mut().center_on(iced::Point::new(x, y), bounds);
    }

    /// Fit the camera to the extents of the sheet's objects (symbols, wires,
    /// labels, ...) plus a 10 mm margin.
    pub fn fit_to_content(&mut self) {
        if let Some(snapshot) = self.active_snapshot()
            && let Some(bounds) = snapshot.content_bounds()
        {
            self.pending_fit.set(Some(Rectangle::new(
                iced::Point::new(bounds.min_x as f32, bounds.min_y as f32),
                iced::Size::new(bounds.width() as f32, bounds.height() as f32),
            )));
        }
    }
}

/// Everything the schematic `draw` path needs that is **not** per-window
/// canvas state — settings, theme colours and app-state collections that
/// already have an owner in `UiState` / `DocumentState`.
///
/// #631 — these used to be fields on the canvas, written by scattered
/// `active_canvas_mut().x = …` assignments and kept equal to their real
/// owner by hand. They are now read straight from app state in `view`,
/// once per frame, so there is no second copy to drift. Scalars are
/// copied (they are `Copy` and cheaper to copy than to chase);
/// `wire_color_overrides` is borrowed, because cloning a `HashMap` every
/// frame is not a cost worth paying to avoid a lifetime.
///
/// Only settings with a single `UiState` owner that every window should
/// agree on live here. Per-window facts stay on [`CanvasSlot`] — the
/// window's paper size and render cache belong to *its* document, and
/// `erc_markers` / `lasso_polygon` / `pending_net_color` /
/// `reorder_picker_armed` are written to the active canvas alone today.
/// Promoting those would change what an undocked window renders, which
/// is not this change's job.
#[derive(Clone, Copy)]
pub struct CanvasViewPrefs<'a> {
    pub grid_visible: bool,
    pub theme_bg: Color,
    pub theme_grid: Color,
    pub theme_paper: Color,
    pub canvas_colors: signex_types::theme::CanvasColors,
    pub snap_enabled: bool,
    pub snap_grid_mm: f64,
    /// Wire clicks jump to the nearest pin end, wire end or junction (Shift+E).
    pub snap_hotspots: bool,
    /// The Wire tool is the active tool. Only wire clicks snap to hotspots
    /// (a Bus click snaps to the grid), so the hotspot marker and the
    /// rubber-band follow hotspots under this tool alone.
    pub wire_tool_active: bool,
    pub visible_grid_mm: f64,
    pub grid_style: crate::render_config::GridStyle,
    pub auto_focus: bool,
    pub zoom_step: f32,
    /// Preview rubber-banded wires while a selection floats (mirrors the
    /// "Keep wires connected when moving" preference).
    pub move_keeps_wires: bool,
    pub draw_mode: crate::app::DrawMode,
    pub wire_color_overrides: &'a std::collections::HashMap<uuid::Uuid, signex_types::theme::Color>,
    /// Resolved net-class style per wire and junction, read from app state
    /// so the canvas never reaches into the project. See
    /// [`crate::schematic_runtime::WireStyling`] for the precedence.
    pub wire_class_styles: &'a crate::schematic_runtime::WireClassStyles,
    /// The class-style cache's revision for `wire_class_styles`; moves when
    /// the map does.
    pub wire_class_revision: u64,
    pub selection_filters: &'a std::collections::HashSet<crate::active_bar::SelectionFilter>,
}

impl CanvasViewPrefs<'_> {
    /// The wire styling the renderer draws with: the per-net colour
    /// override over the net-class style.
    #[must_use]
    pub fn wire_styling(&self) -> crate::schematic_runtime::WireStyling<'_> {
        crate::schematic_runtime::WireStyling {
            color_overrides: self.wire_color_overrides,
            class_styles: self.wire_class_styles,
            class_revision: self.wire_class_revision,
        }
    }
}

/// The schematic `canvas::Program` — a per-frame *view* of the app state,
/// not an owner of it.
///
/// #631 — this used to be one long-lived struct in `InteractionState` with
/// 35 fields, about a dozen of them copies of `UiState` / `DocumentState`
/// data kept in sync across ~93 assignment sites with nothing enforcing
/// agreement. It is now constructed in `view` from two borrows: the
/// per-window [`CanvasSlot`] (the caches, camera and genuinely per-window
/// interaction state, which have no other home) and [`CanvasViewPrefs`]
/// (read from app state each frame). The library editors already had this
/// shape — `FootprintCanvas<'a>` and `SymbolCanvas<'a>` — so this is the
/// schematic canvas catching up, not a new pattern.
///
/// `Deref` to the slot is deliberate: it keeps every `self.selected`,
/// `self.ghost_symbol`, `self.camera()` in the draw and input modules
/// reading exactly as before, so the diff is the state that actually
/// moved rather than a mechanical prefix sweep over 200 field accesses.
pub struct SchematicCanvas<'a> {
    slot: &'a CanvasSlot,
    prefs: CanvasViewPrefs<'a>,
}

impl<'a> SchematicCanvas<'a> {
    /// What the content scene draws instead of the stored values.
    fn scene_overrides(&self) -> crate::schematic_runtime::SceneOverrides<'_> {
        crate::schematic_runtime::SceneOverrides {
            wire_styling: Some(self.prefs.wire_styling()),
            designators: Some(&self.slot.designators),
        }
    }

    /// The moving items of this drag frame and the ids the static layer
    /// leaves out; `None` when nothing is being dragged.
    fn drag_frame(
        &self,
        state: &CanvasState,
        drag_offset: Option<(f64, f64)>,
    ) -> Option<drag_layers::DragFrame> {
        let (dx, dy) = drag_offset?;
        let snap = self.active_snapshot()?;
        if self.selected.is_empty() {
            return None;
        }
        let revision = self.render_cache.as_ref().map_or(0, |c| c.revision());
        let mut layers = self.drag_layers.borrow_mut();
        let session = layers.session_for(snap, revision, &self.selected);
        if self.prefs.move_keeps_wires {
            let (gx, gy) = self.ghost_delta(dx, dy);
            if let Some(frame) = state.ghost.ready_frame(revision, &self.selected, gx, gy) {
                return Some(frame);
            }
        }
        Some(drag_layers::DragFrame {
            moving: std::rc::Rc::new(shift_snapshot_for_selection(
                &session.lifted,
                &self.selected,
                dx,
                dy,
            )),
            skip: session.ids.clone(),
        })
    }

    /// The snapped drag/float delta, while a move is in flight.
    fn drag_delta(&self, state: &CanvasState) -> Option<(f64, f64)> {
        if !state.move_dragging || self.float_is_stale(state) {
            return None;
        }
        state
            .move_origin
            .zip(state.move_current)
            .map(|((ox, oy), (cx, cy))| {
                drag_preview::snapped_move_delta(
                    self.active_snapshot(),
                    &self.selected,
                    cx - ox,
                    cy - oy,
                    self.prefs.snap_enabled,
                    self.prefs.snap_grid_mm,
                )
            })
    }

    /// Compute the rubber-banded ghost for the current drag delta, once per
    /// delta step: the pass is too heavy for `draw`, so it runs from `update`.
    /// With a very large selection the plain shift stands in instead.
    fn refresh_drag_ghost(&self, state: &mut CanvasState) {
        let Some((dx, dy)) = self.drag_delta(state) else {
            state.ghost.clear();
            return;
        };
        let too_big = self.selected.len() > drag_preview::MAX_RUBBER_BAND_GHOST_ITEMS;
        if !self.prefs.move_keeps_wires || self.selected.is_empty() || too_big {
            return;
        }
        if let (Some(snap), Some(cache)) = (self.active_snapshot(), self.render_cache.as_ref()) {
            let (gx, gy) = self.ghost_delta(dx, dy);
            state
                .ghost
                .refresh(snap, cache.revision(), &self.selected, gx, gy);
        }
    }

    /// The drag delta the rubber-banded ghost is computed and looked up at.
    fn ghost_delta(&self, dx: f64, dy: f64) -> (f64, f64) {
        drag_preview::ghost_delta(dx, dy, self.prefs.snap_enabled, self.camera().scale)
    }

    pub fn new(slot: &'a CanvasSlot, prefs: CanvasViewPrefs<'a>) -> Self {
        Self { slot, prefs }
    }

    /// Compute the "focus" uuid set when auto_focus is on — members of
    /// the current selection. Returns None when auto_focus is off; the
    /// renderer then draws every item at full alpha.
    ///
    /// #631 — lives on the borrowing view, not the slot: `auto_focus` is
    /// a `UiState` setting read through `prefs`, while `selected` is the
    /// window's own state reached through `Deref`.
    fn auto_focus_set(&self) -> Option<std::collections::HashSet<uuid::Uuid>> {
        if !self.prefs.auto_focus {
            return None;
        }
        let mut set = std::collections::HashSet::new();
        for item in &self.selected {
            set.insert(item.uuid);
        }
        Some(set)
    }
}

impl std::ops::Deref for SchematicCanvas<'_> {
    type Target = CanvasSlot;

    fn deref(&self) -> &Self::Target {
        self.slot
    }
}

impl canvas::Program<Message> for SchematicCanvas<'_> {
    type State = CanvasState;

    fn update(
        &self,
        state: &mut CanvasState,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        // Remember the bounds so menu zoom (no cursor) can find the view centre.
        self.viewport.set(bounds);
        // Consume any pending fit-to-content before dispatching the event.
        if let Some(action) = self.update_pending_fit(bounds) {
            return Some(action);
        }

        // Dispatch each input event to its handler, in the original order.
        match event {
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let action = self.update_wheel_scrolled(delta, bounds, cursor);
                // A zoom changes the ghost's step (`ghost_delta`); without
                // this the drag shows the plain shift until the cursor moves.
                self.refresh_drag_ghost(state);
                action
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                self.update_left_pressed(state, bounds, cursor)
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                self.update_left_released(state)
            }
            Event::Keyboard(iced::keyboard::Event::ModifiersChanged(mods)) => {
                self.update_modifiers_changed(state, mods)
            }
            Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                ..
            }) => self.update_escape_pressed(state),
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                self.update_right_pressed(state, bounds, cursor)
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Middle)) => {
                self.update_middle_pressed(state, bounds, cursor)
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Right)) => {
                self.update_right_released(state, bounds, cursor)
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Middle)) => {
                self.update_middle_released(state)
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let action = self.update_cursor_moved(state, bounds, cursor);
                self.refresh_drag_ghost(state);
                action
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        state: &CanvasState,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut layers = Vec::with_capacity(4);

        // Layer 1: background (grid + paper)
        layers.push(self.draw_background(renderer, bounds));

        let drag_offset = self.drag_delta(state);

        // A move-drag draws the sheet in two layers (#1335): the items the
        // drag leaves alone in a retained geometry, and each frame only the
        // moving items, shifted. With "keep wires" the moving items include
        // the rubber-banded wires computed when the cursor moved
        // (`refresh_drag_ghost`); until then the plain shift stands in.
        let drag_frame = self.drag_frame(state, drag_offset);
        let Some(drag_frame) = drag_frame else {
            if self.drag_layers.borrow().is_active() {
                self.drag_layers.borrow_mut().clear();
            }
            let snapshot = self.active_snapshot();

            // Layer 2: content (schematic elements)
            layers.push(self.draw_content(state, renderer, bounds, snapshot, drag_offset));

            // Layer 2.5: AutoFocus dim
            if let Some(dim) = self.draw_autofocus_dim(renderer, bounds, snapshot) {
                layers.push(dim);
            }

            // Layer 3: selection overlay
            if let Some(selection) = self.draw_selection(renderer, bounds, snapshot, drag_offset) {
                layers.push(selection);
            }

            // Layer 4: overlay (cursor HUD, previews, drag guides)
            layers.push(self.draw_overlay(state, renderer, bounds, cursor));
            return layers;
        };
        let moving = Some(&*drag_frame.moving);

        // Layer 2: content, static layer then moving items on top
        layers.extend(self.draw_drag_content(renderer, bounds, &drag_frame));

        // Layer 2.5: AutoFocus dim
        if let Some(dim) = self.draw_autofocus_dim(renderer, bounds, moving) {
            layers.push(dim);
        }

        // Layer 3: selection overlay
        if let Some(selection) = self.draw_selection(renderer, bounds, moving, drag_offset) {
            layers.push(selection);
        }

        // Layer 4: overlay (cursor HUD, previews, drag guides)
        layers.push(self.draw_overlay(state, renderer, bounds, cursor));

        layers
    }

    fn mouse_interaction(
        &self,
        state: &CanvasState,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        // Single cursor shape across every canvas mode: default arrow
        // for idle, native pan / move cursors while dragging. No
        // mode-specific shape swaps (the OS Crosshair is white + tiny
        // on Windows so invisible on yellow paper anyway). Visual
        // feedback for armed modes comes from overlay glyphs — the
        // net-colour pencil and the lasso polygon preview.
        if state.panning {
            mouse::Interaction::Grabbing
        } else if state.move_dragging {
            mouse::Interaction::Move
        } else {
            mouse::Interaction::default()
        }
    }
}

// ─── Canvas events sent to the app ────────────────────────────

#[derive(Debug, Clone)]
pub enum CanvasEvent {
    CursorAt {
        x: f32,
        y: f32,
        zoom_pct: f64,
    },
    CursorMoved,
    FitAll,
    /// Recentre on the last reported cursor position, keeping the zoom.
    CenterAtCursor,
    /// Left-click at world coordinates — triggers hit-testing or tool action.
    Clicked {
        world_x: f64,
        world_y: f64,
    },
    /// Ctrl+Left-click for multi-select.
    CtrlClicked {
        world_x: f64,
        world_y: f64,
    },
    /// Double-click at world coordinates.
    DoubleClicked {
        world_x: f64,
        world_y: f64,
        screen_x: f32,
        screen_y: f32,
    },
    /// Box selection — select all items within the rectangle (world coords).
    BoxSelect {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
    },
    /// Drag-move completed — move all selected items by delta (world coords).
    MoveSelected {
        dx: f64,
        dy: f64,
    },
    /// Click-click move picked up at this world point (#271).
    MovePicked {
        world_x: f64,
        world_y: f64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    static NO_CLASS_STYLES: std::sync::LazyLock<crate::schematic_runtime::WireClassStyles> =
        std::sync::LazyLock::new(Default::default);

    /// Minimal render settings for the tests below. `update_pending_fit`
    /// only touches the camera, so the values here just have to exist —
    /// #631 moved the method onto the borrowing view, which needs both
    /// halves to be constructed.
    pub(super) fn test_prefs<'a>(
        overrides: &'a std::collections::HashMap<uuid::Uuid, signex_types::theme::Color>,
        filters: &'a std::collections::HashSet<crate::active_bar::SelectionFilter>,
    ) -> CanvasViewPrefs<'a> {
        test_prefs_styled(overrides, filters, &NO_CLASS_STYLES)
    }

    /// [`test_prefs`] with a net-class style map, for the tests of what the
    /// canvas does with one (hit areas).
    pub(super) fn test_prefs_styled<'a>(
        overrides: &'a std::collections::HashMap<uuid::Uuid, signex_types::theme::Color>,
        filters: &'a std::collections::HashSet<crate::active_bar::SelectionFilter>,
        class_styles: &'a crate::schematic_runtime::WireClassStyles,
    ) -> CanvasViewPrefs<'a> {
        let colors = signex_types::theme::canvas_colors(signex_types::theme::ThemeId::Signex);
        CanvasViewPrefs {
            grid_visible: true,
            theme_bg: crate::render_config::to_iced(&colors.background),
            theme_grid: crate::render_config::to_iced(&colors.grid),
            theme_paper: crate::render_config::to_iced(&colors.paper),
            canvas_colors: colors,
            snap_enabled: true,
            snap_grid_mm: 1.27,
            snap_hotspots: false,
            wire_tool_active: false,
            visible_grid_mm: 1.27,
            grid_style: crate::render_config::GridStyle::Dots,
            auto_focus: false,
            zoom_step: 1.1,
            move_keeps_wires: true,
            draw_mode: crate::app::DrawMode::Ortho90,
            wire_color_overrides: overrides,
            wire_class_styles: class_styles,
            wire_class_revision: 0,
            selection_filters: filters,
        }
    }

    /// #632 regression. The camera used to live in `CanvasState` (the
    /// `Program::State`) and be republished into a
    /// `live_camera: Cell<(f32, f32, f32)>` from `draw`, because `view(&self)`
    /// cannot reach a `Program::State`. Two homes, one of them refreshed only
    /// when a frame was drawn. Mutating the camera the way `Program::update`
    /// does must now be visible through `live_camera()` immediately — there is
    /// no second copy that could lag. Mirrors the same guard on `PcbCanvas`.
    #[test]
    fn live_camera_reflects_the_single_source_after_a_mutation() {
        // Arrange
        let canvas = CanvasSlot::new();
        let (x0, y0, s0) = canvas.live_camera();

        // Act — what `update_cursor_moved` does on a pan drag.
        canvas.camera_mut().pan(12.0, -7.0);

        // Assert
        let (x1, y1, s1) = canvas.live_camera();
        assert_eq!(x1, x0 + 12.0);
        assert_eq!(y1, y0 - 7.0);
        assert_eq!(s1, s0, "pan must not change scale");
    }

    /// #1333 — ERC markers share layer 3 with the selection overlay, so the
    /// layer must draw when there are markers and nothing is selected.
    #[test]
    fn erc_markers_draw_with_nothing_selected() {
        let mut canvas = CanvasSlot::new();
        let (overrides, filters) = Default::default();
        assert!(
            !SchematicCanvas::new(&canvas, test_prefs(&overrides, &filters))
                .selection_layer_needed(),
            "nothing selected and no markers: the layer has nothing to draw"
        );
        canvas.erc_markers.push(ErcMarker {
            x: 10.0,
            y: 10.0,
            severity: ErcMarkerSeverity::Error,
        });
        let view = SchematicCanvas::new(&canvas, test_prefs(&overrides, &filters));
        assert!(view.selected.is_empty());
        assert!(
            view.selection_layer_needed(),
            "an ERC marker must be drawn even when nothing is selected"
        );
        // Pin the draw path too, not only the predicate it consults.
        let sheet = signex_types::schematic::SchematicSheet::default();
        let bounds = Rectangle::new(iced::Point::ORIGIN, iced::Size::new(400.0, 300.0));
        assert!(
            view.draw_selection(
                &crate::layout_probe::headless_renderer(),
                bounds,
                Some(&sheet),
                None
            )
            .is_some(),
            "the selection layer must produce geometry for the ERC marker"
        );
    }

    /// #632 — a fit request used to travel `CanvasSlot::pending_fit` →
    /// `CanvasState::pending_fit` → camera, two `take()`s deep, because the
    /// camera lived in the state the first hop was reaching. With one camera
    /// home the middle hop has no reader, so the request must be consumed and
    /// applied in a single `update_pending_fit` call.
    #[test]
    fn a_pending_fit_reaches_the_camera_in_one_hop() {
        // Arrange
        let canvas = CanvasSlot::new();
        let before = canvas.live_camera();
        canvas.pending_fit.set(Some(Rectangle::new(
            iced::Point::new(0.0, 0.0),
            iced::Size::new(100.0, 50.0),
        )));

        // Act
        let (overrides, filters) = Default::default();
        let view = SchematicCanvas::new(&canvas, test_prefs(&overrides, &filters));
        let action = view.update_pending_fit(Rectangle::new(
            iced::Point::new(0.0, 0.0),
            iced::Size::new(800.0, 600.0),
        ));

        // Assert
        assert!(
            action.is_some(),
            "consuming a fit must publish a redraw, or the new camera never reaches the screen"
        );
        assert!(
            canvas.pending_fit.get().is_none(),
            "the request must be consumed, not left to re-fire on the next event"
        );
        assert_ne!(
            canvas.live_camera(),
            before,
            "the fit must land on the camera `view` and `draw` read"
        );
    }

    /// A second consecutive call has nothing to consume. Before #632 the
    /// second `take()` (from `CanvasState`) made this ambiguous; now the
    /// absence of a pending request must simply be a no-op, so a fit does not
    /// keep re-applying and fighting the user's pan on every later event.
    #[test]
    fn a_consumed_fit_does_not_re_apply() {
        // Arrange
        let canvas = CanvasSlot::new();
        let bounds = Rectangle::new(iced::Point::new(0.0, 0.0), iced::Size::new(800.0, 600.0));
        canvas.pending_fit.set(Some(Rectangle::new(
            iced::Point::new(0.0, 0.0),
            iced::Size::new(100.0, 50.0),
        )));
        let (overrides, filters) = Default::default();
        let view = SchematicCanvas::new(&canvas, test_prefs(&overrides, &filters));
        let _ = view.update_pending_fit(bounds);
        let after_fit = canvas.live_camera();

        // Act — the user pans, then another event arrives.
        canvas.camera_mut().pan(5.0, 5.0);
        let action = view.update_pending_fit(bounds);

        // Assert
        assert!(action.is_none(), "no pending request, no action");
        assert_eq!(
            canvas.live_camera(),
            (after_fit.0 + 5.0, after_fit.1 + 5.0, after_fit.2),
            "the pan must survive — a stale fit must not snap the camera back"
        );
    }
}
