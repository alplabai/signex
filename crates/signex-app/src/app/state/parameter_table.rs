//! Transient state of the Parameter Manager table.
//!
//! Cell text is never held here: every keystroke is written to the document
//! at once, and the engine folds the keystrokes of one cell into one undo
//! step, so no typed text lives outside the document.

#[derive(Debug, Clone, Default)]
pub struct ParameterTableState {
    /// Name typed into the "new parameter" box.
    pub new_key: String,
    /// Case-insensitive row filter over reference / value / footprint.
    pub filter: String,
}
