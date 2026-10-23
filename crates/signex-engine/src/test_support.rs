//! Fixtures shared by this crate's in-src `#[cfg(test)]` modules.
//!
//! `SchematicSheet` is an 18-field struct with no `Default`, so every test
//! module that needs an empty sheet would otherwise carry its own copy of
//! the literal — and each copy would have to be updated by hand the next
//! time the sheet grows a field. Mirrors the `test_support` module
//! `signex-app` keeps for the same reason.

use signex_types::schematic::SchematicSheet;

/// An empty A4 sheet: no symbols, no wires, no graphics, no title block.
pub(crate) fn test_sheet() -> SchematicSheet {
    SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        ..Default::default()
    }
}
