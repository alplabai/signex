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
use crate::app::project_sheets::{lexically_normalize, project_pages, project_root_sheet_path};
use crate::app::sheet_hierarchy::{SheetHierarchy, SheetNode, SheetNodeKind};
use crate::app::state::LoadedProject;
use crate::app::state::project_fs::ProjectFsFacts;
use crate::app::state::project_hierarchy::{HierarchyJob, HierarchyRequest};
use crate::app::state::scope::{CaseFold, path_is_under, path_key_with};
use crate::panels::SheetInfo;

/// What marks a row: open tab, unsaved edits, the focused tab.
pub(super) struct RowMarks<'a> {
    pub(super) open: &'a HashSet<PathBuf>,
    pub(super) dirty: &'a HashSet<PathBuf>,
    pub(super) active: Option<&'a PathBuf>,
}

impl Signex {
    /// The walk the Projects tree wants: one job per project, rooted at its
    /// declared root or, without one, its first listed page, carrying what
    /// the open sheets under the project reference now.
    pub(super) fn hierarchy_request(&self) -> HierarchyRequest {
        let document = &self.document_state;
        let jobs = document
            .projects
            .iter()
            .filter_map(|p| {
                let pages = project_pages(Some(p));
                let root = project_root_sheet_path(p).or_else(|| pages.first().cloned())?;
                let mut open_refs: Vec<_> = document
                    .engines
                    .iter()
                    .filter(|(path, _)| {
                        path_is_under(path, p.dir(), p.case_fold)
                            || pages.iter().any(|page| {
                                path_key_with(page, p.case_fold) == path_key_with(path, p.case_fold)
                            })
                    })
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
                Some(HierarchyJob {
                    id: p.id,
                    root,
                    pages,
                    fold: p.case_fold,
                    open_refs,
                })
            })
            .collect();
        HierarchyRequest { jobs }
    }

    /// The files the last walks placed, to be stat-ed so a sheet that appears
    /// or goes is noticed (and shown) without waiting for another walk.
    pub(super) fn hierarchy_exists_paths(&self) -> Vec<PathBuf> {
        let facts = &self.document_state.project_fs.facts;
        self.document_state
            .projects
            .iter()
            .filter_map(|p| facts.hierarchy_of(p.id))
            .flat_map(SheetHierarchy::node_paths)
            .map(Path::to_path_buf)
            .collect()
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
    let fold = project.case_fold;
    let hierarchy = facts.hierarchy_of(project.id);
    let key_of = |path: &Path| path_key_with(&lexically_normalize(path), fold);
    let listed: Vec<(&signex_types::project::SheetEntry, String)> = project
        .data
        .sheets
        .iter()
        .map(|entry| (entry, key_of(&dir.join(&entry.filename))))
        .collect();
    let ctx = RowCtx {
        dir,
        facts,
        marks,
        fold,
        listed: listed.iter().map(|(_, key)| key.clone()).collect(),
    };

    let mut top_children = nest_children(&listed, hierarchy, &ctx);
    listed
        .iter()
        .enumerate()
        .filter_map(|(i, (entry, _))| {
            let children = top_children.remove(&i)?;
            let abs = dir.join(&entry.filename);
            let mut info = row(&entry.name, &entry.filename, &abs, marks);
            info.in_project_list = true;
            info.sym_count = entry.symbols_count;
            info.wire_count = entry.wires_count;
            info.label_count = entry.labels_count;
            info.missing = facts.is_missing(&abs);
            info.children = children;
            Some(info)
        })
        .collect()
}

/// What every row of one project is built against.
struct RowCtx<'a> {
    dir: &'a Path,
    facts: &'a ProjectFsFacts,
    marks: &'a RowMarks<'a>,
    fold: CaseFold,
    /// Keys of the `.snxprj` pages.
    listed: HashSet<String>,
}

impl RowCtx<'_> {
    fn key(&self, path: &Path) -> String {
        path_key_with(&lexically_normalize(path), self.fold)
    }
}

/// For each listed page that stays a top-level row (by index), the rows nested
/// under it. A page placed by another sheet is nested there and has no entry.
///
/// Subtrees are claimed in this order: the project root, pages no symbol
/// references, then whatever is left (a loop of loose pages), so a file lands
/// under its real parent and not under an unrelated loose page.
fn nest_children(
    listed: &[(&signex_types::project::SheetEntry, String)],
    hierarchy: Option<&SheetHierarchy>,
    ctx: &RowCtx<'_>,
) -> HashMap<usize, Vec<SheetInfo>> {
    let tops: HashMap<String, &SheetNode> = hierarchy
        .into_iter()
        .flat_map(SheetHierarchy::top_level)
        .map(|node| (ctx.key(&node.path), node))
        .collect();
    let referenced: HashSet<String> = hierarchy
        .into_iter()
        .flat_map(SheetHierarchy::top_level)
        .flat_map(|top| top.children.iter())
        .flat_map(|child| placed_keys(child, ctx))
        .collect();
    let root_key = hierarchy
        .and_then(|h| h.root.as_ref())
        .map(|node| ctx.key(&node.path));
    let priority = |key: &str| -> u8 {
        if root_key.as_deref() == Some(key) {
            0
        } else if !referenced.contains(key) {
            1
        } else {
            2
        }
    };
    let mut order: Vec<usize> = (0..listed.len()).collect();
    order.sort_by_key(|&i| priority(&listed[i].1));

    let mut placed: HashSet<String> = HashSet::new();
    let mut top_children: HashMap<usize, Vec<SheetInfo>> = HashMap::new();
    for i in order {
        let key = &listed[i].1;
        if placed.insert(key.clone()) {
            let rows = tops
                .get(key)
                .map(|node| child_rows(node, ctx, &mut placed))
                .unwrap_or_default();
            top_children.insert(i, rows);
        } else if top_children
            .keys()
            .any(|&earlier| listed[earlier].1 == *key)
        {
            // The `.snxprj` lists the file twice: both entries stay rows.
            top_children.insert(i, Vec::new());
        }
    }
    top_children
}

/// Keys of every file a subtree places (cycles and refused references excluded).
fn placed_keys(node: &SheetNode, ctx: &RowCtx<'_>) -> Vec<String> {
    if matches!(node.kind, SheetNodeKind::Cycle | SheetNodeKind::Rejected) {
        return Vec::new();
    }
    let mut keys = vec![ctx.key(&node.path)];
    for child in &node.children {
        keys.extend(placed_keys(child, ctx));
    }
    keys
}

/// The rows under `node`, depth first, each file the first time it is met.
fn child_rows(node: &SheetNode, ctx: &RowCtx<'_>, placed: &mut HashSet<String>) -> Vec<SheetInfo> {
    let mut rows = Vec::new();
    for child in &node.children {
        if matches!(child.kind, SheetNodeKind::Cycle | SheetNodeKind::Rejected) {
            continue;
        }
        let key = ctx.key(&child.path);
        if !placed.insert(key.clone()) {
            continue;
        }
        let relative = relative_to(&child.path, ctx.dir).unwrap_or_else(|| child.filename.clone());
        let mut info = row(&child.name, &relative, &child.path, ctx.marks);
        info.in_project_list = ctx.listed.contains(&key);
        info.is_child_reference = true;
        info.unreadable = child.kind == SheetNodeKind::Unreadable;
        // A symbol whose file the walk found absent is missing until a newer
        // file-status check sees it. A file the walk read stays unmarked: one
        // that is open but not saved yet is not a sheet to create, and one
        // deleted since sends its project back through the walk.
        info.missing =
            child.kind == SheetNodeKind::Missing && !ctx.facts.knows_present(&child.path);
        info.children = child_rows(child, ctx, placed);
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
        unreadable: false,
        is_child_reference: false,
        in_project_list: false,
        children: Vec::new(),
    }
}
