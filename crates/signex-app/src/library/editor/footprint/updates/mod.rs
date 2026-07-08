//! Update logic for the standalone Footprint editor.
//!
//! `apply_footprint_primitive_edit` is the router: the pre-match `msg`
//! rewrite, the exhaustive dispatch match, and the shared pad/undo helpers.
//! Each concern's arms live in a sibling module and are reached through one
//! `|`-grouped delegating arm per concern (ADR-0001 D1/D2). The former
//! monolithic `sketch` module is itself now split by sketch concern
//! (ui / placement / entities / pad-bridge / constraints / tools):
//!
//!   sketch_{ui,placement,entities,pad_bridge,constraints,tools}
//!   · active_bar · geometry · selection · context_menu · view

mod active_bar;
mod context_menu;
mod geometry;
mod selection;
mod sketch_constraints;
mod sketch_entities;
mod sketch_pad_bridge;
mod sketch_placement;
mod sketch_tools;
mod sketch_ui;
mod view;

use crate::library::messages::PrimitiveEditorMsg;

/// v0.14 — apply an [`AlignOp`] to the pads at `indices` in `state`,
/// in place. `step` is the spacing increment (mm) for the
/// Increase/Decrease ops — pass the active grid step. Centre-based
/// throughout: a pad's "position" is its centre, so aligning edges and
/// aligning centres coincide once sizes are equal; for mixed sizes we
/// follow Altium's pad-centre convention (the Properties X/Y is the
/// centre). Callers guarantee `indices` is deduped, in range, and long
/// enough (≥2 for align, ≥3 for distribute).
///
/// [`AlignOp`]: crate::library::editor::footprint::state::AlignOp
fn align_pads(
    state: &mut crate::library::editor::footprint::state::FootprintEditorState,
    indices: &[usize],
    op: crate::library::editor::footprint::state::AlignOp,
    step: f64,
) {
    // Snapshot centres in selection order, run the pure geometry, then
    // write the results back to the same pads. Indexing is safe — the
    // caller guarantees every index is in range.
    let centres: Vec<(f64, f64)> = indices.iter().map(|&i| state.pads[i].position_mm).collect();
    let out = apply_align(&centres, op, step);
    for (&i, &new_centre) in indices.iter().zip(out.iter()) {
        state.pads[i].position_mm = new_centre;
    }
}

/// Pure geometry for [`align_pads`]: take pad CENTRES (in selection
/// order), apply `op`, and return the new centres in the SAME order.
/// `step` is the spacing increment (mm) used only by the
/// Increase/Decrease ops. Factored out as a free function so the
/// geometry is unit-testable without a `FootprintEditorState`.
///
/// Conventions (Altium pad-centre parity):
/// - Left/Right/Top/Bottom move every centre to the extreme centre on
///   that axis (min/max). The cross axis is untouched, which is why the
///   plain and "maintain spacing" variants share an op.
/// - CenterH/CenterV move centres to the selection mean on that axis.
/// - DistributeH/V keep the two extreme pads fixed and re-space the
///   middles at equal centre-to-centre gaps, preserving left→right /
///   top→bottom order.
/// - Increase/Decrease grow/shrink every gap by `step` (span changes by
///   `step*(n-1)`), pivoting about the mean so the centroid is fixed.
fn apply_align(
    centres: &[(f64, f64)],
    op: crate::library::editor::footprint::state::AlignOp,
    step: f64,
) -> Vec<(f64, f64)> {
    use crate::library::editor::footprint::state::AlignOp;

    let mut out = centres.to_vec();
    let n = centres.len();
    if n == 0 {
        return out;
    }

    let min_x = centres.iter().map(|c| c.0).fold(f64::INFINITY, f64::min);
    let max_x = centres
        .iter()
        .map(|c| c.0)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = centres.iter().map(|c| c.1).fold(f64::INFINITY, f64::min);
    let max_y = centres
        .iter()
        .map(|c| c.1)
        .fold(f64::NEG_INFINITY, f64::max);
    let mean_x = centres.iter().map(|c| c.0).sum::<f64>() / n as f64;
    let mean_y = centres.iter().map(|c| c.1).sum::<f64>() / n as f64;

    match op {
        AlignOp::Left => out.iter_mut().for_each(|c| c.0 = min_x),
        AlignOp::Right => out.iter_mut().for_each(|c| c.0 = max_x),
        AlignOp::Top => out.iter_mut().for_each(|c| c.1 = min_y),
        AlignOp::Bottom => out.iter_mut().for_each(|c| c.1 = max_y),
        AlignOp::CenterH => out.iter_mut().for_each(|c| c.0 = mean_x),
        AlignOp::CenterV => out.iter_mut().for_each(|c| c.1 = mean_y),
        AlignOp::DistributeH => {
            // Rank the selection by X, then place each at an equal gap
            // between the fixed extremes.
            let mut order: Vec<usize> = (0..n).collect();
            order.sort_by(|&a, &b| {
                centres[a]
                    .0
                    .partial_cmp(&centres[b].0)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            let gap = (max_x - min_x) / (n as f64 - 1.0);
            for (rank, &idx) in order.iter().enumerate() {
                out[idx].0 = min_x + gap * rank as f64;
            }
        }
        AlignOp::DistributeV => {
            let mut order: Vec<usize> = (0..n).collect();
            order.sort_by(|&a, &b| {
                centres[a]
                    .1
                    .partial_cmp(&centres[b].1)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            let gap = (max_y - min_y) / (n as f64 - 1.0);
            for (rank, &idx) in order.iter().enumerate() {
                out[idx].1 = min_y + gap * rank as f64;
            }
        }
        AlignOp::IncreaseHSpacing => scale_axis(&mut out, SpacingAxis::X, mean_x, step, true),
        AlignOp::DecreaseHSpacing => scale_axis(&mut out, SpacingAxis::X, mean_x, step, false),
        AlignOp::IncreaseVSpacing => scale_axis(&mut out, SpacingAxis::Y, mean_y, step, true),
        AlignOp::DecreaseVSpacing => scale_axis(&mut out, SpacingAxis::Y, mean_y, step, false),
    }
    out
}

/// Axis selector for [`scale_axis`].
#[derive(Clone, Copy)]
enum SpacingAxis {
    X,
    Y,
}

/// Expand (`expand=true`) or contract the centre-to-centre gaps of
/// `centres` along `axis`, pivoting about `pivot`. Every gap changes by
/// `step`, so the outermost span changes by `step*(n-1)`; relative
/// spacing is preserved by scaling each offset from the pivot by
/// `new_span / old_span`. Contract is clamped so the span never goes
/// negative. No-op when all centres are coincident on that axis.
fn scale_axis(centres: &mut [(f64, f64)], axis: SpacingAxis, pivot: f64, step: f64, expand: bool) {
    let get = |c: &(f64, f64)| match axis {
        SpacingAxis::X => c.0,
        SpacingAxis::Y => c.1,
    };
    let lo = centres.iter().map(get).fold(f64::INFINITY, f64::min);
    let hi = centres.iter().map(get).fold(f64::NEG_INFINITY, f64::max);
    let old_span = hi - lo;
    if old_span <= f64::EPSILON {
        return;
    }
    let n = centres.len();
    let delta = step * (n as f64 - 1.0);
    let new_span = if expand {
        old_span + delta
    } else {
        (old_span - delta).max(0.0)
    };
    let factor = new_span / old_span;
    for c in centres.iter_mut() {
        let scaled = pivot + (get(c) - pivot) * factor;
        match axis {
            SpacingAxis::X => c.0 = scaled,
            SpacingAxis::Y => c.1 = scaled,
        }
    }
}

#[cfg(test)]
mod align_geometry_tests {
    use super::apply_align;
    use crate::library::editor::footprint::state::AlignOp;

    /// Compare two centre lists with an absolute tolerance.
    fn approx_eq(a: &[(f64, f64)], b: &[(f64, f64)]) {
        assert_eq!(a.len(), b.len(), "length mismatch");
        for (i, (p, q)) in a.iter().zip(b.iter()).enumerate() {
            assert!(
                (p.0 - q.0).abs() < 1e-9 && (p.1 - q.1).abs() < 1e-9,
                "centre {i} mismatch: got {p:?}, want {q:?}"
            );
        }
    }

    #[test]
    fn align_left_moves_all_x_to_min() {
        let pads = vec![(2.0, 0.0), (5.0, 1.0), (-1.0, 3.0)];
        let out = apply_align(&pads, AlignOp::Left, 1.0);
        // Every centre X → min (-1.0); Y untouched.
        approx_eq(&out, &[(-1.0, 0.0), (-1.0, 1.0), (-1.0, 3.0)]);
    }

    #[test]
    fn align_right_moves_all_x_to_max() {
        let pads = vec![(2.0, 0.0), (5.0, 1.0), (-1.0, 3.0)];
        let out = apply_align(&pads, AlignOp::Right, 1.0);
        approx_eq(&out, &[(5.0, 0.0), (5.0, 1.0), (5.0, 3.0)]);
    }

    #[test]
    fn align_top_bottom_move_y_only() {
        let pads = vec![(0.0, 2.0), (1.0, 8.0), (2.0, -4.0)];
        let top = apply_align(&pads, AlignOp::Top, 1.0);
        approx_eq(&top, &[(0.0, -4.0), (1.0, -4.0), (2.0, -4.0)]);
        let bottom = apply_align(&pads, AlignOp::Bottom, 1.0);
        approx_eq(&bottom, &[(0.0, 8.0), (1.0, 8.0), (2.0, 8.0)]);
    }

    #[test]
    fn center_h_v_align_to_mean() {
        let pads = vec![(0.0, 0.0), (4.0, 10.0)];
        let ch = apply_align(&pads, AlignOp::CenterH, 1.0);
        // mean X = 2.0
        approx_eq(&ch, &[(2.0, 0.0), (2.0, 10.0)]);
        let cv = apply_align(&pads, AlignOp::CenterV, 1.0);
        // mean Y = 5.0
        approx_eq(&cv, &[(0.0, 5.0), (4.0, 5.0)]);
    }

    #[test]
    fn distribute_h_equalises_gaps_and_keeps_extremes() {
        // Unevenly spaced: 0, 1, 9 → after distribute: 0, 4.5, 9.
        let pads = vec![(0.0, 0.0), (1.0, 0.0), (9.0, 0.0)];
        let out = apply_align(&pads, AlignOp::DistributeH, 1.0);
        approx_eq(&out, &[(0.0, 0.0), (4.5, 0.0), (9.0, 0.0)]);
        // Gaps are now equal.
        let g1 = out[1].0 - out[0].0;
        let g2 = out[2].0 - out[1].0;
        assert!((g1 - g2).abs() < 1e-9);
    }

    #[test]
    fn distribute_h_preserves_input_order_when_unsorted() {
        // Input not sorted by X; extremes (0 and 8) stay, middle (idx 0,
        // X=6) gets re-placed at the equal-gap slot for its rank.
        let pads = vec![(6.0, 0.0), (0.0, 0.0), (8.0, 0.0)];
        let out = apply_align(&pads, AlignOp::DistributeH, 1.0);
        // Ranks by X: idx1(0) → 0, idx0(6) → 4, idx2(8) → 8.
        approx_eq(&out, &[(4.0, 0.0), (0.0, 0.0), (8.0, 0.0)]);
    }

    #[test]
    fn distribute_v_equalises_gaps() {
        let pads = vec![(0.0, 0.0), (0.0, 2.0), (0.0, 10.0)];
        let out = apply_align(&pads, AlignOp::DistributeV, 1.0);
        approx_eq(&out, &[(0.0, 0.0), (0.0, 5.0), (0.0, 10.0)]);
    }

    #[test]
    fn increase_h_spacing_grows_span_by_step_times_gaps() {
        // 3 pads at x = 0, 5, 10; mean = 5; step = 1 → each of 2 gaps
        // grows by 1 → new span 12, centred on 5 → x = -1, 5, 11.
        let pads = vec![(0.0, 0.0), (5.0, 0.0), (10.0, 0.0)];
        let out = apply_align(&pads, AlignOp::IncreaseHSpacing, 1.0);
        approx_eq(&out, &[(-1.0, 0.0), (5.0, 0.0), (11.0, 0.0)]);
    }

    #[test]
    fn decrease_h_spacing_shrinks_span() {
        // Inverse of the increase case: span 10 → 8, centred on 5.
        let pads = vec![(0.0, 0.0), (5.0, 0.0), (10.0, 0.0)];
        let out = apply_align(&pads, AlignOp::DecreaseHSpacing, 1.0);
        approx_eq(&out, &[(1.0, 0.0), (5.0, 0.0), (9.0, 0.0)]);
    }

    #[test]
    fn decrease_spacing_clamps_at_zero_span() {
        // Over-contracting must not invert the order or go negative.
        let pads = vec![(0.0, 0.0), (1.0, 0.0)];
        // One gap, step huge → new span clamped to 0 → both at mean 0.5.
        let out = apply_align(&pads, AlignOp::DecreaseHSpacing, 100.0);
        approx_eq(&out, &[(0.5, 0.0), (0.5, 0.0)]);
    }

    #[test]
    fn increase_v_spacing_grows_vertical_span() {
        let pads = vec![(0.0, 0.0), (0.0, 5.0), (0.0, 10.0)];
        let out = apply_align(&pads, AlignOp::IncreaseVSpacing, 1.0);
        approx_eq(&out, &[(0.0, -1.0), (0.0, 5.0), (0.0, 11.0)]);
    }
}

/// v0.26-E — apply Cut / Copy / Paste against the document-level
/// `pad_clipboard`. Split-borrowed at the call site so both the
/// editor and the clipboard slot are mutable.
///
/// Behaviour:
///  - **Copy**: clones the selected pad into the clipboard. No-op
///    when nothing is selected.
///  - **Cut**: Copy + delete; mirrors into the sketch + invalidates
///    the canvas cache.
///  - **Paste**: places a clone of the clipboard pad at the cursor
///    (or `original.position + (1mm, 1mm)` if cursor is unknown),
///    picks a free designator (max + 1), pre-computes a fresh
///    `sketch_entity_id` so the new pad mirrors into the sketch on
///    its first edit, and selects the new pad post-paste.
pub(crate) fn apply_footprint_clipboard_op(
    editor: &mut crate::app::FootprintEditorState,
    clipboard: &mut Option<crate::library::editor::footprint::state::EditorPad>,
    msg: &PrimitiveEditorMsg,
) {
    use crate::library::editor::footprint::pad_to_sketch;
    use crate::library::editor::footprint::state::FootprintEditorState as CanvasState;

    match msg {
        PrimitiveEditorMsg::FootprintCopyPad => {
            let Some(idx) = editor.state.selected_pad else {
                return;
            };
            let Some(pad) = editor.state.pads.get(idx) else {
                return;
            };
            *clipboard = Some(pad.clone());
        }
        PrimitiveEditorMsg::FootprintCutPad => {
            let Some(idx) = editor.state.selected_pad else {
                return;
            };
            // Snapshot history BEFORE the mutation so undo restores
            // the pad. Mirrors the v0.24 push_history pattern.
            editor.push_history();
            let did_delete = editor.with_parts(|state, primitive| {
                let Some(pad) = state.pads.get(idx).cloned() else {
                    return false;
                };
                *clipboard = Some(pad.clone());
                pad_to_sketch::mirror_delete_pad_from_sketch(&pad, primitive);
                state.delete_pad(idx);
                CanvasState::sync_pads_to_primitive(state, primitive);
                true
            });
            if did_delete {
                editor.canvas_cache.clear();
                editor.dirty = true;
            }
        }
        PrimitiveEditorMsg::FootprintPastePad => {
            let Some(template) = clipboard.clone() else {
                return;
            };
            // Paste position: prefer the cursor; fall back to the
            // template''s original + a tiny diagonal offset so the
            // user sees the new pad rather than overlap.
            let (px, py) = match editor.state.cursor_mm {
                Some((cx, cy)) => (cx, cy),
                None => (template.position_mm.0 + 1.0, template.position_mm.1 + 1.0),
            };
            // Pick a designator: max-existing + 1, falling back to
            // the template''s number when nothing parses.
            let next_num = editor
                .state
                .pads
                .iter()
                .filter_map(|p| p.number.parse::<u64>().ok())
                .max()
                .map(|n| (n + 1).to_string())
                .unwrap_or_else(|| template.number.clone());
            editor.push_history();
            editor.with_parts(|state, primitive| {
                let mut new_pad = template.clone();
                new_pad.position_mm = (px, py);
                new_pad.number = next_num.clone();
                // Reset sketch links so the pad mirrors freshly into
                // the sketch on the next mode switch (avoids two
                // pads sharing an entity id).
                new_pad.sketch_entity_id = None;
                new_pad.corner_entity_ids = None;
                state.pads.push(new_pad);
                let new_idx = state.pads.len() - 1;
                state.selected_pad = Some(new_idx);
                state.recompute_courtyard();
                CanvasState::sync_pads_to_primitive(state, primitive);
            });
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        _ => {}
    }
}

/// Apply a primitive-editor event to a standalone Footprint editor
/// state. Mirrors the footprint-tab arms of `apply_inline_edit` but
/// against the path-keyed standalone state.
pub(crate) fn apply_footprint_primitive_edit(
    editor: &mut crate::app::FootprintEditorState,
    msg: PrimitiveEditorMsg,
) {
    // v0.27 — Role=Pad on a Line is shorthand for "make this loop a
    // pad." Rewrite the message here so the SetRole arm only ever
    // sees Point-targeted Pad role assignments (where it makes
    // sense). Without this, Role=Pad on a Line was a silent no-op
    // — the Properties dropdown read as broken.
    let msg = if let PrimitiveEditorMsg::FootprintSketchSetRole {
        id,
        role: crate::library::messages::RoleTag::Pad,
    } = &msg
    {
        let is_line = editor
            .primitive()
            .sketch
            .as_ref()
            .and_then(|s| s.entities.iter().find(|e| e.id == *id))
            .map(|e| matches!(e.kind, signex_sketch::entity::EntityKind::Line { .. }))
            .unwrap_or(false);
        if is_line {
            editor.state.selected_sketch = Some(*id);
            PrimitiveEditorMsg::FootprintSketchMakePadFromProfile
        } else {
            msg
        }
    } else {
        msg
    };

    // v0.24 Phase 1 (Track B) — capture an undo snapshot ahead of
    // any mutating message. Selection-only / cursor-tracking /
    // tool-state messages are pure UI state and don't need history;
    // everything else gets a snapshot so Ctrl+Z reverses it. The
    // dispatcher is the canonical entry point for footprint
    // mutations, so wrapping here covers every message type
    // uniformly without each arm needing its own push.
    if mutates_footprint_state(&msg) {
        editor.push_history();
    }

    match msg {
        PrimitiveEditorMsg::FootprintSelectActiveIdx(..)
        | PrimitiveEditorMsg::FootprintToggleSelectionFilter(..)
        | PrimitiveEditorMsg::FootprintSelectSilkF(..)
        | PrimitiveEditorMsg::FootprintDeleteSilkF
        | PrimitiveEditorMsg::FootprintMovePad { .. }
        | PrimitiveEditorMsg::FootprintCursorAt { .. }
        | PrimitiveEditorMsg::FootprintSelectPad(..)
        | PrimitiveEditorMsg::FootprintSelectPads(..)
        | PrimitiveEditorMsg::FootprintDeleteSelected
        | PrimitiveEditorMsg::FootprintSetSelectionMode2d(..)
        | PrimitiveEditorMsg::FootprintSelectAllOnLayer
        | PrimitiveEditorMsg::FootprintLassoArm
        | PrimitiveEditorMsg::FootprintLassoAddVertex { .. }
        | PrimitiveEditorMsg::FootprintLassoCancel
        | PrimitiveEditorMsg::FootprintLassoCommit
        | PrimitiveEditorMsg::FootprintTouchingLineArm
        | PrimitiveEditorMsg::FootprintTouchingLineFirst { .. }
        | PrimitiveEditorMsg::FootprintTouchingLineCancel
        | PrimitiveEditorMsg::FootprintTouchingLineCommit { .. }
        | PrimitiveEditorMsg::FootprintSelectOverlapped
        | PrimitiveEditorMsg::FootprintSelectNextOverlapped
        | PrimitiveEditorMsg::FootprintSelectOffGridPads => selection::apply(editor, msg),

        PrimitiveEditorMsg::FootprintAddNewSibling
        | PrimitiveEditorMsg::FootprintAddPad { .. }
        | PrimitiveEditorMsg::FootprintAddVia { .. }
        | PrimitiveEditorMsg::FootprintTrackClick { .. }
        | PrimitiveEditorMsg::FootprintTrackCancel
        | PrimitiveEditorMsg::FootprintArcClick { .. }
        | PrimitiveEditorMsg::FootprintArcCancel
        | PrimitiveEditorMsg::FootprintPolygonClick { .. }
        | PrimitiveEditorMsg::FootprintPolygonCommit
        | PrimitiveEditorMsg::FootprintPolygonCancel
        | PrimitiveEditorMsg::FootprintAddText { .. }
        | PrimitiveEditorMsg::FootprintAddTextFrame { .. }
        | PrimitiveEditorMsg::FootprintAddHole { .. }
        | PrimitiveEditorMsg::FootprintMintBody3d
        | PrimitiveEditorMsg::FootprintMintExtrudedBody3d => geometry::apply(editor, msg),
        PrimitiveEditorMsg::FootprintSketchSelectMany(..)
        | PrimitiveEditorMsg::FootprintSketchSelect { .. }
        | PrimitiveEditorMsg::FootprintSketchSetTool(..)
        | PrimitiveEditorMsg::FootprintSketchToggleConstruction
        | PrimitiveEditorMsg::FootprintSketchToggleCenterline
        | PrimitiveEditorMsg::FootprintSketchToolEscape
        | PrimitiveEditorMsg::FootprintSketchDimensionInput(..) => sketch_ui::apply(editor, msg),
        PrimitiveEditorMsg::FootprintSketchPlacementInputChar(..)
        | PrimitiveEditorMsg::FootprintSketchPlacementInputBackspace
        | PrimitiveEditorMsg::FootprintSketchPlacementInputEnter
        | PrimitiveEditorMsg::FootprintSketchPlacementInputEscape
        | PrimitiveEditorMsg::FootprintSketchPlacementInputTab => {
            sketch_placement::apply(editor, msg)
        }
        PrimitiveEditorMsg::FootprintSketchPlacePoint { .. }
        | PrimitiveEditorMsg::FootprintSketchMovePoint { .. }
        | PrimitiveEditorMsg::FootprintSketchMoveLine { .. }
        | PrimitiveEditorMsg::FootprintSketchResizeRoundPad { .. } => {
            sketch_entities::apply(editor, msg)
        }
        PrimitiveEditorMsg::FootprintSketchSetRole { .. }
        | PrimitiveEditorMsg::FootprintSketchMakePadFromProfile
        | PrimitiveEditorMsg::FootprintSketchUnlinkCornerRadius { .. } => {
            sketch_pad_bridge::apply(editor, msg)
        }
        PrimitiveEditorMsg::FootprintSketchEditParameter { .. }
        | PrimitiveEditorMsg::FootprintSketchAddConstraintForSelection(..) => {
            sketch_constraints::apply(editor, msg)
        }
        PrimitiveEditorMsg::FootprintSketchToolClick { .. } => sketch_tools::apply(editor, msg),
        PrimitiveEditorMsg::FootprintToggleLayer(..)
        | PrimitiveEditorMsg::FootprintToggleAutoFit
        | PrimitiveEditorMsg::FootprintSetMode(..)
        | PrimitiveEditorMsg::FootprintTogglePlacementPause
        | PrimitiveEditorMsg::FootprintFitConsumed
        | PrimitiveEditorMsg::FootprintCopyPad
        | PrimitiveEditorMsg::FootprintCutPad
        | PrimitiveEditorMsg::FootprintPastePad
        | PrimitiveEditorMsg::FootprintSetPadsTool(..)
        | PrimitiveEditorMsg::FootprintToolEscape
        | PrimitiveEditorMsg::FootprintAlignPads(..)
        | PrimitiveEditorMsg::FootprintSetName(..)
        | PrimitiveEditorMsg::FootprintRecomputeCourtyardOutline => view::apply(editor, msg),

        PrimitiveEditorMsg::FootprintShowContextMenu { .. }
        | PrimitiveEditorMsg::FootprintCloseContextMenu
        | PrimitiveEditorMsg::FootprintContextMenuOpenSubmenu(..)
        | PrimitiveEditorMsg::FootprintContextMenuAction(..) => context_menu::apply(editor, msg),
        PrimitiveEditorMsg::FootprintToggleActiveBarMenu(..)
        | PrimitiveEditorMsg::FootprintCloseActiveBarMenu
        | PrimitiveEditorMsg::FootprintActiveBarStub(..)
        | PrimitiveEditorMsg::FootprintApplyFilterPreset(..)
        | PrimitiveEditorMsg::FootprintToggleAllFilters
        | PrimitiveEditorMsg::FootprintCaptureFilterPreset
        | PrimitiveEditorMsg::FootprintActiveBarToggleSnap(..)
        | PrimitiveEditorMsg::FootprintActiveBarSetSnappingMode(..)
        | PrimitiveEditorMsg::FootprintActiveBarSetSnapSubTab(..)
        | PrimitiveEditorMsg::FootprintActiveBarRotateSelection
        | PrimitiveEditorMsg::FootprintActiveBarFlipSelection
        | PrimitiveEditorMsg::FootprintActiveBarNudgeSelection
        | PrimitiveEditorMsg::FootprintMoveByOpen
        | PrimitiveEditorMsg::FootprintMoveBySetX(..)
        | PrimitiveEditorMsg::FootprintMoveBySetY(..)
        | PrimitiveEditorMsg::FootprintMoveByConfirm
        | PrimitiveEditorMsg::FootprintMoveByCancel
        | PrimitiveEditorMsg::FootprintActiveBarAlignSelectionToGrid
        | PrimitiveEditorMsg::FootprintActiveBarMoveOriginToGrid
        | PrimitiveEditorMsg::FootprintActiveBarSelectAll
        | PrimitiveEditorMsg::FootprintActiveBarClearSelection
        | PrimitiveEditorMsg::FootprintActiveBarSetSketchTool(..) => active_bar::apply(editor, msg),
        // Symbol variants are no-ops on a Footprint editor.
        PrimitiveEditorMsg::SymbolSetTool(_)
        | PrimitiveEditorMsg::SymbolAddPin { .. }
        | PrimitiveEditorMsg::SymbolAddRectangle { .. }
        | PrimitiveEditorMsg::SymbolAddLine { .. }
        | PrimitiveEditorMsg::SymbolAddCircle { .. }
        | PrimitiveEditorMsg::SymbolAddArc { .. }
        | PrimitiveEditorMsg::SymbolAddText { .. }
        | PrimitiveEditorMsg::SymbolSelect(_)
        | PrimitiveEditorMsg::SymbolDeselect
        | PrimitiveEditorMsg::SymbolMoveSelected { .. }
        | PrimitiveEditorMsg::SymbolMoveGraphicHandle { .. }
        | PrimitiveEditorMsg::SymbolDeleteSelected
        | PrimitiveEditorMsg::SymbolSetPinNumber { .. }
        | PrimitiveEditorMsg::SymbolSetPinName { .. }
        | PrimitiveEditorMsg::SymbolPrevPart
        | PrimitiveEditorMsg::SymbolNextPart
        | PrimitiveEditorMsg::SymbolNewPart
        | PrimitiveEditorMsg::SymbolRemovePart
        | PrimitiveEditorMsg::SymbolPan { .. }
        | PrimitiveEditorMsg::SymbolZoom { .. }
        | PrimitiveEditorMsg::SymbolFit
        | PrimitiveEditorMsg::SymbolCursorAt { .. }
        | PrimitiveEditorMsg::SymbolSetSheetColor(_)
        | PrimitiveEditorMsg::SymbolToggleGrid
        | PrimitiveEditorMsg::SymbolCycleGridSize
        | PrimitiveEditorMsg::SymbolCycleUnit
        | PrimitiveEditorMsg::SymbolToggleActiveBarMenu(_)
        | PrimitiveEditorMsg::SymbolCloseActiveBarMenu
        | PrimitiveEditorMsg::SymbolActiveBarStub(_)
        | PrimitiveEditorMsg::SymbolToggleSelectionFilter(_)
        | PrimitiveEditorMsg::SymbolMoveAll { .. }
        | PrimitiveEditorMsg::SymbolRotateSelected { .. }
        | PrimitiveEditorMsg::SymbolUndo
        | PrimitiveEditorMsg::SymbolRedo
        | PrimitiveEditorMsg::SymbolDragCommit
        | PrimitiveEditorMsg::Save => {}
    }
}

/// Translate the current pad selection by (dx, dy) mm: history
/// snapshot, tested `nudge_pads`, sketch mirror, primitive re-sync.
/// No-op on an empty selection. Shared by the one-step
/// `FootprintActiveBarNudgeSelection` nudge and the typed-delta
/// Move-By modal (`FootprintMoveByConfirm`) so both paths share the
/// exact same proven geometry + sketch-mirror + history behaviour.
fn footprint_nudge_selection(editor: &mut crate::app::FootprintEditorState, dx: f64, dy: f64) {
    use crate::library::editor::footprint::pad_to_sketch;
    use crate::library::editor::footprint::state::FootprintEditorState as CanvasState;

    let mut indices: Vec<usize> = Vec::new();
    if let Some(p) = editor.state.selected_pad {
        indices.push(p);
    }
    indices.extend(editor.state.selected_pads_extra.iter().copied());
    indices.sort_unstable();
    indices.dedup();
    indices.retain(|&i| i < editor.state.pads.len());
    if indices.is_empty() {
        return;
    }

    editor.push_history();
    editor.with_parts(|state, primitive| {
        // Translate the selection via the tested state helper, then
        // mirror exactly the moved pads into the backing sketch and
        // re-sync the literal `Pad` list.
        let moved = state.nudge_pads(&indices, dx, dy);
        let snapshots: Vec<crate::library::editor::footprint::state::EditorPad> = moved
            .iter()
            .filter_map(|&i| state.pads.get(i).cloned())
            .collect();
        for snapshot in &snapshots {
            pad_to_sketch::mirror_move_pad_in_sketch(snapshot, primitive);
        }
        CanvasState::sync_pads_to_primitive(state, primitive);
    });
    editor.canvas_cache.clear();
    editor.dirty = true;
}

/// v0.24 Phase 1 (Track B) — message-kind classifier driving the
/// `push_history` decision in [`apply_footprint_primitive_edit`].
/// Returns `true` for messages that mutate persisted footprint /
/// sketch state (so undo can roll them back), `false` for pure UI
/// state (selection, cursor tracking, tool mode toggles, panel
/// pickers — these don't enter the history because rolling back a
/// "click happened here" doesn't make sense to the user).
///
/// Lean toward `true` when in doubt — extra history entries cost
/// memory but never break correctness; missing entries leave edits
/// unreversable.
fn mutates_footprint_state(msg: &PrimitiveEditorMsg) -> bool {
    use PrimitiveEditorMsg::*;
    match msg {
        // Pure UI state — selection / hover / cursor / tool mode.
        // These don't change persisted geometry and shouldn't enter
        // the history.
        FootprintCursorAt { .. }
        | FootprintSelectPad(_)
        | FootprintSelectSilkF(_)
        | FootprintToggleLayer(_)
        | FootprintSetPadsTool(_)
        | FootprintToolEscape
        | FootprintToggleActiveBarMenu(_)
        | FootprintCloseActiveBarMenu
        | FootprintActiveBarStub(_)
        | FootprintActiveBarToggleSnap(_)
        | FootprintActiveBarSetSnappingMode(_)
        | FootprintActiveBarSetSnapSubTab(_)
        | FootprintActiveBarSelectAll
        | FootprintActiveBarClearSelection
        | FootprintActiveBarSetSketchTool(_)
        | FootprintSetMode(_)
        | FootprintSketchSetTool(_)
        | FootprintSketchToggleConstruction
        | FootprintSketchToggleCenterline
        | FootprintTogglePlacementPause
        | FootprintSketchToolEscape
        // v0.24 Track D — placement-input keypress messages mutate
        // only the transient `placement_input` overlay buffer; they
        // don't touch persisted geometry, so undo doesn't need them.
        | FootprintSketchPlacementInputChar(_)
        | FootprintSketchPlacementInputBackspace
        | FootprintSketchPlacementInputEnter
        | FootprintSketchPlacementInputEscape
        | FootprintSketchSelect { .. }
        | FootprintSketchDimensionInput(_)
        | FootprintToggleSelectionFilter(_)
        // Task 6 — filter preset apply/toggle/capture are UI-only
        // (they mutate `selection_filter` or the on-disk preset
        // list, never persisted footprint geometry), so they must
        // not enter the undo history like the other filter toggles
        // above.
        | FootprintApplyFilterPreset(_)
        | FootprintToggleAllFilters
        | FootprintCaptureFilterPreset
        | FootprintToggleAutoFit
        | FootprintSelectActiveIdx(_)
        | FootprintShowContextMenu { .. }
        | FootprintCloseContextMenu
        | FootprintContextMenuOpenSubmenu(_)
        | FootprintContextMenuAction(_)
        | FootprintFitConsumed
        // v0.26-E — clipboard ops handle their own push_history at
        // call site, so the snapshot-classifier here returns false
        // (Copy mutates nothing; Cut + Paste already snapshotted).
        | FootprintCopyPad
        | FootprintCutPad
        | FootprintPastePad
        // v0.14 — Align/Distribute/Spacing pushes its own snapshot
        // inside the handler, gated on a large-enough selection, so the
        // blanket pre-push here must NOT fire (it would double-stack the
        // history and snapshot even on a sub-2-pad no-op).
        | FootprintAlignPads(_)
        // v0.14 — "Move Selection by X, Y…" (nudge) likewise pushes its
        // own snapshot inside the handler, gated on a non-empty
        // selection. Keep it out of the blanket pre-push to avoid
        // double-stacking the history on an empty-selection no-op.
        | FootprintActiveBarNudgeSelection
        // v0.14 — Move-By modal open/edit/cancel are pure UI state (the
        // typed buffers live on `move_by_modal`, not persisted
        // geometry); Confirm pushes its own snapshot inside the shared
        // `footprint_nudge_selection` helper, same as the one-step
        // nudge above, so it's classified alongside it here too.
        | FootprintMoveByOpen
        | FootprintMoveBySetX(_)
        | FootprintMoveBySetY(_)
        | FootprintMoveByConfirm
        | FootprintMoveByCancel
        // v0.14 — 3D Body mint pushes its own snapshot inside the
        // handler (unconditionally, unlike nudge). Keep it out of the
        // blanket pre-push to avoid double-stacking the history.
        | FootprintMintBody3d
        | FootprintMintExtrudedBody3d
        // v0.14 — Place Text Frame commits once, on release, with
        // the drag already resolved (no intermediate anchor-click
        // message reaches the dispatcher like Track's 2-click
        // gesture does). It pushes its own snapshot inside the
        // handler, so keep it out of the blanket pre-push.
        | FootprintAddTextFrame { .. }
        | Save => false,
        // All other variants either add/remove/move geometry,
        // mutate pad attributes, or rebuild the sketch — they all
        // need a history snapshot.
        _ => true,
    }
}
