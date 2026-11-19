//! Where the ids of objects the engine derives come from (#2429, ADR-0006).
//!
//! A pin junction dot is made by the engine, not named by the caller's
//! command, yet the engine mints no ids: it draws them from the host's
//! [`IdSource`]. The production v7 source stands in until a host sets one
//! ([`crate::Engine::set_id_source`]); a test sets a deterministic one.

use signex_types::object_id::{IdSource, UuidV7Ids};

/// The id source of an engine's derived objects.
pub(crate) struct DerivedIds(Box<dyn IdSource + Send + Sync>);

impl DerivedIds {
    pub(crate) fn new(ids: Box<dyn IdSource + Send + Sync>) -> Self {
        Self(ids)
    }

    /// The source to draw the next ids from.
    pub(crate) fn source(&mut self) -> &mut dyn IdSource {
        &mut *self.0
    }
}

impl Default for DerivedIds {
    fn default() -> Self {
        Self(Box::new(UuidV7Ids))
    }
}

impl std::fmt::Debug for DerivedIds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DerivedIds")
    }
}
