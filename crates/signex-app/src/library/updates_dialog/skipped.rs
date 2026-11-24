//! The library versions Skip All dismissed, per sheet (mockup row 135).
//! Cancel closes the modal and forgets nothing, so the next scan asks
//! again; Skip All remembers each part's offered version, and a later scan
//! of that sheet leaves the part out until its row moves to another version.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::{LibraryUpdateEntry, LibraryUpdatesState};

/// Per sheet, the `(lib_id, offered version)` pairs Skip All dismissed.
/// Session state: it lives in the Model, not in the sheet file.
#[derive(Debug, Clone, Default)]
pub struct SkippedVersions(HashMap<PathBuf, HashSet<(String, String)>>);

impl SkippedVersions {
    /// Remember every version the modal `state` offered for its sheet.
    pub fn remember(&mut self, state: &LibraryUpdatesState) {
        let skipped = self.0.entry(state.schematic_path.clone()).or_default();
        skipped.extend(
            state
                .entries
                .iter()
                .map(|e| (e.lib_id.clone(), e.latest_version.clone())),
        );
    }

    /// Whether Skip All dismissed `entry`'s offered version on `sheet`.
    pub fn is_skipped(&self, sheet: &Path, entry: &LibraryUpdateEntry) -> bool {
        self.0.get(sheet).is_some_and(|skipped| {
            skipped.iter().any(|(lib_id, version)| {
                *lib_id == entry.lib_id && *version == entry.latest_version
            })
        })
    }

    /// Move each sheet's record to the path `rebase` gives it (a folder
    /// rename moved the sheets).
    pub fn rebase(&mut self, rebase: impl Fn(&Path) -> PathBuf) {
        let old = std::mem::take(&mut self.0);
        for (sheet, versions) in old {
            self.0.entry(rebase(&sheet)).or_default().extend(versions);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::updates_dialog::BumpKind;
    use signex_library::RowId;
    use uuid::Uuid;

    fn entry(lib_id: &str, latest: &str) -> LibraryUpdateEntry {
        LibraryUpdateEntry {
            symbol_uuid: Uuid::now_v7(),
            ref_des: "R1".to_string(),
            internal_pn: String::new(),
            lib_id: lib_id.to_string(),
            library_id: Uuid::nil(),
            library_name: "Lib".to_string(),
            row_id: RowId::from_uuid(Uuid::nil()),
            library_path: PathBuf::from("/tmp/x.snxlib"),
            current_version: "1.0.0".to_string(),
            latest_version: latest.to_string(),
            bump_kind: BumpKind::Patch,
            selected: true,
            conflicts: Vec::new(),
        }
    }

    /// Mockup row 135 (accepted 2026-11-24): Skip All remembers the skipped
    /// version on its own sheet only, and a newer version is offered again.
    #[test]
    fn skip_all_remembers_the_offered_version_per_sheet() {
        let sheet = PathBuf::from("/p/a.snxsch");
        let other = PathBuf::from("/p/b.snxsch");
        let mut skipped = SkippedVersions::default();
        skipped.remember(&LibraryUpdatesState::new(
            sheet.clone(),
            vec![entry("L:R", "1.0.1")],
        ));

        assert!(skipped.is_skipped(&sheet, &entry("L:R", "1.0.1")));
        assert!(
            !skipped.is_skipped(&sheet, &entry("L:R", "1.0.2")),
            "a newer version is asked again"
        );
        assert!(
            !skipped.is_skipped(&sheet, &entry("L:C", "1.0.1")),
            "another part is not skipped"
        );
        assert!(
            !skipped.is_skipped(&other, &entry("L:R", "1.0.1")),
            "another sheet is not skipped"
        );
    }

    /// A folder rename carries the record to the sheet's new path.
    #[test]
    fn a_rebased_sheet_keeps_its_skipped_versions() {
        let old = PathBuf::from("/p/a.snxsch");
        let new = PathBuf::from("/q/a.snxsch");
        let mut skipped = SkippedVersions::default();
        skipped.remember(&LibraryUpdatesState::new(
            old.clone(),
            vec![entry("L:R", "1.0.1")],
        ));

        skipped.rebase(|p| {
            if p == old {
                new.clone()
            } else {
                p.to_path_buf()
            }
        });

        assert!(skipped.is_skipped(&new, &entry("L:R", "1.0.1")));
        assert!(!skipped.is_skipped(&old, &entry("L:R", "1.0.1")));
    }
}
