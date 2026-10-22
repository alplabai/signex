//! Footprint sketch updates — selection & tool/mode UI concern.
//!
//! Carved out of the monolithic `sketch::apply` (ADR-0001 D1/D2). `apply`
//! is a thin router; each variant delegates to one named per-action fn
//! below (object→action, ADR-0001 D2).

use crate::library::messages::FootprintEditorMsg;

pub(in crate::library::editor::footprint::updates) fn apply(
    editor: &mut crate::app::FootprintEditorState,
    msg: FootprintEditorMsg,
) {
    match msg {
        FootprintEditorMsg::SketchSelectMany(ids) => select_many(editor, ids),
        FootprintEditorMsg::SketchSetTool(tool) => set_tool(editor, tool),
        FootprintEditorMsg::SketchToggleConstruction => toggle_construction(editor),
        FootprintEditorMsg::SketchToggleCenterline => toggle_centerline(editor),
        FootprintEditorMsg::SketchToolEscape => tool_escape(editor),
        FootprintEditorMsg::SketchSelect { id, shift } => select(editor, id, shift),
        FootprintEditorMsg::SketchDimensionInput(s) => dimension_input(editor, s),
        _ => unreachable!("non-selection & tool/mode UI sketch variant routed to sketch_ui.rs"),
    }
}

// v0.27 — Sketch-mode multi-select replacement. First entity is primary
// (drives the inspector + DOF overlay focus); the second slots into the
// secondary (used by the constraint submenu's "two entities" pairing);
// the rest land in extras. Empty list deselects everything.
fn select_many(
    editor: &mut crate::app::FootprintEditorState,
    ids: Vec<signex_sketch::id::SketchEntityId>,
) {
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

fn set_tool(
    editor: &mut crate::app::FootprintEditorState,
    tool: crate::library::editor::footprint::state::SketchTool,
) {
    editor.state.active_tool = tool;
    editor.state.tool_pending = crate::library::editor::footprint::state::ToolPending::Idle;
    editor.canvas_cache.clear();
}

fn toggle_construction(editor: &mut crate::app::FootprintEditorState) {
    editor.state.construction_mode = !editor.state.construction_mode;
    // v0.22 Phase A5 — mutual exclusivity with centerline.
    if editor.state.construction_mode {
        editor.state.centerline_mode = false;
    }
    editor.canvas_cache.clear();
}

fn toggle_centerline(editor: &mut crate::app::FootprintEditorState) {
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

fn tool_escape(editor: &mut crate::app::FootprintEditorState) {
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

fn select(
    editor: &mut crate::app::FootprintEditorState,
    id: Option<signex_sketch::id::SketchEntityId>,
    shift: bool,
) {
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

fn dimension_input(editor: &mut crate::app::FootprintEditorState, s: String) {
    editor.state.dimension_input = s;
}
