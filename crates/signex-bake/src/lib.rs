//! Sketch → footprint primitive bake pipeline.
//!
//! Phase 7 of the v0.13 sketch-mode plan. Lives in its own crate
//! (rather than inside `signex-sketch` or `signex-library`) so we can
//! depend on both without a circular dependency: `signex-library`
//! depends on `signex-sketch` for `SketchData`, and this crate
//! depends on both to produce `signex-library::Pad` from
//! `signex_sketch` data.
//!
//! Cleanroom: derived from first principles + the Phase 4 expression
//! machinery. No third-party constraint-solver, footprint-generator,
//! or numerical-library source consulted.

pub mod array;
pub mod courtyard;
pub mod mask;
pub mod pad;
pub mod pour;
pub mod profile;
pub mod silk;

pub use array::bake_arrays;
pub use courtyard::bake_courtyard;
pub use mask::{bake_mask_excludes, bake_mask_openings, bake_paste_apertures};
pub use pad::bake_pads;
pub use pour::bake_pours;
pub use profile::{trace_closed_profile, TraceError, TraceResult};
pub use silk::bake_silk;
