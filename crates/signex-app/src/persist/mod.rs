//! Debounced, off-thread persistence building blocks shared by the dock
//! layout and the search-box preferences (#692, #859).

mod debounced;
mod search_prefs;

pub use debounced::{DebouncedStore, DebouncedSync, PendingWrite, WriteBarrier};
pub use search_prefs::{
    SEARCH_PREFS_SAVE_DEBOUNCE, SearchPrefsChange, SearchPrefsMsg, SearchPrefsSync,
    SearchPrefsWrite,
};
