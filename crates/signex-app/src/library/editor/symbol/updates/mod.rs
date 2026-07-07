//! Update logic for the standalone Symbol editor.
//!
//! [`apply_symbol_primitive_edit`] is a thin routing table over
//! [`PrimitiveEditorMsg`]: each symbol-mutating variant is dispatched to
//! the concern module that owns it — [`ui`], [`selection`], [`movement`],
//! [`transform`], [`camera`], [`parts`], or [`history`]. Graphics-placement
//! variants are handled inline. Undo/redo, drag coalescing, and the
//! shared `SymEditor` mutators live here so every concern shares one
//! implementation.

mod camera;
mod history;
mod movement;
mod parts;
mod selection;
mod transform;
mod ui;

use camera::apply_symbol_camera;
use history::apply_symbol_history;
use movement::apply_symbol_move;
use parts::apply_symbol_parts;
use selection::apply_symbol_selection;
use transform::apply_symbol_transform;
use ui::apply_symbol_ui;

use crate::library::messages::{GraphicHandleMsg, PrimitiveEditorMsg, SymbolRotatePivotMsg};

type SymEditor = crate::app::SymbolEditorState;

/// Push a full snapshot onto the undo stack; clear the redo stack.
/// Capped at 100 entries — oldest entry is evicted when the cap is hit.
fn push_undo(editor: &mut SymEditor) {
    let snapshot = editor.primitive().clone();
    editor.undo_snapshots.push(snapshot);
    if editor.undo_snapshots.len() > 100 {
        editor.undo_snapshots.remove(0);
    }
    editor.redo_snapshots.clear();
}

/// Record the first event of a drag gesture.
/// Subsequent events in the same drag are no-ops (mid_drag stays true).
fn begin_drag_if_needed(editor: &mut SymEditor) {
    if !editor.mid_drag {
        push_undo(editor);
        editor.mid_drag = true;
    }
}

/// Mark the symbol as dirty and invalidate the canvas cache.
fn mark_dirty(editor: &mut SymEditor) {
    editor.dirty = true;
    editor.canvas_cache.clear();
}

/// Push a graphic onto the symbol, recording an undo snapshot first.
fn push_graphic(
    editor: &mut SymEditor,
    kind: signex_library::SymbolGraphicKind,
    stroke_width: f64,
) {
    push_undo(editor);
    editor.primitive_mut().graphics.push(signex_library::SymbolGraphic { kind, stroke_width });
    mark_dirty(editor);
}

/// Apply a primitive-editor event to a standalone Symbol editor
/// state. Mirrors the symbol-tab arms of `apply_inline_edit` but
/// against the path-keyed standalone state. Visibility is
/// `pub(crate)` so unit tests in sibling modules can drive the editor
/// through the same code path the dispatcher uses.
pub(crate) fn apply_symbol_primitive_edit(
    editor: &mut crate::app::SymbolEditorState,
    msg: PrimitiveEditorMsg,
) {
    use crate::library::editor::symbol::state::SymbolSelection;

    match msg {
        // ── UI / toolbar (no undo) ───────────────────────────────
        PrimitiveEditorMsg::SymbolSetTool(_)
        | PrimitiveEditorMsg::SymbolToggleActiveBarMenu(_)
        | PrimitiveEditorMsg::SymbolCloseActiveBarMenu
        | PrimitiveEditorMsg::SymbolActiveBarStub(_)
        | PrimitiveEditorMsg::SymbolToggleSelectionFilter(_) => apply_symbol_ui(editor, msg),

        // ── Graphics placement ───────────────────────────────────
        PrimitiveEditorMsg::SymbolAddPin { x, y } => {
            push_undo(editor);
            let active_part = editor.active_part;
            let idx = crate::library::editor::symbol::state::add_pin(
                editor.primitive_mut(), x, y, active_part,
            );
            editor.selected = Some(SymbolSelection::Pin(idx));
            mark_dirty(editor);
        }
        PrimitiveEditorMsg::SymbolAddRectangle { x, y } => {
            const W: f64 = 5.08;
            const H: f64 = 2.54;
            push_graphic(editor, signex_library::SymbolGraphicKind::Rectangle {
                from: [x - W, y - H],
                to: [x + W, y + H],
            }, 0.15);
        }
        PrimitiveEditorMsg::SymbolAddLine { from_x, from_y, to_x, to_y } => {
            push_graphic(editor, signex_library::SymbolGraphicKind::Line {
                from: [from_x, from_y],
                to: [to_x, to_y],
            }, 0.15);
        }
        PrimitiveEditorMsg::SymbolAddArc { cx, cy, radius, start_deg, end_deg } => {
            push_graphic(editor, signex_library::SymbolGraphicKind::Arc {
                center: [cx, cy], radius, start_deg, end_deg,
            }, 0.15);
        }
        PrimitiveEditorMsg::SymbolAddText { x, y } => {
            push_graphic(editor, signex_library::SymbolGraphicKind::Text {
                position: [x, y],
                content: "Text".to_string(),
                size: 1.27,
            }, 0.0);
        }
        PrimitiveEditorMsg::SymbolAddCircle { cx, cy, radius } => {
            push_graphic(editor, signex_library::SymbolGraphicKind::Circle {
                center: [cx, cy], radius,
            }, 0.15);
        }

        // ── Selection ────────────────────────────────────────────
        PrimitiveEditorMsg::SymbolSelect(_)
        | PrimitiveEditorMsg::SymbolDeselect => apply_symbol_selection(editor, msg),

        // ── Move (coalesced undo per drag gesture) ───────────────
        PrimitiveEditorMsg::SymbolMoveSelected { .. }
        | PrimitiveEditorMsg::SymbolMoveAll { .. }
        | PrimitiveEditorMsg::SymbolMoveGraphicHandle { .. } => apply_symbol_move(editor, msg),

        // ── Transform ────────────────────────────────────────────
        PrimitiveEditorMsg::SymbolRotateSelected { .. }
        | PrimitiveEditorMsg::SymbolDeleteSelected
        | PrimitiveEditorMsg::SymbolSetPinNumber { .. }
        | PrimitiveEditorMsg::SymbolSetPinName { .. } => apply_symbol_transform(editor, msg),

        // ── Camera / viewport (no undo) ──────────────────────────
        PrimitiveEditorMsg::SymbolPan { .. }
        | PrimitiveEditorMsg::SymbolZoom { .. }
        | PrimitiveEditorMsg::SymbolFit
        | PrimitiveEditorMsg::SymbolCursorAt { .. } => apply_symbol_camera(editor, msg),

        // ── Display settings intercepted upstream; no-op here ────
        PrimitiveEditorMsg::SymbolSetSheetColor(_)
        | PrimitiveEditorMsg::SymbolToggleGrid
        | PrimitiveEditorMsg::SymbolCycleGridSize
        | PrimitiveEditorMsg::SymbolCycleUnit => {}

        // ── Multi-part management ────────────────────────────────
        PrimitiveEditorMsg::SymbolPrevPart
        | PrimitiveEditorMsg::SymbolNextPart
        | PrimitiveEditorMsg::SymbolNewPart
        | PrimitiveEditorMsg::SymbolRemovePart => apply_symbol_parts(editor, msg),

        // ── Undo / redo / drag-commit ────────────────────────────
        PrimitiveEditorMsg::SymbolUndo
        | PrimitiveEditorMsg::SymbolRedo
        | PrimitiveEditorMsg::SymbolDragCommit => apply_symbol_history(editor, msg),

        // Footprint messages are no-ops in the symbol editor.
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
        | PrimitiveEditorMsg::FootprintSketchAddConstraintForSelection(_)
        | PrimitiveEditorMsg::FootprintSketchDimensionInput(_)
        | PrimitiveEditorMsg::FootprintSketchSetRole { .. }
        | PrimitiveEditorMsg::FootprintSketchMakePadFromProfile
        | PrimitiveEditorMsg::FootprintSketchUnlinkCornerRadius { .. }
        | PrimitiveEditorMsg::FootprintAddTextFrame { .. }
        | PrimitiveEditorMsg::FootprintSelectPads(..)
        | PrimitiveEditorMsg::FootprintSketchSelectMany(..)
        | PrimitiveEditorMsg::FootprintShowContextMenu { .. }
        | PrimitiveEditorMsg::FootprintCloseContextMenu
        | PrimitiveEditorMsg::FootprintContextMenuOpenSubmenu(..)
        | PrimitiveEditorMsg::FootprintContextMenuAction(..)
        | PrimitiveEditorMsg::FootprintFitConsumed
        | PrimitiveEditorMsg::FootprintCopyPad
        | PrimitiveEditorMsg::FootprintCutPad
        | PrimitiveEditorMsg::FootprintPastePad
        | PrimitiveEditorMsg::FootprintApplyFilterPreset(..)
        | PrimitiveEditorMsg::FootprintToggleAllFilters
        | PrimitiveEditorMsg::FootprintCaptureFilterPreset
        | PrimitiveEditorMsg::FootprintActiveBarNudgeSelection
        | PrimitiveEditorMsg::FootprintMoveByOpen
        | PrimitiveEditorMsg::FootprintMoveBySetX(..)
        | PrimitiveEditorMsg::FootprintMoveBySetY(..)
        | PrimitiveEditorMsg::FootprintMoveByConfirm
        | PrimitiveEditorMsg::FootprintMoveByCancel
        | PrimitiveEditorMsg::FootprintMintBody3d
        | PrimitiveEditorMsg::FootprintMintExtrudedBody3d
        | PrimitiveEditorMsg::FootprintAlignPads(..)
        | PrimitiveEditorMsg::FootprintSketchPlacementInputTab
        | PrimitiveEditorMsg::FootprintSketchMoveLine { .. }
        | PrimitiveEditorMsg::FootprintSketchResizeRoundPad { .. }
        | PrimitiveEditorMsg::FootprintSetSelectionMode2d(..)
        | PrimitiveEditorMsg::FootprintSelectAllOnLayer
        | PrimitiveEditorMsg::FootprintAddVia { .. }
        | PrimitiveEditorMsg::FootprintRecomputeCourtyardOutline
        | PrimitiveEditorMsg::FootprintSelectOffGridPads
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
        | PrimitiveEditorMsg::Save => {}
    }
}

/// World-space bbox covering the symbol's body + every pin + every
/// graphic. Used by `SymbolFit` so the dispatcher can compute a
/// `Camera::fit_rect` against the active symbol without reaching
/// into the canvas program. Matches the `SymbolCanvas::bbox` shape
/// so click-Fit and Home key produce the same viewport.
fn symbol_bbox(sym: &signex_library::Symbol) -> (f64, f64, f64, f64) {
    use signex_library::SymbolGraphicKind;
    let mut bounds: Option<(f64, f64, f64, f64)> = None;
    let include_rect =
        |bounds: &mut Option<(f64, f64, f64, f64)>, x0: f64, y0: f64, x1: f64, y1: f64| {
            let rx0 = x0.min(x1);
            let ry0 = y0.min(y1);
            let rx1 = x0.max(x1);
            let ry1 = y0.max(y1);
            if let Some((min_x, min_y, max_x, max_y)) = bounds.as_mut() {
                *min_x = (*min_x).min(rx0);
                *min_y = (*min_y).min(ry0);
                *max_x = (*max_x).max(rx1);
                *max_y = (*max_y).max(ry1);
            } else {
                *bounds = Some((rx0, ry0, rx1, ry1));
            }
        };

    for g in &sym.graphics {
        if let SymbolGraphicKind::Rectangle { from, to } = &g.kind {
            include_rect(
                &mut bounds,
                from[0].min(to[0]) - 5.08,
                from[1].min(to[1]) - 5.08,
                from[0].max(to[0]) + 5.08,
                from[1].max(to[1]) + 5.08,
            );
            break;
        }
    }

    for pin in &sym.pins {
        include_rect(
            &mut bounds,
            pin.position[0] - 1.27,
            pin.position[1] - 1.27,
            pin.position[0] + pin.length + 1.27,
            pin.position[1] + 1.27,
        );
    }

    for g in &sym.graphics {
        match &g.kind {
            SymbolGraphicKind::Rectangle { from, to } | SymbolGraphicKind::Line { from, to } => {
                include_rect(&mut bounds, from[0], from[1], to[0], to[1]);
            }
            SymbolGraphicKind::Circle { center, radius }
            | SymbolGraphicKind::Arc { center, radius, .. } => {
                include_rect(
                    &mut bounds,
                    center[0] - radius,
                    center[1] - radius,
                    center[0] + radius,
                    center[1] + radius,
                );
            }
            SymbolGraphicKind::Text { position, size, .. } => {
                include_rect(
                    &mut bounds,
                    position[0] - size,
                    position[1] - size,
                    position[0] + size,
                    position[1] + size,
                );
            }
        }
    }

    bounds.unwrap_or((-1.27, -1.27, 1.27, 1.27))
}

/// Translate the pure-data [`GraphicHandleMsg`] back into the
/// canvas-side [`crate::library::editor::symbol::state::GraphicHandle`].
fn graphic_handle_msg_to_state(
    msg: GraphicHandleMsg,
) -> crate::library::editor::symbol::state::GraphicHandle {
    use crate::library::editor::symbol::state::GraphicHandle;
    match msg {
        GraphicHandleMsg::RectCorner(c) => GraphicHandle::RectCorner(c),
        GraphicHandleMsg::RectEdge(e) => GraphicHandle::RectEdge(e),
        GraphicHandleMsg::LineEndpoint(e) => GraphicHandle::LineEndpoint(e),
        GraphicHandleMsg::CircleRadius => GraphicHandle::CircleRadius,
        GraphicHandleMsg::ArcStart => GraphicHandle::ArcStart,
        GraphicHandleMsg::ArcEnd => GraphicHandle::ArcEnd,
        GraphicHandleMsg::TextAnchor => GraphicHandle::TextAnchor,
    }
}

/// Translate pure-data rotate pivot messages into Symbol-state pivot mode.
fn rotate_pivot_msg_to_state(
    msg: SymbolRotatePivotMsg,
) -> crate::library::editor::symbol::state::GraphicRotationPivotMode {
    use crate::library::editor::symbol::state::GraphicRotationPivotMode;
    match msg {
        SymbolRotatePivotMsg::WorldOrigin => GraphicRotationPivotMode::WorldOrigin,
        SymbolRotatePivotMsg::GeometryCenter => GraphicRotationPivotMode::GeometryCenter,
    }
}
