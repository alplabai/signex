//! Storage adapter implementations behind feature flags.
//!
//! Each flavour from `LIBRARY_PLAN` ships as its own module gated on a Cargo
//! feature so consumers only pull in the deps they actually use.
//!
//! - `local-git` → [`local_git::LocalGitAdapter`] backed by a `*.snxlib/` dir
//!   plus an embedded libgit2 repo (Phase 1 WS-A).

#[cfg(feature = "local-git")]
pub mod local_git;
