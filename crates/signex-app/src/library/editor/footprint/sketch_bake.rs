//! Baking a solved sketch into the footprint.
//!
//! The half of the solve-on-edit pipeline that follows the solver: it
//! turns a [`FullSolveOutput`] into `Footprint::pads` and the other baked
//! fields, and refreshes the editor-side mirrors. Both the synchronous
//! dispatcher ([`super::sketch_dispatch::solve_and_bake`]) and the
//! off-thread drag solve ([`super::updates`]'s `SketchSolved` handler) call
//! it, so a solve applied late bakes exactly what one applied inline does.

use std::collections::HashMap;

use signex_library::primitive::footprint::Footprint;
use signex_sketch::solver::FullSolveOutput;
use signex_sketch::{SketchError, SolveError};

use super::sketch_dispatch::take_unlogged_warnings;
use super::state::FootprintEditorState;

/// Bake a successful solve into `footprint` and record it as the editor's
/// last solve. Clears the conflict highlight: a sketch that solves has no
/// conflict to show.
pub(super) fn bake_solution(
    state: &mut FootprintEditorState,
    footprint: &mut Footprint,
    resolved: &HashMap<String, f64>,
    out: FullSolveOutput,
) -> Result<(), SketchError> {
    let Some(sketch) = footprint.sketch.as_ref() else {
        return Ok(());
    };
    state.conflict.clear();
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
            resolved,
            &mut baked,
            &mut state.solve_warnings,
        )?;
        signex_bake::bake_arrays(
            sketch,
            &out,
            resolved,
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
        signex_bake::bake_courtyard(sketch, &out, &mut courtyard, &mut state.solve_warnings)?;
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
            resolved,
            &mut pours,
            &mut state.solve_warnings,
        )?;
        signex_bake::bake_keepouts(sketch, &out, &mut keepouts, &mut state.solve_warnings)?;
        signex_bake::bake_cutouts(
            sketch,
            &out,
            resolved,
            &mut cutouts,
            &mut state.solve_warnings,
        )?;
        signex_bake::bake_v_scores(
            sketch,
            &out,
            resolved,
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
            resolved,
            &mut footprint.body_3d,
            &mut state.solve_warnings,
        )?;

        // Bake warnings (a skipped over-cap array, a dropped
        // feature) are failures the user must be able to see:
        // the inspector strip is not the Messages panel.
        for warning in
            take_unlogged_warnings(&state.solve_warnings, &mut state.logged_solve_warnings)
        {
            crate::diagnostics::log_warning(format!("Sketch bake: {warning}"));
        }

        // v0.22 Phase D2 — refresh editor-side pads cache so
        // the canvas + Pads-mode Properties panel see what
        // the bake just produced. Preserves sketch_entity_id
        // / corner_entity_ids by matching pad.number across
        // the old + new lists.
        state.refresh_pads_from_primitive(footprint);

        // v0.24 Phase 3 (Track A4) — reverse mirror. Walk
        // each pad's shape_params (preserved across the
        // refresh above) and re-derive
        // EditorPad.stack.corner_radius_pct from the live
        // resolved corner_r value. Keeps the Pads-mode
        // "Corner radius %" input in sync when the sketch
        // parameter is edited from elsewhere (Properties row,
        // parameter table, drag handle).
        super::pad_to_sketch::mirror_solve_to_pad_stack(state, resolved);

        // v0.25 polish — Oval pads: width/height parameter
        // edits propagate back to `pad.size_mm` so the bbox
        // tracks the resolved values. Without this, the
        // subsequent mirror_solve_to_oval_geometry call would
        // reposition anchors against a stale bbox.
        super::pad_to_sketch::mirror_solve_to_oval_size(state, resolved);

        // v0.24 Track A6 — chamfer anchor reposition. The
        // shared `chamfer_len_<slug>` parameter doesn't drive
        // the anchor Points via solver constraints (literal
        // mint), so this post-solve pass rewrites the
        // anchor coords from the resolved chamfer_len value.
        // Only fires for pads with a `chamfer_len` binding;
        // other pads get an early return inside the helper.
        //
        // v0.24 Phase 6 — same pattern for RoundRect arcs and
        // Oval anchors / centres. Each helper bails early on
        // pads whose `shape_params` don't carry the matching
        // canonical key, so all three run unconditionally
        // without per-shape branching here.
        if let Some(sketch) = footprint.sketch.as_mut() {
            super::pad_to_sketch::mirror_solve_to_chamfer_anchors(state, sketch, resolved);
            super::pad_to_sketch::mirror_solve_to_round_rect_geometry(state, sketch, resolved);
            super::pad_to_sketch::mirror_solve_to_oval_geometry(state, sketch, resolved);
        }
    }

    state.last_solve = Some(out);
    Ok(())
}

/// Drop the previous solve after a failed one, and hand back the error in
/// the shape the dispatcher reports.
pub(super) fn discard_failed_solve(
    state: &mut FootprintEditorState,
    error: SolveError,
) -> SketchError {
    // #613 — drop the previous solve rather than leaving it to
    // be painted. Everything downstream reads `last_solve`
    // through `as_ref()` / `is_none()` and already has an
    // unsolved rendering (`updates/sketch/pad_bridge.rs`
    // documents `None` as "no solve has run yet"), so clearing
    // shows the sketch as unsolved instead of showing the last
    // good answer as though it were current.
    //
    // A conflict makes the solver return `OverConstrained`
    // (no output), so `entity_colours` never runs; without
    // this the stale DoF colours stayed on screen.
    state.last_solve = None;
    // `debug!`, not `warn!`: a drag fails on every pointer tick and a
    // warning here would flush the 200-entry Messages ring. The user hears
    // the failure once, from `report_solve_failure`.
    tracing::debug!(
        target: "signex::sketch",
        error = %error,
        "the sketch solve failed; the previous solve's constraint colours and \
         dimensions were cleared rather than left on screen"
    );
    SketchError::SolveFailed(error)
}
