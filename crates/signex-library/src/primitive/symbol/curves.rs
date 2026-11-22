//! Flattened outlines of the curve kinds (`Bezier`, `Pie`, #2838). The
//! canvas, hit test, bounds and the schematic lowering all read these, so
//! they agree on the shape.

use signex_types::schematic::{Point, SchBezier};
use uuid::Uuid;

use super::SymbolGraphicKind;

/// Segments a pie's arc edge is flattened to.
const PIE_ARC_SEGMENTS: usize = 32;

impl SymbolGraphicKind {
    /// Whether this kind needs the `snxsym/v4` token.
    pub(super) fn is_curve_kind(&self) -> bool {
        matches!(self, Self::Bezier { .. } | Self::Pie { .. })
    }

    /// The outline of a curve kind in symbol mm: a Bezier's open polyline,
    /// or a pie's ring (center first, closed implicitly). Empty for every
    /// other kind.
    pub fn flattened_outline(&self) -> Vec<[f64; 2]> {
        match self {
            Self::Bezier { points } => bezier_outline(points),
            Self::Pie {
                center,
                radius,
                start_deg,
                end_deg,
            } => pie_outline(*center, *radius, *start_deg, *end_deg),
            Self::Line { .. }
            | Self::Rectangle { .. }
            | Self::Circle { .. }
            | Self::Arc { .. }
            | Self::Text { .. }
            | Self::Polygon { .. } => Vec::new(),
        }
    }
}

/// Flattened with the schematic Bezier's own sampling.
fn bezier_outline(points: &[[f64; 2]; 4]) -> Vec<[f64; 2]> {
    let [start, c1, c2, end] = points.map(|p| Point::new(p[0], p[1]));
    SchBezier::new(Uuid::nil(), start, c1, c2, end)
        .flatten()
        .into_iter()
        .map(|p| [p.x, p.y])
        .collect()
}

/// `center`, then the arc from `start_deg` counter-clockwise to `end_deg`.
fn pie_outline(center: [f64; 2], radius: f64, start_deg: f64, end_deg: f64) -> Vec<[f64; 2]> {
    let sweep = (end_deg - start_deg).rem_euclid(360.0);
    let mut ring = Vec::with_capacity(PIE_ARC_SEGMENTS + 2);
    ring.push(center);
    ring.extend((0..=PIE_ARC_SEGMENTS).map(|i| {
        let rad = (start_deg + sweep * i as f64 / PIE_ARC_SEGMENTS as f64).to_radians();
        [
            center[0] + radius * rad.cos(),
            center[1] + radius * rad.sin(),
        ]
    }));
    ring
}
