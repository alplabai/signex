//! The `Document` trait: one question every open document answers.
//!
//! Kind state lives in different maps (engines, symbol and footprint
//! editors, `library.editors`, the dirty path set), so there is no boxing:
//! a [`DocumentKey`] names the owner and [`Signex::document`] borrows
//! it as a [`DocumentRef`]. Only the dirty query lives here so far.

mod key;
#[cfg(test)]
mod tests;
mod undo_target;

pub(crate) use key::DocumentKey;
pub(crate) use undo_target::UndoTarget;

use std::path::Path;

use crate::app::state::DocumentState;
use crate::app::{FootprintDocument, Signex, SymbolDocument};
use crate::library::state::{ComponentPreviewState, LibraryState};

/// What every open document can say about its unsaved edits.
pub(crate) trait Document {
    /// Does it hold edits that are not on disk (the flag the quit and close
    /// gates read)?
    fn is_dirty(&self) -> bool;

    /// Unsaved work including input not yet applied to the document. Differs
    /// from [`Self::is_dirty`] only for a Component row, whose half-typed
    /// parameter buffer counts here (#2132 decides whether it should count
    /// at the gates too).
    fn has_unsaved_input(&self) -> bool {
        self.is_dirty()
    }
}

impl Document for SymbolDocument {
    fn is_dirty(&self) -> bool {
        SymbolDocument::is_dirty(self)
    }
}

impl Document for FootprintDocument {
    fn is_dirty(&self) -> bool {
        FootprintDocument::is_dirty(self)
    }
}

impl Document for ComponentPreviewState {
    fn is_dirty(&self) -> bool {
        self.dirty
    }

    fn has_unsaved_input(&self) -> bool {
        self.has_unsaved_edits()
    }
}

/// A borrow of the owner behind a [`DocumentKey`]. A sheet, PCB or project
/// file is owned by the dirty path set in `docs`; a draft by its editor,
/// plus the set while a save's write has not landed.
pub(crate) enum DocumentRef<'a> {
    Schematic {
        docs: &'a DocumentState,
        path: &'a Path,
    },
    Pcb {
        docs: &'a DocumentState,
        path: &'a Path,
    },
    Project {
        docs: &'a DocumentState,
        path: &'a Path,
    },
    Symbol {
        docs: &'a DocumentState,
        path: &'a Path,
        draft: &'a SymbolDocument,
    },
    Footprint {
        docs: &'a DocumentState,
        path: &'a Path,
        draft: &'a FootprintDocument,
    },
    Row(&'a ComponentPreviewState),
}

impl Document for DocumentRef<'_> {
    fn is_dirty(&self) -> bool {
        match self {
            Self::Schematic { docs, path }
            | Self::Pcb { docs, path }
            | Self::Project { docs, path } => docs.is_document_dirty(path),
            Self::Symbol { docs, path, draft } => {
                docs.is_document_stored_dirty(path) || Document::is_dirty(*draft)
            }
            Self::Footprint { docs, path, draft } => {
                docs.is_document_stored_dirty(path) || Document::is_dirty(*draft)
            }
            Self::Row(row) => Document::is_dirty(*row),
        }
    }

    fn has_unsaved_input(&self) -> bool {
        match self {
            Self::Row(row) => row.has_unsaved_input(),
            other => other.is_dirty(),
        }
    }
}

impl Signex {
    /// [`resolve_document`] over the app's own state.
    pub(crate) fn document<'a>(&'a self, key: &'a DocumentKey) -> Option<DocumentRef<'a>> {
        resolve_document(&self.document_state, &self.library, key)
    }
}

/// The owner of `key`'s state, or `None` when it is not open (a Component
/// row with no editor, a project that closed). Takes the two fields it reads
/// so a caller holding split borrows can use it. A shared, short-lived
/// borrow for queries only: never stored, and writes stay `Signex` methods.
pub(crate) fn resolve_document<'a>(
    docs: &'a DocumentState,
    library: &'a LibraryState,
    key: &'a DocumentKey,
) -> Option<DocumentRef<'a>> {
    match key {
        DocumentKey::Row(address) => library.editors.get(address).map(DocumentRef::Row),
        DocumentKey::Schematic(path) => Some(DocumentRef::Schematic { docs, path }),
        DocumentKey::Pcb(path) => Some(DocumentRef::Pcb { docs, path }),
        DocumentKey::Project(id) => {
            let path = docs.project_by_id(*id)?.path.as_path();
            Some(DocumentRef::Project { docs, path })
        }
        DocumentKey::Symbol(path) => {
            let draft = docs.symbol_editors.get(path)?;
            Some(DocumentRef::Symbol { docs, path, draft })
        }
        DocumentKey::Footprint(path) => {
            let draft = docs.footprint_editors.get(path)?;
            Some(DocumentRef::Footprint { docs, path, draft })
        }
    }
}
