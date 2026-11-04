//! The cascade of a primitive save into the other open libraries (#2004).
//!
//! A row binds a primitive by `(library_id, uuid)`, so a row of library B can
//! use a footprint or symbol that lives in library A. The save in A scans only
//! A's rows; this scans every other open library for rows that bind the saved
//! primitive, each under its own workflow mode: a released row of a Team
//! library stays at its old pin, exactly as in the primitive's own library.

use super::{
    AdapterCascade, CascadeOutcome, Level, SavedKind, SavedPrimitive, cascade_in_adapter, cut,
    saves_in,
};
use crate::library::writes::OwnRevisions;
use signex_library::LibraryAdapter;
use signex_library::adapters::local_git::LocalGitAdapter;
use std::path::Path;
use uuid::Uuid;

/// An open library other than the one the primitive was saved in.
pub(in crate::app::dispatch::library) struct OtherLibrary<'a> {
    /// Its `.snxlib` file.
    pub root: &'a Path,
    /// The ledger of this app's writes to it, when the save holds a place in
    /// its write order.
    pub own: Option<&'a OwnRevisions>,
    /// Waits for that place; `false` when it did not come in time: nothing
    /// is then written to the library and the cascade is owed. Called only
    /// when the cascade is about to write.
    pub ready: Box<dyn Fn() -> bool + 'a>,
}

fn library_name(root: &Path) -> String {
    root.file_stem().map_or_else(
        || root.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// Move the rows of every `others` library that bind a saved primitive of
/// library `primitive_library`. Rows moved are added to `outcome`; a library
/// a retry can still bring up to date makes the whole cascade owed, and so
/// does one that fails to open for a reason a retry can clear. One that
/// cannot be opened for good is reported but not owed: an unrelated broken
/// mount must not keep the editor unsaved.
pub(super) fn cascade_into(
    primitive_library: Uuid,
    kind: SavedKind,
    saved: &[SavedPrimitive],
    others: &[OtherLibrary<'_>],
    (label, version): (&str, Option<&str>),
    outcome: &mut CascadeOutcome,
) -> Vec<(Level, String)> {
    let mut lines = Vec::new();
    let saves = saves_in(kind, primitive_library, saved);
    for other in others {
        let name = library_name(other.root);
        if !(other.ready)() {
            outcome.owed = saved.iter().map(|p| p.uuid).collect();
            lines.push((
                Level::Warn,
                cut(format!(
                    "Save again once library {name} is idle; rows of it that use {label} were not updated"
                )),
            ));
            continue;
        }
        let adapter = match LocalGitAdapter::open(other.root) {
            Ok(adapter) => adapter,
            Err(e) => {
                // The same decision the saved library's own open makes: a
                // failure a retry can clear is owed, a permanent one dropped.
                let text = if signex_library::retry_cannot_fix(&e) {
                    format!(
                        "Library {name} could not be opened ({e}); its rows that use {label} were not checked"
                    )
                } else {
                    outcome.owed = saved.iter().map(|p| p.uuid).collect();
                    format!(
                        "Library {name} could not be opened ({e}); save again to update its rows that use {label}"
                    )
                };
                lines.push((Level::Warn, cut(text)));
                continue;
            }
        };
        if adapter.manifest().library.library_id == primitive_library {
            continue;
        }
        let AdapterCascade {
            bumped,
            owed,
            lines: step,
        } = cascade_in_adapter(
            &adapter,
            &saves,
            other.own,
            &format!("{label} in {name}"),
            version,
        );
        crate::library::held_back::report_held_back(&name, &adapter as &dyn LibraryAdapter);
        if !bumped.is_empty() {
            outcome
                .bumped_elsewhere
                .push((other.root.to_path_buf(), bumped));
        }
        if owed {
            outcome.owed = saved.iter().map(|p| p.uuid).collect();
        }
        lines.extend(step);
    }
    lines
}
