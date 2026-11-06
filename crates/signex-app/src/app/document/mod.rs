//! The `Document` trait: one question every open document answers.
//!
//! Kind state lives in different maps (engines, symbol and footprint
//! editors, `library.editors`, the dirty path set), so there is no boxing:
//! a [`DocumentKey`] names the owner and [`DocumentState::document`] borrows
//! it as a [`DocumentRef`]. Only the dirty query lives here so far.

mod key;
#[cfg(test)]
mod tests;

pub(crate) use key::DocumentKey;

use crate::app::state::DocumentState;
use crate::app::{FootprintDocument, SymbolDocument};
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

/// A borrow of the owner behind a [`DocumentKey`].
#[derive(Debug)]
pub(crate) enum DocumentRef<'a> {
    /// A sheet, PCB or project file: the dirty path set is the owner.
    Path {
        dirty: bool,
    },
    /// A symbol draft; `path_dirty` covers a save whose write has not landed.
    Symbol {
        draft: &'a SymbolDocument,
        path_dirty: bool,
    },
    Footprint {
        draft: &'a FootprintDocument,
        path_dirty: bool,
    },
    Row(&'a ComponentPreviewState),
}

impl Document for DocumentRef<'_> {
    fn is_dirty(&self) -> bool {
        match self {
            Self::Path { dirty } => *dirty,
            Self::Symbol { draft, path_dirty } => *path_dirty || Document::is_dirty(*draft),
            Self::Footprint { draft, path_dirty } => *path_dirty || Document::is_dirty(*draft),
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

impl DocumentState {
    /// The owner of `key`'s state, or `None` for a Component row with no
    /// open editor.
    pub(crate) fn document<'a>(
        &'a self,
        key: &DocumentKey,
        library: &'a LibraryState,
    ) -> Option<DocumentRef<'a>> {
        match key {
            DocumentKey::Row(address) => library.editors.get(address).map(DocumentRef::Row),
            DocumentKey::Symbol(path) => Some(match self.symbol_editors.get(path) {
                Some(draft) => DocumentRef::Symbol {
                    draft,
                    path_dirty: self.is_document_dirty(path),
                },
                None => self.path_document(path),
            }),
            DocumentKey::Footprint(path) => Some(match self.footprint_editors.get(path) {
                Some(draft) => DocumentRef::Footprint {
                    draft,
                    path_dirty: self.is_document_dirty(path),
                },
                None => self.path_document(path),
            }),
            DocumentKey::Schematic(path) | DocumentKey::Pcb(path) => Some(self.path_document(path)),
            DocumentKey::Project(id) => {
                let project = self.project_by_id(*id)?;
                Some(self.path_document(&project.path))
            }
        }
    }

    fn path_document(&self, path: &std::path::Path) -> DocumentRef<'_> {
        DocumentRef::Path {
            dirty: self.is_document_dirty(path),
        }
    }
}
