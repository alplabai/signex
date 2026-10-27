//! Version bump and row cascade after a primitive editor save (#832).
//!
//! A symbol / footprint save moves the primitive's version on. The
//! component rows that reference it pin the version they were made
//! against, and placed instances record the row's version, so a row left
//! at the old version hides the change from drift tracking.
//! `signex_library::cascade_after_saves` moves those rows; only
//! `LocalGitAdapter::save_*` called it, and the editor does not save
//! through that. This runs it on the editor's path:
//!
//! - **Personal mode: bump, then cascade.** Each primitive the save
//!   changed gets a patch bump of its own version, written into the same
//!   container save, under that save's lock and merge. The cascade then
//!   moves the bound rows to it. **Team / Enterprise:** the primitive
//!   version is the user's to bump (the release / bump-dialog rules), so
//!   nothing is bumped here; the cascade follows whatever version the file
//!   holds, and a released row is left at its old pin and reported.
//! - **After the save, not inside it.** `rewrite_file_locked` has released
//!   the library write lock and committed the primitive file by then. The
//!   cascade takes the lock itself, once per row, and commits each row on
//!   its own, as `LocalGitAdapter::save_*` does. Save and cascade are not
//!   one transaction: a cascade failure leaves the saved primitive in
//!   place, says so, and the next save retries the rows still at the old
//!   version (a row already at the saved version is skipped).
//! - **No double cascade.** The editor never calls `adapter.save_*`.
//! - **One library, found as the save found it.** The adapter is the
//!   mounted one, or else one opened on `enclosing_library_file`, the same
//!   lookup `rewrite_primitive_file` falls back to, so the save and the
//!   cascade never disagree about which library holds the file.
//! - Only a save that wrote cascades, only for primitives the merge
//!   reports as changed, and a Save As never does.

use super::container_save::ContainerWrite;
use crate::app::Signex;
use signex_library::LibraryAdapter;
use signex_library::adapters::local_git::LocalGitAdapter;
use signex_library::{
    CascadeKind, CascadeReport, FootprintFile, LibraryError, PrimitiveSave, RowId, SymbolFile,
    WorkflowMode,
};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

/// Which primitive kind a save wrote.
#[derive(Clone, Copy)]
pub(super) enum SavedKind {
    Symbol,
    Footprint,
}

impl SavedKind {
    fn cascade_kind(self) -> CascadeKind {
        match self {
            Self::Symbol => CascadeKind::Symbol,
            Self::Footprint => CascadeKind::Footprint,
        }
    }
}

/// One primitive this save changed, as the cascade needs it.
pub(super) struct SavedPrimitive {
    pub uuid: Uuid,
    pub name: String,
    pub version: String,
}

/// The library that holds a primitive file being saved. Owned, so it can be
/// resolved and used on the worker thread that does the write.
pub(super) enum SaveLibrary {
    /// The file is in no library.
    None,
    /// A library this app has mounted; its adapter commits the write.
    Mounted(Box<LocalGitAdapter>),
    /// Not mounted here; opened for this save.
    Opened(Box<LocalGitAdapter>),
    /// A library holds the file but it could not be opened.
    Unavailable { library: PathBuf, reason: String },
}

impl SaveLibrary {
    /// The library holding `path`. `mounted` is the `.snxlib` of the open
    /// library whose directory holds it, if any. Reads disk: call it from
    /// the worker, not from `update`.
    pub(super) fn resolve(path: &std::path::Path, mounted: Option<&std::path::Path>) -> Self {
        if let Some(library) = mounted {
            return match LocalGitAdapter::open(library) {
                Ok(adapter) => Self::Mounted(Box::new(adapter)),
                Err(e) => Self::Unavailable {
                    library: library.to_path_buf(),
                    reason: e.to_string(),
                },
            };
        }
        let Some(library) = signex_library::enclosing_library_file(path) else {
            return Self::None;
        };
        match LocalGitAdapter::open(&library) {
            Ok(adapter) => Self::Opened(Box::new(adapter)),
            Err(e) => Self::Unavailable {
                library,
                reason: e.to_string(),
            },
        }
    }

    fn adapter(&self) -> Option<&dyn LibraryAdapter> {
        match self {
            Self::Mounted(a) | Self::Opened(a) => Some(a.as_ref()),
            Self::None | Self::Unavailable { .. } => Option::None,
        }
    }

    /// Whether a save bumps the versions of the primitives it changed:
    /// Personal mode, merging into the editor's own file.
    pub(super) fn bumps_versions(&self, write: ContainerWrite) -> bool {
        write == ContainerWrite::Merge
            && self
                .adapter()
                .is_some_and(|a| a.manifest().workflow.mode == WorkflowMode::Personal)
    }
}

impl Signex {
    /// Bring the clean Component Previews of the rows a cascade moved up to
    /// date: their content changed under them. Dirty ones keep their draft
    /// and are refused as stale on save.
    pub(in crate::app::dispatch::library) fn refresh_editors_after_cascade(
        &mut self,
        path: &std::path::Path,
        rows: &[RowId],
    ) {
        if rows.is_empty() {
            return;
        }
        let root = self
            .library
            .open_libraries
            .iter()
            .find(|lib| lib.root_dir().is_some_and(|d| path.starts_with(d)))
            .map(|lib| lib.root.clone());
        if let Some(root) = root {
            self.refresh_clean_editors(&root, rows);
        }
    }
}

/// Patch-bump the version of each changed symbol; returns the new versions.
pub(super) fn bump_symbol_versions(
    file: &mut SymbolFile,
    changed: &[Uuid],
) -> HashMap<Uuid, String> {
    let mut bumped = HashMap::new();
    for symbol in file
        .symbols
        .iter_mut()
        .filter(|s| changed.contains(&s.uuid))
    {
        symbol.version = signex_library::patch_bump(&symbol.version);
        bumped.insert(symbol.uuid, symbol.version.clone());
    }
    bumped
}

/// The footprint counterpart of [`bump_symbol_versions`].
pub(super) fn bump_footprint_versions(
    file: &mut FootprintFile,
    changed: &[Uuid],
) -> HashMap<Uuid, String> {
    let mut bumped = HashMap::new();
    for fp in file
        .footprints
        .iter_mut()
        .filter(|f| changed.contains(&f.uuid))
    {
        fp.version = signex_library::patch_bump(&fp.version);
        bumped.insert(fp.uuid, fp.version.clone());
    }
    bumped
}

/// Carry the versions the save wrote back into an editor's copy.
pub(super) fn apply_symbol_versions(file: &mut SymbolFile, versions: &HashMap<Uuid, String>) {
    for symbol in &mut file.symbols {
        if let Some(v) = versions.get(&symbol.uuid) {
            symbol.version.clone_from(v);
        }
    }
}

/// The footprint counterpart of [`apply_symbol_versions`].
pub(super) fn apply_footprint_versions(file: &mut FootprintFile, versions: &HashMap<Uuid, String>) {
    for fp in &mut file.footprints {
        if let Some(v) = versions.get(&fp.uuid) {
            fp.version.clone_from(v);
        }
    }
}

/// The symbols of `file` the save changed.
pub(super) fn saved_symbols(file: &SymbolFile, changed: &[Uuid]) -> Vec<SavedPrimitive> {
    file.symbols
        .iter()
        .filter(|s| changed.contains(&s.uuid))
        .map(|s| SavedPrimitive {
            uuid: s.uuid,
            name: s.name.clone(),
            version: s.version.clone(),
        })
        .collect()
}

/// The footprints of `file` the save changed.
pub(super) fn saved_footprints(file: &FootprintFile, changed: &[Uuid]) -> Vec<SavedPrimitive> {
    file.footprints
        .iter()
        .filter(|f| changed.contains(&f.uuid))
        .map(|f| SavedPrimitive {
            uuid: f.uuid,
            name: f.name.clone(),
            version: f.version.clone(),
        })
        .collect()
}

/// Move the component rows that use each saved primitive onto its saved
/// version, and tell the user what happened. `saved` is empty when the
/// save wrote nothing. Returns the rows it moved, so the caller can bring
/// their open Component Previews up to date.
pub(super) fn cascade_after_container_save(
    library: &SaveLibrary,
    write: ContainerWrite,
    kind: SavedKind,
    saved: &[SavedPrimitive],
) -> Vec<RowId> {
    if write != ContainerWrite::Merge || saved.is_empty() {
        return Vec::new();
    }
    let label = match saved {
        [one] => one.name.clone(),
        many => format!("{} saved primitives", many.len()),
    };
    let version = match saved {
        [one] => Some(one.version.as_str()),
        _ => None,
    };
    let mut bumped = Vec::new();
    let lines = match library {
        SaveLibrary::None => return Vec::new(),
        SaveLibrary::Unavailable { library, reason } => vec![(
            Level::Warn,
            format!(
                "{label} saved, but {} could not be opened ({reason}); component rows not updated",
                library.display()
            ),
        )],
        SaveLibrary::Mounted(_) | SaveLibrary::Opened(_) => {
            let Some(adapter) = library.adapter() else {
                return Vec::new();
            };
            let mode = adapter.manifest().workflow.mode;
            let library_id = adapter.manifest().library.library_id;
            let saves: Vec<PrimitiveSave> = saved
                .iter()
                .map(|p| PrimitiveSave {
                    kind: kind.cascade_kind(),
                    library_id,
                    uuid: p.uuid,
                    version: p.version.clone(),
                })
                .collect();
            let result = signex_library::cascade_after_saves(adapter, &saves, mode);
            if let Ok(report) = &result {
                bumped = report.auto_bumped.clone();
            }
            summarize(&label, version, &result)
        }
    };
    for (level, line) in lines {
        match level {
            Level::Info => tracing::info!(target: "signex::library", "{line}"),
            Level::Warn => tracing::warn!(target: "signex::library", "{line}"),
            Level::Error => tracing::error!(target: "signex::library", "{line}"),
        }
    }
    bumped
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Level {
    Info,
    Warn,
    Error,
}

/// "1 component row that uses" / "N component rows that use".
fn rows_that_use(n: usize) -> String {
    match n {
        1 => "1 component row that uses".into(),
        n => format!("{n} component rows that use"),
    }
}

/// What to tell the user about a cascade, reason first: the Messages
/// panel cuts long lines. Nothing for a cascade that touched no row.
fn summarize(
    label: &str,
    version: Option<&str>,
    result: &Result<CascadeReport, LibraryError>,
) -> Vec<(Level, String)> {
    let to = version.map(|v| format!(" to v{v}")).unwrap_or_default();
    let report = match result {
        Ok(report) => report,
        Err(e) => {
            return vec![(
                Level::Error,
                format!("{label} saved, but its component rows could not be read: {e}"),
            )];
        }
    };
    let mut lines = Vec::new();
    if !report.failed.is_empty() {
        let ids: Vec<String> = report.failed.iter().map(|(id, _)| id.to_string()).collect();
        let reason = &report.failed[0].1;
        lines.push((
            Level::Error,
            format!(
                "{label} saved; bumped {} rows, {} not bumped ({reason}): {}",
                report.auto_bumped.len(),
                report.failed.len(),
                ids.join(", "),
            ),
        ));
    } else if !report.auto_bumped.is_empty() {
        lines.push((
            Level::Info,
            format!(
                "Bumped {} {label}{to}",
                rows_that_use(report.auto_bumped.len())
            ),
        ));
    }
    if !report.stale.is_empty() {
        let n = report.stale.len();
        let (noun, verb) = if n == 1 {
            ("row", "uses")
        } else {
            ("rows", "use")
        };
        let behind = version
            .map(|v| format!("below v{v}"))
            .unwrap_or_else(|| "at an older version".into());
        lines.push((
            Level::Info,
            format!("{n} released {noun} {verb} {label} and stay {behind}"),
        ));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_library::RowId;

    fn id() -> RowId {
        RowId::from_uuid(Uuid::now_v7())
    }

    #[test]
    fn a_cascade_that_touched_nothing_says_nothing() {
        let lines = summarize("A", Some("0.0.2"), &Ok(CascadeReport::default()));
        assert!(lines.is_empty());
    }

    #[test]
    fn bumped_rows_are_counted_in_the_singular_and_plural() {
        let one = CascadeReport {
            auto_bumped: vec![id()],
            ..Default::default()
        };
        assert_eq!(
            summarize("A", Some("0.0.2"), &Ok(one))[0].1,
            "Bumped 1 component row that uses A to v0.0.2"
        );
        let two = CascadeReport {
            auto_bumped: vec![id(), id()],
            ..Default::default()
        };
        assert!(
            summarize("A", None, &Ok(two))[0]
                .1
                .starts_with("Bumped 2 component rows")
        );
    }

    #[test]
    fn a_partial_failure_says_the_save_worked_and_names_the_rows() {
        let bad = id();
        let report = CascadeReport {
            auto_bumped: vec![id(), id()],
            failed: vec![(bad, "disk full".into())],
            ..Default::default()
        };
        let lines = summarize("A", Some("0.0.2"), &Ok(report));
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].0, Level::Error);
        assert_eq!(
            lines[0].1,
            format!("A saved; bumped 2 rows, 1 not bumped (disk full): {bad}")
        );
    }

    #[test]
    fn released_rows_left_behind_are_reported() {
        let report = CascadeReport {
            stale: vec![id(), id(), id()],
            ..Default::default()
        };
        assert_eq!(
            summarize("A", Some("0.0.2"), &Ok(report))[0].1,
            "3 released rows use A and stay below v0.0.2"
        );
    }
}
