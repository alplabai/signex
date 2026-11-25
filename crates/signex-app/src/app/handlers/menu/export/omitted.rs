//! The pages a netlist export leaves out, one row each, for the
//! netlist-incomplete prompt (#431). Built from the same [`ExportIssues`] as
//! [`ExportIssues::messages`], so the list and the warning logged on "Export
//! anyway" name the same pages.

use std::path::Path;

use super::ExportIssues;
use crate::app::state::{OmittedPage, OmittedReason};

impl ExportIssues {
    /// One row per page left out of the netlist, in a stable order: the
    /// unreadable sheets, the declared pages with no file, then the holes
    /// the stitcher found (a missing child, a cycle, a key collision). The
    /// other stitch issues name a complete netlist and are not listed (see
    /// [`ExportIssues::netlist_is_incomplete`]).
    ///
    /// A child that exists but could not be read is also a `MissingChild` to
    /// the stitcher; it is listed once, as unreadable, so the prompt never
    /// calls a file that is there "missing". Repeats (one parent placed
    /// twice) are listed once.
    pub(crate) fn omitted_pages(&self) -> Vec<OmittedPage> {
        let mut rows: Vec<OmittedPage> = Vec::new();
        for (path, _) in &self.unreadable {
            let cause = self.unreadable_causes.get(path).copied();
            rows.push(OmittedPage {
                file: file_name(path),
                reason: OmittedReason::Unreadable { cause },
            });
        }
        for path in &self.missing_pages {
            rows.push(OmittedPage {
                file: file_name(path),
                reason: OmittedReason::FileMissing { placed_by: None },
            });
        }
        rows.extend(
            self.stitch
                .iter()
                .filter_map(|issue| self.stitch_row(issue)),
        );
        let mut seen = std::collections::HashSet::new();
        rows.retain(|row| seen.insert(row.clone()));
        rows
    }

    /// The row a stitch issue adds, `None` for one that leaves the netlist
    /// whole or a missing child already listed as unreadable.
    fn stitch_row(&self, issue: &signex_net::StitchIssue) -> Option<OmittedPage> {
        use signex_net::StitchIssue as I;
        match issue {
            I::MissingChild {
                sheet_name,
                filename,
                ..
            } => {
                if self.is_unreadable(filename) {
                    return None;
                }
                let placed_by = Some(sheet_name.trim())
                    .filter(|name| !name.is_empty())
                    .map(str::to_owned);
                Some(OmittedPage {
                    file: filename.clone(),
                    reason: OmittedReason::FileMissing { placed_by },
                })
            }
            I::SheetCycle {
                parent_path,
                filename,
            } => Some(OmittedPage {
                file: filename.clone(),
                reason: OmittedReason::SheetCycle {
                    parent: parent_path.clone(),
                },
            }),
            // `kept` stays whole: two collisions with the same file names in
            // different folders are two rows, and a `..`-spelled pair still
            // reads as two paths.
            I::SheetKeyCollision { kept, dropped, .. } => Some(OmittedPage {
                file: file_name(Path::new(dropped)),
                reason: OmittedReason::KeyCollision { kept: kept.clone() },
            }),
            _ => None,
        }
    }

    /// Whether the child reference `filename` names a sheet that exists but
    /// could not be read.
    fn is_unreadable(&self, filename: &str) -> bool {
        let reference = Path::new(filename.trim());
        reference.file_name().is_some()
            && self
                .unreadable
                .iter()
                .any(|(path, _)| path.ends_with(reference))
    }
}

/// The file name of `path`, or the whole path when it has none.
fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}
