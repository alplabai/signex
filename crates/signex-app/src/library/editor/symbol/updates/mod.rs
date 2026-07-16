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

use crate::library::messages::{GraphicHandleMsg, SymbolEditorMsg, SymbolRotatePivotMsg};

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

/// Normalise a click-collected vertex ring before committing it:
///
/// - Drop a trailing vertex that duplicates the first. A closing
///   click is meant to be a "connect back to vertex 0" gesture, not a
///   new point — the click-collect gesture handlers already avoid
///   appending one for the tolerance-based and double-click closes,
///   but a plain click can still land exactly on vertex 0's snapped
///   grid position (a fine snap grid + click slightly outside the
///   gesture-1 hit tolerance is enough), which would otherwise double
///   the closing edge at render time.
/// - Reject a degenerate ring — collinear vertices / zero enclosed
///   area — by returning an empty Vec, which the caller's `>= 3`
///   check then discards. A `<3`-vertex input returns empty
///   unconditionally.
fn normalize_polygon_ring(mut vertices: Vec<(f64, f64)>) -> Vec<[f64; 2]> {
    if vertices.len() >= 2 && vertices.first() == vertices.last() {
        vertices.pop();
    }
    if vertices.len() < 3 {
        return Vec::new();
    }
    let points: Vec<[f64; 2]> = vertices.into_iter().map(|(x, y)| [x, y]).collect();
    if polygon_signed_area2(&points).abs() <= 1e-9 {
        return Vec::new();
    }
    points
}

/// Twice the shoelace-formula signed area of a closed (implicitly)
/// vertex ring. Zero (within epsilon) means every vertex is
/// collinear — a degenerate, invisible "polygon".
fn polygon_signed_area2(vertices: &[[f64; 2]]) -> f64 {
    let n = vertices.len();
    (0..n)
        .map(|i| {
            let a = vertices[i];
            let b = vertices[(i + 1) % n];
            a[0] * b[1] - b[0] * a[1]
        })
        .sum()
}

/// Normalise an about-to-be-stored `SymbolGraphicKind::Arc`'s
/// `start_deg`/`end_deg` into this codebase's CCW-wraparound
/// convention — see `signex_gfx::primitive::arc::ccw_wrapped_sweep_
/// rad`'s doc comment for the full rule (`hit_test.rs`'s `Arc` arm,
/// `rotation.rs`'s Arc rotate arm, and `arc.wgsl`'s `sdf_arc` all
/// already assume it; the CPU canvas draw arm now does too).
///
/// The three-click placement gesture's third click can produce
/// `end_deg < start_deg` two different ways: a genuinely CW drag
/// (the cursor moved backwards past the start angle, so the unwrapped
/// tracked end angle went negative relative to start — e.g. `start:
/// 30, end: -60`), or simply because `start_deg` is a raw `atan2`
/// result while `end_deg` is separately unwrapped and the two never
/// got reconciled into a common `[0, 360)` frame. Either way, under
/// the CCW-wraparound rule `end_deg < start_deg` is read as "the long
/// way around" — the opposite of the short arc the placement preview
/// showed (see `draw_arc_preview`, which builds its ghost from the
/// exact same CCW-wraparound sweep this function's callers store).
///
/// The fix is a swap, not an independent `rem_euclid` on each field:
/// reducing `start_deg` and `end_deg` into `[0, 360)` separately
/// leaves their CCW-wraparound sweep unchanged (subtracting whole
/// turns from either endpoint can't change `end - start` by anything
/// but a multiple of 360°), so it would faithfully preserve the WRONG
/// long-way sweep. Swapping the pair instead stores the endpoints in
/// the other order, whose CCW-wraparound sweep is the complement
/// (`360° - old_sweep`) — the short arc the user actually dragged.
/// Both endpoints are then reduced into `[0, 360)` purely for a
/// canonical, human-readable stored range (matches `rotation.rs`'s
/// stored range); this reduction step alone never changes which arc
/// is represented, only its numeric presentation.
pub(super) fn normalize_arc_commit_deg(start_deg: f64, end_deg: f64) -> (f64, f64) {
    let (start_deg, end_deg) = if end_deg < start_deg {
        (end_deg, start_deg)
    } else {
        (start_deg, end_deg)
    };
    (start_deg.rem_euclid(360.0), end_deg.rem_euclid(360.0))
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
}
