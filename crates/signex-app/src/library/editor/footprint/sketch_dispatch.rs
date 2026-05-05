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
use signex_sketch::id::SketchEntityId;
use signex_sketch::{SketchData, SketchError, parameter};

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
    apply_edit_inner(footprint, edit);
    solve_and_bake(state, footprint)
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
        state.solve_warnings.push(format!("{e}"));
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
    set_entity_role(footprint, id, role);
    solve_and_bake(state, footprint)
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
        state.solve_warnings.push(format!("{e}"));
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
fn apply_edit_inner(footprint: &mut Footprint, edit: SketchEdit) {
    let sketch = footprint.sketch.get_or_insert_with(SketchData::default);
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
/// v0.16.1 — solver is always live. The previous auto-pause /
/// hysteresis early-return was removed because footprint sketches
/// stay small (tens-to-low-hundreds of entities) and the solver's
/// per-frame cost is well below the per-frame budget. `AutoPauseState`
/// still observes elapsed_ms for telemetry but never blocks.
///
/// On `SolveError::Timeout`: feeds the AutoPauseState and returns
/// `Ok(())` (silent — UI handles the pause toast). Other errors
/// propagate.
fn solve_and_bake(
    state: &mut FootprintEditorState,
    footprint: &mut Footprint,
) -> Result<(), SketchError> {
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
                // Pads + array expansions.
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

                // v0.14 closed-profile bakes — silk / courtyard / mask /
                // paste-aperture / pour. Each replaces its corresponding
                // Footprint field (or appends, in the case of the
                // multi-record Vec fields) so the sketch is the source
                // of truth for any geometry it produces.
                let mut silk_f = Vec::new();
                let mut silk_b = Vec::new();
                signex_bake::bake_silk(
                    sketch,
                    &out,
                    &mut silk_f,
                    &mut silk_b,
                    &mut state.solve_warnings,
                )?;
                if !silk_f.is_empty() {
                    footprint.silk_f = silk_f;
                }
                if !silk_b.is_empty() {
                    footprint.silk_b = silk_b;
                }

                let mut courtyard = signex_library::primitive::footprint::Polygon::default();
                signex_bake::bake_courtyard(
                    sketch,
                    &out,
                    &mut courtyard,
                    &mut state.solve_warnings,
                )?;
                if !courtyard.points.is_empty() {
                    footprint.courtyard = courtyard;
                }

                let mut mask_openings = Vec::new();
                let mut mask_excludes = Vec::new();
                let mut paste_apertures = Vec::new();
                let mut pours = Vec::new();
                let mut keepouts = Vec::new();
                let mut cutouts = Vec::new();
                let mut v_scores = Vec::new();
                signex_bake::bake_mask_openings(
                    sketch,
                    &out,
                    &mut mask_openings,
                    &mut state.solve_warnings,
                )?;
                signex_bake::bake_mask_excludes(
                    sketch,
                    &out,
                    &mut mask_excludes,
                    &mut state.solve_warnings,
                )?;
                signex_bake::bake_paste_apertures(
                    sketch,
                    &out,
                    &mut paste_apertures,
                    &mut state.solve_warnings,
                )?;
                signex_bake::bake_pours(
                    sketch,
                    &out,
                    &resolved,
                    &mut pours,
                    &mut state.solve_warnings,
                )?;
                signex_bake::bake_keepouts(sketch, &out, &mut keepouts, &mut state.solve_warnings)?;
                signex_bake::bake_cutouts(
                    sketch,
                    &out,
                    &resolved,
                    &mut cutouts,
                    &mut state.solve_warnings,
                )?;
                signex_bake::bake_v_scores(
                    sketch,
                    &out,
                    &resolved,
                    &mut v_scores,
                    &mut state.solve_warnings,
                )?;
                if !mask_openings.is_empty() {
                    footprint.mask_openings = mask_openings;
                }
                if !mask_excludes.is_empty() {
                    footprint.mask_excludes = mask_excludes;
                }
                if !paste_apertures.is_empty() {
                    footprint.paste_apertures = paste_apertures;
                }
                if !pours.is_empty() {
                    footprint.pours = pours;
                }
                if !keepouts.is_empty() {
                    footprint.keepouts = keepouts;
                }
                if !cutouts.is_empty() {
                    footprint.cutouts = cutouts;
                }
                if !v_scores.is_empty() {
                    footprint.v_scores = v_scores;
                }

                // v0.14.1 — 3D extrude profile from a BodyTop plane
                // enriches the existing body_3d.outline + offset_z_mm.
                signex_bake::bake_body3d(
                    sketch,
                    &out,
                    &resolved,
                    &mut footprint.body_3d,
                    &mut state.solve_warnings,
                )?;
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
