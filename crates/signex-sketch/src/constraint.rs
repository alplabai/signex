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
