//! The sheet rows of the Projects tree (#1618): the `.snxprj` page list with
//! each child sheet nested under the parent whose sheet symbol references it.
//!
//! The list itself is untouched — netlist order, numbering, print order and
//! the BOM keep reading `ProjectData::sheets`. Only the panel's rows are
//! arranged, from the hierarchy the project-tree probe walked
//! ([`crate::app::state::project_hierarchy`]).
//!
//! The tree is about files, so a sheet appears once: under the first parent
//! (document order) that references it. Repeated placements, and a reference
//! back to an ancestor, stay in the Navigator, which is the per-instance view.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::super::*;
use crate::app::project_sheets::lexically_normalize;
use crate::app::sheet_hierarchy::{SheetHierarchy, SheetNode, SheetNodeKind};
use crate::app::state::LoadedProject;
use crate::app::state::project_fs::ProjectFsFacts;
use crate::app::state::project_hierarchy::{HierarchyJob, HierarchyRequest};
use crate::app::state::scope::{CaseFold, path_key_with};
use crate::panels::SheetInfo;

/// What marks a row: open tab, unsaved edits, the focused tab.
pub(super) struct RowMarks<'a> {
    pub(super) open: &'a HashSet<PathBuf>,
    pub(super) dirty: &'a HashSet<PathBuf>,
    pub(super) active: Option<&'a PathBuf>,
}

impl Signex {
    /// The walk the Projects tree wants: every project that declares a root,
    /// and what each open sheet references now.
    pub(super) fn hierarchy_request(&self) -> HierarchyRequest {
        let document = &self.document_state;
        let jobs = document
            .projects
            .iter()
            .filter_map(|p| {
                Some(HierarchyJob {
                    id: p.id,
                    root: crate::app::project_sheets::project_root_sheet_path(p)?,
                    pages: crate::app::project_sheets::project_pages(Some(p)),
                    fold: p.case_fold,
                })
            })
            .collect();
        let mut open_refs: Vec<_> = document
            .engines
            .iter()
            .map(|(path, engine)| {
                let refs = engine
                    .document()
                    .child_sheets
                    .iter()
                    .map(|c| (c.name.clone(), c.filename.clone(), c.uuid))
                    .collect();
                (path.clone(), refs)
            })
            .collect();
        open_refs.sort_by(|a, b| a.0.cmp(&b.0));
        HierarchyRequest { jobs, open_refs }
    }
}

/// The Source Documents rows of `project`, in `.snxprj` order, with child
/// sheets nested under their parents when a hierarchy is known.
pub(super) fn project_sheet_rows(
    project: &LoadedProject,
    facts: &ProjectFsFacts,
    marks: &RowMarks<'_>,
) -> Vec<SheetInfo> {
    let dir = project.dir();
    let hierarchy = facts.hierarchy_of(project.id);
    let fold = project.case_fold;
    let key_of = |path: &Path| path_key_with(&lexically_normalize(path), fold);

    let tops: HashMap<String, &SheetNode> = hierarchy
        .into_iter()
        .flat_map(SheetHierarchy::top_level)
        .map(|node| (key_of(&node.path), node))
        .collect();
    let listed: Vec<(&signex_types::project::SheetEntry, String)> = project
        .data
        .sheets
        .iter()
        .map(|entry| (entry, key_of(&dir.join(&entry.filename))))
        .collect();

    // Which listed page roots a subtree: the project root first, then pages
    // no symbol references, then whatever is left (a loop of loose pages).
    let referenced: HashSet<String> = hierarchy
        .into_iter()
        .flat_map(SheetHierarchy::top_level)
        .flat_map(|top| top.children.iter())
        .flat_map(|child| placed_keys(child, &key_of))
        .collect();
    let root_key = hierarchy
        .and_then(|h| h.root.as_ref())
        .map(|node| key_of(&node.path));
    let by_priority = |key: &str| -> u8 {
        if root_key.as_deref() == Some(key) {
            0
        } else if !referenced.contains(key) {
            1
        } else {
            2
        }
    };
    let mut order: Vec<usize> = (0..listed.len()).collect();
    order.sort_by_key(|&i| by_priority(&listed[i].1));

    let mut placed: HashSet<String> = HashSet::new();
    let mut top_children: HashMap<usize, Vec<SheetInfo>> = HashMap::new();
    for i in order {
        let key = &listed[i].1;
        if placed.insert(key.clone()) {
            let rows = tops
                .get(key)
                .map(|node| child_rows(node, dir, fold, marks, &mut placed))
                .unwrap_or_default();
            top_children.insert(i, rows);
        } else if top_keys_listed_twice(&top_children, &listed, key) {
            // The `.snxprj` lists the file twice: both entries stay rows.
            top_children.insert(i, Vec::new());
        }
    }

    listed
        .iter()
        .enumerate()
        .filter_map(|(i, (entry, _))| {
            let children = top_children.remove(&i)?;
            let mut info = row(
                &entry.name,
                &entry.filename,
                &dir.join(&entry.filename),
                marks,
            );
            info.children = children;
            Some(info)
        })
        .collect()
}

/// Whether an earlier entry with `key` is itself a top-level row.
fn top_keys_listed_twice(
    top_children: &HashMap<usize, Vec<SheetInfo>>,
    listed: &[(&signex_types::project::SheetEntry, String)],
    key: &str,
) -> bool {
    top_children.keys().any(|&i| listed[i].1 == key)
}

/// Keys of every file a subtree places (cycles and refused references excluded).
fn placed_keys<'a>(node: &'a SheetNode, key_of: &'a dyn Fn(&Path) -> String) -> Vec<String> {
    if matches!(node.kind, SheetNodeKind::Cycle | SheetNodeKind::Rejected) {
        return Vec::new();
    }
    let mut keys = vec![key_of(&node.path)];
    for child in &node.children {
        keys.extend(placed_keys(child, key_of));
    }
    keys
}

/// The rows under `node`, depth first, each file the first time it is met.
fn child_rows(
    node: &SheetNode,
    dir: &Path,
    fold: CaseFold,
    marks: &RowMarks<'_>,
    placed: &mut HashSet<String>,
) -> Vec<SheetInfo> {
    let mut rows = Vec::new();
    for child in &node.children {
        if matches!(child.kind, SheetNodeKind::Cycle | SheetNodeKind::Rejected) {
            continue;
        }
        let key = path_key_with(&lexically_normalize(&child.path), fold);
        if !placed.insert(key) {
            continue;
        }
        let relative = relative_to(&child.path, dir).unwrap_or_else(|| child.filename.clone());
        let mut info = row(&child.name, &relative, &child.path, marks);
        info.missing = child.kind == SheetNodeKind::Missing;
        info.is_child_reference = true;
        info.children = child_rows(child, dir, fold, marks, placed);
        rows.push(info);
    }
    rows
}

/// `path` as the `/`-separated name a `.snxprj` entry would carry.
fn relative_to(path: &Path, dir: &Path) -> Option<String> {
    let relative = path.strip_prefix(lexically_normalize(dir)).ok()?;
    let parts: Vec<_> = relative
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    (!parts.is_empty()).then(|| parts.join("/"))
}

fn row(name: &str, filename: &str, abs: &Path, marks: &RowMarks<'_>) -> SheetInfo {
    SheetInfo {
        name: name.to_string(),
        filename: filename.to_string(),
        sym_count: 0,
        wire_count: 0,
        label_count: 0,
        is_open: marks.open.contains(abs),
        is_dirty: marks.dirty.contains(abs),
        is_active: marks.active.is_some_and(|active| active == abs),
        missing: false,
        is_child_reference: false,
        children: Vec::new(),
    }
}
