//! Menu bar and Active Bar rows an extension adds. A GUI adapter: it stays
//! out of `app/core/` (ADR-0006).
//!
//! A row runs a command by id, so one command keeps one behaviour on every
//! surface. Built-in rows keep their order.

/// The menu and Active Bar rows one extension adds. No entry type yet: the
/// change that opens this point (#2495) adds it here.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct MenuRows {}

impl MenuRows {
    /// Were no rows added?
    pub fn is_empty(&self) -> bool {
        true
    }
}
