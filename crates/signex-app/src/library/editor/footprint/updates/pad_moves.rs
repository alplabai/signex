//! Moving and aligning given footprint pads (#3013): the one edit behind
//! Move Selection By's Move, Align Selection's Align and the
//! `footprint_move_pads` / `footprint_align_pads` commands. The dialogs
//! pass the selection; the commands pass the pads their `pad_numbers` operand
//! names. Each kept edit is one undo step; an edit that moves nothing
//! records no step and leaves the document clean.

use super::align_pads;
use crate::app::FootprintDocument;
use crate::library::editor::footprint::pad_to_sketch;
use crate::library::editor::footprint::state::{AlignOp, EditorPad, FootprintEditorState};

/// How many pads `op` needs: two to align, three to distribute.
pub(crate) fn pads_needed(op: AlignOp) -> usize {
    match op {
        AlignOp::DistributeH | AlignOp::DistributeV => 3,
        _ => 2,
    }
}

/// `indices` sorted, each once, and only those of a pad the editor has.
fn live_indices(editor: &FootprintDocument, indices: &[usize]) -> Vec<usize> {
    let mut live = indices.to_vec();
    live.sort_unstable();
    live.dedup();
    live.retain(|&i| i < editor.state.pads.len());
    live
}

/// Mirror the pads at `indices` into the sketch, then re-sync the
/// footprint's pad list.
fn settle_moved(
    state: &mut FootprintEditorState,
    primitive: &mut signex_library::primitive::footprint::Footprint,
    indices: &[usize],
) {
    let moved: Vec<EditorPad> = indices
        .iter()
        .filter_map(|&i| state.pads.get(i).cloned())
        .collect();
    for snapshot in &moved {
        pad_to_sketch::mirror_move_pad_in_sketch(snapshot, primitive);
    }
    FootprintEditorState::sync_pads_to_primitive(state, primitive);
}

/// Move the pads at `indices` by (`dx`, `dy`) mm, one undo step. Returns
/// whether any pad was there to move.
pub(super) fn move_pads(
    editor: &mut FootprintDocument,
    indices: &[usize],
    dx: f64,
    dy: f64,
) -> bool {
    let indices = live_indices(editor, indices);
    if indices.is_empty() {
        return false;
    }
    editor.push_history();
    editor.with_parts(|state, primitive| {
        let moved = state.nudge_pads(&indices, dx, dy);
        settle_moved(state, primitive, &moved);
    });
    editor.canvas_cache.clear();
    editor.mark_dirty();
    true
}

/// Apply `horizontal` and `vertical` to the pads at `indices` under one undo
/// step. An op the pads are too few for is skipped, as picking its own row
/// skips it; with none left nothing is recorded. Returns whether an op ran.
pub(super) fn align_pads_on_axes(
    editor: &mut FootprintDocument,
    indices: &[usize],
    horizontal: Option<AlignOp>,
    vertical: Option<AlignOp>,
) -> bool {
    let indices = live_indices(editor, indices);
    let applicable: Vec<AlignOp> = [horizontal, vertical]
        .into_iter()
        .flatten()
        .filter(|op| indices.len() >= pads_needed(*op))
        .collect();
    if applicable.is_empty() {
        return false;
    }
    editor.push_history();
    editor.with_parts(|state, primitive| {
        // The spacing ops read the grid step; the dialog's ops do not.
        let step = state.snap_options.grid_step_mm.max(0.001);
        for op in &applicable {
            align_pads(state, &indices, *op, step);
        }
        settle_moved(state, primitive, &indices);
    });
    editor.canvas_cache.clear();
    editor.mark_dirty();
    true
}
