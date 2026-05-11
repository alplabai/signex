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
use signex_types::anchor2d::rotate_vec;
use signex_types::rotation2d::Vec2d;
use signex_renderer::schematic::{
    ArcInput, JunctionInput, OverlayInputs, PolygonInput, SchematicRenderer,
    SchematicSnapshot as RendererSnapshot, ViewRenderer, WireInput,
};
use signex_renderer::theme::ResolvedTheme;
use signex_types::schematic::{HAlign, VAlign};
use std::collections::HashMap;

use super::state::{self, GraphicHandle, SymbolSelection};

/// The actions a [`SymbolCanvas`] can emit upward.
#[derive(Debug, Clone)]
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
    /// Place a line segment from `from` to `to` (both grid-snapped
    /// mm world positions). Emitted on the second click of a
    /// two-click draw flow.
    AddLine {
        from_x: f64,
        from_y: f64,
        to_x: f64,
        to_y: f64,
    },
    /// Place a circle with center `(cx, cy)` and the given radius.
    /// Emitted on the second click of a two-click draw flow
    /// (1st click = center, 2nd click = edge defines radius).
    AddCircle {
        cx: f64,
        cy: f64,
        radius: f64,
    },
    /// Place an arc with center, radius, and start/end angles in degrees
    /// (0° = right, 90° = up in world coords). Emitted on the third
    /// click of a three-click draw flow.
    AddArc {
        cx: f64,
        cy: f64,
        radius: f64,
        start_deg: f64,
        end_deg: f64,
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
    /// Shift every pin and graphic by `(dx, dy)` mm.
    /// Emitted while the user drags with `SymbolSelection::All`.
    MoveAll {
        dx: f64,
        dy: f64,
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
    /// Emitted on `ButtonReleased(Left)` when a drag was in progress.
    /// The dispatcher uses this to clear `mid_drag` so the next drag
    /// starts a fresh undo snapshot group.
    DragCommit,
    /// Undo — Ctrl+Z while the canvas has keyboard focus.
    Undo,
    /// Redo — Ctrl+Y / Ctrl+Shift+Z while the canvas has keyboard focus.
    Redo,
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
    /// Last world position during an All-selection drag. Used to
    /// compute delta-based `MoveAll` events since there is no single
    /// anchor to absolute-position against.
    pub last_drag_world_pos: Option<(f64, f64)>,
    /// True while the user holds right- or middle-button to pan.
    pub panning: bool,
    /// Last cursor screen position during a pan, used to compute
    /// per-frame deltas.
    pub last_pan_pos: Option<iced::Point>,
    /// World-space anchor of a rubber-band box selection in progress.
    /// Set on `ButtonPressed(Left)` that hits empty space; cleared on
    /// `ButtonReleased(Left)`.
    pub box_select_origin: Option<(f64, f64)>,
    /// Current cursor world position while a box selection is being
    /// dragged. Updated every `CursorMoved`; used by `draw()` to
    /// paint the rubber band in real time. Direction:
    /// `current.x > origin.x` → Window (blue),
    /// `current.x < origin.x` → Crossing (green).
    pub box_select_current: Option<(f64, f64)>,
    /// First click world position while in the `PlaceLine` two-click
    /// draw flow. `None` = waiting for the first click;
    /// `Some((x, y))` = first point set, next click commits the line.
    pub line_from: Option<(f64, f64)>,
    /// Cursor position (snapped) updated every `CursorMoved` while
    /// `line_from.is_some()`, used to paint the rubber-band preview.
    pub line_cursor: Option<(f64, f64)>,
    /// First click center while in the `PlaceCircle` two-click draw flow.
    pub circle_center: Option<(f64, f64)>,
    /// Live cursor (snapped) while `circle_center.is_some()`, used for
    /// the radius rubber-band preview.
    pub circle_cursor: Option<(f64, f64)>,
    /// First click center while in the `PlaceArc` three-click draw flow.
    pub arc_center: Option<(f64, f64)>,
    /// Second click: `(radius_mm, start_deg)` once the radius and start
    /// angle have been committed by the second click.
    pub arc_radius_start: Option<(f64, f64)>,
    /// Live cursor for arc rubber-band preview (both Phase 1 and 2).
    pub arc_cursor: Option<(f64, f64)>,
    /// Unwrapped (cumulative) end-angle in degrees, updated every
    /// `CursorMoved` while Phase 2 is active. Unlike a raw `atan2`
    /// result this never jumps at the ±180° boundary so arcs that
    /// cross 0° / 360° render continuously.
    pub arc_end_deg_unwrapped: Option<f64>,
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

/// Pre-computed render geometry for one pin.
///
/// All positions are in world-mm. Derived once per frame from the pin's
/// `position`, `orientation`, and `length` so that
/// `build_symbol_renderer_snapshot` contains only push calls.
struct PinRenderGeometry {
    tip:           Vec2d,
    body_end:      Vec2d,
    number_pos:    Vec2d,
    name_pos:      Vec2d,
    text_rotation: f32,
    name_h_align:  HAlign,
}

impl PinRenderGeometry {
    fn compute(pin: &SymbolPin) -> Self {
        use signex_library::PinOrientation;
        use std::f64::consts::FRAC_PI_2;

        // Orientation → angle (CCW from +x axis), tip → body direction.
        let angle_rad: f64 = match pin.orientation {
            PinOrientation::Right => 0.0,
            PinOrientation::Up    => FRAC_PI_2,
            PinOrientation::Left  => std::f64::consts::PI,
            PinOrientation::Down  => -FRAC_PI_2,
            _                     => std::f64::consts::PI,
        };

        let tip  = Vec2d::new(pin.position[0], pin.position[1]);
        let unit = rotate_vec(Vec2d::new(1.0, 0.0), angle_rad);
        let body_end = Vec2d::new(
            tip.x + unit.x * pin.length,
            tip.y + unit.y * pin.length,
        );

        // Outer normal: 90° CCW from unit = (-unit.y, unit.x).
        // Pick the side that is visually "outer": prefer +y, break ties with -x.
        let n_ccw = rotate_vec(unit, FRAC_PI_2);
        let n_cw  = Vec2d::new(-n_ccw.x, -n_ccw.y);
        let normal = if (n_ccw.y - n_cw.y).abs() > f64::EPSILON {
            if n_ccw.y > n_cw.y { n_ccw } else { n_cw }
        } else if n_ccw.x < n_cw.x {
            n_ccw
        } else {
            n_cw
        };

        // Text rotation for iced screen space.
        //
        // World coordinates are Y-up; iced canvas is Y-down. The world→screen
        // transform is: screen_y = oy − world_y × scale, which negates the Y
        // component. To make text align with the pin direction in screen space
        // we must negate unit.y: atan2(−uy, ux).
        //
        // Normalize to (−π/2, π/2] so text is never upside-down.
        // Use strict `<` for the lower bound so that Up-pin angle (exactly
        // −π/2) is kept as-is — it makes text flow upward on screen, which
        // is the correct readable direction for Up pins.
        let mut text_rotation = (-(unit.y as f32)).atan2(unit.x as f32);
        let flipped: bool;
        if text_rotation > std::f32::consts::FRAC_PI_2 {
            text_rotation -= std::f32::consts::PI;
            flipped = true;
        } else if text_rotation < -std::f32::consts::FRAC_PI_2 {
            text_rotation += std::f32::consts::PI;
            flipped = true;
        } else {
            flipped = false;
        }
        // When flipped (only Left-facing pins after normalization), the text's
        // local +x axis points opposite the pin direction — reverse h_align so
        // the name still extends away from the tip.
        let name_h_align = if flipped { HAlign::Right } else { HAlign::Left };

        let number_offset_mm = PIN_TEXT_LAYOUT.pin_pitch_mm as f64
            * PIN_TEXT_LAYOUT.number_offset_ratio_of_pitch as f64;
        let along_mm = pin.length * PIN_TEXT_LAYOUT.number_along_ratio as f64;

        let number_pos = Vec2d::new(
            tip.x + unit.x * along_mm + normal.x * number_offset_mm,
            tip.y + unit.y * along_mm + normal.y * number_offset_mm,
        );
        let name_pos = Vec2d::new(
            tip.x + unit.x * (pin.length + PIN_TEXT_LAYOUT.name_offset_x_mm as f64),
            tip.y + unit.y * (pin.length + PIN_TEXT_LAYOUT.name_offset_x_mm as f64),
        );

        Self { tip, body_end, number_pos, name_pos, text_rotation, name_h_align }
    }
}

fn text_size_px_from_mm(size_mm: f32, scale: f32) -> f32 {
    let em_mm = size_mm.max(0.1) / MM_PER_EM;
    (em_mm * scale).clamp(2.0, 96.0)
}

fn stroke_px_at_zoom(base_width_px_at_100: f32, _scale: f32) -> f32 {
    base_width_px_at_100
}

/// Unwrap a raw `atan2` angle (in degrees, range `[-180, 180]`) so that
/// the result stays within 180° of `prev`. This removes the ±180° branch
/// cut when tracking a continuously-moving cursor angle.
///
/// Example: prev = 170°, raw = -170° → returns 190° (not -170°).
fn unwrap_angle(prev: f64, raw: f64) -> f64 {
    let mut delta = raw - prev;
    // Bring delta into (-180, 180] so we always take the short arc.
    if delta > 180.0 { delta -= 360.0; }
    if delta <= -180.0 { delta += 360.0; }
    prev + delta
}

fn stroke_world_mm(base_width_px_at_100: f32, scale: f32) -> f32 {
    (base_width_px_at_100 / scale.max(0.001))
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

fn selection_anchor(symbol: &Symbol, selection: &SymbolSelection) -> Option<(f64, f64)> {
    match selection {
        SymbolSelection::Pin(idx) => symbol.pins.get(*idx).map(|pin| (pin.position[0], pin.position[1])),
        SymbolSelection::Graphic(idx) => symbol.graphics.get(*idx).map(|graphic| match &graphic.kind {
            SymbolGraphicKind::Rectangle { from, .. } | SymbolGraphicKind::Line { from, .. } => {
                (from[0], from[1])
            }
            SymbolGraphicKind::Circle { center, .. } | SymbolGraphicKind::Arc { center, .. } => {
                (center[0], center[1])
            }
            SymbolGraphicKind::Text { position, .. } => (position[0], position[1]),
        }),
        SymbolSelection::Field(_) | SymbolSelection::All | SymbolSelection::Multiple { .. } => None,
    }
}

/// Returns `true` when `item` (a single-element selection) belongs to the
/// multi-element `group` selection. Used to decide whether a click on an
/// already-selected item should start a group drag rather than replace the
/// selection.
fn item_in_selection(group: &SymbolSelection, item: &SymbolSelection) -> bool {
    match (group, item) {
        (SymbolSelection::All, _) => true,
        (SymbolSelection::Multiple { pin_indices, .. }, SymbolSelection::Pin(idx)) => {
            pin_indices.contains(idx)
        }
        (SymbolSelection::Multiple { graphic_indices, .. }, SymbolSelection::Graphic(idx)) => {
            graphic_indices.contains(idx)
        }
        _ => false,
    }
}

/// Returns `true` when the graphic at `idx` should be drawn in the
/// selection colour. Handles single-graphic, Multiple, and All selections.
fn is_graphic_selected(sel: &Option<SymbolSelection>, idx: usize) -> bool {
    match sel {
        Some(SymbolSelection::Graphic(i)) => *i == idx,
        Some(SymbolSelection::Multiple { graphic_indices, .. }) => {
            graphic_indices.contains(&idx)
        }
        Some(SymbolSelection::All) => true,
        _ => false,
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
                // Placement tools need grid-snapped coordinates; hit-testing for
                // the Select tool must use the raw (unsnapped) cursor position so
                // that objects not sitting exactly on the snap grid can still be
                // clicked. We compute both up-front and choose per tool arm.
                let (wx, wy) = world_for(self, pos.x, pos.y, bounds);
                let (ux, uy) = world_unsnapped(self, pos.x, pos.y, bounds);
                match self.tool {
                    SymbolTool::Select => {
                        // Resize handles win over everything else.
                        // Use a screen-pixel-based tolerance so handles are
                        // equally easy to hit at any zoom level.
                        let tol_mm = (8.0_f32 / self.camera.scale.max(0.01))
                            .clamp(0.5, 4.0) as f64;
                        if let Some((idx, handle)) =
                            state::hit_test_graphic_handle(self.symbol, ux, uy, tol_mm)
                        {
                            state.dragging_handle = Some((idx, handle));
                            state.dragging = false;
                            state.drag_anchor_offset = None;
                            state.last_drag_world_pos = None;
                            state.box_select_origin = None;
                            state.box_select_current = None;
                            return Some(canvas::Action::capture());
                        }
                        if let Some(sel) = state::hit_test(self.symbol, ux, uy) {
                            state.box_select_origin = None;
                            state.box_select_current = None;

                            // If the clicked item is inside the current Multiple /
                            // All selection, drag the whole group as a unit.
                            let in_group = self.selected.as_ref().map_or(false, |s| {
                                item_in_selection(s, &sel)
                            });
                            if in_group {
                                state.dragging = true;
                                state.last_drag_world_pos = Some((wx, wy));
                                state.drag_anchor_offset = None;
                                return Some(canvas::Action::capture());
                            }

                            let effective_sel = sel;

                            let is_delta = matches!(effective_sel, SymbolSelection::All);
                            state.dragging = true;
                            state.last_drag_world_pos = if is_delta {
                                Some((wx, wy))
                            } else {
                                None
                            };
                            state.drag_anchor_offset =
                                selection_anchor(self.symbol, &effective_sel)
                                    .map(|(ax, ay)| (ax - wx, ay - wy));

                            if self.selected.as_ref() == Some(&effective_sel) {
                                return Some(canvas::Action::capture());
                            }
                            Some(
                                canvas::Action::publish(CanvasAction::Select(effective_sel))
                                    .and_capture(),
                            )
                        } else {
                            // Empty space — start a rubber-band box selection.
                            // Use unsnapped coords so box corners track the pointer exactly.
                            state.dragging = false;
                            state.drag_anchor_offset = None;
                            state.last_drag_world_pos = None;
                            state.box_select_origin = Some((ux, uy));
                            state.box_select_current = Some((ux, uy));
                            Some(canvas::Action::capture())
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
                    SymbolTool::PlaceLine => {
                        if let Some((from_x, from_y)) = state.line_from.take() {
                            // Second click — commit the line.
                            state.line_cursor = None;
                            Some(
                                canvas::Action::publish(CanvasAction::AddLine {
                                    from_x,
                                    from_y,
                                    to_x: wx,
                                    to_y: wy,
                                })
                                .and_capture(),
                            )
                        } else {
                            // First click — set the start point and wait.
                            state.line_from = Some((wx, wy));
                            state.line_cursor = Some((wx, wy));
                            Some(canvas::Action::capture())
                        }
                    }
                    SymbolTool::PlaceCircle => {
                        if let Some((center_x, center_y)) = state.circle_center.take() {
                            // Second click — commit the circle.
                            let dx = wx - center_x;
                            let dy = wy - center_y;
                            let radius = (dx * dx + dy * dy).sqrt().max(0.1);
                            state.circle_cursor = None;
                            Some(
                                canvas::Action::publish(CanvasAction::AddCircle {
                                    cx: center_x,
                                    cy: center_y,
                                    radius,
                                })
                                .and_capture(),
                            )
                        } else {
                            // First click — set the center and wait.
                            state.circle_center = Some((wx, wy));
                            state.circle_cursor = Some((wx, wy));
                            Some(canvas::Action::capture())
                        }
                    }
                    SymbolTool::PlaceArc => {
                        if let Some((radius, start_deg)) = state.arc_radius_start.take() {
                            // Third click — commit the arc.
                            let (cx, cy) = state.arc_center.take().unwrap_or((wx, wy));
                            state.arc_cursor = None;
                            // Use the unwrapped end angle so arcs that swept
                            // past ±180° are stored correctly. Fall back to a
                            // fresh atan2 only if the cursor never moved after
                            // the second click.
                            let end_deg = state.arc_end_deg_unwrapped.take().unwrap_or_else(|| {
                                let dx = wx - cx;
                                let dy = wy - cy;
                                dy.atan2(dx).to_degrees()
                            });
                            Some(
                                canvas::Action::publish(CanvasAction::AddArc {
                                    cx,
                                    cy,
                                    radius,
                                    start_deg,
                                    end_deg,
                                })
                                .and_capture(),
                            )
                        } else if let Some((cx, cy)) = state.arc_center {
                            // Second click — define radius + start angle.
                            let dx = wx - cx;
                            let dy = wy - cy;
                            let radius = (dx * dx + dy * dy).sqrt().max(0.1);
                            let start_deg = dy.atan2(dx).to_degrees();
                            state.arc_radius_start = Some((radius, start_deg));
                            // Seed the unwrapped tracker at the start angle so
                            // the first CursorMoved won't produce a large jump.
                            state.arc_end_deg_unwrapped = Some(start_deg);
                            Some(canvas::Action::capture())
                        } else {
                            // First click — set the center and wait.
                            state.arc_center = Some((wx, wy));
                            state.arc_cursor = Some((wx, wy));
                            Some(canvas::Action::capture())
                        }
                    }
                    SymbolTool::PlaceText => Some(
                        canvas::Action::publish(CanvasAction::AddText { x: wx, y: wy })
                            .and_capture(),
                    ),
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right))
            | Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Middle)) => {
                // Right-click cancels any in-progress multi-click draw;
                // otherwise starts a pan (same as schematic canvas).
                let draw_in_progress = match self.tool {
                    SymbolTool::PlaceLine => state.line_from.is_some(),
                    SymbolTool::PlaceCircle => state.circle_center.is_some(),
                    SymbolTool::PlaceArc => {
                        state.arc_center.is_some() || state.arc_radius_start.is_some()
                    }
                    _ => false,
                };
                if draw_in_progress {
                    state.line_from = None;
                    state.line_cursor = None;
                    state.circle_center = None;
                    state.circle_cursor = None;
                    state.arc_center = None;
                    state.arc_radius_start = None;
                    state.arc_cursor = None;
                    state.arc_end_deg_unwrapped = None;
                    return Some(canvas::Action::capture());
                }
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
                let (ux, uy) = world_unsnapped(self, pos.x, pos.y, bounds);
                if let Some((idx, handle)) = state.dragging_handle {
                    return Some(canvas::Action::publish(CanvasAction::MoveGraphicHandle {
                        idx,
                        handle,
                        x: wx,
                        y: wy,
                    }));
                }
                if state.dragging {
                    // All or Multiple selection: delta-based drag.
                    let is_delta_based = matches!(
                        self.selected,
                        Some(SymbolSelection::All) | Some(SymbolSelection::Multiple { .. })
                    );
                    if is_delta_based {
                        if let Some((last_wx, last_wy)) = state.last_drag_world_pos {
                            let dx = wx - last_wx;
                            let dy = wy - last_wy;
                            state.last_drag_world_pos = Some((wx, wy));
                            if dx.abs() > f64::EPSILON || dy.abs() > f64::EPSILON {
                                return Some(canvas::Action::publish(CanvasAction::MoveAll {
                                    dx,
                                    dy,
                                }));
                            }
                        } else {
                            state.last_drag_world_pos = Some((wx, wy));
                        }
                        return None;
                    }
                    // Single-item selection: absolute positioning with anchor offset.
                    let (move_x, move_y) = state
                        .drag_anchor_offset
                        .map(|(dx, dy)| (wx + dx, wy + dy))
                        .unwrap_or((wx, wy));
                    return Some(canvas::Action::publish(CanvasAction::Move {
                        x: move_x,
                        y: move_y,
                    }));
                }
                // Update rubber-band box while the user drags on empty space.
                // Publishing CursorAt forces iced to process the event as a
                // state-changing message, which triggers draw() on the next
                // frame so the rubber band animates in real time.
                if state.box_select_origin.is_some() {
                    state.box_select_current = Some((ux, uy));
                    return Some(
                        canvas::Action::publish(CanvasAction::CursorAt {
                            x_mm: Some(ux),
                            y_mm: Some(uy),
                        })
                        .and_capture(),
                    );
                }
                // While waiting for a subsequent click in a multi-click
                // draw flow, update the rubber-band cursor and force a
                // redraw so the preview animates in real time.
                if self.tool == SymbolTool::PlaceLine && state.line_from.is_some() {
                    state.line_cursor = Some((wx, wy));
                    return Some(
                        canvas::Action::publish(CanvasAction::CursorAt {
                            x_mm: Some(ux),
                            y_mm: Some(uy),
                        })
                        .and_capture(),
                    );
                }
                if self.tool == SymbolTool::PlaceCircle && state.circle_center.is_some() {
                    state.circle_cursor = Some((wx, wy));
                    return Some(
                        canvas::Action::publish(CanvasAction::CursorAt {
                            x_mm: Some(ux),
                            y_mm: Some(uy),
                        })
                        .and_capture(),
                    );
                }
                if self.tool == SymbolTool::PlaceArc
                    && (state.arc_center.is_some() || state.arc_radius_start.is_some())
                {
                    state.arc_cursor = Some((wx, wy));
                    // Phase 2: keep a continuous (unwrapped) end angle so
                    // arcs that sweep past ±180° don't jump.
                    if let Some((cx, cy)) = state.arc_center {
                        if state.arc_radius_start.is_some() {
                            let raw = (wy - cy).atan2(wx - cx).to_degrees();
                            state.arc_end_deg_unwrapped = Some(match state.arc_end_deg_unwrapped {
                                Some(prev) => unwrap_angle(prev, raw),
                                None => raw,
                            });
                        }
                    }
                    return Some(
                        canvas::Action::publish(CanvasAction::CursorAt {
                            x_mm: Some(ux),
                            y_mm: Some(uy),
                        })
                        .and_capture(),
                    );
                }
                // Idle cursor — publish the unsnapped world position
                // for the status footer X/Y readout.
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
                let was_dragging = state.dragging || state.dragging_handle.is_some();
                state.dragging = false;
                state.dragging_handle = None;
                state.drag_anchor_offset = None;
                state.last_drag_world_pos = None;

                // Commit a rubber-band box selection if one was in progress.
                if let (Some((ox, oy)), Some((cx, cy))) =
                    (state.box_select_origin.take(), state.box_select_current.take())
                {
                    let drag_dist_sq = (cx - ox).powi(2) + (cy - oy).powi(2);
                    if drag_dist_sq > 0.5 * 0.5 {
                        // Enough movement — commit as a box selection.
                        let kind = if cx >= ox {
                            state::BoxSelectKind::Window
                        } else {
                            state::BoxSelectKind::Crossing
                        };
                        let result =
                            state::select_in_box(self.symbol, ox, oy, cx, cy, kind);
                        return Some(match result {
                            Some(sel) => canvas::Action::publish(CanvasAction::Select(sel))
                                .and_capture(),
                            None => canvas::Action::publish(CanvasAction::Deselect)
                                .and_capture(),
                        });
                    } else {
                        // Micro-drag treated as a click → deselect.
                        return Some(
                            canvas::Action::publish(CanvasAction::Deselect).and_capture(),
                        );
                    }
                }

                // Notify dispatcher that a move drag completed so it can
                // close the coalesced undo snapshot group.
                if was_dragging {
                    return Some(canvas::Action::publish(CanvasAction::DragCommit));
                }

                None
            }
            Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key,
                modifiers,
                ..
            }) => match key {
                iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) => {
                    let cancelled = match self.tool {
                        SymbolTool::PlaceLine if state.line_from.is_some() => {
                            state.line_from = None;
                            state.line_cursor = None;
                            true
                        }
                        SymbolTool::PlaceCircle if state.circle_center.is_some() => {
                            state.circle_center = None;
                            state.circle_cursor = None;
                            true
                        }
                        SymbolTool::PlaceArc
                            if state.arc_center.is_some()
                                || state.arc_radius_start.is_some() =>
                        {
                            state.arc_center = None;
                            state.arc_radius_start = None;
                            state.arc_cursor = None;
                            state.arc_end_deg_unwrapped = None;
                            true
                        }
                        _ => false,
                    };
                    if cancelled {
                        return Some(canvas::Action::capture());
                    }
                    None
                }
                iced::keyboard::Key::Named(iced::keyboard::key::Named::Delete)
                | iced::keyboard::Key::Named(iced::keyboard::key::Named::Backspace) => {
                    Some(canvas::Action::publish(CanvasAction::DeleteSelected))
                }
                iced::keyboard::Key::Named(iced::keyboard::key::Named::Home) => {
                    Some(canvas::Action::publish(CanvasAction::Fit))
                }
                iced::keyboard::Key::Character(c) if c.as_str() == "a" && modifiers.command() => {
                    Some(
                        canvas::Action::publish(CanvasAction::Select(SymbolSelection::All))
                            .and_capture(),
                    )
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
                // Undo: Ctrl+Z
                iced::keyboard::Key::Character(c)
                    if c.as_str() == "z" && modifiers.command() && !modifiers.shift() =>
                {
                    Some(
                        canvas::Action::publish(CanvasAction::Undo).and_capture(),
                    )
                }
                // Redo: Ctrl+Y  or  Ctrl+Shift+Z
                iced::keyboard::Key::Character(c)
                    if (c.as_str() == "y" && modifiers.command())
                        || (c.as_str() == "z"
                            && modifiers.command()
                            && modifiers.shift()) =>
                {
                    Some(
                        canvas::Action::publish(CanvasAction::Redo).and_capture(),
                    )
                }
                _ => None,
            },
            _ => None,
        }
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
        // While actively dragging a resize handle, keep the cursor that
        // matches the handle so it doesn't flicker back to Crosshair.
        if let Some((_, handle)) = state.dragging_handle {
            return state::handle_interaction(handle);
        }
        if state.dragging {
            return mouse::Interaction::Grab;
        }
        // Hover detection: change the cursor when the pointer is close
        // to any graphic handle. Tolerance is expressed in screen pixels
        // so it feels the same at all zoom levels.
        if self.tool == SymbolTool::Select {
            if let Some(pos) = cursor.position_in(bounds) {
                let (wx, wy) = world_unsnapped(self, pos.x, pos.y, bounds);
                let tol_mm = (8.0_f32 / self.camera.scale.max(0.01))
                    .clamp(0.5, 4.0) as f64;
                if let Some((_, handle)) =
                    state::hit_test_graphic_handle(self.symbol, wx, wy, tol_mm)
                {
                    return state::handle_interaction(handle);
                }
            }
        }
        if cursor.is_over(bounds) {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }

    fn draw(
        &self,
        state: &Self::State,
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
            // Zoom-adaptive: skip render when grid points would be < 4 px apart.
            let dot_screen_spacing = (g as f32) * scale;
            if cols * rows < 60_000 && dot_screen_spacing >= 4.0 {
                // Size/length scales with spacing, clamped to keep dots/crosses
                // visible but not overwhelming at any zoom level.
                let dot_radius = (scale * (g as f32) * 0.03).clamp(0.5, 2.0);
                let cross_arm = (dot_screen_spacing * 0.18).clamp(1.5, 4.0);
                let grid_style = crate::render_config::symbol_grid_style();
                let cross_stroke = canvas::Stroke::default()
                    .with_color(self.grid_color)
                    .with_width(0.6);

                // Lines style: draw full grid lines instead of per-point glyphs.
                if matches!(grid_style, crate::render_config::GridStyle::Lines) {
                    let mut gx = (world_x0 / g).floor() * g;
                    while gx <= world_x1 {
                        let sx = w2s(gx, 0.0).x;
                        if sx >= 0.0 && sx <= bounds.width {
                            let top_sy = w2s(0.0, world_y1).y.max(0.0);
                            let bot_sy = w2s(0.0, world_y0).y.min(bounds.height);
                            if top_sy < bot_sy {
                                frame.stroke(
                                    &canvas::Path::line(
                                        iced::Point::new(sx, top_sy),
                                        iced::Point::new(sx, bot_sy),
                                    ),
                                    cross_stroke,
                                );
                            }
                        }
                        gx += g;
                    }
                    let mut gy = (world_y0 / g).floor() * g;
                    while gy <= world_y1 {
                        let sy = w2s(0.0, gy).y;
                        if sy >= 0.0 && sy <= bounds.height {
                            let left_sx = w2s(world_x0, 0.0).x.max(0.0);
                            let right_sx = w2s(world_x1, 0.0).x.min(bounds.width);
                            if left_sx < right_sx {
                                frame.stroke(
                                    &canvas::Path::line(
                                        iced::Point::new(left_sx, sy),
                                        iced::Point::new(right_sx, sy),
                                    ),
                                    cross_stroke,
                                );
                            }
                        }
                        gy += g;
                    }
                } else {
                    let mut gx = (world_x0 / g).floor() * g;
                    while gx <= world_x1 {
                        let mut gy = (world_y0 / g).floor() * g;
                        while gy <= world_y1 {
                            let p = w2s(gx, gy);
                            if p.x >= -cross_arm
                                && p.x <= bounds.width + cross_arm
                                && p.y >= -cross_arm
                                && p.y <= bounds.height + cross_arm
                            {
                                match grid_style {
                                    crate::render_config::GridStyle::Dots => {
                                        frame.fill(
                                            &canvas::Path::circle(p, dot_radius),
                                            self.grid_color,
                                        );
                                    }
                                    crate::render_config::GridStyle::SmallCrosses => {
                                        frame.stroke(
                                            &canvas::Path::line(
                                                iced::Point::new(p.x - cross_arm, p.y),
                                                iced::Point::new(p.x + cross_arm, p.y),
                                            ),
                                            cross_stroke,
                                        );
                                        frame.stroke(
                                            &canvas::Path::line(
                                                iced::Point::new(p.x, p.y - cross_arm),
                                                iced::Point::new(p.x, p.y + cross_arm),
                                            ),
                                            cross_stroke,
                                        );
                                    }
                                    // Lines handled above; unreachable here.
                                    crate::render_config::GridStyle::Lines => unreachable!(),
                                }
                            }
                            gy += g;
                        }
                        gx += g;
                    }
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
        self.draw_symbol_with_renderer(&mut frame, &self.selected, scale);

        // Resize handles for placed graphics — visible in the Select tool
        // only for the currently-selected graphic(s) so the canvas isn't
        // cluttered when nothing is selected.
        // Corner handles (squares, half=3 px) are visually larger than edge
        // midpoint handles (squares, half=2 px) so the user can tell them
        // apart at a glance.
        if self.tool == SymbolTool::Select {
            for idx in 0..self.symbol.graphics.len() {
                if !is_graphic_selected(&self.selected, idx) {
                    continue;
                }
                for (handle, pos) in state::graphic_handles(self.symbol, idx) {
                    let p = w2s(pos[0], pos[1]);
                    let is_corner = matches!(handle, state::GraphicHandle::RectCorner(_));
                    let half = if is_corner { 3.0_f32 } else { 2.0_f32 };
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
        let tool_label = match self.tool {            SymbolTool::Select => "Tool: Select  (Del to remove)",
            SymbolTool::AddPin => "Tool: Add Pin  (click to place)",
            SymbolTool::PlaceRectangle => "Tool: Place Rectangle  (click)",
            SymbolTool::PlaceLine => "Tool: Place Line  (click start, click end / Esc to cancel)",
            SymbolTool::PlaceCircle => "Tool: Place Ellipse  (click center, click edge / Esc to cancel)",
            SymbolTool::PlaceArc => "Tool: Place Arc  (click center, click start, click end / Esc to cancel)",
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

        // Rubber-band box selection overlay — drawn on top of everything
        // so the dashed border is always visible.
        if let (Some((ox, oy)), Some((cx, cy))) =
            (state.box_select_origin, state.box_select_current)
        {
            let p0 = w2s(ox, oy);
            let p1 = w2s(cx, cy);
            let left = p0.x.min(p1.x);
            let top = p0.y.min(p1.y);
            let width = (p1.x - p0.x).abs();
            let height = (p1.y - p0.y).abs();

            // Direction determines colour:
            //   cx > ox  (L→R) = Window selection  → blue fill + blue outline
            //   cx < ox  (R→L) = Crossing selection → green fill + green outline
            let is_crossing = cx < ox;
            let (fill_color, stroke_color) = if is_crossing {
                (
                    Color::from_rgba(0.1, 0.85, 0.25, 0.08),
                    Color::from_rgba(0.1, 0.85, 0.25, 0.90),
                )
            } else {
                (
                    Color::from_rgba(0.15, 0.45, 0.95, 0.08),
                    Color::from_rgba(0.15, 0.45, 0.95, 0.90),
                )
            };
            let rect_origin = iced::Point::new(left, top);
            let rect_size = Size::new(width, height);
            frame.fill_rectangle(rect_origin, rect_size, fill_color);
            frame.stroke(
                &canvas::Path::rectangle(rect_origin, rect_size),
                canvas::Stroke::default()
                    .with_color(stroke_color)
                    .with_width(1.5),
            );
        }

        // Rubber-band line preview — drawn while the user has set the
        // first point of a two-click line draw and is hovering toward
        // the second point.
        if let (Some((fx, fy)), Some((cx, cy))) = (state.line_from, state.line_cursor) {
            let p0 = w2s(fx, fy);
            let p1 = w2s(cx, cy);
            let preview_color = Color {
                a: 0.55,
                ..self.selected_color
            };
            // Start-point dot so the user can see the anchor.
            frame.fill(
                &canvas::Path::circle(p0, 3.0),
                preview_color,
            );
            frame.stroke(
                &canvas::Path::line(p0, p1),
                canvas::Stroke::default()
                    .with_color(preview_color)
                    .with_width(stroke_px_at_zoom(SYMBOL_GRAPHIC_STROKE_PX_AT_100, scale))
                    .with_line_cap(canvas::LineCap::Round),
            );
        }

        // Rubber-band circle preview — two-click flow: center set,
        // waiting for the edge click that defines the radius.
        if let (Some((cx, cy)), Some((cur_x, cur_y))) =
            (state.circle_center, state.circle_cursor)
        {
            let center_p = w2s(cx, cy);
            let dx = cur_x - cx;
            let dy = cur_y - cy;
            let radius_world = (dx * dx + dy * dy).sqrt().max(0.1);
            let radius_screen = (radius_world as f32) * scale;
            let preview_color = Color {
                a: 0.55,
                ..self.selected_color
            };
            // Center dot.
            frame.fill(&canvas::Path::circle(center_p, 3.0), preview_color);
            // Radius line to the cursor.
            let cursor_p = w2s(cur_x, cur_y);
            frame.stroke(
                &canvas::Path::line(center_p, cursor_p),
                canvas::Stroke::default()
                    .with_color(preview_color)
                    .with_width(1.0),
            );
            // Circle outline at current radius.
            frame.stroke(
                &canvas::Path::circle(center_p, radius_screen),
                canvas::Stroke::default()
                    .with_color(preview_color)
                    .with_width(stroke_px_at_zoom(SYMBOL_GRAPHIC_STROKE_PX_AT_100, scale)),
            );
        }

        // Rubber-band arc preview — three-click flow.
        // Phase 1 (center set, radius not yet): center dot + radius line.
        // Phase 2 (center + radius/start set): faint circle + start dot + arc to cursor.
        if let Some((cx, cy)) = state.arc_center {
            let center_p = w2s(cx, cy);
            let preview_color = Color {
                a: 0.55,
                ..self.selected_color
            };
            frame.fill(&canvas::Path::circle(center_p, 3.0), preview_color);
            if let Some((radius, start_deg)) = state.arc_radius_start {
                // Phase 2: radius and start angle are committed.
                let radius_screen = (radius as f32) * scale;
                // Faint full-circle ghost to show the radius.
                let faint = Color {
                    a: 0.18,
                    ..self.selected_color
                };
                frame.stroke(
                    &canvas::Path::circle(center_p, radius_screen),
                    canvas::Stroke::default().with_color(faint).with_width(1.0),
                );
                // Start-angle endpoint dot.
                let start_rad = start_deg.to_radians();
                let sp = w2s(
                    cx + radius * start_rad.cos(),
                    cy + radius * start_rad.sin(),
                );
                frame.fill(&canvas::Path::circle(sp, 3.0), preview_color);
                // Line from center to cursor (end-angle preview).
                if let Some((cur_x, cur_y)) = state.arc_cursor {
                    let cursor_p = w2s(cur_x, cur_y);
                    frame.stroke(
                        &canvas::Path::line(center_p, cursor_p),
                        canvas::Stroke::default()
                            .with_color(preview_color)
                            .with_width(1.0),
                    );
                    // Arc sweep from start to cursor angle.
                    // canvas::path::Arc lives in screen space (y-down), so we
                    // negate the world-space angles to compensate for the y-flip
                    // applied by w2s: screen_angle = -world_angle.
                    // Use the unwrapped end angle from state to avoid the ±180°
                    // discontinuity that raw atan2 would introduce.
                    let end_deg = state.arc_end_deg_unwrapped.unwrap_or_else(|| {
                        let dx = cur_x - cx;
                        let dy = cur_y - cy;
                        dy.atan2(dx).to_degrees()
                    });
                    let arc_path = canvas::Path::new(|builder| {
                        builder.arc(canvas::path::Arc {
                            center: center_p,
                            radius: radius_screen,
                            start_angle: iced::Radians(-(start_deg as f32).to_radians()),
                            end_angle: iced::Radians(-(end_deg as f32).to_radians()),
                        });
                    });
                    frame.stroke(
                        &arc_path,
                        canvas::Stroke::default()
                            .with_color(preview_color)
                            .with_width(stroke_px_at_zoom(SYMBOL_GRAPHIC_STROKE_PX_AT_100, scale)),
                    );
                }
            } else if let Some((cur_x, cur_y)) = state.arc_cursor {
                // Phase 1: just show the radius line to the cursor.
                let cursor_p = w2s(cur_x, cur_y);
                frame.stroke(
                    &canvas::Path::line(center_p, cursor_p),
                    canvas::Stroke::default()
                        .with_color(preview_color)
                        .with_width(1.0),
                );
            }
        }

        vec![frame.into_geometry()]
    }
}

impl<'a> SymbolCanvas<'a> {
    fn draw_symbol_with_renderer(
        &self,
        frame: &mut canvas::Frame,
        selected: &Option<SymbolSelection>,
        scale: f32,
    ) {
        let snapshot = self.build_symbol_renderer_snapshot(selected, scale);
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
        selected: &Option<SymbolSelection>,
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
            let is_selected = is_graphic_selected(selected, i);
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

            let geom = PinRenderGeometry::compute(pin);
            let selected = match selected {
                Some(SymbolSelection::Pin(j)) => *j == i,
                Some(SymbolSelection::Multiple { pin_indices, .. }) => {
                    pin_indices.contains(&i)
                }
                Some(SymbolSelection::All) => true,
                _ => false,
            };
            let stroke_color = if selected { self.selected_color } else { self.pin_color };

            let tip_f32 = [geom.tip.x as f32, geom.tip.y as f32];
            let body_f32 = [geom.body_end.x as f32, geom.body_end.y as f32];

            wires.push(WireInput {
                id: 100_000 + i as u64,
                p0: tip_f32,
                p1: body_f32,
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
                center: tip_f32,
                radius_mm: screen_px_to_world_mm(2.5, scale),
                color: to_rgba(stroke_color),
            });

            if selected {
                polygons.push(PolygonInput {
                    vertices: circle_vertices(
                        [geom.tip.x, geom.tip.y],
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

            pin_texts.push(signex_renderer::schematic::TextInput {
                content: pin.number.clone(),
                position: [geom.number_pos.x as f32, geom.number_pos.y as f32],
                size_mm: PIN_TEXT_LAYOUT.number_size_mm,
                color: to_rgba(self.text_color),
                bold: false,
                italic: false,
                rotation_rad: geom.text_rotation,
                h_align: HAlign::Center,
                v_align: VAlign::Bottom,
            });

            pin_texts.push(signex_renderer::schematic::TextInput {
                content: pin.name.clone(),
                position: [geom.name_pos.x as f32, geom.name_pos.y as f32],
                size_mm: PIN_TEXT_LAYOUT.name_size_mm,
                color: to_rgba(Color { a: 0.85, ..self.text_color }),
                bold: false,
                italic: false,
                rotation_rad: geom.text_rotation,
                h_align: geom.name_h_align,
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
