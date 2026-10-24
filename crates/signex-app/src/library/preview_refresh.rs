//! Refreshing resolved previews after a primitive save (#968).
//!
//! A Component Preview tab resolves its row's symbol and footprint once
//! and keeps them on [`ComponentPreviewState`]. Saving the primitive in
//! its own editor wrote new bytes, but the tab went on drawing the copy
//! it had resolved before, until it was closed and reopened.

use std::path::Path;

use signex_library::PrimitiveRef;

use super::resolve::{ResolvedKind, report_read_failure};
use super::state::LibraryState;

impl LibraryState {
    /// Re-resolve the symbol and footprint of every Component Preview
    /// whose row binds a primitive in the library whose directory
    /// contains `saved`. The binding's `library_id` decides, not the
    /// library the preview tab belongs to: a row may bind a primitive
    /// from another library.
    ///
    /// Only previews that already resolved a primitive are touched; one
    /// that has not been visited stays lazy. The sim is left alone: its
    /// `sim_body` is a live edit buffer and replacing it would discard
    /// typing. A lookup that fails keeps the previous copy and reports
    /// the failure, rather than blanking a preview over a transient read
    /// error.
    pub fn refresh_loaded_previews(&mut self, saved: &Path) {
        let Some(saved_library) = self
            .open_libraries
            .iter()
            .find(|lib| {
                lib.root_dir()
                    .map(|d| saved.starts_with(d))
                    .unwrap_or(false)
            })
            .map(|lib| lib.library_id)
        else {
            return;
        };
        let set = &self.set;
        for state in self.editors.values_mut() {
            if state.symbol.is_some() && state.row.symbol_ref.library_id == saved_library {
                let reference = state.row.symbol_ref;
                if let Some(fresh) = reresolve(
                    set.resolve_symbol(&reference),
                    ResolvedKind::Symbol,
                    &reference,
                ) {
                    state.symbol = fresh;
                }
            }
            if state.footprint.is_some()
                && let Some(reference) = state.row.footprint_ref
                && reference.library_id == saved_library
                && let Some(fresh) = reresolve(
                    set.resolve_footprint(&reference),
                    ResolvedKind::Footprint,
                    &reference,
                )
            {
                state.footprint = fresh;
            }
        }
    }
}

/// `Some(copy)` when the lookup answered (`None` inside = the primitive
/// is gone), `None` when it failed and the old copy must stay.
fn reresolve<T>(
    resolved: Result<Option<T>, signex_library::LibraryError>,
    kind: ResolvedKind,
    reference: &PrimitiveRef,
) -> Option<Option<T>> {
    if resolved.is_err() {
        report_read_failure(
            resolved,
            kind,
            reference,
            "refreshing a preview after a primitive save",
        );
        return None;
    }
    resolved.ok()
}
