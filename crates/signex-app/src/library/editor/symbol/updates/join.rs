//! Symbol editor — "Join into Polygon" selection op.
//!
//! Chains the currently-selected `Line`/`Arc` graphics end-to-end
//! (via `signex_library::chain_into_closed_contour`) into a single
//! closed `Polygon`, replacing the source graphics. See
//! [`apply_symbol_join`] for the full contract.

use signex_library::{ChainError, ChainSegment, Symbol, SymbolGraphic, SymbolGraphicKind};

use super::{SymEditor, close_pickers, mark_dirty, push_undo};
use crate::library::editor::symbol::state::{self, SymbolSelection};
use crate::library::messages::SymbolEditorMsg;

/// Surfaced when a selection mixes shared (part 0) and unit-specific
/// sources — the one ineligibility reason worth explaining, since
/// silently proceeding would have rescoped the shared shape onto just
/// the active unit. See `state::common_graphic_part_number`.
const MIXED_PARTS_MESSAGE: &str = "Selection mixes shared and unit-specific shapes";

pub(super) fn apply_symbol_join(editor: &mut SymEditor, msg: SymbolEditorMsg) {
    if !matches!(msg, SymbolEditorMsg::JoinSelectionIntoPolygon) {
        return;
    }

    let mut indices = state::join_source_indices(&editor.selected);
    if indices.is_empty() {
        return;
    }
    indices.sort_unstable();
    indices.dedup();

    // One shared predicate gates both the context-menu row's `enabled`
    // flag and this op, so they can never drift apart.
    if !state::selection_is_join_eligible(editor.primitive(), &editor.selected) {
        // Distinguish the mixed-part-number case (surfaced) from
        // every other ineligibility reason — non-Line/Arc graphic,
        // stale index — which stays a silent no-op; the context menu
        // already disables the row in all of these states.
        if state::selection_kinds_are_line_or_arc(editor.primitive(), &indices) {
            editor.status_message = Some(MIXED_PARTS_MESSAGE.to_string());
        }
        return;
    }

    let (segments, stroke_width, part_number) = {
        let sym = editor.primitive();
        let part_number = state::common_graphic_part_number(sym, &indices)
            .expect("selection_is_join_eligible guarantees a common part number");
        let (segments, stroke_width) = segments_for(sym, &indices);
        (segments, stroke_width, part_number)
    };

    let ring = match resolve_ring_with_auto_close(&segments) {
        Ok(ring) => ring,
        Err(err) => {
            editor.status_message = Some(chain_error_message(err));
            return;
        }
    };

    splice_selection_into_polygon(editor, &indices, ring, stroke_width, part_number);
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
/// Polygon on `part_number` (the sources' shared part — see
/// `state::common_graphic_part_number`, never hardcoded to the active
/// unit, so an all-shared source selection stays shared), and select
/// it. Does NOT go through `push_graphic`, which would push a second
/// undo snapshot.
fn splice_selection_into_polygon(
    editor: &mut SymEditor,
    indices: &[usize],
    ring: Vec<[f64; 2]>,
    stroke_width: f64,
    part_number: u8,
) {
    push_undo(editor);

    let mut desc = indices.to_vec();
    desc.sort_unstable_by(|a, b| b.cmp(a));
    for idx in desc {
        editor.primitive_mut().graphics.remove(idx);
    }

    editor.primitive_mut().graphics.push(SymbolGraphic {
        kind: SymbolGraphicKind::Polygon { vertices: ring },
        stroke_width,
        fill: None,
        part_number,
    });
    let new_idx = editor.primitive().graphics.len() - 1;
    editor.selected = Some(SymbolSelection::Graphic(new_idx));
    close_pickers(editor);
    mark_dirty(editor);
}

/// Build the `ChainSegment`s + max source stroke width for `indices`.
/// Assumes `state::selection_is_join_eligible` already confirmed every
/// index names a valid Line/Arc graphic — still defensive (`.get`, not
/// indexing) since a stale index should never reach here but must not
/// panic if it somehow does.
fn segments_for(sym: &Symbol, indices: &[usize]) -> (Vec<ChainSegment>, f64) {
    let mut segments = Vec::with_capacity(indices.len());
    let mut stroke_width = 0.0_f64;
    for &idx in indices {
        let Some(g) = sym.graphics.get(idx) else {
            continue;
        };
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
            _ => continue,
        };
        stroke_width = stroke_width.max(g.stroke_width);
        segments.push(seg);
    }
    (segments, stroke_width)
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

    /// A selection whose sources are all shared (part 0) keeps the
    /// result on part 0 — it must not get silently rescoped onto the
    /// active unit (default 1), which would remove the shared body
    /// from every other unit (part 0 is admitted by hit-test and
    /// box-select on every unit).
    #[test]
    fn all_part0_selection_keeps_part_zero_on_the_result() {
        let mut editor = square_editor();
        assert_eq!(editor.active_part, 1, "default active part");

        apply_symbol_join(&mut editor, SymbolEditorMsg::JoinSelectionIntoPolygon);

        assert_eq!(editor.primitive().graphics.len(), 1);
        assert_eq!(editor.primitive().graphics[0].part_number, 0);
    }

    /// A selection mixing a shared (part 0) source with an
    /// active-unit source is disqualified outright: no mutation, no
    /// undo entry, and a status message explaining why — distinct
    /// from the silent no-op every other ineligibility reason gets.
    #[test]
    fn mixed_shared_and_unit_specific_selection_is_ineligible() {
        let mut editor = new_editor();
        push_line(&mut editor, [0.0, 0.0], [4.0, 0.0]); // part 0
        editor.primitive_mut().graphics.push(SymbolGraphic {
            kind: SymbolGraphicKind::Line {
                from: [4.0, 0.0],
                to: [4.0, 4.0],
            },
            stroke_width: 0.15,
            fill: None,
            part_number: 1, // active unit
        });
        editor.selected = Some(SymbolSelection::Multiple {
            pin_indices: Vec::new(),
            graphic_indices: vec![0, 1],
        });

        apply_symbol_join(&mut editor, SymbolEditorMsg::JoinSelectionIntoPolygon);

        assert_eq!(editor.primitive().graphics.len(), 2, "nothing mutated");
        assert_eq!(editor.undo_snapshots.len(), 0, "no undo entry");
        assert_eq!(
            editor.status_message.as_deref(),
            Some("Selection mixes shared and unit-specific shapes")
        );
    }
}
