//! HTTP route modules — split by resource per the WS-B file plan.
//!
//! WS-D adds the primitive routes (`symbols` / `footprints` / `sims`)
//! alongside the existing component routes, addressed by `(library_id, uuid)`
//! tuples per the v0.9 library refactor plan §9.

pub mod components;
pub mod footprints;
pub mod locks;
pub mod revisions;
pub mod sims;
pub mod symbols;
