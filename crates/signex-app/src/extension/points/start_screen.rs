//! Start-screen sections an extension adds. A GUI adapter: it stays out of
//! `app/core/` (ADR-0006).
//!
//! The Start screen has one named slot for extension sections; with no
//! extension installed it renders nothing there.

/// The Start-screen sections one extension adds. No entry type yet: the
/// change that opens this point (#2495) adds it here.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct StartScreenSections {}

impl StartScreenSections {
    /// Were no sections added?
    pub fn is_empty(&self) -> bool {
        true
    }
}
