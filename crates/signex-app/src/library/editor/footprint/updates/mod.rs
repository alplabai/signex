//! Update logic for the standalone Footprint editor.
//!
//! NOTE (reducer decomposition, step 3a): this module currently holds the
//! ~5.5k-line `apply_footprint_primitive_edit` match verbatim, relocated
//! out of the `dispatch/library.rs` monolith. It is being decomposed into
//! per-concern modules (sketch / active_bar / tracks / lasso / pads /
//! context-menu / layers) per ADR-0001 D1/D2 in follow-up commits. The
//! match reaches everything else through fully-qualified paths, so the
//! only import it needs today is the message enum it dispatches on.

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
    let centres: Vec<(f64, f64)> = indices
        .iter()
        .map(|&i| state.pads[i].position_mm)
        .collect();
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
    let max_x = centres.iter().map(|c| c.0).fold(f64::NEG_INFINITY, f64::max);
    let min_y = centres.iter().map(|c| c.1).fold(f64::INFINITY, f64::min);
    let max_y = centres.iter().map(|c| c.1).fold(f64::NEG_INFINITY, f64::max);
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
    use crate::library::editor::footprint::layers::FpLayer;
    use crate::library::editor::footprint::state::FootprintEditorState as CanvasState;

    use crate::library::editor::footprint::pad_to_sketch;

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

    /// v0.15 — gate the Pads → Sketch mirror on whether the
    /// footprint already has a sketch (i.e. the user has visited
    /// Sketch mode at least once OR auto-mint has already fired).
    /// Mirroring into a non-existent sketch would create one
    /// silently, which is undesirable for users who only ever work
    /// in Pads mode.
    fn footprint_sketch_is_active(fp: &signex_library::primitive::footprint::Footprint) -> bool {
        match fp.sketch.as_ref() {
            Some(s) => !s.entities.is_empty(),
            None => false,
        }
    }

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
        // v0.18.7 — switch the active footprint within the multi-
        // footprint envelope. Resets the canvas pad list off the
        // newly-active primitive, clears selection, refits the
        // camera on the next frame so a different-sized footprint
        // doesn't open at a stale zoom.
        PrimitiveEditorMsg::FootprintSelectActiveIdx(idx) => {
            let last = editor.file.footprints.len().saturating_sub(1);
            let clamped = idx.min(last);
            if clamped == editor.active_idx {
                return;
            }
            editor.active_idx = clamped;
            // Re-derive the canvas-side state from the new active
            // primitive so pads / sketch / courtyard mirror what's
            // on disk for this footprint.
            editor.state =
                crate::library::editor::footprint::state::FootprintEditorState::from_footprint(
                    editor.primitive(),
                );
            editor.canvas_cache.clear();
        }
        // v0.18.7 — append a fresh empty footprint to the envelope
        // and switch onto it. Names the new sibling `Footprint N`
        // where N counts existing siblings + 1; the user can rename
        // via the Properties panel.
        PrimitiveEditorMsg::FootprintAddNewSibling => {
            let next_n = editor.file.footprints.len() + 1;
            let new_fp = signex_library::Footprint::empty(format!("Footprint {next_n}"));
            editor.file.footprints.push(new_fp);
            editor.active_idx = editor.file.footprints.len() - 1;
            editor.state =
                crate::library::editor::footprint::state::FootprintEditorState::from_footprint(
                    editor.primitive(),
                );
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintAddPad { x_mm, y_mm } => {
            // v0.15 — bidirectional Pads → Sketch mirror. The new
            // pad gets a backing sketch Point + PadAttr (when the
            // sketch already has any other backed entity, i.e. the
            // user has been in Sketch mode at least once).
            // v0.18.6 — split-borrow at the top of the arm so
            // `state.pads.get_mut(...)` and `primitive` coexist; both
            // halves originate from disjoint editor fields.
            editor.with_parts(|state, primitive| {
                let idx = state.add_pad_at(x_mm, y_mm);
                if let Some(pad) = state.pads.get_mut(idx) {
                    if footprint_sketch_is_active(primitive) {
                        pad_to_sketch::mirror_add_pad_to_sketch(pad, primitive);
                    }
                }
                CanvasState::sync_pads_to_primitive(state, primitive);
            });
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintAddVia { x_mm, y_mm } => {
            // v0.27 — vias are a small Round plated-through pad. The
            // canonical via geometry is fixed (0.6 mm copper / 0.3 mm
            // drill / Multi-Layer F.Cu+B.Cu+masks) so the user gets a
            // proper via regardless of what `next_pad_defaults` looks
            // like. Bypasses `add_pad_at` (which inherits Pads-mode
            // defaults) and constructs the EditorPad directly.
            use crate::library::editor::footprint::state::EditorPad;
            use signex_library::{LayerId, PadKind, PadShape};
            const VIA_DIAMETER_MM: f64 = 0.6;
            const VIA_DRILL_MM: f64 = 0.3;
            editor.with_parts(|state, primitive| {
                let number = state.next_pad_number();
                let mut pad = EditorPad::new_default(number, (x_mm, y_mm));
                pad.size_mm = (VIA_DIAMETER_MM, VIA_DIAMETER_MM);
                pad.shape = PadShape::Round;
                pad.kind = PadKind::Tht;
                pad.drill_diameter_mm = Some(VIA_DRILL_MM);
                pad.layers = vec![
                    LayerId::new("F.Cu"),
                    LayerId::new("F.Mask"),
                    LayerId::new("B.Cu"),
                    LayerId::new("B.Mask"),
                ];
                state.pads.push(pad);
                let idx = state.pads.len() - 1;
                state.selected_pad = Some(idx);
                if let Some(p) = state.pads.get_mut(idx) {
                    if footprint_sketch_is_active(primitive) {
                        pad_to_sketch::mirror_add_pad_to_sketch(p, primitive);
                    }
                }
                CanvasState::sync_pads_to_primitive(state, primitive);
            });
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        // v0.18.14 — Selection Filter pill toggle from the unified
        // active bar. The panel-side equivalent
        // (`PanelMsg::FpEditorToggleSelectionFilter`) routes through
        // a dedicated handler in `handlers/dock/sch_library`; this
        // arm covers the active-bar dispatch path.
        PrimitiveEditorMsg::FootprintToggleSelectionFilter(kind) => {
            editor.state.selection_filter.toggle(kind);
            editor.canvas_cache.clear();
        }
        // v0.18.15.1 — Place Track 2-click gesture. First click
        // stashes the start in `state.track_first`; second click
        // commits the line to silk_f and chains by re-stashing the
        // second click as the next gesture's start.
        PrimitiveEditorMsg::FootprintTrackClick { x_mm, y_mm } => {
            match editor.state.track_first {
                None => {
                    editor.state.track_first = Some((x_mm, y_mm));
                }
                Some((sx, sy)) => {
                    let primitive = editor.primitive_mut();
                    primitive
                        .silk_f
                        .push(signex_library::primitive::footprint::FpGraphic {
                            kind: signex_library::primitive::footprint::FpGraphicKind::Line {
                                from: [sx, sy],
                                to: [x_mm, y_mm],
                            },
                            stroke_width: 0.15,
                            filled: false,
                        });
                    // Chain — the second click becomes the next
                    // segment's start, matching Altium's stroke-a-
                    // polyline workflow.
                    editor.state.track_first = Some((x_mm, y_mm));
                    editor.dirty = true;
                }
            }
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintTrackCancel => {
            editor.state.track_first = None;
            editor.canvas_cache.clear();
        }
        // v0.18.15.3 — Place Arc 3-click gesture (centre / radius
        // start / sweep end). Idle → Center → Start → commit. After
        // commit the gesture resets to Idle (no chain — arcs
        // typically aren't strung together).
        PrimitiveEditorMsg::FootprintArcClick { x_mm, y_mm } => {
            use crate::library::editor::footprint::state::PlaceArcPending;
            let next = match editor.state.place_arc_pending {
                PlaceArcPending::Idle => PlaceArcPending::Center {
                    center: (x_mm, y_mm),
                },
                PlaceArcPending::Center { center } => PlaceArcPending::Start {
                    center,
                    start: (x_mm, y_mm),
                },
                PlaceArcPending::Start { center, start } => {
                    let (cx, cy) = center;
                    let (sx, sy) = start;
                    let radius = ((sx - cx).powi(2) + (sy - cy).powi(2)).sqrt();
                    if radius > 1e-6 {
                        let start_deg = (sy - cy).atan2(sx - cx).to_degrees();
                        let end_deg = (y_mm - cy).atan2(x_mm - cx).to_degrees();
                        let primitive = editor.primitive_mut();
                        primitive
                            .silk_f
                            .push(signex_library::primitive::footprint::FpGraphic {
                                kind: signex_library::primitive::footprint::FpGraphicKind::Arc {
                                    center: [cx, cy],
                                    radius,
                                    start_deg,
                                    end_deg,
                                },
                                stroke_width: 0.15,
                                filled: false,
                            });
                        editor.dirty = true;
                    }
                    PlaceArcPending::Idle
                }
            };
            editor.state.place_arc_pending = next;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintArcCancel => {
            editor.state.place_arc_pending =
                crate::library::editor::footprint::state::PlaceArcPending::Idle;
            editor.canvas_cache.clear();
        }
        // v0.18.15.4 — Place Polygon multi-click gesture. Each
        // click appends a vertex; commit happens on tool switch /
        // Esc via `FootprintPolygonCommit`.
        PrimitiveEditorMsg::FootprintPolygonClick { x_mm, y_mm } => {
            editor.state.place_polygon_vertices.push((x_mm, y_mm));
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintPolygonCommit => {
            let verts = std::mem::take(&mut editor.state.place_polygon_vertices);
            // v0.18.17 — emit one `Polygon` FpGraphic (instead of
            // N Lines). `filled` follows the active tool —
            // `PlacePolygon` = stroked outline, `PlaceRegion` =
            // solid fill.
            let filled = matches!(
                editor.state.pads_tool,
                crate::library::editor::footprint::state::PadsTool::PlaceRegion
            );
            if verts.len() >= 3 {
                let vertices: Vec<[f64; 2]> = verts.iter().map(|(x, y)| [*x, *y]).collect();
                let primitive = editor.primitive_mut();
                primitive
                    .silk_f
                    .push(signex_library::primitive::footprint::FpGraphic {
                        kind: signex_library::primitive::footprint::FpGraphicKind::Polygon {
                            vertices,
                        },
                        stroke_width: if filled { 0.0 } else { 0.15 },
                        filled,
                    });
                editor.dirty = true;
            }
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintPolygonCancel => {
            editor.state.place_polygon_vertices.clear();
            editor.canvas_cache.clear();
        }
        // v0.18.18 — silk-front graphic selection. Clears
        // selected_pad symmetrically so the Properties panel
        // doesn't try to render two selection-specific bodies at
        // once.
        PrimitiveEditorMsg::FootprintSelectSilkF(sel) => {
            editor.state.selected_silk_f = sel;
            if sel.is_some() {
                editor.state.selected_pad = None;
            }
            editor.canvas_cache.clear();
        }
        // v0.18.18 — delete the selected silk-front graphic.
        // No-op when nothing is selected. Updates `editor.dirty`
        // and clears the selection state.
        PrimitiveEditorMsg::FootprintDeleteSilkF => {
            if let Some(idx) = editor.state.selected_silk_f {
                let primitive = editor.primitive_mut();
                if idx < primitive.silk_f.len() {
                    primitive.silk_f.remove(idx);
                    editor.dirty = true;
                }
                // HI-25: shared selection-adjustment helper — keep
                // `selected_silk_f` valid against the new vec length
                // instead of clearing unconditionally.
                editor.state.selected_silk_f =
                    crate::library::editor::footprint::state::adjust_selection_after_remove(
                        editor.state.selected_silk_f,
                        idx,
                    );
            }
            editor.canvas_cache.clear();
        }
        // v0.18.15 — Place String tool. Appends a silk-layer text
        // label `FpGraphic { kind: Text { position, content: "TEXT",
        // size: 1.0 }, stroke_width: 0.0 }` to the active footprint's
        // `silk_f`. The user edits the content via the Properties
        // panel later (Properties wiring is queued).
        PrimitiveEditorMsg::FootprintAddText { x_mm, y_mm } => {
            let primitive = editor.primitive_mut();
            primitive
                .silk_f
                .push(signex_library::primitive::footprint::FpGraphic {
                    kind: signex_library::primitive::footprint::FpGraphicKind::Text {
                        position: [x_mm, y_mm],
                        content: "TEXT".to_string(),
                        size: 1.0,
                        frame: None,
                    },
                    stroke_width: 0.0,
                    filled: false,
                });
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        // v0.14 — Place Text Frame press-drag-release commit (item
        // ③). Fires once, on release, with the anchor (min corner)
        // and drag size already resolved by the canvas. Pushes its
        // own history snapshot — see `mutates_footprint_state`,
        // classified alongside the 3D Body mint variants — because
        // the intermediate press/drag ticks never reach the
        // dispatcher (unlike Track's 2-click gesture), so there's
        // no risk of double-stacking.
        PrimitiveEditorMsg::FootprintAddTextFrame {
            x_mm,
            y_mm,
            w_mm,
            h_mm,
        } => {
            editor.push_history();
            editor.with_parts(|_state, primitive| {
                crate::library::editor::footprint::text_frame::add_text_frame(
                    primitive, x_mm, y_mm, w_mm, h_mm,
                );
            });
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        // v0.18.12 — Place Hole tool. Drops a non-plated through
        // hole at the cursor (no copper, drill from `next_pad_defaults`).
        PrimitiveEditorMsg::FootprintAddHole { x_mm, y_mm } => {
            editor.with_parts(|state, primitive| {
                let idx = state.add_hole_at(x_mm, y_mm);
                if let Some(pad) = state.pads.get_mut(idx) {
                    if footprint_sketch_is_active(primitive) {
                        pad_to_sketch::mirror_add_pad_to_sketch(pad, primitive);
                    }
                }
                CanvasState::sync_pads_to_primitive(state, primitive);
            });
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintMovePad { idx, x_mm, y_mm } => {
            editor.with_parts(|state, primitive| {
                state.move_pad(idx, x_mm, y_mm);
                // v0.15 — mirror the move into the sketch.
                if let Some(pad) = state.pads.get(idx) {
                    pad_to_sketch::mirror_move_pad_in_sketch(pad, primitive);
                }
                CanvasState::sync_pads_to_primitive(state, primitive);
            });
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintCursorAt { x_mm, y_mm } => {
            editor.state.cursor_mm = Some((x_mm, y_mm));
        }
        PrimitiveEditorMsg::FootprintSelectPad(sel) => {
            editor.state.selected_pad = sel;
            // v0.27 — single-pad select replaces the multi-select
            // extras too. Multi-select uses FootprintSelectPads.
            editor.state.selected_pads_extra.clear();
            // v0.27 — record the click position for Select
            // overlapped / Select next so the dropdown can find
            // the stack at the last-clicked location.
            if sel.is_some() {
                editor.state.last_click_world_mm = editor.state.cursor_mm;
            }
            // v0.18.18 — pad and silk selection are mutually
            // exclusive in the Properties panel; clear the silk
            // selection when a pad is picked.
            if sel.is_some() {
                editor.state.selected_silk_f = None;
            }
            // v0.25 polish — clear verbatim numeric buffers on
            // selection change so a stale "0.1." buffer from one
            // pad doesn't follow the user to the next pad's input.
            editor.state.numeric_buffers.clear();
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintSelectPads(mut pads) => {
            // v0.27 — Altium-parity multi-select. Empty list = clear.
            // First entry becomes the primary (drives Properties);
            // rest land in `selected_pads_extra` for highlight only.
            // Dedupe so a sloppy caller passing [3, 3, 7] still gets
            // [3, 7] selected.
            pads.sort_unstable();
            pads.dedup();
            if pads.is_empty() {
                editor.state.selected_pad = None;
                editor.state.selected_pads_extra.clear();
            } else {
                editor.state.selected_pad = Some(pads[0]);
                editor.state.selected_pads_extra = pads[1..].to_vec();
                editor.state.selected_silk_f = None;
                editor.state.last_click_world_mm = editor.state.cursor_mm;
            }
            editor.state.numeric_buffers.clear();
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintSketchSelectMany(ids) => {
            // v0.27 — Sketch-mode multi-select replacement. First
            // entity is primary (drives the inspector + DOF
            // overlay focus); the second slots into the secondary
            // (used by the constraint submenu's "two entities"
            // pairing); the rest land in extras. Empty list
            // deselects everything.
            if ids.is_empty() {
                editor.state.selected_sketch = None;
                editor.state.selected_sketch_secondary = None;
                editor.state.selected_sketch_extra.clear();
            } else {
                editor.state.selected_sketch = Some(ids[0]);
                editor.state.selected_sketch_secondary = ids.get(1).copied();
                editor.state.selected_sketch_extra =
                    ids.iter().skip(2).copied().collect();
            }
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintDeleteSelected => {
            // v0.27 — Delete walks the full multi-select set, not
            // just the primary `selected_pad`. Rubber-band + Ctrl-
            // click extras get the same treatment as the primary so
            // pressing Delete after a rubber-band sweep clears the
            // whole region. Sketch-mode entities use the sketch
            // dispatcher so the solver re-converges without dangling
            // constraints.
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            use crate::library::editor::footprint::state::EditorMode;

            let did_delete = editor.with_parts(|state, primitive| {
                let mut any = false;

                // Sketch-mode deletion — primary + secondary + extras.
                if state.mode == EditorMode::Sketch {
                    use std::collections::HashSet;
                    let mut seen: HashSet<signex_sketch::id::SketchEntityId> = HashSet::new();
                    let mut victims: Vec<signex_sketch::id::SketchEntityId> = Vec::new();
                    let mut push_unique =
                        |id: signex_sketch::id::SketchEntityId,
                         vs: &mut Vec<signex_sketch::id::SketchEntityId>,
                         seen: &mut HashSet<_>| {
                            if seen.insert(id) {
                                vs.push(id);
                            }
                        };
                    if let Some(id) = state.selected_sketch.take() {
                        push_unique(id, &mut victims, &mut seen);
                    }
                    if let Some(id) = state.selected_sketch_secondary.take() {
                        push_unique(id, &mut victims, &mut seen);
                    }
                    let extras: Vec<_> = state.selected_sketch_extra.drain(..).collect();
                    for id in extras {
                        push_unique(id, &mut victims, &mut seen);
                    }
                    for id in victims {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::DeleteEntity(id),
                        );
                        any = true;
                    }
                }

                // Pads (always — rubber-band can also select pads).
                let mut pad_victims: Vec<usize> = Vec::new();
                if let Some(idx) = state.selected_pad {
                    pad_victims.push(idx);
                }
                pad_victims.extend(state.selected_pads_extra.iter().copied());
                pad_victims.sort_unstable();
                pad_victims.dedup();
                // Remove highest-index first so earlier indices stay
                // valid through the loop.
                pad_victims.sort_unstable_by(|a, b| b.cmp(a));
                for idx in pad_victims {
                    if let Some(pad) = state.pads.get(idx) {
                        pad_to_sketch::mirror_delete_pad_from_sketch(pad, primitive);
                    }
                    state.delete_pad(idx);
                    any = true;
                }
                state.selected_pads_extra.clear();
                CanvasState::sync_pads_to_primitive(state, primitive);
                any
            });
            if did_delete {
                editor.canvas_cache.clear();
                editor.dirty = true;
            }
        }
        PrimitiveEditorMsg::FootprintToggleLayer(name) => {
            if let Some(layer) = FpLayer::from_standard_name(&name) {
                editor.state.layer_visibility.toggle(layer);
                editor.canvas_cache.clear();
            }
        }
        PrimitiveEditorMsg::FootprintToggleAutoFit => {
            editor.state.toggle_auto_fit();
            editor.with_parts(|state, primitive| {
                CanvasState::sync_pads_to_primitive(state, primitive);
            });
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintSetMode(mode) => {
            use crate::library::editor::footprint::state::EditorMode;
            // v0.14.2 — bidirectional sketch ↔ pads foundation.
            // When the user enters Sketch mode for the first time on
            // a footprint that has literal pads but no sketch
            // entities yet, mint a Point + PadAttr for every pad so
            // they're visible / editable in Sketch mode. The bake
            // immediately re-emits identical pads, so the round-trip
            // is identity-preserving.
            let entering_sketch =
                editor.state.mode != EditorMode::Sketch && mode == EditorMode::Sketch;
            if entering_sketch {
                use crate::library::editor::footprint::pad_to_sketch;
                let _ = editor.with_parts(|state, primitive| {
                    pad_to_sketch::auto_mint_for_literal_pads(&mut state.pads, primitive)
                });
            }
            // v0.15 — reset tool state on every mode change so a
            // stale Place Pad / Place Point selection from a prior
            // session in this tab doesn't carry over and cause
            // accidental entity placement on the first click.
            editor.state.pads_tool = crate::library::editor::footprint::state::PadsTool::Select;
            editor.state.active_tool = crate::library::editor::footprint::state::SketchTool::Select;
            editor.state.tool_pending = crate::library::editor::footprint::state::ToolPending::Idle;
            editor.state.mode = mode;
            // Run the dispatcher so the sketch is initialised + solved
            // on first switch into Sketch mode (or no-op otherwise).
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::SetMode(mode));
            });
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintSketchPlacePoint { x_mm, y_mm } => {
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            use signex_sketch::entity::{Entity, EntityKind};
            use signex_sketch::id::SketchEntityId;
            use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
            // Ensure the sketch has at least one plane so the entity has
            // somewhere to live.
            let plane_id = match editor.primitive().sketch.as_ref() {
                Some(s) if !s.planes.is_empty() => s.planes[0].id,
                _ => {
                    let pid = PlaneId::new();
                    let sketch = editor
                        .primitive_mut()
                        .sketch
                        .get_or_insert_with(signex_sketch::SketchData::default);
                    sketch.planes.push(Plane {
                        id: pid,
                        kind: PlaneKind::BoardTop,
                    });
                    pid
                }
            };
            let id = SketchEntityId::new();
            let mut entity = Entity::new(id, plane_id, EntityKind::Point { x: x_mm, y: y_mm });
            entity.construction = editor.state.construction_mode;
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::AddEntity(entity));
            });
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintSketchEditParameter { name, expr } => {
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(
                    state,
                    primitive,
                    SketchEdit::EditParameter { name, expr },
                );
            });
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintSketchSetTool(tool) => {
            editor.state.active_tool = tool;
            editor.state.tool_pending = crate::library::editor::footprint::state::ToolPending::Idle;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintSketchToggleConstruction => {
            editor.state.construction_mode = !editor.state.construction_mode;
            // v0.22 Phase A5 — mutual exclusivity with centerline.
            if editor.state.construction_mode {
                editor.state.centerline_mode = false;
            }
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintSketchToggleCenterline => {
            editor.state.centerline_mode = !editor.state.centerline_mode;
            // Mutual exclusivity — enabling centerline clears
            // construction (same Fusion 360 convention as the
            // Linetype submenu where Normal/Construction/Centerline
            // are radio-style).
            if editor.state.centerline_mode {
                editor.state.construction_mode = false;
            }
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintTogglePlacementPause => {
            editor.state.placement_paused = !editor.state.placement_paused;
            editor.canvas_cache.clear();
        }
        // v0.26 — right-click context menu plumbing. State-only
        // mutations; canvas cache is cleared when target adjusts the
        // selection (right-click on a pad selects it Altium-style).
        PrimitiveEditorMsg::FootprintShowContextMenu { x, y, target } => {
            use crate::library::editor::footprint::state::FootprintContextTarget;
            // Close any active dropdown before opening the context
            // menu so two popups never coexist (Altium parity).
            editor.state.active_bar_menu = None;
            // v0.26-B Altium parity — right-click on a pad selects
            // it (so subsequent menu actions like Delete / Properties
            // act on the right-clicked pad) without losing prior
            // selection on bare-canvas right-click.
            match target {
                FootprintContextTarget::Pad(idx) => {
                    if editor.state.selected_pad != Some(idx) {
                        editor.state.selected_pad = Some(idx);
                        editor.state.selected_silk_f = None;
                        editor.canvas_cache.clear();
                    }
                }
                FootprintContextTarget::SilkF(idx) => {
                    if editor.state.selected_silk_f != Some(idx) {
                        editor.state.selected_silk_f = Some(idx);
                        editor.state.selected_pad = None;
                        editor.canvas_cache.clear();
                    }
                }
                FootprintContextTarget::Empty => {}
            }
            editor.state.context_menu = Some(
                crate::library::editor::footprint::state::FootprintContextMenuState {
                    x,
                    y,
                    target,
                    submenu: None,
                },
            );
        }
        PrimitiveEditorMsg::FootprintCloseContextMenu => {
            editor.state.context_menu = None;
        }
        PrimitiveEditorMsg::FootprintContextMenuOpenSubmenu(sm) => {
            if let Some(ref mut menu) = editor.state.context_menu {
                menu.submenu = sm;
            }
        }
        PrimitiveEditorMsg::FootprintFitConsumed => {
            editor.state.fit_pending = false;
        }
        // v0.26-E — clipboard ops intercepted at the call site
        // (apply_footprint_clipboard_op needs split-borrow with
        // document_state.pad_clipboard). The match arm here is
        // unreachable in practice but required for exhaustiveness.
        PrimitiveEditorMsg::FootprintCopyPad
        | PrimitiveEditorMsg::FootprintCutPad
        | PrimitiveEditorMsg::FootprintPastePad => {}
        PrimitiveEditorMsg::FootprintContextMenuAction(action) => {
            use crate::library::editor::footprint::state::FootprintContextAction as Act;
            match action {
                Act::SelectAllPads => {
                    // Existing semantics: SelectAll only meaningful
                    // when there's at least one pad. With multi-
                    // select not yet implemented, "Select All" maps
                    // to selecting the first pad as a stand-in until
                    // the v0.26 multi-pad selection lands. The dock
                    // SelectAll path on the active bar already does
                    // the right thing — defer to it once it grows.
                    if !editor.state.pads.is_empty() {
                        editor.state.selected_pad = Some(0);
                    }
                    editor.state.context_menu = None;
                    editor.canvas_cache.clear();
                }
                Act::DeselectAll => {
                    editor.state.selected_pad = None;
                    editor.state.selected_pads_extra.clear();
                    editor.state.selected_silk_f = None;
                    editor.state.selected_sketch = None;
                    editor.state.selected_sketch_secondary = None;
                    editor.state.selected_sketch_extra.clear();
                    editor.state.context_menu = None;
                    editor.canvas_cache.clear();
                }
                Act::FitToWindow => {
                    // v0.26-C — arm the one-shot fit signal. The
                    // canvas Program''s next `update` tick has &mut
                    // access to its own State (where `scale` /
                    // `offset` / `did_initial_fit` live) and can
                    // consume the flag; it publishes
                    // `EditorMsg::FootprintFitConsumed` to clear the
                    // request so it doesn''t re-trigger every event.
                    editor.state.fit_pending = true;
                    editor.state.context_menu = None;
                    editor.canvas_cache.clear();
                }
            }
        }
        PrimitiveEditorMsg::FootprintSetPadsTool(tool) => {
            editor.state.pads_tool = tool;
            // v0.18.15.1 — leaving the PlaceTrack tool clears the
            // in-flight gesture so re-entering doesn't start
            // mid-segment from a stale anchor.
            if !matches!(
                tool,
                crate::library::editor::footprint::state::PadsTool::PlaceTrack
            ) {
                editor.state.track_first = None;
            }
            // v0.18.15.3 — same cleanup for Place Arc.
            if !matches!(
                tool,
                crate::library::editor::footprint::state::PadsTool::PlaceArc
            ) {
                editor.state.place_arc_pending =
                    crate::library::editor::footprint::state::PlaceArcPending::Idle;
            }
            // v0.18.15.4/v0.18.17 — leaving Place Polygon /
            // Place Region commits the in-flight vertex stash if
            // it has ≥ 3 vertices, then clears. The `filled` flag
            // follows the OUTGOING tool (we just set
            // editor.state.pads_tool = tool above; check the
            // OLD tool's identity by recording before the swap is
            // unnecessary because PadsTool::PlaceRegion is the
            // only tool that flips filled).
            let was_polygon_or_region = !editor.state.place_polygon_vertices.is_empty();
            if was_polygon_or_region
                && !matches!(
                    tool,
                    crate::library::editor::footprint::state::PadsTool::PlacePolygon
                        | crate::library::editor::footprint::state::PadsTool::PlaceRegion
                )
            {
                let verts = std::mem::take(&mut editor.state.place_polygon_vertices);
                if verts.len() >= 3 {
                    // The dispatcher arm uses
                    // `editor.state.pads_tool` (now equal to the
                    // NEW tool), so `filled` would be wrong. We
                    // can't distinguish whether the user was on
                    // PlacePolygon vs PlaceRegion now — fall back
                    // to `filled: false` and let the user re-fire
                    // PlaceRegion if they wanted fill. Future:
                    // store filled-ness on the in-flight stash
                    // alongside vertices.
                    let vertices: Vec<[f64; 2]> = verts.iter().map(|(x, y)| [*x, *y]).collect();
                    let primitive = editor.primitive_mut();
                    primitive
                        .silk_f
                        .push(signex_library::primitive::footprint::FpGraphic {
                            kind: signex_library::primitive::footprint::FpGraphicKind::Polygon {
                                vertices,
                            },
                            stroke_width: 0.15,
                            filled: false,
                        });
                    editor.dirty = true;
                }
            }
            // v0.14 — close any open active-bar dropdown after a tool
            // pick. The Place dropdown's Move / Drag / Move-Selection
            // rows route through here (footprint pad-move is drag-under-
            // Select); the dropdown widget leaves menu-closing to the
            // owner. Harmless for the top-level Pads-bar tool buttons —
            // no menu is open when those fire.
            editor.state.active_bar_menu = None;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintToolEscape => {
            // v0.15 — global Esc tool cancel. Resets both Pads and
            // Sketch tool state, mode-agnostic.
            editor.state.pads_tool = crate::library::editor::footprint::state::PadsTool::Select;
            editor.state.active_tool = crate::library::editor::footprint::state::SketchTool::Select;
            editor.state.tool_pending = crate::library::editor::footprint::state::ToolPending::Idle;
            // v0.18.15.1 — Esc also bails out of an in-flight
            // Place Track 2-click gesture.
            editor.state.track_first = None;
            // v0.18.15.3 — and Place Arc.
            editor.state.place_arc_pending =
                crate::library::editor::footprint::state::PlaceArcPending::Idle;
            // v0.18.15.4 — Esc drops the in-flight Polygon stash
            // (no commit; matches Altium's Esc-cancels-tool
            // semantic).
            editor.state.place_polygon_vertices.clear();
            // v0.13 — Esc also dismisses any open active-bar dropdown.
            editor.state.active_bar_menu = None;
            // v0.20 — Esc clears the selected pad / silk graphic too,
            // matching the schematic canvas + Altium PCB Library
            // editor. Without this, Esc only reset the tool but the
            // pad selection persisted, leaving the user staring at
            // pad properties they no longer wanted to edit.
            editor.state.selected_pad = None;
            editor.state.selected_silk_f = None;
            editor.state.placement_paused = false;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintToggleActiveBarMenu(menu) => {
            editor.state.active_bar_menu = match editor.state.active_bar_menu {
                Some(m) if m == menu => None,
                _ => Some(menu),
            };
        }
        PrimitiveEditorMsg::FootprintCloseActiveBarMenu => {
            editor.state.active_bar_menu = None;
        }
        PrimitiveEditorMsg::FootprintActiveBarStub(label) => {
            crate::diagnostics::log_info(format!(
                "Active bar: {label} — coming soon (footprint Altium parity)"
            ));
            editor.state.active_bar_menu = None;
        }
        // Task 6 — apply footprint filter preset `idx` from the
        // persisted list. Re-read from disk on every apply so a
        // preset captured in a different tab/session is picked up
        // without needing an in-memory refresh.
        PrimitiveEditorMsg::FootprintApplyFilterPreset(idx) => {
            let presets = crate::fonts::read_footprint_filter_presets();
            if let Some(preset) = presets.get(idx) {
                crate::library::editor::footprint::filter_presets::apply_preset(
                    &mut editor.state,
                    preset,
                );
            }
            editor.state.active_bar_menu = None;
            editor.canvas_cache.clear();
        }
        // Task 6 — Filter dropdown's "All - On / All - Off" chip.
        PrimitiveEditorMsg::FootprintToggleAllFilters => {
            let all_on = crate::library::editor::footprint::state::SelectionFilterKind::ALL
                .iter()
                .all(|&k| editor.state.selection_filter.get(k));
            editor.state.selection_filter.set_all(!all_on);
            editor.canvas_cache.clear();
        }
        // Task 6 — minimal capture affordance: snapshot the current
        // filter as a new default-named preset and persist it. No
        // rename UI yet (deferred — see filter_presets.rs). Silently
        // ignores the capture once `CUSTOM_FILTER_PRESET_LIMIT` slots
        // are full rather than evicting an existing preset.
        PrimitiveEditorMsg::FootprintCaptureFilterPreset => {
            let mut presets = crate::fonts::read_footprint_filter_presets();
            if presets.len() < crate::active_bar::CUSTOM_FILTER_PRESET_LIMIT {
                let name = format!("Filter {}", presets.len() + 1);
                let preset = crate::library::editor::footprint::filter_presets::capture_preset(
                    &editor.state,
                    name,
                );
                presets.push(preset);
                crate::fonts::write_footprint_filter_presets(&presets);
            }
            editor.state.active_bar_menu = None;
        }
        PrimitiveEditorMsg::FootprintActiveBarToggleSnap(flag) => {
            use crate::panels::SnapOptionFlag;
            let opts = &mut editor.state.snap_options;
            match flag {
                SnapOptionFlag::PointHit => opts.point_hit = !opts.point_hit,
                SnapOptionFlag::HorizontalVertical => {
                    opts.horizontal_vertical = !opts.horizontal_vertical
                }
                SnapOptionFlag::Angle => opts.angle = !opts.angle,
                SnapOptionFlag::Grid => opts.grid = !opts.grid,
                SnapOptionFlag::TrackVertices => {
                    opts.snap_track_vertices = !opts.snap_track_vertices
                }
                SnapOptionFlag::TrackLines => opts.snap_track_lines = !opts.snap_track_lines,
                SnapOptionFlag::ArcCenters => opts.snap_arc_centers = !opts.snap_arc_centers,
                SnapOptionFlag::Intersections => opts.snap_intersections = !opts.snap_intersections,
                SnapOptionFlag::PadCenters => opts.snap_pad_centers = !opts.snap_pad_centers,
                SnapOptionFlag::PadVertices => opts.snap_pad_vertices = !opts.snap_pad_vertices,
                SnapOptionFlag::PadEdges => opts.snap_pad_edges = !opts.snap_pad_edges,
                SnapOptionFlag::ViaCenters => opts.snap_via_centers = !opts.snap_via_centers,
                SnapOptionFlag::Texts => opts.snap_texts = !opts.snap_texts,
                SnapOptionFlag::Regions => opts.snap_regions = !opts.snap_regions,
                SnapOptionFlag::FootprintOrigins => {
                    opts.snap_footprint_origins = !opts.snap_footprint_origins
                }
                SnapOptionFlag::Body3dPoints => {
                    opts.snap_3d_body_points = !opts.snap_3d_body_points
                }
                SnapOptionFlag::SnapToGrids => opts.snap_to_grids = !opts.snap_to_grids,
                SnapOptionFlag::SnapToGuides => opts.snap_to_guides = !opts.snap_to_guides,
                SnapOptionFlag::SnapToAxes => opts.snap_to_axes = !opts.snap_to_axes,
            }
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintActiveBarSetSnappingMode(mode) => {
            editor.state.snapping_mode = mode;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintActiveBarSetSnapSubTab(sub) => {
            editor.state.snap_subtab = sub;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintActiveBarRotateSelection => {
            editor.with_parts(|state, primitive| {
                if let Some(idx) = state.selected_pad
                    && let Some(pad) = state.pads.get_mut(idx)
                {
                    pad.rotation_deg = (pad.rotation_deg + 90.0).rem_euclid(360.0);
                    CanvasState::sync_pads_to_primitive(state, primitive);
                }
            });
            editor.state.active_bar_menu = None;
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintActiveBarFlipSelection => {
            editor.with_parts(|state, primitive| {
                if let Some(idx) = state.selected_pad
                    && let Some(pad) = state.pads.get_mut(idx)
                {
                    let new_layers: Vec<signex_library::LayerId> = pad
                        .layers
                        .iter()
                        .map(|l| {
                            let s = l.as_str();
                            let flipped = if let Some(rest) = s.strip_prefix("F.") {
                                format!("B.{rest}")
                            } else if let Some(rest) = s.strip_prefix("B.") {
                                format!("F.{rest}")
                            } else {
                                s.to_string()
                            };
                            signex_library::LayerId::new(flipped)
                        })
                        .collect();
                    pad.layers = new_layers;
                    CanvasState::sync_pads_to_primitive(state, primitive);
                }
            });
            editor.state.active_bar_menu = None;
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintActiveBarNudgeSelection => {
            // v0.14 — "Move Selection by X, Y…" one-step nudge: the
            // combined selection by one active grid step in +X and +Y.
            // Shares geometry + sketch-mirror + history with the
            // typed-delta Move-By modal via `footprint_nudge_selection`.
            let step = editor.state.snap_options.grid_step_mm.max(0.001);
            footprint_nudge_selection(editor, step, step);
            editor.state.active_bar_menu = None;
        }
        PrimitiveEditorMsg::FootprintMoveByOpen => {
            editor.state.move_by_modal = Some(Default::default());
            editor.state.active_bar_menu = None;
        }
        PrimitiveEditorMsg::FootprintMoveBySetX(v) => {
            if let Some(m) = editor.state.move_by_modal.as_mut() {
                m.dx_buf = v;
            }
        }
        PrimitiveEditorMsg::FootprintMoveBySetY(v) => {
            if let Some(m) = editor.state.move_by_modal.as_mut() {
                m.dy_buf = v;
            }
        }
        PrimitiveEditorMsg::FootprintMoveByConfirm => {
            if let Some((dx, dy)) = editor
                .state
                .move_by_modal
                .as_ref()
                .and_then(|m| m.parsed())
            {
                footprint_nudge_selection(editor, dx, dy);
            }
            editor.state.move_by_modal = None;
        }
        PrimitiveEditorMsg::FootprintMoveByCancel => {
            editor.state.move_by_modal = None;
        }
        PrimitiveEditorMsg::FootprintMintBody3d => {
            editor.push_history();
            editor.with_parts(|_state, primitive| {
                crate::library::editor::footprint::body3d_mint::mint_box_from_courtyard(primitive);
            });
            editor.state.active_bar_menu = None;
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintMintExtrudedBody3d => {
            editor.push_history();
            editor.with_parts(|_state, primitive| {
                crate::library::editor::footprint::body3d_mint::mint_extruded_from_fab(primitive);
            });
            editor.state.active_bar_menu = None;
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintActiveBarAlignSelectionToGrid => {
            editor.with_parts(|state, primitive| {
                let step = state.snap_options.grid_step_mm.max(0.001);
                if let Some(idx) = state.selected_pad
                    && let Some(pad) = state.pads.get_mut(idx)
                {
                    let (x, y) = pad.position_mm;
                    pad.position_mm = ((x / step).round() * step, (y / step).round() * step);
                    // v0.23 — mirror the snap into the sketch so the
                    // construction outline + centre Point follow the
                    // pad. Skipping this left the sketch primitive
                    // stranded at the pre-snap position.
                    let pad_snapshot = pad.clone();
                    pad_to_sketch::mirror_move_pad_in_sketch(&pad_snapshot, primitive);
                    CanvasState::sync_pads_to_primitive(state, primitive);
                }
            });
            editor.state.active_bar_menu = None;
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintActiveBarMoveOriginToGrid => {
            editor.with_parts(|state, primitive| {
                let step = state.snap_options.grid_step_mm.max(0.001);
                let mut snapshots: Vec<crate::library::editor::footprint::state::EditorPad> =
                    Vec::with_capacity(state.pads.len());
                for pad in state.pads.iter_mut() {
                    let (x, y) = pad.position_mm;
                    pad.position_mm = ((x / step).round() * step, (y / step).round() * step);
                    snapshots.push(pad.clone());
                }
                // v0.23 — mirror every pad's new position into the
                // sketch. Same fix as the single-pad align path.
                for snapshot in &snapshots {
                    pad_to_sketch::mirror_move_pad_in_sketch(snapshot, primitive);
                }
                CanvasState::sync_pads_to_primitive(state, primitive);
            });
            editor.state.active_bar_menu = None;
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintAlignPads(op) => {
            // v0.14 — active-bar Align/Distribute/Spacing. Operates on
            // the combined selection (`selected_pad` + the ctrl-click
            // extras). Mirrors every moved pad into the backing sketch
            // and pushes one history snapshot, exactly like the
            // group-drag path in `FootprintMovePad`.
            use crate::library::editor::footprint::state::AlignOp;

            // Collect + dedup the selection indices up front so we can
            // bail before touching history if there isn't enough to act
            // on. Align ops need ≥2 pads; distribute needs ≥3.
            let mut indices: Vec<usize> = Vec::new();
            if let Some(p) = editor.state.selected_pad {
                indices.push(p);
            }
            indices.extend(editor.state.selected_pads_extra.iter().copied());
            indices.sort_unstable();
            indices.dedup();
            indices.retain(|&i| i < editor.state.pads.len());

            let min_needed = match op {
                AlignOp::DistributeH | AlignOp::DistributeV => 3,
                _ => 2,
            };
            // Only act when the selection is large enough — align needs
            // ≥2 pads, distribute ≥3. Smaller selections fall through as
            // a clean no-op (menu still dismisses below).
            if indices.len() >= min_needed {
                editor.push_history();
                editor.with_parts(|state, primitive| {
                    // Spacing step: reuse the active grid step so the
                    // expand/contract increment matches what the user
                    // already snaps to (no hardcoded magic size).
                    let step = state.snap_options.grid_step_mm.max(0.001);
                    align_pads(state, &indices, op, step);
                    // Mirror every selected pad's (possibly) new centre
                    // into the sketch, then re-sync the literal `Pad`
                    // list.
                    let mut moved: Vec<crate::library::editor::footprint::state::EditorPad> =
                        Vec::with_capacity(indices.len());
                    for &i in &indices {
                        if let Some(pad) = state.pads.get(i) {
                            moved.push(pad.clone());
                        }
                    }
                    for snapshot in &moved {
                        pad_to_sketch::mirror_move_pad_in_sketch(snapshot, primitive);
                    }
                    CanvasState::sync_pads_to_primitive(state, primitive);
                });
                editor.canvas_cache.clear();
                editor.dirty = true;
            }
            editor.state.active_bar_menu = None;
        }
        PrimitiveEditorMsg::FootprintActiveBarSelectAll => {
            // v0.27 — Altium-parity: Select All multi-selects every
            // pad in Pads mode; Sketch mode still picks the first
            // entity (sketch-side multi-select is a v0.28 follow-up).
            use crate::library::editor::footprint::state::EditorMode;
            match editor.state.mode {
                EditorMode::Sketch => {
                    if editor.state.selected_sketch.is_none() {
                        editor.state.selected_sketch = editor
                            .primitive()
                            .sketch
                            .as_ref()
                            .and_then(|sk| sk.entities.first().map(|e| e.id));
                    }
                }
                EditorMode::Normal => {
                    if !editor.state.pads.is_empty() {
                        editor.state.selected_pad = Some(0);
                        editor.state.selected_pads_extra =
                            (1..editor.state.pads.len()).collect();
                    }
                }
                EditorMode::View3d => {}
            }
            editor.canvas_cache.clear();
            editor.state.active_bar_menu = None;
        }
        PrimitiveEditorMsg::FootprintActiveBarClearSelection => {
            editor.state.selected_pad = None;
            editor.state.selected_pads_extra.clear();
            editor.state.selected_sketch = None;
            editor.state.selected_sketch_secondary = None;
            editor.state.selected_sketch_extra.clear();
            editor.state.selected_silk_f = None;
            editor.canvas_cache.clear();
            editor.state.active_bar_menu = None;
        }
        PrimitiveEditorMsg::FootprintActiveBarSetSketchTool(tool) => {
            // Switch to Sketch mode if not already there, then arm the
            // selected sketch tool. Cancels any in-flight gesture.
            use crate::library::editor::footprint::state::{EditorMode, ToolPending};
            if editor.state.mode != EditorMode::Sketch {
                editor.state.mode = EditorMode::Sketch;
            }
            editor.state.active_tool = tool;
            editor.state.tool_pending = ToolPending::Idle;
            editor.state.active_bar_menu = None;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintSetName(new_name) => {
            // Rename the ACTIVE internal footprint. The .snxfpt
            // envelope holds N footprints; only the user-selected one
            // mutates. Empty names are accepted but treated as
            // "unnamed" for breadcrumb / file display purposes.
            editor.primitive_mut().name = new_name;
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintSketchToolEscape => {
            editor.state.tool_pending = crate::library::editor::footprint::state::ToolPending::Idle;
            // v0.24 Track D — leaving the gesture also drops any
            // numeric buffer the user had been typing. Otherwise a
            // half-typed length would leak across to a freshly-started
            // tool gesture.
            editor.state.placement_input = None;
            // v0.14-footprint — clear every stashed dimension field too.
            editor.state.placement_input_others.clear();
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintSketchPlacementInputChar(ch) => {
            // v0.24 Track D — append `ch` to `placement_input.buffer`,
            // minting a fresh entry against the active tool's matching
            // `PlacementInputKind` if one isn't already pinned. Drops
            // the keypress silently when the active tool / pending
            // state doesn't accept numeric input.
            use crate::library::editor::footprint::state::{PlacementInput, PlacementInputKind};
            let tool = editor.state.active_tool;
            let pending = editor.state.tool_pending.clone();
            let kind_for_active = PlacementInputKind::from_active_tool(tool, &pending);
            // Resolve the kind: if a buffer already exists, keep its
            // kind so the user can finish typing across a second
            // keypress; otherwise mint one matched to the tool.
            let kind = match editor.state.placement_input.as_ref() {
                Some(existing) => existing.kind,
                None => match kind_for_active {
                    Some(k) => k,
                    None => return, // tool doesn't accept numeric input
                },
            };
            // Validation:
            // - digits always allowed,
            // - one decimal point per buffer,
            // - leading minus only for `ArcSweep` and only at position 0,
            // - everything else dropped.
            let buf_ref = editor
                .state
                .placement_input
                .as_ref()
                .map(|p| p.buffer.as_str())
                .unwrap_or("");
            let accept = if ch.is_ascii_digit() {
                true
            } else if ch == '.' {
                !buf_ref.contains('.')
            } else if ch == '-' {
                kind.allows_negative() && buf_ref.is_empty()
            } else {
                false
            };
            if !accept {
                return;
            }
            // Mint or append.
            let entry = editor
                .state
                .placement_input
                .get_or_insert_with(|| PlacementInput {
                    buffer: String::new(),
                    kind,
                });
            entry.buffer.push(ch);
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintSketchPlacementInputBackspace => {
            // v0.24 Track D — pop one character; clear `placement_input`
            // entirely once the buffer empties so the next typed digit
            // mints a fresh entry against the (possibly different)
            // active tool.
            if let Some(entry) = editor.state.placement_input.as_mut() {
                entry.buffer.pop();
                if entry.buffer.is_empty() {
                    editor.state.placement_input = None;
                }
                editor.canvas_cache.clear();
            }
        }
        PrimitiveEditorMsg::FootprintSketchPlacementInputEnter => {
            // v0.24 Track D — Enter is a no-op on state. The buffer
            // stays alive so the next click consumes it. The message
            // is captured at the canvas layer purely so the keypress
            // doesn't fall through to a global shortcut.
        }
        PrimitiveEditorMsg::FootprintSketchPlacementInputEscape => {
            // v0.24 Track D — Esc throws away the buffer immediately;
            // the next click commits at the cursor position with no
            // override. Tool pending state is left intact so the
            // gesture itself isn't cancelled (use right-click / tool
            // Esc for that).
            if editor.state.placement_input.is_some()
                || !editor.state.placement_input_others.is_empty()
            {
                editor.state.placement_input = None;
                // v0.14-footprint — Esc also clears every stashed
                // dimension field so none leaks into the next gesture.
                editor.state.placement_input_others.clear();
                editor.canvas_cache.clear();
            }
        }
        PrimitiveEditorMsg::FootprintSketchPlacementInputTab => {
            // v0.14-footprint — cycle the focused dimension field to the
            // next one in the active tool's Tab order (Line len→angle,
            // Rectangle w→h, Rounded-Rect w→h→radius→w…). The focused
            // field lives in `placement_input`; the rest park in
            // `placement_input_others`, each keeping its own typed
            // digits. The canvas only emits this while a buffer is open
            // on a multi-field tool, but the dispatcher stays defensive
            // and no-ops unless the active tool exposes ≥2 fields.
            use crate::library::editor::footprint::state::{PlacementInput, PlacementInputKind};
            let fields = PlacementInputKind::placement_fields(
                editor.state.active_tool,
                &editor.state.tool_pending,
            );
            if fields.len() >= 2 {
                let current = editor
                    .state
                    .placement_input
                    .as_ref()
                    .map(|p| p.kind)
                    .unwrap_or(fields[0]);
                let idx = fields.iter().position(|k| *k == current).unwrap_or(0);
                let next_kind = fields[(idx + 1) % fields.len()];
                // Park the focused field (preserving its digits),
                // replacing any stale same-kind entry; then pull the
                // next field out of the parked set, or mint it empty so
                // the next keypress appends to it.
                let prev_focused = editor.state.placement_input.take();
                let next_focused = match editor
                    .state
                    .placement_input_others
                    .iter()
                    .position(|p| p.kind == next_kind)
                {
                    Some(pos) => editor.state.placement_input_others.remove(pos),
                    None => PlacementInput {
                        buffer: String::new(),
                        kind: next_kind,
                    },
                };
                if let Some(prev) = prev_focused {
                    editor
                        .state
                        .placement_input_others
                        .retain(|p| p.kind != prev.kind);
                    editor.state.placement_input_others.push(prev);
                }
                editor.state.placement_input = Some(next_focused);
                editor.canvas_cache.clear();
            }
        }
        PrimitiveEditorMsg::FootprintSketchSelect { id, shift } => {
            // None clears every selection slot. Some(id) without
            // shift replaces primary; with shift adds to secondary
            // (or replaces secondary with the new id).
            //
            // v0.14 — clear `selected_sketch_extra` (the rubber-band
            // multi-select set) on every single-click select. Without
            // this, clicking empty space (or a single entity) after a
            // rubber-band left the box-selected extras flagged
            // selected, so they kept rendering in the orange selection
            // colour instead of the blue idle DOF colour — the
            // "unselected shape stays orange" bug. A single click is
            // always a fresh selection, so the extras never carry over.
            editor.state.selected_sketch_extra.clear();
            match (id, shift) {
                (None, _) => {
                    editor.state.selected_sketch = None;
                    editor.state.selected_sketch_secondary = None;
                }
                (Some(new_id), false) => {
                    editor.state.selected_sketch = Some(new_id);
                    editor.state.selected_sketch_secondary = None;
                }
                (Some(new_id), true) => {
                    if editor.state.selected_sketch.is_none() {
                        editor.state.selected_sketch = Some(new_id);
                    } else if editor.state.selected_sketch != Some(new_id) {
                        editor.state.selected_sketch_secondary = Some(new_id);
                    }
                }
            }
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintSketchMovePoint { id, dx, dy } => {
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(
                    state,
                    primitive,
                    SketchEdit::MovePoint { id, dx, dy },
                );
            });
            // v0.16.0.1 fix — when the dragged Point is a pad's
            // centre, also translate that pad's outline-corner Points
            // by the same delta so the construction outline tracks
            // the pad. Without this the corner outline was stranded
            // at the previous centre after a sketch-mode drag.
            let centre_pad_idx = editor
                .state
                .pads
                .iter()
                .position(|p| p.sketch_entity_id == Some(id));
            if let Some(pad_idx) = centre_pad_idx {
                if let Some(corners) = editor.state.pads[pad_idx].corner_entity_ids {
                    for corner_id in corners {
                        editor.with_parts(|state, primitive| {
                            apply_sketch_edit_with_warnings(
                                state,
                                primitive,
                                SketchEdit::MovePoint {
                                    id: corner_id,
                                    dx,
                                    dy,
                                },
                            );
                        });
                    }
                }
                // Keep `EditorPad.position_mm` in sync so a Pads-mode
                // tab switch shows the pad at the new world position.
                editor.state.pads[pad_idx].position_mm.0 += dx;
                editor.state.pads[pad_idx].position_mm.1 += dy;
                editor.with_parts(|state, primitive| {
                    CanvasState::sync_pads_to_primitive(state, primitive);
                });
            }
            // v0.16.1 fix — when the dragged Point is one of any
            // pad's outline-corner Points, recompute that pad's bbox
            // from all 4 corner positions. Update the pad's
            // position_mm + size_mm AND rewrite the centre Point's
            // PadAttr.size_x_expr / size_y_expr so the bake re-emits
            // the pad at the new size. This is the "drag-corner-to-
            // resize-pad" behaviour the user expects when they grab
            // a corner of the construction outline.
            let corner_pad_idx = editor.state.pads.iter().position(|p| {
                p.corner_entity_ids
                    .as_ref()
                    .map(|ids| ids.contains(&id))
                    .unwrap_or(false)
            });
            if let Some(pad_idx) = corner_pad_idx {
                use signex_sketch::entity::EntityKind;
                let Some(corners) = editor.state.pads[pad_idx].corner_entity_ids else {
                    // `position()` above already required `is_some()`; this
                    // arm is unreachable in practice but propagating via
                    // early-let-else avoids the matching `.unwrap()` panic
                    // if a future refactor decouples the two.
                    return;
                };
                let mut min_x = f64::INFINITY;
                let mut min_y = f64::INFINITY;
                let mut max_x = f64::NEG_INFINITY;
                let mut max_y = f64::NEG_INFINITY;
                if let Some(sketch) = editor.primitive().sketch.as_ref() {
                    for cid in corners {
                        if let Some(e) = sketch.entities.iter().find(|e| e.id == cid) {
                            if let EntityKind::Point { x, y } = e.kind {
                                if x < min_x {
                                    min_x = x;
                                }
                                if y < min_y {
                                    min_y = y;
                                }
                                if x > max_x {
                                    max_x = x;
                                }
                                if y > max_y {
                                    max_y = y;
                                }
                            }
                        }
                    }
                }
                if min_x.is_finite() && min_y.is_finite() {
                    let new_w = (max_x - min_x).max(0.05);
                    let new_h = (max_y - min_y).max(0.05);
                    let new_cx = (min_x + max_x) / 2.0;
                    let new_cy = (min_y + max_y) / 2.0;
                    let pad = &mut editor.state.pads[pad_idx];
                    let old_centre = pad.position_mm;
                    pad.position_mm = (new_cx, new_cy);
                    pad.size_mm = (new_w, new_h);
                    let centre_id = pad.sketch_entity_id;
                    // v0.18.12.1 bugfix — re-align the OTHER three
                    // corner Points to the new pad bbox. Previously
                    // only the dragged corner moved, leaving the
                    // pad rectangle (drawn at the new bbox) and the
                    // non-dragged corners stranded at their old
                    // positions — visible as the dashed-construction
                    // outline drifting off the rendered pad on
                    // subsequent corner drags.
                    let new_positions: [(f64, f64); 4] = [
                        (max_x, min_y), // ne
                        (max_x, max_y), // se
                        (min_x, max_y), // sw
                        (min_x, min_y), // nw
                    ];
                    for (corner_id, (target_x, target_y)) in
                        corners.iter().zip(new_positions.iter())
                    {
                        // Skip the corner the user just dragged — it's
                        // already at the right position, and emitting
                        // a zero-delta MovePoint would still trip the
                        // solver.
                        if *corner_id == id {
                            continue;
                        }
                        let cur = editor
                            .primitive()
                            .sketch
                            .as_ref()
                            .and_then(|s| s.entities.iter().find(|e| e.id == *corner_id))
                            .and_then(|e| {
                                if let signex_sketch::entity::EntityKind::Point { x, y } = e.kind {
                                    Some((x, y))
                                } else {
                                    None
                                }
                            });
                        if let Some((cx, cy)) = cur {
                            let cdx = *target_x - cx;
                            let cdy = *target_y - cy;
                            if cdx.abs() > 1e-9 || cdy.abs() > 1e-9 {
                                editor.with_parts(|state, primitive| {
                                    apply_sketch_edit_with_warnings(
                                        state,
                                        primitive,
                                        SketchEdit::MovePoint {
                                            id: *corner_id,
                                            dx: cdx,
                                            dy: cdy,
                                        },
                                    );
                                });
                            }
                        }
                    }
                    // Move the centre Point to the new bbox centre +
                    // rewrite the PadAttr size exprs so the bake
                    // emits the new size.
                    if let Some(centre_id) = centre_id {
                        let cdx = new_cx - old_centre.0;
                        let cdy = new_cy - old_centre.1;
                        if cdx.abs() > 1e-9 || cdy.abs() > 1e-9 {
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::MovePoint {
                                        id: centre_id,
                                        dx: cdx,
                                        dy: cdy,
                                    },
                                );
                            });
                        }
                        if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                            if let Some(centre) =
                                sketch.entities.iter_mut().find(|e| e.id == centre_id)
                            {
                                if let Some(attr) = centre.pad.as_mut() {
                                    attr.size_x_expr = format!("{:.4}mm", new_w);
                                    attr.size_y_expr = format!("{:.4}mm", new_h);
                                }
                            }
                        }
                    }
                    editor.with_parts(|state, primitive| {
                        CanvasState::sync_pads_to_primitive(state, primitive);
                    });
                }
            }
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintSketchMoveLine { id, dx, dy } => {
            // v0.27 — drag a Line edge by translating both its
            // endpoints in one solver pass. The dispatcher reads
            // the Line's start/end IDs, then emits MovePoint for
            // each. The solver re-runs once after both moves so
            // H/V/Distance constraints converge correctly without
            // the brief mid-tick "one corner moved, the other
            // didn't" state a two-message split would produce.
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            use signex_sketch::entity::EntityKind;
            let endpoints = editor
                .primitive()
                .sketch
                .as_ref()
                .and_then(|s| s.entities.iter().find(|e| e.id == id))
                .and_then(|e| match e.kind {
                    EntityKind::Line { start, end } => Some((start, end)),
                    _ => None,
                });
            let Some((start, end)) = endpoints else {
                return;
            };
            // v0.27 — snapshot the line's pre-drag endpoint positions.
            // Used after the translate step to detect which bbox edge
            // the line lay on (so the matching pad can resize). Read
            // BEFORE the MovePoint passes shift these Points.
            let pre_drag_endpoints: Option<((f64, f64), (f64, f64))> = editor
                .primitive()
                .sketch
                .as_ref()
                .and_then(|s| {
                    let pos_of = |pid: signex_sketch::id::SketchEntityId| -> Option<(f64, f64)> {
                        s.entities.iter().find(|e| e.id == pid).and_then(|e| {
                            if let EntityKind::Point { x, y } = e.kind {
                                Some((x, y))
                            } else {
                                None
                            }
                        })
                    };
                    pos_of(start).zip(pos_of(end))
                });
            // v0.27 — gather the arc victim set BEFORE running any
            // moves so adjacency lookups read pre-drag positions.
            // Arc centres + the "other" tangent endpoint of any Arc
            // tangent to a moving line endpoint translate by the
            // same `(dx, dy)` as the rigid edge so rounded-rectangle
            // corners stay rigid (constant radius). The line's own
            // endpoints are handled separately below — they may
            // slide along an adjacent edge rather than translating
            // rigidly (Fusion-style "expand toward dragging").
            let mut arc_victims: std::collections::HashSet<signex_sketch::id::SketchEntityId> =
                std::collections::HashSet::new();
            if let Some(s) = editor.primitive().sketch.as_ref() {
                for e in &s.entities {
                    if let EntityKind::Arc {
                        start: a_s,
                        end: a_e,
                        center: a_c,
                        ..
                    } = e.kind
                    {
                        let touches = a_s == start
                            || a_s == end
                            || a_e == start
                            || a_e == end;
                        if touches {
                            arc_victims.insert(a_c);
                            if a_s != start && a_s != end {
                                arc_victims.insert(a_s);
                            }
                            if a_e != start && a_e != end {
                                arc_victims.insert(a_e);
                            }
                        }
                    }
                }
            }
            // v0.27 — per-endpoint slide. If the endpoint connects
            // to exactly one OTHER line (closed polygon vertex),
            // slide the endpoint along that adjacent line so the
            // dragged edge only stretches/shrinks perpendicular and
            // the adjacent edges retain their direction. The pad
            // bbox case still produces the right answer here because
            // a rect pad's edge endpoints connect to perpendicular
            // edges — sliding along a perpendicular line by the
            // perpendicular drag delta is equivalent to translating.
            // When the endpoint has zero or ≥2 other lines (free
            // vertex, arc tangent, T-junction), fall back to rigid
            // translate so the existing pad / arc-corner flows keep
            // working.
            let read_pos = |pid: signex_sketch::id::SketchEntityId| -> Option<(f64, f64)> {
                editor
                    .primitive()
                    .sketch
                    .as_ref()
                    .and_then(|s| s.entities.iter().find(|e| e.id == pid))
                    .and_then(|e| match e.kind {
                        EntityKind::Point { x, y } => Some((x, y)),
                        _ => None,
                    })
            };
            // Find the unique other line connected to `endpoint`
            // (excluding the dragged line itself). Returns the far
            // endpoint of that line — the one we treat as the
            // slide pivot. Returns `None` when 0 or ≥2 other lines
            // meet at this endpoint.
            let find_far = |endpoint: signex_sketch::id::SketchEntityId|
                -> Option<signex_sketch::id::SketchEntityId> {
                let sketch = editor.primitive().sketch.as_ref()?;
                let mut found: Option<signex_sketch::id::SketchEntityId> = None;
                for e in &sketch.entities {
                    if e.id == id {
                        continue;
                    }
                    if let EntityKind::Line { start: ls, end: le } = e.kind {
                        let far = if ls == endpoint {
                            Some(le)
                        } else if le == endpoint {
                            Some(ls)
                        } else {
                            None
                        };
                        if let Some(f) = far {
                            if found.is_some() {
                                return None;
                            }
                            found = Some(f);
                        }
                    }
                }
                found
            };
            // 2D line-line intersection. `p1 + t*d1 = p2 + s*d2`.
            // Returns `None` for parallel / coincident lines.
            let intersect = |p1: (f64, f64),
                             d1: (f64, f64),
                             p2: (f64, f64),
                             d2: (f64, f64)|
             -> Option<(f64, f64)> {
                let det = d2.0 * d1.1 - d1.0 * d2.1;
                if det.abs() < 1e-9 {
                    return None;
                }
                let t = (d2.0 * (p2.1 - p1.1) - d2.1 * (p2.0 - p1.0)) / det;
                Some((p1.0 + t * d1.0, p1.1 + t * d1.1))
            };
            let target_for =
                |endpoint: signex_sketch::id::SketchEntityId, pos: (f64, f64)| -> (f64, f64) {
                    let rigid = (pos.0 + dx, pos.1 + dy);
                    let Some(far_id) = find_far(endpoint) else {
                        return rigid;
                    };
                    let Some(far_pos) = read_pos(far_id) else {
                        return rigid;
                    };
                    let Some((sx_pre, sy_pre)) = read_pos(start) else {
                        return rigid;
                    };
                    let Some((ex_pre, ey_pre)) = read_pos(end) else {
                        return rigid;
                    };
                    let line_d = (ex_pre - sx_pre, ey_pre - sy_pre);
                    let other_d = (pos.0 - far_pos.0, pos.1 - far_pos.1);
                    intersect(rigid, line_d, far_pos, other_d).unwrap_or(rigid)
                };
            let start_pos_opt = read_pos(start);
            let end_pos_opt = read_pos(end);
            if let (Some(start_pos), Some(end_pos)) = (start_pos_opt, end_pos_opt) {
                let start_target = target_for(start, start_pos);
                let end_target = target_for(end, end_pos);
                let start_delta = (start_target.0 - start_pos.0, start_target.1 - start_pos.1);
                let end_delta = (end_target.0 - end_pos.0, end_target.1 - end_pos.1);
                if start_delta.0.abs() > 1e-9 || start_delta.1.abs() > 1e-9 {
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::MovePoint {
                                id: start,
                                dx: start_delta.0,
                                dy: start_delta.1,
                            },
                        );
                    });
                }
                if end_delta.0.abs() > 1e-9 || end_delta.1.abs() > 1e-9 {
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::MovePoint {
                                id: end,
                                dx: end_delta.0,
                                dy: end_delta.1,
                            },
                        );
                    });
                }
            }
            for pid in arc_victims {
                editor.with_parts(|state, primitive| {
                    apply_sketch_edit_with_warnings(
                        state,
                        primitive,
                        SketchEdit::MovePoint { id: pid, dx, dy },
                    );
                });
            }
            // v0.27 — propagate the edge drag to the literal pad
            // bbox. Without this the sketch outline visibly resizes
            // but `pad.size_mm` / `pad.position_mm` (and the baked
            // pad rendering) stay frozen — the user sees the line
            // move while the pad copper underneath does nothing.
            //
            // Strategy: classify the line's pre-drag pose against
            // each pad's bbox to identify which side it lies on
            // (top / bottom / left / right). Only axis-aligned lines
            // qualify — diagonal sketch lines are never pad edges
            // for Rect / RoundRect / Oval / Chamfered shapes.
            const EDGE_EPS: f64 = 1e-4;
            if let Some(((sx, sy), (ex, ey))) = pre_drag_endpoints {
                let is_horizontal = (sy - ey).abs() < EDGE_EPS;
                let is_vertical = (sx - ex).abs() < EDGE_EPS;
                let pad_count = editor.state.pads.len();
                for pad_idx in 0..pad_count {
                    let bbox_data = {
                        let pad = &editor.state.pads[pad_idx];
                        if pad.corner_entity_ids.is_none() {
                            continue;
                        }
                        let (xmin, ymin, xmax, ymax) = pad.bbox_mm();
                        // Both endpoints must lie on the same bbox
                        // side; partial overlap (line extends past a
                        // corner) means it's not a pad edge.
                        let in_x = sx >= xmin - EDGE_EPS
                            && sx <= xmax + EDGE_EPS
                            && ex >= xmin - EDGE_EPS
                            && ex <= xmax + EDGE_EPS;
                        let in_y = sy >= ymin - EDGE_EPS
                            && sy <= ymax + EDGE_EPS
                            && ey >= ymin - EDGE_EPS
                            && ey <= ymax + EDGE_EPS;
                        if !in_x || !in_y {
                            continue;
                        }
                        let edge: Option<&str> = if is_horizontal
                            && (sy - ymin).abs() < EDGE_EPS
                        {
                            Some("top")
                        } else if is_horizontal && (sy - ymax).abs() < EDGE_EPS {
                            Some("bottom")
                        } else if is_vertical && (sx - xmin).abs() < EDGE_EPS {
                            Some("left")
                        } else if is_vertical && (sx - xmax).abs() < EDGE_EPS {
                            Some("right")
                        } else {
                            None
                        };
                        let Some(edge) = edge else {
                            continue;
                        };
                        let (new_xmin, new_ymin, new_xmax, new_ymax) = match edge {
                            "top" => (xmin, ymin + dy, xmax, ymax),
                            "bottom" => (xmin, ymin, xmax, ymax + dy),
                            "left" => (xmin + dx, ymin, xmax, ymax),
                            "right" => (xmin, ymin, xmax + dx, ymax),
                            _ => unreachable!(),
                        };
                        // Reject degenerate drags that would collapse
                        // or invert the bbox. The user has to release
                        // and re-grab if they want sub-50µm pads.
                        if new_xmax - new_xmin < 0.05 || new_ymax - new_ymin < 0.05 {
                            continue;
                        }
                        Some((new_xmin, new_ymin, new_xmax, new_ymax))
                    };
                    let Some((new_xmin, new_ymin, new_xmax, new_ymax)) = bbox_data else {
                        continue;
                    };
                    let new_w = new_xmax - new_xmin;
                    let new_h = new_ymax - new_ymin;
                    let new_cx = (new_xmin + new_xmax) / 2.0;
                    let new_cy = (new_ymin + new_ymax) / 2.0;
                    let (corners_arr, centre_id) = {
                        let pad = &editor.state.pads[pad_idx];
                        (
                            pad.corner_entity_ids
                                .expect("checked is_some above"),
                            pad.sketch_entity_id,
                        )
                    };
                    // Rewrite the centre Point's PadAttr size exprs
                    // FIRST so the next solve+bake reads the new
                    // size. solve_and_bake → refresh_pads_from_primitive
                    // overwrites state.pads.size_mm from the bake
                    // output, so any earlier write here gets wiped.
                    // Updating PadAttr ahead of the solve makes the
                    // bake produce the resized pad on its own.
                    if let Some(centre_id) = centre_id
                        && let Some(sketch) = editor.primitive_mut().sketch.as_mut()
                        && let Some(centre) =
                            sketch.entities.iter_mut().find(|e| e.id == centre_id)
                        && let Some(attr) = centre.pad.as_mut()
                    {
                        attr.size_x_expr = format!("{:.4}mm", new_w);
                        attr.size_y_expr = format!("{:.4}mm", new_h);
                    }
                    // Move the centre Point to the new bbox centre.
                    // Each apply_sketch_edit_with_warnings runs the
                    // solver + bake; refresh_pads_from_primitive then
                    // pulls state.pads from `footprint.pads`, so
                    // reading the centre's pre-edit position needs to
                    // happen RIGHT BEFORE this MovePoint emission.
                    if let Some(centre_id) = centre_id {
                        let cur_centre = editor
                            .primitive()
                            .sketch
                            .as_ref()
                            .and_then(|s| s.entities.iter().find(|e| e.id == centre_id))
                            .and_then(|e| {
                                if let EntityKind::Point { x, y } = e.kind {
                                    Some((x, y))
                                } else {
                                    None
                                }
                            });
                        if let Some((cur_cx, cur_cy)) = cur_centre {
                            let cdx = new_cx - cur_cx;
                            let cdy = new_cy - cur_cy;
                            if cdx.abs() > 1e-9 || cdy.abs() > 1e-9 {
                                editor.with_parts(|state, primitive| {
                                    apply_sketch_edit_with_warnings(
                                        state,
                                        primitive,
                                        SketchEdit::MovePoint {
                                            id: centre_id,
                                            dx: cdx,
                                            dy: cdy,
                                        },
                                    );
                                });
                            }
                        }
                    }
                    // Realign the 4 bbox corner Points to match the
                    // resized bbox. For Rect pads the line drag's
                    // victim loop already shifted the affected
                    // corners; for RoundRect / Oval / Chamfered the
                    // bbox corners aren't in `victims` so they need
                    // explicit catch-up here. Order: [ne, se, sw, nw]
                    // — see mint_pad_corner_outline.
                    let target_positions: [(f64, f64); 4] = [
                        (new_xmax, new_ymin), // ne
                        (new_xmax, new_ymax), // se
                        (new_xmin, new_ymax), // sw
                        (new_xmin, new_ymin), // nw
                    ];
                    for (corner_id, (target_x, target_y)) in
                        corners_arr.iter().zip(target_positions.iter())
                    {
                        let cur = editor
                            .primitive()
                            .sketch
                            .as_ref()
                            .and_then(|s| s.entities.iter().find(|e| e.id == *corner_id))
                            .and_then(|e| {
                                if let EntityKind::Point { x, y } = e.kind {
                                    Some((x, y))
                                } else {
                                    None
                                }
                            });
                        if let Some((cx, cy)) = cur {
                            let cdx = *target_x - cx;
                            let cdy = *target_y - cy;
                            if cdx.abs() > 1e-9 || cdy.abs() > 1e-9 {
                                editor.with_parts(|state, primitive| {
                                    apply_sketch_edit_with_warnings(
                                        state,
                                        primitive,
                                        SketchEdit::MovePoint {
                                            id: *corner_id,
                                            dx: cdx,
                                            dy: cdy,
                                        },
                                    );
                                });
                            }
                        }
                    }
                }
            }
            editor.with_parts(|state, primitive| {
                CanvasState::sync_pads_to_primitive(state, primitive);
            });
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintSketchResizeRoundPad {
            pad_idx,
            diameter_mm,
        } => {
            // v0.27 — round-pad diameter handle drag. Update three
            // sources of truth in lockstep so the on-canvas handle
            // motion, the bake output, and the parameter table stay
            // consistent:
            //   1. `pad.size_mm = (d, d)` — Editor mirror of the bbox.
            //   2. Circle entity radius — sketch-side geometry the
            //      Sketch overlay renders.
            //   3. `diameter_<slug>` parameter expression + the
            //      centre Point's PadAttr size_x_expr / size_y_expr —
            //      the bake reads these to emit the baked pad.
            let d = diameter_mm.max(0.05);
            let centre_id = editor.state.pads.get(pad_idx).and_then(|p| p.sketch_entity_id);
            let diameter_param =
                editor.state.pads.get(pad_idx).and_then(|p| {
                    p.shape_params.get("diameter").cloned()
                });
            if let Some(pad) = editor.state.pads.get_mut(pad_idx) {
                pad.size_mm = (d, d);
            }
            if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                use signex_sketch::entity::EntityKind;
                if let Some(cid) = centre_id {
                    for entity in sketch.entities.iter_mut() {
                        if let EntityKind::Circle { center, radius } = &mut entity.kind {
                            if *center == cid {
                                *radius = d / 2.0;
                            }
                        }
                        if entity.id == cid {
                            if let Some(attr) = entity.pad.as_mut() {
                                attr.size_x_expr = format!("{:.4}mm", d);
                                attr.size_y_expr = format!("{:.4}mm", d);
                            }
                        }
                    }
                }
                if let Some(name) = diameter_param.as_deref() {
                    sketch
                        .parameters
                        .insert(name.to_string(), format!("{:.4}mm", d));
                }
            }
            editor.with_parts(|state, primitive| {
                CanvasState::sync_pads_to_primitive(state, primitive);
            });
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintSetSelectionMode2d(mode) => {
            // v0.27 — active-bar Selection picker rows. The rubber-
            // band release picker reads this on commit so Inside /
            // Touching / Outside semantics apply.
            editor.state.selection_mode_2d = mode;
            editor.state.active_bar_menu = None;
        }
        PrimitiveEditorMsg::FootprintSelectAllOnLayer => {
            // v0.27 — multi-select every pad on the active layer.
            // Active layer = layer of the currently-selected pad,
            // or F.Cu when nothing is selected.
            let layer = editor
                .state
                .selected_pad
                .and_then(|idx| editor.state.pads.get(idx))
                .map(|p| p.primary_layer())
                .unwrap_or(crate::library::editor::footprint::layers::FpLayer::FCu);
            let mut matches: Vec<usize> = editor
                .state
                .pads
                .iter()
                .enumerate()
                .filter_map(|(idx, p)| {
                    if p.primary_layer() == layer {
                        Some(idx)
                    } else {
                        None
                    }
                })
                .collect();
            if matches.is_empty() {
                editor.state.selected_pad = None;
                editor.state.selected_pads_extra.clear();
            } else {
                editor.state.selected_pad = Some(matches.remove(0));
                editor.state.selected_pads_extra = matches;
            }
            editor.state.active_bar_menu = None;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintLassoArm => {
            editor.state.lasso_mode_active = true;
            editor.state.lasso_vertices.clear();
            editor.state.active_bar_menu = None;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintLassoAddVertex { x_mm, y_mm } => {
            if editor.state.lasso_mode_active {
                editor.state.lasso_vertices.push((x_mm, y_mm));
                editor.canvas_cache.clear();
            }
        }
        PrimitiveEditorMsg::FootprintLassoCancel => {
            editor.state.lasso_mode_active = false;
            editor.state.lasso_vertices.clear();
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintLassoCommit => {
            // v0.27 — close the polygon, multi-select every pad whose
            // centre lies inside (even-odd ray casting). Anything
            // less than three vertices is a degenerate polygon and
            // commits as deselect-all so a stray click doesn't leave
            // the user stuck in lasso mode with no feedback.
            let verts: Vec<(f64, f64)> = std::mem::take(&mut editor.state.lasso_vertices);
            editor.state.lasso_mode_active = false;
            let in_poly = |px: f64, py: f64| -> bool {
                if verts.len() < 3 {
                    return false;
                }
                let mut inside = false;
                let n = verts.len();
                let mut j = n - 1;
                for i in 0..n {
                    let (xi, yi) = verts[i];
                    let (xj, yj) = verts[j];
                    let denom = yj - yi;
                    if denom.abs() < 1e-10 {
                        j = i;
                        continue;
                    }
                    let intersect = ((yi > py) != (yj > py))
                        && (px < (xj - xi) * (py - yi) / denom + xi);
                    if intersect {
                        inside = !inside;
                    }
                    j = i;
                }
                inside
            };
            let mut hits: Vec<usize> = editor
                .state
                .pads
                .iter()
                .enumerate()
                .filter_map(|(idx, p)| {
                    if in_poly(p.position_mm.0, p.position_mm.1) {
                        Some(idx)
                    } else {
                        None
                    }
                })
                .collect();
            if hits.is_empty() {
                editor.state.selected_pad = None;
                editor.state.selected_pads_extra.clear();
            } else {
                editor.state.selected_pad = Some(hits.remove(0));
                editor.state.selected_pads_extra = hits;
            }
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintTouchingLineArm => {
            editor.state.touching_line_active = true;
            editor.state.touching_line_first = None;
            editor.state.active_bar_menu = None;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintTouchingLineFirst { x_mm, y_mm } => {
            if editor.state.touching_line_active {
                editor.state.touching_line_first = Some((x_mm, y_mm));
                editor.canvas_cache.clear();
            }
        }
        PrimitiveEditorMsg::FootprintTouchingLineCancel => {
            editor.state.touching_line_active = false;
            editor.state.touching_line_first = None;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintTouchingLineCommit { x_mm, y_mm } => {
            // v0.27 — Touching Line: every pad whose bbox is
            // intersected by the segment from `touching_line_first`
            // → (x_mm, y_mm) becomes selected. Liang-Barsky-style
            // segment-vs-AABB clip.
            let Some((sx, sy)) = editor.state.touching_line_first.take() else {
                editor.state.touching_line_active = false;
                editor.canvas_cache.clear();
                return;
            };
            editor.state.touching_line_active = false;
            let dx = x_mm - sx;
            let dy = y_mm - sy;
            let segment_hits_aabb = |xmin: f64, ymin: f64, xmax: f64, ymax: f64| -> bool {
                // Both endpoints inside?
                let inside = |x: f64, y: f64| -> bool {
                    x >= xmin && x <= xmax && y >= ymin && y <= ymax
                };
                if inside(sx, sy) || inside(x_mm, y_mm) {
                    return true;
                }
                // Liang-Barsky parametric clip in [0, 1].
                let mut t_enter = 0.0_f64;
                let mut t_exit = 1.0_f64;
                let p = [-dx, dx, -dy, dy];
                let q = [sx - xmin, xmax - sx, sy - ymin, ymax - sy];
                for i in 0..4 {
                    if p[i].abs() < 1e-12 {
                        if q[i] < 0.0 {
                            return false;
                        }
                    } else {
                        let t = q[i] / p[i];
                        if p[i] < 0.0 {
                            if t > t_exit {
                                return false;
                            }
                            if t > t_enter {
                                t_enter = t;
                            }
                        } else {
                            if t < t_enter {
                                return false;
                            }
                            if t < t_exit {
                                t_exit = t;
                            }
                        }
                    }
                }
                t_enter <= t_exit
            };
            let mut hits: Vec<usize> = editor
                .state
                .pads
                .iter()
                .enumerate()
                .filter_map(|(idx, p)| {
                    let (x0, y0, x1, y1) = p.bbox_mm();
                    if segment_hits_aabb(x0, y0, x1, y1) {
                        Some(idx)
                    } else {
                        None
                    }
                })
                .collect();
            if hits.is_empty() {
                editor.state.selected_pad = None;
                editor.state.selected_pads_extra.clear();
            } else {
                editor.state.selected_pad = Some(hits.remove(0));
                editor.state.selected_pads_extra = hits;
            }
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintSelectOverlapped
        | PrimitiveEditorMsg::FootprintSelectNextOverlapped => {
            // v0.27 — Cycle through pads stacked at the most recent
            // click world position. SelectOverlapped goes to the
            // previous pad in z-order; SelectNextOverlapped advances.
            // Without a recorded click position there's no stack to
            // cycle, so the action is a silent no-op.
            let forward = matches!(msg, PrimitiveEditorMsg::FootprintSelectNextOverlapped);
            let Some((wx, wy)) = editor.state.last_click_world_mm else {
                editor.state.active_bar_menu = None;
                return;
            };
            let stack: Vec<usize> = editor
                .state
                .pads
                .iter()
                .enumerate()
                .filter_map(|(idx, p)| if p.contains_mm(wx, wy) { Some(idx) } else { None })
                .collect();
            if stack.is_empty() {
                editor.state.active_bar_menu = None;
                return;
            }
            // Pick next/prev relative to the current primary selection.
            let cur_pos = editor
                .state
                .selected_pad
                .and_then(|s| stack.iter().position(|&i| i == s));
            let next_idx = match cur_pos {
                Some(p) => {
                    if forward {
                        (p + 1) % stack.len()
                    } else {
                        (p + stack.len() - 1) % stack.len()
                    }
                }
                None => 0,
            };
            editor.state.selected_pad = Some(stack[next_idx]);
            editor.state.selected_pads_extra.clear();
            editor.state.active_bar_menu = None;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintRecomputeCourtyardOutline => {
            // v0.27 — outline-following courtyard. Pure read-write
            // on the editor state; the new polygon lands on
            // `state.courtyard_outline_mm` and the canvas draws it
            // in preference to the bbox.
            editor.state.recompute_courtyard_outline();
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintSelectOffGridPads => {
            // v0.27 — pads whose centre falls between grid steps.
            // The active grid step lives on snap_options; defaults
            // to 1 mm. Tolerance is 1% of the step so pads exactly
            // on the grid (with floating-point noise) don't
            // false-positive.
            let step = editor.state.snap_options.grid_step_mm.max(1e-6);
            let tol = step * 0.01;
            let on_grid = |v: f64| -> bool {
                let r = (v / step).round() * step;
                (v - r).abs() <= tol
            };
            let mut matches: Vec<usize> = editor
                .state
                .pads
                .iter()
                .enumerate()
                .filter_map(|(idx, p)| {
                    let (x, y) = p.position_mm;
                    if !on_grid(x) || !on_grid(y) {
                        Some(idx)
                    } else {
                        None
                    }
                })
                .collect();
            if matches.is_empty() {
                editor.state.selected_pad = None;
                editor.state.selected_pads_extra.clear();
            } else {
                editor.state.selected_pad = Some(matches.remove(0));
                editor.state.selected_pads_extra = matches;
            }
            editor.state.active_bar_menu = None;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintSketchDimensionInput(s) => {
            editor.state.dimension_input = s;
        }
        PrimitiveEditorMsg::FootprintSketchSetRole { id, role } => {
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_role_with_warnings;
            use crate::library::editor::footprint::state::EditorPad;
            use signex_library::primitive::footprint::{
                LayerId, PadKind as LibPadKind, PadShape as LibPadShape,
            };

            // v0.27 — the Role=Pad-on-a-Line case is rewritten to
            // MakePadFromProfile at the top of
            // `apply_footprint_primitive_edit`, so this arm only
            // sees Point-targeted Pad assignments + every other
            // role. PadAttr is Point-only on the schema side, so
            // dispatching to `apply_sketch_role_with_warnings` is
            // always meaningful from here on.
            editor.with_parts(|state, primitive| {
                apply_sketch_role_with_warnings(state, primitive, id, role);
            });

            // Diff `state.pads` against the entity's new role so the
            // canvas's pad list mirrors role assignments. Per-entity
            // diff (rather than full rebuild from `primitive.pads`)
            // preserves `sketch_entity_id` + `corner_entity_ids` on
            // previously auto-minted Pads-mode pads.
            let entity_has_pad = editor
                .primitive()
                .sketch
                .as_ref()
                .and_then(|s| s.entities.iter().find(|e| e.id == id))
                .map(|e| e.pad.is_some())
                .unwrap_or(false);
            let existing_idx = editor
                .state
                .pads
                .iter()
                .position(|p| p.sketch_entity_id == Some(id));
            match (entity_has_pad, existing_idx) {
                (true, None) => {
                    use signex_sketch::entity::EntityKind;
                    let (x, y, number) = editor
                        .primitive()
                        .sketch
                        .as_ref()
                        .and_then(|s| s.entities.iter().find(|e| e.id == id))
                        .map(|e| {
                            let (x, y) = match e.kind {
                                EntityKind::Point { x, y } => (x, y),
                                _ => (0.0, 0.0),
                            };
                            let num = e.pad.as_ref().map(|a| a.number.clone()).unwrap_or_default();
                            (x, y, num)
                        })
                        .unwrap_or((0.0, 0.0, String::new()));
                    editor.state.pads.push(EditorPad {
                        number,
                        position_mm: (x, y),
                        size_mm: (1.0, 1.0),
                        kind: LibPadKind::Smd,
                        shape: LibPadShape::Rect,
                        layers: vec![LayerId::new("Top Layer")],
                        sketch_entity_id: Some(id),
                        corner_entity_ids: None,
                        rotation_deg: 0.0,
                        drill_diameter_mm: None,
                        stack: crate::library::editor::footprint::state::PadStackUi::default(),
                        feature_top: signex_sketch::attr::PadFeature::None,
                        feature_bottom: signex_sketch::attr::PadFeature::None,
                        testpoint: signex_sketch::attr::TestpointFlags::default(),
                        template: String::new(),
                        template_library: String::new(),
                        electrical_type: signex_sketch::attr::ElectricalType::Load,
                        net: String::new(),
                        locked: false,
                        hole_tolerance_plus_mm: None,
                        hole_tolerance_minus_mm: None,
                        hole_rotation_deg: None,
                        copper_offset_x_mm: None,
                        copper_offset_y_mm: None,
                        shape_params: crate::library::editor::footprint::state::ShapeParamMap::new(),
                    });
                }
                (false, Some(idx)) => {
                    editor.state.pads.remove(idx);
                    if editor.state.selected_pad == Some(idx) {
                        editor.state.selected_pad = None;
                    } else if let Some(sel) = editor.state.selected_pad {
                        if sel > idx {
                            editor.state.selected_pad = Some(sel - 1);
                        }
                    }
                }
                _ => {}
            }
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintSketchMakePadFromProfile => {
            // v0.22 Phase D4 — convert the closed-loop profile that
            // includes the currently-selected Line into a
            // `PadShape::Custom(SketchProfile)` pad.
            //
            // Walk: start from the selected Line, use
            // `signex_bake::profile::trace_closed_profile` to chase
            // the unique-incident-edge cycle in the sketch. On
            // success, compute the centroid of the traced vertices,
            // mint a centre `Point` there, and attach a `PadAttr`
            // whose `shape` is `Custom(SketchProfile{source: vec![
            // seed_line_id]})`. The bake re-walks the loop on the
            // next solve and emits a `LibPadShape::Custom` polygon.
            //
            // Designator: `next_pad_num` from existing `PadAttr`
            // entities, identical pattern to
            // `apply_sketch_role(.., RoleTag::Pad)` for ordering
            // consistency.
            //
            // Fail modes (silent except for warning push):
            // - No Line selected → "select a Line first".
            // - Line is not part of a closed loop → "loop is open
            //   or branches".
            // - `last_solve` is None (no solve has run yet) → ask
            //   user to interact briefly so a solve fires, then
            //   retry. (Auto-mint paths on entry to Sketch mode
            //   already trigger a solve, so this is rare.)
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            use signex_sketch::attr::{
                CustomPadShape, PadAttr, PadKind, PadShape, PadSide,
                PasteAperturePattern,
            };
            use signex_sketch::entity::{Entity, EntityKind};
            use signex_sketch::id::SketchEntityId;
            use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

            // v0.27 — walk the full sketch selection (primary +
            // secondary + extras) for the first Line. The
            // closed-loop walker doesn't care which seed edge it
            // gets — any Line on the loop seeds the trace. Falls
            // back to scanning every sketch Line when nothing
            // suitable is selected, so the action also works on a
            // bare "select-nothing-and-click-Make-Pad" workflow.
            let line_id: SketchEntityId = {
                let sketch_lookup = editor.primitive().sketch.as_ref();
                let kind_of = |id: SketchEntityId| -> Option<EntityKind> {
                    sketch_lookup
                        .and_then(|s| s.entities.iter().find(|e| e.id == id))
                        .map(|e| e.kind.clone())
                };
                let selection: Vec<SketchEntityId> = editor
                    .state
                    .selected_sketch
                    .into_iter()
                    .chain(editor.state.selected_sketch_secondary.into_iter())
                    .chain(editor.state.selected_sketch_extra.iter().copied())
                    .collect();

                // First pass — Line directly in the selection.
                let direct_line = selection
                    .iter()
                    .find(|id| matches!(kind_of(**id), Some(EntityKind::Line { .. })))
                    .copied();
                // Second pass — a selected Point's incident Line.
                let incident_line = selection.iter().find_map(|id| {
                    if matches!(kind_of(*id), Some(EntityKind::Point { .. })) {
                        sketch_lookup.and_then(|s| {
                            s.entities
                                .iter()
                                .find(|e| match e.kind {
                                    EntityKind::Line { start, end } => {
                                        start == *id || end == *id
                                    }
                                    _ => false,
                                })
                                .map(|e| e.id)
                        })
                    } else {
                        None
                    }
                });
                // Third pass — any sketch Line at all.
                let any_line = sketch_lookup.and_then(|s| {
                    s.entities
                        .iter()
                        .find(|e| matches!(e.kind, EntityKind::Line { .. }))
                        .map(|e| e.id)
                });

                match direct_line.or(incident_line).or(any_line) {
                    Some(id) => id,
                    None => {
                        editor.state.solve_warnings.push(
                            "Make Pad from Profile: no Lines in the sketch — draw a closed shape first"
                                .into(),
                        );
                        editor.canvas_cache.clear();
                        return;
                    }
                }
            };

            // Walk the loop to compute the centroid; needs a fresh
            // solve so vertex positions are accurate.
            let solve = match editor.state.last_solve.as_ref() {
                Some(s) => s,
                None => {
                    editor.state.solve_warnings.push(
                        "Make Pad from Profile: no solve has run yet — interact briefly to trigger a solve, then retry"
                            .into(),
                    );
                    editor.canvas_cache.clear();
                    return;
                }
            };
            let sketch_for_walk = match editor.primitive().sketch.as_ref() {
                Some(s) => s,
                None => return,
            };

            let trace =
                signex_bake::profile::trace_closed_profile(sketch_for_walk, solve, line_id);
            let vertices = match trace {
                Ok(v) if v.len() >= 3 => v,
                Ok(_) => {
                    editor.state.solve_warnings.push(
                        "Make Pad from Profile: traced loop has fewer than 3 vertices".into(),
                    );
                    editor.canvas_cache.clear();
                    return;
                }
                Err(e) => {
                    editor.state.solve_warnings.push(format!(
                        "Make Pad from Profile: loop walk failed ({e:?}) — the loop must be closed and non-branching"
                    ));
                    editor.canvas_cache.clear();
                    return;
                }
            };
            // v0.27 — area-weighted centroid + axis-aligned bbox of
            // the closed-loop polygon. The arithmetic mean of vertex
            // positions only matches the geometric centroid for
            // regular polygons; for an arbitrary triangle / outline
            // it lands biased toward whichever side has the most
            // densely-spaced vertices (which is why the user saw
            // the pad mint near a corner instead of inside the
            // shape). The shoelace centroid is the proper EDA
            // answer — pad sits at the geometric middle of its own
            // copper outline.
            let n_v = vertices.len();
            let mut signed_area = 0.0_f64;
            let mut cx_acc = 0.0_f64;
            let mut cy_acc = 0.0_f64;
            for i in 0..n_v {
                let (x0, y0) = (vertices[i][0], vertices[i][1]);
                let (x1, y1) = (
                    vertices[(i + 1) % n_v][0],
                    vertices[(i + 1) % n_v][1],
                );
                let cross = x0 * y1 - x1 * y0;
                signed_area += cross;
                cx_acc += (x0 + x1) * cross;
                cy_acc += (y0 + y1) * cross;
            }
            let area = signed_area * 0.5;
            let (cx, cy) = if area.abs() > 1e-12 {
                let s = 1.0 / (6.0 * area);
                (cx_acc * s, cy_acc * s)
            } else {
                // Degenerate polygon — fall back to mean.
                let n = n_v as f64;
                (
                    vertices.iter().map(|p| p[0]).sum::<f64>() / n,
                    vertices.iter().map(|p| p[1]).sum::<f64>() / n,
                )
            };
            // Axis-aligned bbox — drives `size_x_expr` / `size_y_expr`
            // so the synced `state.pads` row is at least bbox-sized
            // (instead of the default 1mm × 1mm). Polygon-shape
            // rendering on the editor canvas is a follow-up.
            let mut min_x = f64::INFINITY;
            let mut min_y = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            let mut max_y = f64::NEG_INFINITY;
            for p in &vertices {
                if p[0] < min_x {
                    min_x = p[0];
                }
                if p[1] < min_y {
                    min_y = p[1];
                }
                if p[0] > max_x {
                    max_x = p[0];
                }
                if p[1] > max_y {
                    max_y = p[1];
                }
            }
            let bbox_w = (max_x - min_x).max(0.05);
            let bbox_h = (max_y - min_y).max(0.05);

            // Plane: reuse the seed Line's plane so the new pad
            // entity ends up on the same one.
            let plane_id = sketch_for_walk
                .entities
                .iter()
                .find(|e| e.id == line_id)
                .map(|e| e.plane)
                .unwrap_or_else(|| {
                    sketch_for_walk
                        .planes
                        .first()
                        .map(|p| p.id)
                        .unwrap_or_else(PlaneId::new)
                });
            // Ensure plane exists (defensive — almost always already
            // in `sketch.planes`).
            let _ = Plane {
                id: plane_id,
                kind: PlaneKind::BoardTop,
            };

            // Next pad designator from existing pad attrs.
            let next_pad_num = sketch_for_walk
                .entities
                .iter()
                .filter_map(|e| e.pad.as_ref())
                .filter_map(|attr| attr.number.parse::<u32>().ok())
                .max()
                .unwrap_or(0)
                + 1;

            let centre_id = SketchEntityId::new();
            let mut centre = Entity::new(
                centre_id,
                plane_id,
                EntityKind::Point { x: cx, y: cy },
            );
            centre.pad = Some(PadAttr {
                number: next_pad_num.to_string(),
                kind: PadKind::Smd,
                side: PadSide::Top,
                shape: PadShape::Custom(CustomPadShape::SketchProfile {
                    source: vec![line_id],
                }),
                size_x_expr: format!("{:.3}mm", bbox_w),
                size_y_expr: format!("{:.3}mm", bbox_h),
                rotation_expr: None,
                offset_x_expr: None,
                offset_y_expr: None,
                drill: None,
                mask_margin_expr: None,
                paste_margin_expr: None,
                paste_apertures: PasteAperturePattern::Single,
                ..PadAttr::default()
            });
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(
                    state,
                    primitive,
                    SketchEdit::AddEntity(centre),
                );
            });
            // v0.27 — pivot the selection onto the new pad's centre
            // Point. Without this, the Role dropdown still reads
            // "Unassigned" because the user's prior selection (the
            // Line we walked) has no PadAttr — the new PadAttr lives
            // on the freshly-minted centre. Clearing extras avoids
            // a confusing "primary is the centre but extras still
            // point at the loop's lines" state right after Make Pad.
            editor.state.selected_sketch = Some(centre_id);
            editor.state.selected_sketch_secondary = None;
            editor.state.selected_sketch_extra.clear();
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintSketchUnlinkCornerRadius { arc_entity_id } => {
            // v0.24 Phase 3 (Track A3) — split a RoundRect pad's
            // shared `corner_r_<slug>` parameter into a per-corner
            // override for the right-clicked Arc.
            //
            // Lookup chain:
            //   1. Walk every EditorPad to find the one whose
            //      `shape_params` contains a `corner_r_<corner>_arc`
            //      key whose value (UUID slug) matches `arc_entity_id`.
            //   2. From that match, derive the corner key
            //      (`corner_r_ne` / `_se` / `_sw` / `_nw`).
            //   3. Mint a fresh parameter `<shared_name>_<corner>`,
            //      copy the current shared expression as its value,
            //      and bind the corner key on `pad.shape_params`.
            //   4. Trigger a `ForceRebuild` so the solver re-runs and
            //      the bake reflects the new parametric link.
            //
            // Defensive: arc not part of any pad → tracing::warn +
            // no-op. Pad has no shared `corner_r` binding (e.g.
            // legacy data) → tracing::warn + no-op.
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;

            let arc_id_str = arc_entity_id.0.simple().to_string();

            // Locate the pad + corner this arc belongs to.
            let pad_corner: Option<(usize, &'static str)> = editor
                .state
                .pads
                .iter()
                .enumerate()
                .find_map(|(idx, pad)| {
                    let arc_keys: [(&str, &str); 4] = [
                        ("corner_r_ne_arc", "corner_r_ne"),
                        ("corner_r_se_arc", "corner_r_se"),
                        ("corner_r_sw_arc", "corner_r_sw"),
                        ("corner_r_nw_arc", "corner_r_nw"),
                    ];
                    for (sidecar_key, corner_key) in arc_keys {
                        if pad.shape_params.get(sidecar_key).map(|s| s.as_str())
                            == Some(arc_id_str.as_str())
                        {
                            return Some((idx, corner_key));
                        }
                    }
                    None
                });

            let Some((pad_idx, corner_key)) = pad_corner else {
                tracing::warn!(
                    target: "signex::v024",
                    "FootprintSketchUnlinkCornerRadius: arc {arc_entity_id:?} doesn't belong \
                     to any pad's shape_params; ignoring"
                );
                return;
            };

            // Already unlinked → no-op (idempotent).
            if editor.state.pads[pad_idx]
                .shape_params
                .contains_key(corner_key)
            {
                tracing::warn!(
                    target: "signex::v024",
                    "FootprintSketchUnlinkCornerRadius: corner {corner_key} on pad {pad_idx} \
                     is already unlinked; ignoring"
                );
                return;
            }

            // Resolve the shared parameter name + current value.
            let shared_name = match editor.state.pads[pad_idx]
                .shape_params
                .get("corner_r")
                .cloned()
            {
                Some(n) => n,
                None => {
                    tracing::warn!(
                        target: "signex::v024",
                        "FootprintSketchUnlinkCornerRadius: pad {pad_idx} has no shared \
                         corner_r binding; ignoring"
                    );
                    return;
                }
            };
            let shared_value = editor
                .primitive()
                .sketch
                .as_ref()
                .and_then(|s| s.parameters.get_raw(&shared_name).map(str::to_string))
                .unwrap_or_default();

            // Mint the per-corner parameter name. Use the corner_key
            // suffix (e.g. `_ne`) appended to the shared name's slug
            // so the per-corner names cluster together in the
            // parameter table for inspection.
            let corner_suffix = corner_key
                .strip_prefix("corner_r_")
                .unwrap_or(corner_key);
            let new_param_name = format!("{shared_name}_{corner_suffix}");

            // Apply the rewrite. push_history is already captured at
            // the top of this dispatcher arm via mutates_footprint_state.
            editor.with_parts(|state, primitive| {
                // Mint the new parameter on the sketch.
                if let Some(sketch) = primitive.sketch.as_mut() {
                    sketch.parameters.insert(new_param_name.clone(), shared_value.clone());
                }
                // Record the per-corner override on the pad.
                if let Some(pad) = state.pads.get_mut(pad_idx) {
                    pad.shape_params
                        .insert(corner_key.to_string(), new_param_name.clone());
                }
                // ForceRebuild → solver re-runs, bake regenerates pad
                // geometry from the (now per-corner-aware) parameters.
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
            });
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintSketchAddConstraintForSelection(tag) => {
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            use crate::library::messages::SketchConstraintTag;
            use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
            use signex_sketch::id::ConstraintId;

            let primary = editor.state.selected_sketch;
            let secondary = editor.state.selected_sketch_secondary;
            let dim_target = editor
                .state
                .dimension_input
                .trim()
                .parse::<f64>()
                .ok()
                .map(DimTarget::Literal);

            // Determine selected entity kinds (Point / Line / Arc / Circle)
            // by inspecting the sketch.
            let kind_of = |id: signex_sketch::id::SketchEntityId| -> Option<&'static str> {
                use signex_sketch::entity::EntityKind;
                editor
                    .primitive()
                    .sketch
                    .as_ref()?
                    .entities
                    .iter()
                    .find(|e| e.id == id)
                    .map(|e| match e.kind {
                        EntityKind::Point { .. } => "Point",
                        EntityKind::Line { .. } => "Line",
                        EntityKind::Arc { .. } => "Arc",
                        EntityKind::Circle { .. } => "Circle",
                    })
            };
            let p_kind = primary.and_then(kind_of);
            let s_kind = secondary.and_then(kind_of);
            // v0.15 — third entity for the 3-entity Symmetric
            // constraints comes from the rubber-band extra slot.
            let extra = editor.state.selected_sketch_extra.first().copied();
            let extra_kind = extra.and_then(kind_of);
            // Angle's DimTarget is stored in radians (canonical unit);
            // the dim-input field is degrees, so convert here.
            let angle_target = editor
                .state
                .dimension_input
                .trim()
                .parse::<f64>()
                .ok()
                .map(|deg| DimTarget::Literal(deg.to_radians()));

            let new_kind: Option<ConstraintKind> = match (tag, p_kind, s_kind, primary, secondary) {
                (SketchConstraintTag::Fixed, Some("Point"), _, Some(p), _) => {
                    Some(ConstraintKind::Fixed { point: p })
                }
                (
                    SketchConstraintTag::Coincident,
                    Some("Point"),
                    Some("Point"),
                    Some(p1),
                    Some(p2),
                ) => Some(ConstraintKind::Coincident { p1, p2 }),
                (
                    SketchConstraintTag::DistancePtPt,
                    Some("Point"),
                    Some("Point"),
                    Some(p1),
                    Some(p2),
                ) => dim_target.map(|t| ConstraintKind::DistancePtPt { p1, p2, target: t }),
                (SketchConstraintTag::Horizontal, Some("Line"), _, Some(l), _) => {
                    Some(ConstraintKind::Horizontal { line: l })
                }
                (SketchConstraintTag::Vertical, Some("Line"), _, Some(l), _) => {
                    Some(ConstraintKind::Vertical { line: l })
                }
                (SketchConstraintTag::Parallel, Some("Line"), Some("Line"), Some(l1), Some(l2)) => {
                    Some(ConstraintKind::Parallel { l1, l2 })
                }
                (
                    SketchConstraintTag::Perpendicular,
                    Some("Line"),
                    Some("Line"),
                    Some(l1),
                    Some(l2),
                ) => Some(ConstraintKind::Perpendicular { l1, l2 }),
                (
                    SketchConstraintTag::EqualLength,
                    Some("Line"),
                    Some("Line"),
                    Some(l1),
                    Some(l2),
                ) => Some(ConstraintKind::EqualLength { l1, l2 }),
                (
                    SketchConstraintTag::PointOnLine,
                    Some("Point"),
                    Some("Line"),
                    Some(p),
                    Some(l),
                ) => Some(ConstraintKind::PointOnLine { point: p, line: l }),
                (
                    SketchConstraintTag::PointOnLine,
                    Some("Line"),
                    Some("Point"),
                    Some(l),
                    Some(p),
                ) => Some(ConstraintKind::PointOnLine { point: p, line: l }),
                (SketchConstraintTag::Midpoint, Some("Point"), Some("Line"), Some(p), Some(l)) => {
                    Some(ConstraintKind::Midpoint { point: p, line: l })
                }
                (SketchConstraintTag::Midpoint, Some("Line"), Some("Point"), Some(l), Some(p)) => {
                    Some(ConstraintKind::Midpoint { point: p, line: l })
                }
                // --- v0.15: 9 additional constraint kinds ---
                (
                    SketchConstraintTag::TangentLineArc,
                    Some("Line"),
                    Some("Arc"),
                    Some(line),
                    Some(arc),
                ) => Some(ConstraintKind::TangentLineArc { line, arc }),
                (
                    SketchConstraintTag::TangentLineArc,
                    Some("Arc"),
                    Some("Line"),
                    Some(arc),
                    Some(line),
                ) => Some(ConstraintKind::TangentLineArc { line, arc }),
                (
                    SketchConstraintTag::TangentArcArc,
                    Some("Arc"),
                    Some("Arc"),
                    Some(a1),
                    Some(a2),
                ) => Some(ConstraintKind::TangentArcArc {
                    a1,
                    a2,
                    internal: false,
                }),
                (SketchConstraintTag::Angle, Some("Line"), Some("Line"), Some(l1), Some(l2)) => {
                    angle_target.map(|t| ConstraintKind::Angle { l1, l2, target: t })
                }
                // EqualRadius spans any two of Circle / Arc.
                (
                    SketchConstraintTag::EqualRadius,
                    Some("Circle") | Some("Arc"),
                    Some("Circle") | Some("Arc"),
                    Some(e1),
                    Some(e2),
                ) => Some(ConstraintKind::EqualRadius { e1, e2 }),
                (
                    SketchConstraintTag::PointOnArc,
                    Some("Point"),
                    Some("Arc"),
                    Some(point),
                    Some(arc),
                ) => Some(ConstraintKind::PointOnArc { point, arc }),
                (
                    SketchConstraintTag::PointOnArc,
                    Some("Arc"),
                    Some("Point"),
                    Some(arc),
                    Some(point),
                ) => Some(ConstraintKind::PointOnArc { point, arc }),
                (
                    SketchConstraintTag::DistancePtLine,
                    Some("Point"),
                    Some("Line"),
                    Some(point),
                    Some(line),
                ) => dim_target.map(|t| ConstraintKind::DistancePtLine {
                    point,
                    line,
                    target: t,
                }),
                (
                    SketchConstraintTag::DistancePtLine,
                    Some("Line"),
                    Some("Point"),
                    Some(line),
                    Some(point),
                ) => dim_target.map(|t| ConstraintKind::DistancePtLine {
                    point,
                    line,
                    target: t,
                }),
                // DistancePtCircle: the `circle` field accepts a Circle
                // or an Arc (radius read from live state in both cases).
                (
                    SketchConstraintTag::DistancePtCircle,
                    Some("Point"),
                    Some("Circle") | Some("Arc"),
                    Some(point),
                    Some(circle),
                ) => dim_target.map(|t| ConstraintKind::DistancePtCircle {
                    point,
                    circle,
                    target: t,
                }),
                (
                    SketchConstraintTag::DistancePtCircle,
                    Some("Circle") | Some("Arc"),
                    Some("Point"),
                    Some(circle),
                    Some(point),
                ) => dim_target.map(|t| ConstraintKind::DistancePtCircle {
                    point,
                    circle,
                    target: t,
                }),
                // 3-entity Symmetric constraints: primary + secondary
                // are the two governed Points; the third entity (mirror
                // Line / centre Point) comes from the extra slot.
                (
                    SketchConstraintTag::SymmetricAboutLine,
                    Some("Point"),
                    Some("Point"),
                    Some(p1),
                    Some(p2),
                ) if extra_kind == Some("Line") => extra
                    .map(|line| ConstraintKind::SymmetricAboutLine { p1, p2, line }),
                (
                    SketchConstraintTag::SymmetricAboutPoint,
                    Some("Point"),
                    Some("Point"),
                    Some(p1),
                    Some(p2),
                ) if extra_kind == Some("Point") => extra
                    .map(|center| ConstraintKind::SymmetricAboutPoint { p1, p2, center }),
                _ => None,
            };

            if let Some(kind) = new_kind {
                let constraint = Constraint {
                    id: ConstraintId::new(),
                    kind,
                };
                editor.with_parts(|state, primitive| {
                    apply_sketch_edit_with_warnings(
                        state,
                        primitive,
                        SketchEdit::AddConstraint(constraint),
                    );
                });
                editor.dirty = true;
                editor.canvas_cache.clear();
            }
        }
        PrimitiveEditorMsg::FootprintSketchToolClick {
            x_mm,
            y_mm,
            snap_id,
        } => {
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            use crate::library::editor::footprint::state::{
                PlacementInputKind, SketchTool, ToolPending,
            };
            use signex_sketch::entity::{Entity, EntityKind};
            use signex_sketch::id::SketchEntityId;
            use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

            // v0.14-footprint — TAB-pause is the single source of truth
            // for "suppress click-commit". The canvas layer also gates
            // on `placement_paused` before publishing this message, but
            // multi-click tools (Line / RoundedRectangle / Arc / …)
            // route BOTH their anchor click and their commit click
            // through this one handler, so the authoritative gate lives
            // here too: while paused, drop the click before it can
            // advance `tool_pending` or mint geometry. The Select tool
            // never reaches this arm, so re-anchoring stays possible.
            if editor.state.placement_paused {
                return;
            }

            // v0.16.1 — sticky construction flag captured once so each
            // newly-minted entity can be flagged in one place. Pads
            // (PadAttr-carrying centre Points minted via auto_mint /
            // mirror_add) intentionally bypass this; the bake skips
            // construction entities and a construction pad would
            // disappear from the rendered output.
            let construction_mode = editor.state.construction_mode;
            let centerline_mode = editor.state.centerline_mode;
            let mut flag = |mut e: Entity| -> Entity {
                e.construction = construction_mode;
                e.centerline = centerline_mode;
                e
            };

            // Resolve the click into either an existing snap Point or a
            // freshly-minted Point. For multi-click tools (Line / Rect /
            // Circle / Arc), the dispatcher reuses the snap target's ID
            // so closed-loop detection (canvas.rs::draw_filled_closed_
            // loops) continues to recognise cycles by shared endpoint
            // ID. Otherwise it appends a Point at the click position
            // and uses that new ID for the active tool's gesture state.
            //
            // v0.22 Phase A1 — Auto-Coincident inference for the
            // Place-Point tool. A Place-Point click on an existing
            // Point used to be a silent no-op (snap_id was returned
            // but never acted upon). It now mints a fresh Point at
            // the snap target and pins it to the target with a
            // Coincident constraint, so the user gets a Fusion-style
            // "place a point coincident with this one" gesture
            // without having to switch to the Constraint sub-tool.
            // Multi-click tools deliberately keep shared-ID
            // semantics — their endpoint ID is the bake's vertex
            // identity and switching to constraint-merged points
            // would silently break the closed-loop walker.
            let plane_id = match editor.primitive().sketch.as_ref() {
                Some(s) if !s.planes.is_empty() => s.planes[0].id,
                _ => {
                    let pid = PlaneId::new();
                    let sketch = editor
                        .primitive_mut()
                        .sketch
                        .get_or_insert_with(signex_sketch::SketchData::default);
                    sketch.planes.push(Plane {
                        id: pid,
                        kind: PlaneKind::BoardTop,
                    });
                    pid
                }
            };

            // v0.24 Track D — consume `state.placement_input` if it
            // matches the active tool's pending state. The buffer is
            // parsed as `f64` mm (length / radius) or degrees
            // (sweep), translated into an effective click position
            // overriding `x_mm` / `y_mm`, and the snap target is
            // dropped so the typed-length wins over a coincidence
            // hit. Returns the effective `(x, y)` and a flag whose
            // `true` value means the click was geometry-pinned by a
            // numeric input — used to (1) ignore `snap_id` and (2)
            // clear `state.placement_input` after the gesture
            // commits.
            let placement_input_kind = editor.state.placement_input.as_ref().map(|p| p.kind);
            let placement_input_value = editor
                .state
                .placement_input
                .as_ref()
                .and_then(|p| p.buffer.parse::<f64>().ok());
            // v0.14-footprint — multi-dimension tools (Line len/angle,
            // Rectangle w/h, Rounded-Rect w/h/radius) keep the focused
            // field in `placement_input` and the rest in
            // `placement_input_others`. Pull a field's parsed value out
            // of whichever slot holds it so the commit arms can honour
            // any combination regardless of which field has focus.
            let field_value = |kind: PlacementInputKind| -> Option<f64> {
                std::iter::once(editor.state.placement_input.as_ref())
                    .chain(editor.state.placement_input_others.iter().map(Some))
                    .flatten()
                    .find(|p| p.kind == kind)
                    .and_then(|p| p.buffer.parse::<f64>().ok())
            };
            let line_len_typed = field_value(PlacementInputKind::LineLength);
            let line_ang_typed = field_value(PlacementInputKind::LineAngle);
            let rect_w_typed = field_value(PlacementInputKind::RectWidth);
            let rect_h_typed = field_value(PlacementInputKind::RectHeight);
            let resolve_point_xy =
                |id: SketchEntityId, primitive: &signex_library::primitive::footprint::Footprint| -> Option<(f64, f64)> {
                    primitive
                        .sketch
                        .as_ref()
                        .and_then(|s| s.entities.iter().find(|e| e.id == id))
                        .and_then(|e| match e.kind {
                            EntityKind::Point { x, y } => Some((x, y)),
                            _ => None,
                        })
                };
            let (eff_x_mm, eff_y_mm, used_placement_input): (f64, f64, bool) = match (
                placement_input_kind,
                placement_input_value,
                editor.state.active_tool,
                editor.state.tool_pending.clone(),
            ) {
                // Line second click — honour any typed length / angle.
                // v0.14-footprint:
                //   • length + angle → endpoint = first + (len @ angle°)
                //   • length only    → len along the cursor azimuth (legacy)
                //   • angle only     → azimuth pinned to angle°, length
                //                      taken from the cursor distance
                // The angle is degrees CCW from +X in world space, the
                // same convention the live ghost-preview pill displays
                // (draw_sketch.rs), so the committed segment matches the
                // number the user saw while placing.
                (_, _, SketchTool::Line, ToolPending::LineFirst { first })
                    if line_len_typed.is_some() || line_ang_typed.is_some() =>
                {
                    let primitive = editor.primitive();
                    if let Some((fx, fy)) = resolve_point_xy(first, primitive) {
                        let dx = x_mm - fx;
                        let dy = y_mm - fy;
                        let cursor_len = (dx * dx + dy * dy).sqrt();
                        // World azimuth of the cursor relative to the
                        // first endpoint; 0 when the cursor sits exactly
                        // on `first` (no direction to read).
                        let cursor_ang = if cursor_len > 1e-9 {
                            dy.atan2(dx)
                        } else {
                            0.0
                        };
                        // Typed angle wins; else follow the cursor.
                        let ang_rad = match line_ang_typed {
                            Some(a) => a.to_radians(),
                            None => cursor_ang,
                        };
                        // Typed (positive) length wins; else use the
                        // cursor distance so an angle-only entry still
                        // commits a sensibly-sized segment.
                        let len = match line_len_typed {
                            Some(l) if l > 0.0 => l,
                            _ => cursor_len,
                        };
                        if len > 1e-9 {
                            (fx + len * ang_rad.cos(), fy + len * ang_rad.sin(), true)
                        } else {
                            // Neither a typed length nor a usable cursor
                            // distance — fall back to the raw click.
                            (x_mm, y_mm, false)
                        }
                    } else {
                        (x_mm, y_mm, false)
                    }
                }
                // Circle second click — radius from centre, along
                // the cursor azimuth.
                (
                    Some(PlacementInputKind::CircleRadius),
                    Some(r),
                    SketchTool::Circle,
                    ToolPending::CircleCenter { center },
                ) if r > 0.0 => {
                    let primitive = editor.primitive();
                    if let Some((cx, cy)) = resolve_point_xy(center, primitive) {
                        let dx = x_mm - cx;
                        let dy = y_mm - cy;
                        let cursor_len = (dx * dx + dy * dy).sqrt();
                        if cursor_len > 1e-9 {
                            let ux = dx / cursor_len;
                            let uy = dy / cursor_len;
                            (cx + r * ux, cy + r * uy, true)
                        } else {
                            // Cursor at centre → fall back; the user
                            // can re-position before clicking.
                            (x_mm, y_mm, false)
                        }
                    } else {
                        (x_mm, y_mm, false)
                    }
                }
                // Arc second click — start endpoint at exact radius
                // from centre, along cursor azimuth.
                (
                    Some(PlacementInputKind::ArcRadius),
                    Some(r),
                    SketchTool::Arc,
                    ToolPending::ArcCenter { center },
                ) if r > 0.0 => {
                    let primitive = editor.primitive();
                    if let Some((cx, cy)) = resolve_point_xy(center, primitive) {
                        let dx = x_mm - cx;
                        let dy = y_mm - cy;
                        let cursor_len = (dx * dx + dy * dy).sqrt();
                        if cursor_len > 1e-9 {
                            let ux = dx / cursor_len;
                            let uy = dy / cursor_len;
                            (cx + r * ux, cy + r * uy, true)
                        } else {
                            (x_mm, y_mm, false)
                        }
                    } else {
                        (x_mm, y_mm, false)
                    }
                }
                // Arc third click — sweep from `start` by typed
                // degrees CCW around `center`. Radius is the
                // committed |centre, start| distance.
                (
                    Some(PlacementInputKind::ArcSweep),
                    Some(deg),
                    SketchTool::Arc,
                    ToolPending::ArcStart { center, start },
                ) => {
                    let primitive = editor.primitive();
                    let parts = (
                        resolve_point_xy(center, primitive),
                        resolve_point_xy(start, primitive),
                    );
                    if let (Some((cx, cy)), Some((sx, sy))) = parts {
                        let r = ((sx - cx).powi(2) + (sy - cy).powi(2)).sqrt();
                        if r > 1e-9 {
                            let start_ang = (sy - cy).atan2(sx - cx);
                            let end_ang = start_ang + deg.to_radians();
                            (
                                cx + r * end_ang.cos(),
                                cy + r * end_ang.sin(),
                                true,
                            )
                        } else {
                            (x_mm, y_mm, false)
                        }
                    } else {
                        (x_mm, y_mm, false)
                    }
                }
                // Rectangle / Rounded-Rectangle second click — pin the
                // opposite corner from typed width/height. Each axis is
                // independent: typed width fixes |Δx| (sign from the
                // cursor's quadrant), typed height fixes |Δy|; an
                // untyped axis follows the cursor. The per-tool commit
                // arm builds the box from `first` + this corner (and,
                // for Rounded-Rect, reads the corner radius itself).
                (_, _, SketchTool::Rectangle, ToolPending::RectangleFirst { first })
                | (
                    _,
                    _,
                    SketchTool::RoundedRectangle,
                    ToolPending::RoundedRectangleFirst { first },
                ) if rect_w_typed.is_some() || rect_h_typed.is_some() => {
                    let primitive = editor.primitive();
                    if let Some((fx, fy)) = resolve_point_xy(first, primitive) {
                        // Sign of the cursor offset picks the quadrant
                        // the box grows into; default +1 when the cursor
                        // sits exactly on a corner axis.
                        let sx = if x_mm < fx { -1.0 } else { 1.0 };
                        let sy = if y_mm < fy { -1.0 } else { 1.0 };
                        let ex = match rect_w_typed {
                            Some(w) if w > 0.0 => fx + sx * w,
                            _ => x_mm,
                        };
                        let ey = match rect_h_typed {
                            Some(h) if h > 0.0 => fy + sy * h,
                            _ => y_mm,
                        };
                        (ex, ey, true)
                    } else {
                        (x_mm, y_mm, false)
                    }
                }
                _ => (x_mm, y_mm, false),
            };
            // When numeric input pinned the click, ignore the snap
            // hit (the user explicitly asked for a different
            // distance / angle).
            let effective_snap_id = if used_placement_input {
                None
            } else {
                snap_id
            };

            let resolved_id: SketchEntityId = match effective_snap_id {
                Some(target) if matches!(editor.state.active_tool, SketchTool::Point) => {
                    use signex_sketch::constraint::{Constraint, ConstraintKind};
                    use signex_sketch::id::ConstraintId;

                    let new_id = SketchEntityId::new();
                    let entity = flag(Entity::new(
                        new_id,
                        plane_id,
                        EntityKind::Point {
                            x: eff_x_mm,
                            y: eff_y_mm,
                        },
                    ));
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::AddEntity(entity),
                        );
                    });
                    let constraint = Constraint {
                        id: ConstraintId::new(),
                        kind: ConstraintKind::Coincident {
                            p1: new_id,
                            p2: target,
                        },
                    };
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::AddConstraint(constraint),
                        );
                    });
                    new_id
                }
                Some(id) => id,
                None => {
                    let id = SketchEntityId::new();
                    let entity = flag(Entity::new(
                        id,
                        plane_id,
                        EntityKind::Point {
                            x: eff_x_mm,
                            y: eff_y_mm,
                        },
                    ));
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::AddEntity(entity),
                        );
                    });
                    id
                }
            };

            // v0.23 — RepickPolarCenter intercept. Triggered by the
            // Pattern sub-form's "Re-pick centre" button. The next
            // click on a Point overwrites the array's `center`,
            // independent of the active tool. `resolved_id` is either
            // an existing Point (when snap hit) or a freshly-minted
            // Point at the click location. Skip the tool match below
            // by handling cleanup inline.
            let mut consumed_by_repick = false;
            if let ToolPending::RepickPolarCenter { array_id } = editor.state.tool_pending {
                if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                    if let Some(array) = sketch.arrays.iter_mut().find(|a| a.id == array_id) {
                        if let signex_sketch::array::ArrayKind::Polar { center, .. } =
                            &mut array.kind
                        {
                            *center = resolved_id;
                        }
                    }
                }
                editor.with_parts(|state, primitive| {
                    apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
                });
                editor.state.tool_pending = ToolPending::Idle;
                consumed_by_repick = true;
            }

            if consumed_by_repick {
                editor.canvas_cache.clear();
                editor.dirty = true;
                return;
            }

            // Per-tool state machine — advance `tool_pending` and emit
            // the gesture-completing AddEntity when ready.
            match editor.state.active_tool {
                SketchTool::Select | SketchTool::Point => {
                    // Select: ignore (no add). Point: already added above.
                    editor.state.tool_pending = ToolPending::Idle;
                }
                SketchTool::Line => match editor.state.tool_pending {
                    ToolPending::Idle => {
                        editor.state.tool_pending = ToolPending::LineFirst { first: resolved_id };
                    }
                    ToolPending::LineFirst { first } => {
                        let line_id = SketchEntityId::new();
                        let line = flag(Entity::new(
                            line_id,
                            plane_id,
                            EntityKind::Line {
                                start: first,
                                end: resolved_id,
                            },
                        ));
                        editor.with_parts(|state, primitive| {
                            apply_sketch_edit_with_warnings(
                                state,
                                primitive,
                                SketchEdit::AddEntity(line),
                            );
                        });

                        // v0.22 Phase A2 — Auto-Horizontal/Vertical
                        // inference. If the line's slope is within ±5°
                        // of a cardinal axis, add the matching
                        // constraint so the alignment survives a drag.
                        // The cursor-snap engine already pulls the
                        // click onto the axis when within tolerance,
                        // so this just promotes the implicit alignment
                        // to an explicit constraint visible in the
                        // constraint list.
                        {
                            use signex_sketch::constraint::{Constraint, ConstraintKind};
                            use signex_sketch::id::ConstraintId;
                            const AXIS_THRESHOLD_DEG: f64 = 5.0;
                            let pos_of = |id: SketchEntityId| -> Option<(f64, f64)> {
                                editor
                                    .primitive()
                                    .sketch
                                    .as_ref()
                                    .and_then(|s| s.entities.iter().find(|e| e.id == id))
                                    .and_then(|e| match e.kind {
                                        EntityKind::Point { x, y } => Some((x, y)),
                                        _ => None,
                                    })
                            };
                            if let (Some((x0, y0)), Some((x1, y1))) =
                                (pos_of(first), pos_of(resolved_id))
                            {
                                let dx = x1 - x0;
                                let dy = y1 - y0;
                                let len_sq = dx * dx + dy * dy;
                                if len_sq > 1e-12 {
                                    let len = len_sq.sqrt();
                                    let sin_abs = (dy / len).abs();
                                    let cos_abs = (dx / len).abs();
                                    let thresh = AXIS_THRESHOLD_DEG.to_radians().sin();
                                    let kind = if sin_abs < thresh {
                                        Some(ConstraintKind::Horizontal { line: line_id })
                                    } else if cos_abs < thresh {
                                        Some(ConstraintKind::Vertical { line: line_id })
                                    } else {
                                        None
                                    };
                                    if let Some(k) = kind {
                                        let constraint = Constraint {
                                            id: ConstraintId::new(),
                                            kind: k,
                                        };
                                        editor.with_parts(|state, primitive| {
                                            apply_sketch_edit_with_warnings(
                                                state,
                                                primitive,
                                                SketchEdit::AddConstraint(constraint),
                                            );
                                        });
                                    }
                                }
                            }
                        }

                        // v0.16.1 — chain: keep the Line tool active
                        // and use this click's endpoint as the next
                        // segment's anchor. Esc / right-click cancel
                        // back to Select. Matches Fusion 2D sketch.
                        editor.state.tool_pending = ToolPending::LineFirst { first: resolved_id };
                    }
                    _ => {
                        editor.state.tool_pending = ToolPending::LineFirst { first: resolved_id };
                    }
                },
                SketchTool::Circle => match editor.state.tool_pending {
                    ToolPending::Idle => {
                        editor.state.tool_pending = ToolPending::CircleCenter {
                            center: resolved_id,
                        };
                    }
                    ToolPending::CircleCenter { center } => {
                        // Compute radius from centre + edge points.
                        let r = if let (Some(c_pt), Some(e_pt)) = (
                            editor
                                .primitive()
                                .sketch
                                .as_ref()
                                .and_then(|s| s.entities.iter().find(|e| e.id == center))
                                .and_then(|e| match e.kind {
                                    EntityKind::Point { x, y } => Some((x, y)),
                                    _ => None,
                                }),
                            editor
                                .primitive()
                                .sketch
                                .as_ref()
                                .and_then(|s| s.entities.iter().find(|e| e.id == resolved_id))
                                .and_then(|e| match e.kind {
                                    EntityKind::Point { x, y } => Some((x, y)),
                                    _ => None,
                                }),
                        ) {
                            ((e_pt.0 - c_pt.0).powi(2) + (e_pt.1 - c_pt.1).powi(2)).sqrt()
                        } else {
                            1.0
                        };
                        let circle_id = SketchEntityId::new();
                        let circle = flag(Entity::new(
                            circle_id,
                            plane_id,
                            EntityKind::Circle { center, radius: r },
                        ));
                        editor.with_parts(|state, primitive| {
                            apply_sketch_edit_with_warnings(
                                state,
                                primitive,
                                SketchEdit::AddEntity(circle),
                            );
                        });
                        editor.state.tool_pending = ToolPending::Idle;
                    }
                    _ => {
                        editor.state.tool_pending = ToolPending::CircleCenter {
                            center: resolved_id,
                        };
                    }
                },
                SketchTool::RoundedRectangle => match editor.state.tool_pending {
                    ToolPending::Idle => {
                        editor.state.tool_pending =
                            ToolPending::RoundedRectangleFirst { first: resolved_id };
                    }
                    ToolPending::RoundedRectangleFirst { first } => {
                        // v0.16 — commit the rounded rectangle. Read
                        // first/opposite corner positions, derive the
                        // axis-aligned bbox, clamp the corner radius,
                        // and emit 4 arc-centre Points + 8 arc-end /
                        // line-end Points + 4 Lines (axis-aligned,
                        // shortened by the radius) + 4 Arcs (one per
                        // corner, sweep CCW around the centre).
                        let first_pos = editor
                            .primitive()
                            .sketch
                            .as_ref()
                            .and_then(|s| s.entities.iter().find(|e| e.id == first))
                            .and_then(|e| match e.kind {
                                EntityKind::Point { x, y } => Some((x, y)),
                                _ => None,
                            });
                        let opposite_pos = editor
                            .primitive()
                            .sketch
                            .as_ref()
                            .and_then(|s| s.entities.iter().find(|e| e.id == resolved_id))
                            .and_then(|e| match e.kind {
                                EntityKind::Point { x, y } => Some((x, y)),
                                _ => None,
                            });
                        if let (Some((fx, fy)), Some((ox, oy))) = (first_pos, opposite_pos) {
                            let x0 = fx.min(ox);
                            let y0 = fy.min(oy);
                            let x1 = fx.max(ox);
                            let y1 = fy.max(oy);
                            let half_w = (x1 - x0) / 2.0;
                            let half_h = (y1 - y0) / 2.0;
                            // v0.14-footprint — corner radius source:
                            // prefer a typed RRectRadius (the third Tab
                            // field), then the legacy `dimension_input`
                            // text, else 0.5 mm. Clamp to [0.05, half_min].
                            let r_input = std::iter::once(editor.state.placement_input.as_ref())
                                .chain(editor.state.placement_input_others.iter().map(Some))
                                .flatten()
                                .find(|p| p.kind == PlacementInputKind::RRectRadius)
                                .and_then(|p| p.buffer.parse::<f64>().ok())
                                .or_else(|| editor.state.dimension_input.trim().parse::<f64>().ok())
                                .unwrap_or(0.5);
                            let r_max = half_w.min(half_h).max(0.05);
                            let r = r_input.clamp(0.05, r_max);

                            let tl_c = SketchEntityId::new();
                            let tr_c = SketchEntityId::new();
                            let br_c = SketchEntityId::new();
                            let bl_c = SketchEntityId::new();
                            let tl_right = SketchEntityId::new();
                            let tr_left = SketchEntityId::new();
                            let tr_top = SketchEntityId::new();
                            let br_top = SketchEntityId::new();
                            let br_right = SketchEntityId::new();
                            let bl_left = SketchEntityId::new();
                            let bl_bot = SketchEntityId::new();
                            let tl_bot = SketchEntityId::new();

                            for (id, x, y) in [
                                (tl_c, x0 + r, y0 + r),
                                (tr_c, x1 - r, y0 + r),
                                (br_c, x1 - r, y1 - r),
                                (bl_c, x0 + r, y1 - r),
                                (tl_right, x0 + r, y0),
                                (tr_left, x1 - r, y0),
                                (tr_top, x1, y0 + r),
                                (br_top, x1, y1 - r),
                                (br_right, x1 - r, y1),
                                (bl_left, x0 + r, y1),
                                (bl_bot, x0, y1 - r),
                                (tl_bot, x0, y0 + r),
                            ] {
                                editor.with_parts(|state, primitive| {
                                    apply_sketch_edit_with_warnings(
                                        state,
                                        primitive,
                                        SketchEdit::AddEntity(flag(Entity::new(
                                            id,
                                            plane_id,
                                            EntityKind::Point { x, y },
                                        ))),
                                    );
                                });
                            }
                            // Lines: top, right, bottom, left.
                            for (s, e) in [
                                (tl_right, tr_left),
                                (tr_top, br_top),
                                (br_right, bl_left),
                                (bl_bot, tl_bot),
                            ] {
                                let line_id = SketchEntityId::new();
                                editor.with_parts(|state, primitive| {
                                    apply_sketch_edit_with_warnings(
                                        state,
                                        primitive,
                                        SketchEdit::AddEntity(flag(Entity::new(
                                            line_id,
                                            plane_id,
                                            EntityKind::Line { start: s, end: e },
                                        ))),
                                    );
                                });
                            }
                            // Arcs: TR, BR, BL, TL — sweep CCW around
                            // each centre so each subtends 90°.
                            for (center, start, end) in [
                                (tr_c, tr_left, tr_top),
                                (br_c, br_top, br_right),
                                (bl_c, bl_left, bl_bot),
                                (tl_c, tl_bot, tl_right),
                            ] {
                                let arc_id = SketchEntityId::new();
                                editor.with_parts(|state, primitive| {
                                    apply_sketch_edit_with_warnings(
                                        state,
                                        primitive,
                                        SketchEdit::AddEntity(flag(Entity::new(
                                            arc_id,
                                            plane_id,
                                            EntityKind::Arc {
                                                center,
                                                start,
                                                end,
                                                sweep_ccw: true,
                                            },
                                        ))),
                                    );
                                });
                            }
                        }
                        editor.state.tool_pending = ToolPending::Idle;
                    }
                    _ => {
                        editor.state.tool_pending =
                            ToolPending::RoundedRectangleFirst { first: resolved_id };
                    }
                },
                SketchTool::Rectangle => match editor.state.tool_pending {
                    ToolPending::Idle => {
                        editor.state.tool_pending =
                            ToolPending::RectangleFirst { first: resolved_id };
                    }
                    ToolPending::RectangleFirst { first } => {
                        // v0.15 — commit the rectangle. Resolve the
                        // first corner's world position from the
                        // sketch, then mint 2 new Points (the two
                        // mid-axis corners) and 4 Lines connecting
                        // (first, midA, opposite, midB) into a
                        // closed loop. resolved_id is the opposite
                        // corner the user just clicked.
                        let first_pos = editor
                            .primitive()
                            .sketch
                            .as_ref()
                            .and_then(|s| s.entities.iter().find(|e| e.id == first))
                            .and_then(|e| match e.kind {
                                EntityKind::Point { x, y } => Some((x, y)),
                                _ => None,
                            });
                        let opposite_pos = editor
                            .primitive()
                            .sketch
                            .as_ref()
                            .and_then(|s| s.entities.iter().find(|e| e.id == resolved_id))
                            .and_then(|e| match e.kind {
                                EntityKind::Point { x, y } => Some((x, y)),
                                _ => None,
                            });
                        if let (Some((x0, y0)), Some((x1, y1))) = (first_pos, opposite_pos) {
                            // Mint the two mid-axis corners.
                            let mid_a_id = SketchEntityId::new();
                            let mid_b_id = SketchEntityId::new();
                            let mid_a = flag(Entity::new(
                                mid_a_id,
                                plane_id,
                                EntityKind::Point { x: x1, y: y0 },
                            ));
                            let mid_b = flag(Entity::new(
                                mid_b_id,
                                plane_id,
                                EntityKind::Point { x: x0, y: y1 },
                            ));
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(mid_a),
                                );
                            });
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(mid_b),
                                );
                            });
                            // Now the 4 lines: first → mid_a →
                            // opposite → mid_b → first.
                            for (s, e) in [
                                (first, mid_a_id),
                                (mid_a_id, resolved_id),
                                (resolved_id, mid_b_id),
                                (mid_b_id, first),
                            ] {
                                let line_id = SketchEntityId::new();
                                let line = flag(Entity::new(
                                    line_id,
                                    plane_id,
                                    EntityKind::Line { start: s, end: e },
                                ));
                                editor.with_parts(|state, primitive| {
                                    apply_sketch_edit_with_warnings(
                                        state,
                                        primitive,
                                        SketchEdit::AddEntity(line),
                                    );
                                });
                            }
                        }
                        editor.state.tool_pending = ToolPending::Idle;
                    }
                    _ => {
                        editor.state.tool_pending =
                            ToolPending::RectangleFirst { first: resolved_id };
                    }
                },
                SketchTool::Arc => match editor.state.tool_pending {
                    ToolPending::Idle => {
                        editor.state.tool_pending = ToolPending::ArcCenter {
                            center: resolved_id,
                        };
                    }
                    ToolPending::ArcCenter { center } => {
                        editor.state.tool_pending = ToolPending::ArcStart {
                            center,
                            start: resolved_id,
                        };
                    }
                    ToolPending::ArcStart { center, start } => {
                        let arc_id = SketchEntityId::new();
                        let arc = flag(Entity::new(
                            arc_id,
                            plane_id,
                            EntityKind::Arc {
                                center,
                                start,
                                end: resolved_id,
                                sweep_ccw: true,
                            },
                        ));
                        editor.with_parts(|state, primitive| {
                            apply_sketch_edit_with_warnings(
                                state,
                                primitive,
                                SketchEdit::AddEntity(arc),
                            );
                        });
                        editor.state.tool_pending = ToolPending::Idle;
                    }
                    _ => {
                        editor.state.tool_pending = ToolPending::ArcCenter {
                            center: resolved_id,
                        };
                    }
                },
                SketchTool::Mirror => {
                    // v0.22 Phase B1 + extension — Mirror tool.
                    // Pre-condition: a Line entity must already be
                    // selected via the Select tool; clicks while no
                    // Line is selected are silent no-ops with a
                    // warning surfaced via `solve_warnings`.
                    //
                    // The picked entity's geometry is reflected
                    // across the selected Line and a fresh entity is
                    // minted referencing mirrored copies of every
                    // Point it touches. Each mirrored Point pair
                    // gets a `SymmetricAboutLine` constraint so the
                    // solver maintains symmetry through subsequent
                    // edits (drag the source and the mirror tracks
                    // it parametrically).
                    //
                    // Scope: Points / Lines / Arcs / Circles.
                    // Mirrored Arcs flip `sweep_ccw` because
                    // reflection inverts winding. Mirrored Circles
                    // re-use the source radius (Circle's `radius` is
                    // a literal, not a referenced Point, so it
                    // round-trips unchanged).
                    use signex_sketch::constraint::{Constraint, ConstraintKind};
                    use signex_sketch::id::ConstraintId;

                    let line_id = match editor.state.selected_sketch {
                        Some(id) => id,
                        None => {
                            editor.state.solve_warnings.push(
                                "Mirror: select a Line first (Select tool, click a Line, then click here to mirror)"
                                    .into(),
                            );
                            editor.state.tool_pending = ToolPending::Idle;
                            editor.canvas_cache.clear();
                            return;
                        }
                    };

                    let sketch_ref = match editor.primitive().sketch.as_ref() {
                        Some(s) => s,
                        None => {
                            editor.state.tool_pending = ToolPending::Idle;
                            return;
                        }
                    };
                    let line_endpoints = sketch_ref
                        .entities
                        .iter()
                        .find(|e| e.id == line_id)
                        .and_then(|e| match e.kind {
                            EntityKind::Line { start, end } => Some((start, end)),
                            _ => None,
                        });
                    let (a_id, b_id) = match line_endpoints {
                        Some(p) => p,
                        None => {
                            editor.state.solve_warnings.push(
                                "Mirror: selection is not a Line — pick a Line entity first"
                                    .into(),
                            );
                            editor.state.tool_pending = ToolPending::Idle;
                            editor.canvas_cache.clear();
                            return;
                        }
                    };

                    let pos_of = |id: SketchEntityId| -> Option<(f64, f64)> {
                        sketch_ref
                            .entities
                            .iter()
                            .find(|e| e.id == id)
                            .and_then(|e| match e.kind {
                                EntityKind::Point { x, y } => Some((x, y)),
                                _ => None,
                            })
                    };
                    let kind_of = sketch_ref
                        .entities
                        .iter()
                        .find(|e| e.id == resolved_id)
                        .map(|e| e.kind.clone());
                    let kind_of = match kind_of {
                        Some(k) => k,
                        None => {
                            editor.state.tool_pending = ToolPending::Idle;
                            return;
                        }
                    };

                    let (ax, ay) = match pos_of(a_id) {
                        Some(p) => p,
                        None => return,
                    };
                    let (bx, by) = match pos_of(b_id) {
                        Some(p) => p,
                        None => return,
                    };
                    let vx = bx - ax;
                    let vy = by - ay;
                    let v_dot_v = vx * vx + vy * vy;
                    if v_dot_v <= 1e-12 {
                        editor.state.solve_warnings.push(
                            "Mirror: degenerate Line (zero length)".into(),
                        );
                        editor.state.tool_pending = ToolPending::Idle;
                        editor.canvas_cache.clear();
                        return;
                    }
                    let reflect = |px: f64, py: f64| -> (f64, f64) {
                        let t = ((px - ax) * vx + (py - ay) * vy) / v_dot_v;
                        let foot_x = ax + t * vx;
                        let foot_y = ay + t * vy;
                        (2.0 * foot_x - px, 2.0 * foot_y - py)
                    };

                    // Mirror a Point entity by ID: emits a new Point
                    // at the reflected position and a
                    // SymmetricAboutLine constraint linking source
                    // and mirror. Returns the new Point's ID.
                    // Captured by reference so the closure can be
                    // called repeatedly for chained-Point entities.
                    let mut mint_mirror_point =
                        |editor: &mut crate::app::FootprintEditorState,
                         pt_id: SketchEntityId,
                         pos: (f64, f64)|
                         -> SketchEntityId {
                            let (rx, ry) = reflect(pos.0, pos.1);
                            let new_id = SketchEntityId::new();
                            let new_entity = flag(Entity::new(
                                new_id,
                                plane_id,
                                EntityKind::Point { x: rx, y: ry },
                            ));
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(new_entity),
                                );
                            });
                            let constraint = Constraint {
                                id: ConstraintId::new(),
                                kind: ConstraintKind::SymmetricAboutLine {
                                    p1: pt_id,
                                    p2: new_id,
                                    line: line_id,
                                },
                            };
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddConstraint(constraint),
                                );
                            });
                            new_id
                        };

                    match kind_of {
                        EntityKind::Point { x, y } => {
                            mint_mirror_point(editor, resolved_id, (x, y));
                        }
                        EntityKind::Line { start, end } => {
                            let s_pos = match pos_of(start) {
                                Some(p) => p,
                                None => return,
                            };
                            let e_pos = match pos_of(end) {
                                Some(p) => p,
                                None => return,
                            };
                            let new_start = mint_mirror_point(editor, start, s_pos);
                            let new_end = mint_mirror_point(editor, end, e_pos);
                            let new_line_id = SketchEntityId::new();
                            let new_line = flag(Entity::new(
                                new_line_id,
                                plane_id,
                                EntityKind::Line {
                                    start: new_start,
                                    end: new_end,
                                },
                            ));
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(new_line),
                                );
                            });
                        }
                        EntityKind::Arc {
                            center,
                            start,
                            end,
                            sweep_ccw,
                        } => {
                            let c_pos = match pos_of(center) {
                                Some(p) => p,
                                None => return,
                            };
                            let s_pos = match pos_of(start) {
                                Some(p) => p,
                                None => return,
                            };
                            let e_pos = match pos_of(end) {
                                Some(p) => p,
                                None => return,
                            };
                            let new_center = mint_mirror_point(editor, center, c_pos);
                            let new_start = mint_mirror_point(editor, start, s_pos);
                            let new_end = mint_mirror_point(editor, end, e_pos);
                            // Reflection inverts winding — flip
                            // sweep_ccw so the mirrored arc traces
                            // the same arc on the other side.
                            let new_arc_id = SketchEntityId::new();
                            let new_arc = flag(Entity::new(
                                new_arc_id,
                                plane_id,
                                EntityKind::Arc {
                                    center: new_center,
                                    start: new_start,
                                    end: new_end,
                                    sweep_ccw: !sweep_ccw,
                                },
                            ));
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(new_arc),
                                );
                            });
                        }
                        EntityKind::Circle { center, radius } => {
                            let c_pos = match pos_of(center) {
                                Some(p) => p,
                                None => return,
                            };
                            let new_center = mint_mirror_point(editor, center, c_pos);
                            let new_circle_id = SketchEntityId::new();
                            let new_circle = flag(Entity::new(
                                new_circle_id,
                                plane_id,
                                EntityKind::Circle {
                                    center: new_center,
                                    radius,
                                },
                            ));
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(new_circle),
                                );
                            });
                        }
                    }
                    editor.state.tool_pending = ToolPending::Idle;
                }
                SketchTool::Offset => {
                    // v0.22 Phase B2 — Offset tool. Pre-condition: a
                    // Line / Arc / Circle is in `selected_sketch`. The
                    // click position determines which side of the
                    // source curve the offset lands on. Offset
                    // distance comes from `state.dimension_input`,
                    // default 0.5 mm.
                    //
                    // Lines: emits a parallel Line at perpendicular
                    // distance and adds (Parallel + DistancePtLine)
                    // constraints so the relationship survives source
                    // edits.
                    //
                    // Circles / Arcs: emits a concentric copy that
                    // shares the source's centre Point so the centres
                    // stay locked. The new radius is a literal
                    // (source.radius ± dist) — the schema has no
                    // radius-dimension constraint, so further radius
                    // edits don't auto-propagate; the user can
                    // re-offset or edit the literal directly.
                    use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
                    use signex_sketch::id::ConstraintId;

                    let source_id = match editor.state.selected_sketch {
                        Some(id) => id,
                        None => {
                            editor.state.solve_warnings.push(
                                "Offset: select a Line / Arc / Circle first (Select tool, click the curve, then click on the side to offset)"
                                    .into(),
                            );
                            editor.state.tool_pending = ToolPending::Idle;
                            editor.canvas_cache.clear();
                            return;
                        }
                    };
                    // v0.25 polish — prefer placement_input over the
                    // legacy `dimension_input` text field. The
                    // keypress-driven cursor overlay is the
                    // discoverable path; `dimension_input` stays as
                    // the Properties-panel fallback for users who
                    // already have a value there.
                    let dist_from_placement = editor
                        .state
                        .placement_input
                        .as_ref()
                        .filter(|p| p.kind == PlacementInputKind::OffsetDistance)
                        .and_then(|p| p.buffer.parse::<f64>().ok())
                        .filter(|d| d.is_finite() && *d > 1e-9);
                    let dist = dist_from_placement.unwrap_or_else(|| {
                        editor
                            .state
                            .dimension_input
                            .trim()
                            .parse::<f64>()
                            .ok()
                            .filter(|d| d.is_finite() && *d > 1e-9)
                            .unwrap_or(0.5)
                    });
                    // Clear the buffer so the next Offset click
                    // doesn''t accidentally reuse the old value.
                    if dist_from_placement.is_some() {
                        editor.state.placement_input = None;
                    }

                    let sketch_ref = match editor.primitive().sketch.as_ref() {
                        Some(s) => s,
                        None => {
                            editor.state.tool_pending = ToolPending::Idle;
                            return;
                        }
                    };
                    let pos_of = |id: SketchEntityId| -> Option<(f64, f64)> {
                        sketch_ref
                            .entities
                            .iter()
                            .find(|e| e.id == id)
                            .and_then(|e| match e.kind {
                                EntityKind::Point { x, y } => Some((x, y)),
                                _ => None,
                            })
                    };
                    let source_kind = sketch_ref
                        .entities
                        .iter()
                        .find(|e| e.id == source_id)
                        .map(|e| e.kind.clone());
                    let source_kind = match source_kind {
                        Some(k) => k,
                        None => {
                            editor.state.solve_warnings.push(
                                "Offset: selection no longer exists in the sketch".into(),
                            );
                            editor.state.tool_pending = ToolPending::Idle;
                            editor.canvas_cache.clear();
                            return;
                        }
                    };

                    match source_kind {
                        EntityKind::Line { start, end } => {
                            let (ax, ay) = match pos_of(start) {
                                Some(p) => p,
                                None => return,
                            };
                            let (bx, by) = match pos_of(end) {
                                Some(p) => p,
                                None => return,
                            };
                            let dx = bx - ax;
                            let dy = by - ay;
                            let len = (dx * dx + dy * dy).sqrt();
                            if len < 1e-9 {
                                editor.state.solve_warnings.push(
                                    "Offset: degenerate Line (zero length)".into(),
                                );
                                editor.state.tool_pending = ToolPending::Idle;
                                editor.canvas_cache.clear();
                                return;
                            }
                            // Perpendicular unit vector. Sign from the
                            // cross of (line direction) × (click −
                            // line start): positive = click is on the
                            // (-dy, dx) side, negative = (dy, -dx)
                            // side.
                            let cx = x_mm - ax;
                            let cy = y_mm - ay;
                            let cross = dx * cy - dy * cx;
                            let sign = if cross >= 0.0 { 1.0 } else { -1.0 };
                            let nx = -dy / len * sign;
                            let ny = dx / len * sign;
                            let off_x = nx * dist;
                            let off_y = ny * dist;

                            let new_a = SketchEntityId::new();
                            let new_b = SketchEntityId::new();
                            let new_line_id = SketchEntityId::new();
                            let a_entity = flag(Entity::new(
                                new_a,
                                plane_id,
                                EntityKind::Point {
                                    x: ax + off_x,
                                    y: ay + off_y,
                                },
                            ));
                            let b_entity = flag(Entity::new(
                                new_b,
                                plane_id,
                                EntityKind::Point {
                                    x: bx + off_x,
                                    y: by + off_y,
                                },
                            ));
                            let new_line = flag(Entity::new(
                                new_line_id,
                                plane_id,
                                EntityKind::Line {
                                    start: new_a,
                                    end: new_b,
                                },
                            ));
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(a_entity),
                                );
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(b_entity),
                                );
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(new_line),
                                );
                            });
                            // Parallel + DistancePtLine on the start
                            // endpoint pins the offset distance
                            // parametrically. The end endpoint is left
                            // free along the offset line direction —
                            // the user can drag it without breaking
                            // the offset relationship.
                            let parallel = Constraint {
                                id: ConstraintId::new(),
                                kind: ConstraintKind::Parallel {
                                    l1: source_id,
                                    l2: new_line_id,
                                },
                            };
                            let dist_constraint = Constraint {
                                id: ConstraintId::new(),
                                kind: ConstraintKind::DistancePtLine {
                                    point: new_a,
                                    line: source_id,
                                    target: DimTarget::Literal(dist),
                                },
                            };
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddConstraint(parallel),
                                );
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddConstraint(dist_constraint),
                                );
                            });
                        }
                        EntityKind::Circle { center, radius } => {
                            let (cx, cy) = match pos_of(center) {
                                Some(p) => p,
                                None => return,
                            };
                            // Click distance from centre — inside the
                            // circle = shrink (-dist), outside =
                            // expand (+dist). Clamp to a positive
                            // radius so we don't mint a degenerate
                            // shape.
                            let click_r = ((x_mm - cx).powi(2) + (y_mm - cy).powi(2)).sqrt();
                            let signed = if click_r < radius { -dist } else { dist };
                            let new_radius = (radius + signed).max(1e-6);
                            let new_circle_id = SketchEntityId::new();
                            let new_circle = flag(Entity::new(
                                new_circle_id,
                                plane_id,
                                EntityKind::Circle {
                                    center,
                                    radius: new_radius,
                                },
                            ));
                            // v0.23 — parametric link: mint an anchor
                            // Point on the new circle and pin its
                            // distance to the source circle to
                            // `signed`. Combined with a DistancePtCircle
                            // on the new circle (target=0), this
                            // forces `new_radius = source_radius +
                            // signed` through the solver — so when
                            // the user edits the target via the
                            // Properties panel later, the new
                            // circle's radius follows.
                            let scale = if click_r > 1e-9 {
                                new_radius / click_r
                            } else {
                                1.0
                            };
                            let anchor_id = SketchEntityId::new();
                            let anchor = flag(Entity::new(
                                anchor_id,
                                plane_id,
                                EntityKind::Point {
                                    x: cx + (x_mm - cx) * scale,
                                    y: cy + (y_mm - cy) * scale,
                                },
                            ));
                            let on_new_circle = Constraint {
                                id: ConstraintId::new(),
                                kind: ConstraintKind::DistancePtCircle {
                                    point: anchor_id,
                                    circle: new_circle_id,
                                    target: DimTarget::Literal(0.0),
                                },
                            };
                            let parametric_offset = Constraint {
                                id: ConstraintId::new(),
                                kind: ConstraintKind::DistancePtCircle {
                                    point: anchor_id,
                                    circle: source_id,
                                    target: DimTarget::Literal(signed),
                                },
                            };
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(anchor),
                                );
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(new_circle),
                                );
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddConstraint(on_new_circle),
                                );
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddConstraint(parametric_offset),
                                );
                            });
                        }
                        EntityKind::Arc {
                            center,
                            start,
                            end,
                            sweep_ccw,
                        } => {
                            let (cx, cy) = match pos_of(center) {
                                Some(p) => p,
                                None => return,
                            };
                            let (sx, sy) = match pos_of(start) {
                                Some(p) => p,
                                None => return,
                            };
                            let (ex, ey) = match pos_of(end) {
                                Some(p) => p,
                                None => return,
                            };
                            // Source radius from start position;
                            // direction from start angle.
                            let source_r =
                                ((sx - cx).powi(2) + (sy - cy).powi(2)).sqrt();
                            let click_r =
                                ((x_mm - cx).powi(2) + (y_mm - cy).powi(2)).sqrt();
                            let signed = if click_r < source_r { -dist } else { dist };
                            let new_r = (source_r + signed).max(1e-6);
                            let scale = new_r / source_r.max(1e-9);

                            let new_start = SketchEntityId::new();
                            let new_end = SketchEntityId::new();
                            let new_arc_id = SketchEntityId::new();
                            let s_entity = flag(Entity::new(
                                new_start,
                                plane_id,
                                EntityKind::Point {
                                    x: cx + (sx - cx) * scale,
                                    y: cy + (sy - cy) * scale,
                                },
                            ));
                            let e_entity = flag(Entity::new(
                                new_end,
                                plane_id,
                                EntityKind::Point {
                                    x: cx + (ex - cx) * scale,
                                    y: cy + (ey - cy) * scale,
                                },
                            ));
                            let new_arc = flag(Entity::new(
                                new_arc_id,
                                plane_id,
                                EntityKind::Arc {
                                    center,
                                    start: new_start,
                                    end: new_end,
                                    sweep_ccw,
                                },
                            ));
                            // v0.23 — parametric link: pin both new
                            // endpoints to be `signed` away from the
                            // source arc's underlying circle. Since
                            // both arcs share the same `center`, this
                            // forces the new arc's radius to track
                            // source_radius + signed through the
                            // solver. End Point's angle is left free
                            // — the user can drag it without breaking
                            // the parametric distance.
                            let dist_start = Constraint {
                                id: ConstraintId::new(),
                                kind: ConstraintKind::DistancePtCircle {
                                    point: new_start,
                                    circle: source_id,
                                    target: DimTarget::Literal(signed),
                                },
                            };
                            let dist_end = Constraint {
                                id: ConstraintId::new(),
                                kind: ConstraintKind::DistancePtCircle {
                                    point: new_end,
                                    circle: source_id,
                                    target: DimTarget::Literal(signed),
                                },
                            };
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(s_entity),
                                );
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(e_entity),
                                );
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(new_arc),
                                );
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddConstraint(dist_start),
                                );
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddConstraint(dist_end),
                                );
                            });
                        }
                        EntityKind::Point { .. } => {
                            editor.state.solve_warnings.push(
                                "Offset: selection is a Point — pick a Line / Arc / Circle"
                                    .into(),
                            );
                        }
                    }
                    editor.state.tool_pending = ToolPending::Idle;
                }
                SketchTool::RectPattern => {
                    // v0.22 Phase B3 — Rectangular Pattern. Click 1
                    // picks the source entity (whatever was clicked,
                    // including a freshly-minted Point if the click
                    // missed everything). Mints a default 2×2 grid
                    // with 5 mm spacing, sequential numbering. User
                    // edits via JSON until a Properties sub-form
                    // lands.
                    use signex_sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
                    let array = Array {
                        id: ArrayId::new(),
                        kind: ArrayKind::Grid {
                            source: resolved_id,
                            nx_expr: "2".into(),
                            ny_expr: "2".into(),
                            dx_expr: "5mm".into(),
                            dy_expr: "5mm".into(),
                            depopulation: None,
                        },
                        numbering: NumberingScheme::default(),
                    };
                    let sketch = editor
                        .primitive_mut()
                        .sketch
                        .get_or_insert_with(signex_sketch::SketchData::default);
                    sketch.arrays.push(array);
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::ForceRebuild,
                        );
                    });
                    editor.state.tool_pending = ToolPending::Idle;
                }
                SketchTool::TangentArc => {
                    // v0.24 Track C — Tangent Arc. Two-click chained
                    // arc segment that mints an Arc tangent to the
                    // most recently committed Line whose end Point
                    // matches the first click. The dispatcher also
                    // emits a `TangentLineArc` constraint so the
                    // tangency survives further edits.
                    //
                    // - Click 1: stash the resolved Point as
                    //   `ToolPending::TangentArcFirst { first }`.
                    //   Mirrors the Line tool's first-click flow.
                    // - Click 2: locate a Line whose `end == first`.
                    //   Compute the tangent centre on the line's
                    //   perpendicular bisector through `first` so
                    //   the arc starts off the line tangentially.
                    //   Mint an Arc entity + TangentLineArc
                    //   constraint and chain back to Idle.
                    //
                    // Fallback: when no incident Line is found, the
                    // dispatcher mints a placeholder centre at the
                    // perpendicular bisector of the chord (no
                    // tangency reference) and publishes a warning
                    // via `solve_warnings`. The Arc still appears in
                    // the sketch so the user can constrain it
                    // manually if desired.
                    use signex_sketch::constraint::{Constraint, ConstraintKind};
                    use signex_sketch::id::ConstraintId;

                    match editor.state.tool_pending {
                        ToolPending::TangentArcFirst { first } => {
                            // Look up the first endpoint position +
                            // any Line ending at `first`.
                            let (
                                first_pos,
                                end_pos,
                                incident_line,
                            ): ((f64, f64), (f64, f64), Option<(SketchEntityId, (f64, f64))>) = {
                                let sketch_ref = match editor.primitive().sketch.as_ref() {
                                    Some(s) => s,
                                    None => {
                                        editor.state.tool_pending = ToolPending::Idle;
                                        return;
                                    }
                                };
                                let pos_of = |id: SketchEntityId| -> Option<(f64, f64)> {
                                    sketch_ref
                                        .entities
                                        .iter()
                                        .find(|e| e.id == id)
                                        .and_then(|e| match e.kind {
                                            EntityKind::Point { x, y } => Some((x, y)),
                                            _ => None,
                                        })
                                };
                                let first_p = match pos_of(first) {
                                    Some(p) => p,
                                    None => {
                                        editor.state.tool_pending = ToolPending::Idle;
                                        return;
                                    }
                                };
                                let end_p = match pos_of(resolved_id) {
                                    Some(p) => p,
                                    None => {
                                        editor.state.tool_pending = ToolPending::Idle;
                                        return;
                                    }
                                };
                                // Find a Line whose end matches `first`.
                                // Prefer the most recently authored one
                                // (last in the list) so chained sketches
                                // pick up the immediately preceding
                                // Line, not an unrelated old one.
                                let line = sketch_ref
                                    .entities
                                    .iter()
                                    .rev()
                                    .find_map(|e| match e.kind {
                                        EntityKind::Line { start, end } if end == first => {
                                            pos_of(start).map(|p| (e.id, p))
                                        }
                                        EntityKind::Line { start, end } if start == first => {
                                            pos_of(end).map(|p| (e.id, p))
                                        }
                                        _ => None,
                                    });
                                (first_p, end_p, line)
                            };

                            // Compute the tangent centre.
                            //
                            // With an incident Line, the centre lies
                            // on the line's perpendicular through
                            // `first`. We pick the side of the chord
                            // (`first` → `end_pos`) that lets the arc
                            // reach `end` along that perpendicular,
                            // and place the centre on the
                            // perpendicular bisector of the chord
                            // intersected with the line-perpendicular
                            // through `first`. That intersection is
                            // the unique circle that is tangent to
                            // the line at `first` and passes through
                            // `end_pos`.
                            //
                            // Without an incident Line, fall back to
                            // the chord's perpendicular bisector
                            // midpoint shifted by half-chord —
                            // produces a 90° arc as a sane default.
                            let (cx, cy) = match incident_line {
                                Some((_, line_other_pos)) => {
                                    // Line direction (line_other -> first)
                                    let lx = first_pos.0 - line_other_pos.0;
                                    let ly = first_pos.1 - line_other_pos.1;
                                    let llen_sq = lx * lx + ly * ly;
                                    if llen_sq <= 1e-12 {
                                        // Degenerate; treat as no line.
                                        let mx = (first_pos.0 + end_pos.0) * 0.5;
                                        let my = (first_pos.1 + end_pos.1) * 0.5;
                                        let dx = end_pos.0 - first_pos.0;
                                        let dy = end_pos.1 - first_pos.1;
                                        // Rotate 90° CCW for placeholder.
                                        (mx + (-dy) * 0.5, my + dx * 0.5)
                                    } else {
                                        // Perpendicular to the line at first.
                                        let llen = llen_sq.sqrt();
                                        let nx = -ly / llen;
                                        let ny = lx / llen;
                                        // Centre is on the line through `first`
                                        // along (nx, ny). Solve for the t such
                                        // that |centre - end| = |centre - first|:
                                        //   (first.x + t*nx - end.x)^2
                                        //   + (first.y + t*ny - end.y)^2 = t^2
                                        // Expanding:
                                        //   |first - end|^2
                                        //   + 2*t*((first.x - end.x)*nx + (first.y - end.y)*ny)
                                        //   = 0
                                        // → t = -|first - end|^2 /
                                        //       (2 * ((first - end) · n))
                                        let dx = first_pos.0 - end_pos.0;
                                        let dy = first_pos.1 - end_pos.1;
                                        let denom = 2.0 * (dx * nx + dy * ny);
                                        let chord_sq = dx * dx + dy * dy;
                                        if denom.abs() <= 1e-9 {
                                            // end is on the line — tangent
                                            // circle is undefined (would be
                                            // infinite radius / a straight
                                            // line). Fall back to the chord
                                            // midpoint perpendicular.
                                            let mx = (first_pos.0 + end_pos.0) * 0.5;
                                            let my = (first_pos.1 + end_pos.1) * 0.5;
                                            (mx + nx * 0.5, my + ny * 0.5)
                                        } else {
                                            let t = -chord_sq / denom;
                                            (first_pos.0 + t * nx, first_pos.1 + t * ny)
                                        }
                                    }
                                }
                                None => {
                                    // Placeholder centre — perpendicular
                                    // to the chord at the midpoint, half
                                    // chord length out (gives a 90°
                                    // arc). The user will typically
                                    // re-constrain manually.
                                    editor.state.solve_warnings.push(
                                        "Tangent Arc: no incident line found, placeholder centre"
                                            .into(),
                                    );
                                    let mx = (first_pos.0 + end_pos.0) * 0.5;
                                    let my = (first_pos.1 + end_pos.1) * 0.5;
                                    let dx = end_pos.0 - first_pos.0;
                                    let dy = end_pos.1 - first_pos.1;
                                    // Rotate 90° CCW.
                                    (mx + (-dy) * 0.5, my + dx * 0.5)
                                }
                            };

                            // Mint the centre Point.
                            let centre_id = SketchEntityId::new();
                            let centre = flag(Entity::new(
                                centre_id,
                                plane_id,
                                EntityKind::Point { x: cx, y: cy },
                            ));
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(centre),
                                );
                            });

                            // Mint the Arc entity. Sweep direction
                            // chosen so the arc opens away from the
                            // incident line (when present); without a
                            // line, default CCW.
                            let arc_id = SketchEntityId::new();
                            let sweep_ccw = match incident_line {
                                Some((_, line_other_pos)) => {
                                    // Cross product of (line_other -> first)
                                    // and (first -> end) tells us which
                                    // side of the line `end` is on.
                                    let lx = first_pos.0 - line_other_pos.0;
                                    let ly = first_pos.1 - line_other_pos.1;
                                    let ex = end_pos.0 - first_pos.0;
                                    let ey = end_pos.1 - first_pos.1;
                                    // Cross > 0 → end is to the left of
                                    // the incoming line direction → CCW
                                    // arc opens left.
                                    lx * ey - ly * ex >= 0.0
                                }
                                None => true,
                            };
                            let arc = flag(Entity::new(
                                arc_id,
                                plane_id,
                                EntityKind::Arc {
                                    center: centre_id,
                                    start: first,
                                    end: resolved_id,
                                    sweep_ccw,
                                },
                            ));
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(arc),
                                );
                            });

                            // Add the TangentLineArc constraint when
                            // we have an incident Line.
                            if let Some((line_id, _)) = incident_line {
                                let constraint = Constraint {
                                    id: ConstraintId::new(),
                                    kind: ConstraintKind::TangentLineArc {
                                        line: line_id,
                                        arc: arc_id,
                                    },
                                };
                                editor.with_parts(|state, primitive| {
                                    apply_sketch_edit_with_warnings(
                                        state,
                                        primitive,
                                        SketchEdit::AddConstraint(constraint),
                                    );
                                });
                            }

                            editor.state.tool_pending = ToolPending::Idle;
                        }
                        _ => {
                            // First click — stash the endpoint and
                            // wait for click 2.
                            editor.state.tool_pending =
                                ToolPending::TangentArcFirst { first: resolved_id };
                        }
                    }
                }
                SketchTool::CircularPattern => {
                    // v0.22 Phase B4 — Circular Pattern. Click 1
                    // picks the source entity. The polar array
                    // requires a centre Point — mint a fresh one
                    // 5 mm to the right of the click position so the
                    // array doesn't all stack on the source. Default
                    // count 4, sweep 360°.
                    use signex_sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
                    let centre_id = SketchEntityId::new();
                    let centre = flag(Entity::new(
                        centre_id,
                        plane_id,
                        EntityKind::Point {
                            x: x_mm + 5.0,
                            y: y_mm,
                        },
                    ));
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::AddEntity(centre),
                        );
                    });
                    let array = Array {
                        id: ArrayId::new(),
                        kind: ArrayKind::Polar {
                            source: resolved_id,
                            center: centre_id,
                            count_expr: "4".into(),
                            sweep_angle_expr: "360deg".into(),
                            depopulation: None,
                        },
                        numbering: NumberingScheme::default(),
                    };
                    let sketch = editor
                        .primitive_mut()
                        .sketch
                        .get_or_insert_with(signex_sketch::SketchData::default);
                    sketch.arrays.push(array);
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::ForceRebuild,
                        );
                    });
                    editor.state.tool_pending = ToolPending::Idle;
                }
                SketchTool::Fillet => {
                    // v0.27 — EDA Fillet. Two-click gesture:
                    //   click 1: pick the first Line (we hit-test for
                    //     a Line near the click — fall back to a
                    //     warning if none).
                    //   click 2: pick the second Line that shares an
                    //     endpoint with the first. Compute tangent
                    //     points at radius `r` from the shared corner
                    //     along each line, splice in an Arc connecting
                    //     them centred on the angle bisector, and
                    //     shorten both lines to end at the tangent
                    //     points.
                    //
                    // Radius source — `state.placement_input` (kind
                    // FilletRadius) when the user typed one; else
                    // `state.dimension_input`; else 0.5 mm.
                    fn pick_line_at(
                        sketch: &signex_sketch::SketchData,
                        x: f64,
                        y: f64,
                    ) -> Option<SketchEntityId> {
                        const TOL_MM: f64 = 0.30;
                        let pos_of = |id: SketchEntityId| -> Option<(f64, f64)> {
                            sketch.entities.iter().find(|e| e.id == id).and_then(|e| match e.kind {
                                EntityKind::Point { x, y } => Some((x, y)),
                                _ => None,
                            })
                        };
                        let mut best: Option<(f64, SketchEntityId)> = None;
                        for e in &sketch.entities {
                            if let EntityKind::Line { start, end } = e.kind {
                                let (Some(a), Some(b)) = (pos_of(start), pos_of(end)) else {
                                    continue;
                                };
                                let dx = b.0 - a.0;
                                let dy = b.1 - a.1;
                                let llen2 = dx * dx + dy * dy;
                                if llen2 <= 1e-12 {
                                    continue;
                                }
                                let t = ((x - a.0) * dx + (y - a.1) * dy) / llen2;
                                let tc = t.clamp(0.0, 1.0);
                                let px = a.0 + tc * dx;
                                let py = a.1 + tc * dy;
                                let d2 = (px - x).powi(2) + (py - y).powi(2);
                                if d2 <= TOL_MM * TOL_MM
                                    && best.as_ref().is_none_or(|(b2, _)| d2 < *b2)
                                {
                                    best = Some((d2, e.id));
                                }
                            }
                        }
                        best.map(|(_, id)| id)
                    }

                    let click_xy = (x_mm, y_mm);
                    let radius_mm = editor
                        .state
                        .placement_input
                        .as_ref()
                        .filter(|p| p.kind == PlacementInputKind::FilletRadius)
                        .and_then(|p| p.buffer.parse::<f64>().ok())
                        .filter(|r| r.is_finite() && *r > 1e-9)
                        .unwrap_or_else(|| {
                            editor
                                .state
                                .dimension_input
                                .trim()
                                .parse::<f64>()
                                .ok()
                                .filter(|r| r.is_finite() && *r > 1e-9)
                                .unwrap_or(0.5)
                        });

                    match editor.state.tool_pending {
                        ToolPending::FilletFirst { line: first_line } => {
                            let sketch_ref = match editor.primitive().sketch.as_ref() {
                                Some(s) => s,
                                None => {
                                    editor.state.tool_pending = ToolPending::Idle;
                                    return;
                                }
                            };
                            let second_line = match pick_line_at(sketch_ref, click_xy.0, click_xy.1)
                            {
                                Some(id) if id != first_line => id,
                                _ => {
                                    editor.state.solve_warnings.push(
                                        "Fillet: second click missed a different Line — pick the adjacent line".into(),
                                    );
                                    editor.state.tool_pending = ToolPending::Idle;
                                    return;
                                }
                            };
                            // Resolve the two Lines' endpoints.
                            let line_endpoints = |id: SketchEntityId| -> Option<(SketchEntityId, SketchEntityId)> {
                                sketch_ref.entities.iter().find(|e| e.id == id).and_then(
                                    |e| match e.kind {
                                        EntityKind::Line { start, end } => Some((start, end)),
                                        _ => None,
                                    },
                                )
                            };
                            let pos_of = |id: SketchEntityId| -> Option<(f64, f64)> {
                                sketch_ref
                                    .entities
                                    .iter()
                                    .find(|e| e.id == id)
                                    .and_then(|e| match e.kind {
                                        EntityKind::Point { x, y } => Some((x, y)),
                                        _ => None,
                                    })
                            };
                            let (a_s, a_e) = match line_endpoints(first_line) {
                                Some(p) => p,
                                None => {
                                    editor.state.tool_pending = ToolPending::Idle;
                                    return;
                                }
                            };
                            let (b_s, b_e) = match line_endpoints(second_line) {
                                Some(p) => p,
                                None => {
                                    editor.state.tool_pending = ToolPending::Idle;
                                    return;
                                }
                            };
                            // Find the shared corner Point.
                            let corner_id = if a_s == b_s || a_s == b_e {
                                a_s
                            } else if a_e == b_s || a_e == b_e {
                                a_e
                            } else {
                                editor.state.solve_warnings.push(
                                    "Fillet: the two Lines do not share an endpoint — bridge them with a Coincident constraint first".into(),
                                );
                                editor.state.tool_pending = ToolPending::Idle;
                                return;
                            };
                            // Identify the "outer" endpoint of each line.
                            let a_other = if a_s == corner_id { a_e } else { a_s };
                            let b_other = if b_s == corner_id { b_e } else { b_s };
                            let (cx, cy) = match pos_of(corner_id) {
                                Some(p) => p,
                                None => {
                                    editor.state.tool_pending = ToolPending::Idle;
                                    return;
                                }
                            };
                            let (ax, ay) = match pos_of(a_other) {
                                Some(p) => p,
                                None => {
                                    editor.state.tool_pending = ToolPending::Idle;
                                    return;
                                }
                            };
                            let (bx, by) = match pos_of(b_other) {
                                Some(p) => p,
                                None => {
                                    editor.state.tool_pending = ToolPending::Idle;
                                    return;
                                }
                            };
                            // Direction unit vectors away from corner.
                            let dax = ax - cx;
                            let day = ay - cy;
                            let dbx = bx - cx;
                            let dby = by - cy;
                            let alen = (dax * dax + day * day).sqrt();
                            let blen = (dbx * dbx + dby * dby).sqrt();
                            if alen <= 1e-9 || blen <= 1e-9 {
                                editor.state.tool_pending = ToolPending::Idle;
                                return;
                            }
                            let aux = dax / alen;
                            let auy = day / alen;
                            let bux = dbx / blen;
                            let buy = dby / blen;
                            // Half-angle between the two lines via dot product.
                            let cos_theta = (aux * bux + auy * buy).clamp(-1.0, 1.0);
                            let theta = cos_theta.acos();
                            if theta < 1e-3 || (std::f64::consts::PI - theta) < 1e-3 {
                                editor.state.solve_warnings.push(
                                    "Fillet: lines are colinear — nothing to round".into(),
                                );
                                editor.state.tool_pending = ToolPending::Idle;
                                return;
                            }
                            let half = theta * 0.5;
                            // Distance from corner to tangent point along each line.
                            let trim = radius_mm / half.tan();
                            let cap = trim.min(alen * 0.999).min(blen * 0.999);
                            if cap < radius_mm * 0.05 {
                                editor.state.solve_warnings.push(
                                    "Fillet: radius too large for these lines — pick a smaller r".into(),
                                );
                                editor.state.tool_pending = ToolPending::Idle;
                                return;
                            }
                            let r_used = cap * half.tan();
                            let ta_x = cx + aux * cap;
                            let ta_y = cy + auy * cap;
                            let tb_x = cx + bux * cap;
                            let tb_y = cy + buy * cap;
                            // Arc centre — on the angle bisector at
                            // distance r / sin(half) from the corner.
                            let bis_x = (aux + bux).abs() + (auy + buy).abs();
                            let _ = bis_x; // appease borrow checker, no-op
                            let mid_x = aux + bux;
                            let mid_y = auy + buy;
                            let mid_len = (mid_x * mid_x + mid_y * mid_y).sqrt().max(1e-9);
                            let bx_unit = mid_x / mid_len;
                            let by_unit = mid_y / mid_len;
                            let centre_off = r_used / half.sin();
                            let centre_x = cx + bx_unit * centre_off;
                            let centre_y = cy + by_unit * centre_off;
                            // Determine sweep direction — the arc opens
                            // away from the corner; pick CCW if the
                            // cross product (a -> b) is positive.
                            let cross = aux * buy - auy * bux;
                            let sweep_ccw = cross > 0.0;
                            // Mint two new tangent Points + an Arc; replace
                            // the corner endpoint references on the source
                            // Lines with the new tangent Points so the
                            // arc bridges them. We do this by updating the
                            // existing Line entities in-place via the
                            // sketch (no SketchEdit::EditLine variant
                            // exists yet — fall back to delete + re-add).
                            let ta_id = SketchEntityId::new();
                            let tb_id = SketchEntityId::new();
                            let centre_id = SketchEntityId::new();
                            let arc_id = SketchEntityId::new();
                            let entities = vec![
                                flag(Entity::new(
                                    ta_id,
                                    plane_id,
                                    EntityKind::Point { x: ta_x, y: ta_y },
                                )),
                                flag(Entity::new(
                                    tb_id,
                                    plane_id,
                                    EntityKind::Point { x: tb_x, y: tb_y },
                                )),
                                flag(Entity::new(
                                    centre_id,
                                    plane_id,
                                    EntityKind::Point {
                                        x: centre_x,
                                        y: centre_y,
                                    },
                                )),
                                flag(Entity::new(
                                    arc_id,
                                    plane_id,
                                    EntityKind::Arc {
                                        center: centre_id,
                                        start: ta_id,
                                        end: tb_id,
                                        sweep_ccw,
                                    },
                                )),
                            ];
                            for ent in entities {
                                editor.with_parts(|state, primitive| {
                                    apply_sketch_edit_with_warnings(
                                        state,
                                        primitive,
                                        SketchEdit::AddEntity(ent),
                                    );
                                });
                            }
                            // Rewrite the two source Lines so the corner
                            // endpoint becomes the new tangent point.
                            // No public SketchEdit variant rewrites a
                            // Line's endpoints, so we mutate the schema
                            // directly and trigger a force-rebuild.
                            if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                                for e in sketch.entities.iter_mut() {
                                    if e.id == first_line {
                                        if let EntityKind::Line { start, end } = &mut e.kind {
                                            if *start == corner_id {
                                                *start = ta_id;
                                            } else if *end == corner_id {
                                                *end = ta_id;
                                            }
                                        }
                                    }
                                    if e.id == second_line {
                                        if let EntityKind::Line { start, end } = &mut e.kind {
                                            if *start == corner_id {
                                                *start = tb_id;
                                            } else if *end == corner_id {
                                                *end = tb_id;
                                            }
                                        }
                                    }
                                }
                            }
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::ForceRebuild,
                                );
                            });
                            editor.state.tool_pending = ToolPending::Idle;
                        }
                        _ => {
                            // First click — pick the first Line.
                            let sketch_ref = match editor.primitive().sketch.as_ref() {
                                Some(s) => s,
                                None => {
                                    editor.state.tool_pending = ToolPending::Idle;
                                    return;
                                }
                            };
                            match pick_line_at(sketch_ref, click_xy.0, click_xy.1) {
                                Some(id) => {
                                    editor.state.tool_pending = ToolPending::FilletFirst { line: id };
                                }
                                None => {
                                    editor.state.solve_warnings.push(
                                        "Fillet: click missed any Line — try clicking closer to a line stroke".into(),
                                    );
                                    editor.state.tool_pending = ToolPending::Idle;
                                }
                            }
                        }
                    }
                }
                SketchTool::Trim => {
                    // v0.27 — EDA Trim. Single click on a Line: find
                    // its self-intersections with all other Lines,
                    // pick the two intersections that bracket the
                    // click point on the line, split the line into
                    // up-to-three segments, and remove the middle
                    // segment containing the click. If only one
                    // intersection exists, remove the side containing
                    // the click. If no intersection exists, remove
                    // the whole Line (Fusion-style "trim to nothing"
                    // is a useful EDA fallback for stripping a stray
                    // overlap).
                    fn line_xy(
                        sketch: &signex_sketch::SketchData,
                        id: SketchEntityId,
                    ) -> Option<((f64, f64), (f64, f64))> {
                        let pos_of = |pid: SketchEntityId| -> Option<(f64, f64)> {
                            sketch.entities.iter().find(|e| e.id == pid).and_then(|e| match e.kind {
                                EntityKind::Point { x, y } => Some((x, y)),
                                _ => None,
                            })
                        };
                        sketch.entities.iter().find(|e| e.id == id).and_then(|e| match e.kind {
                            EntityKind::Line { start, end } => {
                                Some((pos_of(start)?, pos_of(end)?))
                            }
                            _ => None,
                        })
                    }
                    fn pick_line_at_for_trim(
                        sketch: &signex_sketch::SketchData,
                        x: f64,
                        y: f64,
                    ) -> Option<SketchEntityId> {
                        const TOL_MM: f64 = 0.30;
                        let mut best: Option<(f64, SketchEntityId)> = None;
                        for e in &sketch.entities {
                            if let EntityKind::Line { .. } = e.kind
                                && let Some(((ax, ay), (bx, by))) = line_xy(sketch, e.id)
                            {
                                let dx = bx - ax;
                                let dy = by - ay;
                                let llen2 = dx * dx + dy * dy;
                                if llen2 <= 1e-12 {
                                    continue;
                                }
                                let t = ((x - ax) * dx + (y - ay) * dy) / llen2;
                                let tc = t.clamp(0.0, 1.0);
                                let px = ax + tc * dx;
                                let py = ay + tc * dy;
                                let d2 = (px - x).powi(2) + (py - y).powi(2);
                                if d2 <= TOL_MM * TOL_MM
                                    && best.as_ref().is_none_or(|(b2, _)| d2 < *b2)
                                {
                                    best = Some((d2, e.id));
                                }
                            }
                        }
                        best.map(|(_, id)| id)
                    }

                    let target_line = match editor.primitive().sketch.as_ref() {
                        Some(s) => pick_line_at_for_trim(s, x_mm, y_mm),
                        None => None,
                    };
                    let Some(target_line) = target_line else {
                        editor.state.solve_warnings.push(
                            "Trim: click missed any Line — try clicking closer to a line stroke".into(),
                        );
                        editor.state.tool_pending = ToolPending::Idle;
                        return;
                    };
                    // Compute intersections of `target_line` with every
                    // other Line; collect parametric `t` values.
                    let mut hits: Vec<f64> = Vec::new();
                    if let Some(s) = editor.primitive().sketch.as_ref()
                        && let Some(((ax, ay), (bx, by))) = line_xy(s, target_line)
                    {
                        let dx = bx - ax;
                        let dy = by - ay;
                        let llen2 = dx * dx + dy * dy;
                        if llen2 > 1e-12 {
                            for e in &s.entities {
                                if e.id == target_line {
                                    continue;
                                }
                                if let EntityKind::Line { .. } = e.kind
                                    && let Some(((cx, cy), (ex, ey))) = line_xy(s, e.id)
                                {
                                    let r_x = dx;
                                    let r_y = dy;
                                    let s_x = ex - cx;
                                    let s_y = ey - cy;
                                    let denom = r_x * s_y - r_y * s_x;
                                    if denom.abs() <= 1e-12 {
                                        continue;
                                    }
                                    let qx = cx - ax;
                                    let qy = cy - ay;
                                    let t = (qx * s_y - qy * s_x) / denom;
                                    let u = (qx * r_y - qy * r_x) / denom;
                                    if (1e-6..=1.0 - 1e-6).contains(&t)
                                        && (-1e-6..=1.0 + 1e-6).contains(&u)
                                    {
                                        hits.push(t);
                                    }
                                }
                            }
                        }
                        // Click t-value on target_line.
                        let click_t = if llen2 > 1e-12 {
                            ((x_mm - ax) * dx + (y_mm - ay) * dy) / llen2
                        } else {
                            0.5
                        };
                        // Bracketing the click between the nearest
                        // intersection below and above.
                        let lo = hits.iter().copied().filter(|t| *t < click_t).fold(0.0_f64, f64::max);
                        let hi = hits
                            .iter()
                            .copied()
                            .filter(|t| *t > click_t)
                            .fold(1.0_f64, f64::min);
                        // Three cases: full line (hits empty), half line
                        // (one hit), middle slice (two hits).
                        let trim_full = hits.is_empty();
                        let trim_lo = (lo - 0.0).abs() < 1e-9;
                        let trim_hi = (hi - 1.0).abs() < 1e-9;

                        if trim_full {
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::DeleteEntity(target_line),
                                );
                            });
                        } else if trim_lo && !trim_hi {
                            // Click is before the first intersection —
                            // shorten the line to start at `hi`.
                            let new_start = (ax + dx * hi, ay + dy * hi);
                            // Replace the line's start endpoint with a
                            // new Point at `new_start`.
                            let new_pid = SketchEntityId::new();
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(flag(Entity::new(
                                        new_pid,
                                        plane_id,
                                        EntityKind::Point {
                                            x: new_start.0,
                                            y: new_start.1,
                                        },
                                    ))),
                                );
                            });
                            if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                                for e in sketch.entities.iter_mut() {
                                    if e.id == target_line
                                        && let EntityKind::Line { start, .. } = &mut e.kind
                                    {
                                        *start = new_pid;
                                    }
                                }
                            }
                        } else if trim_hi && !trim_lo {
                            // Click is after the last intersection —
                            // shorten the line to end at `lo`.
                            let new_end = (ax + dx * lo, ay + dy * lo);
                            let new_pid = SketchEntityId::new();
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(flag(Entity::new(
                                        new_pid,
                                        plane_id,
                                        EntityKind::Point {
                                            x: new_end.0,
                                            y: new_end.1,
                                        },
                                    ))),
                                );
                            });
                            if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                                for e in sketch.entities.iter_mut() {
                                    if e.id == target_line
                                        && let EntityKind::Line { end, .. } = &mut e.kind
                                    {
                                        *end = new_pid;
                                    }
                                }
                            }
                        } else {
                            // Click bracketed by two intersections —
                            // split the line into two: [start..lo] and
                            // [hi..end]. We keep the original entity as
                            // the [start..lo] piece (rewriting its end)
                            // and mint a new Line for [hi..end].
                            let lo_pt = (ax + dx * lo, ay + dy * lo);
                            let hi_pt = (ax + dx * hi, ay + dy * hi);
                            let lo_pid = SketchEntityId::new();
                            let hi_pid = SketchEntityId::new();
                            let new_line_id = SketchEntityId::new();
                            // Capture the original end-point id so the
                            // mint of the second segment is correct.
                            let orig_end = if let Some(sk) = editor.primitive().sketch.as_ref() {
                                sk.entities.iter().find(|e| e.id == target_line).and_then(
                                    |e| match e.kind {
                                        EntityKind::Line { end, .. } => Some(end),
                                        _ => None,
                                    },
                                )
                            } else {
                                None
                            };
                            let Some(orig_end) = orig_end else {
                                editor.state.tool_pending = ToolPending::Idle;
                                return;
                            };
                            for ent in [
                                flag(Entity::new(
                                    lo_pid,
                                    plane_id,
                                    EntityKind::Point {
                                        x: lo_pt.0,
                                        y: lo_pt.1,
                                    },
                                )),
                                flag(Entity::new(
                                    hi_pid,
                                    plane_id,
                                    EntityKind::Point {
                                        x: hi_pt.0,
                                        y: hi_pt.1,
                                    },
                                )),
                                flag(Entity::new(
                                    new_line_id,
                                    plane_id,
                                    EntityKind::Line {
                                        start: hi_pid,
                                        end: orig_end,
                                    },
                                )),
                            ] {
                                editor.with_parts(|state, primitive| {
                                    apply_sketch_edit_with_warnings(
                                        state,
                                        primitive,
                                        SketchEdit::AddEntity(ent),
                                    );
                                });
                            }
                            if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                                for e in sketch.entities.iter_mut() {
                                    if e.id == target_line
                                        && let EntityKind::Line { end, .. } = &mut e.kind
                                    {
                                        *end = lo_pid;
                                    }
                                }
                            }
                        }
                        editor.with_parts(|state, primitive| {
                            apply_sketch_edit_with_warnings(
                                state,
                                primitive,
                                SketchEdit::ForceRebuild,
                            );
                        });
                    }
                    editor.state.tool_pending = ToolPending::Idle;
                }
            }
            // v0.24 Track D — buffer is consumed once per click. The
            // user has to type again before the next gesture step,
            // mirroring Fusion. Always clear when the resolve step
            // honoured the buffer; leave alone otherwise so a stray
            // pre-tool-pending keystroke survives until the user
            // either commits or Esc-clears.
            if used_placement_input {
                editor.state.placement_input = None;
                // v0.14-footprint — drop every parked dimension field
                // too so the next gesture starts with a clean buffer.
                editor.state.placement_input_others.clear();
            }
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
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
