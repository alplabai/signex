//! Designator and value (comment) text fields on the symbol canvas.
//!
//! The library `Symbol` keeps an optional symbol-local position per
//! field (`designator_pos` / `comment_pos`). While that is `None` the
//! field sits at a default placement derived from the body extent, so
//! an untouched symbol needs no stored position and a fresh drag is the
//! first thing that writes one.

use super::hit_test::text_bounds;
use super::*;

/// Rendered text size of both fields, in mm (matches the pin labels).
pub const FIELD_TEXT_SIZE_MM: f64 = 1.27;

/// Clearance between the body extent and a default-placed field, mm.
const FIELD_GAP_MM: f64 = 0.5;

/// Half-size of the extent assumed for a symbol with no pins or
/// graphics, so the fields still land somewhere sensible.
const EMPTY_EXTENT_HALF_MM: f64 = 2.54;

/// Padding around a field's text box that still counts as a hit, mm.
const FIELD_HIT_PAD_MM: f64 = 0.3;

/// Drawn when the comment is empty, so the field stays visible and
/// grabbable.
const VALUE_PLACEHOLDER: &str = "Value";

/// Drawn when the designator is empty.
const DESIGNATOR_PLACEHOLDER: &str = "Designator";

/// The string a field is drawn with.
pub fn field_text(sym: &Symbol, key: FieldKey) -> &str {
    let (text, placeholder) = match key {
        FieldKey::Reference => (sym.designator.as_str(), DESIGNATOR_PLACEHOLDER),
        FieldKey::Value => (sym.comment.as_str(), VALUE_PLACEHOLDER),
    };
    if text.is_empty() { placeholder } else { text }
}

/// Anchor (top-left corner of the text, symbol-local mm, y-up) of a
/// field: the stored position, else the default placement. The default
/// puts the designator just above the top-left of the body extent and
/// the value just below its bottom-left. The default comes from the
/// active part's extent, while a stored position is shared by all parts.
pub fn field_pos(sym: &Symbol, key: FieldKey, active_part: u8) -> [f64; 2] {
    let stored = match key {
        FieldKey::Reference => sym.designator_pos,
        FieldKey::Value => sym.comment_pos,
    };
    stored.unwrap_or_else(|| {
        let (min_x, min_y, _, max_y) = body_extent(sym, active_part);
        match key {
            // Text hangs down from its anchor, so lift it by one line.
            FieldKey::Reference => [min_x, max_y + FIELD_GAP_MM + FIELD_TEXT_SIZE_MM],
            FieldKey::Value => [min_x, min_y - FIELD_GAP_MM],
        }
    })
}

/// Estimated drawn bounds `(xmin, xmax, ymin, ymax)` of a field's text,
/// padded for grabbing. Same rule as a `Text` graphic (left / top
/// anchored) so the hit box tracks what the canvas draws.
pub fn field_bounds(sym: &Symbol, key: FieldKey, active_part: u8) -> (f64, f64, f64, f64) {
    text_bounds(
        &field_pos(sym, key, active_part),
        field_text(sym, key),
        FIELD_TEXT_SIZE_MM,
        FIELD_HIT_PAD_MM,
    )
}

/// Which field's text box contains `(x, y)`, honouring the Designators
/// and Values filter pills. The designator wins an overlap.
pub fn hit_test_field(
    sym: &Symbol,
    x: f64,
    y: f64,
    active_part: u8,
    filter: &SymbolSelectionFilter,
) -> Option<FieldKey> {
    [
        (FieldKey::Reference, filter.designators),
        (FieldKey::Value, filter.values),
    ]
    .into_iter()
    .filter(|&(_, selectable)| selectable)
    .map(|(key, _)| key)
    .find(|&key| {
        let (x0, x1, y0, y1) = field_bounds(sym, key, active_part);
        x >= x0 && x <= x1 && y >= y0 && y <= y1
    })
}

/// Put a field's anchor at `(x, y)`, pinning it so it stops following
/// the default placement.
pub fn move_field_to(sym: &mut Symbol, key: FieldKey, x: f64, y: f64) {
    match key {
        FieldKey::Reference => sym.designator_pos = Some([x, y]),
        FieldKey::Value => sym.comment_pos = Some([x, y]),
    }
}

/// Whether putting a field's anchor at `(x, y)` would leave it where it
/// already is (within float noise), so a click with cursor jitter does not
/// turn a default placement into a stored position.
pub fn field_move_is_noop(sym: &Symbol, key: FieldKey, active_part: u8, x: f64, y: f64) -> bool {
    let [fx, fy] = field_pos(sym, key, active_part);
    (fx - x).abs() < 1e-9 && (fy - y).abs() < 1e-9
}

/// Shift the explicitly placed fields by `(dx, dy)`. Default-placed
/// fields already follow the body, so they are left alone. Callers
/// shift only on a whole-symbol move (see `move_multiple`): a default
/// placement follows the ACTIVE unit's extent, while a stored position
/// is one value shared by every unit.
pub fn shift_placed_fields(sym: &mut Symbol, dx: f64, dy: f64) {
    for pos in [&mut sym.designator_pos, &mut sym.comment_pos]
        .into_iter()
        .flatten()
    {
        pos[0] += dx;
        pos[1] += dy;
    }
}

/// `(min_x, min_y, max_x, max_y)` of the pins and graphics visible on
/// `active_part` (not the fields themselves).
fn body_extent(sym: &Symbol, active_part: u8) -> (f64, f64, f64, f64) {
    let mut pts: Vec<[f64; 2]> = Vec::new();
    for pin in sym.pins.iter().filter(|p| pin_on_part(p, active_part)) {
        let (dx, dy) = super::rotation::pin_body_delta(pin);
        pts.push(pin.position);
        pts.push([pin.position[0] + dx, pin.position[1] + dy]);
    }
    for g in sym
        .graphics
        .iter()
        .filter(|g| graphic_on_part(g, active_part))
    {
        match &g.kind {
            SymbolGraphicKind::Rectangle { from, to } | SymbolGraphicKind::Line { from, to } => {
                pts.extend([*from, *to]);
            }
            SymbolGraphicKind::Circle { center, radius }
            | SymbolGraphicKind::Arc { center, radius, .. } => {
                pts.push([center[0] - radius, center[1] - radius]);
                pts.push([center[0] + radius, center[1] + radius]);
            }
            SymbolGraphicKind::Text { position, .. } => pts.push(*position),
            SymbolGraphicKind::Polygon { vertices } => pts.extend(vertices.iter().copied()),
        }
    }
    if pts.is_empty() {
        let h = EMPTY_EXTENT_HALF_MM;
        return (-h, -h, h, h);
    }
    pts.iter().fold(
        (f64::MAX, f64::MAX, f64::MIN, f64::MIN),
        |(x0, y0, x1, y1), p| (x0.min(p[0]), y0.min(p[1]), x1.max(p[0]), y1.max(p[1])),
    )
}
