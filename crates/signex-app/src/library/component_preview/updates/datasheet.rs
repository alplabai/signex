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

/// Apply the result of a pinned-PDF upload: hash the bytes with SHA-256
/// and pin the datasheet to that content hash. A cancelled pick (`None`)
/// leaves the row untouched.
pub(super) fn apply_upload_result(
    state: &mut ComponentPreviewState,
    payload: Option<(Vec<u8>, String)>,
) {
    if let Some((bytes, filename)) = payload {
        use sha2::Digest;
        let mut hasher = sha2::Sha256::new();
        hasher.update(&bytes);
        let hash = format!("{:x}", hasher.finalize());
        state.row.datasheet = signex_library::DatasheetRef::hash_pinned(hash, filename);
        state.dirty = true;
    }
}
