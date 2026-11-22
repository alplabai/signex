//! Flattened outlines of the curve kinds (`Bezier`, `Pie`, `EllipticalArc`,
//! `RoundRectangle`, #2838). The canvas, hit test, bounds and the schematic
//! lowering all read these, so they agree on the shape.

use std::f64::consts::{FRAC_PI_2, TAU};

use signex_types::schematic::{Point, SchBezier};
use uuid::Uuid;

use super::SymbolGraphicKind;

/// Segments a pie's arc edge is flattened to.
const PIE_ARC_SEGMENTS: usize = 32;
/// Segments an elliptical arc is flattened to.
const ELLIPSE_ARC_SEGMENTS: usize = 48;
/// Segments each round-rectangle corner is flattened to.
const CORNER_SEGMENTS: usize = 8;
/// Two outline points this close are one point.
const SAME_POINT_MM: f64 = 1e-9;

impl SymbolGraphicKind {
    /// Corner radius a new Round Rectangle takes (DECISIONS 2026-11-22).
    pub const DEFAULT_CORNER_RADIUS_MM: f64 = 1.27;

    /// Whether this kind needs the `snxsym/v4` token.
    pub(super) fn is_curve_kind(&self) -> bool {
        matches!(
            self,
            Self::Bezier { .. }
                | Self::Pie { .. }
                | Self::EllipticalArc { .. }
                | Self::RoundRectangle { .. }
        )
    }

    /// A new Round Rectangle on the corners `a` and `b`: `from` is the
    /// minimum corner, and the default radius is clamped to the shape.
    pub fn round_rectangle(a: [f64; 2], b: [f64; 2]) -> Self {
        let from = [a[0].min(b[0]), a[1].min(b[1])];
        let to = [a[0].max(b[0]), a[1].max(b[1])];
        Self::RoundRectangle {
            from,
            to,
            corner_radius: Self::DEFAULT_CORNER_RADIUS_MM.min(Self::max_corner_radius(from, to)),
        }
    }

    /// The largest corner radius a rectangle takes: half its short side.
    pub fn max_corner_radius(from: [f64; 2], to: [f64; 2]) -> f64 {
        (to[0] - from[0]).abs().min((to[1] - from[1]).abs()) * 0.5
    }

    /// Where the ray at the geometric angle `deg` from `center` meets the
    /// ellipse.
    pub fn point_on_ellipse(center: [f64; 2], radius_x: f64, radius_y: f64, deg: f64) -> [f64; 2] {
        let t = ellipse_parameter(radius_x, radius_y, deg);
        [
            center[0] + radius_x * t.cos(),
            center[1] + radius_y * t.sin(),
        ]
    }

    /// The outline of a curve kind in symbol mm: an open polyline for a
    /// Bezier or elliptical arc, an implicitly closed ring for a pie
    /// (center first) or round rectangle. Empty for every other kind.
    pub fn flattened_outline(&self) -> Vec<[f64; 2]> {
        match self {
            Self::Bezier { points } => bezier_outline(points),
            Self::Pie {
                center,
                radius,
                start_deg,
                end_deg,
            } => pie_outline(*center, *radius, *start_deg, *end_deg),
            Self::EllipticalArc {
                center,
                radius_x,
                radius_y,
                start_deg,
                end_deg,
            } => elliptical_outline(*center, *radius_x, *radius_y, *start_deg, *end_deg),
            Self::RoundRectangle {
                from,
                to,
                corner_radius,
            } => round_rectangle_outline(*from, *to, *corner_radius),
            Self::Line { .. }
            | Self::Rectangle { .. }
            | Self::Circle { .. }
            | Self::Arc { .. }
            | Self::Text { .. }
            | Self::Polygon { .. } => Vec::new(),
        }
    }
}

/// The ellipse parameter `t` (point `(rx cos t, ry sin t)`) of the geometric
/// angle `deg`, so sampling in `t` stays even on a narrow ellipse.
fn ellipse_parameter(radius_x: f64, radius_y: f64, deg: f64) -> f64 {
    let (sin, cos) = deg.to_radians().sin_cos();
    (radius_x * sin).atan2(radius_y * cos)
}

/// The arc counter-clockwise from the geometric `start_deg` to `end_deg`.
fn elliptical_outline(
    center: [f64; 2],
    radius_x: f64,
    radius_y: f64,
    start_deg: f64,
    end_deg: f64,
) -> Vec<[f64; 2]> {
    let t0 = ellipse_parameter(radius_x, radius_y, start_deg);
    let t1 = ellipse_parameter(radius_x, radius_y, end_deg);
    let sweep = (t1 - t0).rem_euclid(TAU);
    (0..=ELLIPSE_ARC_SEGMENTS)
        .map(|i| {
            let t = t0 + sweep * i as f64 / ELLIPSE_ARC_SEGMENTS as f64;
            [
                center[0] + radius_x * t.cos(),
                center[1] + radius_y * t.sin(),
            ]
        })
        .collect()
}

/// The ring counter-clockwise from the top-right corner's arc. A radius past
/// half the short side (a later resize) draws clamped. Repeated points, as
/// where two corners meet at the full radius, are dropped.
fn round_rectangle_outline(from: [f64; 2], to: [f64; 2], radius: f64) -> Vec<[f64; 2]> {
    let r = radius.clamp(0.0, SymbolGraphicKind::max_corner_radius(from, to));
    let (x0, x1) = (from[0].min(to[0]), from[0].max(to[0]));
    let (y0, y1) = (from[1].min(to[1]), from[1].max(to[1]));
    let corners = [
        (x1 - r, y1 - r),
        (x0 + r, y1 - r),
        (x0 + r, y0 + r),
        (x1 - r, y0 + r),
    ];
    let mut ring: Vec<[f64; 2]> = Vec::with_capacity(4 * (CORNER_SEGMENTS + 1));
    for (quarter, (cx, cy)) in corners.into_iter().enumerate() {
        for i in 0..=CORNER_SEGMENTS {
            let a = FRAC_PI_2 * (quarter as f64 + i as f64 / CORNER_SEGMENTS as f64);
            let p = [cx + r * a.cos(), cy + r * a.sin()];
            if ring.last().is_none_or(|&q| !same_point(p, q)) {
                ring.push(p);
            }
        }
    }
    if ring.len() > 1 && same_point(ring[0], ring[ring.len() - 1]) {
        ring.pop();
    }
    ring
}

fn same_point(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).abs() <= SAME_POINT_MM && (a[1] - b[1]).abs() <= SAME_POINT_MM
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
