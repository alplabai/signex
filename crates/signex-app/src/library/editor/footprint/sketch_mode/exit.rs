//! The one way out of sketch mode, the same on both hosts (#2518):
//! Bake preview · Discard changes · Bake. Bake keeps the edits and leaves;
//! Discard changes asks first when there are edits, then drops them and
//! leaves; Bake preview shows what a bake writes. The board host runs Bake
//! and Discard through its session (`app::handlers::board_sketch`); this
//! module is the footprint host's half and the state both read.
//! See `docs/FOOTPRINT_SKETCH.md`, "Leaving sketch mode".

use std::sync::Arc;

use signex_library::Footprint;
use signex_sketch::SketchError;
use signex_types::pcb_sketch::BakedBoard;
use signex_types::sketch::baked_kinds::BakedKind;
use signex_types::sketch::board_attr::SketchBoardItems;

use crate::app::FootprintDocument;
use crate::library::editor::footprint::state::EditorPad;
use crate::library::messages::FootprintEditorMsg;

/// What the exit set shows: whether the bake preview is open and whether
/// Discard changes is waiting for its confirmation. On a footprint it also
/// keeps the footprint as sketch mode found it.
#[derive(Debug, Clone, Default)]
pub struct SketchExit {
    /// The footprint and its pads when sketch mode started; Discard changes
    /// restores them. `None` on a board, whose session keeps its own start.
    entry: Option<Box<Entry>>,
    /// The bake preview card is open.
    pub preview: bool,
    /// Discard changes asked to confirm and waits for the answer.
    pub confirming_discard: bool,
    /// What a board bake of the sketch writes, while the preview is open
    /// and the sketch bakes: the canvas draws it (mockup row 190). Always `None` on
    /// a footprint, whose own fields are the bake result.
    pub board_result: Option<Arc<BakedBoard>>,
    /// The pours, mechanical drawings and anchors that bake writes.
    pub board_items: Option<Arc<SketchBoardItems>>,
    /// The router tool diameter of the board's project, in mm (mockup row 179d), set
    /// by the board session from the open project; the inside-corner check
    /// reads it. Always `None` on a footprint.
    pub router_tool_diameter: Option<f64>,
    /// Why the last Bake found the sketch unsolvable. Footprint host only.
    solve_refusal: Option<SolveRefusal>,
    /// What the last Bake wrote, for Properties (mockup row 190).
    outcome: Option<BakeOutcome>,
}

/// What a footprint Bake wrote. Properties shows it until the selection
/// changes, or until the counts it states are no longer true.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BakeOutcome {
    pub name: String,
    /// Each target the bake wrote something to, with its count.
    pub written: Vec<(BakedKind, usize)>,
    pub pads: usize,
    selection: SelectionAtBake,
}

/// The pad, silk and sketch selection when Bake finished.
type SelectionAtBake = (
    Option<usize>,
    Vec<usize>,
    Option<usize>,
    Option<signex_sketch::id::SketchEntityId>,
);

fn selection_now(editor: &FootprintDocument) -> SelectionAtBake {
    let state = &editor.state;
    (
        state.selected_pad,
        state.selected_pads_extra.clone(),
        state.selected_silk_f,
        state.selected_sketch,
    )
}

fn written_counts(fp: &Footprint) -> Vec<(BakedKind, usize)> {
    match BakePreview::of_footprint(fp) {
        BakePreview::Counts(counts) => counts.into_iter().filter(|(_, n)| *n > 0).collect(),
        BakePreview::Refused(_) => Vec::new(),
    }
}

/// Bake finished on a footprint: remember what it wrote for Properties.
pub fn record_outcome(editor: &mut FootprintDocument) {
    if editor.state.sketch_host() != signex_types::sketch::SketchHost::Footprint {
        return;
    }
    let outcome = BakeOutcome {
        name: editor.primitive().name.clone(),
        written: written_counts(editor.primitive()),
        pads: editor.primitive().pads.len(),
        selection: selection_now(editor),
    };
    editor.state.sketch_exit.outcome = Some(outcome);
}

/// The outcome Properties shows: none once the selection moved or the
/// footprint no longer holds what it states.
pub fn shown_outcome(editor: &FootprintDocument) -> Option<&BakeOutcome> {
    editor.state.sketch_exit.outcome.as_ref().filter(|outcome| {
        outcome.selection == selection_now(editor)
            && outcome.written == written_counts(editor.primitive())
    })
}

/// Forget an outcome that no longer shows, so it cannot come back.
pub fn settle_outcome(editor: &mut FootprintDocument) {
    if shown_outcome(editor).is_none() {
        editor.state.sketch_exit.outcome = None;
    }
}

/// A solver's refusal, for the sketch generation it was found on: an edit
/// moves the generation, and the reason no longer applies.
#[derive(Debug, Clone)]
struct SolveRefusal {
    reason: String,
    generation: u64,
}

#[derive(Debug, Clone)]
struct Entry {
    footprint: Footprint,
    pads: Vec<EditorPad>,
}

/// What the bake preview card shows: a count per bake target of the host,
/// or why the sketch does not bake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BakePreview {
    Counts(Vec<(BakedKind, usize)>),
    Refused(String),
}

impl BakePreview {
    /// The footprint's preview: it bakes on every solve, so its fields are
    /// the result.
    pub fn of_footprint(fp: &Footprint) -> Self {
        Self::Counts(
            signex_types::sketch::SketchHost::Footprint
                .bake_targets()
                .iter()
                .filter_map(|&kind| footprint_count(fp, kind).map(|n| (kind, n)))
                .collect(),
        )
    }

    /// The preview of the footprint `editor` edits: why it does not bake
    /// while the refusal Bake found still applies, else its counts.
    pub fn of_footprint_editor(editor: &FootprintDocument) -> Self {
        let state = &editor.state;
        match &state.sketch_exit.solve_refusal {
            Some(refusal)
                if state.last_solve.is_none()
                    && refusal.generation == state.sketch_solve.generation() =>
            {
                Self::Refused(refusal.reason.clone())
            }
            _ => Self::of_footprint(editor.primitive()),
        }
    }
}

/// Whether a footprint exit message keeps the editor in sketch mode or
/// takes it back to the pads view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitOutcome {
    Stay,
    Leave,
}

/// Sketch mode started on the footprint `editor` edits: remember it, so
/// Discard changes can give it back.
pub fn begin(editor: &mut FootprintDocument) {
    let entry = Entry {
        footprint: editor.primitive().clone(),
        pads: editor.state.pads.clone(),
    };
    editor.state.sketch_exit = SketchExit {
        entry: Some(Box::new(entry)),
        ..SketchExit::default()
    };
}

/// Whether the footprint differs from what sketch mode found. Compared by
/// content, so undoing back to the start is no change.
pub fn has_changes(editor: &FootprintDocument) -> bool {
    editor
        .state
        .sketch_exit
        .entry
        .as_ref()
        .is_some_and(|entry| entry.footprint != *editor.primitive())
}

/// Whether sketch mode has a start to compare with and the footprint and
/// its pads still equal it. `false` on a host that keeps no start.
pub fn is_unedited(editor: &FootprintDocument) -> bool {
    editor
        .state
        .sketch_exit
        .entry
        .as_ref()
        .is_some_and(|entry| {
            entry.footprint == *editor.primitive() && entry.pads == editor.state.pads
        })
}

/// An open profile stops the bake (mockup row 190): select its open ends and
/// open the preview, which names the gap. `false` when no profile is open.
fn show_open_profile(editor: &mut FootprintDocument) -> bool {
    let Some(open) = editor
        .state
        .first_open_end(editor.primitive().sketch.as_ref())
    else {
        return false;
    };
    let state = &mut editor.state;
    state.selected_sketch = open.ends.first().copied();
    state.selected_sketch_secondary = open.ends.get(1).copied();
    state.selected_sketch_extra.clear();
    state.sketch_exit.preview = true;
    editor.canvas_cache.clear();
    true
}

/// A sketch with no solve is solved now (a footprint can open unsolved, or
/// its display solve may not have landed). One that cannot be solved stops
/// the bake: open the preview, which says why. `false` when it solves or has
/// nothing to solve. Nothing is baked here, so the document stays as it was.
fn show_unsolvable(editor: &mut FootprintDocument) -> bool {
    if editor.state.last_solve.is_some() {
        return false;
    }
    let Some(sketch) = editor
        .primitive()
        .sketch
        .as_ref()
        .filter(|sketch| !sketch.entities.is_empty())
    else {
        return false;
    };
    let solved = signex_sketch::parameter::resolve(&sketch.parameters)
        .map_err(SketchError::Expr)
        .and_then(|params| {
            editor
                .state
                .sketch_solver
                .solve_deferring_redundancy(sketch, &params)
                .map_err(SketchError::SolveFailed)
        });
    let state = &mut editor.state;
    match solved {
        Ok(out) => {
            state
                .sketch_solve
                .set_redundancy_owed(out.redundancy_pending);
            state.last_solve = Some(out);
            false
        }
        Err(error) => {
            state.sketch_exit.solve_refusal = Some(SolveRefusal {
                reason: error.to_string(),
                generation: state.sketch_solve.generation(),
            });
            state.sketch_exit.preview = true;
            editor.canvas_cache.clear();
            true
        }
    }
}

/// Apply one exit message to a footprint in sketch mode.
pub fn apply(editor: &mut FootprintDocument, msg: &FootprintEditorMsg) -> ExitOutcome {
    match msg {
        FootprintEditorMsg::SketchBakePreviewToggled => {
            let exit = &mut editor.state.sketch_exit;
            exit.preview = !exit.preview;
            // The canvas draws the bake result while the preview is open.
            editor.canvas_cache.clear();
            ExitOutcome::Stay
        }
        FootprintEditorMsg::SketchBakeRequested => {
            // Both run: an unsolvable sketch can have an open profile too,
            // and the card names each.
            let unsolvable = show_unsolvable(editor);
            let open = show_open_profile(editor);
            if unsolvable || open {
                ExitOutcome::Stay
            } else {
                ExitOutcome::Leave
            }
        }
        FootprintEditorMsg::SketchDiscardRequested => {
            if has_changes(editor) {
                editor.state.sketch_exit.confirming_discard = true;
                ExitOutcome::Stay
            } else {
                ExitOutcome::Leave
            }
        }
        FootprintEditorMsg::SketchDiscardCancelled => {
            editor.state.sketch_exit.confirming_discard = false;
            ExitOutcome::Stay
        }
        FootprintEditorMsg::SketchDiscardConfirmed => {
            restore_entry(editor);
            ExitOutcome::Leave
        }
        _ => ExitOutcome::Stay,
    }
}

/// Put back the footprint sketch mode found. Runs inside one message, so
/// it is one undo step: Undo brings the discarded edits back. Memory only;
/// nothing is written to disk.
fn restore_entry(editor: &mut FootprintDocument) {
    let Some(entry) = editor.state.sketch_exit.entry.take() else {
        return;
    };
    let Entry { footprint, pads } = *entry;
    *editor.primitive_mut() = footprint;
    let state = &mut editor.state;
    state.pads = pads;
    state.selected_pad = None;
    state.selected_pads_extra.clear();
    state.selected_sketch = None;
    state.selected_sketch_secondary = None;
    state.selected_sketch_extra.clear();
    state.clear_silk_selection();
    // A solve still in flight ran on the sketch that is gone.
    state.sketch_solve.supersede();
    editor.canvas_cache.clear();
    // The restored footprint may differ from the last save.
    editor.mark_dirty();
}

/// How many items of `kind` the footprint holds, for the bake preview.
/// The footprint bakes on every solve, so its fields are the bake result.
/// `None`: a footprint never bakes `kind`.
pub fn footprint_count(fp: &Footprint, kind: BakedKind) -> Option<usize> {
    Some(match kind {
        BakedKind::Pads => fp.pads.len(),
        BakedKind::SilkF => fp.silk_f.len(),
        BakedKind::SilkB => fp.silk_b.len(),
        BakedKind::Courtyard => usize::from(!fp.courtyard.points.is_empty()),
        BakedKind::MaskOpenings => fp.mask_openings.len(),
        BakedKind::MaskExcludes => fp.mask_excludes.len(),
        BakedKind::PasteApertures => fp.paste_apertures.len(),
        BakedKind::Pours => fp.pours.len(),
        BakedKind::Keepouts => fp.keepouts.len(),
        BakedKind::Cutouts => fp.cutouts.len(),
        BakedKind::VScores => fp.v_scores.len(),
        BakedKind::BoardOutline
        | BakedKind::MountingHoles
        | BakedKind::BoardPours
        | BakedKind::Mechanical
        | BakedKind::ComponentAnchors => return None,
    })
}

#[cfg(test)]
#[path = "exit_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "exit_preview_tests.rs"]
mod preview_tests;
