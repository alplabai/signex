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

/// Source Documents rows sit at `[project, Source Documents, page]`; a nested
/// child is one level deeper at least.
const FIRST_NESTED_DEPTH: usize = 4;

impl Signex {
    /// The missing child the row at `tree_path` stands for, or `None` for any
    /// other row — including a *listed* page whose file is gone, which keeps
    /// reporting through the normal open.
    pub(super) fn missing_child_sheet_at(&self, tree_path: &[usize]) -> Option<MissingChildSheet> {
        if tree_path.len() < FIRST_NESTED_DEPTH {
            return None;
        }
        let node = signex_widgets::tree_view::get_node(
            self.document_state.panel_ctx.project_tree.as_slice(),
            tree_path,
        )?;
        let is_missing_sheet = node.label.ends_with("  (missing)")
            && matches!(node.icon, signex_widgets::tree_view::TreeIcon::SnxSchematic);
        if !is_missing_sheet {
            return None;
        }
        let parent_path = &tree_path[..tree_path.len() - 1];
        Some(MissingChildSheet {
            parent: self.tree_path_to_file_path(parent_path)?,
            child: self.tree_path_to_file_path(tree_path)?,
        })
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
