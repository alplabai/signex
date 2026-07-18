//! Storage adapter implementations behind feature flags.
//!
//! Each flavour ships as its own module gated on a Cargo feature so
//! consumers only pull in the deps they actually use.
//!
//! - `local-git` → [`local_git::LocalGitAdapter`] backed by a `*.snxlib/` dir
//!   plus an embedded libgit2 repo.
//! - `database` → [`database::DatabaseAdapter`] HTTP client speaking to
//!   `signex-library-server`.
//! - [`library_set::LibrarySet`] composes any number of `LibraryAdapter`
//!   trait objects into a single resolver for cross-library
//!   [`crate::primitive::PrimitiveRef`] lookup.

#[cfg(feature = "local-git")]
pub mod local_git;

// The project-scoped adapter now lives at `local_git::project`; keep
// the flat `local_git_project` path so external
// `adapters::local_git_project::…` consumers resolve unchanged.
#[cfg(feature = "local-git")]
pub use local_git::project as local_git_project;

#[cfg(feature = "database")]
pub mod database;

pub mod library_set;
