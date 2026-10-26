//! The documents of a placed symbol's library row, for the Properties
//! panel (#1231). A symbol placed from a `.snxlib` row carries the row's
//! identity; the row itself is read from the open library caches, so the
//! list follows the library without being copied into the sheet.

use signex_library::{ComponentRow, DatasheetRef};
use uuid::Uuid;

use super::OpenLibrary;

/// Selection-info key prefix for one library document; the rest of the
/// key is the document's label.
pub(crate) const DOCUMENT_KEY: &str = "Document: ";

/// The row `row_id` of the open library `library_id`, if it is loaded.
fn find_row(libraries: &[OpenLibrary], library_id: Uuid, row_id: Uuid) -> Option<&ComponentRow> {
    libraries
        .iter()
        .filter(|lib| lib.library_id == library_id)
        .flat_map(|lib| lib.tables.values().flatten())
        .find(|row| row.row_id == row_id)
}

/// What a document shows: its URL, or the pinned file's name. `None`
/// for an empty slot.
fn shown(source: &DatasheetRef) -> Option<String> {
    match source {
        DatasheetRef::Url { url } if !url.trim().is_empty() => Some(url.trim().to_string()),
        DatasheetRef::HashPinned { filename, hash } if !hash.is_empty() => Some(filename.clone()),
        _ => None,
    }
}

/// `("Document: <label>", value)` for every non-empty document on the
/// row, the primary first. An unlabelled document is called "Datasheet".
pub(crate) fn document_rows(
    libraries: &[OpenLibrary],
    library_id: Option<Uuid>,
    row_id: Option<Uuid>,
) -> Vec<(String, String)> {
    let (Some(library_id), Some(row_id)) = (library_id, row_id) else {
        return Vec::new();
    };
    find_row(libraries, library_id, row_id)
        .map(row_document_rows)
        .unwrap_or_default()
}

/// The selection-info rows for one library row's documents.
fn row_document_rows(row: &ComponentRow) -> Vec<(String, String)> {
    row.documents()
        .into_iter()
        .filter_map(|doc| {
            let value = shown(&doc.source)?;
            let label = if doc.label.trim().is_empty() {
                "Datasheet".to_string()
            } else {
                doc.label.trim().to_string()
            };
            Some((format!("{DOCUMENT_KEY}{label}"), value))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_library::DatasheetDoc;

    #[test]
    fn rows_list_every_filled_document_primary_first() {
        let mut row = crate::library::component_preview::test_row();
        row.set_documents(vec![
            DatasheetDoc::new("", DatasheetRef::url(" https://ti.example/u1.pdf ")),
            DatasheetDoc::new("Errata", DatasheetRef::hash_pinned("ab12", "errata.pdf")),
            DatasheetDoc::new("Empty", DatasheetRef::default()),
            DatasheetDoc::new("Unpinned", DatasheetRef::hash_pinned("", "")),
        ]);
        assert_eq!(
            row_document_rows(&row),
            vec![
                (
                    "Document: Datasheet".to_string(),
                    "https://ti.example/u1.pdf".to_string()
                ),
                ("Document: Errata".to_string(), "errata.pdf".to_string()),
            ]
        );
    }

    #[test]
    fn a_symbol_without_a_library_row_lists_nothing() {
        assert!(document_rows(&[], None, None).is_empty());
        assert!(document_rows(&[], Some(Uuid::new_v4()), Some(Uuid::new_v4())).is_empty());
    }
}
