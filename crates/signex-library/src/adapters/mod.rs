//! Adapter implementations for `LibraryAdapter` flavours. Each module is
//! self-contained — no cross-module deps — so workstreams (WS-A local-git,
//! WS-B database, future WS for PLM) can land independently.

#[cfg(feature = "database")]
pub mod database;

// WS-A's `local_git` module ships in a sibling commit on the same branch.
