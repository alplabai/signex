//! A document's content changed: which one, what changed, and why.

use std::path::PathBuf;

use signex_engine::DocumentPatch;

/// Why a document changed. A history step (undo, redo, an agent's revert)
/// can bring back an older embedded library symbol, so the library drift
/// check follows it; an edit does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChangeCause {
    Edit,
    Step,
}

/// The document at `path` changed by `patch` (the engine's own fact,
/// `signex_engine::DocumentPatch`, #2445).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DocumentChanged {
    pub(crate) path: PathBuf,
    pub(crate) patch: DocumentPatch,
    pub(crate) cause: ChangeCause,
}
