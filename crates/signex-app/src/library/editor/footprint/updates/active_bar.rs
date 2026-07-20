//! Footprint editor — active_bar update logic.
//!
//! Split out of `apply_footprint_primitive_edit` per ADR-0001 D1/D2.
//! The router delegates all active_bar `FootprintEditorMsg` variants here;
//! bodies are verbatim, so each arm keeps its own inner `use`s.

use super::align_pads;
use super::footprint_nudge_selection;
use crate::library::editor::footprint::pad_to_sketch;
use crate::library::editor::footprint::state::FootprintEditorState as CanvasState;
use crate::library::messages::FootprintEditorMsg;

pub(super) fn apply(editor: &mut crate::app::FootprintEditorState, msg: FootprintEditorMsg) {
    match msg {
        FootprintEditorMsg::ToggleActiveBarMenu(menu) => {
            editor.state.active_bar_menu = match editor.state.active_bar_menu {
                Some(m) if m == menu => None,
                _ => Some(menu),
            };
        }
        FootprintEditorMsg::CloseActiveBarMenu => {
            editor.state.active_bar_menu = None;
        }
        FootprintEditorMsg::ActiveBarStub(label) => {
            crate::diagnostics::log_info(format!(
                "Active bar: {label} — coming soon (footprint Altium parity)"
            ));
            editor.state.active_bar_menu = None;
        }
        // Task 6 — apply footprint filter preset `idx` from the
        // persisted list. Re-read from disk on every apply so a
        // preset captured in a different tab/session is picked up
        // without needing an in-memory refresh.
        FootprintEditorMsg::ApplyFilterPreset(idx) => {
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
        FootprintEditorMsg::ToggleAllFilters => {
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
        FootprintEditorMsg::CaptureFilterPreset => {
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
        FootprintEditorMsg::ActiveBarToggleSnap(flag) => {
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
        FootprintEditorMsg::ActiveBarSetSnappingMode(mode) => {
            editor.state.snapping_mode = mode;
            editor.canvas_cache.clear();
        }
        FootprintEditorMsg::ActiveBarSetSnapSubTab(sub) => {
            editor.state.snap_subtab = sub;
            editor.canvas_cache.clear();
        }
        // v0.28 — Rotate / Flip / Align-to-Grid act on the WHOLE
        // selection. All three used to read `state.selected_pad` alone
        // and silently transform one pad out of N; a partial flip in
        // particular leaves mixed F./B. layers, which is a fab error.
        // #146 put all three on `mutates_footprint_state`'s exemption
        // list, so `apply_footprint_primitive_edit` does NOT blanket-push
        // for them: each snapshots here itself — exactly once, and only
        // when the selection is non-empty, so a no-op transform never
        // stacks undo history or dirties the document.
        FootprintEditorMsg::ActiveBarRotateSelection => {
            // #146 + #433 — snapshot + dirty only when at least one pad is
            // selected (a no-op rotate must not stack undo history or dirty
            // the document), then rotate EVERY selected pad (#390 multi-select)
            // and re-mint each pad's derived sketch geometry through its one
            // owner so positions off the moved frame follow it (#433).
            if !editor.state.selected_pad_indices().is_empty() {
                editor.push_history();
                editor.with_parts(|state, primitive| {
                    for idx in state.selected_pad_indices() {
                        let Some(pad) = state.pads.get_mut(idx) else {
                            continue;
                        };
                        pad.rotation_deg = (pad.rotation_deg + 90.0).rem_euclid(360.0);
                        if !pad_to_sketch::remint_pad_geometry(pad, primitive) {
                            pad_to_sketch::warn_profile_pad_untransformed("Rotate", &pad.number);
                        }
                    }
                    CanvasState::sync_pads_to_primitive(state, primitive);
                });
                editor.canvas_cache.clear();
                editor.dirty = true;
            }
            editor.state.active_bar_menu = None;
        }
        FootprintEditorMsg::ActiveBarFlipSelection => {
            // #146 + #433 — gate history + dirty on a real selection (see
            // rotate), then flip EVERY selected pad in place (#390 multi-
            // select). Flipping to the other side mirrors the pad's copper
            // about its own vertical axis: `signex_bake::pad` consumes the
            // stored fields verbatim, so the WHOLE mirror-sensitive set moves
            // together (angle, hole angle, copper X offset, chamfer corners,
            // custom outline) — mirroring only the angle bakes a shape that is
            // neither the front nor the back one. Pad POSITIONS are not
            // mirrored (this flips each pad in place, not the footprint about
            // its origin). The re-mint refreshes the sketch `PadAttr`, so the
            // bake reads the swapped chamfer corners, not the pre-flip ones.
            if !editor.state.selected_pad_indices().is_empty() {
                editor.push_history();
                editor.with_parts(|state, primitive| {
                    for idx in state.selected_pad_indices() {
                        let Some(pad) = state.pads.get_mut(idx) else {
                            continue;
                        };
                        pad.layers = pad.layers.iter().map(flip_layer).collect();
                        pad.mirror_about_own_vertical_axis();
                        if !pad_to_sketch::remint_pad_geometry(pad, primitive) {
                            pad_to_sketch::warn_profile_pad_untransformed("Flip", &pad.number);
                        }
                    }
                    CanvasState::sync_pads_to_primitive(state, primitive);
                });
                editor.canvas_cache.clear();
                editor.dirty = true;
            }
            editor.state.active_bar_menu = None;
        }
        FootprintEditorMsg::ActiveBarNudgeSelection => {
            // v0.14 — "Move Selection by X, Y…" one-step nudge: the
            // combined selection by one active grid step in +X and +Y.
            // Shares geometry + sketch-mirror + history with the
            // typed-delta Move-By modal via `footprint_nudge_selection`.
            let step = editor.state.snap_options.grid_step_mm.max(0.001);
            footprint_nudge_selection(editor, step, step);
            editor.state.active_bar_menu = None;
        }
        FootprintEditorMsg::MoveByOpen => {
            editor.state.move_by_modal = Some(Default::default());
            editor.state.active_bar_menu = None;
        }
        FootprintEditorMsg::MoveBySetX(v) => {
            if let Some(m) = editor.state.move_by_modal.as_mut() {
                m.dx_buf = v;
            }
        }
        FootprintEditorMsg::MoveBySetY(v) => {
            if let Some(m) = editor.state.move_by_modal.as_mut() {
                m.dy_buf = v;
            }
        }
        FootprintEditorMsg::MoveByConfirm => {
            if let Some((dx, dy)) = editor.state.move_by_modal.as_ref().and_then(|m| m.parsed()) {
                footprint_nudge_selection(editor, dx, dy);
            }
            editor.state.move_by_modal = None;
        }
        FootprintEditorMsg::MoveByCancel => {
            editor.state.move_by_modal = None;
        }
        // #370 — "Align…" dialog. Open/edit/cancel mutate only the
        // transient `align_modal` state; Confirm composes the chosen
        // per-axis ops over the SAME `align_pads` helper the concrete
        // `AlignPads` rows use, under exactly one history snapshot.
        FootprintEditorMsg::AlignOpen => {
            editor.state.align_modal = Some(Default::default());
            editor.state.active_bar_menu = None;
        }
        FootprintEditorMsg::AlignSetHorizontal(op) => {
            if let Some(m) = editor.state.align_modal.as_mut() {
                m.horizontal = op;
            }
        }
        FootprintEditorMsg::AlignSetVertical(op) => {
            if let Some(m) = editor.state.align_modal.as_mut() {
                m.vertical = op;
            }
        }
        FootprintEditorMsg::AlignConfirm => {
            use crate::library::editor::footprint::state::AlignOp;

            // Read the two chosen ops before mutating anything.
            let (chosen_h, chosen_v) = editor
                .state
                .align_modal
                .as_ref()
                .map(|m| (m.horizontal, m.vertical))
                .unwrap_or((None, None));

            // Collect + dedup the selection indices up front so we can
            // decide whether ANY chosen op can apply before touching
            // history. Mirrors the `AlignPads` handler's collection.
            let mut indices: Vec<usize> = Vec::new();
            if let Some(p) = editor.state.selected_pad {
                indices.push(p);
            }
            indices.extend(editor.state.selected_pads_extra.iter().copied());
            indices.sort_unstable();
            indices.dedup();
            indices.retain(|&i| i < editor.state.pads.len());

            // Keep only the chosen ops the selection is large enough to
            // apply — align needs ≥2 pads, distribute ≥3 (the exact gate
            // `AlignPads` uses per concrete row). This makes Confirm
            // identical to picking those rows one at a time: each too-
            // small op falls through as a clean no-op.
            let applicable: Vec<AlignOp> = [chosen_h, chosen_v]
                .into_iter()
                .flatten()
                .filter(|op| {
                    let min_needed = match op {
                        AlignOp::DistributeH | AlignOp::DistributeV => 3,
                        _ => 2,
                    };
                    indices.len() >= min_needed
                })
                .collect();

            // Choosing neither axis (or a selection too small for every
            // chosen op) is a clean no-op: no history, no dirty. Only
            // when at least one op will actually move pads do we snapshot
            // ONCE and apply every applicable op under it — that single
            // `push_history` is what makes the whole confirm one undo
            // step even when both axes are applied. The two axes are
            // independent (H touches X, V touches Y), so applying them in
            // sequence equals two separate `AlignPads` dispatches.
            if !applicable.is_empty() {
                editor.push_history();
                editor.with_parts(|state, primitive| {
                    // Reuse the active grid step for the (unused here, but
                    // required) spacing increment, matching `AlignPads`.
                    let step = state.snap_options.grid_step_mm.max(0.001);
                    for op in &applicable {
                        align_pads(state, &indices, *op, step);
                    }
                    // Mirror every selected pad's final centre into the
                    // sketch once, after BOTH axes settle, then re-sync
                    // the literal `Pad` list — same as `AlignPads`.
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
            editor.state.align_modal = None;
        }
        FootprintEditorMsg::AlignCancel => {
            editor.state.align_modal = None;
        }
        FootprintEditorMsg::ActiveBarAlignSelectionToGrid => {
            // #146 + #433 — gate history + dirty on a real selection (see
            // rotate), then snap EVERY selected pad to the grid (#390 multi-
            // select) and mirror each snap into the sketch so the construction
            // outline + centre Point follow the pad (v0.23).
            if !editor.state.selected_pad_indices().is_empty() {
                editor.push_history();
                editor.with_parts(|state, primitive| {
                    let step = state.snap_options.grid_step_mm.max(0.001);
                    let mut snapshots: Vec<crate::library::editor::footprint::state::EditorPad> =
                        Vec::new();
                    for idx in state.selected_pad_indices() {
                        if let Some(pad) = state.pads.get_mut(idx) {
                            let (x, y) = pad.position_mm;
                            pad.position_mm = ((x / step).round() * step, (y / step).round() * step);
                            snapshots.push(pad.clone());
                        }
                    }
                    for snapshot in &snapshots {
                        pad_to_sketch::mirror_move_pad_in_sketch(snapshot, primitive);
                    }
                    CanvasState::sync_pads_to_primitive(state, primitive);
                });
                editor.canvas_cache.clear();
                editor.dirty = true;
            }
            editor.state.active_bar_menu = None;
        }
        FootprintEditorMsg::ActiveBarMoveOriginToGrid => {
            // #146 — snapshot + dirty only when there is at least one
            // pad to move; an empty footprint must stay clean.
            if !editor.state.pads.is_empty() {
                editor.push_history();
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
                editor.canvas_cache.clear();
                editor.dirty = true;
            }
            editor.state.active_bar_menu = None;
        }
        FootprintEditorMsg::ActiveBarSelectAll => {
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
                        editor.state.selected_pads_extra = (1..editor.state.pads.len()).collect();
                    }
                }
                EditorMode::View3d => {}
            }
            editor.canvas_cache.clear();
            editor.state.active_bar_menu = None;
        }
        FootprintEditorMsg::ActiveBarClearSelection => {
            editor.state.selected_pad = None;
            editor.state.selected_pads_extra.clear();
            editor.state.selected_sketch = None;
            editor.state.selected_sketch_secondary = None;
            editor.state.selected_sketch_extra.clear();
            editor.state.selected_silk_f = None;
            editor.canvas_cache.clear();
            editor.state.active_bar_menu = None;
        }
        FootprintEditorMsg::ActiveBarSetSketchTool(tool) => {
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
        _ => unreachable!("non-active_bar variant routed to active_bar::apply"),
    }
}

/// Swap a layer between the front and back side. Anything without an
/// `F.` / `B.` prefix (`*.Cu`, bare names) is side-agnostic and passes
/// through unchanged.
fn flip_layer(layer: &signex_library::LayerId) -> signex_library::LayerId {
    let s = layer.as_str();
    let flipped = if let Some(rest) = s.strip_prefix("F.") {
        format!("B.{rest}")
    } else if let Some(rest) = s.strip_prefix("B.") {
        format!("F.{rest}")
    } else {
        s.to_string()
    };
    signex_library::LayerId::new(flipped)
}
