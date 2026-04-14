use std::path::PathBuf;

use super::DrawMode;

/// Find the KiCad symbol library directory.
pub(super) fn find_kicad_symbols_dir() -> Option<PathBuf> {
    for ver in &["9.0", "8.0", "7.0"] {
        let p = PathBuf::from(format!("C:/Program Files/KiCad/{ver}/share/kicad/symbols"));
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// List .kicad_sym filenames in a directory.
pub(super) fn list_kicad_libraries(dir: &std::path::Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .ok()
        .map(|entries| {
            let mut names: Vec<String> = entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "kicad_sym"))
                .map(|e| {
                    e.path()
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string()
                })
                .collect();
            names.sort();
            names
        })
        .unwrap_or_default()
}

/// Given a start and end point, produce wire segments constrained by the draw mode.
/// - Ortho90: horizontal then vertical (two segments forming a 90-degree corner)
/// - Angle45: snap to nearest 45-degree angle (may produce one or two segments)
/// - FreeAngle: single straight segment
pub(super) fn constrain_segments(
    start: signex_types::schematic::Point,
    end: signex_types::schematic::Point,
    mode: DrawMode,
) -> Vec<(
    signex_types::schematic::Point,
    signex_types::schematic::Point,
)> {
    use signex_types::schematic::Point;

    let dx = end.x - start.x;
    let dy = end.y - start.y;

    if dx.abs() < 0.01 && dy.abs() < 0.01 {
        return vec![];
    }

    match mode {
        DrawMode::FreeAngle => {
            vec![(start, end)]
        }
        DrawMode::Ortho90 => {
            // Horizontal first, then vertical (like Altium default)
            if dx.abs() < 0.01 {
                // Pure vertical
                vec![(start, end)]
            } else if dy.abs() < 0.01 {
                // Pure horizontal
                vec![(start, end)]
            } else {
                let corner = Point::new(end.x, start.y);
                vec![(start, corner), (corner, end)]
            }
        }
        DrawMode::Angle45 => {
            // Snap to nearest 45-degree increment
            let adx = dx.abs();
            let ady = dy.abs();
            if adx < 0.01 || ady < 0.01 {
                // Already axis-aligned
                vec![(start, end)]
            } else if (adx - ady).abs() < adx * 0.4 {
                // Close to 45-degree: make it exactly 45-degree
                let d = adx.min(ady);
                let sx = if dx > 0.0 { 1.0 } else { -1.0 };
                let sy = if dy > 0.0 { 1.0 } else { -1.0 };
                let diag_end = Point::new(start.x + d * sx, start.y + d * sy);
                if (adx - ady).abs() < 0.01 {
                    // Exactly 45-degree
                    vec![(start, diag_end)]
                } else if adx > ady {
                    // Diagonal then horizontal
                    vec![(start, diag_end), (diag_end, Point::new(end.x, diag_end.y))]
                } else {
                    // Diagonal then vertical
                    vec![(start, diag_end), (diag_end, Point::new(diag_end.x, end.y))]
                }
            } else {
                // Mostly axis-aligned: use ortho
                let corner = Point::new(end.x, start.y);
                vec![(start, corner), (corner, end)]
            }
        }
    }
}

/// Check whether point `p` lies strictly on the interior of wire segment `wire`.
/// "Strictly interior" excludes the start/end endpoints (within `tol` mm).
fn point_on_wire_interior(
    p: signex_types::schematic::Point,
    wire: &signex_types::schematic::Wire,
    tol: f64,
) -> bool {
    let (ax, ay) = (wire.start.x, wire.start.y);
    let (bx, by) = (wire.end.x, wire.end.y);
    let (px, py) = (p.x, p.y);
    let (abx, aby) = (bx - ax, by - ay);
    let (apx, apy) = (px - ax, py - ay);
    let len_sq = abx * abx + aby * aby;
    if len_sq < tol * tol {
        return false; // degenerate (zero-length) wire
    }
    // Must be collinear: |AB x AP|^2 / |AB|^2 < tol^2
    let cross = abx * apy - aby * apx;
    if (cross * cross) > tol * tol * len_sq {
        return false;
    }
    // Parameter t = AP . AB / |AB|^2. Interior means t in (tol/len, 1 - tol/len)
    let t = (apx * abx + apy * aby) / len_sq;
    let margin = tol / len_sq.sqrt();
    t > margin && t < 1.0 - margin
}

/// Collect junctions needed at the given point `pt` in the existing sheet.
/// Returns a new `Junction` if:
///   - `pt` lies strictly on the interior of any existing wire segment, OR
///   - 3 or more wire endpoints (start/end) coincide at `pt`
/// Returns `None` if no junction is needed or one already exists.
pub(super) fn needed_junction(
    pt: signex_types::schematic::Point,
    sheet: &signex_types::schematic::SchematicSheet,
    tol: f64,
) -> Option<signex_types::schematic::Junction> {
    // Already has a junction here?
    let already = sheet.junctions.iter().any(|j| {
        (j.position.x - pt.x).abs() < tol && (j.position.y - pt.y).abs() < tol
    });
    if already {
        return None;
    }
    // T-junction: pt lies on the interior of an existing wire
    let on_interior = sheet
        .wires
        .iter()
        .any(|w| point_on_wire_interior(pt, w, tol));
    if on_interior {
        return Some(signex_types::schematic::Junction {
            uuid: uuid::Uuid::new_v4(),
            position: pt,
            diameter: 0.0,
        });
    }
    // Y-junction: 3+ wire endpoints share this point
    let endpoint_count = sheet.wires.iter().filter(|w| {
        let at_s = (w.start.x - pt.x).abs() < tol && (w.start.y - pt.y).abs() < tol;
        let at_e = (w.end.x - pt.x).abs() < tol && (w.end.y - pt.y).abs() < tol;
        at_s || at_e
    }).count();
    if endpoint_count >= 3 {
        return Some(signex_types::schematic::Junction {
            uuid: uuid::Uuid::new_v4(),
            position: pt,
            diameter: 0.0,
        });
    }
    None
}
