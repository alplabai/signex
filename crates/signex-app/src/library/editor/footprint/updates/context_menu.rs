//! Footprint editor — context_menu update logic.
//!
//! Split out of `apply_footprint_primitive_edit` per ADR-0001 D1/D2.
//! The router delegates all context_menu `PrimitiveEditorMsg` variants here;
//! bodies are verbatim, so each arm keeps its own inner `use`s.

use crate::library::messages::PrimitiveEditorMsg;

pub(super) fn apply(editor: &mut crate::app::FootprintEditorState, msg: PrimitiveEditorMsg) {
    match msg {
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
        _ => unreachable!("non-context_menu variant routed to context_menu::apply"),
    }
}
