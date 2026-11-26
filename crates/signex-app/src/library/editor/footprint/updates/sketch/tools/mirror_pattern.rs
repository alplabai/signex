//! Mirror and the pattern tools with a pattern selected (mockup row 191, decision
//! 10, Caner 2026-11-20). Mirror keeps a live mirror of the pattern, a
//! pattern object of its own, rather than copies of geometry; a pattern
//! tool makes the new array repeat every instance of the selected pattern.
//! Mirror's pattern objects come from the Mirror card (`transform::mirror`).
//! A link that would loop is refused with a warning and changes nothing.
//! The tool click or the card's OK is already one undo step (the
//! dispatcher's edit attempt); a refusal is recorded as a rejection so it
//! leaves none.

use signex_sketch::array::ArrayId;
use signex_sketch::sketch::{PatternCycle, SketchData};

use crate::app::FootprintDocument;
use crate::library::editor::footprint::sketch_conflict::mutate_solve_or_revert;
use crate::library::editor::footprint::sketch_dispatch::report_solve_failure;

/// The selected pattern, when the sketch still has it.
pub(super) fn selected_pattern(editor: &FootprintDocument) -> Option<ArrayId> {
    let pattern = editor.state.sketch_overlay.selected_pattern?;
    let sketch = editor.primitive().sketch.as_ref()?;
    sketch.is_pattern(pattern).then_some(pattern)
}

/// Run `link` on the sketch, then solve and bake. A refused link changes
/// nothing and is handed back.
pub(super) fn link_patterns(
    editor: &mut FootprintDocument,
    link: impl FnOnce(&mut SketchData) -> Result<(), PatternCycle>,
) -> Result<(), PatternCycle> {
    let before = editor.primitive().sketch.clone();
    let Some(mut next) = before.clone() else {
        return Ok(());
    };
    // Tried on a copy first: a refused link must not even re-bake.
    link(&mut next)?;
    let result = editor
        .with_parts(|state, fp| mutate_solve_or_revert(state, fp, |fp| fp.sketch = Some(next)));
    if let Err(error) = &result {
        report_solve_failure(&mut editor.state, error);
    }
    if editor.primitive().sketch != before {
        editor.mark_dirty();
    }
    editor.canvas_cache.clear();
    Ok(())
}

/// Report a refused pattern link. The click is recorded as rejected, so
/// it leaves no undo step and no dirty flag.
pub(super) fn refuse(editor: &mut FootprintDocument, warning: String) {
    editor.state.solve_warnings.push(warning);
    editor
        .state
        .conflict
        .record_rejection(Vec::new(), Vec::new(), Vec::new());
    editor.canvas_cache.clear();
}
