//! HTTP route modules — split by resource.
//!
//! The DBLib row tier lives in `routes::rows`; primitives are
//! `routes::symbols` / `routes::footprints` / `routes::sims`. The
//! shared `ApiError` envelope lives in `routes::error` so every
//! module can sit on the same status-code → JSON-body contract.

pub mod error;
pub mod footprints;
pub mod locks;
pub mod rows;
pub mod sims;
pub mod symbols;
pub mod tables;
