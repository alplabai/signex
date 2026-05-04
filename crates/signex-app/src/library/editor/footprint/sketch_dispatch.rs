//! Phase 5.4 + 7.3 — solve-on-edit dispatcher.
//!
//! Applies a [`SketchEdit`] to the footprint's `Option<SketchData>`,
//! then runs the LM solver, captures DOF colouring, and (Phase 7.3)
//! invokes `signex_bake` to regenerate `Footprint::pads` from the
//! solved sketch.
//!
//! Design:
//! - The dispatcher is a free function so it can be unit-tested
//!   without spinning up an iced runtime.
//! - The auto-pause state lives on `FootprintEditorState`; the
//!   dispatcher feeds it elapsed_ms after every solve and skips the
//!   next solve if `paused()`.
//! - Errors that aren't `SolveError::Timeout` propagate as
//!   `SketchError::SolveFailed` so the caller can surface them in
//!   the inspector. Timeout is consumed silently — the auto-pause
//!   state machine handles user-visible feedback.

use signex_library::primitive::footprint::Footprint;
use signex_sketch::error::SolveError;
use signex_sketch::{parameter, SketchData, SketchError};

use super::sketch_mode::SketchEdit;
use super::state::FootprintEditorState;

/// Apply a single [`SketchEdit`] and (if the sketch is non-trivial
/// and live-solve is not paused) run a solve + bake.
///
/// `state` is the editor's in-memory state; `footprint` is the
/// authoritative library primitive whose `sketch` and `pads` fields
/// the dispatcher mutates.
pub fn apply_sketch_edit(
    state: &mut FootprintEditorState,
    footprint: &mut Footprint,
    edit: SketchEdit,
) -> Result<(), SketchError> {
    apply_edit_inner(footprint, edit);
    solve_and_bake(state, footprint)
}

/// Mutates `footprint.sketch` per the edit. Idempotent on its inputs
/// so the test harness can inspect intermediate state.
fn apply_edit_inner(footprint: &mut Footprint, edit: SketchEdit) {
    let sketch = footprint
        .sketch
        .get_or_insert_with(SketchData::default);
    match edit {
        SketchEdit::AddEntity(e) => sketch.entities.push(e),
        SketchEdit::DeleteEntity(id) => {
            sketch.entities.retain(|e| e.id != id);
            // Drop dangling constraint refs — coarse rule, drop any
            // constraint that mentions the deleted ID via the kind's
            // payload. Most kinds carry one or two SketchEntityIds;
            // we use Debug to stringify for a simple text search.
            // This is intentionally simple for v0.13; Phase 6 wires
            // a structured constraint-rewrite pass.
            sketch
                .constraints
                .retain(|c| !format!("{:?}", c.kind).contains(&id.to_string()));
        }
        SketchEdit::MovePoint { id, dx, dy } => {
            for ent in sketch.entities.iter_mut() {
                if ent.id == id {
                    if let signex_sketch::entity::EntityKind::Point { x, y } = &mut ent.kind {
                        *x += dx;
                        *y += dy;
                    }
                }
            }
        }
        SketchEdit::AddConstraint(c) => sketch.constraints.push(c),
        SketchEdit::DeleteConstraint(id) => sketch.constraints.retain(|c| c.id != id),
        SketchEdit::EditParameter { name, expr } => sketch.parameters.insert(name, expr),
        SketchEdit::DeleteParameter { name } => {
            sketch.parameters.0.remove(&name);
        }
        SketchEdit::SetMode(_) | SketchEdit::ForceRebuild | SketchEdit::ToggleAutoPause => {
            // Mode / pause state machine lives in FootprintEditorState
            // and is set by the iced update path before calling this
            // function; nothing to apply at the SketchData level.
        }
    }
}

/// Resolve parameters, run LM, capture DOF, bake pads.
///
/// On `SolveError::Timeout`: feeds the AutoPauseState and returns
/// `Ok(())` (silent — UI handles the pause toast). Other errors
/// propagate.
fn solve_and_bake(
    state: &mut FootprintEditorState,
    footprint: &mut Footprint,
) -> Result<(), SketchError> {
    if state.auto_pause.paused() {
        return Ok(());
    }
    let sketch = match footprint.sketch.as_ref() {
        Some(s) => s,
        None => return Ok(()),
    };

    state.solve_warnings.clear();

    let resolved = parameter::resolve(&sketch.parameters).map_err(SketchError::Expr)?;

    match state.sketch_solver.solve(sketch, &resolved) {
        Ok(out) => {
            state
                .auto_pause
                .observe(out.result.elapsed_ms, state.sketch_solver.timeout_ms);

            // Phase 7.3 — bake pads + arrays into the footprint, but
            // skip the overwrite on an empty sketch: a fresh
            // SetMode(Sketch) toggle on a footprint with literal pads
            // would otherwise produce an empty Vec and clobber the
            // user's existing pad authoring on first entry. Once the
            // user adds sketch entities, the sketch becomes the
            // source of truth and a (possibly empty) bake result is
            // intentional.
            if !sketch.entities.is_empty() {
                let mut baked: Vec<signex_library::primitive::footprint::Pad> = Vec::new();
                signex_bake::bake_pads(
                    sketch,
                    &out,
                    &resolved,
                    &mut baked,
                    &mut state.solve_warnings,
                )?;
                signex_bake::bake_arrays(
                    sketch,
                    &out,
                    &resolved,
                    &mut baked,
                    &mut state.solve_warnings,
                )?;
                footprint.pads = baked;
            }

            state.last_solve = Some(out);
        }
        Err(SolveError::Timeout {
            elapsed_ms,
            budget_ms,
        }) => {
            state.auto_pause.observe(elapsed_ms, budget_ms);
        }
        Err(e) => return Err(SketchError::SolveFailed(e)),
    }
    Ok(())
}
