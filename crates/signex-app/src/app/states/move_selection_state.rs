/// Transient state for the Altium-style Move Selection dialog.
/// Deltas are stored as strings so mid-edit partial values (`-`, `2.`)
/// don't panic through number parsing; the Apply handler parses them.
#[derive(Debug, Clone, Default)]
pub struct MoveSelectionState {
    pub open: bool,
    pub dx: String,
    pub dy: String,
}
