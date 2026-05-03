use serde::{Deserialize, Serialize};

use crate::id::ConstraintId;

/// Stub container — full enum lands in Phase 2.
///
/// The Phase 2 plan (Tasks 2.1–2.8) replaces this with a full
/// `Constraint` enum + per-variant residual functions covering
/// Coincident, Distance, Horizontal, Vertical, Parallel,
/// Perpendicular, Tangent, Equal, Symmetric, PointOnLine, PointOnArc,
/// Angle, Concentric, Fixed, Midpoint, plus typed `DimTarget` /
/// `AngleTarget` for parametric dimensions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Constraint {
    pub id: ConstraintId,
}
