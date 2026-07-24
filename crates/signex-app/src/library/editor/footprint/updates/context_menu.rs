//! Footprint editor — context_menu update logic.
//!
//! Split out of `apply_footprint_primitive_edit` per ADR-0001 D1/D2.
//! `apply` is a thin router; each `FootprintEditorMsg` variant delegates
//! to one named per-action fn below (object→action, ADR-0001 D2).

use crate::library::messages::FootprintEditorMsg;

pub(super) fn apply(editor: &mut crate::app::FootprintEditorState, msg: FootprintEditorMsg) {
    match msg {
        FootprintEditorMsg::ShowContextMenu { x, y, target } => show(editor, x, y, target),
        FootprintEditorMsg::CloseContextMenu => close(editor),
        FootprintEditorMsg::ContextMenuOpenSubmenu(sm) => open_submenu(editor, sm),
        FootprintEditorMsg::ContextMenuAction(action) => run_action(editor, action),
        _ => unreachable!("non-context_menu variant routed to context_menu::apply"),
    }
}

// v0.26 — right-click context menu plumbing. State-only
// mutations; canvas cache is cleared when target adjusts the
// selection (right-click on a pad selects it Altium-style).
fn show(
    editor: &mut crate::app::FootprintEditorState,
    x: f32,
    y: f32,
    target: crate::library::editor::footprint::state::FootprintContextTarget,
) {
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
                // #146 — a context click that changes the primary
                // selection must drop stale multi-select extras;
                // otherwise a later menu action (Align, Delete…)
                // would silently act on pads the user no longer
                // sees selected.
                editor.state.selected_pads_extra.clear();
                editor.state.selected_silk_f = None;
                editor.canvas_cache.clear();
            }
        }
        FootprintContextTarget::SilkF(idx) => {
            if editor.state.selected_silk_f != Some(idx) {
                editor.state.selected_silk_f = Some(idx);
                editor.state.selected_pad = None;
                // #146 — selecting a silk item abandons any pad
                // selection; clear the pad extras too so they
                // don't linger as an invisible multi-selection.
                editor.state.selected_pads_extra.clear();
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

fn close(editor: &mut crate::app::FootprintEditorState) {
    editor.state.context_menu = None;
}

fn open_submenu(
    editor: &mut crate::app::FootprintEditorState,
    sm: Option<crate::library::editor::footprint::state::FootprintContextSubmenu>,
) {
    if let Some(ref mut menu) = editor.state.context_menu {
        menu.submenu = sm;
    }
}

fn run_action(
    editor: &mut crate::app::FootprintEditorState,
    action: crate::library::editor::footprint::state::FootprintContextAction,
) {
    use crate::library::editor::footprint::state::FootprintContextAction as Act;
    match action {
        Act::SelectAllPads => {
            // #146 — mirror the active-bar Select All model: pad 0
            // becomes the primary selection and every remaining
            // pad lands in `selected_pads_extra`, so context-menu
            // Select All and the active-bar action select the same
            // set (previously this only picked pad 0).
            if !editor.state.pads.is_empty() {
                editor.state.selected_pad = Some(0);
                editor.state.selected_pads_extra = (1..editor.state.pads.len()).collect();
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
            // `EditorMsg::Footprint(FootprintEditorMsg::FitConsumed)` to clear the
            // request so it doesn''t re-trigger every event.
            editor.state.fit_pending = true;
            editor.state.context_menu = None;
            editor.canvas_cache.clear();
        }
    }
}
