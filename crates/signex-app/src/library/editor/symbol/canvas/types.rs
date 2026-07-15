//! Canvas surface types — the upward action enum, the rotate-pivot
//! mode, the tool enum, and the Program `State` struct. Pure code
//! motion out of `mod.rs`; re-exported there (`pub use types::…`) so
//! external consumers (`standalone.rs`, `documents.rs`) keep importing
//! them from `…::symbol::canvas`.

use super::super::state::{GraphicHandle, SymbolSelection};

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
