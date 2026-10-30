//! Resolving primitives through shared adapter handles, for workers.
//!
//! `LibrarySet::resolve_*` borrows the set, so it cannot leave the UI
//! thread. [`SharedResolver`] holds the same adapters as `Arc` handles and
//! answers the same way: a library that is not mounted, or a primitive it
//! does not hold, is `Ok(None)`; a failed lookup is an `Err` (#859).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use signex_library::{
    Footprint, LibraryAdapter, LibraryError, PrimitiveRef, SimModel, Symbol, absence_is_none,
};
use uuid::Uuid;

use super::state::LibraryState;

/// Every mounted adapter, by library id.
pub struct SharedResolver {
    adapters: HashMap<Uuid, Arc<dyn LibraryAdapter>>,
}

impl SharedResolver {
    /// A symbol, as `LibrarySet::resolve_symbol` would answer.
    pub fn resolve_symbol(&self, r: &PrimitiveRef) -> Result<Option<Symbol>, LibraryError> {
        match self.adapters.get(&r.library_id) {
            Some(lib) => absence_is_none(lib.get_symbol(r.uuid)),
            None => Ok(None),
        }
    }

    /// A footprint, as `LibrarySet::resolve_footprint` would answer.
    pub fn resolve_footprint(&self, r: &PrimitiveRef) -> Result<Option<Footprint>, LibraryError> {
        match self.adapters.get(&r.library_id) {
            Some(lib) => absence_is_none(lib.get_footprint(r.uuid)),
            None => Ok(None),
        }
    }

    /// A simulation model, as `LibrarySet::resolve_sim` would answer.
    pub fn resolve_sim(&self, r: &PrimitiveRef) -> Result<Option<SimModel>, LibraryError> {
        match self.adapters.get(&r.library_id) {
            Some(lib) => absence_is_none(lib.get_sim(r.uuid)),
            None => Ok(None),
        }
    }
}

impl LibraryState {
    /// A resolver over everything mounted now.
    pub fn shared_resolver(&self) -> SharedResolver {
        let adapters = self
            .set
            .library_ids()
            .filter_map(|id| self.set.get_shared(id).map(|adapter| (id, adapter)))
            .collect();
        SharedResolver { adapters }
    }
}

/// One-shot carrier for a worker's answer across the `Task::perform`
/// boundary (`LibraryError` is not `Clone`). `take` yields the payload
/// once; the payload is `None` when the worker died.
pub struct OneShotCell<T>(Arc<Mutex<Option<Option<T>>>>);

impl<T> OneShotCell<T> {
    /// Wrap the answer (`None` = the worker died).
    pub fn new(answer: Option<T>) -> Self {
        Self(Arc::new(Mutex::new(Some(answer))))
    }

    /// Take the payload; `None` on a second read.
    pub fn take(&self) -> Option<Option<T>> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
    }
}

impl<T> Clone for OneShotCell<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<T> std::fmt::Debug for OneShotCell<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OneShotCell(..)")
    }
}
