//! Storage adapter implementations behind feature flags.
//!
//! Each flavour from `LIBRARY_PLAN` ships as its own module gated on a Cargo
//! feature so consumers only pull in the deps they actually use.
//!
//! - `local-git` → [`local_git::LocalGitAdapter`] backed by a `*.snxlib/` dir
//!   plus an embedded libgit2 repo (Phase 1 WS-A).
//! - `database` → [`database::DatabaseAdapter`] HTTP client speaking to
//!   `signex-library-server` (Phase 1 WS-B).
//! - [`library_set::LibrarySet`] composes any number of `LibraryAdapter`
//!   trait objects into a single resolver for cross-library
//!   [`crate::primitive::PrimitiveRef`] lookup (v0.9 refactor §8 step C4).

#[cfg(feature = "local-git")]
pub mod local_git;

#[cfg(feature = "database")]
pub mod database;

pub mod library_set;
