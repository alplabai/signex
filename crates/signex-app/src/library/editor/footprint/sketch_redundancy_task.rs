//! Running the footprint sketch redundancy pass off the UI thread (#1685).
//!
//! The pass costs hundreds of milliseconds on a dense single-block sketch,
//! more than its work-unit price says. Every solve that runs inside `update`
//! therefore leaves it out and notes it is owed; [`dispatch_redundancy`]
//! returns a `Task` that runs it on a snapshot of the solved state, and the
//! answer returns as [`FootprintEditorMsg::RedundancyChecked`]. It is applied
//! only if no edit or solve has happened since, so a stale answer changes
//! nothing.

use std::sync::Arc;

use iced::Task;
use signex_sketch::SketchData;
use signex_sketch::id::ConstraintId;
use signex_sketch::solver::residual::ResolvedParams;
use signex_sketch::solver::state::EntityIndex;
use signex_sketch::solver::{RedundancyReport, Solver};
use uuid::Uuid;

use crate::app::FootprintDocument;
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};

/// What the pass needs, owned: the sketch and the solved state it was
/// solved to, tagged with the generation it was taken at.
#[derive(Debug, Clone)]
pub struct RedundancyRequest {
    instance: Uuid,
    generation: u64,
    footprint_idx: usize,
    sketch: SketchData,
    state: Vec<f64>,
    index: EntityIndex,
    params: ResolvedParams,
    violated: Vec<ConstraintId>,
    solver: Solver,
}

/// Run the pass on the snapshot and package the answer as the message that
/// carries it back. Pure: runs on whichever thread the `Task` lands on, and
/// in tests runs inline.
pub fn run_redundancy(request: RedundancyRequest) -> FootprintEditorMsg {
    let report = request.solver.redundancy_report(
        &request.sketch,
        &request.state,
        &request.index,
        &request.params,
        &request.violated,
    );
    checked_message(&request, report)
}

fn checked_message(request: &RedundancyRequest, report: RedundancyReport) -> FootprintEditorMsg {
    FootprintEditorMsg::RedundancyChecked {
        instance: request.instance,
        generation: request.generation,
        footprint_idx: request.footprint_idx,
        report: Ok(Arc::new(report)),
    }
}

/// [`run_redundancy`] on tokio's blocking pool. A task that panicked or was
/// cancelled answers with the failure, so the slot is freed and the editor
/// never waits for it, and the failure is shown rather than read as "no
/// redundant constraints".
async fn run_redundancy_blocking(request: RedundancyRequest) -> FootprintEditorMsg {
    let (instance, generation, footprint_idx) =
        (request.instance, request.generation, request.footprint_idx);
    match tokio::task::spawn_blocking(move || run_redundancy(request)).await {
        Ok(msg) => msg,
        Err(e) => FootprintEditorMsg::RedundancyChecked {
            instance,
            generation,
            footprint_idx,
            report: Err(format!("the redundancy check did not complete: {e}")),
        },
    }
}

/// The snapshot a pass would run on right now, or `None` when the editor
/// has no solved sketch to run it on.
pub fn snapshot_redundancy_request(editor: &FootprintDocument) -> Option<RedundancyRequest> {
    let solve = editor.state.last_solve.as_ref()?;
    let slot = &editor.state.sketch_solve;
    Some(RedundancyRequest {
        instance: slot.instance(),
        generation: slot.generation(),
        footprint_idx: editor.active_idx,
        sketch: editor.primitive().sketch.clone()?,
        state: solve.result.state.clone(),
        index: solve.result.index.clone(),
        params: solve.params.clone(),
        violated: solve.over_constraints.clone(),
        solver: editor.state.sketch_solver.clone(),
    })
}

/// Start the pass a solve left owed, unless one is already running or a
/// solve is still owed (the solved state is not the sketch's yet). The
/// next call, made when the running pass lands, starts the follow-up.
pub(crate) fn dispatch_redundancy(
    editor: &mut FootprintDocument,
    path: &std::path::Path,
) -> Task<LibraryMessage> {
    let slot = &editor.state.sketch_solve;
    if !slot.is_redundancy_owed() || slot.is_redundancy_running() || slot.is_solve_owed() {
        return Task::none();
    }
    let Some(request) = snapshot_redundancy_request(editor) else {
        editor.state.sketch_solve.clear_redundancy_owed();
        return Task::none();
    };
    editor.state.sketch_solve.start_redundancy();
    let path = path.to_path_buf();
    Task::perform(run_redundancy_blocking(request), move |msg| {
        LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: PrimitiveEdit::Footprint(msg),
        }
    })
}
