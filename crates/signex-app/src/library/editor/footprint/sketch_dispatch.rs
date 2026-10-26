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
//! - The solver is always live. v0.22 stripped the previous
//!   auto-pause hysteresis: a "paused" state was confusing for both
//!   users and the downstream Signal AI agent reading solver state.
//!   A solve is bounded by one work budget; a large sketch can exhaust
//!   it, and that is reported, not hidden.
//! - All `SketchError` cases — including an exhausted work budget — propagate as
//!   `SketchError::SolveFailed`. The `_with_warnings` wrappers
//!   surface them in `state.solve_warnings`; nothing is silently
//!   swallowed.

use signex_library::primitive::footprint::Footprint;
use signex_sketch::id::SketchEntityId;
use signex_sketch::{SketchData, SketchError, parameter};

use super::sketch_bake::{bake_solution, discard_failed_solve};
use super::sketch_conflict::{mutate_solve_or_revert, was_reverted};
use super::sketch_mode::SketchEdit;
use super::state::FootprintEditorState;
use crate::library::messages::RoleTag;

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
    mutate_solve_or_revert(state, footprint, |fp| apply_edit_inner(fp, edit))
}

/// The warnings in `current` that were not in the previous solve's set,
/// remembering `current` as the new set. A drag re-solves every frame
/// with the same warnings; only a change in the set reaches the log.
pub(super) fn take_unlogged_warnings(current: &[String], logged: &mut Vec<String>) -> Vec<String> {
    let fresh = current
        .iter()
        .filter(|w| !logged.contains(w))
        .cloned()
        .collect();
    logged.clear();
    logged.extend_from_slice(current);
    fresh
}

/// Surface a failed edit: Properties-strip warning plus the Messages
/// panel. A named conflict was reverted and says so; any other failure
/// (no convergence, an empty conflict, a zero-length line) left the edit in
/// place and says that. A drag fails the same way on every pointer tick, so
/// the log gets the message once per change of the warning set, not once per
/// tick (the Properties strip shows it for as long as it holds).
pub(super) fn report_solve_failure(state: &mut FootprintEditorState, error: &SketchError) {
    let text = format!("{error}");
    state.solve_warnings.push(text.clone());
    for warning in take_unlogged_warnings(&state.solve_warnings, &mut state.logged_solve_warnings) {
        let message = if warning != text {
            format!("Sketch bake: {warning}")
        } else if was_reverted(error) {
            format!("Sketch edit rejected: {error}")
        } else {
            format!("Sketch solve failed (the edit was kept): {error}")
        };
        crate::diagnostics::log_warning(message);
    }
}

/// Same as [`apply_sketch_edit`] but captures any returned
/// [`SketchError`] into `state.solve_warnings` instead of dropping it.
/// Used at app-dispatch call sites where there is no caller to
/// propagate the error to — the inspector strip surfaces the warning
/// list to the user.
pub fn apply_sketch_edit_with_warnings(
    state: &mut FootprintEditorState,
    footprint: &mut Footprint,
    edit: SketchEdit,
) {
    if let Err(e) = apply_sketch_edit(state, footprint, edit) {
        report_solve_failure(state, &e);
    }
}

/// v0.16.2 — apply a [`RoleTag`] change to the entity at `id`.
///
/// Behaviour:
/// 1. Snapshot the next pad-designator from the existing pad attrs
///    (excluding the target entity) so re-assigning Pad after a clear
///    doesn't double-issue a number.
/// 2. Clear every `*Attr` slot on the target entity.
/// 3. Set the matching attr per `role` with sensible defaults. Pad
///    role on a non-Point entity is a silent no-op (the construction
///    geometry doesn't carry a position the bake can use).
/// 4. Run a solve + bake so the new geometry materialises in
///    `Footprint::pads / silk_f / silk_b / courtyard / mask_openings
///    / mask_excludes / paste_apertures / pours / keepouts / cutouts`.
///
/// Returns `Err(SketchError)` only if the solver fails. The
/// no-op-on-non-Point case returns `Ok(())` so the caller can shrug
/// it off without inspecting the geometry.
pub fn apply_sketch_role(
    state: &mut FootprintEditorState,
    footprint: &mut Footprint,
    id: SketchEntityId,
    role: RoleTag,
) -> Result<(), SketchError> {
    mutate_solve_or_revert(state, footprint, |fp| set_entity_role(fp, id, role))
}

/// `_with_warnings` companion to [`apply_sketch_role`] — captures the
/// solver error into `state.solve_warnings` instead of propagating.
pub fn apply_sketch_role_with_warnings(
    state: &mut FootprintEditorState,
    footprint: &mut Footprint,
    id: SketchEntityId,
    role: RoleTag,
) {
    if let Err(e) = apply_sketch_role(state, footprint, id, role) {
        report_solve_failure(state, &e);
    }
}

/// Mutates the entity's role attrs in place. Pure — no solver work.
/// Visible to tests so they can assert the shape of the resulting
/// Entity without spinning up a solve.
pub fn set_entity_role(footprint: &mut Footprint, id: SketchEntityId, role: RoleTag) {
    use signex_sketch::attr::{
        BoardCutoutAttr, CourtyardAttr, KeepoutAttr, KeepoutKinds, MaskExcludeAttr,
        MaskOpeningAttr, PadAttr, PadKind, PadShape, PadSide, PasteApertureAttr,
        PasteAperturePattern, PourAttr, SilkAttr, ThermalRelief,
    };
    use signex_sketch::entity::EntityKind;
    use signex_types::layer::SignexLayer;

    let sketch = match footprint.sketch.as_mut() {
        Some(s) => s,
        None => return,
    };

    // Snapshot the next pad designator before we mutate; counts every
    // pad attr OTHER than the target entity's, so re-assigning Pad to
    // an already-Pad entity yields the same number we just cleared.
    let next_pad_num = sketch
        .entities
        .iter()
        .filter(|e| e.id != id)
        .filter_map(|e| e.pad.as_ref())
        .filter_map(|attr| attr.number.parse::<u32>().ok())
        .max()
        .unwrap_or(0)
        + 1;

    let entity = match sketch.entities.iter_mut().find(|e| e.id == id) {
        Some(e) => e,
        None => return,
    };

    // Clear every role attr first — fresh slate before assigning.
    entity.pad = None;
    entity.silk = None;
    entity.courtyard = None;
    entity.mask_opening = None;
    entity.mask_exclude = None;
    entity.paste_aperture = None;
    entity.pour = None;
    entity.keepout = None;
    entity.board_cutout = None;
    entity.v_score = None;

    let is_point = matches!(entity.kind, EntityKind::Point { .. });

    match role {
        RoleTag::Unassigned => {
            // Already cleared above.
        }
        RoleTag::Pad => {
            if is_point {
                entity.pad = Some(PadAttr {
                    number: next_pad_num.to_string(),
                    kind: PadKind::Smd,
                    side: PadSide::Top,
                    shape: PadShape::Rect,
                    size_x_expr: "1mm".into(),
                    size_y_expr: "1mm".into(),
                    rotation_expr: None,
                    offset_x_expr: None,
                    offset_y_expr: None,
                    drill: None,
                    mask_margin_expr: None,
                    paste_margin_expr: None,
                    paste_apertures: PasteAperturePattern::Single,
                    ..PadAttr::default()
                });
            }
            // Non-Point: silent no-op (Pad attr requires a single point
            // anchor; lines and arcs would have ambiguous bake geometry).
        }
        RoleTag::SilkTop => {
            entity.silk = Some(SilkAttr {
                layer: SignexLayer::TopSilk,
            });
        }
        RoleTag::SilkBottom => {
            entity.silk = Some(SilkAttr {
                layer: SignexLayer::BottomSilk,
            });
        }
        RoleTag::Courtyard => {
            entity.courtyard = Some(CourtyardAttr);
        }
        RoleTag::Keepout => {
            entity.keepout = Some(KeepoutAttr {
                layer: SignexLayer::TopCopper,
                kinds: KeepoutKinds::NO_ROUTING,
            });
        }
        RoleTag::Cutout => {
            entity.board_cutout = Some(BoardCutoutAttr {
                edge_radius_expr: None,
                through: true,
            });
        }
        RoleTag::MaskOpeningTop => {
            entity.mask_opening = Some(MaskOpeningAttr {
                layer: SignexLayer::TopSolderMask,
            });
        }
        RoleTag::MaskOpeningBottom => {
            entity.mask_opening = Some(MaskOpeningAttr {
                layer: SignexLayer::BottomSolderMask,
            });
        }
        RoleTag::MaskExcludeTop => {
            entity.mask_exclude = Some(MaskExcludeAttr {
                layer: SignexLayer::TopSolderMask,
            });
        }
        RoleTag::MaskExcludeBottom => {
            entity.mask_exclude = Some(MaskExcludeAttr {
                layer: SignexLayer::BottomSolderMask,
            });
        }
        RoleTag::PourTop => {
            entity.pour = Some(PourAttr {
                layer: SignexLayer::TopCopper,
                net: None,
                fill_type: Default::default(),
                thermal_relief: ThermalRelief::default(),
                clearance_expr: None,
                min_thickness_expr: None,
                priority: 0,
            });
        }
        RoleTag::PourBottom => {
            entity.pour = Some(PourAttr {
                layer: SignexLayer::BottomCopper,
                net: None,
                fill_type: Default::default(),
                thermal_relief: ThermalRelief::default(),
                clearance_expr: None,
                min_thickness_expr: None,
                priority: 0,
            });
        }
        RoleTag::PasteApertureTop => {
            entity.paste_aperture = Some(PasteApertureAttr {
                layer: SignexLayer::TopPaste,
            });
        }
        RoleTag::PasteApertureBottom => {
            entity.paste_aperture = Some(PasteApertureAttr {
                layer: SignexLayer::BottomPaste,
            });
        }
    }
}

/// Read the current [`RoleTag`] of an entity by inspecting which
/// `*Attr` slot is populated. Returns `RoleTag::Unassigned` when no
/// role attr is set (the default for fresh entities). Used by the
/// inspector to highlight the active dropdown value.
pub fn current_role_of(entity: &signex_sketch::entity::Entity) -> RoleTag {
    use signex_types::layer::SignexLayer;

    if entity.pad.is_some() {
        return RoleTag::Pad;
    }
    if let Some(silk) = entity.silk.as_ref() {
        return match silk.layer {
            SignexLayer::TopSilk => RoleTag::SilkTop,
            SignexLayer::BottomSilk => RoleTag::SilkBottom,
            _ => RoleTag::SilkTop,
        };
    }
    if entity.courtyard.is_some() {
        return RoleTag::Courtyard;
    }
    if entity.keepout.is_some() {
        return RoleTag::Keepout;
    }
    if entity.board_cutout.is_some() {
        return RoleTag::Cutout;
    }
    if let Some(m) = entity.mask_opening.as_ref() {
        return match m.layer {
            SignexLayer::TopSolderMask => RoleTag::MaskOpeningTop,
            SignexLayer::BottomSolderMask => RoleTag::MaskOpeningBottom,
            _ => RoleTag::MaskOpeningTop,
        };
    }
    if let Some(m) = entity.mask_exclude.as_ref() {
        return match m.layer {
            SignexLayer::TopSolderMask => RoleTag::MaskExcludeTop,
            SignexLayer::BottomSolderMask => RoleTag::MaskExcludeBottom,
            _ => RoleTag::MaskExcludeTop,
        };
    }
    if let Some(p) = entity.pour.as_ref() {
        return match p.layer {
            SignexLayer::TopCopper => RoleTag::PourTop,
            SignexLayer::BottomCopper => RoleTag::PourBottom,
            _ => RoleTag::PourTop,
        };
    }
    if let Some(p) = entity.paste_aperture.as_ref() {
        return match p.layer {
            SignexLayer::TopPaste => RoleTag::PasteApertureTop,
            SignexLayer::BottomPaste => RoleTag::PasteApertureBottom,
            _ => RoleTag::PasteApertureTop,
        };
    }
    RoleTag::Unassigned
}

/// Mutates `footprint.sketch` per the edit. Idempotent on its inputs
/// so the test harness can inspect intermediate state.
pub(super) fn apply_edit_inner(footprint: &mut Footprint, edit: SketchEdit) {
    let sketch = footprint.sketch.get_or_insert_with(SketchData::default);
    match edit {
        SketchEdit::AddEntity(e) => sketch.entities.push(*e),
        SketchEdit::DeleteEntity(id) => {
            // Structural cascade: lines/arcs/circles built on the entity
            // and every constraint or array naming a removed entity go with it, in
            // this one mutation (one undo step).
            let removed = sketch.remove_entity_cascade(id);
            let others = removed.entities.len().saturating_sub(1);
            if others > 0 || !removed.constraints.is_empty() || !removed.arrays.is_empty() {
                crate::diagnostics::log_info(format!(
                    "Deleting a sketch entity also removed {others} dependent entity(ies), {} constraint(s) and {} array(s)",
                    removed.constraints.len(),
                    removed.arrays.len()
                ));
            }
            if removed.repaired > 0 {
                crate::diagnostics::log_warning(format!(
                    "Repaired {} dangling sketch reference(s) left by an earlier edit",
                    removed.repaired
                ));
            }
        }
        SketchEdit::MovePoint { id, dx, dy } => {
            for ent in sketch.entities.iter_mut() {
                if ent.id == id
                    && let signex_sketch::entity::EntityKind::Point { x, y } = &mut ent.kind
                {
                    *x += dx;
                    *y += dy;
                }
            }
        }
        SketchEdit::AddConstraint(c) => sketch.constraints.push(c),
        SketchEdit::DeleteConstraint(id) => sketch.constraints.retain(|c| c.id != id),
        SketchEdit::EditParameter { name, expr } => sketch.parameters.insert(name, expr),
        SketchEdit::DeleteParameter { name } => {
            sketch.parameters.0.remove(&name);
        }
        SketchEdit::SetMode(_) | SketchEdit::ForceRebuild => {
            // Mode lives in FootprintEditorState and is set by the
            // iced update path before calling this function; nothing
            // to apply at the SketchData level.
        }
    }
}

/// Resolve parameters, run LM, capture DOF, bake pads.
///
/// v0.22 — solver is always live, no hysteresis, no pause state. All
/// `SolveError` variants (including `BudgetExceeded`) propagate as
/// `SketchError::SolveFailed` so the `_with_warnings` wrappers
/// surface them to the user in `state.solve_warnings`. Minted pads add
/// free points to the sketch (a rectangular pad 5, a rounded-rectangle pad
/// 17, none fixed); points no constraint touches stay out of the solve
/// (#1055), so pads cost solve work only once they are constrained.
pub(super) fn solve_and_bake(
    state: &mut FootprintEditorState,
    footprint: &mut Footprint,
) -> Result<(), SketchError> {
    // An inline solve covers every edit made so far, so a drag solve still
    // running off-thread is stale the moment this one starts.
    // Every path that reaches here settles an owed drag solve first; a
    // batch still pending would be dropped without a rollback or collapse.
    debug_assert!(
        !state.sketch_solve.has_unsolved(),
        "an inline solve discarded an unsettled drag batch"
    );
    if state.sketch_solve.has_unsolved() {
        crate::diagnostics::log_warning(
            "Sketch solve: an unsettled drag batch was discarded by an inline solve",
        );
    }
    state.sketch_solve.supersede();

    let sketch = match footprint.sketch.as_ref() {
        Some(s) => s,
        None => return Ok(()),
    };

    state.solve_warnings.clear();

    let resolved = parameter::resolve(&sketch.parameters).map_err(SketchError::Expr)?;

    // Runs synchronously inside `update`, for edits that are not a drag tick (a
    // drag tick solves off the UI thread, #1047, under the same budget). The LM work of the whole solve (first run,
    // retry, offset undo, conflict narrowing) draws on ONE work budget,
    // `Solver::work_budget`, so a solve that fails is bounded: measured in
    // release, roughly 90 to 150 ms at the default budget. A solve that converges
    // also runs the DOF analysis, which the budget does not bound: 14 ms at
    // n = 254, and for an already-converged sketch 92 ms at n = 574
    // (under-constrained) or 400 to 520 ms at n = 510 (fully constrained).
    // A converged sketch is never refused for its size, so it still bakes.
    match state.sketch_solver.solve(sketch, &resolved) {
        Ok(out) => bake_solution(state, footprint, &resolved, out)?,
        Err(e) => return Err(discard_failed_solve(state, e)),
    }
    Ok(())
}
