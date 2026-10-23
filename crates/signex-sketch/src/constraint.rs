use serde::{Deserialize, Serialize};

use crate::id::{ConstraintId, SketchEntityId};

/// Optional dimension target — either a literal length/angle in
/// canonical units (mm or rad) or an expression string evaluated at
/// solve time.
///
/// Phase 2 honours `Literal` only; `Expr` is preserved through the
/// schema and evaluated by the parser/evaluator added in Phase 4.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DimTarget {
    Literal(f64),
    Expr(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Constraint {
    pub id: ConstraintId,
    #[serde(flatten)]
    pub kind: ConstraintKind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "PascalCase")]
pub enum ConstraintKind {
    Coincident {
        p1: SketchEntityId,
        p2: SketchEntityId,
    },
    PointOnLine {
        point: SketchEntityId,
        line: SketchEntityId,
    },
    PointOnArc {
        point: SketchEntityId,
        arc: SketchEntityId,
    },
    Horizontal {
        line: SketchEntityId,
    },
    Vertical {
        line: SketchEntityId,
    },
    Parallel {
        l1: SketchEntityId,
        l2: SketchEntityId,
    },
    Perpendicular {
        l1: SketchEntityId,
        l2: SketchEntityId,
    },
    DistancePtPt {
        p1: SketchEntityId,
        p2: SketchEntityId,
        target: DimTarget,
    },
    DistancePtLine {
        point: SketchEntityId,
        line: SketchEntityId,
        target: DimTarget,
    },
    /// v0.23 — Parametric distance from a Point to a Circle / Arc's
    /// boundary. Residual is `|p - centre| - radius - target` where
    /// radius is read from the live state (Circle: `radius` field;
    /// Arc: distance from `center` to `start`). Equivalent to
    /// `target = 0` placing the point ON the curve; positive target
    /// offsets outward, negative offsets inward.
    ///
    /// Used by the Offset tool's Circle/Arc paths to keep a fresh
    /// parallel curve linked to the source — the user can then edit
    /// the offset distance from the Properties panel without
    /// re-running the tool.
    DistancePtCircle {
        point: SketchEntityId,
        circle: SketchEntityId,
        target: DimTarget,
    },
    Angle {
        l1: SketchEntityId,
        l2: SketchEntityId,
        target: DimTarget,
    },
    EqualLength {
        l1: SketchEntityId,
        l2: SketchEntityId,
    },
    EqualRadius {
        e1: SketchEntityId,
        e2: SketchEntityId,
    },
    TangentLineArc {
        line: SketchEntityId,
        arc: SketchEntityId,
    },
    TangentArcArc {
        a1: SketchEntityId,
        a2: SketchEntityId,
        internal: bool,
    },
    SymmetricAboutLine {
        p1: SketchEntityId,
        p2: SketchEntityId,
        line: SketchEntityId,
    },
    SymmetricAboutPoint {
        p1: SketchEntityId,
        p2: SketchEntityId,
        center: SketchEntityId,
    },
    Midpoint {
        point: SketchEntityId,
        line: SketchEntityId,
    },
    Fixed {
        point: SketchEntityId,
    },
}

impl ConstraintKind {
    /// Every entity this constraint names, in field order.
    pub fn entity_refs(&self) -> Vec<SketchEntityId> {
        use ConstraintKind::*;
        match self {
            Coincident { p1, p2 } | DistancePtPt { p1, p2, .. } => vec![*p1, *p2],
            PointOnLine { point, line }
            | DistancePtLine { point, line, .. }
            | Midpoint { point, line } => vec![*point, *line],
            PointOnArc { point, arc } => vec![*point, *arc],
            DistancePtCircle { point, circle, .. } => vec![*point, *circle],
            Horizontal { line } | Vertical { line } => vec![*line],
            Parallel { l1, l2 }
            | Perpendicular { l1, l2 }
            | Angle { l1, l2, .. }
            | EqualLength { l1, l2 } => vec![*l1, *l2],
            EqualRadius { e1, e2 } => vec![*e1, *e2],
            TangentLineArc { line, arc } => vec![*line, *arc],
            TangentArcArc { a1, a2, .. } => vec![*a1, *a2],
            SymmetricAboutLine { p1, p2, line } => vec![*p1, *p2, *line],
            SymmetricAboutPoint { p1, p2, center } => vec![*p1, *p2, *center],
            Fixed { point } => vec![*point],
        }
    }

    /// Number of scalar residuals this constraint contributes.
    pub fn residual_count(&self) -> usize {
        use ConstraintKind::*;
        match self {
            Coincident { .. } => 2,
            PointOnLine { .. } => 1,
            PointOnArc { .. } => 1,
            Horizontal { .. } => 1,
            Vertical { .. } => 1,
            Parallel { .. } => 1,
            Perpendicular { .. } => 1,
            DistancePtPt { .. } => 1,
            DistancePtLine { .. } => 1,
            DistancePtCircle { .. } => 1,
            Angle { .. } => 1,
            EqualLength { .. } => 1,
            EqualRadius { .. } => 1,
            TangentLineArc { .. } => 1,
            TangentArcArc { .. } => 1,
            SymmetricAboutLine { .. } => 2,
            SymmetricAboutPoint { .. } => 2,
            Midpoint { .. } => 2,
            Fixed { .. } => 0,
        }
    }
}
