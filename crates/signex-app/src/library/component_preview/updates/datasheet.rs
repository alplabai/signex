//! Datasheet edits for a Component Preview row.
//!
//! Owns the Datasheet tab's three actions: switching between URL and
//! pinned-PDF modes, live URL entry, and applying an upload result by
//! content-hashing the picked bytes.

use crate::library::ComponentPreviewState;
use crate::library::editor::datasheet_picker::DatasheetMode;

/// Switch the datasheet reference between URL and pinned-PDF modes.
///
/// Resets the reference to a fresh default of the chosen kind only when
/// the current value is of the other kind, so re-selecting the active
/// mode is a no-op and does not clobber the existing value.
pub(super) fn set_mode(state: &mut ComponentPreviewState, mode: DatasheetMode) {
    match mode {
        DatasheetMode::Url => match &state.row.datasheet {
            signex_library::DatasheetRef::Url { .. } => {}
            _ => {
                state.row.datasheet = signex_library::DatasheetRef::default();
                state.dirty = true;
            }
        },
        DatasheetMode::PinnedPdf => match &state.row.datasheet {
            signex_library::DatasheetRef::HashPinned { .. } => {}
            _ => {
                state.row.datasheet = signex_library::DatasheetRef::HashPinned {
                    hash: String::new(),
                    filename: String::new(),
                };
                state.dirty = true;
            }
        },
    }
}

/// Set the datasheet to a URL reference, clearing it to the default when
/// the field is emptied.
pub(super) fn set_url(state: &mut ComponentPreviewState, url: String) {
    let trimmed = url.trim();
    state.row.datasheet = if trimmed.is_empty() {
        signex_library::DatasheetRef::default()
    } else {
        signex_library::DatasheetRef::url(trimmed)
    };
    state.dirty = true;
}

/// Apply the result of a pinned-PDF upload. The worker already stored the
/// bytes in the library's asset directory, so a stored pick pins the
/// datasheet to that hash. A cancelled pick (`Ok(None)`) leaves the row
/// untouched; a failure leaves the row untouched and shows the reason.
pub(super) fn apply_upload_result(
    state: &mut ComponentPreviewState,
    result: Result<Option<(String, String)>, String>,
) {
    match result {
        Ok(Some((hash, filename))) => {
            state.row.datasheet = signex_library::DatasheetRef::hash_pinned(hash, filename);
            state.datasheet_error = None;
            state.dirty = true;
        }
        Ok(None) => {}
        Err(reason) => state.datasheet_error = Some(reason),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_library::{DatasheetRef, store_datasheet};

    fn state_for(library_file: std::path::PathBuf) -> ComponentPreviewState {
        use signex_library::{
            ComponentClass, ComponentRow, InternalPn, LifecycleState, ManufacturerPart, ParamMap,
            PlmReserved, PrimitiveRef,
        };
        use uuid::Uuid;
        let row = ComponentRow {
            alternate_symbols: Vec::new(),
            row_id: Uuid::new_v4(),
            internal_pn: InternalPn::new("TEST_PN"),
            class: ComponentClass::generic(),
            datasheet: DatasheetRef::default(),
            state: LifecycleState::Draft,
            symbol_ref: PrimitiveRef::new(Uuid::new_v4(), Uuid::new_v4()),
            footprint_ref: None,
            sim_ref: None,
            pin_map_overrides: Vec::new(),
            primary_mpn: ManufacturerPart::draft("Acme", "ACM-001"),
            alternates: Vec::new(),
            supply: Vec::new(),
            parameters: ParamMap::new(),
            plm: PlmReserved::default(),
            version: "0.0.1".into(),
            released: false,
            symbol_version: String::new(),
            footprint_version: String::new(),
            sim_version: String::new(),
            created: chrono::Utc::now(),
            updated: chrono::Utc::now(),
            content_hash: [0u8; 32],
            server_revision: None,
        };
        ComponentPreviewState::from_row(library_file, "t".into(), row)
    }

    #[test]
    fn pinned_hash_resolves_to_stored_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let lib = dir.path().join("lib.snxlib");
        let bytes = b"%PDF-1.7 datasheet bytes";
        let hash = store_datasheet(&lib, bytes).unwrap();
        let mut state = state_for(lib.clone());
        apply_upload_result(&mut state, Ok(Some((hash, "ds.pdf".into()))));
        let DatasheetRef::HashPinned { hash, .. } = &state.row.datasheet else {
            panic!("expected a pinned datasheet");
        };
        let path = signex_library::datasheet_asset_path(&lib, hash).unwrap();
        assert_eq!(std::fs::read(path).unwrap(), bytes);
        assert!(state.dirty);
    }

    #[test]
    fn failed_upload_keeps_row_and_reports() {
        let mut state = state_for("x.snxlib".into());
        apply_upload_result(&mut state, Err("disk full".into()));
        assert_eq!(state.row.datasheet, DatasheetRef::default());
        assert_eq!(state.datasheet_error.as_deref(), Some("disk full"));
        assert!(!state.dirty);
    }
}
