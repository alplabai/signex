//! The parametric sketch data model shared by footprints and boards
//! (#2234): entities, constraints, planes, arrays, the parameter table,
//! the bake attributes and the baked-kind ownership markers.
//!
//! Data only. The constraint solver, the expression language and the
//! geometry kernels live in `signex-sketch`, which re-exports these
//! modules under their old paths.

pub mod array;
pub mod attr;
pub mod baked_kinds;
pub mod constraint;
pub mod data;
pub mod driven;
pub mod entity;
pub mod host;
pub mod id;
pub(crate) mod layer_serde;
pub mod parameter;
pub mod plane;

pub use attr::LinkedRadius;
pub use data::SketchData;
pub use host::{SketchAction, SketchHost, SketchRole};

#[cfg(test)]
mod board_tests;
pub mod format_level;
pub mod pad_override;
pub mod pattern_links;
#[cfg(test)]
mod pattern_links_tests;
