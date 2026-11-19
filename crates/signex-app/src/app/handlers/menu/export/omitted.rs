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
        for (path, why) in &self.unreadable {
            rows.push(OmittedPage {
                file: file_name(path),
                reason: OmittedReason::Unreadable(why.clone()),
            });
        }
        for path in &self.missing_pages {
            rows.push(OmittedPage {
                file: file_name(path),
                reason: OmittedReason::FileMissing { placed_by: None },
            });
        }
        for issue in &self.stitch {
            let row = match issue {
                signex_net::StitchIssue::MissingChild {
                    sheet_name,
                    filename,
                    ..
                } => {
                    if self.is_unreadable(filename) {
                        continue;
                    }
                    let placed_by = Some(sheet_name.trim())
                        .filter(|name| !name.is_empty())
                        .map(str::to_owned);
                    OmittedPage {
                        file: filename.clone(),
                        reason: OmittedReason::FileMissing { placed_by },
                    }
                }
                signex_net::StitchIssue::SheetCycle { filename, .. } => OmittedPage {
                    file: filename.clone(),
                    reason: stitch_reason(issue),
                },
                signex_net::StitchIssue::SheetKeyCollision { dropped, .. } => OmittedPage {
                    file: file_name(Path::new(dropped)),
                    reason: stitch_reason(issue),
                },
                _ => continue,
            };
            rows.push(row);
        }
        let mut seen = std::collections::HashSet::new();
        rows.retain(|row| seen.insert(row.clone()));
        rows
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

/// The stitcher's English line for `issue`, without the "Netlist: " lead the
/// Messages panel needs and the list does not.
fn stitch_reason(issue: &signex_net::StitchIssue) -> OmittedReason {
    let line = crate::app::project_sheets::stitch_issue_message(issue);
    let reason = line.strip_prefix("Netlist: ").unwrap_or(&line).to_owned();
    OmittedReason::Stitch(reason)
}

/// The file name of `path`, or the whole path when it has none.
fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}
