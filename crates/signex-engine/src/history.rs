use signex_types::schematic::SchematicSheet;

use crate::EngineError;
use crate::patch::{DocumentPatch, PatchPair};

use super::Engine;
use super::history_snapshot::SheetSnapshot;

#[derive(Debug)]
pub(super) struct HistoryEntry {
    pub snapshot: SheetSnapshot,
    pub patch_pair: PatchPair,
    /// Set when this entry is one sheet's share of a project-wide step; the
    /// app undoes/redoes every sheet carrying the same id together.
    pub group: Option<UndoGroupId>,
}

/// A net-class pick made from this sheet. The class table is project data
/// (`NetClassHistory`), so the marker only fixes the pick's place among the
/// sheet's own steps; the app steps the project when it comes up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassPickMarker {
    /// The project whose class table the pick changed (the app's project id).
    pub project: u32,
    /// The pick's step in that project's class history.
    pub step: crate::NetClassStepId,
}

impl ClassPickMarker {
    /// The step id of a dead marker: class history ids count up from 0, so
    /// none ever reaches it.
    pub const DEAD_STEP: crate::NetClassStepId = crate::NetClassStepId::MAX;

    /// This marker, no longer able to match any class step.
    pub const fn dead(self) -> Self {
        Self {
            step: Self::DEAD_STEP,
            ..self
        }
    }
}

/// One step of the sheet's timeline: an edit of the sheet, or a marker that
/// changes nothing on it.
#[derive(Debug)]
pub(super) enum HistoryStep {
    Edit(Box<HistoryEntry>),
    Pick(ClassPickMarker),
}

impl HistoryStep {
    pub(super) fn group(&self) -> Option<UndoGroupId> {
        match self {
            Self::Edit(entry) => entry.group,
            Self::Pick(_) => None,
        }
    }

    fn marker(&self) -> Option<ClassPickMarker> {
        match self {
            Self::Pick(marker) => Some(*marker),
            Self::Edit(_) => None,
        }
    }

    #[cfg(test)]
    pub(super) fn edit(&self) -> &HistoryEntry {
        match self {
            Self::Edit(entry) => entry,
            Self::Pick(_) => panic!("a marker, not an edit"),
        }
    }
}

/// The snapshot of the newest edit on `steps`, markers skipped, to share
/// collections with.
fn last_snapshot(steps: &[HistoryStep]) -> Option<&SheetSnapshot> {
    steps.iter().rev().find_map(|step| match step {
        HistoryStep::Edit(entry) => Some(&entry.snapshot),
        HistoryStep::Pick(_) => None,
    })
}

/// Identifies one project-wide undo step (e.g. an annotate run) whose
/// per-sheet history entries must be undone and redone together. The engine
/// only stamps and reports it; coordinating sheets is the app's job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UndoGroupId(u64);

impl UndoGroupId {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// The raw value, as an agent names the group in `revert`.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// The parameter cell a run of keystrokes is editing, and where the history
/// stood right after the run's entry was recorded.
#[derive(Debug)]
pub(super) struct FieldEditRun {
    symbol_id: uuid::Uuid,
    key: String,
    position: usize,
}

impl Engine {
    /// Record a `SetSymbolField` edit. Consecutive edits of the same cell
    /// share the one history entry the first of them recorded, so typing a
    /// value is one undo step while every keystroke is already in the
    /// document. The run ends when anything else lands in the history, when
    /// another cell is edited, on undo/redo, or on
    /// [`Engine::end_field_edit_run`].
    pub(crate) fn record_field_edit(
        &mut self,
        before: SchematicSheet,
        patch_pair: PatchPair,
        symbol_id: uuid::Uuid,
        key: &str,
    ) {
        if self.history_suppressed {
            self.record_history(before, patch_pair);
            return;
        }
        let continues = !self.history.can_redo()
            && self.field_edit_run.as_ref().is_some_and(|run| {
                run.symbol_id == symbol_id && run.key == key && run.position == self.undo_position()
            });
        if continues {
            // The run rewrites the entry the history already ends in, so a
            // save taken at this position no longer matches the document.
            self.history.forget_if_current();
        } else {
            self.record_history(before, patch_pair);
            self.field_edit_run = Some(FieldEditRun {
                symbol_id,
                key: key.to_string(),
                position: self.undo_position(),
            });
        }
    }

    /// Close the current cell-typing run: the next edit of the same cell
    /// starts a new undo step. Enter, closing the Parameter Manager.
    pub fn end_field_edit_run(&mut self) {
        self.field_edit_run = None;
    }

    /// The document now matches the file just written: remember this point
    /// of the history, so undoing or redoing back to it is clean again.
    pub fn mark_saved(&mut self) {
        self.history.mark_saved();
    }

    /// The file holds something no step of this history reproduces (a save
    /// whose snapshot the document has moved past): nothing is clean until
    /// the next [`Engine::mark_saved`].
    pub fn forget_save_point(&mut self) {
        self.history.forget();
    }

    /// Does the document equal the last saved or loaded state, judged by
    /// the history alone? Undo and redo move this both ways; an edit made
    /// below the save point makes it unreachable until the next save.
    pub fn is_at_save_point(&self) -> bool {
        // A marker between the save and now changed nothing on the sheet.
        !self
            .history
            .is_dirty_past(|step| matches!(step, HistoryStep::Pick(_)))
    }

    /// Record a net-class pick made from this sheet. It is a step of the
    /// timeline (undone and redone in order, evicted by the depth) that
    /// never touches the document or its save point, and it keeps the
    /// sheet's redo branch: a pick does not change the sheet.
    pub fn record_class_pick(&mut self, marker: ClassPickMarker) {
        self.field_edit_run = None;
        self.history.record_inert(HistoryStep::Pick(marker));
    }

    /// The marker on top of the undo (`undo`) or redo stack, if the next step
    /// that way is one. [`Engine::undo`] and [`Engine::redo`] refuse to step
    /// over it: the caller applies or drops it, then
    /// [`Engine::pass_class_pick`]s.
    pub fn next_class_pick(&self, undo: bool) -> Option<ClassPickMarker> {
        let steps = if undo {
            self.history.undo_steps()
        } else {
            self.history.redo_steps()
        };
        steps.last().and_then(HistoryStep::marker)
    }

    /// Move the marker on top of the undo (`undo`) or redo stack to the other
    /// one. `false` when the top step is not a marker.
    pub fn pass_class_pick(&mut self, undo: bool) -> bool {
        let Some(marker) = self.next_class_pick(undo) else {
            return false;
        };
        self.field_edit_run = None;
        self.history.step(HistoryStep::Pick(marker), undo).is_some()
    }

    /// [`Engine::pass_class_pick`] for a marker the caller found dead: it
    /// moves over as a marker that can never match a class step again, so a
    /// later change of the project's history cannot bring it back to life.
    pub fn pass_dead_class_pick(&mut self, undo: bool) -> bool {
        let Some(marker) = self.next_class_pick(undo) else {
            return false;
        };
        self.field_edit_run = None;
        self.history
            .step(HistoryStep::Pick(marker.dead()), undo)
            .is_some()
    }

    /// The steps one way from the top, newest first: `None` for an edit of
    /// the sheet, the marker for a pick.
    pub fn steps_from_top(&self, undo: bool) -> impl Iterator<Item = Option<ClassPickMarker>> + '_ {
        let steps = if undo {
            self.history.undo_steps()
        } else {
            self.history.redo_steps()
        };
        steps.iter().rev().map(HistoryStep::marker)
    }

    /// Is there an edit of the sheet (not just a marker) to undo or redo?
    pub fn has_document_step(&self, undo: bool) -> bool {
        self.steps_from_top(undo).any(|marker| marker.is_none())
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// How many undo steps this sheet keeps; see [`Engine::set_undo_depth`].
    pub fn undo_depth(&self) -> usize {
        self.history.cap()
    }

    /// Keep `depth` undo steps from now on (the `undo_depth` setting).
    /// Lowering it drops the oldest steps at once; a save point that falls
    /// off the bottom is gone, so the sheet stays dirty until the next save.
    pub fn set_undo_depth(&mut self, depth: usize) {
        self.history.set_cap(depth);
    }

    /// Whether any undo entry still belongs to `group`.
    pub fn has_undo_group(&self, group: UndoGroupId) -> bool {
        self.history
            .undo_steps()
            .iter()
            .any(|step| step.group() == Some(group))
    }

    /// Group of the entry the next [`Engine::undo`] would revert, if any.
    pub fn next_undo_group(&self) -> Option<UndoGroupId> {
        self.history
            .undo_steps()
            .last()
            .and_then(HistoryStep::group)
    }

    /// Group of the entry the next [`Engine::redo`] would restore, if any.
    pub fn next_redo_group(&self) -> Option<UndoGroupId> {
        self.history
            .redo_steps()
            .last()
            .and_then(HistoryStep::group)
    }

    /// Monotonic position in the edit timeline: how many recorded edits
    /// are currently applied, counting any evicted by the history cap.
    /// Undo lowers it, redo raises it, a new edit raises it and drops
    /// the redo branch. App-level state that interleaves with engine
    /// history (net colours) stamps itself with this to order undo.
    pub fn undo_position(&self) -> usize {
        self.history.position()
    }

    /// An undo or redo swaps in a snapshot; connectivity moves with it when
    /// the step being replayed touched any electrical collection.
    fn note_history_step(&mut self, patch_pair: PatchPair) {
        if patch_pair.document.intersects(DocumentPatch::ELECTRICAL) {
            self.bump_connectivity_revision();
        }
    }

    /// Install a history snapshot as the document and return the one it
    /// replaced. The document id is the live one, not the snapshot's: a Save
    /// As re-keyed it, and an undo past that must not bring the old id back
    /// (#2172).
    fn swap_in_snapshot(&mut self, snapshot: SheetSnapshot) -> SchematicSheet {
        let mut restored = snapshot.into_sheet();
        restored.uuid = self.document.uuid;
        std::mem::replace(&mut self.document, restored)
    }

    pub fn undo(&mut self) -> Result<Option<PatchPair>, EngineError> {
        self.field_edit_run = None;
        // An empty stack, or a marker the app has yet to step, changes
        // nothing, so it moves no revision.
        if self.next_class_pick(true).is_some() {
            return Ok(None);
        }
        let Some(HistoryStep::Edit(entry)) = self.history.begin_step(true) else {
            return Ok(None);
        };
        self.bump_revision();

        let patch_pair = entry.patch_pair;
        self.note_history_step(patch_pair);
        let group = entry.group;
        let redo_snapshot = self.swap_in_snapshot(entry.snapshot);
        let redo_snapshot =
            SheetSnapshot::capture(redo_snapshot, last_snapshot(self.history.redo_steps()));
        self.history.finish_step(
            HistoryStep::Edit(Box::new(HistoryEntry {
                snapshot: redo_snapshot,
                patch_pair,
                group,
            })),
            true,
        );
        Ok(Some(patch_pair))
    }

    /// Undo the top entry when it belongs to `group`, and forget it: it does
    /// not land on the redo stack. For a grouped run that failed halfway and
    /// was reported as never having happened, so no Redo can bring half of it
    /// back. `None` (nothing changed) when the top entry is not `group`'s.
    pub fn discard_undo_group_top(
        &mut self,
        group: UndoGroupId,
    ) -> Result<Option<PatchPair>, EngineError> {
        if self.next_undo_group() != Some(group) {
            return Ok(None);
        }
        let patch_pair = self.undo()?;
        self.history.discard_top();
        Ok(patch_pair)
    }

    pub fn redo(&mut self) -> Result<Option<PatchPair>, EngineError> {
        self.field_edit_run = None;
        // As in `undo`: nothing to move, no revision.
        if self.next_class_pick(false).is_some() {
            return Ok(None);
        }
        let Some(HistoryStep::Edit(entry)) = self.history.begin_step(false) else {
            return Ok(None);
        };
        self.bump_revision();

        let patch_pair = entry.patch_pair;
        self.note_history_step(patch_pair);
        let group = entry.group;
        let undo_snapshot = self.swap_in_snapshot(entry.snapshot);
        let undo_snapshot =
            SheetSnapshot::capture(undo_snapshot, last_snapshot(self.history.undo_steps()));
        self.history.finish_step(
            HistoryStep::Edit(Box::new(HistoryEntry {
                snapshot: undo_snapshot,
                patch_pair,
                group,
            })),
            false,
        );
        Ok(Some(patch_pair))
    }

    pub(crate) fn record_history(&mut self, snapshot: SchematicSheet, patch_pair: PatchPair) {
        self.record_grouped_history(snapshot, patch_pair, None);
    }

    pub(crate) fn record_grouped_history(
        &mut self,
        snapshot: SchematicSheet,
        patch_pair: PatchPair,
        group: Option<UndoGroupId>,
    ) {
        // Inside `execute_batch` the individual commands must not each land
        // an entry: the batch pushes one coalesced entry of its own once the
        // loop is done. Skipping the redo clear below along with the push is
        // deliberate — a batch that turns out to have changed nothing records
        // nothing, so it must not consume a redoable step either. See
        // `exec/batch.rs` for why this is suppressed up front rather than
        // truncated away afterwards.
        if self.history_suppressed {
            self.absorb_into_transaction(patch_pair);
            return;
        }

        // Share every collection the step left unchanged with the entry
        // before it, so a long history holds one copy of each.
        let snapshot = SheetSnapshot::capture(snapshot, last_snapshot(self.history.undo_steps()));
        // The new entry drops the redo branch (and a save point inside it)
        // and evicts the oldest entry past the cap.
        self.history
            .record(HistoryStep::Edit(Box::new(HistoryEntry {
                snapshot,
                patch_pair,
                group,
            })));
    }
}
