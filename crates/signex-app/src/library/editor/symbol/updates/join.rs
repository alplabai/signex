//! Symbol editor — "Join into Polygon" selection op.
//!
//! Chains the currently-selected `Line`/`Arc` graphics end-to-end
//! (via `signex_library::chain_into_closed_contour`) into a single
//! closed `Polygon`, replacing the source graphics. See
//! [`apply_symbol_join`] for the full contract.

use signex_library::{ChainError, ChainSegment, SymbolGraphic, SymbolGraphicKind};

use super::{SymEditor, close_pickers, mark_dirty, push_undo};
use crate::library::editor::symbol::state::SymbolSelection;
use crate::library::messages::SymbolEditorMsg;

pub(super) fn apply_symbol_join(editor: &mut SymEditor, msg: SymbolEditorMsg) {
    if !matches!(msg, SymbolEditorMsg::JoinSelectionIntoPolygon) {
        return;
    }

    let mut indices = selected_graphic_indices(&editor.selected);
    if indices.is_empty() {
        return;
    }
    indices.sort_unstable();
    indices.dedup();

    let Some((segments, stroke_width)) = eligible_segments(editor, &indices) else {
        // Selection contains a non-Line/Arc graphic (or a stale
        // index) — no-op per the "belt and braces" contract; the
        // context menu also disables the item in this state.
        return;
    };

    let ring = match resolve_ring_with_auto_close(&segments) {
        Ok(ring) => ring,
        Err(err) => {
            editor.status_message = Some(chain_error_message(err));
            return;
        }
    };

    splice_selection_into_polygon(editor, &indices, ring, stroke_width);
    editor.status_message = None;
}

/// Chain `segments` into a closed ring, auto-closing exactly once by
/// synthesizing the missing edge between an [`ChainError::OpenChain`]'s
/// two loose ends if the first attempt doesn't close.
fn resolve_ring_with_auto_close(segments: &[ChainSegment]) -> Result<Vec<[f64; 2]>, ChainError> {
    match signex_library::chain_into_closed_contour(segments) {
        Err(ChainError::OpenChain { ends, .. }) => {
            let mut retried = segments.to_vec();
            retried.push(ChainSegment::Line {
                from: ends[0],
                to: ends[1],
            });
            signex_library::chain_into_closed_contour(&retried)
        }
        result => result,
    }
}

/// Composite mutation on success: one undo snapshot, remove the
/// source graphics (descending index order), append the joined
/// Polygon, and select it. Does NOT go through `push_graphic`, which
/// would push a second undo snapshot.
fn splice_selection_into_polygon(
    editor: &mut SymEditor,
    indices: &[usize],
    ring: Vec<[f64; 2]>,
    stroke_width: f64,
) {
    push_undo(editor);

    let mut desc = indices.to_vec();
    desc.sort_unstable_by(|a, b| b.cmp(a));
    for idx in desc {
        editor.primitive_mut().graphics.remove(idx);
    }

    let active_part = editor.active_part;
    editor.primitive_mut().graphics.push(SymbolGraphic {
        kind: SymbolGraphicKind::Polygon { vertices: ring },
        stroke_width,
        fill: None,
        part_number: active_part,
    });
    let new_idx = editor.primitive().graphics.len() - 1;
    editor.selected = Some(SymbolSelection::Graphic(new_idx));
    close_pickers(editor);
    mark_dirty(editor);
}

/// Graphic indices named by the current selection — the empty `Vec`
/// for every selection kind that doesn't name individual graphics
/// (`None`, `Pin`, `Field`, `All`).
fn selected_graphic_indices(selected: &Option<SymbolSelection>) -> Vec<usize> {
    match selected {
        Some(SymbolSelection::Graphic(idx)) => vec![*idx],
        Some(SymbolSelection::Multiple {
            graphic_indices, ..
        }) => graphic_indices.clone(),
        _ => Vec::new(),
    }
}

/// Resolve `indices` against the active symbol's graphics, returning
/// the matching `ChainSegment`s plus the max source stroke width —
/// or `None` if any index is stale or names a non-Line/Arc graphic
/// (Polygon/Rectangle/Circle/Text), per the eligibility contract.
fn eligible_segments(editor: &SymEditor, indices: &[usize]) -> Option<(Vec<ChainSegment>, f64)> {
    let mut segments = Vec::with_capacity(indices.len());
    let mut stroke_width = 0.0_f64;
    for &idx in indices {
        let g = editor.primitive().graphics.get(idx)?;
        let seg = match g.kind {
            SymbolGraphicKind::Line { from, to } => ChainSegment::Line { from, to },
            SymbolGraphicKind::Arc {
                center,
                radius,
                start_deg,
                end_deg,
            } => ChainSegment::Arc {
                center,
                radius,
                start_deg,
                end_deg,
            },
            _ => return None,
        };
        stroke_width = stroke_width.max(g.stroke_width);
        segments.push(seg);
    }
    Some((segments, stroke_width))
}

/// Human-readable status-line text for a failed join attempt.
fn chain_error_message(err: ChainError) -> String {
    match err {
        ChainError::OpenChain { gap_mm, .. } => {
            format!("Shapes don't connect end-to-end (gap {gap_mm:.2} mm)")
        }
        ChainError::Branching { at } => format!("Shapes branch at ({:.2}, {:.2})", at[0], at[1]),
        ChainError::Disjoint => "Selection splits into separate chains".to_string(),
        ChainError::Empty
        | ChainError::InvalidInput { .. }
        | ChainError::DegenerateSegment { .. }
        | ChainError::DegenerateResult => "Selection is degenerate".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::editor::symbol::state::SymbolSelection;
    use signex_library::{Symbol, SymbolFile};
    use std::path::PathBuf;

    fn new_editor() -> SymEditor {
        SymEditor::new(
            PathBuf::from("t.snxsym"),
            SymbolFile::from_symbol(Symbol::empty("T")),
        )
    }

    fn push_line(editor: &mut SymEditor, from: [f64; 2], to: [f64; 2]) -> usize {
        editor.primitive_mut().graphics.push(SymbolGraphic {
            kind: SymbolGraphicKind::Line { from, to },
            stroke_width: 0.15,
            fill: None,
            part_number: 0,
        });
        editor.primitive().graphics.len() - 1
    }

    fn square_editor() -> SymEditor {
        let mut editor = new_editor();
        push_line(&mut editor, [0.0, 0.0], [4.0, 0.0]);
        push_line(&mut editor, [4.0, 0.0], [4.0, 4.0]);
        push_line(&mut editor, [4.0, 4.0], [0.0, 4.0]);
        push_line(&mut editor, [0.0, 4.0], [0.0, 0.0]);
        editor.selected = Some(SymbolSelection::Multiple {
            pin_indices: Vec::new(),
            graphic_indices: vec![0, 1, 2, 3],
        });
        editor
    }

    /// Square from 4 selected lines joins into 1 polygon; the 4
    /// sources are gone; undo restores all 4 sources and removes the
    /// polygon.
    #[test]
    fn square_from_four_lines_joins_and_undo_restores_sources() {
        let mut editor = square_editor();

        apply_symbol_join(&mut editor, SymbolEditorMsg::JoinSelectionIntoPolygon);

        assert_eq!(editor.primitive().graphics.len(), 1);
        match &editor.primitive().graphics[0].kind {
            SymbolGraphicKind::Polygon { vertices } => assert_eq!(vertices.len(), 4),
            other => panic!("expected Polygon, got {other:?}"),
        }
        assert_eq!(
            editor.selected,
            Some(SymbolSelection::Graphic(0)),
            "new polygon is selected"
        );
        assert_eq!(
            editor.undo_snapshots.len(),
            1,
            "exactly one undo snapshot for the whole composite op"
        );

        let snapshot = editor.undo_snapshots.pop().unwrap();
        *editor.primitive_mut() = snapshot;
        assert_eq!(
            editor.primitive().graphics.len(),
            4,
            "undo restores sources"
        );
        assert!(
            editor
                .primitive()
                .graphics
                .iter()
                .all(|g| matches!(g.kind, SymbolGraphicKind::Line { .. })),
            "undo removes the polygon"
        );
    }

    /// 3 of the 4 sides selected (open chain) auto-closes via the
    /// missing edge and still produces one polygon.
    #[test]
    fn open_three_side_chain_auto_closes() {
        let mut editor = new_editor();
        push_line(&mut editor, [0.0, 0.0], [4.0, 0.0]);
        push_line(&mut editor, [4.0, 0.0], [4.0, 4.0]);
        push_line(&mut editor, [4.0, 4.0], [0.0, 4.0]);
        editor.selected = Some(SymbolSelection::Multiple {
            pin_indices: Vec::new(),
            graphic_indices: vec![0, 1, 2],
        });

        apply_symbol_join(&mut editor, SymbolEditorMsg::JoinSelectionIntoPolygon);

        assert_eq!(editor.primitive().graphics.len(), 1);
        match &editor.primitive().graphics[0].kind {
            SymbolGraphicKind::Polygon { vertices } => assert_eq!(vertices.len(), 4),
            other => panic!("expected Polygon, got {other:?}"),
        }
        assert_eq!(editor.undo_snapshots.len(), 1);
        assert!(editor.status_message.is_none());
    }

    /// A triangle built from two lines and one arc side joins fine.
    #[test]
    fn arc_and_lines_join() {
        let mut editor = new_editor();
        let p0 = [0.0, 0.0];
        let p1 = [4.0, 0.0];
        let p2 = [4.0, 4.0];
        push_line(&mut editor, p0, p1);
        push_line(&mut editor, p1, p2);
        editor.primitive_mut().graphics.push(SymbolGraphic {
            kind: SymbolGraphicKind::Arc {
                center: [0.0, 4.0],
                radius: 4.0,
                start_deg: 0.0,
                end_deg: 270.0,
            },
            stroke_width: 0.25,
            fill: None,
            part_number: 0,
        });
        editor.selected = Some(SymbolSelection::Multiple {
            pin_indices: Vec::new(),
            graphic_indices: vec![0, 1, 2],
        });

        apply_symbol_join(&mut editor, SymbolEditorMsg::JoinSelectionIntoPolygon);

        assert_eq!(editor.primitive().graphics.len(), 1);
        match &editor.primitive().graphics[0].kind {
            SymbolGraphicKind::Polygon { vertices } => assert!(vertices.len() > 3),
            other => panic!("expected Polygon, got {other:?}"),
        }
    }

    /// A branching (T-junction) selection errors, mutates nothing, and
    /// pushes no undo entry.
    #[test]
    fn branching_selection_errors_with_no_mutation() {
        let mut editor = new_editor();
        push_line(&mut editor, [0.0, 0.0], [2.0, 0.0]);
        push_line(&mut editor, [0.0, 0.0], [0.0, 2.0]);
        push_line(&mut editor, [0.0, 0.0], [-2.0, 0.0]);
        editor.selected = Some(SymbolSelection::Multiple {
            pin_indices: Vec::new(),
            graphic_indices: vec![0, 1, 2],
        });

        apply_symbol_join(&mut editor, SymbolEditorMsg::JoinSelectionIntoPolygon);

        assert_eq!(
            editor.primitive().graphics.len(),
            3,
            "no source removed on error"
        );
        assert!(
            !editor
                .primitive()
                .graphics
                .iter()
                .any(|g| matches!(g.kind, SymbolGraphicKind::Polygon { .. })),
            "no polygon appended on error"
        );
        assert_eq!(editor.undo_snapshots.len(), 0, "no undo entry on error");
        assert!(editor.status_message.is_some());
        assert!(editor.status_message.as_deref().unwrap().contains("branch"));
    }

    /// A selection containing a non-Line/Arc graphic (Rectangle) is a
    /// no-op: nothing removed, nothing appended, no undo entry, no
    /// status message (dispatch-level guard, distinct from a chain
    /// error).
    #[test]
    fn selection_with_rectangle_is_a_no_op() {
        let mut editor = new_editor();
        push_line(&mut editor, [0.0, 0.0], [4.0, 0.0]);
        editor.primitive_mut().graphics.push(SymbolGraphic {
            kind: SymbolGraphicKind::Rectangle {
                from: [0.0, 0.0],
                to: [2.0, 2.0],
            },
            stroke_width: 0.15,
            fill: None,
            part_number: 0,
        });
        editor.selected = Some(SymbolSelection::Multiple {
            pin_indices: Vec::new(),
            graphic_indices: vec![0, 1],
        });

        apply_symbol_join(&mut editor, SymbolEditorMsg::JoinSelectionIntoPolygon);

        assert_eq!(editor.primitive().graphics.len(), 2, "nothing mutated");
        assert_eq!(editor.undo_snapshots.len(), 0);
        assert!(editor.status_message.is_none());
    }

    /// The joined polygon's stroke width is the max of the source
    /// graphics' stroke widths.
    #[test]
    fn stroke_width_is_max_of_sources() {
        let mut editor = new_editor();
        editor.primitive_mut().graphics.push(SymbolGraphic {
            kind: SymbolGraphicKind::Line {
                from: [0.0, 0.0],
                to: [4.0, 0.0],
            },
            stroke_width: 0.1,
            fill: None,
            part_number: 0,
        });
        editor.primitive_mut().graphics.push(SymbolGraphic {
            kind: SymbolGraphicKind::Line {
                from: [4.0, 0.0],
                to: [2.0, 3.0],
            },
            stroke_width: 0.5,
            fill: None,
            part_number: 0,
        });
        editor.primitive_mut().graphics.push(SymbolGraphic {
            kind: SymbolGraphicKind::Line {
                from: [2.0, 3.0],
                to: [0.0, 0.0],
            },
            stroke_width: 0.3,
            fill: None,
            part_number: 0,
        });
        editor.selected = Some(SymbolSelection::Multiple {
            pin_indices: Vec::new(),
            graphic_indices: vec![0, 1, 2],
        });

        apply_symbol_join(&mut editor, SymbolEditorMsg::JoinSelectionIntoPolygon);

        assert_eq!(editor.primitive().graphics[0].stroke_width, 0.5);
    }

    /// Empty selection is a no-op.
    #[test]
    fn empty_selection_is_a_no_op() {
        let mut editor = new_editor();
        apply_symbol_join(&mut editor, SymbolEditorMsg::JoinSelectionIntoPolygon);
        assert!(editor.primitive().graphics.is_empty());
        assert_eq!(editor.undo_snapshots.len(), 0);
    }
}
