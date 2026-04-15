#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticPatch {
    DocumentReplaced,
    TextUpdated,
    SymbolFieldsUpdated,
    SelectionDeleted,
    SelectionMoved,
    SelectionRotated,
    SelectionMirrored,
    ObjectPlaced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentPatch {
    DocumentReplaced,
    TextUpdated,
    SymbolFieldsUpdated,
    SelectionDeleted,
    SelectionMoved,
    SelectionRotated,
    SelectionMirrored,
    ObjectPlaced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PatchPair {
    pub semantic: SemanticPatch,
    pub document: DocumentPatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandResult {
    pub changed: bool,
    pub patch_pair: Option<PatchPair>,
}

impl CommandResult {
    pub fn changed(patch_pair: PatchPair) -> Self {
        Self {
            changed: true,
            patch_pair: Some(patch_pair),
        }
    }

    pub fn unchanged() -> Self {
        Self {
            changed: false,
            patch_pair: None,
        }
    }
}