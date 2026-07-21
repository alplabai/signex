//! Move + box-select operations for the symbol editor.

use super::rotation::{pin_body_delta, translate_graphic_to};
use super::*;

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
        // All / Multiple — delta-based movement handled by move_all / move_multiple.
        Some(SymbolSelection::Field(_))
        | Some(SymbolSelection::All)
        | Some(SymbolSelection::Multiple { .. })
        | None => {}
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

/// Shift only the specified pins and graphics by `(dx, dy)` mm.
///
/// Used when the user drags with `SymbolSelection::Multiple` active
/// (box selection result). Out-of-range indices are silently skipped.
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

/// Snap every pin/graphic named by `sel` onto the nearest multiple of
/// `step_mm`, in place. Mirrors the footprint editor's
/// `ActiveBarAlignSelectionToGrid` (#426): each element's own anchor
/// point(s) land on the grid independently — a `Rectangle`/`Line`
/// snaps `from` and `to` separately (so a shape already square with
/// the grid on one corner doesn't get skewed to keep it), `Circle`/
/// `Arc` snap `center` only (radius untouched), `Text` snaps
/// `position`, and `Polygon` snaps every vertex. Returns `true` when
/// at least one pin or graphic was touched, so a caller can gate an
/// undo snapshot on an actual change — see [`super::selected_is_alignable`]
/// for the selection-kind precheck most callers should run first.
pub fn align_selected_to_grid(
    sym: &mut Symbol,
    sel: &Option<SymbolSelection>,
    step_mm: f64,
) -> bool {
    let step = step_mm.max(1e-6);
    match sel {
        Some(SymbolSelection::Pin(idx)) => sym
            .pins
            .get_mut(*idx)
            .map(|pin| snap_pin_to_grid(pin, step))
            .is_some(),
        Some(SymbolSelection::Graphic(idx)) => sym
            .graphics
            .get_mut(*idx)
            .map(|g| snap_graphic_to_grid(&mut g.kind, step))
            .is_some(),
        Some(SymbolSelection::Multiple {
            pin_indices,
            graphic_indices,
        }) => {
            let mut changed = false;
            for &i in pin_indices {
                if let Some(pin) = sym.pins.get_mut(i) {
                    snap_pin_to_grid(pin, step);
                    changed = true;
                }
            }
            for &i in graphic_indices {
                if let Some(g) = sym.graphics.get_mut(i) {
                    snap_graphic_to_grid(&mut g.kind, step);
                    changed = true;
                }
            }
            changed
        }
        Some(SymbolSelection::All) => {
            for pin in sym.pins.iter_mut() {
                snap_pin_to_grid(pin, step);
            }
            for g in sym.graphics.iter_mut() {
                snap_graphic_to_grid(&mut g.kind, step);
            }
            !sym.pins.is_empty() || !sym.graphics.is_empty()
        }
        Some(SymbolSelection::Field(_)) | None => false,
    }
}

fn snap_pin_to_grid(pin: &mut SymbolPin, step: f64) {
    pin.position[0] = snap_value_to_grid(pin.position[0], step);
    pin.position[1] = snap_value_to_grid(pin.position[1], step);
}

fn snap_graphic_to_grid(kind: &mut SymbolGraphicKind, step: f64) {
    match kind {
        SymbolGraphicKind::Rectangle { from, to } | SymbolGraphicKind::Line { from, to } => {
            from[0] = snap_value_to_grid(from[0], step);
            from[1] = snap_value_to_grid(from[1], step);
            to[0] = snap_value_to_grid(to[0], step);
            to[1] = snap_value_to_grid(to[1], step);
        }
        SymbolGraphicKind::Circle { center, .. } | SymbolGraphicKind::Arc { center, .. } => {
            center[0] = snap_value_to_grid(center[0], step);
            center[1] = snap_value_to_grid(center[1], step);
        }
        SymbolGraphicKind::Text { position, .. } => {
            position[0] = snap_value_to_grid(position[0], step);
            position[1] = snap_value_to_grid(position[1], step);
        }
        SymbolGraphicKind::Polygon { vertices } => {
            for v in vertices.iter_mut() {
                v[0] = snap_value_to_grid(v[0], step);
                v[1] = snap_value_to_grid(v[1], step);
            }
        }
    }
}

fn snap_value_to_grid(v: f64, step: f64) -> f64 {
    (v / step).round() * step
}

/// Perform a rubber-band box selection against all symbol primitives.
///
/// The selection kind (`Window` / `Crossing`) is determined by the
/// caller from the drag direction before calling this function.
/// Returns `None` when nothing falls inside the box.
pub fn select_in_box(
    sym: &Symbol,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    kind: BoxSelectKind,
    active_part: u8,
) -> Option<SymbolSelection> {
    let xmin = x0.min(x1);
    let xmax = x0.max(x1);
    let ymin = y0.min(y1);
    let ymax = y0.max(y1);

    let mut pin_indices = Vec::new();
    for (i, pin) in sym.pins.iter().enumerate() {
        if !super::pin_on_part(pin, active_part) {
            continue;
        }
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
        if !super::graphic_on_part(g, active_part) {
            continue;
        }
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
    // Compare against the counts of items VISIBLE on the active unit,
    // not the whole-symbol totals — otherwise a box enclosing every
    // visible item never resolves to `All` on a multi-unit symbol,
    // silently downgrading to `Multiple` (whose Delete is not the
    // documented accidental-wipe no-op).
    let visible_pins = sym
        .pins
        .iter()
        .filter(|p| super::pin_on_part(p, active_part))
        .count();
    let visible_graphics = sym
        .graphics
        .iter()
        .filter(|g| super::graphic_on_part(g, active_part))
        .count();
    if pin_indices.len() == visible_pins && graphic_indices.len() == visible_graphics {
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
        SymbolGraphicKind::Polygon { vertices } => match polygon_bbox(vertices) {
            Some((gx0, gy0, gx1, gy1)) => gx0 >= xmin && gx1 <= xmax && gy0 >= ymin && gy1 <= ymax,
            None => false,
        },
    }
}

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
        SymbolGraphicKind::Polygon { vertices } => match polygon_bbox(vertices) {
            Some((gx0, gy0, gx1, gy1)) => gx0 <= xmax && gx1 >= xmin && gy0 <= ymax && gy1 >= ymin,
            None => false,
        },
    }
}

/// Axis-aligned bounding box over a polygon's vertices. `None` for an
/// empty vertex list (degenerate — box-select treats it as a miss).
fn polygon_bbox(vertices: &[[f64; 2]]) -> Option<(f64, f64, f64, f64)> {
    let mut bounds: Option<(f64, f64, f64, f64)> = None;
    for v in vertices {
        bounds = Some(match bounds {
            Some((x0, y0, x1, y1)) => (x0.min(v[0]), y0.min(v[1]), x1.max(v[0]), y1.max(v[1])),
            None => (v[0], v[1], v[0], v[1]),
        });
    }
    bounds
}

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
