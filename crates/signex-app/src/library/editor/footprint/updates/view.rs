//! Footprint editor — view update logic.
//!
//! Split out of `apply_footprint_primitive_edit` per ADR-0001 D1/D2.
//! The router delegates all view `FootprintEditorMsg` variants here;
//! bodies are verbatim, so each arm keeps its own inner `use`s.

use super::align_pads;
use crate::library::editor::footprint::layers::FpLayer;
use crate::library::editor::footprint::pad_to_sketch;
use crate::library::editor::footprint::state::FootprintEditorState as CanvasState;
use crate::library::messages::FootprintEditorMsg;

pub(super) fn apply(editor: &mut crate::app::FootprintEditorState, msg: FootprintEditorMsg) {
    match msg {
        FootprintEditorMsg::ToggleLayer(name) => {
            if let Some(layer) = FpLayer::from_standard_name(&name) {
                editor.state.layer_visibility.toggle(layer);
                editor.canvas_cache.clear();
            }
        }
        FootprintEditorMsg::ToggleAutoFit => {
            editor.state.toggle_auto_fit();
            editor.with_parts(|state, primitive| {
                CanvasState::sync_pads_to_primitive(state, primitive);
            });
            editor.canvas_cache.clear();
        }
        FootprintEditorMsg::SetMode(mode) => {
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
            // Same reason, for the open dropdown: Sketch ▸ Create and
            // Sketch ▸ Modify only have a trigger button while the bar
            // is in Sketch mode, so a mode change with one of them open
            // would strand the panel over a button that no longer
            // exists. (Mouse-driven mode changes can't reach here with
            // a menu open — the dropdown's backstop eats the click —
            // but a keyboard shortcut can.)
            editor.state.active_bar_menu = None;
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
        FootprintEditorMsg::TogglePlacementPause => {
            editor.state.placement_paused = !editor.state.placement_paused;
            editor.canvas_cache.clear();
        }

        FootprintEditorMsg::FitConsumed => {
            editor.state.fit_pending = false;
        }
        // v0.26-E — clipboard ops intercepted at the call site
        // (apply_footprint_clipboard_op needs split-borrow with
        // document_state.pad_clipboard). The match arm here is
        // unreachable in practice but required for exhaustiveness.
        FootprintEditorMsg::CopyPad | FootprintEditorMsg::CutPad | FootprintEditorMsg::PastePad => {
        }
        FootprintEditorMsg::SetPadsTool(tool) => {
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
        FootprintEditorMsg::ToolEscape => {
            // Esc unwinds the deepest thing first — the same ladder
            // `bootstrap::subscription` walks for modals, and what the
            // symbol editor already does. With the right-click menu
            // open, Esc dismisses it and stops there; falling through
            // would ALSO drop the pad selection the menu was opened to
            // act on, so the user loses their selection just for
            // backing out of a menu. Until now the footprint editor had
            // no Esc path for its context menu at all — the only way
            // out was a click somewhere harmless.
            if editor.state.context_menu.is_some() {
                editor.state.context_menu = None;
                editor.canvas_cache.clear();
                return;
            }
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
        FootprintEditorMsg::AlignPads(op) => {
            // v0.14 — active-bar Align/Distribute/Spacing. Operates on
            // the combined selection (`selected_pad` + the ctrl-click
            // extras). Mirrors every moved pad into the backing sketch
            // and pushes one history snapshot, exactly like the
            // group-drag path in `FootprintMovePad`.
            use crate::library::editor::footprint::state::AlignOp;

            // Collect + dedup the selection indices up front so we can
            // bail before touching history if there isn't enough to act
            // on. Align ops need ≥2 pads; distribute needs ≥3.
            let indices = editor.state.selected_pad_indices();

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
        FootprintEditorMsg::SetName(new_name) => {
            // Rename the ACTIVE internal footprint. The .snxfpt
            // envelope holds N footprints; only the user-selected one
            // mutates. Empty names are accepted but treated as
            // "unnamed" for breadcrumb / file display purposes.
            editor.primitive_mut().name = new_name;
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        FootprintEditorMsg::RecomputeCourtyardOutline => {
            // v0.27 — outline-following courtyard. Pure read-write
            // on the editor state; the new polygon lands on
            // `state.courtyard_outline_mm` and the canvas draws it
            // in preference to the bbox.
            editor.state.recompute_courtyard_outline();
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        _ => unreachable!("non-view variant routed to view::apply"),
    }
}
