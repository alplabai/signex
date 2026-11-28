//! The selection each undo step gives back, kept in the editor's view state
//! rather than in the step: selection is view state, the step is draft state
//! (#2326, principle H7). A step carries only its id, a shared
//! `Arc<u64>`; Undo and Redo look the selection up here by that id.

use std::collections::HashMap;
use std::sync::{Arc, Weak};

use signex_sketch::id::SketchEntityId;

/// The picks a step gives back. The pad is an index into the pads, as in
/// the editor: a pad has no stable identity. Sketch picks are entity ids.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct StepSelection {
    pub pad: Option<usize>,
    pub sketch: Option<SketchEntityId>,
    pub sketch_secondary: Option<SketchEntityId>,
}

/// Selections by step id. A step can be held outside the history (an edit
/// not settled yet, a drag batch's first step, the steps a pending drag
/// pushed off or cleared), so an entry lives as long as any clone of its
/// step does, not as long as the history lists it.
#[derive(Debug, Clone, Default)]
pub struct SelectionMemo {
    by_step: HashMap<u64, (Weak<u64>, StepSelection)>,
}

impl SelectionMemo {
    /// Keep `selection` for the step `step` that is being taken now.
    pub fn remember(&mut self, step: &Arc<u64>, selection: StepSelection) {
        self.by_step.retain(|_, (held, _)| held.strong_count() > 0);
        self.by_step
            .insert(**step, (Arc::downgrade(step), selection));
    }

    /// The selection kept for `step`. Every step is taken through
    /// [`Self::remember`] and keeps its entry alive, so the empty fallback
    /// is not reached.
    pub fn recall(&self, step: &Arc<u64>) -> StepSelection {
        self.by_step
            .get(step.as_ref())
            .map(|(_, selection)| *selection)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on_pad(pad: usize) -> StepSelection {
        StepSelection {
            pad: Some(pad),
            ..StepSelection::default()
        }
    }

    #[test]
    fn a_step_and_its_clones_recall_the_selection_kept_for_it() {
        let mut memo = SelectionMemo::default();
        let (first, second) = (Arc::new(1), Arc::new(2));
        memo.remember(&first, on_pad(1));
        memo.remember(&second, on_pad(2));

        assert_eq!(memo.recall(&first), on_pad(1));
        assert_eq!(memo.recall(&Arc::clone(&second)), on_pad(2));
    }

    #[test]
    fn an_entry_goes_once_no_clone_of_its_step_is_held() {
        let mut memo = SelectionMemo::default();
        let kept = Arc::new(1);
        memo.remember(&kept, on_pad(1));
        let clone = Arc::clone(&kept);
        memo.remember(&Arc::new(2), on_pad(2));
        drop(kept);

        memo.remember(&Arc::new(3), on_pad(3));

        assert_eq!(memo.by_step.len(), 2, "the clone keeps step 1 alive");
        assert_eq!(memo.recall(&clone), on_pad(1));
    }
}
