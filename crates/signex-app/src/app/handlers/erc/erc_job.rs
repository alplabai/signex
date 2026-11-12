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
    /// Identity of the rule set `overrides`/`config` came from, carried to the
    /// outcome so the report header names the set that produced the rows.
    pub rule_set: signex_output::ErcReportRuleSet,
    pub config: signex_erc::ErcConfig,
    /// The project's net waivers (#1581); empty without stored rules.
    pub net_waivers: signex_erc::waivers::NetWaivers,
    /// The project's per-finding waivers (#1285); empty without stored rules.
    pub violation_waivers: Vec<signex_types::project::ErcWaiver>,
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
    /// The rule set the run used.
    pub rule_set: signex_output::ErcReportRuleSet,
    /// Findings each waived (rule id, net) pair hid in this run (#1581).
    pub waived_hidden: std::collections::BTreeMap<(String, String), usize>,
    /// Findings a per-finding waiver hid (#1285), kept so the panel and the
    /// report still show them as waived.
    pub waived_findings: Vec<WaivedFinding>,
    /// (rule id, objects) of waivers whose finding came back because an
    /// involved object changed.
    pub lapsed_waivers: std::collections::BTreeSet<(String, Vec<String>)>,
    /// Each checked sheet's page frame, closed sheets included, so a
    /// finding's Y shows from that sheet's page origin (#2034).
    pub page_frames: PageFrames,
}

/// Per sheet: the page origin and the paper height shown Y counts from.
pub type PageFrames = HashMap<PathBuf, (signex_types::schematic::PageOrigin, f64)>;

/// A finding a per-finding waiver hid, with the waiver that hid it.
#[derive(Debug, Clone)]
pub struct WaivedFinding {
    pub path: PathBuf,
    pub violation: signex_erc::Violation,
    pub waiver: signex_types::project::ErcWaiver,
}

/// The findings a net waiver hid, in sheet path order: the sheet, the finding
/// and the (rule id, net) waiver.
pub type NetWaived = Vec<(PathBuf, signex_erc::Violation, (String, String))>;

pub(super) fn run_erc_job(inputs: ErcRunInputs) -> ErcRunOutcome {
    run_job(inputs, true, None).0
}

/// The run `check_erc` makes (#2199): [`run_erc_job`] raising no Messages
/// line (what Run ERC would say there is in the outcome's `warnings`), with
/// the findings a net waiver hid kept. With `confine`, the project folder, a
/// file is read from disk only as a headless query may read it
/// (`document_ids::confined_read`); one that is not is in `warnings`.
pub(super) fn run_erc_check_job(
    inputs: ErcRunInputs,
    confine: Option<&Path>,
) -> (ErcRunOutcome, NetWaived) {
    run_job(inputs, false, confine)
}

/// `logged`: a refused child reference is said in Messages as it is met;
/// otherwise it is a warning of the outcome.
fn run_job(
    inputs: ErcRunInputs,
    logged: bool,
    confine: Option<&Path>,
) -> (ErcRunOutcome, NetWaived) {
    let inputs_rule_set = inputs.rule_set.clone();
    let mut warnings = Vec::new();
    let mut infos = Vec::new();
    let may_read = |path: &Path| {
        confine.map_or(Ok(()), |dir| {
            crate::app::document_ids::confined_read(dir, path)
        })
    };
    let dsl_eval_fns = load_project_dsl_eval_fns(
        inputs.project_dir.as_deref(),
        &may_read,
        &mut infos,
        &mut warnings,
    );

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
    let sheet_guard = |path: &Path| may_read(path).map_err(|why| format!("is not read: {why}"));
    let project_set = match &inputs.root {
        Some(root) if logged => project_sheets::assemble_project_sheets_from(
            &inputs.open,
            &inputs.pages,
            root,
            inputs.fold,
        ),
        Some(root) => project_sheets::assemble_project_sheets_guarded_from(
            &inputs.open,
            &inputs.pages,
            root,
            inputs.fold,
            &sheet_guard,
        ),
        None => project_sheets::ProjectSheetSet::empty(),
    };
    for (path, why) in &project_set.unreadable {
        warnings.push(format!("ERC: sheet {} {why}", path.display()));
    }
    if !logged {
        warnings.extend(refused_reference_lines(&project_set));
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
    let stitch = if logged {
        project_sheets::project_graph_with
    } else {
        project_sheets::project_graph_quiet
    };
    let graph = stitch(
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
    // The project's own sheets, for the project-wide passes; loose tabs from
    // elsewhere are checked on their own.
    let project_paths: std::collections::HashSet<PathBuf> =
        project_set.sheets.keys().cloned().collect();
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
    add_project_units_not_placed(&mut by_path, &snapshots, &project_paths, &inputs.overrides);
    for line in refile_child_sheet_violations(&mut by_path, logged) {
        if !warnings.contains(&line) {
            warnings.push(line);
        }
    }
    dedupe_descendant_clashes(&mut by_path);

    let (waived_findings, lapsed_waivers) =
        apply_violation_waivers(&mut by_path, &snapshots, &inputs.violation_waivers);

    // Last, so every pass above (including the project-wide net-name pass)
    // is waivable. Hidden findings are counted, not lost: the panel lists
    // the waivers and offers to restore them.
    let net_waived = split_net_waived(&mut by_path, &inputs.net_waivers);
    let mut waived_hidden = std::collections::BTreeMap::new();
    for (_, _, key) in &net_waived {
        *waived_hidden.entry(key.clone()).or_default() += 1;
    }

    let page_frames = snapshots
        .iter()
        .map(|(path, sheet)| {
            let height = signex_types::schematic::paper_height_mm(&sheet.paper_size);
            (path.clone(), (sheet.page.origin, height))
        })
        .collect();
    let total: usize = by_path.values().map(Vec::len).sum();
    infos.push(format!(
        "ERC: {total} total violations across {} sheet(s)",
        by_path.len()
    ));
    let outcome = ErcRunOutcome {
        by_path,
        warnings,
        infos,
        rule_set: inputs_rule_set,
        waived_hidden,
        waived_findings,
        lapsed_waivers,
        page_frames,
    };
    (outcome, net_waived)
}

/// Take out of each sheet's list, in sheet path order, the findings a net
/// waiver (#1581) hides.
fn split_net_waived(
    by_path: &mut ViolationsByPath,
    waivers: &signex_erc::waivers::NetWaivers,
) -> NetWaived {
    let mut paths: Vec<PathBuf> = by_path.keys().cloned().collect();
    paths.sort();
    let mut hidden = Vec::new();
    for path in paths {
        let Some(list) = by_path.get_mut(&path) else {
            continue;
        };
        let (kept, waived) = signex_erc::waivers::partition_waived(std::mem::take(list), waivers);
        *list = kept;
        hidden.extend(waived.into_iter().map(|(v, key)| (path.clone(), v, key)));
    }
    hidden
}

/// Key every finding against the sheet it is filed under, then take out the
/// ones a per-finding waiver hides (#1285). Returns those, and the identities
/// of waivers that lapsed because an involved object changed.
fn apply_violation_waivers(
    by_path: &mut ViolationsByPath,
    snapshots: &HashMap<PathBuf, SchematicRenderSnapshot>,
    waivers: &[signex_types::project::ErcWaiver],
) -> (
    Vec<WaivedFinding>,
    std::collections::BTreeSet<(String, Vec<String>)>,
) {
    let mut waived = Vec::new();
    let mut lapsed = std::collections::BTreeSet::new();
    let mut paths: Vec<PathBuf> = by_path.keys().cloned().collect();
    paths.sort();
    for path in paths {
        let (Some(list), Some(sheet)) = (by_path.get_mut(&path), snapshots.get(&path)) else {
            continue;
        };
        signex_erc::waivers::violation::stamp_keys(list, sheet);
        let split = signex_erc::waivers::violation::split_waived(std::mem::take(list), waivers);
        *list = split.kept;
        for (violation, i) in split.waived {
            waived.push(WaivedFinding {
                path: path.clone(),
                violation,
                waiver: waivers[i].clone(),
            });
        }
        lapsed.extend(
            split
                .lapsed
                .into_iter()
                .map(|i| (waivers[i].rule_id.clone(), waivers[i].objects.clone())),
        );
    }
    (waived, lapsed)
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

/// The project-wide `UnitsNotPlaced` pass (#1286): units of one part are
/// often split across sheets, so the per-sheet findings of the project's
/// sheets are replaced by one pass over those sheets, in path order. A loose
/// tab outside the project keeps its own per-sheet findings.
fn add_project_units_not_placed(
    by_path: &mut ViolationsByPath,
    snapshots: &HashMap<PathBuf, SchematicRenderSnapshot>,
    project_paths: &std::collections::HashSet<PathBuf>,
    overrides: &HashMap<signex_erc::RuleKind, signex_erc::Severity>,
) {
    let is_units =
        |v: &signex_erc::Violation| v.builtin_kind() == Some(signex_erc::RuleKind::UnitsNotPlaced);
    for (path, list) in by_path.iter_mut() {
        if project_paths.contains(path) {
            list.retain(|v| !is_units(v));
        }
    }
    let mut sheets: Vec<(&PathBuf, &SchematicRenderSnapshot)> = snapshots
        .iter()
        .filter(|(path, _)| project_paths.contains(*path))
        .collect();
    sheets.sort_by(|a, b| a.0.cmp(b.0));
    for (path, violation) in signex_erc::project_units_not_placed(&sheets) {
        let mut list = by_path.remove(path).unwrap_or_default();
        list.push(violation);
        by_path.insert(path.clone(), signex_erc::apply_overrides(list, overrides));
    }
}

/// Move every child-space violation (e.g. `BadHierSheetPin` on a child's
/// hierarchical label) from the parent's list to the child sheet that owns the
/// coordinate, so its marker and panel entry land on the right canvas (#956).
/// A child that was not part of the run stays under the parent as a
/// non-locatable entry: it is reported, never drawn at a foreign coordinate.
/// Not `logged`, a refused reference is returned instead of said.
fn refile_child_sheet_violations(by_path: &mut ViolationsByPath, logged: bool) -> Vec<String> {
    let ran: std::collections::HashSet<std::path::PathBuf> = by_path.keys().cloned().collect();
    let mut moved: Vec<(std::path::PathBuf, signex_erc::Violation)> = Vec::new();
    let mut refused = Vec::new();
    for (parent, list) in by_path.iter_mut() {
        let dir = parent.parent().unwrap_or_else(|| std::path::Path::new(""));
        let (child_space, kept): (Vec<_>, Vec<_>) = std::mem::take(list)
            .into_iter()
            .partition(|v| v.location_space == signex_erc::LocationSpace::ChildSheet);
        *list = kept;
        let mut resolve = |name: &str| {
            if logged {
                return project_sheets::resolve_child_reference(dir, name);
            }
            project_sheets::resolve_child_reference_quiet(dir, name)
                .map_err(|refusal| refused.extend(project_sheets::refusal_message(&refusal)))
                .ok()
        };
        for v in child_space {
            let owner = v
                .child_sheet
                .as_deref()
                .and_then(&mut resolve)
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
    refused
}

/// What Run ERC says in Messages for each child reference the walk refused
/// (`project_sheets::log_refusal`), once each.
fn refused_reference_lines(set: &project_sheets::ProjectSheetSet) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for refused in &set.refused {
        let dir = refused.parent.parent().unwrap_or_else(|| Path::new(""));
        let Err(refusal) = project_sheets::resolve_child_reference_quiet(dir, &refused.reference)
        else {
            continue;
        };
        if let Some(line) = project_sheets::refusal_message(&refusal)
            && !lines.contains(&line)
        {
            lines.push(line);
        }
    }
    lines
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

/// The project's DSL rules; `may_read` refuses a file a headless check may
/// not read (always allows for Run ERC).
fn load_project_dsl_eval_fns(
    project_root: Option<&Path>,
    may_read: &dyn Fn(&Path) -> Result<(), String>,
    infos: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> Option<Vec<signex_erc::engine::EvalFn>> {
    let project_root = project_root?;
    let dsl_path = ["erc.dsl", "signex.erc.dsl"]
        .iter()
        .map(|name| project_root.join(name))
        .find(|p| p.exists())?;
    if let Err(why) = may_read(&dsl_path) {
        warnings.push(format!("ERC DSL: {why}"));
        return None;
    }

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
