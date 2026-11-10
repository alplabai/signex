//! The name side of the headless intent checks, judged on the whole project
//! (#2199): a name can reach a net from another sheet (a sheet pin, a
//! hierarchical or global label, a power port), so a one-sheet check would
//! miss a short. The edited sheet is replaced by its scratch copy in the
//! project's sheet set and the stitch before and after is compared by net
//! identity (`signex_net::project_name_changes`). The geometric extra-item
//! check stays per sheet. A loose sheet has no project: its per-sheet check
//! stands. The check is read-only and silent: no Messages line. Rules:
//! `docs/CLI.md`, "Why `allow_merge`".

use signex_net::ProjectNetChange;
use signex_types::schematic::{Point, SchematicSheet};

use crate::app::Signex;
use crate::app::project_sheets::{
    ProjectSheetSet, assemble_project_sheets_quiet, project_graph_quiet, project_pages,
    project_roots, resolve_netlist_root_for, sheet_key_with,
};
use crate::app::state::scope::path_key_with;

impl Signex {
    /// The nets of the project that the edit, which turned the edit target into
    /// `after` at `points`, merges or renames, with whether each needs
    /// `allow_merge` (empty for a loose sheet, and for an edit whose nets reach
    /// no other sheet: nothing there can carry a name from elsewhere). When the
    /// project cannot be read completely and a net of the edit reaches another
    /// sheet, its names are unknown: refused, unless `allow_merge`.
    pub(super) fn project_name_changes(
        &self,
        after: &SchematicSheet,
        points: &[Point],
        allow_merge: bool,
    ) -> Result<Vec<ProjectNetChange>, String> {
        let docs = &self.document_state;
        let Some(path) = docs.edit_path().map(std::path::Path::to_path_buf) else {
            return Ok(Vec::new());
        };
        let Some(project) = docs.document_project(&path) else {
            return Ok(Vec::new());
        };
        // Edits only add: a net or bus run that reaches no other sheet after
        // the edit reached none before, so no project name is in play.
        if !points.iter().any(|p| {
            signex_net::net_reaches_other_sheets(after, *p)
                || signex_net::bus_reaches_other_sheets(after, *p)
        }) {
            return Ok(Vec::new());
        }
        let fold = crate::app::state::project_case::fold_of(Some(project));
        let root = resolve_netlist_root_for(docs, &path);
        let pages = project_pages(Some(project));
        let assemble = || assemble_project_sheets_quiet(docs, &pages, &root, fold);
        // Inside an edit list the other sheets are read once.
        let mut cache = docs.agent_edits.project_set.borrow_mut();
        let mut own;
        let set = if docs.agent_edits.listing {
            cache.0.get_or_insert_with(assemble)
        } else {
            own = assemble();
            &mut own
        };
        let complete = set.unreadable.is_empty()
            && set.absent.is_empty()
            && set.refused.is_empty()
            && set.sheets.contains_key(&root);
        if !complete {
            return if allow_merge {
                Ok(Vec::new())
            } else {
                Err(format!(
                    "the project cannot be read completely ({}), so the names on other sheets are unknown and this net reaches them; fix the project, or pass allow_merge=true to place it anyway (see docs/CLI.md, \"Why allow_merge\")",
                    unreadable_reason(set)
                ))
            };
        }
        let Some(current) = docs.edit_engine().map(|e| e.document().clone()) else {
            return Ok(Vec::new());
        };
        // The walk keys a sheet by the spelling it first met (`Child.snxsch`
        // for a tab opened as `child.snxsch`): find it as the walk dedupes.
        let wanted = path_key_with(&path, fold);
        let Some(spelled) = set
            .sheets
            .keys()
            .find(|p| path_key_with(p, fold) == wanted)
            .cloned()
        else {
            return if allow_merge {
                Ok(Vec::new())
            } else {
                Err("this sheet is not among the sheets the project reads, so the names on its net are unknown; pass allow_merge=true to place it anyway (see docs/CLI.md, \"Why allow_merge\")".to_string())
            };
        };
        let base = project.dir().to_path_buf();
        set.sheets.insert(spelled.clone(), current);
        let before = project_graph_quiet(&set.sheets, Some(&base), Some(&root), fold);
        set.sheets.insert(spelled, after.clone());
        let after_graph = project_graph_quiet(&set.sheets, Some(&base), Some(&root), fold);
        let roots = project_roots(
            sheet_key_with(&root, Some(&base), fold),
            set,
            &before,
            Some(&base),
            fold,
        );
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

/// The first change that needs `allow_merge`.
pub(super) fn needing_merge(changes: &[ProjectNetChange]) -> Option<&ProjectNetChange> {
    changes.iter().find(|change| change.needs_allow_merge)
}

/// The names the changed nets now carry only through a child's hierarchical
/// label, sorted.
pub(super) fn aliases_of(changes: &[ProjectNetChange]) -> Vec<String> {
    let aliases: std::collections::BTreeSet<&String> =
        changes.iter().flat_map(|change| &change.aliases).collect();
    aliases.into_iter().cloned().collect()
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
fn unreadable_reason(set: &ProjectSheetSet) -> String {
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
    parts.extend(set.refused.iter().map(|child| {
        format!(
            "{} has a sheet reference that leaves its folder",
            name(&child.parent)
        )
    }));
    if parts.is_empty() {
        "the root sheet is not part of the project".to_string()
    } else {
        parts.join("; ")
    }
}
