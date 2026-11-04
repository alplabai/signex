//! Footprint sketch updates — applying a redundancy pass that finished off
//! the UI thread (#1685).
//!
//! The solve that owed it already baked; this adds the pass's findings to
//! its output. The answer is for one generation of the sketch and is
//! applied only if that generation is still current.

use crate::app::FootprintDocument;
use crate::library::editor::footprint::sketch_dispatch::take_unlogged_warnings;
use crate::library::messages::FootprintEditorMsg;
use signex_sketch::solver::REDUNDANCY_SKIPPED;

pub(in crate::library::editor::footprint::updates) fn apply(
    editor: &mut FootprintDocument,
    msg: FootprintEditorMsg,
) {
    let FootprintEditorMsg::RedundancyChecked {
        instance,
        generation,
        footprint_idx,
        report,
    } = msg
    else {
        unreachable!("non-RedundancyChecked variant routed to sketch/redundancy.rs");
    };
    if instance != editor.state.sketch_solve.instance() {
        return;
    }
    editor.state.sketch_solve.finish_redundancy();
    if generation != editor.state.sketch_solve.generation() || footprint_idx != editor.active_idx {
        return;
    }
    editor.state.sketch_solve.clear_redundancy_owed();
    let report = match report {
        Ok(report) => report,
        Err(failure) => {
            crate::diagnostics::log_warning(format!("Sketch redundancy check: {failure}"));
            return;
        }
    };
    let Some(solve) = editor.state.last_solve.as_mut() else {
        return;
    };
    solve.redundant_constraints = report.redundant_constraints.clone();
    solve.redundancy_pending = false;
    for warning in &report.warnings {
        if !editor.state.solve_warnings.contains(warning) {
            editor.state.solve_warnings.push(warning.clone());
        }
    }
    // A pass that completed ends the skip: a later skip is news again.
    if !report.warnings.iter().any(|w| w == REDUNDANCY_SKIPPED) {
        editor
            .state
            .logged_solve_warnings
            .retain(|w| w != REDUNDANCY_SKIPPED);
    }
    // The Messages panel, like every other solve warning (the bake does the same).
    for warning in take_unlogged_warnings(
        &editor.state.solve_warnings,
        &mut editor.state.logged_solve_warnings,
    ) {
        crate::diagnostics::log_warning(format!("Sketch bake: {warning}"));
    }
    editor.canvas_cache.clear();
}
