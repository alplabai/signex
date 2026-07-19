//! Footprint editor — active_bar update logic.
//!
//! Split out of `apply_footprint_primitive_edit` per ADR-0001 D1/D2.
//! The router delegates all active_bar `FootprintEditorMsg` variants here;
//! bodies are verbatim, so each arm keeps its own inner `use`s.

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
        // No `push_history()` here — `apply_footprint_primitive_edit`
        // blanket-pushes for every message `mutates_footprint_state`
        // classifies as mutating, and none of these three are on its
        // exemption list. Pushing again would double-stack the history.
        FootprintEditorMsg::ActiveBarRotateSelection => {
            editor.with_parts(|state, primitive| {
                for idx in state.selected_pad_indices() {
                    if let Some(pad) = state.pads.get_mut(idx) {
                        pad.rotation_deg = (pad.rotation_deg + 90.0).rem_euclid(360.0);
                    }
                }
                CanvasState::sync_pads_to_primitive(state, primitive);
            });
            editor.state.active_bar_menu = None;
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        FootprintEditorMsg::ActiveBarFlipSelection => {
            editor.with_parts(|state, primitive| {
                for idx in state.selected_pad_indices() {
                    if let Some(pad) = state.pads.get_mut(idx) {
                        pad.layers = pad.layers.iter().map(flip_layer).collect();
                    }
                }
                CanvasState::sync_pads_to_primitive(state, primitive);
            });
            editor.state.active_bar_menu = None;
            editor.canvas_cache.clear();
            editor.dirty = true;
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
        FootprintEditorMsg::ActiveBarAlignSelectionToGrid => {
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
                // v0.23 — mirror the snap into the sketch so the
                // construction outline + centre Point follow the pad.
                // Skipping this left the sketch primitive stranded at
                // the pre-snap position.
                for snapshot in &snapshots {
                    pad_to_sketch::mirror_move_pad_in_sketch(snapshot, primitive);
                }
                CanvasState::sync_pads_to_primitive(state, primitive);
            });
            editor.state.active_bar_menu = None;
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        FootprintEditorMsg::ActiveBarMoveOriginToGrid => {
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
