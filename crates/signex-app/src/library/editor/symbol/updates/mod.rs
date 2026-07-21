//! Update logic for the standalone Symbol editor.
//!
//! [`apply_symbol_primitive_edit`] is a thin routing table over
//! [`SymbolEditorMsg`]: each symbol-mutating variant is dispatched to
//! the concern module that owns it — [`ui`], [`selection`], [`movement`],
//! [`transform`], [`camera`], [`parts`], or [`history`]. Graphics-placement
//! variants are handled inline. Undo/redo, drag coalescing, and the
//! shared `SymEditor` mutators live here so every concern shares one
//! implementation.

mod camera;
mod context_menu;
mod history;
mod join;
mod movement;
mod parts;
mod selection;
mod transform;
mod ui;

use camera::apply_symbol_camera;
use context_menu::apply_symbol_context_menu;
use history::apply_symbol_history;
use join::apply_symbol_join;
use movement::apply_symbol_move;
use parts::apply_symbol_parts;
use selection::apply_symbol_selection;
use transform::apply_symbol_transform;
use ui::apply_symbol_ui;

use crate::library::messages::{
    GraphicHandleMsg, SymbolContextSubmenuMsg, SymbolContextTargetMsg, SymbolEditorMsg,
    SymbolRotatePivotMsg,
};

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

/// `SymbolEditorState::status_message`'s contract is "cleared on the
/// next successful action" — enforced centrally here, once, rather
/// than at every individual mutation's success point. Clears for
/// every message that represents an attempted document/selection
/// mutation; left untouched for the continuous or chrome-only
/// messages that fire on every frame/hover and would otherwise flash
/// a just-set message away before the user can read it (camera pan/
/// zoom/cursor readout, and the context-menu / active-bar-menu
/// open/close/toggle chrome, which manage their own state and aren't
/// "actions" against the symbol itself). A message that itself fails
/// (e.g. a second bad `JoinSelectionIntoPolygon`) re-sets the message
/// right after this clear, so the net effect always reflects the
/// outcome of the most recent relevant action.
fn clear_stale_status_message(editor: &mut SymEditor, msg: &SymbolEditorMsg) {
    if !matches!(
        msg,
        SymbolEditorMsg::Pan { .. }
            | SymbolEditorMsg::Zoom { .. }
            | SymbolEditorMsg::CursorAt { .. }
            | SymbolEditorMsg::ShowContextMenu { .. }
            | SymbolEditorMsg::CloseContextMenu
            | SymbolEditorMsg::ContextMenuOpenSubmenu(_)
            | SymbolEditorMsg::ToggleActiveBarMenu(_)
            | SymbolEditorMsg::CloseActiveBarMenu
            | SymbolEditorMsg::SetSheetColor(_)
            | SymbolEditorMsg::ToggleGrid
            | SymbolEditorMsg::CycleGridSize
            | SymbolEditorMsg::CycleUnit
    ) {
        editor.status_message = None;
    }
}

/// Close any open colour picker (graphic-fill / local-colours). Call
/// whenever the selection is dropped or the graphics vector is
/// structurally mutated (delete / undo / redo / part switch) so a
/// picker keyed by a now-stale graphic index can't silently reopen on
/// an unrelated shape that happens to reuse that index.
pub(super) fn close_pickers(editor: &mut SymEditor) {
    editor.graphic_fill_picker = None;
    editor.local_color_picker = None;
}

/// Push a graphic onto the symbol, recording an undo snapshot first.
fn push_graphic(
    editor: &mut SymEditor,
    kind: signex_library::SymbolGraphicKind,
    stroke_width: f64,
) {
    push_undo(editor);
    // Phase C2: new shapes scope to the active unit so they only draw
    // on that sub-part, mirroring the render/hit-test visibility filter.
    // Legacy shared part-0 geometry from older files still draws on
    // every unit.
    let active_part = editor.active_part;
    editor
        .primitive_mut()
        .graphics
        .push(signex_library::SymbolGraphic {
            kind,
            stroke_width,
            fill: None,
            part_number: active_part,
        });
    mark_dirty(editor);
}

/// Commit `editor.polygon_vertices` (the Place Polygon click-collect
/// stash) if it holds a valid closed ring, else silently discard it.
/// Always empties the stash. Shared by the `PolygonCommit` message
/// handler (close-by-click / double-click / Enter) and the `SetTool`
/// handler's synchronous "leaving Place Polygon" flush — see
/// `apply_symbol_ui`'s `SetTool` arm for why that flush has to be
/// synchronous rather than deferred to a later event.
pub(super) fn commit_or_discard_polygon(editor: &mut SymEditor) {
    let vertices = normalize_polygon_ring(std::mem::take(&mut editor.polygon_vertices));
    if vertices.len() >= 3 {
        push_graphic(
            editor,
            signex_library::SymbolGraphicKind::Polygon { vertices },
            0.15,
        );
    } else {
        // No graphic pushed — no undo snapshot, no dirty flag; just
        // repaint so the ghost preview disappears.
        editor.canvas_cache.clear();
    }
}

/// Perpendicular-distance threshold (mm) below which a click-collected
/// vertex is treated as lying exactly on the reference line for
/// [`polygon_is_collinear`]'s degeneracy test.
const POLYGON_COLLINEAR_EPS_MM: f64 = 1e-6;

/// Normalise a click-collected vertex ring before committing it:
///
/// - Collapse consecutive epsilon-duplicate vertices (e.g. two slow
///   clicks landing on the same snapped point mid-sequence — `[P, P,
///   Q, R]` -> `[P, Q, R]`), including the wrap-around last-to-first
///   pair (a closing click landing back on vertex 0's snapped grid
///   position, which would otherwise double the closing edge at
///   render time). Mirrors `signex_library`'s chain `finalize_ring`
///   dedup pass exactly, using the same `CHAIN_ENDPOINT_EPSILON_MM`
///   constant, so this click-collect commit path and the
///   Join-into-Polygon chain path agree on what counts as "the same
///   point."
/// - Reject a degenerate ring — every vertex collinear — by returning
///   an empty Vec, which the caller's `>= 3` check then discards. A
///   `<3`-vertex input (before or after dedup) returns empty
///   unconditionally. Deliberately NOT a zero-net-shoelace-area test:
///   see [`polygon_is_collinear`]'s doc comment for why a
///   self-intersecting-but-non-collinear ring (a bowtie) must still
///   commit.
fn normalize_polygon_ring(vertices: Vec<(f64, f64)>) -> Vec<[f64; 2]> {
    let raw: Vec<[f64; 2]> = vertices.into_iter().map(|(x, y)| [x, y]).collect();
    let points = collapse_consecutive_duplicate_vertices(raw);
    if points.len() < 3 {
        return Vec::new();
    }
    if polygon_is_collinear(&points, POLYGON_COLLINEAR_EPS_MM) {
        return Vec::new();
    }
    points
}

/// Collapse consecutive epsilon-duplicate points, including the
/// wrap-around last-to-first pair — mirrors `signex_library`'s chain
/// `finalize_ring` dedup pass exactly.
fn collapse_consecutive_duplicate_vertices(raw: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    let eps_sq =
        signex_library::CHAIN_ENDPOINT_EPSILON_MM * signex_library::CHAIN_ENDPOINT_EPSILON_MM;
    let mut points: Vec<[f64; 2]> = Vec::with_capacity(raw.len());
    for p in raw {
        match points.last() {
            Some(&last) if dist_sq(last, p) <= eps_sq => {}
            _ => points.push(p),
        }
    }
    while points.len() > 1 && dist_sq(points[0], *points.last().unwrap()) <= eps_sq {
        points.pop();
    }
    points
}

fn dist_sq(a: [f64; 2], b: [f64; 2]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    dx * dx + dy * dy
}

/// `true` when every vertex in `points` lies within `eps` mm of the
/// infinite line through the first two DISTINCT vertices — mirrors
/// `signex_library`'s chain `is_collinear` (same algorithm,
/// independently implemented: it's a private helper on the other side
/// of the crate boundary). A genuinely degenerate (zero-width) ring,
/// the case this gate is documented to catch.
///
/// Deliberately NOT a zero-net-shoelace-area test (what this
/// replaced): a self-intersecting ring whose crossed lobes cancel to
/// ~zero NET area — a bowtie, e.g. `(0,0), (1.27,1.27), (1.27,0),
/// (0,1.27)` — has real 2D extent and renders even-odd; the user drew
/// it on purpose and it must commit, not be silently discarded as if
/// it were a straight line. Requires `points.len() >= 2` (guaranteed
/// by the `< 3` length gate that runs immediately before this).
fn polygon_is_collinear(points: &[[f64; 2]], eps: f64) -> bool {
    let p0 = points[0];
    let Some(p1) = points
        .iter()
        .skip(1)
        .find(|&&p| dist_sq(p, p0).sqrt() > eps)
    else {
        // Every vertex coincides with p0 within eps.
        return true;
    };
    let dir = [p1[0] - p0[0], p1[1] - p0[1]];
    let dir_len = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt();
    points.iter().all(|&p| {
        // Perpendicular distance from p to the line through p0/p1:
        // |cross(dir, p - p0)| / |dir|.
        let v = [p[0] - p0[0], p[1] - p0[1]];
        let cross = dir[0] * v[1] - dir[1] * v[0];
        (cross / dir_len).abs() <= eps
    })
}

/// Normalise an about-to-be-stored `SymbolGraphicKind::Arc`'s
/// `start_deg`/`end_deg` into this codebase's CCW-wraparound
/// convention. The three-click placement gesture's third click can
/// produce `end_deg < start_deg` two different ways: a genuinely CW
/// drag (the cursor moved backwards past the start angle, so the
/// unwrapped tracked end angle went negative relative to start — e.g.
/// `start: 30, end: -60`), or simply because `start_deg` is a raw
/// `atan2` result while `end_deg` is separately unwrapped and the two
/// never got reconciled into a common `[0, 360)` frame. Either way,
/// under the CCW-wraparound rule `end_deg < start_deg` is read as
/// "the long way around" — the opposite of the short arc the
/// placement preview showed (see `draw_arc_preview`, which builds its
/// ghost from the exact same CCW-wraparound sweep this function's
/// callers store).
///
/// A thin wrapper over `signex_library::normalize_arc_endpoints_deg`
/// — that function's doc comment has the full swap-vs-`rem_euclid`
/// rationale, shared verbatim with `SymbolFile::from_toml_str`'s
/// legacy-arc load migration (which reuses the exact same function
/// rather than duplicating this formula a third time; signex-library
/// must not depend on signex-app, so the shared implementation lives
/// there and this crate calls into it, not the reverse).
pub(super) fn normalize_arc_commit_deg(start_deg: f64, end_deg: f64) -> (f64, f64) {
    signex_library::normalize_arc_endpoints_deg(start_deg, end_deg)
}

/// Apply a primitive-editor event to a standalone Symbol editor
/// state. Mirrors the symbol-tab arms of `apply_inline_edit` but
/// against the path-keyed standalone state. Visibility is
/// `pub(crate)` so unit tests in sibling modules can drive the editor
/// through the same code path the dispatcher uses.
pub(crate) fn apply_symbol_primitive_edit(
    editor: &mut crate::app::SymbolEditorState,
    msg: SymbolEditorMsg,
) {
    use crate::library::editor::symbol::state::SymbolSelection;

    clear_stale_status_message(editor, &msg);

    match msg {
        // ── UI / toolbar (no undo — except SetTool's synchronous
        // polygon flush, which commits a >=3-vertex stash through
        // push_graphic and therefore pushes one undo snapshot) ────
        SymbolEditorMsg::SetTool(_)
        | SymbolEditorMsg::ToggleActiveBarMenu(_)
        | SymbolEditorMsg::CloseActiveBarMenu
        | SymbolEditorMsg::ActiveBarStub(_)
        | SymbolEditorMsg::ToggleSelectionFilter(_) => apply_symbol_ui(editor, msg),

        // ── Graphics placement ───────────────────────────────────
        SymbolEditorMsg::AddPin { x, y } => {
            push_undo(editor);
            let active_part = editor.active_part;
            let idx = crate::library::editor::symbol::state::add_pin(
                editor.primitive_mut(),
                x,
                y,
                active_part,
            );
            editor.selected = Some(SymbolSelection::Pin(idx));
            mark_dirty(editor);
        }
        SymbolEditorMsg::AddRectangle {
            from_x,
            from_y,
            to_x,
            to_y,
        } => {
            // Normalize the two clicked corners so `from` is the
            // bottom-left (min) and `to` is the top-right (max),
            // regardless of which direction the user dragged.
            push_graphic(
                editor,
                signex_library::SymbolGraphicKind::Rectangle {
                    from: [from_x.min(to_x), from_y.min(to_y)],
                    to: [from_x.max(to_x), from_y.max(to_y)],
                },
                0.15,
            );
        }
        SymbolEditorMsg::AddLine {
            from_x,
            from_y,
            to_x,
            to_y,
        } => {
            push_graphic(
                editor,
                signex_library::SymbolGraphicKind::Line {
                    from: [from_x, from_y],
                    to: [to_x, to_y],
                },
                0.15,
            );
        }
        SymbolEditorMsg::AddArc {
            cx,
            cy,
            radius,
            start_deg,
            end_deg,
        } => {
            let (start_deg, end_deg) = normalize_arc_commit_deg(start_deg, end_deg);
            push_graphic(
                editor,
                signex_library::SymbolGraphicKind::Arc {
                    center: [cx, cy],
                    radius,
                    start_deg,
                    end_deg,
                },
                0.15,
            );
        }
        // A >= 360° drag was rejected at the gesture level (see
        // `arc_sweep_exceeds_full_turn`) rather than committed — no
        // graphic, no undo snapshot; just surface why.
        SymbolEditorMsg::ArcSweepRejected => {
            editor.status_message = Some("Arc sweep must be less than a full turn.".to_string());
        }
        SymbolEditorMsg::AddText { x, y } => {
            push_graphic(
                editor,
                signex_library::SymbolGraphicKind::Text {
                    position: [x, y],
                    content: "Text".to_string(),
                    size: 1.27,
                },
                0.0,
            );
        }
        SymbolEditorMsg::AddCircle { cx, cy, radius } => {
            push_graphic(
                editor,
                signex_library::SymbolGraphicKind::Circle {
                    center: [cx, cy],
                    radius,
                },
                0.15,
            );
        }
        SymbolEditorMsg::PolygonClick { x, y } => {
            editor.polygon_vertices.push((x, y));
            editor.canvas_cache.clear();
        }
        SymbolEditorMsg::PolygonCommit => commit_or_discard_polygon(editor),
        SymbolEditorMsg::PolygonCancel => {
            editor.polygon_vertices.clear();
            editor.canvas_cache.clear();
        }
        SymbolEditorMsg::JoinSelectionIntoPolygon => apply_symbol_join(editor, msg),

        // ── Selection ────────────────────────────────────────────
        SymbolEditorMsg::Select(_) | SymbolEditorMsg::Deselect => {
            apply_symbol_selection(editor, msg)
        }

        // ── Move (coalesced undo per drag gesture) ───────────────
        SymbolEditorMsg::MoveSelected { .. }
        | SymbolEditorMsg::MoveAll { .. }
        | SymbolEditorMsg::MoveGraphicHandle { .. } => apply_symbol_move(editor, msg),

        // ── Transform ────────────────────────────────────────────
        SymbolEditorMsg::RotateSelected { .. }
        | SymbolEditorMsg::DeleteSelected
        | SymbolEditorMsg::AlignSelectedToGrid
        | SymbolEditorMsg::SetPinNumber { .. }
        | SymbolEditorMsg::SetPinName { .. } => apply_symbol_transform(editor, msg),

        // ── Camera / viewport (no undo) ──────────────────────────
        SymbolEditorMsg::Pan { .. }
        | SymbolEditorMsg::Zoom { .. }
        | SymbolEditorMsg::Fit
        | SymbolEditorMsg::CursorAt { .. } => apply_symbol_camera(editor, msg),

        // ── Display settings intercepted upstream; no-op here ────
        SymbolEditorMsg::SetSheetColor(_)
        | SymbolEditorMsg::ToggleGrid
        | SymbolEditorMsg::CycleGridSize
        | SymbolEditorMsg::CycleUnit => {}

        // ── Multi-part management ────────────────────────────────
        SymbolEditorMsg::PrevPart
        | SymbolEditorMsg::NextPart
        | SymbolEditorMsg::NewPart
        | SymbolEditorMsg::RemovePart => apply_symbol_parts(editor, msg),

        // ── Undo / redo / drag-commit ────────────────────────────
        SymbolEditorMsg::Undo | SymbolEditorMsg::Redo | SymbolEditorMsg::DragCommit => {
            apply_symbol_history(editor, msg)
        }

        // ── Right-click context menu ─────────────────────────────
        SymbolEditorMsg::ShowContextMenu { .. }
        | SymbolEditorMsg::CloseContextMenu
        | SymbolEditorMsg::ContextMenuOpenSubmenu(_) => apply_symbol_context_menu(editor, msg),

        // A menu row's real action: apply it (recursing back into this
        // same dispatcher) then close the popover — the "any click on
        // a real action closes the menu" behaviour every row wants,
        // expressed once instead of per-row.
        SymbolEditorMsg::ContextMenuAction(inner) => {
            editor.context_menu = None;
            apply_symbol_primitive_edit(editor, *inner);
        }
    }
}

/// Translate the pure-data [`SymbolContextTargetMsg`] into the
/// canvas/state-side [`crate::library::editor::symbol::state::SymbolContextTarget`].
fn context_target_msg_to_state(
    msg: SymbolContextTargetMsg,
) -> crate::library::editor::symbol::state::SymbolContextTarget {
    use crate::library::editor::symbol::state::SymbolContextTarget;
    match msg {
        SymbolContextTargetMsg::Empty => SymbolContextTarget::Empty,
        SymbolContextTargetMsg::Pin(idx) => SymbolContextTarget::Pin(idx),
        SymbolContextTargetMsg::Graphic(idx) => SymbolContextTarget::Graphic(idx),
    }
}

/// Translate the pure-data [`SymbolContextSubmenuMsg`] into the
/// canvas/state-side [`crate::library::editor::symbol::state::SymbolContextSubmenu`].
fn context_submenu_msg_to_state(
    msg: SymbolContextSubmenuMsg,
) -> crate::library::editor::symbol::state::SymbolContextSubmenu {
    use crate::library::editor::symbol::state::SymbolContextSubmenu;
    match msg {
        SymbolContextSubmenuMsg::Place => SymbolContextSubmenu::Place,
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
            SymbolGraphicKind::Polygon { vertices } => {
                for v in vertices {
                    include_rect(&mut bounds, v[0], v[1], v[0], v[1]);
                }
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
        GraphicHandleMsg::PolygonVertex(i) => GraphicHandle::PolygonVertex(i),
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

#[cfg(test)]
mod tests {
    use super::*;
    use signex_library::{Symbol, SymbolFile, SymbolGraphicKind};
    use std::path::PathBuf;

    fn new_editor() -> SymEditor {
        SymEditor::new(
            PathBuf::from("t.snxsym"),
            SymbolFile::from_symbol(Symbol::empty("T")),
        )
    }

    /// A stale status message (e.g. left over from a failed
    /// `JoinSelectionIntoPolygon`) is cleared by the very next
    /// mutating message — here `DeleteSelected` on an empty
    /// selection, itself a no-op — so it never lingers past the
    /// action it described. Contract lives on
    /// `SymbolEditorState::status_message`'s doc comment.
    #[test]
    fn stale_status_message_clears_on_next_mutating_message() {
        let mut editor = new_editor();
        editor.status_message = Some("stale".to_string());

        apply_symbol_primitive_edit(&mut editor, SymbolEditorMsg::DeleteSelected);

        assert!(editor.status_message.is_none());
    }

    /// Continuous/chrome-only messages (camera pan here) must NOT
    /// clear a just-set status message before the user can read it.
    #[test]
    fn status_message_survives_camera_pan() {
        let mut editor = new_editor();
        editor.status_message = Some("keep me".to_string());

        apply_symbol_primitive_edit(&mut editor, SymbolEditorMsg::Pan { dx: 1.0, dy: 1.0 });

        assert_eq!(editor.status_message.as_deref(), Some("keep me"));
    }

    /// Three `PolygonClick`s then `PolygonCommit` push exactly one
    /// graphic and exactly one undo snapshot — mirrors what every
    /// close gesture (click-on-first-vertex / double-click / Enter)
    /// collapses to from the dispatcher's point of view.
    #[test]
    fn polygon_click_then_commit_pushes_one_graphic_and_one_undo_entry() {
        let mut editor = new_editor();
        apply_symbol_primitive_edit(
            &mut editor,
            SymbolEditorMsg::PolygonClick { x: 0.0, y: 0.0 },
        );
        apply_symbol_primitive_edit(
            &mut editor,
            SymbolEditorMsg::PolygonClick { x: 4.0, y: 0.0 },
        );
        apply_symbol_primitive_edit(
            &mut editor,
            SymbolEditorMsg::PolygonClick { x: 2.0, y: 3.0 },
        );
        assert_eq!(
            editor.undo_snapshots.len(),
            0,
            "clicks alone don't push undo"
        );

        apply_symbol_primitive_edit(&mut editor, SymbolEditorMsg::PolygonCommit);

        assert_eq!(editor.primitive().graphics.len(), 1);
        assert_eq!(
            editor.undo_snapshots.len(),
            1,
            "commit pushes exactly one undo snapshot"
        );
        assert!(
            editor.polygon_vertices.is_empty(),
            "stash is emptied on commit"
        );
        match &editor.primitive().graphics[0].kind {
            SymbolGraphicKind::Polygon { vertices } => {
                assert_eq!(vertices, &vec![[0.0, 0.0], [4.0, 0.0], [2.0, 3.0]]);
            }
            other => panic!("expected Polygon, got {other:?}"),
        }
    }

    /// Fewer than 3 collected vertices — `PolygonCommit` is a silent
    /// discard: no graphic, no undo snapshot, stash still clears.
    #[test]
    fn polygon_commit_with_fewer_than_three_vertices_is_discarded() {
        let mut editor = new_editor();
        apply_symbol_primitive_edit(
            &mut editor,
            SymbolEditorMsg::PolygonClick { x: 0.0, y: 0.0 },
        );
        apply_symbol_primitive_edit(
            &mut editor,
            SymbolEditorMsg::PolygonClick { x: 4.0, y: 0.0 },
        );
        apply_symbol_primitive_edit(&mut editor, SymbolEditorMsg::PolygonCommit);

        assert!(editor.primitive().graphics.is_empty());
        assert_eq!(editor.undo_snapshots.len(), 0);
        assert!(editor.polygon_vertices.is_empty());
    }

    /// A degenerate (collinear, zero-area) ring is discarded even
    /// with >= 3 vertices.
    #[test]
    fn polygon_commit_with_collinear_vertices_is_discarded() {
        let mut editor = new_editor();
        apply_symbol_primitive_edit(
            &mut editor,
            SymbolEditorMsg::PolygonClick { x: 0.0, y: 0.0 },
        );
        apply_symbol_primitive_edit(
            &mut editor,
            SymbolEditorMsg::PolygonClick { x: 1.0, y: 1.0 },
        );
        apply_symbol_primitive_edit(
            &mut editor,
            SymbolEditorMsg::PolygonClick { x: 2.0, y: 2.0 },
        );
        apply_symbol_primitive_edit(&mut editor, SymbolEditorMsg::PolygonCommit);

        assert!(editor.primitive().graphics.is_empty());
        assert_eq!(editor.undo_snapshots.len(), 0);
    }

    /// A trailing vertex equal to the first (a plain click landed
    /// exactly on vertex 0's snapped grid position without triggering
    /// the tolerance-based close gesture) is dropped before
    /// committing, so the ring doesn't double its closing edge.
    #[test]
    fn polygon_commit_drops_duplicate_closing_vertex() {
        let mut editor = new_editor();
        apply_symbol_primitive_edit(
            &mut editor,
            SymbolEditorMsg::PolygonClick { x: 0.0, y: 0.0 },
        );
        apply_symbol_primitive_edit(
            &mut editor,
            SymbolEditorMsg::PolygonClick { x: 4.0, y: 0.0 },
        );
        apply_symbol_primitive_edit(
            &mut editor,
            SymbolEditorMsg::PolygonClick { x: 2.0, y: 3.0 },
        );
        apply_symbol_primitive_edit(
            &mut editor,
            SymbolEditorMsg::PolygonClick { x: 0.0, y: 0.0 },
        );
        apply_symbol_primitive_edit(&mut editor, SymbolEditorMsg::PolygonCommit);

        match &editor.primitive().graphics[0].kind {
            SymbolGraphicKind::Polygon { vertices } => {
                assert_eq!(vertices.len(), 3, "duplicate closing vertex dropped");
                assert_eq!(vertices, &vec![[0.0, 0.0], [4.0, 0.0], [2.0, 3.0]]);
            }
            other => panic!("expected Polygon, got {other:?}"),
        }
    }

    /// A self-intersecting bowtie whose crossed lobes cancel to
    /// exactly zero NET shoelace area still commits — it has real 2D
    /// extent and renders even-odd, unlike a genuinely collinear
    /// (zero-width) ring.
    #[test]
    fn polygon_commit_with_self_intersecting_bowtie_commits() {
        let mut editor = new_editor();
        for (x, y) in [(0.0, 0.0), (1.27, 1.27), (1.27, 0.0), (0.0, 1.27)] {
            apply_symbol_primitive_edit(&mut editor, SymbolEditorMsg::PolygonClick { x, y });
        }
        apply_symbol_primitive_edit(&mut editor, SymbolEditorMsg::PolygonCommit);

        assert_eq!(editor.primitive().graphics.len(), 1);
        match &editor.primitive().graphics[0].kind {
            SymbolGraphicKind::Polygon { vertices } => assert_eq!(vertices.len(), 4),
            other => panic!("expected Polygon, got {other:?}"),
        }
    }

    /// Two slow clicks landing on the same snapped point mid-sequence
    /// collapse into one vertex — `[P, P, Q, R]` -> `[P, Q, R]` —
    /// mirroring `signex_library`'s chain `finalize_ring` dedup pass.
    #[test]
    fn polygon_commit_collapses_a_consecutive_duplicate_mid_sequence() {
        let mut editor = new_editor();
        for (x, y) in [(0.0, 0.0), (0.0, 0.0), (4.0, 0.0), (2.0, 3.0)] {
            apply_symbol_primitive_edit(&mut editor, SymbolEditorMsg::PolygonClick { x, y });
        }
        apply_symbol_primitive_edit(&mut editor, SymbolEditorMsg::PolygonCommit);

        match &editor.primitive().graphics[0].kind {
            SymbolGraphicKind::Polygon { vertices } => {
                assert_eq!(vertices, &vec![[0.0, 0.0], [4.0, 0.0], [2.0, 3.0]]);
            }
            other => panic!("expected Polygon, got {other:?}"),
        }
    }

    /// `PolygonCancel` discards the stash with no commit, regardless
    /// of vertex count.
    #[test]
    fn polygon_cancel_discards_without_committing() {
        let mut editor = new_editor();
        apply_symbol_primitive_edit(
            &mut editor,
            SymbolEditorMsg::PolygonClick { x: 0.0, y: 0.0 },
        );
        apply_symbol_primitive_edit(
            &mut editor,
            SymbolEditorMsg::PolygonClick { x: 4.0, y: 0.0 },
        );
        apply_symbol_primitive_edit(
            &mut editor,
            SymbolEditorMsg::PolygonClick { x: 2.0, y: 3.0 },
        );
        apply_symbol_primitive_edit(&mut editor, SymbolEditorMsg::PolygonCancel);

        assert!(editor.primitive().graphics.is_empty());
        assert_eq!(editor.undo_snapshots.len(), 0);
        assert!(editor.polygon_vertices.is_empty());
    }

    /// `normalize_arc_commit_deg` swaps a CW-dragged pair (`end <
    /// start`) so the stored endpoints represent the same short arc
    /// under the CCW-wraparound convention, instead of preserving the
    /// (wrong) long-way-around sweep a per-field `rem_euclid` would.
    #[test]
    fn normalize_arc_commit_deg_swaps_a_cw_dragged_pair() {
        // The task's concrete repro: dragging CW past the start angle
        // leaves the unwrapped tracker negative.
        assert_eq!(normalize_arc_commit_deg(30.0, -60.0), (300.0, 30.0));
    }

    /// An already-CCW (non-wrapped) pair is untouched beyond the
    /// canonicalising `rem_euclid` — no swap, matching the "already
    /// correct" arcs this whole pass leaves unchanged.
    #[test]
    fn normalize_arc_commit_deg_leaves_ccw_pairs_unswapped() {
        assert_eq!(normalize_arc_commit_deg(10.0, 100.0), (10.0, 100.0));
    }

    /// `AddArc` commits a CW-dragged placement (`end_deg < start_deg`)
    /// with its endpoints swapped, so the graphic that lands in
    /// `Symbol::graphics` — not just the intermediate helper — stores
    /// the CCW-wraparound form of the short arc the preview showed.
    #[test]
    fn add_arc_commit_stores_swapped_endpoints_for_a_cw_drag() {
        let mut editor = new_editor();
        apply_symbol_primitive_edit(
            &mut editor,
            SymbolEditorMsg::AddArc {
                cx: 0.0,
                cy: 0.0,
                radius: 5.0,
                start_deg: 30.0,
                end_deg: -60.0,
            },
        );

        assert_eq!(editor.primitive().graphics.len(), 1);
        match &editor.primitive().graphics[0].kind {
            SymbolGraphicKind::Arc {
                start_deg,
                end_deg,
                radius,
                ..
            } => {
                assert_eq!(*start_deg, 300.0);
                assert_eq!(*end_deg, 30.0);
                assert_eq!(*radius, 5.0);
            }
            other => panic!("expected Arc, got {other:?}"),
        }
    }

    /// `ArcSweepRejected` surfaces a status message and commits
    /// nothing — no graphic, no undo snapshot. The gesture-level
    /// "third click ignored" behavior lives in `canvas::input::tools`
    /// (see `arc_sweep_exceeds_full_turn`'s tests); this covers the
    /// message-dispatch half of the fix.
    #[test]
    fn arc_sweep_rejected_sets_status_message_without_committing() {
        let mut editor = new_editor();
        apply_symbol_primitive_edit(&mut editor, SymbolEditorMsg::ArcSweepRejected);

        assert_eq!(
            editor.status_message.as_deref(),
            Some("Arc sweep must be less than a full turn.")
        );
        assert!(editor.primitive().graphics.is_empty());
        assert_eq!(editor.undo_snapshots.len(), 0);
    }
}
