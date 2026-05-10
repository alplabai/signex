//! Symbol-tab editor state.
//!
//! The editor mutates a typed [`signex_library::Symbol`] primitive
//! in-place. Helpers below operate on a `&mut Symbol` so the
//! dispatcher can call them directly off the active editor state.
//!
//! Selection / hit-test / pin-add / move / delete logic preserves
//! the canvas + AI-stub apply behaviour the pre-refactor `SymbolDoc`
//! had.

use signex_library::{PinOrientation, Symbol, SymbolGraphicKind, SymbolPin};
use signex_types::anchor2d::rotate_vec;
use signex_types::rotation2d::{
    normalize_angle_rad, rotate_object, Pose2d, Rotatable2d, RotationPivot, RotationSpace,
    Vec2d,
};

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolSelection {
    Pin(usize),
    Field(FieldKey),
    /// A placed [`signex_library::SymbolGraphic`] at the given index
    /// in the active symbol's `graphics` vector. Picked up by the
    /// canvas hit-test on Select-tool clicks that miss every pin and
    /// every graphic resize handle but land inside a graphic body.
    Graphic(usize),
    /// All pins and graphics selected together (Ctrl+A). Drag moves
    /// the whole symbol body as a unit; rotate/delete are no-ops so
    /// the user cannot accidentally wipe the entire symbol with a
    /// key press.
    All,
}

/// Pivot mode for Symbol graphic rotation.
///
/// `WorldOrigin` preserves legacy behavior where geometry orbits `(0, 0)`.
/// `GeometryCenter` rotates each graphic around its own center.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphicRotationPivotMode {
    WorldOrigin,
    GeometryCenter,
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
        // SymbolSelection::All — use move_all for delta-based movement.
        Some(SymbolSelection::Field(_)) | Some(SymbolSelection::All) | None => {}
    }
}

/// Shift every pin and every graphic by `(dx, dy)` mm.
///
/// Used when the user drags with `SymbolSelection::All` active (Ctrl+A
/// select-all). The caller is responsible for computing the delta and
/// for grid-snapping if desired.
pub fn move_all(sym: &mut Symbol, dx: f64, dy: f64) {
    for pin in &mut sym.pins {
        pin.position[0] += dx;
        pin.position[1] += dy;
    }
    for graphic in &mut sym.graphics {
        translate_graphic_by(&mut graphic.kind, dx, dy);
    }
}

/// Rotate the selected entity by 90°.
///
/// Pins rotate in place around their own center (orientation only),
/// while graphics rotate around world origin to preserve the existing
/// symbol-body rotate behavior.
/// `clockwise = true` rotates CW, `false` rotates CCW.
pub fn rotate_selected(sym: &mut Symbol, sel: Option<SymbolSelection>, clockwise: bool) {
    rotate_selected_with_pivot(sym, sel, clockwise, GraphicRotationPivotMode::WorldOrigin);
}

/// Rotate the selected entity by 90° with explicit graphic pivot mode.
pub fn rotate_selected_with_pivot(
    sym: &mut Symbol,
    sel: Option<SymbolSelection>,
    clockwise: bool,
    graphic_pivot_mode: GraphicRotationPivotMode,
) {
    match sel {
        Some(SymbolSelection::Pin(idx)) => {
            if let Some(pin) = sym.pins.get_mut(idx) {
                // Rotate tip around the body-end so the symbol-body
                // attachment point stays fixed (B-type anchor behavior).
                let (dx, dy) = pin_body_delta(pin);
                let body_end = Vec2d::new(pin.position[0] + dx, pin.position[1] + dy);
                let tip_local = Vec2d::new(-dx, -dy);
                let delta = if clockwise {
                    -std::f64::consts::FRAC_PI_2
                } else {
                    std::f64::consts::FRAC_PI_2
                };
                let new_tip_local = rotate_vec(tip_local, delta);
                pin.position[0] = snap_axis_value(body_end.x + new_tip_local.x);
                pin.position[1] = snap_axis_value(body_end.y + new_tip_local.y);
                pin.orientation = rotate_pin_orientation_90(pin.orientation, clockwise);
            }
        }
        Some(SymbolSelection::Graphic(idx)) => {
            rotate_graphic_90(sym, idx, clockwise, graphic_pivot_mode)
        }
        Some(SymbolSelection::Field(_)) | Some(SymbolSelection::All) | None => {}
    }
}

/// Convenience wrapper for geometry-center graphic rotation scenarios.
pub fn rotate_selected_about_geometry_center(
    sym: &mut Symbol,
    sel: Option<SymbolSelection>,
    clockwise: bool,
) {
    rotate_selected_with_pivot(sym, sel, clockwise, GraphicRotationPivotMode::GeometryCenter);
}

fn rotate_graphic_90(
    sym: &mut Symbol,
    idx: usize,
    clockwise: bool,
    graphic_pivot_mode: GraphicRotationPivotMode,
) {
    let Some(g) = sym.graphics.get_mut(idx) else {
        return;
    };

    let geometry_center = graphic_geometry_center(&g.kind);
    let pivot = match graphic_pivot_mode {
        GraphicRotationPivotMode::WorldOrigin => RotationPivot::WorldOrigin,
        GraphicRotationPivotMode::GeometryCenter => RotationPivot::GeometryCenter,
    };

    match &mut g.kind {
        SymbolGraphicKind::Rectangle { from, to } | SymbolGraphicKind::Line { from, to } => {
            *from = rotate_graphic_point_90(
                *from,
                geometry_center,
                clockwise,
                RotationSpace::World,
                pivot,
            );
            *to = rotate_graphic_point_90(
                *to,
                geometry_center,
                clockwise,
                RotationSpace::World,
                pivot,
            );
        }
        SymbolGraphicKind::Circle { center, .. } => {
            *center = rotate_graphic_point_90(
                *center,
                geometry_center,
                clockwise,
                RotationSpace::World,
                pivot,
            );
        }
        SymbolGraphicKind::Arc {
            center,
            start_deg,
            end_deg,
            ..
        } => {
            *center = rotate_graphic_point_90(
                *center,
                geometry_center,
                clockwise,
                RotationSpace::World,
                pivot,
            );
            let delta = if clockwise {
                -std::f64::consts::FRAC_PI_2
            } else {
                std::f64::consts::FRAC_PI_2
            };
            *start_deg = normalize_angle_rad((*start_deg).to_radians() + delta)
                .to_degrees()
                .rem_euclid(360.0);
            *end_deg = normalize_angle_rad((*end_deg).to_radians() + delta)
                .to_degrees()
                .rem_euclid(360.0);
        }
        SymbolGraphicKind::Text { position, .. } => {
            *position = rotate_graphic_point_90(
                *position,
                geometry_center,
                clockwise,
                RotationSpace::World,
                pivot,
            );
        }
    }
}

fn graphic_geometry_center(kind: &SymbolGraphicKind) -> [f64; 2] {
    match kind {
        SymbolGraphicKind::Rectangle { from, to } | SymbolGraphicKind::Line { from, to } => {
            [(from[0] + to[0]) * 0.5, (from[1] + to[1]) * 0.5]
        }
        SymbolGraphicKind::Circle { center, .. } | SymbolGraphicKind::Arc { center, .. } => {
            *center
        }
        SymbolGraphicKind::Text { position, .. } => *position,
    }
}

fn rotate_graphic_point_90(
    p: [f64; 2],
    geometry_center_world: [f64; 2],
    clockwise: bool,
    space: RotationSpace,
    pivot: RotationPivot,
) -> [f64; 2] {
    let delta = if clockwise {
        -std::f64::consts::FRAC_PI_2
    } else {
        std::f64::consts::FRAC_PI_2
    };

    let mut point_object = GraphicPointObject::new(p, geometry_center_world);
    rotate_object(&mut point_object, space, pivot, delta);
    point_object.world_point()
}

fn snap_axis_value(v: f64) -> f64 {
    if v.abs() < 1e-12 {
        return 0.0;
    }

    let rounded = v.round();
    if (v - rounded).abs() < 1e-12 {
        rounded
    } else {
        v
    }
}

#[derive(Debug, Clone, Copy)]
struct GraphicPointObject {
    pose: Pose2d,
    geometry_center_local: Vec2d,
}

impl GraphicPointObject {
    fn new(world_point: [f64; 2], geometry_center_world: [f64; 2]) -> Self {
        let point_origin = Vec2d::new(world_point[0], world_point[1]);
        let center_local = Vec2d::new(
            geometry_center_world[0] - world_point[0],
            geometry_center_world[1] - world_point[1],
        );
        Self {
            pose: Pose2d::new(point_origin, 0.0),
            geometry_center_local: center_local,
        }
    }

    fn world_point(self) -> [f64; 2] {
        [
            snap_axis_value(self.pose.origin.x),
            snap_axis_value(self.pose.origin.y),
        ]
    }
}

impl Rotatable2d for GraphicPointObject {
    fn pose(&self) -> Pose2d {
        self.pose
    }

    fn geometry_center_local(&self) -> Vec2d {
        self.geometry_center_local
    }

    fn set_pose(&mut self, pose: Pose2d) {
        self.pose = pose;
    }
}

/// Returns the (dx, dy) vector from the tip (connection point) to the
/// body-end (symbol-body attachment point) based on orientation and length.
fn pin_body_delta(pin: &SymbolPin) -> (f64, f64) {
    match pin.orientation {
        PinOrientation::Right => (pin.length, 0.0),
        PinOrientation::Up => (0.0, pin.length),
        PinOrientation::Left => (-pin.length, 0.0),
        PinOrientation::Down => (0.0, -pin.length),
        _ => (-pin.length, 0.0),
    }
}

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
        _ => o,
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

/// Shift a single graphic kind by `(dx, dy)` mm in-place.
pub fn translate_graphic_by(kind: &mut SymbolGraphicKind, dx: f64, dy: f64) {
    match kind {
        SymbolGraphicKind::Rectangle { from, to } => {
            from[0] += dx; from[1] += dy;
            to[0]   += dx; to[1]   += dy;
        }
        SymbolGraphicKind::Line { from, to } => {
            from[0] += dx; from[1] += dy;
            to[0]   += dx; to[1]   += dy;
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
        Some(SymbolSelection::All) => None,
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

/// Enumerate every resize handle for the graphic at `idx`.
/// Returns `(handle_variant, world_position)` pairs for Select-tool
/// handle rendering.
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

#[cfg(test)]
mod tests {
    use super::*;
    use signex_library::Symbol;

    #[test]
    fn add_pin_assigns_next_number() {
        let mut s = Symbol::empty("test");
        add_pin(&mut s, 0.0, 0.0, 1); // seed first pin so numbering starts at "1"
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
        // 2 demoted from part 2, no default pin
        assert_eq!(ones, 2);
    }

    #[test]
    fn delete_pin_clears_selection_via_return() {
        let mut s = Symbol::empty("test");
        add_pin(&mut s, 0.0, 0.0, 1); // first pin
        add_pin(&mut s, 1.0, 1.0, 1); // second pin
        let new_sel = delete_selected(&mut s, Some(SymbolSelection::Pin(0)));
        assert_eq!(new_sel, Some(None));
        assert_eq!(s.pins.len(), 1);
    }

    #[test]
    fn move_selected_updates_position() {
        let mut s = Symbol::empty("test");
        add_pin(&mut s, 0.0, 0.0, 1);
        move_selected(&mut s, Some(SymbolSelection::Pin(0)), 5.5, -2.0);
        assert_eq!(s.pins[0].position, [5.5, -2.0]);
    }

    #[test]
    fn hit_test_returns_pin() {
        let mut s = Symbol::empty("test");
        add_pin(&mut s, 3.0, 4.0, 1);
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
        // No pins in empty symbol — graphic hit is unambiguous.
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
        // Tip orbits around it CCW by 90°: new tip = (4.54, -1.54).
        assert_eq!(s.pins[idx].position, [4.54, -1.54]);
        assert_eq!(s.pins[idx].orientation, PinOrientation::Up);
    }

    #[test]
    fn rotate_selected_about_geometry_center_keeps_rectangle_center() {
        let mut s = Symbol::empty("test");
        s.graphics.push(signex_library::SymbolGraphic {
            kind: SymbolGraphicKind::Rectangle {
                from: [1.0, 2.0],
                to: [3.0, 4.0],
            },
            stroke_width: 0.15,
        });

        rotate_selected_with_pivot(
            &mut s,
            Some(SymbolSelection::Graphic(0)),
            true,
            GraphicRotationPivotMode::GeometryCenter,
        );

        match &s.graphics[0].kind {
            SymbolGraphicKind::Rectangle { from, to } => {
                assert_eq!(*from, [1.0, 4.0]);
                assert_eq!(*to, [3.0, 2.0]);
            }
            _ => panic!("expected Rectangle"),
        }
    }

    #[test]
    fn rotate_selected_about_geometry_center_keeps_text_anchor_fixed() {
        let mut s = Symbol::empty("test");
        s.graphics.push(signex_library::SymbolGraphic {
            kind: SymbolGraphicKind::Text {
                position: [5.0, -7.0],
                content: "R".into(),
                size: 1.0,
            },
            stroke_width: 0.15,
        });

        rotate_selected_about_geometry_center(&mut s, Some(SymbolSelection::Graphic(0)), false);

        match &s.graphics[0].kind {
            SymbolGraphicKind::Text { position, .. } => {
                assert_eq!(*position, [5.0, -7.0]);
            }
            _ => panic!("expected Text"),
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
}
