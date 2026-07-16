//! Delete + hit-testing + graphic-handle geometry for the symbol editor.

use super::*;

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
        Some(SymbolSelection::Multiple {
            pin_indices,
            graphic_indices,
        }) => {
            // Delete in reverse index order so removing an item doesn't
            // invalidate the indices of the remaining ones.
            let mut pins_desc = pin_indices.clone();
            pins_desc.sort_unstable_by(|a, b| b.cmp(a));
            for idx in pins_desc {
                if idx < sym.pins.len() {
                    sym.pins.remove(idx);
                }
            }
            let mut gfx_desc = graphic_indices.clone();
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
pub fn hit_test(sym: &Symbol, x: f64, y: f64, active_part: u8) -> Option<SymbolSelection> {
    const PIN_HIT_R_SQ: f64 = 1.5 * 1.5;
    for (i, pin) in sym.pins.iter().enumerate() {
        if !super::pin_on_part(pin, active_part) {
            continue;
        }
        let dx = pin.position[0] - x;
        let dy = pin.position[1] - y;
        if dx * dx + dy * dy <= PIN_HIT_R_SQ {
            return Some(SymbolSelection::Pin(i));
        }
    }
    for idx in (0..sym.graphics.len()).rev() {
        let Some(g) = sym.graphics.get(idx) else {
            continue;
        };
        if !super::graphic_on_part(g, active_part) {
            continue;
        }
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
        SymbolGraphicKind::Polygon { vertices } => {
            if vertices.len() < 2 {
                false
            } else if g.fill.is_some() {
                point_in_polygon([x, y], vertices)
            } else {
                polygon_outline_hit(x, y, vertices, GRAPHIC_BODY_TOL)
            }
        }
    }
}

fn point_to_segment_dist_sq(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    signex_sketch::geom::point_to_segment_distance_sq(p, a, b)
}

/// Even-odd point-in-polygon test (implicitly-closed vertex ring) — a
/// thin adapter over `signex_sketch::geom::point_in_polygon`. Mirrors
/// the footprint canvas's own adapter of the same shared helper.
fn point_in_polygon(p: [f64; 2], vertices: &[[f64; 2]]) -> bool {
    let polygon: Vec<signex_sketch::geom::Point2> = vertices.iter().map(|&v| v.into()).collect();
    signex_sketch::geom::point_in_polygon(p, &polygon)
}

/// `true` when `(x, y)` lies within `tol` of any closed-polygon edge
/// (including the implicit last-to-first segment).
fn polygon_outline_hit(x: f64, y: f64, vertices: &[[f64; 2]], tol: f64) -> bool {
    let n = vertices.len();
    for i in 0..n {
        let j = (i + 1) % n;
        if point_to_segment_dist_sq([x, y], vertices[i], vertices[j]) <= tol * tol {
            return true;
        }
    }
    false
}

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
        (SymbolGraphicKind::Rectangle { from, to }, GraphicHandle::RectEdge(e)) => match e {
            0 => [(from[0] + to[0]) * 0.5, to[1]],   // Top midpoint
            1 => [to[0], (from[1] + to[1]) * 0.5],   // Right midpoint
            2 => [(from[0] + to[0]) * 0.5, from[1]], // Bottom midpoint
            3 => [from[0], (from[1] + to[1]) * 0.5], // Left midpoint
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
        (SymbolGraphicKind::Polygon { vertices }, GraphicHandle::PolygonVertex(i)) => {
            *vertices.get(i as usize)?
        }
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
            // Four corners.
            (GraphicHandle::RectCorner(0), [from[0], to[1]]),
            (GraphicHandle::RectCorner(1), [to[0], to[1]]),
            (GraphicHandle::RectCorner(2), [to[0], from[1]]),
            (GraphicHandle::RectCorner(3), [from[0], from[1]]),
            // Four edge midpoints.
            (GraphicHandle::RectEdge(0), [(from[0] + to[0]) * 0.5, to[1]]),
            (GraphicHandle::RectEdge(1), [to[0], (from[1] + to[1]) * 0.5]),
            (
                GraphicHandle::RectEdge(2),
                [(from[0] + to[0]) * 0.5, from[1]],
            ),
            (
                GraphicHandle::RectEdge(3),
                [from[0], (from[1] + to[1]) * 0.5],
            ),
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
        SymbolGraphicKind::Polygon { vertices } => vertices
            .iter()
            .enumerate()
            .map(|(i, v)| (GraphicHandle::PolygonVertex(i as u16), *v))
            .collect(),
    }
}

/// Hit-test world coordinates against every placed graphic's resize
/// handles. Returns `(graphic_idx, handle)` for the first hit, scanning
/// graphics in reverse so the most-recently-placed graphic wins when
/// handles overlap.
///
/// `tol_mm` is the world-space hit-test radius in millimetres. The
/// caller should derive it from screen pixels so the hit area stays
/// consistent at all zoom levels, e.g.:
/// ```text
/// let tol_mm = (8.0_f32 / camera.scale.max(0.01)).clamp(0.5, 4.0) as f64;
/// ```
pub fn hit_test_graphic_handle(
    sym: &Symbol,
    x: f64,
    y: f64,
    tol_mm: f64,
    active_part: u8,
) -> Option<(usize, GraphicHandle)> {
    let r_sq = tol_mm * tol_mm;
    for idx in (0..sym.graphics.len()).rev() {
        let Some(g) = sym.graphics.get(idx) else {
            continue;
        };
        if !super::graphic_on_part(g, active_part) {
            continue;
        }
        for (handle, pos) in graphic_handles(sym, idx) {
            let dx = pos[0] - x;
            let dy = pos[1] - y;
            if dx * dx + dy * dy <= r_sq {
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
        // Edge midpoint drag — only the constrained axis is updated so
        // the handle slides along its edge without skewing the rectangle.
        (SymbolGraphicKind::Rectangle { to, .. }, GraphicHandle::RectEdge(0)) => {
            to[1] = y; // Top edge: move top y
        }
        (SymbolGraphicKind::Rectangle { to, .. }, GraphicHandle::RectEdge(1)) => {
            to[0] = x; // Right edge: move right x
        }
        (SymbolGraphicKind::Rectangle { from, .. }, GraphicHandle::RectEdge(2)) => {
            from[1] = y; // Bottom edge: move bottom y
        }
        (SymbolGraphicKind::Rectangle { from, .. }, GraphicHandle::RectEdge(3)) => {
            from[0] = x; // Left edge: move left x
        }
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
        (SymbolGraphicKind::Polygon { vertices }, GraphicHandle::PolygonVertex(i)) => {
            if let Some(v) = vertices.get_mut(i as usize) {
                v[0] = x;
                v[1] = y;
            }
        }
        _ => {}
    }
}
