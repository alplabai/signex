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
        Some(root) => {
            project_sheets::assemble_project_sheets_from(&inputs.open, &inputs.pages, root)
        }
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
    // `None` root: ERC reports on every sheet regardless of reachability.
    let graph = project_sheets::project_graph(&project_set.sheets, base_dir.as_deref(), None);
    warnings.extend(
        graph
            .issues
            .iter()
            .map(project_sheets::stitch_issue_message),
    );

    // Loose open tabs are on screen, so their violations are wanted too.
    for (path, sheet) in project_set.sheets {
        snapshots.entry(path).or_insert(sheet);
    }

    let empty_resolved: HashMap<String, signex_net::SheetKey> = HashMap::new();
    let mut by_path: ViolationsByPath = HashMap::new();
    for (path, snapshot) in &snapshots {
        let key = project_sheets::sheet_key(path, base_dir.as_deref());
        let resolved = graph.resolved.get(&key).unwrap_or(&empty_resolved);
        let raw = match dsl_eval_fns.as_ref() {
            Some(eval_fns) => signex_erc::run_with_project_dsl_and_config(
                snapshot,
                resolved,
                &graph.sheets,
                eval_fns,
                &inputs.config,
            ),
            None => signex_erc::run_with_project_and_config(
                snapshot,
                resolved,
                &graph.sheets,
                &inputs.config,
            ),
        };
        by_path.insert(
            path.clone(),
            signex_erc::apply_overrides(raw, &inputs.overrides),
        );
    }

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
