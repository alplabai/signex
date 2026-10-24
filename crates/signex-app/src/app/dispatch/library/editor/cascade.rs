//! Version cascade after a primitive editor save (#832).
//!
//! A symbol / footprint save moves the primitive's version on. The
//! component rows that reference it pin the version they were made
//! against, and placed instances record the row's version, so a row left
//! at the old version hides the change from drift tracking.
//! `signex_library::cascade_after_*_save` bumps those rows; only
//! `LocalGitAdapter::save_*` called it, and the editor does not save
//! through that. This runs it on the editor's path.
//!
//! It matches what `LocalGitAdapter::save_*` does, so the two paths agree:
//!
//! - It runs after the save returned, when `rewrite_file_locked` has
//!   released the library write lock and committed the primitive file. The
//!   cascade takes the lock itself, once per row (`update_row`), and
//!   commits each row on its own, as the adapter does after `save_*`.
//!   Every step is atomic and commits, but save and cascade are not one
//!   transaction: a cascade failure leaves the saved primitive in place and
//!   the rows at their old version.
//! - The editor never calls `adapter.save_*`, so no save is cascaded twice.
//! - `mode` is the library's own `manifest().workflow.mode`, the value the
//!   adapter reads: Personal bumps every bound row, Team and Enterprise
//!   bump unreleased rows and leave released ones stale.
//! - Only a save that wrote cascades, and only for the primitives the
//!   merge reports as changed. A Save As replaces another file and never
//!   cascades.

use super::container_save::ContainerWrite;
use super::*;
use signex_library::{CascadeReport, LibraryError};
use uuid::Uuid;

/// Which primitive kind a save wrote.
#[derive(Clone, Copy)]
pub(super) enum SavedKind {
    Symbol,
    Footprint,
}

/// One primitive this save changed, as the cascade needs it.
pub(super) struct SavedPrimitive {
    pub uuid: Uuid,
    pub name: String,
    pub version: String,
}

impl Signex {
    /// Bump the component rows that use each saved primitive, and tell the
    /// user. `saved` is empty when the save wrote nothing. Nothing to do
    /// for a Save As or a file outside every mounted library.
    pub(super) fn cascade_after_container_save(
        &self,
        path: &std::path::Path,
        write: ContainerWrite,
        kind: SavedKind,
        saved: &[SavedPrimitive],
    ) {
        if write != ContainerWrite::Merge || saved.is_empty() {
            return;
        }
        let Some(adapter) = self.library_adapter_for(path) else {
            return;
        };
        let mode = adapter.manifest().workflow.mode;
        for primitive in saved {
            let result = match kind {
                SavedKind::Symbol => signex_library::cascade_after_symbol_save(
                    adapter,
                    primitive.uuid,
                    &primitive.version,
                    mode,
                ),
                SavedKind::Footprint => signex_library::cascade_after_footprint_save(
                    adapter,
                    primitive.uuid,
                    &primitive.version,
                    mode,
                ),
            };
            report_cascade(primitive, result);
        }
    }
}

fn report_cascade(primitive: &SavedPrimitive, result: Result<CascadeReport, LibraryError>) {
    match result {
        Ok(report) if report.auto_bumped.is_empty() => {}
        Ok(report) => tracing::info!(
            target: "signex::library",
            "Bumped {} component rows that use {} to v{}",
            report.auto_bumped.len(),
            primitive.name,
            primitive.version,
        ),
        Err(e) => tracing::error!(
            target: "signex::library",
            "{} saved, but its component rows were not all bumped: {e}",
            primitive.name,
        ),
    }
}
