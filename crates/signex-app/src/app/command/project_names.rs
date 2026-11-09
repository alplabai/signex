//! The name side of the headless intent checks, judged on the whole project
//! (#2199): a name can reach a net from another sheet (a sheet pin, a
//! hierarchical or global label, a power port), so a one-sheet check would
//! miss a short. The edited sheet is replaced by its scratch copy in the
//! project's sheet set and the stitch before and after is compared
//! (`signex_net::project_name_changes`). The geometric extra-item check stays
//! per sheet. A loose sheet has no project: its per-sheet check stands.
//! Rules: `docs/CLI.md`, "Why `allow_merge`".

use signex_types::schematic::{Point, SchematicSheet};

use crate::app::Signex;
use crate::app::project_sheets::{
    assemble_project_sheets_from, open_sheets_snapshot, project_graph_with, project_pages,
    project_roots, resolve_netlist_root_for, sheet_key_with,
};

impl Signex {
    /// The sets of names that a net of the project carries after the edit that
    /// turned the edit target into `after`, and none carried before; empty
    /// for a loose sheet. `at` is a point of the edit, for the unreadable case:
    /// when the rest of the project cannot be read and the net there reaches
    /// another sheet, its names are unknown and the edit is refused.
    pub(super) fn project_name_changes(
        &self,
        after: &SchematicSheet,
        at: Point,
    ) -> Result<Vec<Vec<String>>, String> {
        let docs = &self.document_state;
        let Some(path) = docs.edit_path().map(std::path::Path::to_path_buf) else {
            return Ok(Vec::new());
        };
        let Some(project) = docs.document_project(&path) else {
            return Ok(Vec::new());
        };
        let fold = crate::app::state::project_case::fold_of(Some(project));
        let root = resolve_netlist_root_for(docs, &path);
        let set = assemble_project_sheets_from(
            &open_sheets_snapshot(docs),
            &project_pages(Some(project)),
            &root,
            fold,
        );
        // A sheet the project's netlist does not reach has no names to share.
        if !set.sheets.contains_key(&path) {
            return Ok(Vec::new());
        }
        let base = project.dir().to_path_buf();
        let before = project_graph_with(&set.sheets, Some(&base), Some(&root), fold);
        let root_key = sheet_key_with(&root, Some(&base), fold);
        let complete = set.unreadable.is_empty()
            && set.absent.is_empty()
            && before.sheets.contains_key(&root_key);
        if !complete {
            return if signex_net::net_reaches_other_sheets(after, at) {
                Err(format!(
                    "the project cannot be read completely ({}), so the names on other sheets are unknown and this net reaches them; fix the project or place it where no sheet pin, hierarchical or global label, or power port is",
                    unreadable_reason(&set)
                ))
            } else {
                Ok(Vec::new())
            };
        }
        let mut edited = set.sheets.clone();
        edited.insert(path, after.clone());
        let after_graph = project_graph_with(&edited, Some(&base), Some(&root), fold);
        let roots = project_roots(root_key, &set, &before, Some(&base), fold);
        let project_before = signex_net::ProjectGraph {
            sheets: &before.sheets,
            resolved: &before.resolved,
            roots: &roots,
        };
        let project_after = signex_net::ProjectGraph {
            sheets: &after_graph.sheets,
            resolved: &after_graph.resolved,
            roots: &roots,
        };
        Ok(signex_net::project_name_changes(
            &project_before,
            &project_after,
        ))
    }
}

/// The refusal for an edit that gives a net of the project names it did not
/// carry together: a name from another sheet counts.
pub(super) fn project_change_refusal(what: &str, at: Point, names: &[String]) -> String {
    format!(
        "a {what} at {},{} would give a net of the project the names {{{}}}, some of which reach it from other sheets; pass allow_merge=true to merge on purpose (see docs/CLI.md, \"Why allow_merge\")",
        at.x,
        at.y,
        names.join(", ")
    )
}

/// What kept the project from being read in full, for a refusal.
fn unreadable_reason(set: &crate::app::project_sheets::ProjectSheetSet) -> String {
    // File names only: an answer carries no path.
    let name = |path: &std::path::Path| {
        path.file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
    };
    let mut parts: Vec<String> = set
        .unreadable
        .iter()
        .map(|(path, _)| format!("{} is unreadable", name(path)))
        .collect();
    parts.extend(
        set.absent
            .iter()
            .map(|path| format!("{} is missing", name(path))),
    );
    if parts.is_empty() {
        "the root sheet is not part of the project".to_string()
    } else {
        parts.join("; ")
    }
}
