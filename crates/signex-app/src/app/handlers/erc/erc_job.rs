//! The ERC run as a pure, blocking job: everything that reads the disk or
//! walks the project graph, with no borrow of the live `Signex` state, so
//! `handle_run_erc` can hand it to `spawn_blocking` instead of running it on
//! the UI thread (#111).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::app::project_sheets::{self, OpenSheets};
use crate::schematic_runtime::SchematicRenderSnapshot;

type ViolationsByPath = HashMap<PathBuf, Vec<signex_erc::Violation>>;

/// Everything the job needs, captured from `update` as plain owned data.
pub(super) struct ErcRunInputs {
    /// Snapshot of every open engine's document.
    pub open: OpenSheets,
    /// Open tabs that have an engine.
    pub tab_paths: Vec<PathBuf>,
    /// The active tab's render snapshot, which wins over its engine's.
    pub active: Option<(PathBuf, SchematicRenderSnapshot)>,
    /// Declared pages of the owning project.
    pub pages: Vec<PathBuf>,
    /// The netlist root; `None` when there is no active document.
    pub root: Option<PathBuf>,
    pub project_dir: Option<PathBuf>,
    pub overrides: HashMap<signex_erc::RuleKind, signex_erc::Severity>,
    pub config: signex_erc::ErcConfig,
    /// The owning project's key rules, chosen once by whoever picked the project.
    pub fold: crate::app::state::scope::CaseFold,
}

/// What the job found. Messages are replayed to the Messages panel by the
/// completion handler (the job itself never logs).
#[derive(Debug)]
pub struct ErcRunOutcome {
    pub by_path: ViolationsByPath,
    pub warnings: Vec<String>,
    pub infos: Vec<String>,
}

pub(super) fn run_erc_job(inputs: ErcRunInputs) -> ErcRunOutcome {
    let mut warnings = Vec::new();
    let mut infos = Vec::new();
    let dsl_eval_fns =
        load_project_dsl_eval_fns(inputs.project_dir.as_deref(), &mut infos, &mut warnings);

    // Live snapshots for open tabs; the active tab's render snapshot wins.
    let mut snapshots: HashMap<PathBuf, SchematicRenderSnapshot> = inputs
        .tab_paths
        .iter()
        .filter_map(|p| inputs.open.get(p).map(|s| (p.clone(), s.clone())))
        .collect();
    if let Some((path, snapshot)) = inputs.active {
        snapshots.insert(path, snapshot);
    }

    // The project itself, from the one assembler the export and the cached
    // canvas netlist also use (#406).
    let project_set = match &inputs.root {
        Some(root) => project_sheets::assemble_project_sheets_from(
            &inputs.open,
            &inputs.pages,
            root,
            inputs.fold,
        ),
        None => project_sheets::ProjectSheetSet::empty(),
    };
    for (path, why) in &project_set.unreadable {
        warnings.push(format!("ERC: sheet {} {why}", path.display()));
    }
    let base_dir = inputs.project_dir.clone().or_else(|| {
        project_set
            .root
            .as_ref()
            .and_then(|r| r.parent().map(PathBuf::from))
    });
    // Built exactly as the export builds it, so a key collision keeps the
    // same sheet. ERC still reports on every sheet, reached or not.
    let fold = project_set.fold;
    let graph = project_sheets::project_graph_with(
        &project_set.sheets,
        base_dir.as_deref(),
        project_set.root.as_deref(),
        fold,
    );
    warnings.extend(
        graph
            .issues
            .iter()
            .map(project_sheets::stitch_issue_message),
    );

    // The export's own roots (the netlist root plus the declared pages its
    // hierarchy never reaches), so "the netlist uses 'X'" is what the export
    // writes. No root, or one that did not assemble: no project netlist.
    let roots = project_set
        .root
        .as_ref()
        .map(|r| project_sheets::sheet_key_with(r, base_dir.as_deref(), fold))
        .filter(|key| graph.sheets.contains_key(key))
        .map(|key| {
            project_sheets::project_roots(key, &project_set, &graph, base_dir.as_deref(), fold)
        })
        .unwrap_or_default();

    // Loose open tabs are on screen, so their violations are wanted too.
    for (path, sheet) in project_set.sheets {
        snapshots.entry(path).or_insert(sheet);
    }

    let mut by_path: ViolationsByPath = HashMap::new();
    // One memo for the whole run: a sheet reached from several parents (or
    // from several sheets' runs) is projected once (#998).
    let mut hierarchy_memo = signex_erc::HierarchyMemo::new();
    for (path, snapshot) in &snapshots {
        let key = project_sheets::sheet_key_with(path, base_dir.as_deref(), fold);
        let raw = signex_erc::run_with_project_memoised(
            snapshot,
            Some(&key),
            &graph.resolved,
            &graph.sheets,
            dsl_eval_fns.as_deref().unwrap_or(&[]),
            &inputs.config,
            &mut hierarchy_memo,
        );
        by_path.insert(
            path.clone(),
            signex_erc::apply_overrides(raw, &inputs.overrides),
        );
    }

    add_project_net_names(
        &mut by_path,
        &graph,
        &roots,
        base_dir.as_deref(),
        fold,
        &inputs.overrides,
    );
    refile_child_sheet_violations(&mut by_path);
    dedupe_descendant_clashes(&mut by_path);

    let total: usize = by_path.values().map(Vec::len).sum();
    infos.push(format!(
        "ERC: {total} total violations across {} sheet(s)",
        by_path.len()
    ));
    ErcRunOutcome {
        by_path,
        warnings,
        infos,
    }
}

/// The project-wide `NetMultipleNames` pass (#1061): names a net gets on other
/// sheets are only visible in the stitched netlist. A sheet's own per-sheet
/// finding is dropped only where an emitted project finding covers it (a
/// sheet the roots never reach, or a part of a net the project finding leaves
/// out, keeps its own); each project finding is filed under the path the
/// graph read its sheet from, and the lists are re-sorted.
fn add_project_net_names(
    by_path: &mut ViolationsByPath,
    graph: &project_sheets::AssembledGraph,
    roots: &[signex_net::ProjectRoot],
    base_dir: Option<&Path>,
    fold: crate::app::state::scope::CaseFold,
    overrides: &HashMap<signex_erc::RuleKind, signex_erc::Severity>,
) {
    if roots.is_empty() {
        return;
    }
    let project = signex_net::ProjectGraph {
        sheets: &graph.sheets,
        resolved: &graph.resolved,
        roots,
    };
    let found = signex_erc::project_net_multiple_names(&project);
    for (path, list) in by_path.iter_mut() {
        let key = project_sheets::sheet_key_with(path, base_dir, fold);
        // A path that lost a key collision is not the sheet the graph holds.
        if graph.paths.get(&key) != Some(path) {
            continue;
        }
        list.retain(|v| {
            v.builtin_kind() != Some(signex_erc::RuleKind::NetMultipleNames)
                || !found
                    .covered
                    .contains(&(key.clone(), signex_net::pt_key(&v.location)))
        });
    }
    let mut touched: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
    for (key, violation) in found.findings {
        let Some(path) = graph.paths.get(&key) else {
            continue;
        };
        by_path.entry(path.clone()).or_default().push(violation);
        touched.insert(path.clone());
    }
    // Overrides are idempotent; re-applying sorts the appended findings in.
    for path in touched {
        if let Some(list) = by_path.remove(&path) {
            by_path.insert(path, signex_erc::apply_overrides(list, overrides));
        }
    }
}

/// Move every child-space violation (e.g. `BadHierSheetPin` on a child's
/// hierarchical label) from the parent's list to the child sheet that owns the
/// coordinate, so its marker and panel entry land on the right canvas (#956).
/// A child that was not part of the run stays under the parent as a
/// non-locatable entry: it is reported, never drawn at a foreign coordinate.
fn refile_child_sheet_violations(by_path: &mut ViolationsByPath) {
    let ran: std::collections::HashSet<std::path::PathBuf> = by_path.keys().cloned().collect();
    let mut moved: Vec<(std::path::PathBuf, signex_erc::Violation)> = Vec::new();
    for (parent, list) in by_path.iter_mut() {
        let dir = parent.parent().unwrap_or_else(|| std::path::Path::new(""));
        let (child_space, kept): (Vec<_>, Vec<_>) = std::mem::take(list)
            .into_iter()
            .partition(|v| v.location_space == signex_erc::LocationSpace::ChildSheet);
        *list = kept;
        for v in child_space {
            let owner = v
                .child_sheet
                .as_deref()
                .and_then(|name| crate::app::project_sheets::resolve_child_reference(dir, name))
                .filter(|p| p != parent && ran.contains(p));
            match owner {
                Some(child) => moved.push((child, v.into_child_sheet_space())),
                None => list.push(v),
            }
        }
    }
    for (child, v) in moved {
        by_path.entry(child).or_default().push(v);
    }
}

/// A duplicate-designator clash between descendant sheets is found by each
/// sheet that sees both sides (the rule already stops ancestors above the
/// lowest one). When a sub-sheet is reused under several parents, those
/// parents still all report the identical finding; keep it once per ERC run,
/// under the first sheet path in sort order so the result is stable (#998).
/// Identity is the owning sheet's resolved key, not the display text: two
/// copied channels with identical file names and positions are distinct
/// sheets and each keep their own finding.
fn dedupe_descendant_clashes(by_path: &mut ViolationsByPath) {
    let is_clash = |v: &signex_erc::Violation| {
        v.rule == signex_erc::RuleKind::DuplicateRefDesignator
            && v.location_space == signex_erc::LocationSpace::ChildSheet
    };
    let mut paths: Vec<std::path::PathBuf> = by_path.keys().cloned().collect();
    paths.sort();
    let key_of = |v: &signex_erc::Violation| {
        (
            v.message.clone(),
            v.location.x.to_bits(),
            v.location.y.to_bits(),
            v.owner_sheet.clone(),
        )
    };
    // Only findings from earlier sheets count as already reported, so two
    // distinct clashes inside one sheet's own list are never merged.
    let mut reported: std::collections::HashSet<(String, u64, u64, Option<signex_net::SheetKey>)> =
        std::collections::HashSet::new();
    for path in paths {
        let Some(list) = by_path.get_mut(&path) else {
            continue;
        };
        list.retain(|v| !is_clash(v) || !reported.contains(&key_of(v)));
        reported.extend(list.iter().filter(|v| is_clash(v)).map(key_of));
    }
}

fn load_project_dsl_eval_fns(
    project_root: Option<&Path>,
    infos: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> Option<Vec<signex_erc::engine::EvalFn>> {
    let project_root = project_root?;
    let dsl_path = ["erc.dsl", "signex.erc.dsl"]
        .iter()
        .map(|name| project_root.join(name))
        .find(|p| p.exists())?;

    let src = match std::fs::read_to_string(&dsl_path) {
        Ok(src) => src,
        Err(e) => {
            warnings.push(format!(
                "ERC DSL: failed to read {}: {e}",
                dsl_path.display()
            ));
            return None;
        }
    };
    match signex_erc_dsl::parse_validate_compile_to_eval_fns(&src) {
        Ok(eval_fns) => {
            infos.push(format!(
                "ERC DSL: loaded {} compiled rule(s) from {}",
                eval_fns.len(),
                dsl_path.display()
            ));
            Some(eval_fns)
        }
        // Warning: the user's rules are not running.
        Err(errors) => {
            warnings.extend(errors.into_iter().map(|e| format!("ERC DSL: {e}")));
            None
        }
    }
}

#[cfg(test)]
mod tests;
