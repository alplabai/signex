//! Flattened outlines of the curve kinds (`Bezier`, `Pie`, `EllipticalArc`,
//! `RoundRectangle`, #2838). The canvas, hit test, bounds and the schematic
//! lowering all read these, so they agree on the shape. The geometry is the
//! one the schematic drawings use: [`CurveShape`] and [`SchBezier`] in
//! `signex_types::schematic` (#1267).

use signex_types::schematic::{self, CurveShape, Point, SchBezier};
use uuid::Uuid;

use super::SymbolGraphicKind;

fn pt(p: [f64; 2]) -> Point {
    Point::new(p[0], p[1])
}

fn xy(p: Point) -> [f64; 2] {
    [p.x, p.y]
}

impl SymbolGraphicKind {
    /// Corner radius a new Round Rectangle takes (DECISIONS 2026-11-22).
    pub const DEFAULT_CORNER_RADIUS_MM: f64 = CurveShape::DEFAULT_CORNER_RADIUS_MM;

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
        schematic::max_corner_radius(pt(from), pt(to))
    }

    /// Hold a Round Rectangle's stored radius to half its short side, after
    /// anything that resized it. Other kinds are untouched.
    pub fn clamp_corner_radius(&mut self) {
        if let Self::RoundRectangle {
            from,
            to,
            corner_radius,
        } = self
        {
            *corner_radius = corner_radius.min(Self::max_corner_radius(*from, *to));
        }
    }

    /// Where the ray at the geometric angle `deg` from `center` meets the
    /// ellipse.
    pub fn point_on_ellipse(center: [f64; 2], radius_x: f64, radius_y: f64, deg: f64) -> [f64; 2] {
        xy(schematic::point_on_ellipse(
            pt(center),
            radius_x,
            radius_y,
            deg,
        ))
    }

    /// The shared curve model of a Pie, Elliptical Arc or Round Rectangle;
    /// `None` for every other kind.
    pub fn curve_shape(&self) -> Option<CurveShape> {
        Some(match *self {
            Self::Pie {
                center,
                radius,
                start_deg,
                end_deg,
            } => CurveShape::Pie {
                center: pt(center),
                radius,
                start_deg,
                end_deg,
            },
            Self::EllipticalArc {
                center,
                radius_x,
                radius_y,
                start_deg,
                end_deg,
            } => CurveShape::EllipticalArc {
                center: pt(center),
                radius_x,
                radius_y,
                start_deg,
                end_deg,
            },
            Self::RoundRectangle {
                from,
                to,
                corner_radius,
            } => CurveShape::RoundRect {
                from: pt(from),
                to: pt(to),
                corner_radius,
            },
            Self::Line { .. }
            | Self::Rectangle { .. }
            | Self::Circle { .. }
            | Self::Arc { .. }
            | Self::Text { .. }
            | Self::Polygon { .. }
            | Self::Bezier { .. } => return None,
        })
    }

    /// The outline of a curve kind in symbol mm: an open polyline for a
    /// Bezier or elliptical arc, an implicitly closed ring for a pie
    /// (center first) or round rectangle. Empty for every other kind.
    pub fn flattened_outline(&self) -> Vec<[f64; 2]> {
        if let Self::Bezier { points } = self {
            return bezier_outline(points);
        }
        self.curve_shape()
            .map(|shape| shape.flatten().into_iter().map(xy).collect())
            .unwrap_or_default()
    }
}

/// Flattened with the schematic Bezier's own sampling.
fn bezier_outline(points: &[[f64; 2]; 4]) -> Vec<[f64; 2]> {
    let [start, c1, c2, end] = points.map(pt);
    SchBezier::new(Uuid::nil(), start, c1, c2, end)
        .flatten()
        .into_iter()
        .map(xy)
        .collect()
}
