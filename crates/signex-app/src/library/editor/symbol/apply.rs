//! Symbol-editor message reducer, extracted from the library
//! dispatcher so the editor's state transitions live next to the
//! `SymbolEditorState` they mutate (issue #98 — decomposing the
//! 10k-line `app/dispatch/library.rs`). Pure state reduction: it takes
//! the editor and a `PrimitiveEditorMsg` and returns nothing, so it
//! moves verbatim with no behavioural change.

use crate::library::messages::{
    GraphicHandleMsg, PrimitiveEditorMsg, SymbolSelectionMsg, SymbolToolMsg,
};

/// Apply a primitive-editor event to a standalone Symbol editor
/// state. Mirrors the symbol-tab arms of `apply_inline_edit` but
/// against the path-keyed standalone state. Visibility is
/// `pub(crate)` so unit tests in sibling modules can drive the editor
/// through the same code path the dispatcher uses.
pub(crate) fn apply_symbol_primitive_edit(
    editor: &mut crate::app::SymbolEditorState,
    msg: PrimitiveEditorMsg,
) {
    use crate::library::editor::symbol::canvas::SymbolTool;
    use crate::library::editor::symbol::state::{FieldKey, SymbolSelection};

    // Capture an undo snapshot ahead of any mutating message — mirrors
    // the footprint reducer. Undo/Redo and pure-UI messages (tool /
    // selection / pan / cursor) are excluded so they don't pollute the
    // history timeline.
    if mutates_symbol_state(&msg) {
        editor.push_history();
    }

    match msg {
        PrimitiveEditorMsg::SymbolUndo => {
            editor.undo();
        }
        PrimitiveEditorMsg::SymbolRedo => {
            editor.redo();
        }
        PrimitiveEditorMsg::SymbolSetTool(tool) => {
            editor.tool = match tool {
                SymbolToolMsg::Select => SymbolTool::Select,
                SymbolToolMsg::AddPin => SymbolTool::AddPin,
                SymbolToolMsg::PlaceRectangle => SymbolTool::PlaceRectangle,
                SymbolToolMsg::PlaceLine => SymbolTool::PlaceLine,
                SymbolToolMsg::PlaceCircle => SymbolTool::PlaceCircle,
                SymbolToolMsg::PlaceArc => SymbolTool::PlaceArc,
                SymbolToolMsg::PlaceText => SymbolTool::PlaceText,
            };
        }
        PrimitiveEditorMsg::SymbolToggleActiveBarMenu(menu) => {
            editor.active_bar_menu = match editor.active_bar_menu {
                Some(m) if m == menu => None,
                _ => Some(menu),
            };
        }
        PrimitiveEditorMsg::SymbolCloseActiveBarMenu => {
            editor.active_bar_menu = None;
        }
        PrimitiveEditorMsg::SymbolActiveBarStub(label) => {
            crate::diagnostics::log_info(format!(
                "Symbol active bar: {label} — coming soon (SchLib Altium parity)"
            ));
            editor.active_bar_menu = None;
        }
        PrimitiveEditorMsg::SymbolToggleSelectionFilter(kind) => {
            editor.selection_filter.toggle(kind);
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolAddPin { x, y } => {
            let active_part = editor.active_part;
            let idx = crate::library::editor::symbol::state::add_pin(
                editor.primitive_mut(),
                x,
                y,
                active_part,
            );
            editor.selected = Some(SymbolSelection::Pin(idx));
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolAddRectangle { x, y } => {
            // Default 10×5 mm rectangle centred on the click. User
            // edits the corners later via Properties (graphics-properties
            // surface lands in a follow-up; for now they can move/delete
            // through the Select tool).
            const W: f64 = 5.08; // half-width 5.08 mm → 10.16 mm overall
            const H: f64 = 2.54; // half-height
            editor
                .primitive_mut()
                .graphics
                .push(signex_library::SymbolGraphic {
                    kind: signex_library::SymbolGraphicKind::Rectangle {
                        from: [x - W, y - H],
                        to: [x + W, y + H],
                    },
                    stroke_width: 0.15,
                });
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolAddLine { x, y } => {
            // 5 mm horizontal line going right.
            const L: f64 = 5.08;
            editor
                .primitive_mut()
                .graphics
                .push(signex_library::SymbolGraphic {
                    kind: signex_library::SymbolGraphicKind::Line {
                        from: [x, y],
                        to: [x + L, y],
                    },
                    stroke_width: 0.15,
                });
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolAddArc { x, y } => {
            // Default 2 mm-radius arc, 0°→90° quadrant centred on
            // the click. User edits start/end angle via Properties
            // (or drag-to-resize the start/end handles).
            editor
                .primitive_mut()
                .graphics
                .push(signex_library::SymbolGraphic {
                    kind: signex_library::SymbolGraphicKind::Arc {
                        center: [x, y],
                        radius: 2.0,
                        start_deg: 0.0,
                        end_deg: 90.0,
                    },
                    stroke_width: 0.15,
                });
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolAddText { x, y } => {
            // Default "Text" label at the click position. User edits
            // the content + size via Properties.
            editor
                .primitive_mut()
                .graphics
                .push(signex_library::SymbolGraphic {
                    kind: signex_library::SymbolGraphicKind::Text {
                        position: [x, y],
                        content: "Text".to_string(),
                        size: 1.27,
                    },
                    stroke_width: 0.0,
                });
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolAddCircle { x, y } => {
            // 2 mm-radius circle centred on the click.
            editor
                .primitive_mut()
                .graphics
                .push(signex_library::SymbolGraphic {
                    kind: signex_library::SymbolGraphicKind::Circle {
                        center: [x, y],
                        radius: 2.0,
                    },
                    stroke_width: 0.15,
                });
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolSelect(sel) => {
            editor.selected = Some(match sel {
                SymbolSelectionMsg::Pin(idx) => SymbolSelection::Pin(idx),
                SymbolSelectionMsg::FieldReference => SymbolSelection::Field(FieldKey::Reference),
                SymbolSelectionMsg::FieldValue => SymbolSelection::Field(FieldKey::Value),
                SymbolSelectionMsg::Graphic(idx) => SymbolSelection::Graphic(idx),
            });
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolDeselect => {
            editor.selected = None;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolMoveSelected { x, y } => {
            let selected = editor.selected.clone();
            crate::library::editor::symbol::state::move_selected(
                editor.primitive_mut(),
                selected,
                x,
                y,
            );
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolMoveGraphicHandle { idx, handle, x, y } => {
            let h = graphic_handle_msg_to_state(handle);
            crate::library::editor::symbol::state::move_graphic_handle(
                editor.primitive_mut(),
                idx,
                h,
                x,
                y,
            );
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolDeleteSelected => {
            let selected = editor.selected.clone();
            if let Some(new_sel) = crate::library::editor::symbol::state::delete_selected(
                editor.primitive_mut(),
                selected,
            ) {
                editor.selected = new_sel;
                editor.dirty = true;
                editor.canvas_cache.clear();
            }
        }
        PrimitiveEditorMsg::SymbolRotateSelected { clockwise } => {
            let selected = editor.selected.clone();
            crate::library::editor::symbol::state::rotate_selected(
                editor.primitive_mut(),
                selected,
                clockwise,
            );
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolSetPinNumber { idx, number } => {
            if let Some(pin) = editor.primitive_mut().pins.get_mut(idx) {
                pin.number = number;
                editor.dirty = true;
            }
        }
        PrimitiveEditorMsg::SymbolSetPinName { idx, name } => {
            if let Some(pin) = editor.primitive_mut().pins.get_mut(idx) {
                pin.name = name;
                editor.dirty = true;
            }
        }
        // ── View / camera ────────────────────────────────────────
        PrimitiveEditorMsg::SymbolPan { dx, dy } => {
            editor.camera.pan(dx, dy);
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolZoom { sx, sy, delta } => {
            // Wheel events feed `delta`; the camera applies its own
            // ZOOM_FACTOR + clamp inside `zoom_at`.
            let viewport = iced::Rectangle {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            };
            if editor
                .camera
                .zoom_at(iced::Point::new(sx, sy), delta, viewport)
            {
                editor.canvas_cache.clear();
            }
        }
        PrimitiveEditorMsg::SymbolFit => {
            // Compute the symbol bbox using the canvas helper, then
            // ask the camera to fit it. We don't have the actual
            // viewport size here — use a sensible default so the
            // first Fit recovers from any pan/zoom; the user can
            // press Home again after resizing the window for a
            // tighter fit.
            let (min_x, min_y, max_x, max_y) = symbol_bbox(editor.primitive());
            let world_rect = iced::Rectangle {
                x: min_x as f32,
                y: -(max_y as f32), // Standard y-up → screen y-down
                width: (max_x - min_x).max(1.0) as f32,
                height: (max_y - min_y).max(1.0) as f32,
            };
            let viewport = iced::Rectangle {
                x: 0.0,
                y: 0.0,
                width: 800.0,
                height: 500.0,
            };
            editor.camera.fit_rect(world_rect, viewport);
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolCursorAt { x_mm, y_mm } => {
            editor.cursor_mm = match (x_mm, y_mm) {
                (Some(x), Some(y)) => Some((x, y)),
                _ => None,
            };
        }
        // SymbolSetSheetColor / SymbolToggleGrid / SymbolCycleGridSize
        // / SymbolCycleUnit are intercepted in handle_primitive_editor_event
        // before this match runs — they mutate `OpenLibrary.display`,
        // not the per-tab editor state. List them here so the
        // dispatcher stays exhaustive across the enum (and matches
        // the footprint catch-all).
        PrimitiveEditorMsg::SymbolSetSheetColor(_)
        | PrimitiveEditorMsg::SymbolToggleGrid
        | PrimitiveEditorMsg::SymbolCycleGridSize
        | PrimitiveEditorMsg::SymbolCycleUnit => {}
        // ── Multi-part component ────────────────────────────────
        PrimitiveEditorMsg::SymbolPrevPart => {
            if editor.active_part > 1 {
                editor.active_part -= 1;
                editor.canvas_cache.clear();
            }
        }
        PrimitiveEditorMsg::SymbolNextPart => {
            let max = crate::library::editor::symbol::state::max_part_number(editor.primitive());
            if editor.active_part < max {
                editor.active_part += 1;
                editor.canvas_cache.clear();
            }
        }
        PrimitiveEditorMsg::SymbolNewPart => {
            // Bump the symbol's max declared part by one and switch
            // to it. The new part starts pinless; the user adds pins
            // in Add Pin mode with the new active_part selected.
            let new_part =
                crate::library::editor::symbol::state::max_part_number(editor.primitive()) + 1;
            editor.active_part = new_part;
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolRemovePart => {
            // Refuse to remove if this is the only part — a single-
            // part symbol must always have part 1 active.
            let max = crate::library::editor::symbol::state::max_part_number(editor.primitive());
            if max <= 1 || editor.active_part <= 1 {
                // No-op; surface a tracing line so a user-visible
                // toast can land later if needed.
                tracing::debug!(
                    target: "signex::library",
                    active = editor.active_part,
                    max,
                    "SymbolRemovePart: refusing to remove the only part"
                );
            } else {
                let to_remove = editor.active_part;
                crate::library::editor::symbol::state::demote_part_pins_to_part_one(
                    editor.primitive_mut(),
                    to_remove,
                );
                editor.active_part = 1;
                editor.dirty = true;
                editor.canvas_cache.clear();
            }
        }
        // Footprint variants are no-ops on a Symbol editor — the
        // dispatcher uses path-keyed lookup so a misrouted event
        // can't actually reach this match arm in practice.
        PrimitiveEditorMsg::FootprintSelectActiveIdx(_)
        | PrimitiveEditorMsg::FootprintAddNewSibling
        | PrimitiveEditorMsg::FootprintAddPad { .. }
        | PrimitiveEditorMsg::FootprintAddHole { .. }
        | PrimitiveEditorMsg::FootprintAddText { .. }
        | PrimitiveEditorMsg::FootprintTrackClick { .. }
        | PrimitiveEditorMsg::FootprintTrackCancel
        | PrimitiveEditorMsg::FootprintArcClick { .. }
        | PrimitiveEditorMsg::FootprintArcCancel
        | PrimitiveEditorMsg::FootprintPolygonClick { .. }
        | PrimitiveEditorMsg::FootprintPolygonCommit
        | PrimitiveEditorMsg::FootprintPolygonCancel
        | PrimitiveEditorMsg::FootprintSelectSilkF(_)
        | PrimitiveEditorMsg::FootprintDeleteSilkF
        | PrimitiveEditorMsg::FootprintToggleSelectionFilter(_)
        | PrimitiveEditorMsg::FootprintMovePad { .. }
        | PrimitiveEditorMsg::FootprintCursorAt { .. }
        | PrimitiveEditorMsg::FootprintSelectPad(_)
        | PrimitiveEditorMsg::FootprintSelectPads(_)
        | PrimitiveEditorMsg::FootprintSketchSelectMany(_)
        | PrimitiveEditorMsg::FootprintDeleteSelected
        | PrimitiveEditorMsg::FootprintToggleLayer(_)
        | PrimitiveEditorMsg::FootprintToggleAutoFit
        | PrimitiveEditorMsg::FootprintSetPadsTool(_)
        | PrimitiveEditorMsg::FootprintToolEscape
        | PrimitiveEditorMsg::FootprintToggleActiveBarMenu(_)
        | PrimitiveEditorMsg::FootprintCloseActiveBarMenu
        | PrimitiveEditorMsg::FootprintActiveBarStub(_)
        | PrimitiveEditorMsg::FootprintActiveBarToggleSnap(_)
        | PrimitiveEditorMsg::FootprintActiveBarSetSnappingMode(_)
        | PrimitiveEditorMsg::FootprintActiveBarSetSnapSubTab(_)
        | PrimitiveEditorMsg::FootprintActiveBarRotateSelection
        | PrimitiveEditorMsg::FootprintActiveBarFlipSelection
        | PrimitiveEditorMsg::FootprintActiveBarAlignSelectionToGrid
        | PrimitiveEditorMsg::FootprintActiveBarMoveOriginToGrid
        | PrimitiveEditorMsg::FootprintActiveBarSelectAll
        | PrimitiveEditorMsg::FootprintActiveBarClearSelection
        | PrimitiveEditorMsg::FootprintActiveBarSetSketchTool(_)
        | PrimitiveEditorMsg::FootprintSetName(_)
        | PrimitiveEditorMsg::FootprintSetMode(_)
        | PrimitiveEditorMsg::FootprintSketchPlacePoint { .. }
        | PrimitiveEditorMsg::FootprintSketchEditParameter { .. }
        | PrimitiveEditorMsg::FootprintSketchSetTool(_)
        | PrimitiveEditorMsg::FootprintSketchToggleConstruction
        | PrimitiveEditorMsg::FootprintSketchToggleCenterline
        | PrimitiveEditorMsg::FootprintTogglePlacementPause
        | PrimitiveEditorMsg::FootprintSketchToolClick { .. }
        | PrimitiveEditorMsg::FootprintSketchToolEscape
        | PrimitiveEditorMsg::FootprintSketchPlacementInputChar(_)
        | PrimitiveEditorMsg::FootprintSketchPlacementInputBackspace
        | PrimitiveEditorMsg::FootprintSketchPlacementInputEnter
        | PrimitiveEditorMsg::FootprintSketchPlacementInputEscape
        | PrimitiveEditorMsg::FootprintSketchSelect { .. }
        | PrimitiveEditorMsg::FootprintSketchMovePoint { .. }
        | PrimitiveEditorMsg::FootprintSketchMoveLine { .. }
        | PrimitiveEditorMsg::FootprintSketchResizeRoundPad { .. }
        | PrimitiveEditorMsg::FootprintSetSelectionMode2d(_)
        | PrimitiveEditorMsg::FootprintSelectAllOnLayer
        | PrimitiveEditorMsg::FootprintAddVia { .. }
        | PrimitiveEditorMsg::FootprintSelectOffGridPads
        | PrimitiveEditorMsg::FootprintRecomputeCourtyardOutline
        | PrimitiveEditorMsg::FootprintLassoArm
        | PrimitiveEditorMsg::FootprintLassoAddVertex { .. }
        | PrimitiveEditorMsg::FootprintLassoCommit
        | PrimitiveEditorMsg::FootprintLassoCancel
        | PrimitiveEditorMsg::FootprintTouchingLineArm
        | PrimitiveEditorMsg::FootprintTouchingLineFirst { .. }
        | PrimitiveEditorMsg::FootprintTouchingLineCommit { .. }
        | PrimitiveEditorMsg::FootprintTouchingLineCancel
        | PrimitiveEditorMsg::FootprintSelectOverlapped
        | PrimitiveEditorMsg::FootprintSelectNextOverlapped
        | PrimitiveEditorMsg::FootprintSketchAddConstraintForSelection(_)
        | PrimitiveEditorMsg::FootprintSketchDimensionInput(_)
        | PrimitiveEditorMsg::FootprintSketchSetRole { .. }
        | PrimitiveEditorMsg::FootprintSketchMakePadFromProfile
        | PrimitiveEditorMsg::FootprintSketchUnlinkCornerRadius { .. }
        | PrimitiveEditorMsg::FootprintShowContextMenu { .. }
        | PrimitiveEditorMsg::FootprintCloseContextMenu
        | PrimitiveEditorMsg::FootprintContextMenuOpenSubmenu(_)
        | PrimitiveEditorMsg::FootprintContextMenuAction(_)
        | PrimitiveEditorMsg::FootprintFitConsumed
        | PrimitiveEditorMsg::FootprintCopyPad
        | PrimitiveEditorMsg::FootprintCutPad
        | PrimitiveEditorMsg::FootprintPastePad
        | PrimitiveEditorMsg::Save => {}
    }
}

fn symbol_bbox(sym: &signex_library::Symbol) -> (f64, f64, f64, f64) {
    use signex_library::SymbolGraphicKind;
    let mut min_x: f64 = -10.16;
    let mut min_y: f64 = -7.62;
    let mut max_x: f64 = 10.16;
    let mut max_y: f64 = 7.62;
    for g in &sym.graphics {
        if let SymbolGraphicKind::Rectangle { from, to } = &g.kind {
            min_x = min_x.min(from[0]).min(to[0]) - 5.08;
            min_y = min_y.min(from[1]).min(to[1]) - 5.08;
            max_x = max_x.max(from[0]).max(to[0]) + 5.08;
            max_y = max_y.max(from[1]).max(to[1]) + 5.08;
            break;
        }
    }
    for pin in &sym.pins {
        min_x = min_x.min(pin.position[0] - 1.27);
        min_y = min_y.min(pin.position[1] - 1.27);
        max_x = max_x.max(pin.position[0] + pin.length + 1.27);
        max_y = max_y.max(pin.position[1] + 1.27);
    }
    for g in &sym.graphics {
        match &g.kind {
            SymbolGraphicKind::Rectangle { from, to } | SymbolGraphicKind::Line { from, to } => {
                min_x = min_x.min(from[0]).min(to[0]);
                min_y = min_y.min(from[1]).min(to[1]);
                max_x = max_x.max(from[0]).max(to[0]);
                max_y = max_y.max(from[1]).max(to[1]);
            }
            SymbolGraphicKind::Circle { center, radius }
            | SymbolGraphicKind::Arc { center, radius, .. } => {
                min_x = min_x.min(center[0] - radius);
                min_y = min_y.min(center[1] - radius);
                max_x = max_x.max(center[0] + radius);
                max_y = max_y.max(center[1] + radius);
            }
            SymbolGraphicKind::Text { position, size, .. } => {
                min_x = min_x.min(position[0] - size);
                min_y = min_y.min(position[1] - size);
                max_x = max_x.max(position[0] + size);
                max_y = max_y.max(position[1] + size);
            }
        }
    }
    (min_x, min_y, max_x, max_y)
}

/// Translate the pure-data [`GraphicHandleMsg`] back into the
/// canvas-side [`crate::library::editor::symbol::state::GraphicHandle`].
fn graphic_handle_msg_to_state(
    msg: GraphicHandleMsg,
) -> crate::library::editor::symbol::state::GraphicHandle {
    use crate::library::editor::symbol::state::GraphicHandle;
    match msg {
        GraphicHandleMsg::RectCorner(c) => GraphicHandle::RectCorner(c),
        GraphicHandleMsg::LineEndpoint(e) => GraphicHandle::LineEndpoint(e),
        GraphicHandleMsg::CircleRadius => GraphicHandle::CircleRadius,
        GraphicHandleMsg::ArcStart => GraphicHandle::ArcStart,
        GraphicHandleMsg::ArcEnd => GraphicHandle::ArcEnd,
        GraphicHandleMsg::TextAnchor => GraphicHandle::TextAnchor,
    }
}

/// True when a symbol-editor message mutates the document (the symbol
/// geometry / parts), so the reducer should snapshot for undo before
/// applying it. Pure-UI messages (tool, selection, pan/zoom, cursor,
/// grid, active bar) and Undo/Redo themselves return false. Mirrors
/// `mutates_footprint_state`.
fn mutates_symbol_state(msg: &PrimitiveEditorMsg) -> bool {
    use PrimitiveEditorMsg::*;
    matches!(
        msg,
        SymbolAddPin { .. }
            | SymbolAddRectangle { .. }
            | SymbolAddLine { .. }
            | SymbolAddCircle { .. }
            | SymbolAddArc { .. }
            | SymbolAddText { .. }
            | SymbolMoveSelected { .. }
            | SymbolMoveGraphicHandle { .. }
            | SymbolDeleteSelected
            | SymbolRotateSelected { .. }
            | SymbolSetPinNumber { .. }
            | SymbolSetPinName { .. }
            | SymbolNewPart
            | SymbolRemovePart
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor_with_one_symbol() -> crate::app::SymbolEditorState {
        let sym = signex_library::Symbol::empty("U1");
        let file = signex_library::SymbolFile::from_symbol(sym);
        crate::app::SymbolEditorState::new(std::path::PathBuf::from("test.snxsym"), file)
    }

    #[test]
    fn undo_and_redo_a_symbol_mutation() {
        let mut ed = editor_with_one_symbol();
        let before = ed.primitive().pins.len();

        // A mutating message snapshots first, then adds a pin.
        apply_symbol_primitive_edit(&mut ed, PrimitiveEditorMsg::SymbolAddPin { x: 1.0, y: 2.0 });
        let after_add = ed.primitive().pins.len();
        assert_eq!(after_add, before + 1, "add-pin should add one pin");
        assert_eq!(ed.history.len(), 1, "a mutation pushes one history entry");

        // Undo restores the pre-mutation state.
        apply_symbol_primitive_edit(&mut ed, PrimitiveEditorMsg::SymbolUndo);
        assert_eq!(ed.primitive().pins.len(), before, "undo removes the pin");
        assert_eq!(ed.redo.len(), 1, "undo pushes a redo entry");
        assert!(ed.history.is_empty(), "history is empty after the single undo");

        // Redo re-applies it.
        apply_symbol_primitive_edit(&mut ed, PrimitiveEditorMsg::SymbolRedo);
        assert_eq!(ed.primitive().pins.len(), after_add, "redo re-adds the pin");
    }

    #[test]
    fn undo_on_empty_history_is_a_noop() {
        let mut ed = editor_with_one_symbol();
        let before = ed.primitive().pins.len();
        apply_symbol_primitive_edit(&mut ed, PrimitiveEditorMsg::SymbolUndo);
        assert_eq!(ed.primitive().pins.len(), before);
    }

    #[test]
    fn a_new_mutation_clears_the_redo_stack() {
        let mut ed = editor_with_one_symbol();
        apply_symbol_primitive_edit(&mut ed, PrimitiveEditorMsg::SymbolAddPin { x: 1.0, y: 2.0 });
        apply_symbol_primitive_edit(&mut ed, PrimitiveEditorMsg::SymbolUndo);
        assert_eq!(ed.redo.len(), 1);
        // A fresh mutation must invalidate the redo lineage.
        apply_symbol_primitive_edit(&mut ed, PrimitiveEditorMsg::SymbolAddCircle { x: 0.0, y: 0.0 });
        assert!(ed.redo.is_empty(), "a new mutation clears redo");
    }

    #[test]
    fn pure_ui_messages_do_not_push_history() {
        let mut ed = editor_with_one_symbol();
        apply_symbol_primitive_edit(&mut ed, PrimitiveEditorMsg::SymbolDeselect);
        apply_symbol_primitive_edit(
            &mut ed,
            PrimitiveEditorMsg::SymbolSetTool(SymbolToolMsg::AddPin),
        );
        assert!(ed.history.is_empty(), "pure-UI messages must not snapshot");
    }
}
