//! A sheet symbol whose child file does not exist, shown in the Projects tree
//! as a `<file>  (missing)` row under its parent (#1618).
//!
//! Activating the row is the Create Sheet entry point (#1275), the same one
//! the Navigator's missing rows take. The sheet is never read: there is no
//! file, so the usual open path would only report a read error.

use super::*;

/// The sheet symbol behind a missing nested row.
pub(super) struct MissingChildSheet {
    /// The sheet carrying the symbol.
    pub(super) parent: std::path::PathBuf,
    /// Where the symbol's file should be.
    pub(super) child: std::path::PathBuf,
}

impl Signex {
    /// The missing child the row at `tree_path` stands for, or `None` for any
    /// other row — including a *listed* page whose file is gone, which keeps
    /// reporting through the normal open.
    pub(super) fn missing_child_sheet_at(&self, tree_path: &[usize]) -> Option<MissingChildSheet> {
        let row = self.sheet_info_at(tree_path)?;
        if !(row.is_child_reference && row.missing) {
            return None;
        }
        let parent_path = tree_path.split_last()?.1;
        Some(MissingChildSheet {
            parent: self.tree_path_to_file_path(parent_path)?,
            child: self.tree_path_to_file_path(tree_path)?,
        })
    }

    /// Say why Remove or Rename does nothing on a missing symbol row: there is
    /// no file to act on, and the symbol belongs to its parent sheet.
    /// A `.snxprj` page can still be excluded (`listed_ok`).
    pub(super) fn refuse_on_missing_child(
        &self,
        tree_path: &[usize],
        action: &str,
        listed_ok: bool,
    ) -> bool {
        let refused = self.missing_child_sheet_at(tree_path).is_some()
            && !(listed_ok
                && self
                    .sheet_info_at(tree_path)
                    .is_some_and(|r| r.in_project_list));
        if refused {
            crate::diagnostics::log_warning(format!(
                "{action} cancelled: the sheet does not exist yet. Create it from this row, \
                 or change the sheet symbol on its parent."
            ));
        }
        refused
    }

    /// Offer to create the child sheet a symbol on `missing.parent` names.
    ///
    /// The Create Sheet dialog (#1275) is not on this branch yet; until it
    /// lands this says why nothing opens instead of failing in a read error.
    pub(super) fn offer_create_child_sheet(&mut self, missing: &MissingChildSheet) {
        crate::diagnostics::log_warning(format!(
            "Sheet {} is referenced by {} but does not exist; creating it needs the Create Sheet dialog",
            missing.child.display(),
            missing.parent.display()
        ));
    }
}
