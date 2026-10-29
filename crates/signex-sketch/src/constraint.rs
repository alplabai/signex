use serde::{Deserialize, Serialize};

use crate::id::{ConstraintId, SketchEntityId};

/// Parse the text of a dimension input box into a canonical-unit number.
///
/// Rejects anything that is not a finite number: `NaN`, `inf` and values
/// that overflow `f64` all parse under `f64::from_str` but would make
/// every later solve diverge.
pub fn parse_dimension_input(text: &str) -> Result<f64, crate::expr::ExprError> {
    signex_types::numeric::parse_finite_f64(text)
        .map_err(|_| crate::expr::ExprError::BadQuantity(text.to_string()))
}

/// Optional dimension target — either a literal length/angle in
/// canonical units (mm or rad) or an expression string evaluated at
/// solve time.
///
/// `Expr` is evaluated against the resolved parameter map when the
/// constraint's residual is computed.
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
    /// The dimension this constraint is driven toward, for the kinds that
    /// carry one.
    pub fn dim_target(&self) -> Option<&DimTarget> {
        use ConstraintKind::*;
        match self {
            DistancePtPt { target, .. }
            | DistancePtLine { target, .. }
            | DistancePtCircle { target, .. }
            | Angle { target, .. } => Some(target),
            _ => None,
        }
    }

    /// The kind's name as the constraints list shows it ("Coincident",
    /// "DistancePtPt", ...).
    pub fn label(&self) -> &'static str {
        use ConstraintKind::*;
        match self {
            Coincident { .. } => "Coincident",
            PointOnLine { .. } => "PointOnLine",
            PointOnArc { .. } => "PointOnArc",
            Horizontal { .. } => "Horizontal",
            Vertical { .. } => "Vertical",
            Parallel { .. } => "Parallel",
            Perpendicular { .. } => "Perpendicular",
            DistancePtPt { .. } => "DistancePtPt",
            DistancePtLine { .. } => "DistancePtLine",
            DistancePtCircle { .. } => "DistancePtCircle",
            Angle { .. } => "Angle",
            EqualLength { .. } => "EqualLength",
            EqualRadius { .. } => "EqualRadius",
            TangentLineArc { .. } => "TangentLineArc",
            TangentArcArc { .. } => "TangentArcArc",
            SymmetricAboutLine { .. } => "SymmetricAboutLine",
            SymmetricAboutPoint { .. } => "SymmetricAboutPoint",
            Midpoint { .. } => "Midpoint",
            Fixed { .. } => "Fixed",
        }
    }

    /// The first entity this constraint names: the one a click on its row
    /// selects.
    pub fn focus_entity(&self) -> Option<SketchEntityId> {
        self.entities().next()
    }

    /// Every entity this constraint names, in field order. Allocation-free.
    pub fn entities(&self) -> impl Iterator<Item = SketchEntityId> {
        use ConstraintKind::*;
        let slots: [Option<SketchEntityId>; 3] = match self {
            Coincident { p1, p2 } | DistancePtPt { p1, p2, .. } => [Some(*p1), Some(*p2), None],
            PointOnLine { point, line }
            | DistancePtLine { point, line, .. }
            | Midpoint { point, line } => [Some(*point), Some(*line), None],
            PointOnArc { point, arc } => [Some(*point), Some(*arc), None],
            DistancePtCircle { point, circle, .. } => [Some(*point), Some(*circle), None],
            Horizontal { line } | Vertical { line } => [Some(*line), None, None],
            Parallel { l1, l2 }
            | Perpendicular { l1, l2 }
            | Angle { l1, l2, .. }
            | EqualLength { l1, l2 } => [Some(*l1), Some(*l2), None],
            EqualRadius { e1, e2 } => [Some(*e1), Some(*e2), None],
            TangentLineArc { line, arc } => [Some(*line), Some(*arc), None],
            TangentArcArc { a1, a2, .. } => [Some(*a1), Some(*a2), None],
            SymmetricAboutLine { p1, p2, line } => [Some(*p1), Some(*p2), Some(*line)],
            SymmetricAboutPoint { p1, p2, center } => [Some(*p1), Some(*p2), Some(*center)],
            Fixed { point } => [Some(*point), None, None],
        };
        slots.into_iter().flatten()
    }

    /// Every entity this constraint names, in field order.
    pub fn entity_refs(&self) -> Vec<SketchEntityId> {
        self.entities().collect()
    }

    /// Whether this constraint names `id`.
    pub fn touches(&self, id: SketchEntityId) -> bool {
        self.entities().any(|e| e == id)
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
