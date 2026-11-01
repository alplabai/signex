//! Field autoplace algorithm (reference/value text placement).

use super::wire_index::WireIndex;
use signex_types::schematic::shown_on;

// ---------------------------------------------------------------------------
// Field autoplace (v0.12 cleanroom rewrite — Wave 2.5)
//
// Reposition the visible Reference and Value fields onto the side of
// the symbol body with the fewest visible pins. Tie-break:
//
//     Bottom > Top > Left > Right
//
// Hardware-engineering convention places reference / value text below
// the body for readability; top is the second-best free side; horizontal
// sides last. Deliberately disjoint from any third-party tool's order.
//
// The autoplacer respects two invariants:
//
// 1. `Symbol::fields_user_placed == true` ⇒ skip entirely. Once the
//    user has manually placed a field, rotate / mirror never silently
//    overwrites that placement.
// 2. Anchor-aware bias: candidate sides whose anchor would land within
//    `ANCHOR_AVOID_RADIUS_MM` of an existing wire endpoint or label
//    receive a small score penalty so the autoplacer prefers cleaner
//    sides where the field text won't visually crash into other
//    document elements (Q9 (c) Aggressive-tier improvement).
// ---------------------------------------------------------------------------

const ANCHOR_AVOID_RADIUS_MM: f64 = 1.27;
const ANCHOR_PENALTY: u32 = 1;

/// Place the reference / value fields of `symbol`, a part about to be placed
/// from `lib`, clear of its body, as a turn or mirror of it would. The fields
/// of a part dropped on the sheet start where the autoplacer would put them,
/// not at a fixed offset from the origin that lands inside a body taller than
/// the offset.
pub fn place_fields_for_new_symbol(
    symbol: &mut signex_types::schematic::Symbol,
    lib: &signex_types::schematic::LibSymbol,
) {
    autoplace_fields(
        symbol,
        lib,
        &signex_types::schematic::SchematicSheet::default(),
    );
}

/// Pick a free side for `symbol`'s reference / value fields and write
/// new positions / justifies / rotation into the field `TextProp`s.
pub(super) fn autoplace_fields(
    symbol: &mut signex_types::schematic::Symbol,
    lib: &signex_types::schematic::LibSymbol,
    document: &signex_types::schematic::SchematicSheet,
) {
    use signex_types::schematic::{HAlign, VAlign};

    if symbol.fields_user_placed {
        return;
    }

    // 1. Body bbox in world space (graphics only).
    let mut body_bbox: Option<(f64, f64, f64, f64)> = None;
    let extend = |bbox: &mut Option<(f64, f64, f64, f64)>, x: f64, y: f64| match bbox {
        None => *bbox = Some((x, y, x, y)),
        Some((lx, ly, hx, hy)) => {
            if x < *lx {
                *lx = x;
            }
            if y < *ly {
                *ly = y;
            }
            if x > *hx {
                *hx = x;
            }
            if y > *hy {
                *hy = y;
            }
        }
    };
    for g in &lib.graphics {
        if !shown_on(lib, g.unit, g.body_style, symbol) {
            continue;
        }
        for (lx, ly) in graphic_extent_points(&g.graphic) {
            let (wx, wy) = transform_local_point(symbol, lx, ly);
            extend(&mut body_bbox, wx, wy);
        }
    }
    let body = body_bbox.unwrap_or((
        symbol.position.x - 1.27,
        symbol.position.y - 1.27,
        symbol.position.x + 1.27,
        symbol.position.y + 1.27,
    ));
    let (body_min_x, body_min_y, body_max_x, body_max_y) = body;
    let body_cx = (body_min_x + body_max_x) * 0.5;
    let body_cy = (body_min_y + body_max_y) * 0.5;

    // 2. Outer bbox = body + visible pin endpoints.
    let mut outer_bbox = body_bbox;
    for p in &lib.pins {
        if !shown_on(lib, p.unit, p.body_style, symbol) {
            continue;
        }
        if !p.pin.visible {
            continue;
        }
        let rad = p.pin.rotation.to_radians();
        let (sx, sy) = (p.pin.position.x, p.pin.position.y);
        let (ex, ey) = (sx + p.pin.length * rad.cos(), sy + p.pin.length * rad.sin());
        for (lx, ly) in [(sx, sy), (ex, ey)] {
            let (wx, wy) = transform_local_point(symbol, lx, ly);
            extend(&mut outer_bbox, wx, wy);
        }
    }
    let (min_x, min_y, max_x, max_y) = outer_bbox.unwrap_or(body);
    let cx = (min_x + max_x) * 0.5;
    let cy = (min_y + max_y) * 0.5;

    // 3. Count pins per side relative to the body centre.
    let (mut pins_right, mut pins_left, mut pins_top, mut pins_bottom) = (0u32, 0u32, 0u32, 0u32);
    for p in &lib.pins {
        if !shown_on(lib, p.unit, p.body_style, symbol) {
            continue;
        }
        let (wx, wy) = transform_local_point(symbol, p.pin.position.x, p.pin.position.y);
        let dx = wx - body_cx;
        let dy = wy - body_cy;
        if dx.abs() >= dy.abs() {
            if dx >= 0.0 {
                pins_right += 1;
            } else {
                pins_left += 1;
            }
        } else if dy >= 0.0 {
            pins_bottom += 1;
        } else {
            pins_top += 1;
        }
    }

    // 4. Pick the side. Score = pin_count + anchor_penalty.
    //    Tie-break order is Signex-original: Bottom > Top > Left > Right.
    #[derive(Clone, Copy)]
    enum Side {
        Bottom,
        Top,
        Left,
        Right,
    }
    let candidates_anchors = [
        (Side::Bottom, (cx, max_y + 1.27), pins_bottom),
        (Side::Top, (cx, min_y - 1.27), pins_top),
        (Side::Left, (min_x - 1.27, cy), pins_left),
        (Side::Right, (max_x + 1.27, cy), pins_right),
    ];
    let mut scored: Vec<(Side, u32, usize)> = Vec::with_capacity(4);
    for (i, (side, (ax, ay), pin_count)) in candidates_anchors.iter().enumerate() {
        let near = anchor_obstacle_count(*ax, *ay, document);
        scored.push((*side, *pin_count + near * ANCHOR_PENALTY, i));
    }
    let chosen = scored
        .iter()
        .min_by(|a, b| a.1.cmp(&b.1).then(a.2.cmp(&b.2)))
        .map(|(s, _, _)| *s)
        .unwrap_or(Side::Bottom);

    // 5. Collect visible fields, anchor + justify per chosen side.
    let mut fields: Vec<&mut signex_types::schematic::TextProp> = Vec::new();
    if let Some(rt) = symbol.ref_text.as_mut()
        && !rt.hidden
    {
        fields.push(rt);
    }
    if let Some(vt) = symbol.val_text.as_mut()
        && !vt.hidden
    {
        fields.push(vt);
    }
    if fields.is_empty() {
        symbol.fields_autoplaced = true;
        return;
    }

    let font_size = fields[0].font_size.max(0.1);
    let line_height = font_size * 1.5;
    let margin = (font_size * 1.5).max(0.762);
    let n = fields.len() as f64;

    let (anchor_x, anchor_y_first, justify_h, justify_v): (f64, f64, HAlign, VAlign) = match chosen
    {
        Side::Bottom => (cx, max_y + margin, HAlign::Center, VAlign::Top),
        Side::Top => (
            cx,
            min_y - margin - (n - 1.0) * line_height,
            HAlign::Center,
            VAlign::Bottom,
        ),
        Side::Left => (
            min_x - margin,
            cy - (n - 1.0) * line_height * 0.5,
            HAlign::Right,
            VAlign::Center,
        ),
        Side::Right => (
            max_x + margin,
            cy - (n - 1.0) * line_height * 0.5,
            HAlign::Left,
            VAlign::Center,
        ),
    };

    // 6. Field rotation: keep horizontal whenever possible. Symbol
    //    rotated 90° / 270° gets a 90° field rotation so the rendered
    //    glyph still reads horizontally on screen.
    let sym_rot = symbol.rotation.rem_euclid(360.0).round() as i32;
    let field_rotation = if matches!(sym_rot, 90 | 270) {
        90.0
    } else {
        0.0
    };

    for (i, prop) in fields.iter_mut().enumerate() {
        prop.position.x = anchor_x;
        prop.position.y = anchor_y_first + i as f64 * line_height;
        prop.justify_h = justify_h;
        prop.justify_v = justify_v;
        prop.rotation = field_rotation;
    }

    symbol.fields_autoplaced = true;
}

/// Apply a symbol instance's position, rotation, and mirror to a
/// library-space point, returning world-space coordinates.
///
/// HI-19: thin wrapper over the shared `SymbolTransform::apply` so the
/// math lives in exactly one place (`signex-types::schematic`). Kept
/// as a free function so existing call sites that pass `(lx, ly)`
/// don't need to reshape into `Point`.
fn transform_local_point(sym: &signex_types::schematic::Symbol, lx: f64, ly: f64) -> (f64, f64) {
    let p = signex_types::schematic::SymbolTransform::from_symbol(sym)
        .apply(signex_types::schematic::Point::new(lx, ly));
    (p.x, p.y)
}

fn graphic_extent_points(g: &signex_types::schematic::Graphic) -> Vec<(f64, f64)> {
    use signex_types::schematic::Graphic;
    match g {
        Graphic::Polyline { points, .. } | Graphic::Bezier { points, .. } => {
            points.iter().map(|p| (p.x, p.y)).collect()
        }
        Graphic::Rectangle { start, end, .. } => vec![
            (start.x, start.y),
            (end.x, start.y),
            (end.x, end.y),
            (start.x, end.y),
        ],
        Graphic::Circle { center, radius, .. } => vec![
            (center.x - *radius, center.y - *radius),
            (center.x + *radius, center.y - *radius),
            (center.x - *radius, center.y + *radius),
            (center.x + *radius, center.y + *radius),
        ],
        Graphic::Arc {
            start, mid, end, ..
        } => {
            vec![(start.x, start.y), (mid.x, mid.y), (end.x, end.y)]
        }
        Graphic::Text { position, .. } | Graphic::TextBox { position, .. } => {
            vec![(position.x, position.y)]
        }
    }
}

/// Count wire endpoints + label anchors within
/// `ANCHOR_AVOID_RADIUS_MM` of `(ax, ay)`. Used as a tie-break bias so
/// candidate sides crowded with wires / labels lose to cleaner sides.
fn anchor_obstacle_count(
    ax: f64,
    ay: f64,
    document: &signex_types::schematic::SchematicSheet,
) -> u32 {
    let r2 = ANCHOR_AVOID_RADIUS_MM * ANCHOR_AVOID_RADIUS_MM;
    let close = |x: f64, y: f64| (x - ax).powi(2) + (y - ay).powi(2) < r2;
    let mut n = 0u32;
    for w in &document.wires {
        if close(w.start.x, w.start.y) || close(w.end.x, w.end.y) {
            n = n.saturating_add(1);
        }
    }
    for l in &document.labels {
        if close(l.position.x, l.position.y) {
            n = n.saturating_add(1);
        }
    }
    n
}

// ---------------------------------------------------------------------------
// Geometry helpers
// ---------------------------------------------------------------------------

fn point_on_wire_interior(
    point: signex_types::schematic::Point,
    wire: &signex_types::schematic::Wire,
    tolerance: f64,
) -> bool {
    let (ax, ay) = (wire.start.x, wire.start.y);
    let (bx, by) = (wire.end.x, wire.end.y);
    let (px, py) = (point.x, point.y);
    let (abx, aby) = (bx - ax, by - ay);
    let (apx, apy) = (px - ax, py - ay);
    let len_sq = abx * abx + aby * aby;

    if len_sq < tolerance * tolerance {
        return false;
    }

    let cross = abx * apy - aby * apx;
    if (cross * cross) > tolerance * tolerance * len_sq {
        return false;
    }

    let t = (apx * abx + apy * aby) / len_sq;
    let margin = tolerance / len_sq.sqrt();
    t > margin && t < 1.0 - margin
}

/// True when a dot at `point` would actually merge two wires — i.e. at least
/// two of the sheet's wires contain `point` **exactly** in the netlist's 1 µm
/// key space.
///
/// The geometry above works in `f64` mm with a 0.01 mm tolerance, but
/// `SheetConnectivity` honours a junction only where `point_on_segment` holds
/// with *exact* collinearity in the key space (D5.5). A candidate a few µm off
/// a wire therefore yields a dot the netlist refuses to act on: a reassuring
/// visual asserting a connection that does not exist, which is strictly worse
/// than the undotted T it was meant to fix (issue #402). Off-grid endpoints —
/// imported or legacy geometry — are exactly where the two metrics diverge, so
/// every dot this module mints is gated on the netlist's own answer rather than
/// on the float tolerance alone.
fn junction_is_honoured(point: signex_types::schematic::Point, index: &WireIndex) -> bool {
    let k = signex_net::pt_key(&point);
    index
        .segment_candidates(point)
        .into_iter()
        .filter(|&i| {
            let w = index.wire(i);
            signex_net::point_on_segment(
                k,
                signex_net::pt_key(&w.start),
                signex_net::pt_key(&w.end),
            )
        })
        .count()
        >= 2
}

fn junction_at(point: signex_types::schematic::Point) -> signex_types::schematic::Junction {
    signex_types::schematic::Junction {
        unknown: Default::default(),
        unknown_columns: Default::default(),
        uuid: uuid::Uuid::now_v7(),
        position: point,
        diameter: 0.0,
        // Every dot this module places is algorithmically derived, never
        // user-placed — see `Junction::minted` (issue #422).
        minted: true,
    }
}

/// Every junction dot the sheet needs on account of `wire` — **both**
/// directions of the T: the wire's own endpoints landing on something
/// ([`needed_junction`]) and something else's endpoint landing on this wire's
/// interior ([`junctions_under_new_wire`]).
///
/// Call with `wire` already present in `document`. Any command that creates or
/// moves wire geometry must route through here; reconciling only the placement
/// path leaves drag / rotate / mirror minting junction-less Ts (issue #402).
pub(crate) fn junctions_for_wire(
    wire: &signex_types::schematic::Wire,
    junctions: &[signex_types::schematic::Junction],
    index: &WireIndex,
) -> Vec<signex_types::schematic::Junction> {
    let mut placed: Vec<signex_types::schematic::Junction> = Vec::new();
    for point in [wire.start, wire.end] {
        if let Some(junction) = needed_junction(point, junctions, index) {
            placed.push(junction);
        }
    }
    placed.extend(junctions_under_new_wire(wire, junctions, index));
    placed
}

/// Junction dots a newly drawn wire needs because an **existing** wire's
/// endpoint lands on the new wire's interior — the mirror image of
/// [`needed_junction`], which only ever inspects the *new* wire's own two
/// endpoints.
///
/// Without this, drawing a stub and then a trunk through the stub's endpoint
/// produced a real junction-less T with no dot. The netlist deliberately treats
/// that as disconnected (issue #107), so the connection was silently lost
/// (issue #402). `document` may already contain the new wire; a wire endpoint
/// can never sit on its own interior, so no self-exclusion is needed.
pub(crate) fn junctions_under_new_wire(
    wire: &signex_types::schematic::Wire,
    junctions: &[signex_types::schematic::Junction],
    index: &WireIndex,
) -> Vec<signex_types::schematic::Junction> {
    let tolerance = index.tolerance();
    let mut placed: Vec<signex_types::schematic::Junction> = Vec::new();
    for point in index
        .endpoint_candidates_near_wire(wire)
        .into_iter()
        .flat_map(|i| [index.wire(i).start, index.wire(i).end])
        .filter(|p| point_on_wire_interior(*p, wire, tolerance))
        .filter(|p| wire_meeting_justifies_junction(*p, index))
    {
        let already = junctions.iter().chain(placed.iter()).any(|j| {
            (j.position.x - point.x).abs() < tolerance && (j.position.y - point.y).abs() < tolerance
        });
        if already {
            continue;
        }
        placed.push(junction_at(point));
    }
    placed
}

/// Number of the sheet's wires that terminate (start or end) at `point`.
fn wire_endpoint_count(point: signex_types::schematic::Point, index: &WireIndex) -> usize {
    let tolerance = index.tolerance();
    index
        .endpoint_candidates(point)
        .into_iter()
        .filter(|&i| {
            let wire = index.wire(i);
            let at_start = (wire.start.x - point.x).abs() < tolerance
                && (wire.start.y - point.y).abs() < tolerance;
            let at_end = (wire.end.x - point.x).abs() < tolerance
                && (wire.end.y - point.y).abs() < tolerance;
            at_start || at_end
        })
        .count()
}

/// True when a genuine wire *meeting* at `point` justifies a junction dot —
/// a T (one wire terminates here, on the interior of another) or a star
/// (3+ wires terminate here) — as opposed to two wires' interiors merely
/// *crossing* at `point` with neither one ending there.
///
/// This is the shared gate for both minting a new dot ([`needed_junction`])
/// and re-validating one that already exists (`reconcile_wire_junctions`,
/// `transform/mod.rs`): a point no wire terminates at is never a real
/// connection, no matter how many wire interiors happen to pass through it.
/// Without the `endpoint_count == 0` guard, a stale dot at a former T can
/// look "still honoured" once an unrelated wire is later dragged to merely
/// cross the same point — silently merging two nets the user never
/// connected (issue #422).
pub(crate) fn wire_meeting_justifies_junction(
    point: signex_types::schematic::Point,
    index: &WireIndex,
) -> bool {
    let endpoint_count = wire_endpoint_count(point, index);
    if endpoint_count == 0 {
        return false;
    }

    let tolerance = index.tolerance();
    let on_wire_interior = index
        .segment_candidates(point)
        .into_iter()
        .any(|i| point_on_wire_interior(point, index.wire(i), tolerance));

    (on_wire_interior || endpoint_count >= 3) && junction_is_honoured(point, index)
}

pub(crate) fn needed_junction(
    point: signex_types::schematic::Point,
    junctions: &[signex_types::schematic::Junction],
    index: &WireIndex,
) -> Option<signex_types::schematic::Junction> {
    let tolerance = index.tolerance();
    let already_present = junctions.iter().any(|junction| {
        (junction.position.x - point.x).abs() < tolerance
            && (junction.position.y - point.y).abs() < tolerance
    });
    if already_present {
        return None;
    }

    if !wire_meeting_justifies_junction(point, index) {
        return None;
    }

    Some(junction_at(point))
}

#[cfg(test)]
mod oracle_tests;
