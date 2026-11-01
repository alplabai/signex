//! Who else depends on a sheet file the Projects tree can remove or rename
//! (#1618).
//!
//! With the hierarchy in the tree a sheet row is no longer just a `.snxprj`
//! entry: it can be placed by symbols on other sheets, in this project or in
//! another open project, and a nested row may have no entry at all. These
//! checks read the hierarchies the project-tree probe already walked and the
//! sheet symbols of the open documents, so they cost no disk access.
//!
//! A deleted file cannot be brought back, so incomplete data refuses: when a
//! hierarchy is missing, still being walked, or holds a sheet that could not
//! be read, "nobody else places it" is unknown, not true.

use std::path::{Path, PathBuf};

use super::*;
use crate::app::path_ops::PlacingSymbol;
use crate::app::project_sheets::{lexically_normalize, resolve_child_reference};
use crate::app::sheet_hierarchy::{SheetHierarchy, SheetNode, SheetNodeKind};
use crate::app::state::scope::{CaseFold, path_key_with};
use crate::panels::SheetInfo;

/// Why a Remove cannot offer every choice for a sheet row.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct RemoveBlockers {
    /// A schematic with no `.snxprj` entry: there is nothing to exclude.
    pub(crate) unlisted: bool,
    /// Sheets, other than the row's own parent, whose symbols place the file.
    pub(crate) other_parents: Vec<PathBuf>,
    /// The row's own parent, when it places the file: after a delete its
    /// symbol shows the sheet as missing.
    pub(crate) own_parent: Option<PathBuf>,
    /// The sheet symbols that place the file (every one when delete is not
    /// blocked, since then only the row's own parent places it).
    pub(crate) symbols: Vec<PlacingSymbol>,
    /// Some sheet places the file, so excluding it from the project does not
    /// take it out of the design.
    pub(crate) placed: bool,
    /// Why "nobody else places it" cannot be told, when it cannot.
    pub(crate) unknown: Option<String>,
}

impl RemoveBlockers {
    /// Names for a Messages line or the dialog.
    pub(crate) fn parent_names(&self) -> String {
        names(&self.other_parents)
    }

    /// Delete is withheld: another sheet places the file, or that is unknown.
    /// A directory (a `.snxlib` package) can never be placed as a sheet, so
    /// incomplete data does not hold it back.
    pub(crate) fn blocks_delete(&self, is_dir: bool) -> bool {
        !self.other_parents.is_empty() || (self.unknown.is_some() && !is_dir)
    }
}

fn names(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Every sheet that places a file, and whether that list can be trusted.
#[derive(Debug, Default)]
pub(crate) struct SheetUsers {
    /// Sorted, no repeats.
    pub(crate) parents: Vec<PathBuf>,
    /// The symbols on those sheets that place the file, sorted, one per
    /// symbol.
    pub(crate) symbols: Vec<PlacingSymbol>,
    /// Set when some project or document could not be ruled out.
    pub(crate) unknown: Option<String>,
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
        // Keyed by the file and the project's case rules, never by its
        // extension: a symbol can name any file, and "nobody places it" must
        // not depend on how the name is spelled.
        let own_parent = (state.tree_path.len() >= 4)
            .then(|| self.tree_path_to_file_path(&state.tree_path[..state.tree_path.len() - 1]))
            .flatten();
        let users = self.sheet_users(&state.target_path, None);
        let fold = self.dialog_fold(state);
        let is_own = |p: &Path| {
            own_parent.as_deref().is_some_and(|own| {
                path_key_with(&lexically_normalize(own), fold)
                    == path_key_with(&lexically_normalize(p), fold)
            })
        };
        let (own, other): (Vec<PathBuf>, Vec<PathBuf>) =
            users.parents.iter().cloned().partition(|p| is_own(p));
        RemoveBlockers {
            unlisted: self
                .sheet_info_at(&state.tree_path)
                .is_some_and(|row| row.is_child_reference && !row.in_project_list),
            other_parents: other,
            own_parent: own.into_iter().next(),
            placed: !users.parents.is_empty(),
            symbols: users.symbols,
            unknown: users.unknown,
        }
    }

    /// Every sheet whose symbols place `target`: in the walked hierarchy of
    /// each open project, and in the open documents themselves (unsaved
    /// symbols, sheets outside any project).
    ///
    /// `skip_project` leaves one project out, for a rename that already
    /// re-points its own project's parents.
    pub(crate) fn sheet_users(
        &self,
        target: &Path,
        skip_project: Option<crate::app::state::ProjectId>,
    ) -> SheetUsers {
        let target = lexically_normalize(target);
        let mut users = SheetUsers::default();
        self.collect_walked_users(&target, skip_project, &mut users);
        self.collect_open_users(&target, skip_project, &mut users);
        users.parents.sort();
        users.parents.dedup();
        users.symbols.sort();
        users
            .symbols
            .dedup_by(|a, b| a.parent == b.parent && a.id == b.id);
        users
    }

    fn collect_walked_users(
        &self,
        target: &Path,
        skip_project: Option<crate::app::state::ProjectId>,
        users: &mut SheetUsers,
    ) {
        let cache = &self.document_state.project_fs;
        for project in &self.document_state.projects {
            if skip_project == Some(project.id) {
                continue;
            }
            let Some(current) = cache.hierarchy_current(project.id) else {
                continue; // no sheets to walk
            };
            let hierarchy = cache.facts.hierarchy_of(project.id);
            if hierarchy.is_none() || !current {
                note_unknown(
                    users,
                    format!(
                        "the sheets of project {} are still being read",
                        project.data.name
                    ),
                );
            }
            let Some(hierarchy) = hierarchy else { continue };
            // The sheet being deleted may itself be the broken one.
            if let Some(bad) = incomplete_node(hierarchy, target) {
                note_unknown(
                    users,
                    format!(
                        "{} in project {} could not be read completely; fix or remove that \
                         sheet first",
                        bad.display(),
                        project.data.name
                    ),
                );
            }
            for top in SheetHierarchy::top_level(hierarchy) {
                collect_parents(top, target, hierarchy.fold, users);
            }
        }
    }

    fn collect_open_users(
        &self,
        target: &Path,
        skip_project: Option<crate::app::state::ProjectId>,
        users: &mut SheetUsers,
    ) {
        for (path, engine) in &self.document_state.engines {
            let owner = self.document_state.document_project(path);
            if skip_project.is_some() && owner.map(|p| p.id) == skip_project {
                continue;
            }
            let fold = crate::app::state::project_case::fold_of(owner);
            let dir = path.parent().unwrap_or_else(|| Path::new(""));
            let wanted = path_key_with(target, fold);
            let parent = lexically_normalize(path);
            let mut places = false;
            for c in &engine.document().child_sheets {
                let hit = resolve_child_reference(dir, &c.filename)
                    .is_some_and(|p| path_key_with(&p, fold) == wanted);
                if hit {
                    places = true;
                    users.symbols.push(PlacingSymbol {
                        parent: parent.clone(),
                        name: c.name.clone(),
                        id: c.uuid,
                    });
                }
            }
            if places {
                users.parents.push(parent);
            }
        }
    }
}

fn note_unknown(users: &mut SheetUsers, reason: String) {
    users.unknown.get_or_insert(reason);
}

/// A node whose children are unknown (unreadable, or cut off by the budget),
/// other than the `target` itself.
fn incomplete_node<'a>(hierarchy: &'a SheetHierarchy, target: &Path) -> Option<&'a Path> {
    fn find<'a>(node: &'a SheetNode, target: &Path, fold: CaseFold) -> Option<&'a Path> {
        let incomplete = matches!(
            node.kind,
            SheetNodeKind::Unreadable | SheetNodeKind::Truncated
        );
        if incomplete && !node.is_at(target, fold) {
            return Some(&node.path);
        }
        node.children.iter().find_map(|c| find(c, target, fold))
    }
    let fold = hierarchy.fold;
    SheetHierarchy::top_level(hierarchy).find_map(|top| find(top, target, fold))
}

fn collect_parents(node: &SheetNode, target: &Path, fold: CaseFold, users: &mut SheetUsers) {
    let mut places_target = false;
    for c in &node.children {
        if c.kind == SheetNodeKind::Rejected || !c.is_at(target, fold) {
            continue;
        }
        places_target = true;
        users.symbols.push(PlacingSymbol {
            parent: node.path.clone(),
            name: c.name.clone(),
            id: c.placed_as.unwrap_or_default(),
        });
    }
    if places_target {
        users.parents.push(node.path.clone());
    }
    for child in &node.children {
        collect_parents(child, target, fold, users);
    }
}
