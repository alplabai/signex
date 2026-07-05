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
use signex_library::{Symbol, SymbolGraphicKind, SymbolPin};

use super::state::{self, GraphicHandle, SymbolSelection};

/// The actions a [`SymbolCanvas`] can emit upward.
///
/// No longer `Copy`: `Select` now carries a `SymbolSelection`, which
/// owns index vectors for box selections.
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
    /// Commit a rubber-band box selection. `(x0, y0)`–`(x1, y1)` is the
    /// world-mm box (already normalised); `crossing = true` when the
    /// drag went right-to-left (touch-select), `false` for a
    /// left-to-right window (fully-contained) select. Salvaged from
    /// feature/v0.13-symbol.
    BoxSelect {
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        crossing: bool,
    },
    Move {
        x: f64,
        y: f64,
    },
    /// Move the current group selection (`All` / `Multiple`) by a
    /// per-tick world-mm delta. The reducer reads `editor.selected` to
    /// dispatch to `move_all` vs `move_multiple`. Salvaged from
    /// feature/v0.13-symbol.
    MoveGroup {
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
    /// True while the user holds right- or middle-button to pan.
    pub panning: bool,
    /// Last cursor screen position during a pan, used to compute
    /// per-frame deltas.
    pub last_pan_pos: Option<iced::Point>,
    /// Anchor (press) screen point of an in-progress rubber-band box
    /// selection. `Some` while the user left-drags on empty canvas
    /// with the Select tool. Mirrors the footprint canvas's
    /// `box_select_anchor_screen`. Salvaged from feature/v0.13-symbol.
    pub box_anchor_screen: Option<iced::Point>,
    /// Live cursor screen point during a box-select drag, updated each
    /// CursorMoved tick so `draw` can render the rubber-band to the
    /// current cursor. Cleared on release alongside the anchor.
    pub box_current_screen: Option<iced::Point>,
    /// `Some(last_world)` while dragging a whole-group selection
    /// (`All` / `Multiple`) — the previous cursor world point, so each
    /// CursorMoved tick emits the delta since the last. `None` outside
    /// a group drag. Mutually exclusive with `dragging` (single item).
    pub group_drag_last: Option<(f64, f64)>,
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

    /// Body rectangle: derived from the first `SymbolGraphicKind::Rectangle`
    /// in `symbol.graphics`, or a sensible default.
    fn body_rect(&self) -> (f64, f64, f64, f64) {
        for g in &self.symbol.graphics {
            if let SymbolGraphicKind::Rectangle { from, to } = &g.kind {
                return (from[0], from[1], to[0], to[1]);
            }
        }
        (-5.08, -2.54, 5.08, 2.54)
    }

    /// Bounding box around the body + every pin + every graphic,
    /// with a generous pad. Used by `Fit` (Home key) to centre the
    /// camera on the symbol's content.
    pub(crate) fn bbox(&self) -> (f64, f64, f64, f64) {
        let (bx0, by0, bx1, by1) = self.body_rect();
        let mut min_x = bx0.min(bx1) - 5.08;
        let mut min_y = by0.min(by1) - 5.08;
        let mut max_x = bx0.max(bx1) + 5.08;
        let mut max_y = by0.max(by1) + 5.08;
        for pin in &self.symbol.pins {
            min_x = min_x.min(pin.position[0] - 1.27);
            min_y = min_y.min(pin.position[1] - 1.27);
            max_x = max_x.max(pin.position[0] + pin.length + 1.27);
            max_y = max_y.max(pin.position[1] + 1.27);
        }
        // Include every graphic's extent so Fit doesn't leave shapes
        // off-screen.
        for g in &self.symbol.graphics {
            match &g.kind {
                SymbolGraphicKind::Rectangle { from, to }
                | SymbolGraphicKind::Line { from, to } => {
                    min_x = min_x.min(from[0]).min(to[0]);
                    min_y = min_y.min(from[1]).min(to[1]);
                    max_x = max_x.max(from[0]).max(to[0]);
                    max_y = max_y.max(from[1]).max(to[1]);
                }
                SymbolGraphicKind::Circle { center, radius }
                | SymbolGraphicKind::Arc { center, radius, .. } => {
                    min_x = min_x.min(center[0] - radius);
                    min_y = min_y.min(center[1] - radius);
                    max_x = max_x.max(center[0] + radius);
                    max_y = max_y.max(center[1] + radius);
                }
                SymbolGraphicKind::Text { position, size, .. } => {
                    min_x = min_x.min(position[0] - size);
                    min_y = min_y.min(position[1] - size);
                    max_x = max_x.max(position[0] + size);
                    max_y = max_y.max(position[1] + size);
                }
            }
        }
        (min_x, min_y, max_x, max_y)
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
                            // Capture without publishing — the actual
                            // geometry mutation rides on CursorMoved.
                            return Some(canvas::Action::capture());
                        }
                        if let Some(sel) = state::hit_test(self.symbol, wx, wy) {
                            // Pressing on a member of the current group
                            // selection drags the whole group instead of
                            // collapsing to that one item.
                            if self.hit_is_in_group(&sel) {
                                state.group_drag_last = Some((wx, wy));
                                return Some(canvas::Action::capture());
                            }
                            state.dragging = true;
                            Some(canvas::Action::publish(CanvasAction::Select(sel)).and_capture())
                        } else {
                            // Empty space: arm a rubber-band box select.
                            // We don't Deselect yet — a plain click (no
                            // drag) deselects on release; a drag commits
                            // a box select instead. Salvaged from
                            // feature/v0.13-symbol.
                            state.box_anchor_screen = Some(pos);
                            state.box_current_screen = Some(pos);
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
                if let Some((lx, ly)) = state.group_drag_last {
                    let dx = wx - lx;
                    let dy = wy - ly;
                    if dx != 0.0 || dy != 0.0 {
                        state.group_drag_last = Some((wx, wy));
                        return Some(canvas::Action::publish(CanvasAction::MoveGroup { dx, dy }));
                    }
                    return None;
                }
                if state.dragging {
                    return Some(canvas::Action::publish(CanvasAction::Move { x: wx, y: wy }));
                }
                // Box-select drag: track the live corner so `draw`
                // renders the rubber-band. Publishing CursorAt below
                // also drives the redraw (the canvas isn't cached).
                if state.box_anchor_screen.is_some() {
                    state.box_current_screen = Some(pos);
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
                state.group_drag_last = None;
                // Finish a rubber-band box select, if one was armed.
                if let (Some(a), Some(c)) =
                    (state.box_anchor_screen.take(), state.box_current_screen.take())
                {
                    // A negligible drag is a plain click on empty space
                    // → deselect. Threshold in screen pixels.
                    const CLICK_SLOP_PX: f32 = 3.0;
                    if (a.x - c.x).abs() < CLICK_SLOP_PX && (a.y - c.y).abs() < CLICK_SLOP_PX {
                        return Some(canvas::Action::publish(CanvasAction::Deselect));
                    }
                    // Unsnapped so the box matches exactly where the
                    // user dragged (grid-snapping the corners could pull
                    // boundary items in or out unexpectedly).
                    let (ax, ay) = world_unsnapped(self, a.x, a.y, bounds);
                    let (cx, cy) = world_unsnapped(self, c.x, c.y, bounds);
                    // Drag direction picks the mode: left-to-right (the
                    // release is to the right of the press) is a Window
                    // / fully-contained select; right-to-left is a
                    // Crossing / touch select. Screen x increases
                    // rightward, same as world x, so compare screen x.
                    let crossing = c.x < a.x;
                    return Some(canvas::Action::publish(CanvasAction::BoxSelect {
                        x0: ax,
                        y0: ay,
                        x1: cx,
                        y1: cy,
                        crossing,
                    }));
                }
                None
            }
            Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) => match key {
                iced::keyboard::Key::Named(iced::keyboard::key::Named::Delete)
                | iced::keyboard::Key::Named(iced::keyboard::key::Named::Backspace) => {
                    Some(canvas::Action::publish(CanvasAction::DeleteSelected))
                }
                iced::keyboard::Key::Named(iced::keyboard::key::Named::Home) => {
                    Some(canvas::Action::publish(CanvasAction::Fit))
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// Cursor feedback — salvaged from feature/v0.13-symbol, adapted to
    /// dev's canvas state. Active gestures win; otherwise the Select
    /// tool shows a grab hand over anything draggable (a resize handle,
    /// a pin, or a graphic body) and the placement tools show a
    /// crosshair for precise click placement.
    fn mouse_interaction(
        &self,
        state: &CanvasState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.panning
            || state.dragging
            || state.dragging_handle.is_some()
            || state.group_drag_last.is_some()
        {
            return mouse::Interaction::Grabbing;
        }
        let Some(pos) = cursor.position_in(bounds) else {
            return mouse::Interaction::default();
        };
        match self.tool {
            SymbolTool::Select => {
                let (wx, wy) = world_unsnapped(self, pos.x, pos.y, bounds);
                if state::hit_test_graphic_handle(self.symbol, wx, wy).is_some()
                    || state::hit_test(self.symbol, wx, wy).is_some()
                {
                    // Draggable / resizable target under the cursor.
                    mouse::Interaction::Grab
                } else {
                    // Empty canvas — a drag here starts a box select.
                    mouse::Interaction::Crosshair
                }
            }
            // Placement tools: precise click point.
            _ => mouse::Interaction::Crosshair,
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

        // Axis lines through world (0, 0) — Altium-style centre
        // crosshair so the symbol's anchor is always visible. Drawn
        // edge-to-edge across the visible viewport in a low-alpha
        // sheet-aware colour.
        let origin = w2s(0.0, 0.0);
        if origin.x >= -1.0 && origin.x <= bounds.width + 1.0 {
            let path = canvas::Path::line(
                iced::Point::new(origin.x, 0.0),
                iced::Point::new(origin.x, bounds.height),
            );
            frame.stroke(
                &path,
                canvas::Stroke::default()
                    .with_color(self.axis_color)
                    .with_width(1.0),
            );
        }
        if origin.y >= -1.0 && origin.y <= bounds.height + 1.0 {
            let path = canvas::Path::line(
                iced::Point::new(0.0, origin.y),
                iced::Point::new(bounds.width, origin.y),
            );
            frame.stroke(
                &path,
                canvas::Stroke::default()
                    .with_color(self.axis_color)
                    .with_width(1.0),
            );
        }

        // ── Body + every other graphic ──
        // Render the first Rectangle as the filled "body" (translucent
        // fill + thick stroke); all other graphics (additional rects,
        // lines, circles, arcs) render as outlines only. Selection
        // halo: the currently-selected graphic gets the accent stroke
        // colour with extra width so it stands out against the body.
        let mut body_drawn = false;
        for (i, g) in self.symbol.graphics.iter().enumerate() {
            let is_selected = self.is_graphic_selected(i);
            let stroke_color = if is_selected {
                self.selected_color
            } else {
                self.body_color
            };
            let stroke_w = if is_selected { 2.5 } else { 1.5 };
            // Rectangle defaults to a thicker stroke than other
            // outline graphics so the "body" reads cleanly; selection
            // overrides both with the accent stroke width.
            let rect_w = if is_selected { 2.5 } else { 2.0 };
            // Text colour follows the same selection rule; body uses
            // the regular text colour, selected uses the accent.
            let text_c = if is_selected {
                self.selected_color
            } else {
                self.text_color
            };
            match &g.kind {
                SymbolGraphicKind::Rectangle { from, to } => {
                    let p1 = w2s(from[0], from[1]);
                    let p2 = w2s(to[0], to[1]);
                    let top_left = iced::Point::new(p1.x.min(p2.x), p1.y.min(p2.y));
                    let size = Size::new((p2.x - p1.x).abs(), (p2.y - p1.y).abs());
                    let path = canvas::Path::rectangle(top_left, size);
                    if !body_drawn {
                        frame.fill(
                            &path,
                            Color {
                                a: 0.16,
                                ..self.body_color
                            },
                        );
                        body_drawn = true;
                    }
                    frame.stroke(
                        &path,
                        canvas::Stroke::default()
                            .with_color(stroke_color)
                            .with_width(rect_w),
                    );
                }
                SymbolGraphicKind::Line { from, to } => {
                    let mut builder = canvas::path::Builder::new();
                    builder.move_to(w2s(from[0], from[1]));
                    builder.line_to(w2s(to[0], to[1]));
                    let path = builder.build();
                    frame.stroke(
                        &path,
                        canvas::Stroke::default()
                            .with_color(stroke_color)
                            .with_width(stroke_w),
                    );
                }
                SymbolGraphicKind::Circle { center, radius } => {
                    let p = w2s(center[0], center[1]);
                    let path = canvas::Path::circle(p, (*radius as f32) * scale);
                    frame.stroke(
                        &path,
                        canvas::Stroke::default()
                            .with_color(stroke_color)
                            .with_width(stroke_w),
                    );
                }
                SymbolGraphicKind::Arc {
                    center,
                    radius,
                    start_deg,
                    end_deg,
                } => {
                    let p = w2s(center[0], center[1]);
                    let r = (*radius as f32) * scale;
                    // `start_deg` / `end_deg` are world-space angles
                    // (Y-up, CCW) — the same convention as the arc
                    // handles (`graphic_handles`) and hit-test. The
                    // canvas frame is screen-space (Y-down), so negate
                    // to map the sweep into screen space; otherwise the
                    // drawn arc mirrors across the X axis and lands in
                    // the wrong quadrant relative to its own endpoints.
                    // (Salvaged from feature/v0.13-symbol.)
                    let s = -(*start_deg as f32).to_radians();
                    let e = -(*end_deg as f32).to_radians();
                    let mut builder = canvas::path::Builder::new();
                    builder.arc(canvas::path::Arc {
                        center: p,
                        radius: r,
                        start_angle: iced::Radians(s),
                        end_angle: iced::Radians(e),
                    });
                    let path = builder.build();
                    frame.stroke(
                        &path,
                        canvas::Stroke::default()
                            .with_color(stroke_color)
                            .with_width(stroke_w),
                    );
                }
                SymbolGraphicKind::Text {
                    position,
                    content,
                    size: text_size,
                } => {
                    frame.fill_text(canvas::Text {
                        content: content.clone(),
                        position: w2s(position[0], position[1]),
                        size: ((*text_size as f32) * scale * 0.5).into(),
                        color: text_c,
                        ..canvas::Text::default()
                    });
                }
            }
        }

        // No graphics → fall back to a default body rectangle so the
        // user sees the symbol bounds while the body geometry is still
        // empty.
        if !body_drawn {
            let (bx0, by0, bx1, by1) = self.body_rect();
            let p1 = w2s(bx0, by0);
            let p2 = w2s(bx1, by1);
            let top_left = iced::Point::new(p1.x.min(p2.x), p1.y.min(p2.y));
            let size = Size::new((p2.x - p1.x).abs(), (p2.y - p1.y).abs());
            let path = canvas::Path::rectangle(top_left, size);
            frame.fill(
                &path,
                Color {
                    a: 0.10,
                    ..self.body_color
                },
            );
            frame.stroke(
                &path,
                canvas::Stroke::default()
                    .with_color(Color {
                        a: 0.4,
                        ..self.body_color
                    })
                    .with_width(1.0),
            );
        }

        // Pins — filtered by active_part. Pins with part_number == 0
        // (Part Zero) render on every part; other pins render only
        // when their part matches editor.active_part.
        for (i, pin) in self.symbol.pins.iter().enumerate() {
            if !self.pin_visible_on_active_part(pin) {
                continue;
            }
            self.draw_pin(&mut frame, &w2s, scale, pin, i);
        }

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
                            .with_width(1.0),
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
            ..canvas::Text::default()
        });

        // Rubber-band box (screen space, no camera transform). Solid
        // accent outline for a left-to-right window select; a lighter
        // fill hints at the touch (crossing) select on right-to-left.
        if let (Some(a), Some(c)) = (state.box_anchor_screen, state.box_current_screen) {
            let top_left = iced::Point::new(a.x.min(c.x), a.y.min(c.y));
            let size = Size::new((c.x - a.x).abs(), (c.y - a.y).abs());
            let path = canvas::Path::rectangle(top_left, size);
            let crossing = c.x < a.x;
            frame.fill(
                &path,
                Color {
                    a: if crossing { 0.10 } else { 0.06 },
                    ..self.selected_color
                },
            );
            frame.stroke(
                &path,
                canvas::Stroke::default()
                    .with_color(self.selected_color)
                    .with_width(1.0),
            );
        }

        vec![frame.into_geometry()]
    }
}

impl<'a> SymbolCanvas<'a> {
    /// True when pin `idx` is part of the current selection — as the
    /// single `Pin`, or within an `All` / `Multiple` group select.
    fn is_pin_selected(&self, idx: usize) -> bool {
        match &self.selected {
            Some(SymbolSelection::Pin(i)) => *i == idx,
            Some(SymbolSelection::All) => true,
            Some(SymbolSelection::Multiple { pin_indices, .. }) => pin_indices.contains(&idx),
            _ => false,
        }
    }

    /// True when a freshly hit-tested single item (`sel`) belongs to
    /// the current group selection — i.e. pressing it should drag the
    /// whole group rather than reselect just that item. Only `All` and
    /// `Multiple` are groups; a single Pin/Graphic selection is not.
    fn hit_is_in_group(&self, sel: &SymbolSelection) -> bool {
        match &self.selected {
            Some(SymbolSelection::All) => {
                matches!(sel, SymbolSelection::Pin(_) | SymbolSelection::Graphic(_))
            }
            Some(SymbolSelection::Multiple {
                pin_indices,
                graphic_indices,
            }) => match sel {
                SymbolSelection::Pin(i) => pin_indices.contains(i),
                SymbolSelection::Graphic(i) => graphic_indices.contains(i),
                _ => false,
            },
            _ => false,
        }
    }

    /// True when graphic `idx` is part of the current selection — as
    /// the single `Graphic`, or within an `All` / `Multiple` group.
    fn is_graphic_selected(&self, idx: usize) -> bool {
        match &self.selected {
            Some(SymbolSelection::Graphic(i)) => *i == idx,
            Some(SymbolSelection::All) => true,
            Some(SymbolSelection::Multiple {
                graphic_indices, ..
            }) => graphic_indices.contains(&idx),
            _ => false,
        }
    }

    fn draw_pin<F>(
        &self,
        frame: &mut canvas::Frame,
        w2s: &F,
        _scale: f32,
        pin: &SymbolPin,
        idx: usize,
    ) where
        F: Fn(f64, f64) -> iced::Point,
    {
        use signex_library::PinOrientation;
        let (dx, dy) = match pin.orientation {
            PinOrientation::Right => (pin.length, 0.0),
            PinOrientation::Up => (0.0, pin.length),
            PinOrientation::Left => (-pin.length, 0.0),
            PinOrientation::Down => (0.0, -pin.length),
            // `PinOrientation` is `non_exhaustive` — fall back to a
            // sensible default if signex-library adds new variants.
            _ => (-pin.length, 0.0),
        };
        let tip = w2s(pin.position[0], pin.position[1]);
        let body_end = w2s(pin.position[0] + dx, pin.position[1] + dy);
        let selected = self.is_pin_selected(idx);
        let stroke_color = if selected {
            self.selected_color
        } else {
            self.pin_color
        };

        frame.stroke(
            &canvas::Path::line(tip, body_end),
            canvas::Stroke::default()
                .with_color(stroke_color)
                .with_width(if selected { 2.5 } else { 1.5 }),
        );
        // Selection halo.
        if selected {
            frame.stroke(
                &canvas::Path::circle(tip, 5.0),
                canvas::Stroke::default()
                    .with_color(self.selected_color)
                    .with_width(1.0),
            );
        }
        // Marker dot at the electrical end.
        frame.fill(&canvas::Path::circle(tip, 2.5), stroke_color);

        // Pin number + name — rotate to run along the pin (Y-flip
        // corrected) and put the name on the side that extends away
        // from the tip. Salvaged from feature/v0.13-symbol's
        // PinRenderGeometry text logic. (Pixel offsets are tunable.)
        let tg = state::PinTextGeometry::compute(pin.orientation);

        // Number — centred over the pin line, nudged just above it in
        // the pin's own rotated frame so it stays above for vertical
        // pins too.
        let mid = iced::Point::new((tip.x + body_end.x) * 0.5, (tip.y + body_end.y) * 0.5);
        frame.with_save(|inner| {
            inner.translate(iced::Vector::new(mid.x, mid.y));
            inner.rotate(iced::Radians(tg.text_rotation));
            inner.fill_text(canvas::Text {
                content: pin.number.clone(),
                position: iced::Point::new(0.0, -10.0),
                size: 10.0.into(),
                color: self.text_color,
                ..canvas::Text::default()
            });
        });

        // Name — anchored past the body_end (away from the tip in world
        // space, so it lands correctly for every orientation), rotated
        // to match. `name_flipped` (Left pins) reverses the local x so
        // the name still runs outward.
        const NAME_GAP_MM: f64 = 0.8;
        let len = (dx * dx + dy * dy).sqrt().max(1e-9);
        let (ux, uy) = (dx / len, dy / len);
        let name_anchor = w2s(
            pin.position[0] + dx + ux * NAME_GAP_MM,
            pin.position[1] + dy + uy * NAME_GAP_MM,
        );
        frame.with_save(|inner| {
            inner.translate(iced::Vector::new(name_anchor.x, name_anchor.y));
            inner.rotate(iced::Radians(tg.text_rotation));
            inner.fill_text(canvas::Text {
                content: pin.name.clone(),
                position: iced::Point::new(if tg.name_flipped { -4.0 } else { 4.0 }, -6.0),
                size: 10.0.into(),
                color: Color {
                    a: 0.85,
                    ..self.text_color
                },
                ..canvas::Text::default()
            });
        });
    }
}
