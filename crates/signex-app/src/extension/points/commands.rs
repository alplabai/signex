//! Catalog commands an extension adds.
//!
//! The application core reads this table (ADR-0006), so this file names no
//! iced type and no GUI state. A command's id sits under its owner's
//! `ext.<name>.` prefix, never in `scripts/catalog-ids.txt`. Headless runs
//! follow the core's rules: a command has a core handler or is GUI-only.

/// The catalog commands one extension adds. No entry type yet: the change
/// that opens this point (#2495) adds it here.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct CatalogCommands {}

impl CatalogCommands {
    /// Were no commands added?
    pub fn is_empty(&self) -> bool {
        true
    }
}
