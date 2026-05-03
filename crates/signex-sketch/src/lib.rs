//! Signex 2D parametric sketcher.
//!
//! Cleanroom implementation against
//! [`SKETCH_MODE_PLAN.md`] (`docs/internal/`). No third-party
//! constraint-solver source consulted; algorithm sourced from
//! Hearn & Baker §10–§12 and Numerical Recipes (Press et al.) §15.
//!
//! Public surface:
//! - [`SketchData`] — top-level sketch container
//! - [`Solver`] — Newton-LM constraint solver
//! - [`bake`] — sketch → footprint primitive bake
//!
//! See `docs/internal/SKETCH_MODE_v0.13_PLAN.md` for the
//! release plan.

pub mod array;
pub mod attr;
pub mod bake;
pub mod constraint;
pub mod entity;
pub mod error;
pub mod expr;
pub mod id;
pub mod parameter;
pub mod plane;
pub mod sketch;
pub mod solver;
pub mod unit;

pub use error::{SketchError, SolveError};
pub use sketch::SketchData;
pub use solver::Solver;
