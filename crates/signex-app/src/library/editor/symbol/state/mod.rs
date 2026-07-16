//! Symbol-tab editor state.
//!
//! The editor mutates a typed [`signex_library::Symbol`] primitive
//! in-place. Helpers below operate on a `&mut Symbol` so the
//! dispatcher can call them directly off the active editor state.
//!
//! Selection / hit-test / pin-add / move / delete logic preserves
//! the canvas + AI-stub apply behaviour the pre-refactor `SymbolDoc`
//! had.

use iced::mouse;
use signex_library::{PinOrientation, Symbol, SymbolGraphicKind, SymbolPin};
use signex_types::anchor2d::rotate_vec;
use signex_types::rotation2d::{
    Pose2d, Rotatable2d, RotationPivot, RotationSpace, Vec2d, normalize_angle_rad, rotate_object,
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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
    /// Specific subset of pins and graphics from a rubber-band box
    /// selection. Drag moves the group; delete removes only the
    /// selected items; rotate is a no-op to match `All`.
    Multiple {
        pin_indices: Vec<usize>,
        graphic_indices: Vec<usize>,
    },
}

/// Box selection mode — determined by drag direction.
///
/// `Window` selects items **fully contained** within the box
/// (left-to-right drag, blue outline).
/// `Crossing` selects items that **touch or intersect** the box
/// (right-to-left drag, green outline).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxSelectKind {
    Window,
    Crossing,
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
/// Edge ordering for `RectEdge`: `0=Top, 1=Right, 2=Bottom, 3=Left`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphicHandle {
    /// Rectangle corner — `0=TL, 1=TR, 2=BR, 3=BL`.
    RectCorner(u8),
    /// Rectangle edge midpoint — `0=Top, 1=Right, 2=Bottom, 3=Left`.
    /// Dragging an edge handle constrains movement to a single axis:
    /// top/bottom change the Y extent, left/right change the X extent.
    RectEdge(u8),
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
    /// Polygon vertex at the given index into `vertices`. A selected
    /// polygon shows one of these per vertex; dragging one moves
    /// only that vertex (mirrors `LineEndpoint`'s per-point drag).
    PolygonVertex(u16),
}

/// Map a [`GraphicHandle`] to the mouse cursor that should be shown
/// while the cursor hovers over or drags that handle.
pub fn handle_interaction(handle: GraphicHandle) -> mouse::Interaction {
    match handle {
        // TL and BR corners — resize along the \ diagonal.
        GraphicHandle::RectCorner(0) | GraphicHandle::RectCorner(2) => {
            mouse::Interaction::ResizingDiagonallyDown
        }
        // TR and BL corners — resize along the / diagonal.
        GraphicHandle::RectCorner(1) | GraphicHandle::RectCorner(3) => {
            mouse::Interaction::ResizingDiagonallyUp
        }
        // Top and bottom edge midpoints — resize vertically.
        GraphicHandle::RectEdge(0) | GraphicHandle::RectEdge(2) => {
            mouse::Interaction::ResizingVertically
        }
        // Left and right edge midpoints — resize horizontally.
        GraphicHandle::RectEdge(1) | GraphicHandle::RectEdge(3) => {
            mouse::Interaction::ResizingHorizontally
        }
        GraphicHandle::LineEndpoint(_) | GraphicHandle::TextAnchor => mouse::Interaction::Grab,
        GraphicHandle::CircleRadius | GraphicHandle::ArcStart | GraphicHandle::ArcEnd => {
            mouse::Interaction::Crosshair
        }
        _ => mouse::Interaction::Grab,
    }
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
    let pin_max = sym
        .pins
        .iter()
        .filter(|p| p.part_number > 0)
        .map(|p| p.part_number)
        .max()
        .unwrap_or(1);
    // `part_count` is the authoritative declared unit count; reconcile
    // it with the highest pin part so an empty New Part (no pins yet)
    // still counts and legacy files never lose a populated part.
    sym.part_count.max(pin_max).max(1)
}

/// Delete sub-part `part` from the symbol: drop its pins, renumber
/// every higher part down by one, and decrement the declared
/// `part_count`. Part 0 (the "appears on every part" marker) and the
/// last remaining part are never deleted. Returns the sub-part the
/// caller should make active after the delete.
pub fn delete_unit(sym: &mut Symbol, part: u8) -> u8 {
    let count = max_part_number(sym);
    if part == 0 || part > count || count <= 1 {
        // Nothing to delete — part 0 is the "every part" marker, an
        // out-of-range `part` (e.g. a stale `active_part` after undo)
        // must not decrement the count, and a single-unit symbol has
        // nothing to drop. Normalise the declared count and clamp the
        // active part to a valid unit.
        sym.part_count = count.max(1);
        return count.max(1);
    }
    sym.pins.retain(|pin| pin.part_number != part);
    for pin in sym.pins.iter_mut() {
        if pin.part_number > part {
            pin.part_number -= 1;
        }
    }
    // Graphics carry a unit too (Phase C1) — prune and renumber them
    // in lockstep with the pins so a deleted unit leaves no orphaned
    // body geometry and higher units' bodies stay aligned with their
    // pins. Shared graphics (part 0) are untouched.
    sym.graphics.retain(|g| g.part_number != part);
    for g in sym.graphics.iter_mut() {
        if g.part_number > part {
            g.part_number -= 1;
        }
    }
    sym.part_count = (count - 1).max(1);
    // Stay on the same slot index if it still exists, else clamp to
    // the new top part.
    part.min(sym.part_count).max(1)
}

/// A graphic is visible/editable on `active_part` when it is shared
/// (part 0) or scoped to that exact unit — mirrors pin part visibility.
pub fn graphic_on_part(g: &signex_library::SymbolGraphic, active_part: u8) -> bool {
    g.part_number == 0 || g.part_number == active_part
}

/// Centroid (average of every vertex) of a Polygon graphic — the
/// shared anchor definition used by canvas selection-anchor lookup,
/// rotate-pivot geometry-center, and whole-shape translate so all
/// three agree on the same point. Returns `[0.0, 0.0]` for an empty
/// vertex list (should not occur — placement always commits >= 3).
pub fn polygon_centroid(vertices: &[[f64; 2]]) -> [f64; 2] {
    let n = vertices.len().max(1) as f64;
    let (sx, sy) = vertices
        .iter()
        .fold((0.0, 0.0), |(sx, sy), v| (sx + v[0], sy + v[1]));
    [sx / n, sy / n]
}

/// A pin is visible/editable on `active_part` when it is shared (Part
/// Zero) or scoped to that exact unit — the interaction-side mirror of
/// `SymbolCanvas::pin_visible_on_active_part`, so click / box-select /
/// handle hit-tests match what the canvas actually draws.
pub fn pin_on_part(pin: &SymbolPin, active_part: u8) -> bool {
    pin.part_number == 0 || pin.part_number == active_part
}

/// Graphic indices the current selection names individually — the
/// empty `Vec` for every selection kind that doesn't name individual
/// graphics (`None`, `Pin`, `Field`, `All`). Shared by the "Join into
/// Polygon" op (`updates::join`) and its context-menu enablement
/// check (`context_menu`) so both agree on exactly which selections
/// name eligible sources.
pub fn join_source_indices(selected: &Option<SymbolSelection>) -> Vec<usize> {
    match selected {
        Some(SymbolSelection::Graphic(idx)) => vec![*idx],
        Some(SymbolSelection::Multiple {
            graphic_indices, ..
        }) => graphic_indices.clone(),
        _ => Vec::new(),
    }
}

/// Whether the current selection is eligible for "Join into Polygon":
/// at least one graphic named, and every named graphic is a `Line` or
/// an `Arc` — a Rectangle/Circle/Text/Polygon anywhere in the
/// selection disqualifies the whole op. A stale index (selection
/// outlived a delete) also disqualifies.
pub fn selection_is_join_eligible(sym: &Symbol, selected: &Option<SymbolSelection>) -> bool {
    let indices = join_source_indices(selected);
    !indices.is_empty()
        && indices.iter().all(|&idx| {
            matches!(
                sym.graphics.get(idx).map(|g| &g.kind),
                Some(SymbolGraphicKind::Line { .. }) | Some(SymbolGraphicKind::Arc { .. })
            )
        })
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

mod context_menu;
mod hit_test;
mod movement;
mod rotation;
#[cfg(test)]
mod tests;

pub use context_menu::*;
pub use hit_test::*;
pub use movement::*;
pub use rotation::*;
