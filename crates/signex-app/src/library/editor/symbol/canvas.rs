//! Symbol-tab interactive canvas.
//!
//! The canvas reads the typed [`signex_library::Symbol`] primitive
//! directly. The body rectangle is derived from `Symbol.graphics`
//! (first `Rectangle` graphic), or defaults to a
//! `[-5.08, -2.54] .. [5.08, 2.54]` rectangle when the primitive
//! carries no body geometry yet.
//!
//! World-space convention mirrors the schematic editor: Standard y-axis
//! (positive going up; on screen y goes down so we flip). The
//! camera ([`crate::canvas::Camera`]) handles pan/zoom; the user
//! pans with right- or middle-button drag and zooms with the wheel.
//! Press Home (or click the Fit button) to fit the symbol bbox to
//! the viewport — also the implicit state on tab open.
//!
//! Background colour, grid size + visibility, snap, and the cursor
//! coordinate readout follow the same Altium-parity surface as the
//! schematic canvas: bg + grid colour come from the active theme's
//! `CanvasColors`; grid spacing follows `panel_ctx.grid_size_mm`;
//! the unit ([`signex_types::coord::Unit`]) drives the status
//! footer. Sheet colour is per-tab (Altium "Document Options")
//! and shifts the bg fill alpha so the user can pick Black / White
//! / Dark Gray / Light Gray / Cream per-symbol library.

use iced::Color;
use iced::Rectangle;
use iced::Renderer;
use iced::Size;
use iced::Theme;
use iced::event::Event;
use iced::mouse;
use iced::widget::canvas;
use signex_gfx::scene::{DirtyFlags, Scene};
use signex_library::{Symbol, SymbolGraphicKind, SymbolPin};
use signex_renderer::schematic::{
    ArcInput, JunctionInput, OverlayInputs, PolygonInput, SchematicRenderer,
    SchematicSnapshot as RendererSnapshot, ViewRenderer, WireInput,
};
use signex_renderer::theme::ResolvedTheme;
use signex_types::schematic::{HAlign, VAlign};
use std::collections::HashMap;

use super::state::{self, GraphicHandle, SymbolSelection};

/// The actions a [`SymbolCanvas`] can emit upward.
#[derive(Debug, Clone, Copy)]
pub enum CanvasAction {
    AddPin {
        x: f64,
        y: f64,
    },
    /// Stamp a default-sized rectangle (10 × 5 mm) centred on
    /// `(x, y)`. Drag-to-resize lands in a follow-up — for the
    /// first cut the rectangle is committed in one click and
    /// the user can later edit the corners via the Properties
    /// panel (or move/delete via the Select tool).
    AddRectangle {
        x: f64,
        y: f64,
    },
    /// Stamp a 5 mm horizontal line starting at `(x, y)` going
    /// right.
    AddLine {
        x: f64,
        y: f64,
    },
    /// Stamp a circle of radius 2 mm centred on `(x, y)`.
    AddCircle {
        x: f64,
        y: f64,
    },
    /// Stamp a default 2 mm-radius arc centred on `(x, y)` sweeping
    /// 0°→90° (quadrant arc).
    AddArc {
        x: f64,
        y: f64,
    },
    /// Stamp a default text label "Text" anchored at `(x, y)`.
    AddText {
        x: f64,
        y: f64,
    },
    Select(SymbolSelection),
    Deselect,
    Move {
        x: f64,
        y: f64,
    },
    /// Drag-to-resize a graphic handle. Fired continuously while the
    /// user drags the handle of a placed graphic in the Select tool.
    MoveGraphicHandle {
        idx: usize,
        handle: GraphicHandle,
        x: f64,
        y: f64,
    },
    RotateSelected {
        clockwise: bool,
        pivot_mode: RotatePivotMode,
    },
    DeleteSelected,
    // ── View / camera ──
    /// Pan the camera by `(dx, dy)` screen pixels. Fired by right-
    /// or middle-button drag.
    Pan {
        dx: f32,
        dy: f32,
    },
    /// Zoom centred on `(sx, sy)` (canvas-local pixels). Positive
    /// `delta` zooms in.
    Zoom {
        sx: f32,
        sy: f32,
        delta: f32,
    },
    /// Fit the symbol bbox into the viewport (Home key).
    Fit,
    /// Cursor world position update — drives the status footer.
    /// `None` clears the readout when the cursor leaves bounds.
    CursorAt {
        x_mm: Option<f64>,
        y_mm: Option<f64>,
    },
}

/// Pivot mode carried by rotate actions emitted from the Symbol canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotatePivotMode {
    WorldOrigin,
    GeometryCenter,
}

/// Canvas tools — Altium-style `Tool` enum scoped to this surface.
/// Mirrors the SchLib Place menu: Pin / Line / Rectangle / Ellipse
/// (Circle) / Arc / Text are the working tools; `Polygon` /
/// `RoundRectangle` / `Bezier` / `Image` etc. live on the Active
/// Bar as stubs and are deferred to v0.9.x.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolTool {
    Select,
    AddPin,
    PlaceRectangle,
    PlaceLine,
    PlaceCircle,
    PlaceArc,
    PlaceText,
}

impl SymbolTool {
    #[allow(dead_code)]
    pub fn label(self) -> &'static str {
        match self {
            SymbolTool::Select => "Select",
            SymbolTool::AddPin => "Add Pin",
            SymbolTool::PlaceRectangle => "Rectangle",
            SymbolTool::PlaceLine => "Line",
            SymbolTool::PlaceCircle => "Ellipse",
            SymbolTool::PlaceArc => "Arc",
            SymbolTool::PlaceText => "Text",
        }
    }
}

/// Canvas-program ephemeral state — drag + pan tracking.
#[derive(Debug, Default)]
pub struct CanvasState {
    /// True when the user is mid-drag of the currently-selected pin.
    pub dragging: bool,
    /// `(graphic_idx, handle)` while the user drags a graphic resize
    /// handle. `None` outside of a handle drag. Mutually exclusive
    /// with `dragging` — a click either lands on a pin or on a
    /// graphic handle, never both.
    pub dragging_handle: Option<(usize, GraphicHandle)>,
    /// Anchor offset (anchor - cursor) captured on drag start so
    /// selected items keep their click point while moving.
    pub drag_anchor_offset: Option<(f64, f64)>,
    /// True while the user holds right- or middle-button to pan.
    pub panning: bool,
    /// Last cursor screen position during a pan, used to compute
    /// per-frame deltas.
    pub last_pan_pos: Option<iced::Point>,
}

/// Builder for the per-render [`SymbolCanvas`] — all the inputs the
/// canvas needs from the surrounding state. The canvas itself is
/// constructed fresh on every iced view tick (see
/// `library/editor/standalone.rs::view_symbol_canvas`).
pub struct SymbolCanvas<'a> {
    pub symbol: &'a Symbol,
    pub selected: Option<SymbolSelection>,
    pub tool: SymbolTool,
    /// Active sub-part the canvas is filtering pins for. Pins with
    /// `part_number == 0` (Part Zero) render on every part; pins
    /// with `part_number == active_part` render on the active part
    /// only. Defaults to `1` (single-part components).
    pub active_part: u8,
    /// Pan/zoom state owned by the editor tab — see
    /// [`crate::app::SymbolEditorState::camera`].
    pub camera: &'a crate::canvas::Camera,
    /// Visible grid spacing in mm — sourced from
    /// `panel_ctx.grid_size_mm` so the schematic + library editors
    /// share the global grid setting.
    pub grid_size_mm: f64,
    /// Whether the grid is rendered. Sourced from
    /// `panel_ctx.grid_visible` (View ▸ Toggle Grid / status-bar
    /// click).
    pub grid_visible: bool,
    pub bg_color: Color,
    pub grid_color: Color,
    pub body_color: Color,
    pub pin_color: Color,
    pub selected_color: Color,
    pub text_color: Color,
    /// Stroke color for the X/Y axis lines through world (0, 0) —
    /// Altium-style "centre crosshair" so the symbol's anchor is
    /// always visible.
    pub axis_color: Color,
}

impl<'a> SymbolCanvas<'a> {
    /// Construct the per-frame canvas with the inputs from
    /// `SymbolEditorState` + the active theme + global grid/unit
    /// settings. See module-level docs for the parity rationale.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        symbol: &'a Symbol,
        selected: Option<SymbolSelection>,
        tool: SymbolTool,
        active_part: u8,
        camera: &'a crate::canvas::Camera,
        grid_size_mm: f64,
        grid_visible: bool,
        sheet_color: Color,
        accent_color: Color,
        _body_color_unused: Color,
        _text_color_unused: Color,
        _grid_color_unused: Color,
    ) -> Self {
        // Pick canvas-content colours based on sheet luminance —
        // Altium-style. Light sheets (Cream/White/LightGray) want
        // dark body strokes + black text; dark sheets keep the
        // signature yellow body + light text. Theme-text colours
        // are ignored — they're tuned for the surrounding panel
        // chrome and read as washed-out on a Cream sheet.
        let palette = SymbolPalette::for_sheet(sheet_color);
        Self {
            symbol,
            selected,
            tool,
            active_part,
            camera,
            grid_size_mm,
            grid_visible,
            bg_color: sheet_color,
            grid_color: palette.grid,
            body_color: palette.body,
            pin_color: palette.pin,
            selected_color: accent_color,
            text_color: palette.text,
            axis_color: palette.axis,
        }
    }

    /// True when `pin` should render on the currently-active part.
    /// Part Zero (`part_number == 0`) appears on every part; other
    /// pins only render when they match `active_part`.
    fn pin_visible_on_active_part(&self, pin: &SymbolPin) -> bool {
        pin.part_number == 0 || pin.part_number == self.active_part
    }

    /// Body rectangle, when present, derived from the first
    /// `SymbolGraphicKind::Rectangle` in `symbol.graphics`.
    fn body_rect(&self) -> Option<(f64, f64, f64, f64)> {
        for g in &self.symbol.graphics {
            if let SymbolGraphicKind::Rectangle { from, to } = &g.kind {
                return Some((from[0], from[1], to[0], to[1]));
            }
        }
        None
    }

    /// Bounding box around every visible symbol entity.
    ///
    /// If the symbol has no pins and no graphics, return a tiny box
    /// around world origin so Fit keeps the origin marker centered.
    pub(crate) fn bbox(&self) -> (f64, f64, f64, f64) {
        let mut bounds: Option<(f64, f64, f64, f64)> = None;
        let include_rect =
            |bounds: &mut Option<(f64, f64, f64, f64)>, x0: f64, y0: f64, x1: f64, y1: f64| {
                let rx0 = x0.min(x1);
                let ry0 = y0.min(y1);
                let rx1 = x0.max(x1);
                let ry1 = y0.max(y1);
                if let Some((min_x, min_y, max_x, max_y)) = bounds.as_mut() {
                    *min_x = (*min_x).min(rx0);
                    *min_y = (*min_y).min(ry0);
                    *max_x = (*max_x).max(rx1);
                    *max_y = (*max_y).max(ry1);
                } else {
                    *bounds = Some((rx0, ry0, rx1, ry1));
                }
            };

        if let Some((bx0, by0, bx1, by1)) = self.body_rect() {
            include_rect(
                &mut bounds,
                bx0.min(bx1) - 5.08,
                by0.min(by1) - 5.08,
                bx0.max(bx1) + 5.08,
                by0.max(by1) + 5.08,
            );
        }

        for pin in &self.symbol.pins {
            include_rect(
                &mut bounds,
                pin.position[0] - 1.27,
                pin.position[1] - 1.27,
                pin.position[0] + pin.length + 1.27,
                pin.position[1] + 1.27,
            );
        }

        // Include every graphic's extent so Fit doesn't leave shapes
        // off-screen.
        for g in &self.symbol.graphics {
            match &g.kind {
                SymbolGraphicKind::Rectangle { from, to }
                | SymbolGraphicKind::Line { from, to } => {
                    include_rect(&mut bounds, from[0], from[1], to[0], to[1]);
                }
                SymbolGraphicKind::Circle { center, radius }
                | SymbolGraphicKind::Arc { center, radius, .. } => {
                    include_rect(
                        &mut bounds,
                        center[0] - radius,
                        center[1] - radius,
                        center[0] + radius,
                        center[1] + radius,
                    );
                }
                SymbolGraphicKind::Text { position, size, .. } => {
                    include_rect(
                        &mut bounds,
                        position[0] - size,
                        position[1] - size,
                        position[0] + size,
                        position[1] + size,
                    );
                }
            }
        }

        bounds.unwrap_or((-1.27, -1.27, 1.27, 1.27))
    }
}

/// Palette derived from the active sheet colour — picks a content
/// foreground that reads correctly on the sheet bg. Two flavours:
/// dark-on-light (Cream / White / LightGray) and light-on-dark
/// (Black / DarkGray). Mirrors Altium's per-sheet contrast rule.
struct SymbolPalette {
    body: Color,
    pin: Color,
    text: Color,
    grid: Color,
    /// Slight stroke for axis lines through (0, 0).
    axis: Color,
}

impl SymbolPalette {
    fn for_sheet(sheet: Color) -> Self {
        // Rec. 601 luma — perceptually-weighted brightness.
        let luma = 0.299 * sheet.r + 0.587 * sheet.g + 0.114 * sheet.b;
        if luma > 0.5 {
            // Light sheet: dark text + the Altium signature blue body.
            Self {
                body: Color::from_rgb(0.10, 0.20, 0.55),
                pin: Color::from_rgb(0.10, 0.10, 0.10),
                text: Color::from_rgb(0.10, 0.10, 0.10),
                grid: Color::from_rgba(0.00, 0.00, 0.00, 0.18),
                axis: Color::from_rgba(0.00, 0.00, 0.00, 0.45),
            }
        } else {
            // Dark sheet: keep the warm yellow body + light text.
            Self {
                body: Color::from_rgb(0.95, 0.78, 0.30),
                pin: Color::from_rgb(0.85, 0.88, 0.92),
                text: Color::from_rgb(0.85, 0.88, 0.92),
                grid: Color::from_rgba(1.0, 1.0, 1.0, 0.12),
                axis: Color::from_rgba(1.0, 1.0, 1.0, 0.35),
            }
        }
    }
}

/// World-space mm grid the symbol canvas snaps cursor positions
/// to when the user is placing/moving things. Independent of the
/// visible grid (which follows `panel_ctx.grid_size_mm`) so a 0.635
/// mm visible grid still snaps to 1.27 mm — Altium's "smaller grid
/// for visual precision, larger for commit". Future toolbar work
/// could expose a separate snap-grid picker.
const SNAP_GRID_MM: f64 = 1.27;
const ORIGIN_MARKER_MM: f32 = 1.27;
const MM_PER_EM: f32 = 0.72;
const SYMBOL_AXIS_STROKE_PX_AT_100: f32 = 1.0;
const SYMBOL_GRAPHIC_STROKE_PX_AT_100: f32 = 1.5;
const SYMBOL_GRAPHIC_SELECTED_STROKE_PX_AT_100: f32 = 2.5;
const SYMBOL_RECT_STROKE_PX_AT_100: f32 = 2.0;
const SYMBOL_RECT_SELECTED_STROKE_PX_AT_100: f32 = 2.5;
const SYMBOL_HANDLE_STROKE_PX_AT_100: f32 = 1.0;
const SYMBOL_PIN_SELECTION_HALO_STROKE_PX_AT_100: f32 = 1.0;

#[derive(Debug, Clone, Copy)]
struct PinTextLayout {
    // Pin number (physical number) on/above the pin line.
    number_size_mm: f32,
    pin_pitch_mm: f32,
    number_offset_ratio_of_pitch: f32,
    number_along_ratio: f32,
    // Pin name label near the symbol body edge.
    name_size_mm: f32,
    name_offset_x_mm: f32,
    name_offset_y_mm: f32,
}

const PIN_TEXT_LAYOUT: PinTextLayout = PinTextLayout {
    number_size_mm: 1.27,
    pin_pitch_mm: 2.54,
    number_offset_ratio_of_pitch: 0.10,
    number_along_ratio: 0.18,
    name_size_mm: 1.27,
    name_offset_x_mm: 0.50,
    name_offset_y_mm: 0.00,
};

fn text_size_px_from_mm(size_mm: f32, scale: f32) -> f32 {
    let em_mm = size_mm.max(0.1) / MM_PER_EM;
    (em_mm * scale).clamp(2.0, 96.0)
}

fn stroke_px_at_zoom(base_width_px_at_100: f32, scale: f32) -> f32 {
    let zoom_factor = (scale / signex_types::schematic::SCHEMATIC_ZOOM_100_SCALE).max(0.0);
    let scaled = base_width_px_at_100 * zoom_factor;
    let max_stroke =
        base_width_px_at_100 * signex_types::schematic::SCHEMATIC_RENDER_STROKE_MAX_SCALE_MULTIPLIER;
    scaled.clamp(signex_types::schematic::SCHEMATIC_RENDER_MIN_STROKE_PX, max_stroke)
}

fn stroke_world_mm(base_width_px_at_100: f32, scale: f32) -> f32 {
    (stroke_px_at_zoom(base_width_px_at_100, scale) / scale.max(0.001))
        .max(signex_types::schematic::SCHEMATIC_RENDER_MIN_STROKE_MM as f32)
}

fn screen_px_to_world_mm(px: f32, scale: f32) -> f32 {
    (px / scale.max(0.001)).max(0.01)
}

fn to_rgba(color: Color) -> [f32; 4] {
    [color.r, color.g, color.b, color.a]
}

fn circle_vertices(center: [f64; 2], radius: f32, segments: usize) -> Vec<[f32; 2]> {
    let segment_count = segments.max(12);
    let cx = center[0] as f32;
    let cy = center[1] as f32;
    let r = radius.max(0.01);

    (0..segment_count)
        .map(|step| {
            let theta = (step as f32 / segment_count as f32) * std::f32::consts::TAU;
            [cx + theta.cos() * r, cy + theta.sin() * r]
        })
        .collect()
}

/// Convert screen coords → world-mm via the camera, then snap to
/// the symbol-canvas grid. The canvas's Standard y-flip happens at
/// the world↔screen boundary inside `world_to_screen` /
/// `screen_to_world`; we mirror it here so screen-down → world-up.
fn world_for(canvas: &SymbolCanvas<'_>, sx: f32, sy: f32, bounds: Rectangle) -> (f64, f64) {
    // The camera's screen_to_world doesn't know about y-flip — it
    // assumes screen and world share the same y-axis direction.
    // Symbol coords are Standard y-up; mirror by negating after.
    let world = canvas
        .camera
        .screen_to_world(iced::Point::new(sx, sy), bounds);
    let wx = world.x as f64;
    let wy = -world.y as f64;
    (
        (wx / SNAP_GRID_MM).round() * SNAP_GRID_MM,
        (wy / SNAP_GRID_MM).round() * SNAP_GRID_MM,
    )
}

/// Same as `world_for` but without the snap — used by the cursor
/// readout so the status footer shows the unsnapped position the
/// user actually pointed at.
fn world_unsnapped(canvas: &SymbolCanvas<'_>, sx: f32, sy: f32, bounds: Rectangle) -> (f64, f64) {
    let world = canvas
        .camera
        .screen_to_world(iced::Point::new(sx, sy), bounds);
    (world.x as f64, -world.y as f64)
}

fn selection_anchor(symbol: &Symbol, selection: SymbolSelection) -> Option<(f64, f64)> {
    match selection {
        SymbolSelection::Pin(idx) => symbol.pins.get(idx).map(|pin| (pin.position[0], pin.position[1])),
        SymbolSelection::Graphic(idx) => symbol.graphics.get(idx).map(|graphic| match &graphic.kind {
            SymbolGraphicKind::Rectangle { from, .. } | SymbolGraphicKind::Line { from, .. } => {
                (from[0], from[1])
            }
            SymbolGraphicKind::Circle { center, .. } | SymbolGraphicKind::Arc { center, .. } => {
                (center[0], center[1])
            }
            SymbolGraphicKind::Text { position, .. } => (position[0], position[1]),
        }),
        SymbolSelection::Field(_) => None,
    }
}

impl<'a> canvas::Program<CanvasAction> for SymbolCanvas<'a> {
    type State = CanvasState;

    fn update(
        &self,
        state: &mut CanvasState,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<CanvasAction>> {
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let pos = cursor.position_in(bounds)?;
                let (wx, wy) = world_for(self, pos.x, pos.y, bounds);
                match self.tool {
                    SymbolTool::Select => {
                        // Resize handles win over pin hits — corners /
                        // endpoints / radius are usually inside or right
                        // next to the body where pins might also live.
                        if let Some((idx, handle)) =
                            state::hit_test_graphic_handle(self.symbol, wx, wy)
                        {
                            state.dragging_handle = Some((idx, handle));
                            state.dragging = false;
                            state.drag_anchor_offset = None;
                            // Capture without publishing — the actual
                            // geometry mutation rides on CursorMoved.
                            return Some(canvas::Action::capture());
                        }
                        if let Some(sel) = state::hit_test(self.symbol, wx, wy) {
                            if self.selected == Some(sel) {
                                state.dragging = true;
                                state.drag_anchor_offset = selection_anchor(self.symbol, sel)
                                    .map(|(anchor_x, anchor_y)| (anchor_x - wx, anchor_y - wy));
                                return Some(canvas::Action::capture());
                            }

                            state.dragging = false;
                            state.drag_anchor_offset = None;
                            Some(canvas::Action::publish(CanvasAction::Select(sel)).and_capture())
                        } else {
                            state.dragging = false;
                            state.drag_anchor_offset = None;
                            Some(canvas::Action::publish(CanvasAction::Deselect).and_capture())
                        }
                    }
                    SymbolTool::AddPin => Some(
                        canvas::Action::publish(CanvasAction::AddPin { x: wx, y: wy })
                            .and_capture(),
                    ),
                    SymbolTool::PlaceRectangle => Some(
                        canvas::Action::publish(CanvasAction::AddRectangle { x: wx, y: wy })
                            .and_capture(),
                    ),
                    SymbolTool::PlaceLine => Some(
                        canvas::Action::publish(CanvasAction::AddLine { x: wx, y: wy })
                            .and_capture(),
                    ),
                    SymbolTool::PlaceCircle => Some(
                        canvas::Action::publish(CanvasAction::AddCircle { x: wx, y: wy })
                            .and_capture(),
                    ),
                    SymbolTool::PlaceArc => Some(
                        canvas::Action::publish(CanvasAction::AddArc { x: wx, y: wy })
                            .and_capture(),
                    ),
                    SymbolTool::PlaceText => Some(
                        canvas::Action::publish(CanvasAction::AddText { x: wx, y: wy })
                            .and_capture(),
                    ),
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right))
            | Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Middle)) => {
                // Right- or middle-button starts a pan. Schematic
                // canvas uses the same gesture (`canvas/mod.rs`).
                let pos = cursor.position_in(bounds)?;
                state.panning = true;
                state.last_pan_pos = Some(pos);
                Some(canvas::Action::capture())
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Right))
            | Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Middle)) => {
                state.panning = false;
                state.last_pan_pos = None;
                None
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let pos = cursor.position_in(bounds)?;
                // Pan first so panning while a handle is grabbed
                // doesn't accidentally reshape geometry.
                if state.panning {
                    let last = state.last_pan_pos.unwrap_or(pos);
                    let dx = pos.x - last.x;
                    let dy = pos.y - last.y;
                    state.last_pan_pos = Some(pos);
                    if dx != 0.0 || dy != 0.0 {
                        return Some(canvas::Action::publish(CanvasAction::Pan { dx, dy }));
                    }
                    return None;
                }
                let (wx, wy) = world_for(self, pos.x, pos.y, bounds);
                if let Some((idx, handle)) = state.dragging_handle {
                    return Some(canvas::Action::publish(CanvasAction::MoveGraphicHandle {
                        idx,
                        handle,
                        x: wx,
                        y: wy,
                    }));
                }
                if state.dragging {
                    let (move_x, move_y) = state
                        .drag_anchor_offset
                        .map(|(dx, dy)| (wx + dx, wy + dy))
                        .unwrap_or((wx, wy));
                    return Some(canvas::Action::publish(CanvasAction::Move {
                        x: move_x,
                        y: move_y,
                    }));
                }
                // Idle cursor — publish the unsnapped world position
                // for the status footer X/Y readout.
                let (ux, uy) = world_unsnapped(self, pos.x, pos.y, bounds);
                Some(canvas::Action::publish(CanvasAction::CursorAt {
                    x_mm: Some(ux),
                    y_mm: Some(uy),
                }))
            }
            Event::Mouse(mouse::Event::CursorLeft) => {
                state.panning = false;
                state.last_pan_pos = None;
                Some(canvas::Action::publish(CanvasAction::CursorAt {
                    x_mm: None,
                    y_mm: None,
                }))
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let pos = cursor.position_in(bounds)?;
                let dy = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => *y,
                    mouse::ScrollDelta::Pixels { y, .. } => *y / 30.0,
                };
                if dy.abs() < f32::EPSILON {
                    return None;
                }
                Some(
                    canvas::Action::publish(CanvasAction::Zoom {
                        sx: pos.x,
                        sy: pos.y,
                        delta: dy,
                    })
                    .and_capture(),
                )
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.dragging = false;
                state.dragging_handle = None;
                state.drag_anchor_offset = None;
                None
            }
            Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key,
                modifiers,
                ..
            }) => match key {
                iced::keyboard::Key::Named(iced::keyboard::key::Named::Delete)
                | iced::keyboard::Key::Named(iced::keyboard::key::Named::Backspace) => {
                    Some(canvas::Action::publish(CanvasAction::DeleteSelected))
                }
                iced::keyboard::Key::Named(iced::keyboard::key::Named::Home) => {
                    Some(canvas::Action::publish(CanvasAction::Fit))
                }
                iced::keyboard::Key::Named(iced::keyboard::key::Named::Space) => {
                    let pivot_mode = if modifiers.alt() {
                        RotatePivotMode::GeometryCenter
                    } else {
                        RotatePivotMode::WorldOrigin
                    };
                    Some(
                        canvas::Action::publish(CanvasAction::RotateSelected {
                            clockwise: !modifiers.shift(),
                            pivot_mode,
                        })
                        .and_capture(),
                    )
                }
                iced::keyboard::Key::Character(c) if c == " " => {
                    let pivot_mode = if modifiers.alt() {
                        RotatePivotMode::GeometryCenter
                    } else {
                        RotatePivotMode::WorldOrigin
                    };
                    Some(
                        canvas::Action::publish(CanvasAction::RotateSelected {
                            clockwise: !modifiers.shift(),
                            pivot_mode,
                        })
                        .and_capture(),
                    )
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        frame.fill_rectangle(iced::Point::ORIGIN, bounds.size(), self.bg_color);

        // Camera-driven world↔screen. Symbol coords are Standard y-up;
        // the camera doesn't know that, so we negate y on the way
        // out to match screen y-down.
        let cam = self.camera;
        let scale = cam.scale;
        let ox = cam.offset.x;
        let oy = cam.offset.y;
        let w2s = |x: f64, y: f64| -> iced::Point {
            iced::Point::new(ox + (x as f32) * scale, oy - (y as f32) * scale)
        };
        // Grid — read spacing from the global panel_ctx so the
        // schematic + library editors share the View ▸ Grid
        // setting.
        let (min_x, min_y, max_x, max_y) = self.bbox();
        if self.grid_visible {
            let g = self.grid_size_mm.max(0.001);
            // Visible bounds in world space (camera screen→world,
            // y-flipped). The grid pad lets dots peek past the
            // bbox so panning shows continuity.
            let pad = 6.0 * g;
            let (vx0, vy0) = world_unsnapped(self, 0.0, bounds.height, bounds);
            let (vx1, vy1) = world_unsnapped(self, bounds.width, 0.0, bounds);
            let world_x0 = (min_x - pad).min(vx0);
            let world_x1 = (max_x + pad).max(vx1);
            let world_y0 = (min_y - pad).min(vy0);
            let world_y1 = (max_y + pad).max(vy1);
            // Cap the iteration count so a zoomed-out view doesn't
            // try to plot millions of dots.
            let cols = ((world_x1 - world_x0) / g).abs() as i64 + 1;
            let rows = ((world_y1 - world_y0) / g).abs() as i64 + 1;
            // Skip render entirely when dots would be < 4 px apart
            // (they'd just smear into noise).
            let dot_screen_spacing = (g as f32) * scale;
            if cols * rows < 60_000 && dot_screen_spacing >= 4.0 {
                let dot_radius = (scale * 0.3).clamp(0.5, 2.0);
                let mut gx = (world_x0 / g).floor() * g;
                while gx <= world_x1 {
                    let mut gy = (world_y0 / g).floor() * g;
                    while gy <= world_y1 {
                        let p = w2s(gx, gy);
                        if p.x >= -dot_radius
                            && p.x <= bounds.width + dot_radius
                            && p.y >= -dot_radius
                            && p.y <= bounds.height + dot_radius
                        {
                            frame.fill(&canvas::Path::circle(p, dot_radius), self.grid_color);
                        }
                        gy += g;
                    }
                    gx += g;
                }
            }
        }

        // Origin marker at world (0, 0) — no default box/pin when a
        // symbol is created, so this gives a stable visual anchor.
        let origin = w2s(0.0, 0.0);
        let marker_half = text_size_px_from_mm(ORIGIN_MARKER_MM, scale).clamp(4.0, 18.0);
        if origin.x >= -marker_half
            && origin.x <= bounds.width + marker_half
            && origin.y >= -marker_half
            && origin.y <= bounds.height + marker_half
        {
            let h = canvas::Path::line(
                iced::Point::new(origin.x - marker_half, origin.y),
                iced::Point::new(origin.x + marker_half, origin.y),
            );
            let v = canvas::Path::line(
                iced::Point::new(origin.x, origin.y - marker_half),
                iced::Point::new(origin.x, origin.y + marker_half),
            );
            frame.stroke(
                &h,
                canvas::Stroke::default()
                    .with_color(self.axis_color)
                    .with_width(stroke_px_at_zoom(SYMBOL_AXIS_STROKE_PX_AT_100, scale)),
            );
            frame.stroke(
                &v,
                canvas::Stroke::default()
                    .with_color(self.axis_color)
                    .with_width(stroke_px_at_zoom(SYMBOL_AXIS_STROKE_PX_AT_100, scale)),
            );
            frame.fill(
                &canvas::Path::circle(origin, (marker_half * 0.14).clamp(1.0, 2.5)),
                self.axis_color,
            );
        }

        // ── Body + every other graphic ──
        // All symbol graphics and pin primitives now flow through the
        // renderer scene bridge so stroke/zoom behavior is unified.
        let selected_graphic_idx = match self.selected {
            Some(SymbolSelection::Graphic(i)) => Some(i),
            _ => None,
        };
        self.draw_symbol_with_renderer(&mut frame, selected_graphic_idx, scale);

        // Resize handles for every placed graphic — visible when the
        // Select tool is active so the user can grab any corner /
        // endpoint / radius / arc-angle / text-anchor at any time.
        if self.tool == SymbolTool::Select {
            for idx in 0..self.symbol.graphics.len() {
                for (_handle, pos) in state::graphic_handles(self.symbol, idx) {
                    let p = w2s(pos[0], pos[1]);
                    let half = 3.0_f32;
                    let top_left = iced::Point::new(p.x - half, p.y - half);
                    let size = Size::new(half * 2.0, half * 2.0);
                    let path = canvas::Path::rectangle(top_left, size);
                    frame.fill(&path, self.bg_color);
                    frame.stroke(
                        &path,
                        canvas::Stroke::default()
                            .with_color(self.selected_color)
                            .with_width(stroke_px_at_zoom(SYMBOL_HANDLE_STROKE_PX_AT_100, scale)),
                    );
                }
            }
        }

        // Tool hint.
        let tool_label = match self.tool {
            SymbolTool::Select => "Tool: Select  (Del to remove)",
            SymbolTool::AddPin => "Tool: Add Pin  (click to place)",
            SymbolTool::PlaceRectangle => "Tool: Place Rectangle  (click)",
            SymbolTool::PlaceLine => "Tool: Place Line  (click)",
            SymbolTool::PlaceCircle => "Tool: Place Ellipse  (click)",
            SymbolTool::PlaceArc => "Tool: Place Arc  (click)",
            SymbolTool::PlaceText => "Tool: Place Text  (click)",
        };
        frame.fill_text(canvas::Text {
            content: tool_label.to_string(),
            position: iced::Point::new(8.0, 8.0),
            size: 11.0.into(),
            color: Color {
                a: 0.55,
                ..self.text_color
            },
            font: crate::render_config::IOSEVKA,
            ..canvas::Text::default()
        });

        vec![frame.into_geometry()]
    }
}

impl<'a> SymbolCanvas<'a> {
    fn draw_symbol_with_renderer(
        &self,
        frame: &mut canvas::Frame,
        selected_graphic_idx: Option<usize>,
        scale: f32,
    ) {
        let snapshot = self.build_symbol_renderer_snapshot(selected_graphic_idx, scale);
        if snapshot.wires.is_empty()
            && snapshot.junctions.is_empty()
            && snapshot.arcs.is_empty()
            && snapshot.polygons.is_empty()
            && snapshot.labels.is_empty()
            && snapshot.pin_texts.is_empty()
        {
            return;
        }

        let mut scene = Scene::default();
        SchematicRenderer::build_scene(
            &snapshot,
            &ResolvedTheme::from_canvas_colors(signex_types::theme::canvas_colors(
                signex_types::theme::ThemeId::Signex,
            )),
            DirtyFlags::LINES
                | DirtyFlags::CIRCLES
                | DirtyFlags::ARCS
                | DirtyFlags::POLYGONS
                | DirtyFlags::TEXT,
            &mut scene,
        );

        let ox = self.camera.offset.x;
        let oy = self.camera.offset.y;
        crate::renderer_scene_canvas::draw_scene_with_world_to_screen(
            frame,
            &scene,
            |point| iced::Point::new(ox + point[0] * scale, oy - point[1] * scale),
            crate::renderer_scene_canvas::SceneDrawOptions {
                scale_px_per_mm: scale,
                min_stroke_px: signex_types::schematic::SCHEMATIC_RENDER_MIN_STROKE_PX,
                text_mm_per_em: MM_PER_EM,
                text_min_px: 2.0,
                text_max_px: 96.0,
            },
        );
    }

    fn build_symbol_renderer_snapshot(
        &self,
        selected_graphic_idx: Option<usize>,
        scale: f32,
    ) -> RendererSnapshot {
        let mut wires = Vec::new();
        let mut junctions = Vec::new();
        let mut arcs = Vec::new();
        let mut polygons = Vec::new();
        let mut labels = Vec::new();
        let mut pin_texts = Vec::new();

        let mut body_drawn = false;
        for (i, g) in self.symbol.graphics.iter().enumerate() {
            let is_selected = selected_graphic_idx == Some(i);
            let stroke_color = if is_selected {
                self.selected_color
            } else {
                self.body_color
            };
            let stroke_w = if is_selected {
                SYMBOL_GRAPHIC_SELECTED_STROKE_PX_AT_100
            } else {
                SYMBOL_GRAPHIC_STROKE_PX_AT_100
            };
            let rect_w = if is_selected {
                SYMBOL_RECT_SELECTED_STROKE_PX_AT_100
            } else {
                SYMBOL_RECT_STROKE_PX_AT_100
            };

            match &g.kind {
                SymbolGraphicKind::Rectangle { from, to } => {
                    let x0 = from[0] as f32;
                    let y0 = from[1] as f32;
                    let x1 = to[0] as f32;
                    let y1 = to[1] as f32;
                    let fill = if !body_drawn {
                        body_drawn = true;
                        to_rgba(Color {
                            a: 0.16,
                            ..self.body_color
                        })
                    } else {
                        [0.0, 0.0, 0.0, 0.0]
                    };

                    polygons.push(PolygonInput {
                        vertices: vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]],
                        fill_color: fill,
                        stroke_color: Some(to_rgba(stroke_color)),
                        stroke_width_mm: stroke_world_mm(rect_w, scale),
                    });
                }
                SymbolGraphicKind::Line { from, to } => {
                    wires.push(WireInput {
                        id: i as u64,
                        p0: [from[0] as f32, from[1] as f32],
                        p1: [to[0] as f32, to[1] as f32],
                        width_mm: stroke_world_mm(stroke_w, scale),
                        explicit_color: Some(to_rgba(stroke_color)),
                    });
                }
                SymbolGraphicKind::Circle { center, radius } => {
                    polygons.push(PolygonInput {
                        vertices: circle_vertices(*center, *radius as f32, 40),
                        fill_color: [0.0, 0.0, 0.0, 0.0],
                        stroke_color: Some(to_rgba(stroke_color)),
                        stroke_width_mm: stroke_world_mm(stroke_w, scale),
                    });
                }
                SymbolGraphicKind::Arc {
                    center,
                    radius,
                    start_deg,
                    end_deg,
                } => {
                    arcs.push(ArcInput {
                        center: [center[0] as f32, center[1] as f32],
                        radius_mm: *radius as f32,
                        start_angle_rad: (*start_deg as f32).to_radians(),
                        end_angle_rad: (*end_deg as f32).to_radians(),
                        width_mm: stroke_world_mm(stroke_w, scale),
                        color: to_rgba(stroke_color),
                    });
                }
                SymbolGraphicKind::Text {
                    position,
                    content,
                    size,
                } => {
                    labels.push(signex_renderer::schematic::TextInput {
                        content: content.clone(),
                        position: [position[0] as f32, position[1] as f32],
                        size_mm: (*size as f32).max(0.1),
                        color: to_rgba(if is_selected {
                            self.selected_color
                        } else {
                            self.text_color
                        }),
                        bold: false,
                        italic: false,
                        rotation_rad: 0.0,
                        h_align: HAlign::Left,
                        v_align: VAlign::Top,
                    });
                }
            }
        }

        for (i, pin) in self.symbol.pins.iter().enumerate() {
            if !self.pin_visible_on_active_part(pin) {
                continue;
            }

            use signex_library::PinOrientation;
            let (dx, dy) = match pin.orientation {
                PinOrientation::Right => (pin.length, 0.0),
                PinOrientation::Up => (0.0, pin.length),
                PinOrientation::Left => (-pin.length, 0.0),
                PinOrientation::Down => (0.0, -pin.length),
                _ => (-pin.length, 0.0),
            };

            let selected = matches!(self.selected, Some(SymbolSelection::Pin(j)) if j == i);
            let stroke_color = if selected {
                self.selected_color
            } else {
                self.pin_color
            };

            let tip = [pin.position[0] as f32, pin.position[1] as f32];
            let body_end = [(pin.position[0] + dx) as f32, (pin.position[1] + dy) as f32];
            wires.push(WireInput {
                id: 100_000 + i as u64,
                p0: tip,
                p1: body_end,
                width_mm: stroke_world_mm(
                    if selected {
                        signex_types::schematic::PIN_STROKE_SELECTED_PX
                    } else {
                        signex_types::schematic::PIN_STROKE_PX
                    },
                    scale,
                ),
                explicit_color: Some(to_rgba(stroke_color)),
            });

            junctions.push(JunctionInput {
                center: tip,
                radius_mm: screen_px_to_world_mm(2.5, scale),
                color: to_rgba(stroke_color),
            });

            if selected {
                polygons.push(PolygonInput {
                    vertices: circle_vertices(
                        [pin.position[0], pin.position[1]],
                        screen_px_to_world_mm(5.0, scale),
                        40,
                    ),
                    fill_color: [0.0, 0.0, 0.0, 0.0],
                    stroke_color: Some(to_rgba(self.selected_color)),
                    stroke_width_mm: stroke_world_mm(
                        SYMBOL_PIN_SELECTION_HALO_STROKE_PX_AT_100,
                        scale,
                    ),
                });
            }

            let seg_len = (dx * dx + dy * dy).sqrt().max(1e-6);
            let ux = dx / seg_len;
            let uy = dy / seg_len;
            let (n1x, n1y) = (-uy, ux);
            let (n2x, n2y) = (uy, -ux);
            // Prefer the visually "outer" side: top in world-Y, and left on ties.
            let choose_n1 = if (n1y - n2y).abs() > f64::EPSILON {
                n1y > n2y
            } else {
                n1x < n2x
            };
            let (nx, ny) = if choose_n1 { (n1x, n1y) } else { (n2x, n2y) };

            // Keep pin labels aligned with the pin axis while avoiding
            // upside-down horizontal text for left-facing pins.
            let mut text_rotation = (uy as f32).atan2(ux as f32);
            if text_rotation > std::f32::consts::FRAC_PI_2 {
                text_rotation -= std::f32::consts::PI;
            } else if text_rotation <= -std::f32::consts::FRAC_PI_2 {
                // Use <= so Down pins (atan2 = exactly -π/2) are also
                // normalized to +π/2, keeping vertical text reading
                // direction consistent with Up pins.
                text_rotation += std::f32::consts::PI;
            }

            let along_mm = seg_len * PIN_TEXT_LAYOUT.number_along_ratio as f64;
            let number_offset_mm = PIN_TEXT_LAYOUT.pin_pitch_mm as f64
                * PIN_TEXT_LAYOUT.number_offset_ratio_of_pitch as f64;
            let number_pos = [
                pin.position[0] + ux * along_mm + nx * number_offset_mm,
                pin.position[1] + uy * along_mm + ny * number_offset_mm,
            ];

            pin_texts.push(signex_renderer::schematic::TextInput {
                content: pin.number.clone(),
                position: [number_pos[0] as f32, number_pos[1] as f32],
                size_mm: PIN_TEXT_LAYOUT.number_size_mm,
                color: to_rgba(self.text_color),
                bold: false,
                italic: false,
                rotation_rad: text_rotation,
                h_align: HAlign::Center,
                v_align: VAlign::Bottom,
            });

            let name_pos = [
                pin.position[0]
                    + ux * (seg_len + PIN_TEXT_LAYOUT.name_offset_x_mm as f64)
                    + nx * number_offset_mm,
                pin.position[1]
                    + uy * (seg_len + PIN_TEXT_LAYOUT.name_offset_x_mm as f64)
                    + ny * number_offset_mm,
            ];
            pin_texts.push(signex_renderer::schematic::TextInput {
                content: pin.name.clone(),
                position: [name_pos[0] as f32, name_pos[1] as f32],
                size_mm: PIN_TEXT_LAYOUT.name_size_mm,
                color: to_rgba(Color {
                    a: 0.85,
                    ..self.text_color
                }),
                bold: false,
                italic: false,
                rotation_rad: text_rotation,
                h_align: HAlign::Left,
                v_align: VAlign::Center,
            });
        }

        RendererSnapshot {
            wires,
            junctions,
            arcs,
            polygons,
            labels,
            pin_texts,
            reference_value_texts: Vec::new(),
            parameter_texts: Vec::new(),
            overlays: OverlayInputs::default(),
            erc_markers: Vec::new(),
            wire_color_overrides: HashMap::new(),
        }
    }
}
