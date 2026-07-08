//! Footprint sketch updates — selection & tool/mode UI concern.
//!
//! Carved out of the monolithic `sketch::apply` (ADR-0001 D1/D2). Arm
//! bodies are moved verbatim; each keeps its own inner `use`s.

use crate::library::messages::PrimitiveEditorMsg;

pub(super) fn apply(editor: &mut crate::app::FootprintEditorState, msg: PrimitiveEditorMsg) {
    match msg {
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
                editor.state.selected_sketch_extra = ids.iter().skip(2).copied().collect();
            }
            editor.canvas_cache.clear();
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
        PrimitiveEditorMsg::FootprintSketchDimensionInput(s) => {
            editor.state.dimension_input = s;
        }
        _ => unreachable!("non-selection & tool/mode UI sketch variant routed to sketch_ui.rs"),
    }
}
