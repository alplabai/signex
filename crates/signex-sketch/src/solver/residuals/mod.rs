//! Per-family residual implementations.
//!
//! Each module owns one family of constraint kinds and exposes
//! `pub fn <name>(...) -> Result<Vec<f64>, SketchError>` functions
//! invoked by the dispatcher in [`crate::solver::residual`].

pub mod equal_tangent;
pub mod parallel_perp_angle;
pub mod point_on;
pub mod symmetric_midpoint;
