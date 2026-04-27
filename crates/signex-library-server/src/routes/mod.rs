//! HTTP route modules — split by resource per the WS-B file plan.
//!
//! WS-4 (`v0.9-refactor-2-plan.md` §9) replaced the legacy
//! `routes::components` + `routes::revisions` pair with `routes::tables` +
//! `routes::rows` over the DBLib row model. The primitive route family
//! (symbols / footprints / sims) is unchanged. The shared `ApiError`
//! envelope lives in `routes::error` so every module can sit on the same
//! status-code → JSON-body contract.

pub mod error;
pub mod footprints;
pub mod locks;
pub mod rows;
pub mod sims;
pub mod symbols;
pub mod tables;
