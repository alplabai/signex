//! What the Remove dialog says about a library's placed parts, and the
//! Messages-panel line for what a library delete left on disk.

use std::path::{Path, PathBuf};

use super::*;

/// Parts placed from a library on the sheets that are open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LibraryUsage {
    pub placed_parts: usize,
    /// Project sheets without an open engine: their parts were not counted.
    pub unchecked_sheets: usize,
}

impl Signex {
    /// Placed parts of the library at `library` on open sheets, or `None`
    /// when it is not mounted (nothing can be matched to it).
    pub(crate) fn library_usage(
        &self,
        library: &Path,
        tree_path: &[usize],
    ) -> Option<LibraryUsage> {
        let library_id = self.library.library_at(library)?.library_id;
        // Only the owning project's sheets: that is the project being changed.
        let owner = tree_path
            .first()
            .and_then(|i| self.document_state.projects.get(*i));
        let placed_parts = owner.map_or(0, |p| {
            p.data
                .sheets
                .iter()
                .filter_map(|s| self.document_state.engines.get(&p.dir().join(&s.filename)))
                .map(|engine| {
                    engine
                        .document()
                        .symbols
                        .iter()
                        .filter(|s| s.library_id == Some(library_id))
                        .count()
                })
                .sum()
        });
        let unchecked_sheets = tree_path
            .first()
            .and_then(|i| self.document_state.projects.get(*i))
            .map_or(0, |p| {
                p.data
                    .sheets
                    .iter()
                    .filter(|s| {
                        !self
                            .document_state
                            .engines
                            .contains_key(&p.dir().join(&s.filename))
                    })
                    .count()
            });
        Some(LibraryUsage {
            placed_parts,
            unchecked_sheets,
        })
    }

    /// The caption line for a library target, empty for anything else.
    pub(crate) fn library_usage_line(
        &self,
        state: &crate::app::RemoveDialogState,
    ) -> Option<String> {
        let usage = self.library_usage(&state.target_path, &state.tree_path)?;
        Some(usage_text(&usage))
    }
}

pub(crate) fn usage_text(usage: &LibraryUsage) -> String {
    let mut line = match usage.placed_parts {
        0 => "No parts on the open sheets are placed from this library.".to_string(),
        1 => "1 part on the open sheets is placed from this library; it keeps its data but \
              can no longer be updated from it."
            .to_string(),
        n => format!(
            "{n} parts on the open sheets are placed from this library; they keep their data \
             but can no longer be updated from it."
        ),
    };
    if usage.unchecked_sheets > 0 {
        line.push_str(&format!(
            " {} sheet(s) that are not open were not checked.",
            usage.unchecked_sheets
        ));
    }
    line
}

/// Say in the Messages panel what a library delete left behind.
pub(super) fn report_library_leftovers(name: &str, left: &[PathBuf]) {
    let shown: Vec<String> = left
        .iter()
        .take(5)
        .map(|p| p.display().to_string())
        .collect();
    let more = left.len().saturating_sub(shown.len());
    let tail = if more > 0 {
        format!(" and {more} more")
    } else {
        String::new()
    };
    crate::diagnostics::log_warning(format!(
        "Deleted library {name}; {} file(s) or folder(s) were left on disk: {}{tail}",
        left.len(),
        shown.join(", ")
    ));
}
