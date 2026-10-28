//! Who else depends on a sheet file the Projects tree can remove or rename
//! (#1618).
//!
//! With the hierarchy in the tree a sheet row is no longer just a `.snxprj`
//! entry: it can be placed by symbols on other sheets, in this project or in
//! another open project, and a nested row may have no entry at all. These
//! checks read the hierarchies the project-tree probe already walked, so they
//! cost no disk access.

use std::path::{Path, PathBuf};

use super::*;
use crate::app::project_sheets::lexically_normalize;
use crate::app::sheet_hierarchy::{SheetHierarchy, SheetNode, SheetNodeKind};
use crate::app::state::library_entry::entry_resolves_to;
use crate::app::state::scope::path_key_with;
use crate::panels::SheetInfo;

/// Why a Remove cannot offer every choice for a sheet row.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct RemoveBlockers {
    /// A schematic with no `.snxprj` entry: there is nothing to exclude.
    pub(crate) unlisted: bool,
    /// Sheets, other than the row's own parent, whose symbols place the file.
    pub(crate) other_parents: Vec<PathBuf>,
}

impl RemoveBlockers {
    /// Names for a Messages line or the dialog.
    pub(crate) fn parent_names(&self) -> String {
        self.other_parents
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl Signex {
    /// The row at `tree_path` in the Source Documents branch.
    pub(super) fn sheet_info_at(&self, tree_path: &[usize]) -> Option<&SheetInfo> {
        let (&project, rest) = tree_path.split_first()?;
        let (&docs, rest) = rest.split_first()?;
        let branch = signex_widgets::tree_view::get_node(
            self.document_state.panel_ctx.project_tree.as_slice(),
            &[project, docs],
        )?;
        if branch.label != "Source Documents" {
            return None;
        }
        let (&page, nested) = rest.split_first()?;
        let mut info = self
            .document_state
            .panel_ctx
            .projects
            .get(project)?
            .sheets
            .get(page)?;
        for &index in nested {
            info = info.children.get(index)?;
        }
        Some(info)
    }

    /// What stands in the way of removing the sheet a Remove dialog names.
    pub(crate) fn remove_blockers(&self, state: &RemoveDialogState) -> RemoveBlockers {
        let is_sheet = state.target_path.extension().and_then(|e| e.to_str()) == Some("snxsch");
        if !is_sheet {
            return RemoveBlockers::default();
        }
        let own_parent = (state.tree_path.len() >= 4)
            .then(|| self.tree_path_to_file_path(&state.tree_path[..state.tree_path.len() - 1]))
            .flatten();
        RemoveBlockers {
            unlisted: !self.sheet_is_listed(state),
            other_parents: self.other_sheet_parents(
                &state.target_path,
                own_parent.as_deref(),
                None,
            ),
        }
    }

    /// Is the dialog's target a `.snxprj` page or the declared root?
    fn sheet_is_listed(&self, state: &RemoveDialogState) -> bool {
        let Some(project) = state
            .project_id
            .and_then(|id| self.document_state.project_by_id(id))
        else {
            return true;
        };
        let (dir, fold) = (project.dir(), project.case_fold);
        let is_target = |entry: &str| entry_resolves_to(dir, entry, &state.target_path, fold);
        project.data.sheets.iter().any(|e| is_target(&e.filename))
            || project
                .data
                .schematic_root
                .as_deref()
                .is_some_and(is_target)
    }

    /// Every sheet, in any open project, whose symbols place `target`, except
    /// `own_parent` (the one the clicked row hangs under). Sorted, no repeats.
    ///
    /// `skip_project` leaves one project out, for a rename that already
    /// re-points its own project's parents.
    pub(crate) fn other_sheet_parents(
        &self,
        target: &Path,
        own_parent: Option<&Path>,
        skip_project: Option<crate::app::state::ProjectId>,
    ) -> Vec<PathBuf> {
        let facts = &self.document_state.project_fs.facts;
        let target = lexically_normalize(target);
        let mut parents: Vec<PathBuf> = Vec::new();
        for project in &self.document_state.projects {
            if skip_project == Some(project.id) {
                continue;
            }
            let Some(hierarchy) = facts.hierarchy_of(project.id) else {
                continue;
            };
            let fold = hierarchy.fold;
            let skip = own_parent.map(|p| path_key_with(&lexically_normalize(p), fold));
            for top in SheetHierarchy::top_level(hierarchy) {
                collect_parents(top, &target, fold, skip.as_deref(), &mut parents);
            }
        }
        parents.sort();
        parents.dedup();
        parents
    }
}

fn collect_parents(
    node: &SheetNode,
    target: &Path,
    fold: crate::app::state::scope::CaseFold,
    skip: Option<&str>,
    out: &mut Vec<PathBuf>,
) {
    let places_target = node
        .children
        .iter()
        .any(|c| c.kind != SheetNodeKind::Rejected && c.is_at(target, fold));
    let parent_key = path_key_with(&node.path, fold);
    if places_target && skip != Some(parent_key.as_str()) {
        out.push(node.path.clone());
    }
    for child in &node.children {
        collect_parents(child, target, fold, skip, out);
    }
}
