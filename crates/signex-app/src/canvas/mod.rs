//! Schematic/PCB canvas — wgpu rendering with Altium-style pan/zoom/grid.
//!
//! Uses `iced::widget::canvas::Program` with a 3-layer cache:
//! - background: grid, sheet border (cleared on theme/grid/zoom change)
//! - content: schematic elements (cleared on document edit)
//! - overlay: selection, cursor, wire-in-progress (cleared every frame)

mod camera;
mod draw;
pub mod grid;
mod input;

use iced::event::Event;
use iced::mouse;
use iced::widget::canvas;
use iced::{Color, Rectangle, Renderer, Theme};

pub use camera::Camera;
pub use grid::GridState;

#[allow(deprecated)]
use crate::schematic_runtime::SchematicSheetExt as _;

use crate::app::{ContextMenuMsg, Message};
use crate::toolbar::ToolMessage;

// ─── Canvas State (per-canvas mutable state) ──────────────────

#[derive(Debug, Default)]
pub struct CanvasState {
    pub camera: Camera,
    pub _grid: GridState,
    /// Is the user currently panning (right-click or middle-click drag)?
    panning: bool,
    /// Whether actual pan movement occurred (to distinguish click from drag).
    pan_moved: bool,
    /// Last cursor position during a pan (in screen pixels).
    last_pan_pos: Option<iced::Point>,
    /// Pending fit target — consumed on next update.
    pub pending_fit: Option<Rectangle>,
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
    // ─── Double-click detection ───
    /// Timestamp of last left-click (for double-click detection).
    last_click_time: Option<std::time::Instant>,
    /// World position of last left-click.
    last_click_world: Option<(f64, f64)>,
}

// ─── SchematicCanvas (the Program) ────────────────────────────

/// The canvas program that handles input and rendering.
/// Holds references to app state needed for drawing (theme colors, etc).
pub struct SchematicCanvas {
    pub bg_cache: canvas::Cache,
    pub content_cache: canvas::Cache,
    pub overlay_cache: canvas::Cache,
    /// Camera state when content_cache was last built — used to compute offset delta.
    pub content_cache_camera: std::cell::Cell<(f32, f32, f32)>, // (offset_x, offset_y, scale)
    /// Camera state as of the most recent draw, updated every frame including
    /// mid-pan. Overlays positioned relative to world coordinates (inline text
    /// editor, measurements) read this so they track pan/zoom in real time.
    pub live_camera: std::cell::Cell<(f32, f32, f32)>,
    pub grid_visible: bool,
    pub theme_bg: Color,
    pub theme_grid: Color,
    pub theme_paper: Color,
    pub canvas_colors: signex_types::theme::CanvasColors,
    /// Render-facing cache of the currently visible schematic.
    /// The app updates this from the active engine or active tab cache.
    pub render_cache: Option<crate::schematic_runtime::SchematicRenderCache>,
    /// Currently selected items — drives selection overlay rendering.
    pub selected: Vec<signex_types::schematic::SelectedItem>,
    /// Pending fit target to transfer to CanvasState.
    /// Uses Cell so canvas::Program::update (&self) can consume it.
    pub pending_fit: std::cell::Cell<Option<Rectangle>>,
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
    /// When true, placement is paused (TAB pressed → pre-placement form
    /// open). The ghost freezes and canvas clicks don't place — the user
    /// interacts with the Properties panel until they confirm with OK.
    pub placement_paused: bool,
    /// Current draw mode for wire preview constraint (90°, 45°, free).
    pub draw_mode: crate::app::DrawMode,
    /// Whether snap-to-grid is enabled (for rubber-band cursor snapping).
    pub snap_enabled: bool,
    /// Grid size in mm for rubber-band cursor snapping AND visible grid rendering.
    pub snap_grid_mm: f64,
    /// Visible grid dot spacing in mm (independent of snap grid).
    pub visible_grid_mm: f64,
    /// Active paper width in mm (world units).
    pub paper_width_mm: f32,
    /// Active paper height in mm (world units).
    pub paper_height_mm: f32,
    /// When true, non-selected items dim on the canvas (F9). Synced
    /// from `ui_state.auto_focus` so the renderer can compute a focus
    /// uuid set without reaching back into app state.
    pub auto_focus: bool,
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
    /// Per-wire colour overrides consulted when drawing wires. Synced
    /// from `ui_state.wire_color_overrides` on every canvas rebuild.
    pub wire_color_overrides: std::collections::HashMap<uuid::Uuid, signex_types::theme::Color>,
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
    /// Two-click shape anchor + which shape is being drawn. Used by
    /// the rubber-band preview for Line / Rectangle / Circle.
    pub shape_anchor: Option<(signex_types::schematic::Point, ShapePreviewKind)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapePreviewKind {
    Line,
    Rect,
    Circle,
}

/// Canvas-side projection of an ERC violation — just enough to draw
/// its marker without pulling the full Violation type into the render
/// crate.
#[derive(Debug, Clone)]
pub struct ErcMarker {
    pub x: f64,
    pub y: f64,
    pub severity: ErcMarkerSeverity,
    /// Uuid of the primary offending item (label, wire, symbol, …).
    /// Reserved for a future halo/highlight pass; unused today.
    #[allow(dead_code)]
    pub primary_uuid: Option<uuid::Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErcMarkerSeverity {
    Error,
    Warning,
    Info,
}

impl SchematicCanvas {
    pub fn active_render_cache(&self) -> Option<&crate::schematic_runtime::SchematicRenderCache> {
        self.render_cache.as_ref()
    }

    pub fn active_snapshot(&self) -> Option<&crate::schematic_runtime::SchematicRenderSnapshot> {
        self.render_cache.as_ref().map(|cache| cache.snapshot())
    }

    pub fn new() -> Self {
        let default_colors =
            signex_types::theme::canvas_colors(signex_types::theme::ThemeId::Signex);
        Self {
            bg_cache: canvas::Cache::default(),
            content_cache: canvas::Cache::default(),
            overlay_cache: canvas::Cache::default(),
            content_cache_camera: std::cell::Cell::new((0.0, 0.0, 1.0)),
            live_camera: std::cell::Cell::new((0.0, 0.0, 1.0)),
            grid_visible: true,
            theme_bg: {
                let c = &default_colors.background;
                Color::from_rgb8(c.r, c.g, c.b)
            },
            theme_grid: {
                let c = &default_colors.grid;
                Color::from_rgb8(c.r, c.g, c.b)
            },
            theme_paper: {
                let c = &default_colors.paper;
                Color::from_rgb8(c.r, c.g, c.b)
            },
            canvas_colors: default_colors,
            render_cache: None,
            selected: Vec::new(),
            pending_fit: std::cell::Cell::new(None),
            wire_preview: Vec::new(),
            drawing_mode: false,
            tool_preview: None,
            ghost_label: None,
            ghost_symbol: None,
            ghost_text: None,
            placement_paused: false,
            draw_mode: crate::app::DrawMode::Ortho90,
            snap_enabled: true,
            // Altium default is 1.27 mm (50 mil); also matches Standard's default schematic grid step.
            snap_grid_mm: 1.27,
            visible_grid_mm: 1.27,
            paper_width_mm: 297.0,
            paper_height_mm: 210.0,
            auto_focus: false,
            erc_markers: Vec::new(),
            pending_net_color: None,
            wire_color_overrides: std::collections::HashMap::new(),
            lasso_polygon: None,
            arc_points: Vec::new(),
            polyline_points: Vec::new(),
            reorder_picker_armed: false,
            shape_anchor: None,
        }
    }

    /// Compute the "focus" uuid set when auto_focus is on — members of
    /// the current selection. Returns None when auto_focus is off; the
    /// renderer then draws every item at full alpha.
    fn auto_focus_set(&self) -> Option<std::collections::HashSet<uuid::Uuid>> {
        if !self.auto_focus {
            return None;
        }
        let mut set = std::collections::HashSet::new();
        for item in &self.selected {
            set.insert(item.uuid);
        }
        Some(set)
    }

    pub fn clear_overlay_cache(&mut self) {
        self.overlay_cache.clear();
    }

    pub fn clear_bg_cache(&mut self) {
        self.bg_cache.clear();
    }

    pub fn clear_content_cache(&mut self) {
        self.content_cache.clear();
    }

    pub fn set_render_cache(
        &mut self,
        render_cache: Option<crate::schematic_runtime::SchematicRenderCache>,
    ) {
        self.render_cache = render_cache;
    }

    /// Fit the camera to show the schematic content.
    pub fn fit_to_paper(&mut self) {
        if let Some(snapshot) = self.active_snapshot()
            && let Some(bounds) = snapshot.content_bounds()
        {
            self.pending_fit.set(Some(Rectangle::new(
                iced::Point::new(bounds.min_x as f32, bounds.min_y as f32),
                iced::Size::new(bounds.width() as f32, bounds.height() as f32),
            )));
        }
    }

    pub fn set_theme_colors(&mut self, bg: Color, grid: Color, paper: Color) {
        self.theme_bg = bg;
        self.theme_grid = grid;
        self.theme_paper = paper;
        self.bg_cache.clear();
    }
}

impl canvas::Program<Message> for SchematicCanvas {
    type State = CanvasState;

    fn update(
        &self,
        state: &mut CanvasState,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        // Consume any pending fit-to-content before dispatching the event.
        if let Some(action) = self.update_pending_fit(state, bounds) {
            return Some(action);
        }

        // Dispatch each input event to its handler, in the original order.
        match event {
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                self.update_wheel_scrolled(state, delta, bounds, cursor)
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
                self.update_cursor_moved(state, bounds, cursor)
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
        layers.push(self.draw_background(state, renderer, bounds));

        // Shared drag/selection snapshot prep — the shifted snapshot is owned
        // here so the content, auto-focus dim, and selection layers can all
        // borrow the same effective snapshot.
        // If mid-drag, build a snapshot with selected items shifted so the
        // "move" visually lifts the originals out of place and places them at
        // the cursor — matches Altium's behavior where the dragged objects
        // travel with the mouse and nothing stays behind at the old location.
        let drag_offset = if state.move_dragging {
            state
                .move_origin
                .zip(state.move_current)
                .map(|((ox, oy), (cx, cy))| (cx - ox, cy - oy))
        } else {
            None
        };

        let shifted_snapshot =
            if let (Some((dx, dy)), Some(snap)) = (drag_offset, self.active_snapshot()) {
                if !self.selected.is_empty() {
                    Some(shift_snapshot_for_selection(snap, &self.selected, dx, dy))
                } else {
                    None
                }
            } else {
                None
            };
        let effective_snapshot: Option<&crate::schematic_runtime::SchematicRenderSnapshot> =
            shifted_snapshot.as_ref().or_else(|| self.active_snapshot());

        // Layer 2: content (schematic elements)
        layers.push(self.draw_content(state, renderer, bounds, effective_snapshot, drag_offset));

        // Layer 2.5: AutoFocus dim
        if let Some(dim) = self.draw_autofocus_dim(state, renderer, bounds, effective_snapshot) {
            layers.push(dim);
        }

        // Layer 3: selection overlay
        if let Some(selection) =
            self.draw_selection(state, renderer, bounds, effective_snapshot, drag_offset)
        {
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

// ─── Active Bar hit detection for right-click ────────────────

/// Map a relative x position within the Active Bar to a dropdown menu.
/// Returns None for buttons without dropdowns (Select, Add Component).
/// Clone the snapshot and translate the world position of every item that
/// appears in `selection` by `(dx, dy)`. Used during drag-to-move so the live
/// render shows selected objects at the cursor position while the originals
/// are not drawn at their pre-drag location.
fn shift_snapshot_for_selection(
    snap: &crate::schematic_runtime::SchematicRenderSnapshot,
    selection: &[signex_types::schematic::SelectedItem],
    dx: f64,
    dy: f64,
) -> crate::schematic_runtime::SchematicRenderSnapshot {
    use signex_types::schematic::{Point, SelectedKind};

    let is_selected = |uuid: uuid::Uuid, kind: SelectedKind| -> bool {
        selection.iter().any(|s| s.uuid == uuid && s.kind == kind)
    };
    let shift = |p: Point| -> Point { Point::new(p.x + dx, p.y + dy) };

    let mut out = snap.clone();
    for w in out.wires.iter_mut() {
        if is_selected(w.uuid, SelectedKind::Wire) {
            w.start = shift(w.start);
            w.end = shift(w.end);
        }
    }
    for b in out.buses.iter_mut() {
        if is_selected(b.uuid, SelectedKind::Bus) {
            b.start = shift(b.start);
            b.end = shift(b.end);
        }
    }
    for be in out.bus_entries.iter_mut() {
        if is_selected(be.uuid, SelectedKind::BusEntry) {
            be.position = shift(be.position);
        }
    }
    for j in out.junctions.iter_mut() {
        if is_selected(j.uuid, SelectedKind::Junction) {
            j.position = shift(j.position);
        }
    }
    for nc in out.no_connects.iter_mut() {
        if is_selected(nc.uuid, SelectedKind::NoConnect) {
            nc.position = shift(nc.position);
        }
    }
    for l in out.labels.iter_mut() {
        if is_selected(l.uuid, SelectedKind::Label) {
            l.position = shift(l.position);
        }
    }
    for tn in out.text_notes.iter_mut() {
        if is_selected(tn.uuid, SelectedKind::TextNote) {
            tn.position = shift(tn.position);
        }
    }
    for sym in out.symbols.iter_mut() {
        if is_selected(sym.uuid, SelectedKind::Symbol) {
            // Whole-symbol drag: anchor + both fields travel together.
            sym.position = shift(sym.position);
            if let Some(rt) = &mut sym.ref_text {
                rt.position = shift(rt.position);
            }
            if let Some(vt) = &mut sym.val_text {
                vt.position = shift(vt.position);
            }
        } else {
            // Field-only drag: shift just the selected field; the symbol
            // body stays put so the user can reposition the label
            // independently (Altium convention).
            if is_selected(sym.uuid, SelectedKind::SymbolRefField)
                && let Some(rt) = &mut sym.ref_text
            {
                rt.position = shift(rt.position);
            }
            if is_selected(sym.uuid, SelectedKind::SymbolValField)
                && let Some(vt) = &mut sym.val_text
            {
                vt.position = shift(vt.position);
            }
        }
    }
    for cs in out.child_sheets.iter_mut() {
        if is_selected(cs.uuid, SelectedKind::ChildSheet) {
            cs.position = shift(cs.position);
            for pin in &mut cs.pins {
                pin.position = shift(pin.position);
            }
        } else {
            for pin in &mut cs.pins {
                if is_selected(pin.uuid, SelectedKind::SheetPin) {
                    pin.position = shift(pin.position);
                }
            }
        }
    }
    use signex_types::schematic::SchDrawing;
    for d in out.drawings.iter_mut() {
        let uuid = match d {
            SchDrawing::Line { uuid, .. }
            | SchDrawing::Rect { uuid, .. }
            | SchDrawing::Circle { uuid, .. }
            | SchDrawing::Arc { uuid, .. }
            | SchDrawing::Polyline { uuid, .. } => *uuid,
        };
        if !is_selected(uuid, SelectedKind::Drawing) {
            continue;
        }
        match d {
            SchDrawing::Line { start, end, .. } => {
                *start = shift(*start);
                *end = shift(*end);
            }
            SchDrawing::Rect { start, end, .. } => {
                *start = shift(*start);
                *end = shift(*end);
            }
            SchDrawing::Circle { center, .. } => {
                *center = shift(*center);
            }
            SchDrawing::Arc {
                start, mid, end, ..
            } => {
                *start = shift(*start);
                *mid = shift(*mid);
                *end = shift(*end);
            }
            SchDrawing::Polyline { points, .. } => {
                for p in points {
                    *p = shift(*p);
                }
            }
        }
    }
    out
}

fn active_bar_hit(x: f32) -> Option<crate::active_bar::ActiveBarMenu> {
    use crate::active_bar::ActiveBarMenu;
    // Each btn=29px (28 cell + 1 spacing), sep=2px, pad=4px.
    // [Filter][Move] | [Select][Align] | [Wire][Power] | [Harness][Sheet][Port][Dir] | [Text][Shapes][NetColor]
    let x = x - 4.0;
    let b = 29; // button width (cell + spacing)
    let s = 2; // separator
    let xi = x as i32;
    if xi < 0 {
        return None;
    }
    // btn 0: Filter
    if xi < b {
        return Some(ActiveBarMenu::Filter);
    }
    // btn 1: Move
    if xi < 2 * b {
        return Some(ActiveBarMenu::Select);
    }
    // sep
    let off = 2 * b + s;
    // btn 2: Select
    if xi >= off && xi < off + b {
        return Some(ActiveBarMenu::SelectMode);
    }
    // btn 3: Align
    if xi >= off + b && xi < off + 2 * b {
        return Some(ActiveBarMenu::Align);
    }
    // sep
    let off = off + 2 * b + s;
    // btn 4: Wire
    if xi >= off && xi < off + b {
        return Some(ActiveBarMenu::Wiring);
    }
    // btn 5: Power
    if xi >= off + b && xi < off + 2 * b {
        return Some(ActiveBarMenu::Power);
    }
    // sep
    let off = off + 2 * b + s;
    // btn 6: Harness
    if xi >= off && xi < off + b {
        return Some(ActiveBarMenu::Harness);
    }
    // btn 7: Sheet Symbol
    if xi >= off + b && xi < off + 2 * b {
        return Some(ActiveBarMenu::SheetSymbol);
    }
    // btn 8: Port
    if xi >= off + 2 * b && xi < off + 3 * b {
        return Some(ActiveBarMenu::Port);
    }
    // btn 9: Directives
    if xi >= off + 3 * b && xi < off + 4 * b {
        return Some(ActiveBarMenu::Directives);
    }
    // sep
    let off = off + 4 * b + s;
    // btn 10: Text
    if xi >= off && xi < off + b {
        return Some(ActiveBarMenu::TextTools);
    }
    // btn 11: Shapes
    if xi >= off + b && xi < off + 2 * b {
        return Some(ActiveBarMenu::Shapes);
    }
    // btn 12: Net Color
    if xi >= off + 2 * b && xi < off + 3 * b {
        return Some(ActiveBarMenu::NetColor);
    }
    None
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
    #[allow(dead_code)]
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
}
