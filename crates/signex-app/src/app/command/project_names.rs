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
    ProjectSheetSet, project_graph_quiet, project_roots, resolve_netlist_root_for, sheet_key_with,
};
use crate::app::state::scope::path_key_with;

impl Signex {
    /// Whether the project check judges the names of the edit that turned the
    /// edit target into `after` at `points`: the sheet is in a project and a
    /// net or bus run there reaches another sheet. Then its verdict is the
    /// only name verdict; the per-sheet one, blind to the other sheets and to
    /// names hidden behind a port, decides only otherwise (#2199).
    pub(super) fn project_judges_names(&self, after: &SchematicSheet, points: &[Point]) -> bool {
        let docs = &self.document_state;
        let in_project = docs
            .edit_path()
            .is_some_and(|path| docs.document_project(path).is_some());
        // Edits only add: a net or bus run that reaches no other sheet after
        // the edit reached none before, so no project name is in play.
        in_project
            && points.iter().any(|p| {
                signex_net::net_reaches_other_sheets(after, *p)
                    || signex_net::bus_reaches_other_sheets(after, *p)
            })
    }

    /// The nets of the project that the edit, which turned the edit target into
    /// `after` at `points`, merges or renames, with whether each needs
    /// `allow_merge`; `None` when the project did not judge (a loose sheet, an
    /// edit whose nets reach no other sheet, or an incomplete project passed
    /// with `allow_merge`). When the project cannot be read completely and a
    /// net of the edit reaches another sheet, its names are unknown: refused,
    /// unless `allow_merge`.
    pub(super) fn project_name_changes(
        &self,
        after: &SchematicSheet,
        points: &[Point],
        allow_merge: bool,
    ) -> Result<Option<Vec<ProjectNetChange>>, String> {
        if !self.project_judges_names(after, points) {
            return Ok(None);
        }
        self.project_name_changes_of(after, allow_merge)
    }

    /// [`Self::project_name_changes`] for any edit of the target, wherever on
    /// the sheet: a sheet symbol brings its file's nets in (#2199).
    pub(super) fn project_name_changes_of(
        &self,
        after: &SchematicSheet,
        allow_merge: bool,
    ) -> Result<Option<Vec<ProjectNetChange>>, String> {
        let docs = &self.document_state;
        let Some(path) = docs.edit_path().map(std::path::Path::to_path_buf) else {
            return Ok(None);
        };
        let Some(project) = docs.document_project(&path) else {
            return Ok(None);
        };
        let fold = crate::app::state::project_case::fold_of(Some(project));
        let root = resolve_netlist_root_for(docs, &path);
        // The closed sheets were read off the UI thread before the edit ran.
        let mut own = self.edit_project_set(&path)?;
        let set = &mut own;
        let complete = set.unreadable.is_empty()
            && set.absent.is_empty()
            && set.refused.is_empty()
            && set.sheets.contains_key(&root);
        if !complete {
            return if allow_merge {
                Ok(None)
            } else {
                Err(format!(
                    "the project cannot be read completely ({}), so the names on other sheets are unknown and this net reaches them; fix the project, or pass allow_merge=true to place it anyway (see docs/CLI.md, \"Why allow_merge\")",
                    unreadable_reason(set)
                ))
            };
        }
        let Some(current) = docs.edit_engine().map(|e| e.document().clone()) else {
            return Ok(None);
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
                Ok(None)
            } else {
                Err("this sheet is not among the sheets the project reads, so the names on its net are unknown; pass allow_merge=true to place it anyway (see docs/CLI.md, \"Why allow_merge\")".to_string())
            };
        };
        let base = project.dir().to_path_buf();
        // A child's names behind its port stay names on the edited sheet.
        let edited = sheet_key_with(&spelled, Some(&base), fold);
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
        Ok(Some(signex_net::project_name_changes(
            &project_before,
            &project_after,
            &edited,
        )))
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
pub(super) fn unreadable_reason(set: &ProjectSheetSet) -> String {
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
