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

    /// Open the Create Sheet dialog (#1275) for the symbol on `missing.parent`
    /// that names `missing.child`.
    ///
    /// The row was marked missing by an earlier file-status check, so the file
    /// is looked for again first: one that has appeared since is never offered
    /// for creation. The dialog's own create path refuses an existing file
    /// too, and writes without overwriting.
    pub(super) fn offer_create_child_sheet(&mut self, missing: &MissingChildSheet) {
        if missing.child.exists() {
            crate::diagnostics::log_warning(format!(
                "Sheet {} already exists, so it was not offered for creation; it opens normally                  once the tree refreshes",
                missing.child.display()
            ));
            self.document_state.project_fs.invalidate();
            return;
        }
        // The dialog acts on the focused sheet's document, as from the canvas.
        let parent_focused = self.active_tab_path().as_deref() == Some(missing.parent.as_path())
            || self.open_or_focus_sheet_path(missing.parent.clone());
        let symbol = parent_focused
            .then(|| self.symbol_placing(&missing.child))
            .flatten();
        match symbol {
            Some(symbol) => self.open_create_child_sheet_dialog(symbol, true),
            None => crate::diagnostics::log_warning(format!(
                "Sheet {} cannot be created from the tree: {} could not be opened or no longer                  has a sheet symbol for it",
                missing.child.display(),
                missing.parent.display()
            )),
        }
    }

    /// The sheet symbol on the focused sheet whose file is `child`.
    fn symbol_placing(&self, child: &std::path::Path) -> Option<uuid::Uuid> {
        let engine = self.document_state.active_engine()?;
        let parent = self.active_tab_path()?;
        let dir = parent.parent()?;
        let wanted = crate::app::project_sheets::lexically_normalize(child);
        engine
            .document()
            .child_sheets
            .iter()
            .find(|c| {
                crate::app::project_sheets::resolve_child_reference(dir, &c.filename)
                    .is_some_and(|p| p == wanted)
            })
            .map(|c| c.uuid)
    }
}
