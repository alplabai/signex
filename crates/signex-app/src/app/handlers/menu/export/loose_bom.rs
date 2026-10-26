//! The BOM scope of a loose (project-less) document, independent of which tab
//! is focused (#1139).
//!
//! A scope walks the hierarchy from one root sheet, and a sheet reached twice
//! prints (and counts) twice. Rooted at the focused tab, a parent that places
//! a child twice listed the child twice, while the same two tabs rooted at the
//! child listed it once. So a loose BOM walks from every top of the loose
//! hierarchy instead: each unowned open tab that no other unowned open tab
//! places as a child. The result is the same whichever tab is focused.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use signex_output::ExportContext;

use super::{ExportIssues, build_scope};
use crate::app::state::DocumentState;
use crate::app::state::scope::{CaseFold, path_key_with};

/// The loose BOM scope for `source`: `source`'s own scope (metadata, current
/// sheet), with its page set replaced by the union of every loose top's walk.
pub(super) fn loose_bom_scope(
    document_state: &DocumentState,
    source: &Path,
) -> Option<(ExportContext, ExportIssues)> {
    let fold = CaseFold::host_default();
    let unowned = document_state.unowned_engine_paths();
    let tops = loose_tops(document_state, &unowned, fold);

    let (mut ctx, mut issues) = build_scope(document_state, source, false)?;
    let mut sheets = Vec::new();
    for top in &tops {
        let closure = closure_keys(document_state, top, &unowned, fold);
        let (run, run_issues) = if top.as_path() == source {
            (ctx.clone(), None)
        } else {
            let (run, run_issues) = build_scope(document_state, top, false)?;
            (run, Some(run_issues))
        };
        sheets.extend(
            run.sheets
                .into_iter()
                .filter(|page| closure.contains(&path_key_with(&page.path, fold))),
        );
        if let Some(extra) = run_issues {
            issues.stitch.extend(extra.stitch);
            issues.missing_pages.extend(extra.missing_pages);
            issues.unreadable.extend(extra.unreadable);
        }
    }
    let count = sheets.len();
    for (i, page) in sheets.iter_mut().enumerate() {
        page.sheet_number = i + 1;
        page.sheet_count = count;
    }
    ctx.sheets = sheets;
    Some((ctx, issues))
}

/// Unowned open tabs that no other unowned open tab places, sorted. A cycle
/// has no top, so any tab no top reaches is promoted to a top of its own.
fn loose_tops(document_state: &DocumentState, unowned: &[PathBuf], fold: CaseFold) -> Vec<PathBuf> {
    let placed: HashSet<String> = unowned
        .iter()
        .flat_map(|parent| children_of(document_state, parent, unowned, fold))
        .collect();
    let mut tops: Vec<PathBuf> = unowned
        .iter()
        .filter(|path| !placed.contains(&path_key_with(path, fold)))
        .cloned()
        .collect();
    let mut covered: HashSet<String> = HashSet::new();
    for top in &tops {
        covered.extend(closure_keys(document_state, top, unowned, fold));
    }
    for path in unowned {
        if covered.insert(path_key_with(path, fold)) {
            covered.extend(closure_keys(document_state, path, unowned, fold));
            tops.push(path.clone());
        }
    }
    tops.sort();
    tops
}

/// Keys of `top` and every open unowned tab it places, transitively.
fn closure_keys(
    document_state: &DocumentState,
    top: &Path,
    unowned: &[PathBuf],
    fold: CaseFold,
) -> HashSet<String> {
    let by_key: HashMap<String, &PathBuf> = unowned
        .iter()
        .map(|p| (path_key_with(p, fold), p))
        .collect();
    let mut seen: HashSet<String> = HashSet::new();
    let mut stack = vec![top.to_path_buf()];
    while let Some(path) = stack.pop() {
        if !seen.insert(path_key_with(&path, fold)) {
            continue;
        }
        for key in children_of(document_state, &path, unowned, fold) {
            if let Some(child) = by_key.get(&key) {
                stack.push((*child).clone());
            }
        }
    }
    seen
}

/// Keys of the open unowned tabs `parent` places as child sheets.
fn children_of(
    document_state: &DocumentState,
    parent: &Path,
    unowned: &[PathBuf],
    fold: CaseFold,
) -> Vec<String> {
    let Some(engine) = document_state.engines.get(parent) else {
        return Vec::new();
    };
    let dir = parent.parent().unwrap_or_else(|| Path::new(""));
    let open: HashSet<String> = unowned.iter().map(|p| path_key_with(p, fold)).collect();
    engine
        .document()
        .child_sheets
        .iter()
        .filter_map(|cs| crate::app::project_sheets::resolve_child_reference(dir, &cs.filename))
        .map(|path| path_key_with(&path, fold))
        .filter(|key| open.contains(key))
        .collect()
}
