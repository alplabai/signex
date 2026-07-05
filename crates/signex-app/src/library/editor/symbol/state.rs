//! Symbol-tab editor state.
//!
//! The editor mutates a typed [`signex_library::Symbol`] primitive
//! in-place. Helpers below operate on a `&mut Symbol` so the
//! dispatcher can call them directly off the active editor state.
//!
//! Selection / hit-test / pin-add / move / delete logic preserves
//! the canvas + AI-stub apply behaviour the pre-refactor `SymbolDoc`
//! had.

use signex_library::{PinDirection, PinOrientation, Symbol, SymbolGraphicKind, SymbolPin};

/// Coarse pin classification — kept independent of the canonical
/// [`PinDirection`] so the AI-stub heuristic can hand back a
/// limited subset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PinKind {
    Input,
    Output,
    Bidirectional,
    Power,
    Passive,
    Unknown,
}

impl PinKind {
    pub fn from_ai_stub(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "input" => PinKind::Input,
            "output" => PinKind::Output,
            "bidirectional" | "bidir" => PinKind::Bidirectional,
            "power" | "power_in" | "power_out" => PinKind::Power,
            "passive" => PinKind::Passive,
            _ => PinKind::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldKey {
    Reference,
    Value,
}

/// Selected element on the Symbol canvas — drives delete + drag and
/// the right-dock Properties panel.
///
/// No longer `Copy`: the `Multiple` variant carries owned index
/// vectors from a rubber-band box selection. Callers that used to copy
/// `editor.selected` now `.clone()` it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SymbolSelection {
    Pin(usize),
    Field(FieldKey),
    /// A placed [`signex_library::SymbolGraphic`] at the given index
    /// in the active symbol's `graphics` vector. Picked up by the
    /// canvas hit-test on Select-tool clicks that miss every pin and
    /// every graphic resize handle but land inside a graphic body.
    Graphic(usize),
    /// Every pin and graphic selected together (box that encloses the
    /// whole symbol). Drag moves the whole body as a unit; rotate and
    /// delete are no-ops so a stray key press can't wipe the symbol.
    /// Salvaged from `feature/v0.13-symbol`.
    All,
    /// A specific subset of pins and graphics from a rubber-band box
    /// selection. Drag moves the group; delete removes only the
    /// selected items; rotate is a no-op (matches `All`). Salvaged
    /// from `feature/v0.13-symbol`.
    Multiple {
        pin_indices: Vec<usize>,
        graphic_indices: Vec<usize>,
    },
}

/// Box-selection mode, chosen by the caller from the drag direction.
///
/// * `Window` — select items **fully contained** in the box
///   (left-to-right drag; conventionally a solid/blue outline).
/// * `Crossing` — select items that **touch or cross** the box
///   (right-to-left drag; conventionally a dashed/green outline).
///
/// Salvaged from `feature/v0.13-symbol`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxSelectKind {
    Window,
    Crossing,
}

/// Resize-handle identity for a placed [`SymbolGraphic`]. Each
/// variant identifies one grabbable point on the graphic so the
/// canvas can fire [`canvas::CanvasAction::MoveGraphicHandle`] with
/// enough context for the dispatcher to mutate the right field.
///
/// Corner ordering for `RectCorner`: `0=TL, 1=TR, 2=BR, 3=BL` in the
/// Standard y-up world (so TL has minx + maxy).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphicHandle {
    /// Rectangle corner — `0=TL, 1=TR, 2=BR, 3=BL`.
    RectCorner(u8),
    /// Line endpoint — `0=from, 1=to`.
    LineEndpoint(u8),
    /// Circle radius handle (drawn at `(center.x + radius, center.y)`).
    CircleRadius,
    /// Arc start point on the circumference.
    ArcStart,
    /// Arc end point on the circumference.
    ArcEnd,
    /// Text anchor / `position` field.
    TextAnchor,
}

/// Default new-pin layout: place new pins to the right of the body.
const DEFAULT_PIN_LENGTH_MM: f64 = 2.54;

/// Highest declared part number across every pin on `sym`. `0`
/// v0.13 — SchLib editor active-bar dropdown menu identifier. One
/// per chevron-bearing button on the unified active bar. Mirrors the
/// footprint editor's `FpActiveBarMenu`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymActiveBarMenu {
    Filter,
    Snap,
    Place,
    Select,
    Align,
    /// Place Pin variants (input / output / passive / etc.).
    Pin,
    /// String / Text Frame.
    Text,
    /// Line / Arc / Ellipse / Polygon / Rectangle / Round Rectangle /
    /// Bezier — full SchLib shape set.
    Shapes,
}

/// v0.13 — Per-kind selectable flags for the SchLib editor.
/// Mirrors the footprint editor's SelectionFilter struct.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SymbolSelectionFilter {
    pub pins: bool,
    pub drawings: bool,
    pub texts: bool,
    pub designators: bool,
    pub values: bool,
    pub parameters: bool,
    pub other: bool,
}

impl Default for SymbolSelectionFilter {
    fn default() -> Self {
        Self {
            pins: true,
            drawings: true,
            texts: true,
            designators: true,
            values: true,
            parameters: true,
            other: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolFilterKind {
    Pins,
    Drawings,
    Texts,
    Designators,
    Values,
    Parameters,
    Other,
}

impl SymbolFilterKind {
    pub const ALTIUM_PILLS: &'static [SymbolFilterKind] = &[
        Self::Pins,
        Self::Drawings,
        Self::Texts,
        Self::Designators,
        Self::Values,
        Self::Parameters,
        Self::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Pins => "Pins",
            Self::Drawings => "Drawings",
            Self::Texts => "Texts",
            Self::Designators => "Designators",
            Self::Values => "Values",
            Self::Parameters => "Parameters",
            Self::Other => "Other",
        }
    }
}

impl SymbolSelectionFilter {
    pub fn get(&self, kind: SymbolFilterKind) -> bool {
        match kind {
            SymbolFilterKind::Pins => self.pins,
            SymbolFilterKind::Drawings => self.drawings,
            SymbolFilterKind::Texts => self.texts,
            SymbolFilterKind::Designators => self.designators,
            SymbolFilterKind::Values => self.values,
            SymbolFilterKind::Parameters => self.parameters,
            SymbolFilterKind::Other => self.other,
        }
    }

    pub fn toggle(&mut self, kind: SymbolFilterKind) {
        match kind {
            SymbolFilterKind::Pins => self.pins = !self.pins,
            SymbolFilterKind::Drawings => self.drawings = !self.drawings,
            SymbolFilterKind::Texts => self.texts = !self.texts,
            SymbolFilterKind::Designators => self.designators = !self.designators,
            SymbolFilterKind::Values => self.values = !self.values,
            SymbolFilterKind::Parameters => self.parameters = !self.parameters,
            SymbolFilterKind::Other => self.other = !self.other,
        }
    }
}

/// (Part Zero) is excluded — it's the special "appears on every
/// part" marker, not a real part. Returns `1` for symbols with no
/// pins or only Part Zero pins so multi-part wiring still has a
/// sensible "current max part = 1" baseline.
pub fn max_part_number(sym: &Symbol) -> u8 {
    let mut max = 1;
    for pin in &sym.pins {
        if pin.part_number > 0 && pin.part_number > max {
            max = pin.part_number;
        }
    }
    max
}

/// Demote every pin on the active part down to `part_number = 1`.
/// Used by Tools ▸ Remove Part — the partition disappears but the
/// pins survive on part 1.
pub fn demote_part_pins_to_part_one(sym: &mut Symbol, part: u8) {
    if part == 0 || part == 1 {
        return;
    }
    for pin in sym.pins.iter_mut() {
        if pin.part_number == part {
            pin.part_number = 1;
        }
    }
}

/// Add a pin at the given canvas coordinates and return its index in
/// `Symbol::pins`. Auto-assigns the next free numeric pin number and
/// scopes it to `part_number` (typically the editor's active sub-part
/// for multi-part components; `1` for single-part).
pub fn add_pin(sym: &mut Symbol, x: f64, y: f64, part_number: u8) -> usize {
    let next_num = next_pin_number(sym);
    let mut pin = SymbolPin::new(next_num.clone(), format!("PIN{next_num}"));
    pin.position = [x, y];
    pin.length = DEFAULT_PIN_LENGTH_MM;
    pin.part_number = part_number;
    sym.pins.push(pin);
    sym.pins.len() - 1
}

/// Pick the next integer pin number — one above the highest numeric
/// pin number, or `"1"` if no numeric pins exist.
fn next_pin_number(sym: &Symbol) -> String {
    let highest = sym
        .pins
        .iter()
        .filter_map(|p| p.number.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    (highest + 1).to_string()
}

/// Move the currently-selected element to a new canvas position.
/// Coordinates are in mm; callers should snap to the grid first.
/// For graphics this translates the entire shape so its anchor (TL
/// corner / `from` endpoint / `center` / `position`) lands on `(x, y)`.
pub fn move_selected(sym: &mut Symbol, sel: Option<SymbolSelection>, x: f64, y: f64) {
    match sel {
        Some(SymbolSelection::Pin(idx)) => {
            if let Some(pin) = sym.pins.get_mut(idx) {
                pin.position = [x, y];
            }
        }
        Some(SymbolSelection::Graphic(idx)) => {
            translate_graphic_to(sym, idx, x, y);
        }
        // SymbolSelection::Field — no-op; the on-canvas designator /
        // value drag re-binds against `ComponentRow` once that pipeline
        // ships.
        // All / Multiple — group drag is delta-based (move_all /
        // move_multiple), not absolute, so no-op here.
        Some(SymbolSelection::Field(_))
        | Some(SymbolSelection::All)
        | Some(SymbolSelection::Multiple { .. })
        | None => {}
    }
}

/// Translate the graphic at `idx` so its primary anchor lands on
/// `(x, y)`. Anchors are picked to match the visual centre of mass
/// of each shape: rectangles + lines move by the delta between the
/// new point and `from`; circles + arcs use `center`; text uses
/// `position`. No-op when `idx` is out of range.
fn translate_graphic_to(sym: &mut Symbol, idx: usize, x: f64, y: f64) {
    let Some(g) = sym.graphics.get_mut(idx) else {
        return;
    };
    match &mut g.kind {
        SymbolGraphicKind::Rectangle { from, to } => {
            let dx = x - from[0];
            let dy = y - from[1];
            from[0] += dx;
            from[1] += dy;
            to[0] += dx;
            to[1] += dy;
        }
        SymbolGraphicKind::Line { from, to } => {
            let dx = x - from[0];
            let dy = y - from[1];
            from[0] += dx;
            from[1] += dy;
            to[0] += dx;
            to[1] += dy;
        }
        SymbolGraphicKind::Circle { center, .. } | SymbolGraphicKind::Arc { center, .. } => {
            center[0] = x;
            center[1] = y;
        }
        SymbolGraphicKind::Text { position, .. } => {
            position[0] = x;
            position[1] = y;
        }
    }
}

/// Translate a graphic by `(dx, dy)` mm (delta, not absolute). Used by
/// the group-move paths (`move_all` / `move_multiple`). Salvaged from
/// `feature/v0.13-symbol`.
pub fn translate_graphic_by(kind: &mut SymbolGraphicKind, dx: f64, dy: f64) {
    match kind {
        SymbolGraphicKind::Rectangle { from, to } | SymbolGraphicKind::Line { from, to } => {
            from[0] += dx;
            from[1] += dy;
            to[0] += dx;
            to[1] += dy;
        }
        SymbolGraphicKind::Circle { center, .. } | SymbolGraphicKind::Arc { center, .. } => {
            center[0] += dx;
            center[1] += dy;
        }
        SymbolGraphicKind::Text { position, .. } => {
            position[0] += dx;
            position[1] += dy;
        }
    }
}

/// Shift every pin and graphic by `(dx, dy)` mm — the drag path for
/// `SymbolSelection::All`. Salvaged from `feature/v0.13-symbol`.
pub fn move_all(sym: &mut Symbol, dx: f64, dy: f64) {
    for pin in &mut sym.pins {
        pin.position[0] += dx;
        pin.position[1] += dy;
    }
    for g in &mut sym.graphics {
        translate_graphic_by(&mut g.kind, dx, dy);
    }
}

/// Shift only the listed pins and graphics by `(dx, dy)` mm — the drag
/// path for `SymbolSelection::Multiple`. Out-of-range indices are
/// silently skipped. Salvaged from `feature/v0.13-symbol`.
pub fn move_multiple(
    sym: &mut Symbol,
    pin_indices: &[usize],
    graphic_indices: &[usize],
    dx: f64,
    dy: f64,
) {
    for &i in pin_indices {
        if let Some(pin) = sym.pins.get_mut(i) {
            pin.position[0] += dx;
            pin.position[1] += dy;
        }
    }
    for &i in graphic_indices {
        if let Some(g) = sym.graphics.get_mut(i) {
            translate_graphic_by(&mut g.kind, dx, dy);
        }
    }
}

/// Delete whatever is currently selected. Returns `Some(new_sel)` if
/// the caller should update its selection (typically `None` after a
/// pin removal), or `None` if no selection change is needed.
pub fn delete_selected(
    sym: &mut Symbol,
    sel: Option<SymbolSelection>,
) -> Option<Option<SymbolSelection>> {
    match sel {
        Some(SymbolSelection::Pin(idx)) => {
            if idx < sym.pins.len() {
                sym.pins.remove(idx);
                Some(None)
            } else {
                None
            }
        }
        Some(SymbolSelection::Graphic(idx)) => {
            if idx < sym.graphics.len() {
                sym.graphics.remove(idx);
                Some(None)
            } else {
                None
            }
        }
        Some(SymbolSelection::Field(_)) => None,
        // Guard the whole-symbol selection: a stray Delete must not
        // wipe every primitive.
        Some(SymbolSelection::All) => None,
        Some(SymbolSelection::Multiple {
            pin_indices,
            graphic_indices,
        }) => {
            // Remove in descending index order so earlier removals
            // don't shift the indices of the ones still to delete.
            let mut pins_desc = pin_indices;
            pins_desc.sort_unstable_by(|a, b| b.cmp(a));
            for idx in pins_desc {
                if idx < sym.pins.len() {
                    sym.pins.remove(idx);
                }
            }
            let mut gfx_desc = graphic_indices;
            gfx_desc.sort_unstable_by(|a, b| b.cmp(a));
            for idx in gfx_desc {
                if idx < sym.graphics.len() {
                    sym.graphics.remove(idx);
                }
            }
            Some(None)
        }
        None => None,
    }
}

/// Hit-test cursor world coordinates against pins, then graphic
/// bodies. Pins win (small hit target, often inside graphics);
/// graphics scan in reverse so the most-recently-placed graphic
/// wins overlap.
pub fn hit_test(sym: &Symbol, x: f64, y: f64) -> Option<SymbolSelection> {
    const PIN_HIT_R_SQ: f64 = 1.5 * 1.5;
    for (i, pin) in sym.pins.iter().enumerate() {
        let dx = pin.position[0] - x;
        let dy = pin.position[1] - y;
        if dx * dx + dy * dy <= PIN_HIT_R_SQ {
            return Some(SymbolSelection::Pin(i));
        }
    }
    for idx in (0..sym.graphics.len()).rev() {
        if hit_test_graphic_body(sym, idx, x, y) {
            return Some(SymbolSelection::Graphic(idx));
        }
    }
    None
}

/// Tolerance band around line / arc / circle outlines (mm).
const GRAPHIC_BODY_TOL: f64 = 0.5;

/// Body hit test for the graphic at `idx`. Rectangle counts every
/// interior point; line / arc / circle count any point within the
/// stroke tolerance band so the user can grab thin strokes without
/// pixel-perfect aim.
fn hit_test_graphic_body(sym: &Symbol, idx: usize, x: f64, y: f64) -> bool {
    let Some(g) = sym.graphics.get(idx) else {
        return false;
    };
    match &g.kind {
        SymbolGraphicKind::Rectangle { from, to } => {
            let xmin = from[0].min(to[0]);
            let xmax = from[0].max(to[0]);
            let ymin = from[1].min(to[1]);
            let ymax = from[1].max(to[1]);
            x >= xmin && x <= xmax && y >= ymin && y <= ymax
        }
        SymbolGraphicKind::Line { from, to } => {
            point_to_segment_dist_sq([x, y], *from, *to) <= GRAPHIC_BODY_TOL * GRAPHIC_BODY_TOL
        }
        SymbolGraphicKind::Circle { center, radius } => {
            let dx = x - center[0];
            let dy = y - center[1];
            let d = (dx * dx + dy * dy).sqrt();
            (d - radius).abs() <= GRAPHIC_BODY_TOL
        }
        SymbolGraphicKind::Arc {
            center,
            radius,
            start_deg,
            end_deg,
        } => {
            let dx = x - center[0];
            let dy = y - center[1];
            let d = (dx * dx + dy * dy).sqrt();
            if (d - radius).abs() > GRAPHIC_BODY_TOL {
                return false;
            }
            // Angle of the click point in degrees, normalised to [0, 360).
            let a = dy.atan2(dx).to_degrees().rem_euclid(360.0);
            let s = start_deg.rem_euclid(360.0);
            let e = end_deg.rem_euclid(360.0);
            if s <= e {
                a >= s && a <= e
            } else {
                a >= s || a <= e
            }
        }
        SymbolGraphicKind::Text { position, size, .. } => {
            // Approximate text bounds as a small box around the anchor.
            let half_w = size * 0.5;
            let half_h = size * 0.5;
            (x - position[0]).abs() <= half_w && (y - position[1]).abs() <= half_h
        }
    }
}

fn point_to_segment_dist_sq(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let len_sq = dx * dx + dy * dy;
    if len_sq <= f64::EPSILON {
        let ddx = p[0] - a[0];
        let ddy = p[1] - a[1];
        return ddx * ddx + ddy * ddy;
    }
    let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len_sq).clamp(0.0, 1.0);
    let cx = a[0] + t * dx;
    let cy = a[1] + t * dy;
    let ddx = p[0] - cx;
    let ddy = p[1] - cy;
    ddx * ddx + ddy * ddy
}

/// Hit radius for graphic resize handles — same 1.5 mm budget as the
/// pin click target so the gesture feels consistent across the canvas.
const HANDLE_HIT_R_SQ: f64 = 1.5 * 1.5;

/// Compute the world (mm) position of a graphic's resize handle.
/// Returns `None` if the handle variant doesn't match the graphic
/// kind — defensive against stale `GraphicHandle` values lingering
/// across selection swaps.
#[allow(dead_code)]
pub fn graphic_handle_position(
    sym: &Symbol,
    idx: usize,
    handle: GraphicHandle,
) -> Option<[f64; 2]> {
    let g = sym.graphics.get(idx)?;
    Some(match (&g.kind, handle) {
        (SymbolGraphicKind::Rectangle { from, to }, GraphicHandle::RectCorner(c)) => match c {
            0 => [from[0], to[1]],   // TL
            1 => [to[0], to[1]],     // TR
            2 => [to[0], from[1]],   // BR
            3 => [from[0], from[1]], // BL
            _ => return None,
        },
        (SymbolGraphicKind::Line { from, to }, GraphicHandle::LineEndpoint(e)) => match e {
            0 => *from,
            1 => *to,
            _ => return None,
        },
        (SymbolGraphicKind::Circle { center, radius }, GraphicHandle::CircleRadius) => {
            [center[0] + radius, center[1]]
        }
        (
            SymbolGraphicKind::Arc {
                center,
                radius,
                start_deg,
                ..
            },
            GraphicHandle::ArcStart,
        ) => {
            let s = start_deg.to_radians();
            [center[0] + radius * s.cos(), center[1] + radius * s.sin()]
        }
        (
            SymbolGraphicKind::Arc {
                center,
                radius,
                end_deg,
                ..
            },
            GraphicHandle::ArcEnd,
        ) => {
            let e = end_deg.to_radians();
            [center[0] + radius * e.cos(), center[1] + radius * e.sin()]
        }
        (SymbolGraphicKind::Text { position, .. }, GraphicHandle::TextAnchor) => *position,
        _ => return None,
    })
}

/// Enumerate every resize handle for the graphic at `idx` (variant
/// + world position). Used by the canvas to draw the handle squares
/// when the Select tool is active.
pub fn graphic_handles(sym: &Symbol, idx: usize) -> Vec<(GraphicHandle, [f64; 2])> {
    let Some(g) = sym.graphics.get(idx) else {
        return Vec::new();
    };
    match &g.kind {
        SymbolGraphicKind::Rectangle { from, to } => vec![
            (GraphicHandle::RectCorner(0), [from[0], to[1]]),
            (GraphicHandle::RectCorner(1), [to[0], to[1]]),
            (GraphicHandle::RectCorner(2), [to[0], from[1]]),
            (GraphicHandle::RectCorner(3), [from[0], from[1]]),
        ],
        SymbolGraphicKind::Line { from, to } => vec![
            (GraphicHandle::LineEndpoint(0), *from),
            (GraphicHandle::LineEndpoint(1), *to),
        ],
        SymbolGraphicKind::Circle { center, radius } => {
            vec![(GraphicHandle::CircleRadius, [center[0] + radius, center[1]])]
        }
        SymbolGraphicKind::Arc {
            center,
            radius,
            start_deg,
            end_deg,
        } => {
            let s = start_deg.to_radians();
            let e = end_deg.to_radians();
            vec![
                (
                    GraphicHandle::ArcStart,
                    [center[0] + radius * s.cos(), center[1] + radius * s.sin()],
                ),
                (
                    GraphicHandle::ArcEnd,
                    [center[0] + radius * e.cos(), center[1] + radius * e.sin()],
                ),
            ]
        }
        SymbolGraphicKind::Text { position, .. } => {
            vec![(GraphicHandle::TextAnchor, *position)]
        }
    }
}

/// Hit-test world coordinates against every placed graphic's resize
/// handles. Returns `(graphic_idx, handle)` for the first hit, scanning
/// graphics in reverse so the most-recently-placed graphic wins when
/// handles overlap.
pub fn hit_test_graphic_handle(sym: &Symbol, x: f64, y: f64) -> Option<(usize, GraphicHandle)> {
    for idx in (0..sym.graphics.len()).rev() {
        for (handle, pos) in graphic_handles(sym, idx) {
            let dx = pos[0] - x;
            let dy = pos[1] - y;
            if dx * dx + dy * dy <= HANDLE_HIT_R_SQ {
                return Some((idx, handle));
            }
        }
    }
    None
}

/// Move the named handle of the graphic at `idx` to world coordinates
/// `(x, y)`. No-op when `idx` is out of range or the handle variant
/// doesn't match the graphic kind. For arc endpoints the handle drag
/// only updates the angle (radius is preserved) so the user can sweep
/// the arc without resizing it.
pub fn move_graphic_handle(sym: &mut Symbol, idx: usize, handle: GraphicHandle, x: f64, y: f64) {
    let Some(g) = sym.graphics.get_mut(idx) else {
        return;
    };
    match (&mut g.kind, handle) {
        (SymbolGraphicKind::Rectangle { from, to }, GraphicHandle::RectCorner(c)) => match c {
            0 => {
                from[0] = x;
                to[1] = y;
            }
            1 => {
                to[0] = x;
                to[1] = y;
            }
            2 => {
                to[0] = x;
                from[1] = y;
            }
            3 => {
                from[0] = x;
                from[1] = y;
            }
            _ => {}
        },
        (SymbolGraphicKind::Line { from, .. }, GraphicHandle::LineEndpoint(0)) => {
            from[0] = x;
            from[1] = y;
        }
        (SymbolGraphicKind::Line { to, .. }, GraphicHandle::LineEndpoint(1)) => {
            to[0] = x;
            to[1] = y;
        }
        (SymbolGraphicKind::Circle { center, radius }, GraphicHandle::CircleRadius) => {
            let dx = x - center[0];
            let dy = y - center[1];
            // Floor at 0.1 mm so a click on the centre doesn't make
            // the circle vanish — matches the pin-length floor.
            *radius = (dx * dx + dy * dy).sqrt().max(0.1);
        }
        (
            SymbolGraphicKind::Arc {
                center, start_deg, ..
            },
            GraphicHandle::ArcStart,
        ) => {
            *start_deg = (y - center[1]).atan2(x - center[0]).to_degrees();
        }
        (
            SymbolGraphicKind::Arc {
                center, end_deg, ..
            },
            GraphicHandle::ArcEnd,
        ) => {
            *end_deg = (y - center[1]).atan2(x - center[0]).to_degrees();
        }
        (SymbolGraphicKind::Text { position, .. }, GraphicHandle::TextAnchor) => {
            position[0] = x;
            position[1] = y;
        }
        _ => {}
    }
}

/// Rotate the currently-selected element by 90°.
///
/// * **Pins** rotate around their body-end (the symbol-body attachment
///   point) so the tip swings like a clock hand and the orientation
///   advances one quarter-turn — the pin stays joined to the body.
/// * **Graphics** rotate around the **world origin**, preserving the
///   symbol-body rotate convention where `(0, 0)` is the symbol's
///   reference point.
/// * **Fields** and the empty selection are no-ops.
///
/// `clockwise = true` rotates CW (Space), `false` CCW. Salvaged from
/// `feature/v0.13-symbol` and ported onto dev's selection model
/// without the `Rotatable2d` / `Pose2d` util layer (a 90° turn is
/// exact integer arithmetic on axis-aligned geometry). The
/// geometry-centre pivot variant (v0.13's Alt-modifier) is deferred
/// with box/multi-select.
pub fn rotate_selected(sym: &mut Symbol, sel: Option<SymbolSelection>, clockwise: bool) {
    match sel {
        Some(SymbolSelection::Pin(idx)) => {
            if let Some(pin) = sym.pins.get_mut(idx) {
                let (dx, dy) = pin_body_delta(pin);
                let body_end = [pin.position[0] + dx, pin.position[1] + dy];
                let tip = rotate_point_90(pin.position, body_end, clockwise);
                pin.position = [snap_axis_value(tip[0]), snap_axis_value(tip[1])];
                pin.orientation = rotate_pin_orientation_90(pin.orientation, clockwise);
            }
        }
        Some(SymbolSelection::Graphic(idx)) => rotate_graphic_90(sym, idx, clockwise),
        // Rotating a group would need a shared pivot + per-item
        // re-place; v0.13 treats All / Multiple rotate as a no-op, and
        // so do we until group-rotate is designed.
        Some(SymbolSelection::Field(_))
        | Some(SymbolSelection::All)
        | Some(SymbolSelection::Multiple { .. })
        | None => {}
    }
}

/// Rotate the graphic at `idx` by 90° around the world origin. No-op
/// when `idx` is out of range.
fn rotate_graphic_90(sym: &mut Symbol, idx: usize, clockwise: bool) {
    let Some(g) = sym.graphics.get_mut(idx) else {
        return;
    };
    let pivot = [0.0, 0.0];
    match &mut g.kind {
        SymbolGraphicKind::Rectangle { from, to } | SymbolGraphicKind::Line { from, to } => {
            *from = rotate_point_90(*from, pivot, clockwise);
            *to = rotate_point_90(*to, pivot, clockwise);
        }
        SymbolGraphicKind::Circle { center, .. } => {
            *center = rotate_point_90(*center, pivot, clockwise);
        }
        SymbolGraphicKind::Arc {
            center,
            start_deg,
            end_deg,
            ..
        } => {
            *center = rotate_point_90(*center, pivot, clockwise);
            let delta = if clockwise { -90.0 } else { 90.0 };
            *start_deg = (*start_deg + delta).rem_euclid(360.0);
            *end_deg = (*end_deg + delta).rem_euclid(360.0);
        }
        SymbolGraphicKind::Text { position, .. } => {
            *position = rotate_point_90(*position, pivot, clockwise);
        }
    }
}

/// Rotate `p` by ±90° around `pivot` (world mm). A quarter turn maps
/// the pivot-relative delta `(dx, dy)` to `(dy, -dx)` (CW) or
/// `(-dy, dx)` (CCW) — exact for axis-aligned geometry.
fn rotate_point_90(p: [f64; 2], pivot: [f64; 2], clockwise: bool) -> [f64; 2] {
    let dx = p[0] - pivot[0];
    let dy = p[1] - pivot[1];
    let (rx, ry) = if clockwise { (dy, -dx) } else { (-dy, dx) };
    [pivot[0] + rx, pivot[1] + ry]
}

/// `(dx, dy)` from a pin's connection tip to its body-end attachment
/// point, from orientation + length. The body-end is the pivot for pin
/// rotation so the symbol-body join is invariant.
fn pin_body_delta(pin: &SymbolPin) -> (f64, f64) {
    match pin.orientation {
        PinOrientation::Right => (pin.length, 0.0),
        PinOrientation::Up => (0.0, pin.length),
        PinOrientation::Left => (-pin.length, 0.0),
        PinOrientation::Down => (0.0, -pin.length),
        // `PinOrientation` is #[non_exhaustive]; treat any future
        // orientation like Left (body extends toward -x).
        _ => (-pin.length, 0.0),
    }
}

/// Advance a pin orientation one 90° step. Salvaged from
/// `feature/v0.13-symbol`.
fn rotate_pin_orientation_90(o: PinOrientation, clockwise: bool) -> PinOrientation {
    match (o, clockwise) {
        (PinOrientation::Up, true) => PinOrientation::Right,
        (PinOrientation::Right, true) => PinOrientation::Down,
        (PinOrientation::Down, true) => PinOrientation::Left,
        (PinOrientation::Left, true) => PinOrientation::Up,
        (PinOrientation::Up, false) => PinOrientation::Left,
        (PinOrientation::Left, false) => PinOrientation::Down,
        (PinOrientation::Down, false) => PinOrientation::Right,
        (PinOrientation::Right, false) => PinOrientation::Up,
        // Non-exhaustive enum — leave unrecognised orientations as-is.
        _ => o,
    }
}

/// Snap a coordinate within 1e-9 mm of an integer back onto the grid,
/// so repeated quarter-turns don't accrete floating-point error.
fn snap_axis_value(v: f64) -> f64 {
    let r = v.round();
    if (v - r).abs() < 1e-9 { r } else { v }
}

// ── Rubber-band box selection (salvaged from feature/v0.13-symbol) ────

/// Rubber-band box selection over all symbol primitives. The caller
/// picks `kind` from the drag direction. Returns:
/// * `None` — nothing inside the box,
/// * `Some(All)` — every pin and graphic fell in,
/// * `Some(Multiple { .. })` — a proper subset.
///
/// `Window` matches pins by tip position and graphics fully contained;
/// `Crossing` matches pins whose body segment touches the box and
/// graphics whose bounding extent overlaps it.
pub fn select_in_box(
    sym: &Symbol,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    kind: BoxSelectKind,
) -> Option<SymbolSelection> {
    let xmin = x0.min(x1);
    let xmax = x0.max(x1);
    let ymin = y0.min(y1);
    let ymax = y0.max(y1);

    let mut pin_indices = Vec::new();
    for (i, pin) in sym.pins.iter().enumerate() {
        let hit = match kind {
            BoxSelectKind::Window => {
                point_in_box(pin.position[0], pin.position[1], xmin, xmax, ymin, ymax)
            }
            BoxSelectKind::Crossing => {
                let (bdx, bdy) = pin_body_delta(pin);
                let bx = pin.position[0] + bdx;
                let by = pin.position[1] + bdy;
                point_in_box(pin.position[0], pin.position[1], xmin, xmax, ymin, ymax)
                    || point_in_box(bx, by, xmin, xmax, ymin, ymax)
                    || segment_crosses_box(
                        [pin.position[0], pin.position[1]],
                        [bx, by],
                        xmin,
                        xmax,
                        ymin,
                        ymax,
                    )
            }
        };
        if hit {
            pin_indices.push(i);
        }
    }

    let mut graphic_indices = Vec::new();
    for (i, g) in sym.graphics.iter().enumerate() {
        let hit = match kind {
            BoxSelectKind::Window => graphic_fully_inside_box(&g.kind, xmin, xmax, ymin, ymax),
            BoxSelectKind::Crossing => graphic_intersects_box(&g.kind, xmin, xmax, ymin, ymax),
        };
        if hit {
            graphic_indices.push(i);
        }
    }

    if pin_indices.is_empty() && graphic_indices.is_empty() {
        return None;
    }
    if pin_indices.len() == sym.pins.len() && graphic_indices.len() == sym.graphics.len() {
        return Some(SymbolSelection::All);
    }
    Some(SymbolSelection::Multiple {
        pin_indices,
        graphic_indices,
    })
}

fn point_in_box(x: f64, y: f64, xmin: f64, xmax: f64, ymin: f64, ymax: f64) -> bool {
    x >= xmin && x <= xmax && y >= ymin && y <= ymax
}

/// True when the graphic's full extent lies inside the box (Window).
fn graphic_fully_inside_box(
    kind: &SymbolGraphicKind,
    xmin: f64,
    xmax: f64,
    ymin: f64,
    ymax: f64,
) -> bool {
    match kind {
        SymbolGraphicKind::Rectangle { from, to } | SymbolGraphicKind::Line { from, to } => {
            let (gx0, gx1) = (from[0].min(to[0]), from[0].max(to[0]));
            let (gy0, gy1) = (from[1].min(to[1]), from[1].max(to[1]));
            gx0 >= xmin && gx1 <= xmax && gy0 >= ymin && gy1 <= ymax
        }
        SymbolGraphicKind::Circle { center, radius }
        | SymbolGraphicKind::Arc { center, radius, .. } => {
            center[0] - radius >= xmin
                && center[0] + radius <= xmax
                && center[1] - radius >= ymin
                && center[1] + radius <= ymax
        }
        SymbolGraphicKind::Text { position, size, .. } => {
            let h = size * 0.5;
            position[0] - h >= xmin
                && position[0] + h <= xmax
                && position[1] - h >= ymin
                && position[1] + h <= ymax
        }
    }
}

/// True when the graphic's bounding extent overlaps the box (Crossing).
fn graphic_intersects_box(
    kind: &SymbolGraphicKind,
    xmin: f64,
    xmax: f64,
    ymin: f64,
    ymax: f64,
) -> bool {
    match kind {
        SymbolGraphicKind::Rectangle { from, to } | SymbolGraphicKind::Line { from, to } => {
            let (gx0, gx1) = (from[0].min(to[0]), from[0].max(to[0]));
            let (gy0, gy1) = (from[1].min(to[1]), from[1].max(to[1]));
            gx0 <= xmax && gx1 >= xmin && gy0 <= ymax && gy1 >= ymin
        }
        SymbolGraphicKind::Circle { center, radius }
        | SymbolGraphicKind::Arc { center, radius, .. } => {
            let (cx, cy, r) = (center[0], center[1], *radius);
            cx - r <= xmax && cx + r >= xmin && cy - r <= ymax && cy + r >= ymin
        }
        SymbolGraphicKind::Text { position, size, .. } => {
            let h = size * 0.5;
            position[0] - h <= xmax
                && position[0] + h >= xmin
                && position[1] - h <= ymax
                && position[1] + h >= ymin
        }
    }
}

/// True when segment `a-b` has any point inside the box or crosses one
/// of its four edges.
fn segment_crosses_box(
    a: [f64; 2],
    b: [f64; 2],
    xmin: f64,
    xmax: f64,
    ymin: f64,
    ymax: f64,
) -> bool {
    if point_in_box(a[0], a[1], xmin, xmax, ymin, ymax)
        || point_in_box(b[0], b[1], xmin, xmax, ymin, ymax)
    {
        return true;
    }
    let box_edges: [([f64; 2], [f64; 2]); 4] = [
        ([xmin, ymin], [xmax, ymin]),
        ([xmax, ymin], [xmax, ymax]),
        ([xmax, ymax], [xmin, ymax]),
        ([xmin, ymax], [xmin, ymin]),
    ];
    box_edges
        .iter()
        .any(|(p, q)| segments_intersect(a, b, *p, *q))
}

/// Proper-crossing test for segments `a-b` and `c-d` (endpoints on a
/// segment don't count — good enough for rubber-band hit-testing).
fn segments_intersect(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> bool {
    let cross2d = |o: [f64; 2], p: [f64; 2], q: [f64; 2]| -> f64 {
        (p[0] - o[0]) * (q[1] - o[1]) - (p[1] - o[1]) * (q[0] - o[0])
    };
    let d1 = cross2d(c, d, a);
    let d2 = cross2d(c, d, b);
    let d3 = cross2d(a, b, c);
    let d4 = cross2d(a, b, d);
    ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_library::Symbol;

    #[test]
    fn add_pin_assigns_next_number() {
        let mut s = Symbol::empty("test");
        // Symbol::empty seeds one default pin "1".
        let idx = add_pin(&mut s, 1.0, 1.0, 1);
        assert_eq!(idx, 1);
        assert_eq!(s.pins[1].number, "2");
    }

    #[test]
    fn add_pin_records_active_part() {
        let mut s = Symbol::empty("test");
        let idx = add_pin(&mut s, 0.0, 0.0, 3);
        assert_eq!(s.pins[idx].part_number, 3);
    }

    #[test]
    fn max_part_number_ignores_part_zero() {
        let mut s = Symbol::empty("test");
        add_pin(&mut s, 0.0, 0.0, 0); // shared
        add_pin(&mut s, 0.0, 0.0, 2);
        add_pin(&mut s, 0.0, 0.0, 4);
        assert_eq!(max_part_number(&s), 4);
    }

    #[test]
    fn max_part_number_defaults_to_one() {
        let s = Symbol::empty("test");
        assert_eq!(max_part_number(&s), 1);
    }

    #[test]
    fn demote_part_pins_collapses_target_part() {
        let mut s = Symbol::empty("test");
        add_pin(&mut s, 0.0, 0.0, 2);
        add_pin(&mut s, 0.0, 0.0, 2);
        add_pin(&mut s, 0.0, 0.0, 3);
        demote_part_pins_to_part_one(&mut s, 2);
        let twos = s.pins.iter().filter(|p| p.part_number == 2).count();
        let ones = s.pins.iter().filter(|p| p.part_number == 1).count();
        assert_eq!(twos, 0);
        // 1 default + 2 demoted = 3 ones
        assert_eq!(ones, 3);
    }

    #[test]
    fn delete_pin_clears_selection_via_return() {
        let mut s = Symbol::empty("test");
        add_pin(&mut s, 1.0, 1.0, 1);
        let new_sel = delete_selected(&mut s, Some(SymbolSelection::Pin(0)));
        assert_eq!(new_sel, Some(None));
        assert_eq!(s.pins.len(), 1);
    }

    #[test]
    fn move_selected_updates_position() {
        let mut s = Symbol::empty("test");
        move_selected(&mut s, Some(SymbolSelection::Pin(0)), 5.5, -2.0);
        assert_eq!(s.pins[0].position, [5.5, -2.0]);
    }

    #[test]
    fn hit_test_returns_pin() {
        let mut s = Symbol::empty("test");
        s.pins[0].position = [3.0, 4.0];
        let sel = hit_test(&s, 3.0, 4.0);
        assert_eq!(sel, Some(SymbolSelection::Pin(0)));
    }

    #[test]
    fn graphic_handle_position_returns_rectangle_corners() {
        let mut s = Symbol::empty("test");
        s.graphics.push(signex_library::SymbolGraphic {
            kind: SymbolGraphicKind::Rectangle {
                from: [-2.0, -1.0],
                to: [2.0, 1.0],
            },
            stroke_width: 0.15,
        });
        // TL = (from.x, to.y), BR = (to.x, from.y)
        assert_eq!(
            graphic_handle_position(&s, 0, GraphicHandle::RectCorner(0)),
            Some([-2.0, 1.0])
        );
        assert_eq!(
            graphic_handle_position(&s, 0, GraphicHandle::RectCorner(2)),
            Some([2.0, -1.0])
        );
    }

    #[test]
    fn hit_test_graphic_handle_finds_rectangle_corner() {
        let mut s = Symbol::empty("test");
        s.graphics.push(signex_library::SymbolGraphic {
            kind: SymbolGraphicKind::Rectangle {
                from: [0.0, 0.0],
                to: [10.0, 5.0],
            },
            stroke_width: 0.15,
        });
        // BR corner is at (to.x, from.y) = (10.0, 0.0).
        let hit = hit_test_graphic_handle(&s, 10.0, 0.0);
        assert_eq!(hit, Some((0, GraphicHandle::RectCorner(2))));
    }

    #[test]
    fn move_graphic_handle_moves_line_endpoint() {
        let mut s = Symbol::empty("test");
        s.graphics.push(signex_library::SymbolGraphic {
            kind: SymbolGraphicKind::Line {
                from: [0.0, 0.0],
                to: [5.0, 0.0],
            },
            stroke_width: 0.15,
        });
        move_graphic_handle(&mut s, 0, GraphicHandle::LineEndpoint(1), 7.0, 3.0);
        match &s.graphics[0].kind {
            SymbolGraphicKind::Line { to, .. } => assert_eq!(*to, [7.0, 3.0]),
            _ => panic!("expected Line"),
        }
    }

    #[test]
    fn move_graphic_handle_resizes_circle_radius() {
        let mut s = Symbol::empty("test");
        s.graphics.push(signex_library::SymbolGraphic {
            kind: SymbolGraphicKind::Circle {
                center: [0.0, 0.0],
                radius: 1.0,
            },
            stroke_width: 0.15,
        });
        move_graphic_handle(&mut s, 0, GraphicHandle::CircleRadius, 3.0, 4.0);
        match &s.graphics[0].kind {
            SymbolGraphicKind::Circle { radius, .. } => assert!((*radius - 5.0).abs() < 1e-9),
            _ => panic!("expected Circle"),
        }
    }

    #[test]
    fn hit_test_returns_graphic_inside_rectangle() {
        let mut s = Symbol::empty("test");
        s.graphics.push(signex_library::SymbolGraphic {
            kind: SymbolGraphicKind::Rectangle {
                from: [0.0, 0.0],
                to: [10.0, 5.0],
            },
            stroke_width: 0.15,
        });
        // Pin still at default (0, 0) — would have to move it for clean test.
        s.pins[0].position = [-99.0, -99.0]; // park the pin off-canvas
        let hit = hit_test(&s, 5.0, 2.5);
        assert_eq!(hit, Some(SymbolSelection::Graphic(0)));
    }

    #[test]
    fn move_selected_translates_rectangle_by_anchor_delta() {
        let mut s = Symbol::empty("test");
        s.graphics.push(signex_library::SymbolGraphic {
            kind: SymbolGraphicKind::Rectangle {
                from: [0.0, 0.0],
                to: [10.0, 5.0],
            },
            stroke_width: 0.15,
        });
        move_selected(&mut s, Some(SymbolSelection::Graphic(0)), 3.0, 7.0);
        match &s.graphics[0].kind {
            SymbolGraphicKind::Rectangle { from, to } => {
                assert_eq!(*from, [3.0, 7.0]);
                assert_eq!(*to, [13.0, 12.0]);
            }
            _ => panic!("expected Rectangle"),
        }
    }

    #[test]
    fn delete_selected_removes_graphic() {
        let mut s = Symbol::empty("test");
        s.graphics.push(signex_library::SymbolGraphic {
            kind: SymbolGraphicKind::Circle {
                center: [0.0, 0.0],
                radius: 1.0,
            },
            stroke_width: 0.15,
        });
        let new_sel = delete_selected(&mut s, Some(SymbolSelection::Graphic(0)));
        assert_eq!(new_sel, Some(None));
        assert!(s.graphics.is_empty());
    }

    #[test]
    fn move_graphic_handle_no_op_for_mismatched_variant() {
        let mut s = Symbol::empty("test");
        s.graphics.push(signex_library::SymbolGraphic {
            kind: SymbolGraphicKind::Line {
                from: [0.0, 0.0],
                to: [5.0, 0.0],
            },
            stroke_width: 0.15,
        });
        // Asking to move a rectangle corner on a Line — should silently no-op.
        move_graphic_handle(&mut s, 0, GraphicHandle::RectCorner(0), 99.0, 99.0);
        match &s.graphics[0].kind {
            SymbolGraphicKind::Line { from, to } => {
                assert_eq!(*from, [0.0, 0.0]);
                assert_eq!(*to, [5.0, 0.0]);
            }
            _ => panic!("expected Line"),
        }
    }

    // ── rotate_selected (salvaged from feature/v0.13-symbol) ──────────

    #[test]
    fn rotate_selected_rotates_rectangle_clockwise_around_origin() {
        let mut s = Symbol::empty("test");
        s.graphics.push(signex_library::SymbolGraphic {
            kind: SymbolGraphicKind::Rectangle {
                from: [1.0, 2.0],
                to: [3.0, 4.0],
            },
            stroke_width: 0.15,
        });

        rotate_selected(&mut s, Some(SymbolSelection::Graphic(0)), true);

        match &s.graphics[0].kind {
            SymbolGraphicKind::Rectangle { from, to } => {
                assert_eq!(*from, [2.0, -1.0]);
                assert_eq!(*to, [4.0, -3.0]);
            }
            _ => panic!("expected Rectangle"),
        }
    }

    #[test]
    fn rotate_selected_rotates_pin_orientation_in_place() {
        let mut s = Symbol::empty("test");
        let idx = add_pin(&mut s, 2.0, 1.0, 1);
        s.pins[idx].orientation = PinOrientation::Right;

        rotate_selected(&mut s, Some(SymbolSelection::Pin(idx)), false);

        // Body-end (pivot) was at (2.0 + 2.54, 1.0) = (4.54, 1.0).
        // The tip orbits it CCW by 90°: new tip = (4.54, -1.54).
        assert_eq!(s.pins[idx].position, [4.54, -1.54]);
        assert_eq!(s.pins[idx].orientation, PinOrientation::Up);
    }

    #[test]
    fn rotate_selected_arc_advances_sweep_angles() {
        let mut s = Symbol::empty("test");
        s.graphics.push(signex_library::SymbolGraphic {
            kind: SymbolGraphicKind::Arc {
                center: [0.0, 0.0],
                radius: 2.0,
                start_deg: 0.0,
                end_deg: 90.0,
            },
            stroke_width: 0.15,
        });

        // CCW quarter-turn advances both sweep angles by +90°; the
        // centre is on the pivot so it stays fixed.
        rotate_selected(&mut s, Some(SymbolSelection::Graphic(0)), false);

        match &s.graphics[0].kind {
            SymbolGraphicKind::Arc {
                center,
                start_deg,
                end_deg,
                ..
            } => {
                assert_eq!(*center, [0.0, 0.0]);
                assert_eq!(*start_deg, 90.0);
                assert_eq!(*end_deg, 180.0);
            }
            _ => panic!("expected Arc"),
        }
    }

    #[test]
    fn rotate_selected_field_and_empty_are_noops() {
        let mut s = Symbol::empty("test");
        s.graphics.push(signex_library::SymbolGraphic {
            kind: SymbolGraphicKind::Rectangle {
                from: [1.0, 2.0],
                to: [3.0, 4.0],
            },
            stroke_width: 0.15,
        });

        rotate_selected(&mut s, Some(SymbolSelection::Field(FieldKey::Reference)), true);
        rotate_selected(&mut s, None, true);

        match &s.graphics[0].kind {
            SymbolGraphicKind::Rectangle { from, to } => {
                assert_eq!(*from, [1.0, 2.0]);
                assert_eq!(*to, [3.0, 4.0]);
            }
            _ => panic!("expected Rectangle"),
        }
    }

    // ── box selection + group move (salvaged from v0.13-symbol) ───────

    /// Empty symbol with the default pin/graphics cleared, so box tests
    /// see only the geometry they add.
    fn box_test_symbol() -> Symbol {
        let mut s = Symbol::empty("test");
        s.pins.clear();
        s.graphics.clear();
        s
    }

    fn rect(from: [f64; 2], to: [f64; 2]) -> signex_library::SymbolGraphic {
        signex_library::SymbolGraphic {
            kind: SymbolGraphicKind::Rectangle { from, to },
            stroke_width: 0.15,
        }
    }

    #[test]
    fn point_in_box_boundaries_are_inclusive() {
        assert!(point_in_box(0.0, 0.0, 0.0, 10.0, 0.0, 10.0));
        assert!(point_in_box(10.0, 10.0, 0.0, 10.0, 0.0, 10.0));
        assert!(!point_in_box(10.1, 5.0, 0.0, 10.0, 0.0, 10.0));
    }

    #[test]
    fn window_select_takes_only_fully_contained_graphics() {
        let mut s = box_test_symbol();
        s.graphics.push(rect([2.0, 2.0], [4.0, 4.0])); // idx 0 — fully inside
        s.graphics.push(rect([8.0, 8.0], [12.0, 12.0])); // idx 1 — straddles edge
        match select_in_box(&s, 0.0, 0.0, 10.0, 10.0, BoxSelectKind::Window) {
            Some(SymbolSelection::Multiple {
                pin_indices,
                graphic_indices,
            }) => {
                assert!(pin_indices.is_empty());
                assert_eq!(graphic_indices, vec![0]);
            }
            other => panic!("expected Multiple, got {other:?}"),
        }
    }

    #[test]
    fn crossing_select_takes_touching_graphics() {
        let mut s = box_test_symbol();
        s.graphics.push(rect([2.0, 2.0], [4.0, 4.0])); // inside
        s.graphics.push(rect([8.0, 8.0], [12.0, 12.0])); // straddles
        s.graphics.push(rect([20.0, 20.0], [22.0, 22.0])); // outside
        match select_in_box(&s, 0.0, 0.0, 10.0, 10.0, BoxSelectKind::Crossing) {
            Some(SymbolSelection::Multiple {
                graphic_indices, ..
            }) => {
                assert_eq!(graphic_indices, vec![0, 1]);
            }
            other => panic!("expected Multiple, got {other:?}"),
        }
    }

    #[test]
    fn select_in_box_returns_all_when_everything_enclosed() {
        let mut s = box_test_symbol();
        s.graphics.push(rect([1.0, 1.0], [2.0, 2.0]));
        add_pin(&mut s, 3.0, 3.0, 1); // tip inside the box
        assert_eq!(
            select_in_box(&s, 0.0, 0.0, 10.0, 10.0, BoxSelectKind::Window),
            Some(SymbolSelection::All)
        );
    }

    #[test]
    fn select_in_box_returns_none_when_nothing_hit() {
        let mut s = box_test_symbol();
        s.graphics.push(rect([20.0, 20.0], [22.0, 22.0]));
        assert_eq!(
            select_in_box(&s, 0.0, 0.0, 10.0, 10.0, BoxSelectKind::Window),
            None
        );
    }

    #[test]
    fn crossing_select_catches_pin_whose_body_reaches_in() {
        let mut s = box_test_symbol();
        // Tip at (12,5) is outside the (0,0)-(10,10) box, but the pin
        // points Left with length 4, so its body-end (8,5) is inside.
        let idx = add_pin(&mut s, 12.0, 5.0, 1);
        s.pins[idx].orientation = PinOrientation::Left;
        s.pins[idx].length = 4.0;
        // A far-away graphic keeps this a proper subset (→ Multiple,
        // not All) and stays unselected in both modes.
        s.graphics.push(rect([50.0, 50.0], [52.0, 52.0]));
        // Window keys off the tip only → the pin misses; nothing hit.
        assert_eq!(
            select_in_box(&s, 0.0, 0.0, 10.0, 10.0, BoxSelectKind::Window),
            None
        );
        // Crossing follows the body segment → the pin (only) is hit.
        match select_in_box(&s, 0.0, 0.0, 10.0, 10.0, BoxSelectKind::Crossing) {
            Some(SymbolSelection::Multiple {
                pin_indices,
                graphic_indices,
            }) => {
                assert_eq!(pin_indices, vec![0]);
                assert!(graphic_indices.is_empty());
            }
            other => panic!("expected Multiple, got {other:?}"),
        }
    }

    #[test]
    fn move_all_shifts_every_primitive() {
        let mut s = box_test_symbol();
        s.graphics.push(rect([0.0, 0.0], [2.0, 2.0]));
        let idx = add_pin(&mut s, 5.0, 5.0, 1);
        move_all(&mut s, 1.0, -2.0);
        assert_eq!(s.pins[idx].position, [6.0, 3.0]);
        match &s.graphics[0].kind {
            SymbolGraphicKind::Rectangle { from, to } => {
                assert_eq!(*from, [1.0, -2.0]);
                assert_eq!(*to, [3.0, 0.0]);
            }
            _ => panic!("expected Rectangle"),
        }
    }

    #[test]
    fn move_multiple_shifts_only_listed_items() {
        let mut s = box_test_symbol();
        s.graphics.push(rect([0.0, 0.0], [2.0, 2.0])); // idx 0 — moved
        s.graphics.push(rect([10.0, 10.0], [12.0, 12.0])); // idx 1 — untouched
        let p0 = add_pin(&mut s, 5.0, 5.0, 1);
        let p1 = add_pin(&mut s, 8.0, 8.0, 1);
        move_multiple(&mut s, &[p0], &[0], 1.0, 1.0);
        assert_eq!(s.pins[p0].position, [6.0, 6.0]);
        assert_eq!(s.pins[p1].position, [8.0, 8.0]);
        match &s.graphics[0].kind {
            SymbolGraphicKind::Rectangle { from, .. } => assert_eq!(*from, [1.0, 1.0]),
            _ => panic!("expected Rectangle"),
        }
        match &s.graphics[1].kind {
            SymbolGraphicKind::Rectangle { from, .. } => assert_eq!(*from, [10.0, 10.0]),
            _ => panic!("expected Rectangle"),
        }
    }

    #[test]
    fn delete_multiple_removes_in_reverse_safely() {
        let mut s = box_test_symbol();
        for i in 0..4 {
            s.graphics.push(rect([i as f64, 0.0], [i as f64 + 0.5, 1.0]));
        }
        let new = delete_selected(
            &mut s,
            Some(SymbolSelection::Multiple {
                pin_indices: vec![],
                graphic_indices: vec![1, 3],
            }),
        );
        assert_eq!(new, Some(None));
        assert_eq!(s.graphics.len(), 2);
        // The survivors are the original graphics 0 and 2.
        let xs: Vec<f64> = s
            .graphics
            .iter()
            .map(|g| match &g.kind {
                SymbolGraphicKind::Rectangle { from, .. } => from[0],
                _ => panic!("expected Rectangle"),
            })
            .collect();
        assert_eq!(xs, vec![0.0, 2.0]);
    }

    #[test]
    fn delete_all_selection_is_a_noop() {
        let mut s = box_test_symbol();
        s.graphics.push(rect([0.0, 0.0], [1.0, 1.0]));
        add_pin(&mut s, 5.0, 5.0, 1);
        assert_eq!(delete_selected(&mut s, Some(SymbolSelection::All)), None);
        assert_eq!(s.graphics.len(), 1);
        assert_eq!(s.pins.len(), 1);
    }
}
