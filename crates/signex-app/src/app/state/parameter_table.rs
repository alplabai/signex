//! Transient state of the Parameter Manager table.
//!
//! A cell is edited as text first and written to the document once, so a
//! burst of keystrokes is one undo step rather than one per character.

/// The cell being typed into and the text not yet written to the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellDraft {
    pub symbol_uuid: uuid::Uuid,
    pub key: String,
    pub text: String,
}

impl CellDraft {
    /// Whether this draft belongs to the cell `(symbol_uuid, key)`.
    pub fn is_for(&self, symbol_uuid: uuid::Uuid, key: &str) -> bool {
        self.symbol_uuid == symbol_uuid && self.key == key
    }
}

#[derive(Debug, Clone, Default)]
pub struct ParameterTableState {
    /// The uncommitted cell edit, if any.
    pub draft: Option<CellDraft>,
    /// Name typed into the "new parameter" box.
    pub new_key: String,
    /// Case-insensitive row filter over reference / value / footprint.
    pub filter: String,
}
