//! Canvas surface types — the upward action enum, the rotate-pivot
//! mode, the tool enum, and the Program `State` struct. Pure code
//! motion out of `mod.rs`; re-exported there (`pub use types::…`) so
//! external consumers (`standalone.rs`, `documents.rs`) keep importing
//! them from `…::symbol::canvas`.

use super::super::state::{GraphicHandle, SymbolContextTarget, SymbolSelection};

/// The actions a [`SymbolCanvas`] can emit upward.
#[derive(Debug, Clone)]
pub enum CanvasAction {
    AddPin {
        x: f64,
        y: f64,
    },
    /// Place an axis-aligned rectangle spanning the two opposite
    /// corners `(from_x, from_y)` and `(to_x, to_y)` (both grid-snapped
    /// mm world positions). Emitted on the second click of a two-click
    /// draw flow (1st click = first corner, 2nd click = opposite
    /// corner); the handler normalizes the corners.
    AddRectangle {
        from_x: f64,
        from_y: f64,
        to_x: f64,
        to_y: f64,
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
    /// Append one grid-snapped vertex to the Place Polygon stash
    /// (`SymbolEditorState::polygon_vertices`). Emitted on a plain
    /// click while the `PlacePolygon` tool is active and the click
    /// doesn't match a close gesture.
    PolygonClick {
        x: f64,
        y: f64,
    },
    /// Commit the Place Polygon stash: pushes a closed-polygon graphic
    /// when it holds a valid ring (>= 3 vertices after normalising),
    /// otherwise silently discards it. Emitted by any of the close
    /// gestures — click on the first vertex, double-click, or Enter.
    PolygonCommit,
    /// Discard the Place Polygon stash with no commit. Emitted by Esc
    /// or a right-click while a polygon placement is in flight.
    PolygonCancel,
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
    /// A right-release-without-pan-motion — open the context menu at
    /// window-absolute `(x, y)`. `target` is what the release-time
    /// hit-test found (pin / graphic / empty canvas).
    ShowContextMenu {
        x: f32,
        y: f32,
        target: SymbolContextTarget,
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
/// (Circle) / Arc / Text / Polygon are the working tools;
/// `RoundRectangle` / `Bezier` / `Image` etc. live on the Active Bar
/// as stubs and are deferred to v0.9.x.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolTool {
    Select,
    AddPin,
    PlaceRectangle,
    PlaceLine,
    PlaceCircle,
    PlaceArc,
    PlaceText,
    /// Click-collect closed polygon (>= 3 vertices) — see
    /// `SymbolEditorState::polygon_vertices` and the close gestures
    /// documented on `CanvasAction::PolygonClick` / `PolygonCommit` /
    /// `PolygonCancel`.
    PlacePolygon,
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
            SymbolTool::PlacePolygon => "Polygon",
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
    /// Set the first time a right/middle-button drag actually moves
    /// (mirrors the footprint canvas's `pan_moved`). A right-release
    /// with this still `false` opens the context menu instead of
    /// having panned; cleared on release.
    pub pan_moved: bool,
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
    /// First click corner while in the `PlaceRectangle` two-click draw
    /// flow. `None` = waiting for the first corner; `Some((x, y))` =
    /// first corner set, next click commits the opposite corner.
    pub rect_from: Option<(f64, f64)>,
    /// Cursor position (snapped) updated every `CursorMoved` while
    /// `rect_from.is_some()`, used to paint the rubber-band preview.
    pub rect_cursor: Option<(f64, f64)>,
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
    /// Live cursor (snapped) updated every `CursorMoved` while the
    /// Place Polygon stash (`SymbolEditorState::polygon_vertices`,
    /// NOT stored here — see that field's doc comment) is non-empty —
    /// the open end of the rubber-band preview polyline. Purely a
    /// view-layer readout, so it's harmless for this to be ephemeral
    /// per-widget-slot state that could in principle be stale right
    /// after a tab switch; it never drives a commit by itself.
    pub polygon_cursor: Option<(f64, f64)>,
    /// Timestamp of the last `PlacePolygon` left-click — paired with
    /// `polygon_last_click_pos` to detect the close-by-double-click
    /// gesture (300 ms, same snapped vertex) without appending a
    /// duplicate vertex. Ephemeral click-timing only, not vertex data
    /// — see `polygon_cursor`'s doc comment for why that's safe to
    /// leave in this per-widget-slot state.
    pub polygon_last_click_time: Option<std::time::Instant>,
    /// Grid-snapped world position of the last `PlacePolygon`
    /// left-click, paired with `polygon_last_click_time` for the
    /// double-click check — the second click must land on the exact
    /// same snapped vertex, not just within a fixed mm radius, so a
    /// fine snap grid can't misread two adjacent-but-distinct clicks
    /// as a double-click.
    pub polygon_last_click_pos: Option<(f64, f64)>,
}
