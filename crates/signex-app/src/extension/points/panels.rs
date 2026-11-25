//! Dock panels an extension adds. A GUI adapter: it stays out of `app/core/`
//! (ADR-0006).
//!
//! A panel has a stable layout key under its owner's `ext.<name>.` prefix. A
//! saved layout that names a key no installed extension registered keeps
//! that key (`docs/DECISIONS.md`, 2026-11-25).

/// The dock panels one extension adds. No entry type yet: the change that
/// opens this point (#2495) adds it here.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct Panels {}

impl Panels {
    /// Were no panels added?
    pub fn is_empty(&self) -> bool {
        true
    }
}
