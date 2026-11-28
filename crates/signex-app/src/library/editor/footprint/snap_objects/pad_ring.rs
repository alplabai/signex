//! A pad's copper outline in world mm, for Pad Vertices and Pad Edges
//! (#2726): the shape the canvas draws, about the pad centre, turned by the
//! pad's rotation. Curved corners are walked in short chords.

use signex_library::PadShape;

use super::super::state::EditorPad;

/// Chords per quarter turn of a round edge, as on a board.
const EDGE_STEPS: usize = 8;

/// The corners of a polygonal pad (Rect, Chamfered, Trapezoid, Custom);
/// empty for a round outline, which has none.
pub(super) fn vertices(pad: &EditorPad) -> Vec<(f64, f64)> {
    match pad.shape {
        PadShape::Rect
        | PadShape::Chamfered { .. }
        | PadShape::Trapezoid { .. }
        | PadShape::Custom(_) => ring(pad),
        PadShape::Round | PadShape::Oval | PadShape::RoundRect { .. } => Vec::new(),
    }
}

/// The closed outline of `pad`.
pub(super) fn ring(pad: &EditorPad) -> Vec<(f64, f64)> {
    let (hw, hh) = (pad.size_mm.0 / 2.0, pad.size_mm.1 / 2.0);
    let local = match &pad.shape {
        PadShape::Rect => rect(hw, hh),
        PadShape::Round | PadShape::Oval => ellipse(hw, hh),
        PadShape::RoundRect { radius_ratio } => {
            rounded(hw, hh, hw.min(hh) * (radius_ratio * 2.0).clamp(0.0, 1.0))
        }
        PadShape::Chamfered {
            chamfer_ratio,
            corners,
        } => {
            let c = hw.min(hh) * (chamfer_ratio * 2.0).clamp(0.0, 1.0);
            // Corners in y-down order: top-left, top-right, bottom-right,
            // bottom-left, as the canvas draws them.
            let cut = [
                corners.top_left,
                corners.top_right,
                corners.bottom_right,
                corners.bottom_left,
            ];
            chamfered(hw, hh, c, cut)
        }
        PadShape::Trapezoid { delta } => {
            use signex_types::pcb::Point;
            match signex_types::pcb_pad_shape::trapezoid_corners(
                Point::new(pad.size_mm.0, pad.size_mm.1),
                Point::new(delta[0], delta[1]),
            ) {
                Ok(corners) => corners.iter().map(|c| (c.x, c.y)).collect(),
                Err(_) => Vec::new(),
            }
        }
        PadShape::Custom(polygon) => polygon.points.iter().map(|p| (p[0], p[1])).collect(),
    };
    let (cx, cy) = pad.position_mm;
    local
        .into_iter()
        .map(|(x, y)| {
            let (dx, dy) = pad.rotate_delta_to_world_mm(x, y);
            (cx + dx, cy + dy)
        })
        .collect()
}

fn rect(hw: f64, hh: f64) -> Vec<(f64, f64)> {
    vec![(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)]
}

fn ellipse(hw: f64, hh: f64) -> Vec<(f64, f64)> {
    let steps = EDGE_STEPS * 4;
    (0..steps)
        .map(|i| {
            let t = i as f64 / steps as f64 * std::f64::consts::TAU;
            (hw * t.cos(), hh * t.sin())
        })
        .collect()
}

/// A rectangle whose corners are quarter circles of radius `r`.
fn rounded(hw: f64, hh: f64, r: f64) -> Vec<(f64, f64)> {
    if r <= 1e-9 {
        return rect(hw, hh);
    }
    let centres = [
        (hw - r, hh - r),
        (-(hw - r), hh - r),
        (-(hw - r), -(hh - r)),
        (hw - r, -(hh - r)),
    ];
    let mut out = Vec::with_capacity(4 * (EDGE_STEPS + 1));
    for (quarter, (cx, cy)) in centres.into_iter().enumerate() {
        for step in 0..=EDGE_STEPS {
            let t =
                (quarter as f64 + step as f64 / EDGE_STEPS as f64) * std::f64::consts::FRAC_PI_2;
            out.push((cx + r * t.cos(), cy + r * t.sin()));
        }
    }
    out
}

/// A rectangle with the corners flagged in `cut` cut back by `c`.
fn chamfered(hw: f64, hh: f64, c: f64, cut: [bool; 4]) -> Vec<(f64, f64)> {
    let corners = [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)];
    let mut out = Vec::with_capacity(8);
    for (i, &(x, y)) in corners.iter().enumerate() {
        if !cut[i] || c <= 1e-9 {
            out.push((x, y));
            continue;
        }
        let (prev, next) = (corners[(i + 3) % 4], corners[(i + 1) % 4]);
        out.push(toward((x, y), prev, c));
        out.push(toward((x, y), next, c));
    }
    out
}

/// The point `d` from `from` toward `to`.
fn toward(from: (f64, f64), to: (f64, f64), d: f64) -> (f64, f64) {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let len = dx.hypot(dy);
    if len <= 1e-12 {
        return from;
    }
    (from.0 + dx / len * d, from.1 + dy / len * d)
}
