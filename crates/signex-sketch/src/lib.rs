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
//!
//! See `docs/internal/SKETCH_MODE_v0.13_PLAN.md` for the
//! release plan.
//!
//! The sketch data model (entities, constraints, planes, arrays, the
//! parameter table, bake attributes, baked kinds) lives in
//! [`signex_types::sketch`] so footprints and boards share it (#2234). The
//! `array`, `attr`, `baked_kinds`, `constraint`, `entity`, `id`,
//! `parameter`, `plane` and `sketch` modules here re-export it under the
//! old paths.

pub mod arc_snap;
pub mod array;
pub mod attr;
pub mod baked_kinds;
pub mod constraint;
pub mod entity;
pub mod error;
pub mod expr;
pub mod geom;
pub mod id;
pub mod parameter;
pub mod plane;
pub mod sketch;
pub mod solver;
pub mod split;
pub mod unit;

pub use arc_snap::snap_arc_ends_onto_circles;
pub use attr::LinkedRadius;
pub use error::{SketchError, SolveError};
pub use sketch::SketchData;
pub use solver::Solver;
pub use split::{SplitError, SplitResult, split_line};
