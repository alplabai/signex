//! What a sheet-symbol file brings into the sheet it is placed on (#2199,
//! #1276): the facts the rules in `signex_engine::check_sheet_file` judge (the
//! sheets around the edit as a graph, the sheets above it, a link out of the
//! project folder) and the names it brings in. Everything is judged on what
//! the edit read before it ran (`edit_reads`); sheets are matched by their
//! folded key. The GUI's Create Sheet Symbol From Sheet and the operand forms
//! share this path. Rules: `docs/CLI.md`, "Placing parts and hierarchy".

use std::collections::HashSet;
use std::path::Path;

use signex_engine::{Command, SheetFilePlacement, SheetGraph, SheetGraphNode, SubtreeFacts};
use signex_types::schematic::{Point, SchematicSheet};

use crate::app::Signex;
use crate::app::project_sheets::{project_pages, resolve_child_reference_quiet};
use crate::app::sheet_hierarchy::{CaseFold, normalised_key};

fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

/// The graph node of `sheet`, read from `path`.
fn read_node(path: &Path, sheet: &SchematicSheet, fold: CaseFold, in_project: bool) -> SheetGraphNode {
    let dir = path.parent().unwrap_or_else(|| Path::new(""));
    let children = sheet
        .child_sheets
        .iter()
        .filter_map(|child| resolve_child_reference_quiet(dir, &child.filename).ok())
        .map(|next| normalised_key(&next, fold))
        .collect();
    SheetGraphNode::Read {
        name: file_name(path),
        children,
        reaching_names: signex_engine::carries_reaching_name(sheet),
        in_project,
    }
}

impl Signex {
    /// The sheets around an edit of `sheet`: the project as read (or for a
    /// loose sheet the subtree as read), then the open sheets in their live
    /// state. A sheet the read found unreadable counts as such unless open.
    fn sheet_graph(&self, sheet: &Path, fold: CaseFold) -> Result<SheetGraph, String> {
        let docs = &self.document_state;
        let owned = match docs.document_project(sheet) {
            Some(_) => Some(self.edit_project_set(sheet)?),
            None => None,
        };
        let in_project = owned.is_some();
        let set = owned.as_ref().or_else(|| self.prefetched_subtree());
        let mut graph = SheetGraph::default();
        let mut project_keys = HashSet::new();
        if let Some(set) = set {
            for (path, content) in &set.sheets {
                let key = normalised_key(path, fold);
                if in_project {
                    project_keys.insert(key.clone());
                }
                graph.insert(key, read_node(path, content, fold, in_project));
            }
            for (path, why) in &set.unreadable {
                let node = SheetGraphNode::Unreadable {
                    name: file_name(path),
                    why: why.clone(),
                };
                graph.insert(normalised_key(path, fold), node);
            }
        }
        for (path, engine) in &docs.engines {
            let key = normalised_key(path, fold);
            let node = read_node(path, engine.document(), fold, project_keys.contains(&key));
            graph.insert(key, node);
        }
        Ok(graph)
    }

    /// The walk down `file`'s subtree from the sheet an edit acts on.
    pub(super) fn file_subtree(
        &self,
        file: &Path,
        sheet: &Path,
        fold: CaseFold,
    ) -> Result<SubtreeFacts, String> {
        let graph = self.sheet_graph(sheet, fold)?;
        let (file, edited) = (normalised_key(file, fold), normalised_key(sheet, fold));
        Ok(signex_engine::subtree_facts(&graph, &file, &edited))
    }

    /// The file rules of a sheet symbol placing `path` on the sheet an edit
    /// acts on (`signex_engine::check_sheet_file`): the project folder, the
    /// sheet itself, one above it, a loop at any depth and, unless
    /// `allow_merge`, an unreadable subtree and a file placed already.
    pub(super) fn check_sheet_symbol_file(&self, path: &Path, allow_merge: bool) -> Result<(), String> {
        let escapes_project = self.prefetched_escape(path)?;
        let docs = &self.document_state;
        let sheet = docs.edit_path().ok_or("no schematic is open")?;
        let project = docs.document_project(sheet);
        let fold = crate::app::state::project_case::fold_of(project);
        let (file, edited) = (normalised_key(path, fold), normalised_key(sheet, fold));
        let above = self.sheets_above_edited(sheet, fold)?;
        let graph = self.sheet_graph(sheet, fold)?;
        let pages: Option<HashSet<String>> = project.map(|project| {
            project_pages(Some(project))
                .iter()
                .map(|page| normalised_key(page, fold))
                .collect()
        });
        let placement = SheetFilePlacement {
            edited: &edited,
            file: &file,
            escapes_project,
            above: &above,
            graph: &graph,
            project_pages: pages.as_ref(),
            allow_merge,
        };
        signex_engine::check_sheet_file(&placement)
            .map(|_| ())
            .map_err(|refusal| refusal.to_string())
    }

    /// The name rule (#2199 comment 6761836527) for the nets a sheet symbol's
    /// file brings into the project: a short or a rename of a project net is
    /// refused unless `allow_merge=true`. A file not there yet brings none. In
    /// a project that cannot be read completely the names on other sheets are
    /// unknown, so a file whose subtree carries a name that reaches other
    /// sheets is refused; one that carries none is placed. A loose sheet is
    /// not judged for names (`docs/CLI.md`).
    pub(super) fn refuse_child_names(
        &self,
        command: &Command,
        path: &Path,
        at: Point,
    ) -> Result<(), String> {
        let docs = &self.document_state;
        let sheet = docs.edit_path().ok_or("no schematic is open")?;
        if docs.document_project(sheet).is_none() {
            return Ok(());
        }
        let set = self.edit_project_set(sheet)?;
        let fold = set.fold;
        let key = normalised_key(path, fold);
        if set.absent.iter().any(|p| normalised_key(p, fold) == key) {
            return Ok(());
        }
        let root = crate::app::project_sheets::resolve_netlist_root_for(docs, sheet);
        let complete = set.unreadable.is_empty()
            && set.absent.is_empty()
            && set.refused.is_empty()
            && set.sheets.contains_key(&root);
        if !complete {
            if !self.file_subtree(path, sheet, fold)?.names {
                return Ok(());
            }
            return Err(format!(
                "the project cannot be read completely ({}), so the names on other sheets are unknown and the file's names reach them; fix the project, or pass allow_merge=true to place it anyway (see docs/CLI.md, \"Why allow_merge\")",
                super::project_names::unreadable_reason(&set)
            ));
        }
        let engine = docs.edit_engine().ok_or("no schematic is open")?;
        let after = Self::sheet_after(engine.document(), std::slice::from_ref(command))?;
        let changes = self
            .project_name_changes_of(&after, false)?
            .unwrap_or_default();
        let Some(change) = super::project_names::needing_merge(&changes) else {
            return Ok(());
        };
        Err(super::project_names::project_change_refusal(
            "sheet symbol",
            at,
            &change.names,
        ))
    }
}
