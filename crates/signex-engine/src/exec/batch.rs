//! `Engine::execute_batch` — run N commands as one undoable step.
//!
//! Pasting 40 objects, or a find/replace that rewrites 12 labels, has to
//! undo as one action rather than forty. Until now that grouping lived in
//! an app-side marker stack that recorded how many engine steps each user
//! action was worth. That stack was global across the per-path engines and
//! was never cleared on tab switch, so its counts drifted out of step with
//! the history they were counting and the app could revert a document
//! without reporting it (#533). Grouping belongs next to the history it
//! groups, which is here.
//!
//! ## Why the inner writes are suppressed rather than truncated afterwards
//!
//! The obvious shape is to let each inner `execute` record its own entry
//! and then truncate the batch's entries away and push one coalesced entry
//! in their place. That silently erodes undo depth. `record_history` evicts
//! from the *front* once `history` reaches `MAX_HISTORY_ENTRIES`, so a batch
//! of 10 run against a full history evicts 10 genuine pre-batch entries on
//! the way in. Truncating the 10 transient entries afterwards cannot bring
//! those back: the user is left at depth 91 after an action that should have
//! cost one slot, and a batch longer than the cap wipes the pre-batch
//! history outright. Suppressing the inner writes up front means the
//! transient entries never exist, so the batch causes exactly one eviction —
//! the correct number. It also drops the truncate shape's reliance on the
//! unenforced "exactly one `record_history` per changed result" convention,
//! which a future handler could break without any test noticing.

use crate::*;

impl Engine {
    /// Execute `commands` as a single undoable step.
    ///
    /// One `HistoryEntry` is recorded for the whole batch: the snapshot is
    /// the document as it stood before the first command, and the
    /// `DocumentPatch` flags of every command that changed something are
    /// OR-ed together, so one invalidation covers everything the batch
    /// touched. Commands reporting `changed == false` contribute nothing; a
    /// batch in which nothing changed records no entry at all and leaves the
    /// redo stack intact, exactly as a single unchanged `execute` does.
    ///
    /// `semantic` is widened to [`SemanticPatch::DocumentReplaced`] when the
    /// batch mixes kinds, because no single variant describes the result.
    ///
    /// On `Err` the batch aborts transactionally — the document is restored,
    /// no history entry is written, and the redo stack is untouched, so the
    /// engine is left exactly as it was before the call. The error carries
    /// no accumulated patch, so a caller that repaints defensively after a
    /// failure must invalidate [`DocumentPatch::FULL`].
    ///
    /// Batches do not nest. A nested call would consume the outer batch's
    /// history entry, so it trips a `debug_assert`.
    ///
    /// No error is logged here. `signex-engine` has no logging dependency,
    /// and the `Result` is the observability: `unused_must_use` is denied
    /// workspace-wide, so no caller can drop it silently.
    pub fn execute_batch(
        &mut self,
        commands: impl IntoIterator<Item = Command>,
    ) -> Result<CommandResult, EngineError> {
        self.execute_batch_grouped(commands, None)
    }

    /// [`Engine::execute_batch`] whose history entry is stamped with
    /// `group`, so an app-level step that spans several sheets (a
    /// project-wide replace) can undo and redo every sheet together. `None`
    /// is the ungrouped batch. Everything else — transactional abort, no
    /// entry for an unchanged batch, no nesting — is `execute_batch`'s.
    pub fn execute_batch_grouped(
        &mut self,
        commands: impl IntoIterator<Item = Command>,
        group: Option<UndoGroupId>,
    ) -> Result<CommandResult, EngineError> {
        debug_assert!(
            !self.history_suppressed,
            "execute_batch does not nest: the inner batch would consume the outer batch's history entry"
        );

        self.refuse_if_read_only(None)?;
        let base_revision = self.revision;
        self.bump_revision();
        let before = self.document.clone();

        // Everything between here and the reset runs with `record_history`
        // disabled. `run_batch` exists so the `?` inside its loop cannot
        // return past this line with the flag still set.
        self.history_suppressed = true;
        let coalesced = self.run_batch(commands);
        self.history_suppressed = false;
        // The inner commands each bumped; the batch is one edit.
        self.base_revision = base_revision;

        let patch_pair = match coalesced {
            Ok(Some(patch_pair)) => patch_pair,
            Ok(None) => return Ok(CommandResult::unchanged()),
            Err(error) => {
                // Nothing reached `history` and `redo_stack` was never
                // cleared, so restoring the pre-image is the whole rollback.
                self.document = before;
                return Err(error);
            }
        };

        self.record_grouped_history(before, patch_pair, group);

        Ok(CommandResult::changed(patch_pair))
    }

    /// [`Engine::execute`] without a history entry, for maintenance applied
    /// while a sheet loads (the user did not make the edit, so Undo must not
    /// offer it). Load-time only: the pre-edit snapshots already on the undo
    /// or redo stack would restore documents that predate this edit, so it
    /// is refused with [`EngineError::UnrecordedEditRefused`] once either
    /// stack holds an entry, and when called inside a batch. The checks are
    /// real (release builds too); the engine is left untouched on refusal.
    pub fn execute_unrecorded(&mut self, cmd: Command) -> Result<CommandResult, EngineError> {
        if self.history_suppressed || !self.history.is_empty() || !self.redo_stack.is_empty() {
            return Err(EngineError::UnrecordedEditRefused);
        }
        self.history_suppressed = true;
        let result = self.execute(cmd);
        self.history_suppressed = false;
        // No history step can reproduce the file now: the edit has no entry.
        if result.as_ref().is_ok_and(|r| r.changed) {
            self.saved_point = None;
        }
        result
    }

    /// The suppressed half of [`Engine::execute_batch`].
    ///
    /// Returns the coalesced patch, or `None` when no command in the batch
    /// changed anything.
    fn run_batch(
        &mut self,
        commands: impl IntoIterator<Item = Command>,
    ) -> Result<Option<PatchPair>, EngineError> {
        let mut document = DocumentPatch::NONE;
        let mut semantic: Option<SemanticPatch> = None;
        let mut semantics_differ = false;

        for command in commands {
            let Some(patch_pair) = self.execute(command)?.patch_pair else {
                continue;
            };

            document |= patch_pair.document;

            match semantic {
                None => semantic = Some(patch_pair.semantic),
                Some(first) if first != patch_pair.semantic => semantics_differ = true,
                Some(_) => {}
            }
        }

        Ok(semantic.map(|first| PatchPair {
            semantic: if semantics_differ {
                SemanticPatch::DocumentReplaced
            } else {
                first
            },
            document,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::MAX_HISTORY_ENTRIES;
    use crate::test_support::test_sheet;
    use signex_types::schematic::{HAlign, Label, LabelType, NoConnect, Point, VAlign};

    fn engine() -> Engine {
        Engine::new(test_sheet()).unwrap()
    }

    fn place_no_connect() -> Command {
        Command::PlaceNoConnect {
            no_connect: NoConnect {
                uuid: uuid::Uuid::now_v7(),
                position: Point::new(0.0, 0.0),
            },
        }
    }

    #[test]
    fn history_cap_evicts_oldest_and_undo_walks_newest_first() {
        let mut engine = engine();
        let extra = 5;
        for _ in 0..MAX_HISTORY_ENTRIES + extra {
            engine.execute(place_no_connect()).unwrap();
        }
        assert_eq!(engine.history.len(), MAX_HISTORY_ENTRIES);
        assert_eq!(engine.undo_position(), MAX_HISTORY_ENTRIES + extra);

        for _ in 0..MAX_HISTORY_ENTRIES {
            assert!(engine.undo().unwrap().is_some());
        }
        // The evicted steps are gone: the sheet keeps the first `extra`.
        assert!(engine.undo().unwrap().is_none());
        assert_eq!(engine.document().no_connects.len(), extra);
        assert_eq!(engine.undo_position(), extra);
    }

    fn place_label() -> Command {
        Command::PlaceLabel {
            label: Label {
                unknown_columns: Default::default(),
                uuid: uuid::Uuid::now_v7(),
                text: "NET".to_string(),
                position: Point::new(0.0, 0.0),
                rotation: 0.0,
                label_type: LabelType::Net,
                shape: "input".to_string(),
                font_size: 1.27,
                justify: HAlign::Left,
                justify_v: VAlign::Bottom,
                font_family: None,
                locked: false,
            },
        }
    }

    /// `DeleteSelection` with nothing selected takes the `if !changed` early
    /// return, so it is the cheapest command that reports
    /// `changed == false`.
    fn no_op() -> Command {
        Command::DeleteSelection { items: Vec::new() }
    }

    #[test]
    fn a_grouped_batch_stamps_its_entry_and_survives_undo_and_redo() {
        let mut engine = engine();
        engine
            .execute_batch_grouped([place_label(), place_label()], Some(UndoGroupId::new(9)))
            .unwrap();
        assert_eq!(engine.next_undo_group(), Some(UndoGroupId::new(9)));
        engine.undo().unwrap();
        assert_eq!(engine.next_redo_group(), Some(UndoGroupId::new(9)));
        engine.redo().unwrap();
        assert_eq!(engine.next_undo_group(), Some(UndoGroupId::new(9)));
    }

    #[test]
    fn a_batch_of_three_places_is_one_undo_step() {
        let mut engine = engine();

        let result = engine
            .execute_batch([place_no_connect(), place_no_connect(), place_no_connect()])
            .unwrap();

        assert!(result.changed);
        assert_eq!(engine.document().no_connects.len(), 3);
        assert_eq!(engine.history.len(), 1);

        assert!(engine.undo().unwrap().is_some());

        assert_eq!(engine.document().no_connects.len(), 0);
        assert!(!engine.can_undo());
    }

    #[test]
    fn an_empty_batch_is_unchanged() {
        let mut engine = engine();

        let result = engine.execute_batch(Vec::new()).unwrap();

        assert!(!result.changed);
        assert!(result.patch_pair.is_none());
        assert!(!engine.can_undo());
    }

    #[test]
    fn redo_restores_the_whole_batch() {
        let mut engine = engine();

        engine
            .execute_batch([place_no_connect(), place_no_connect()])
            .unwrap();
        engine.undo().unwrap();

        assert_eq!(engine.document().no_connects.len(), 0);

        assert!(engine.redo().unwrap().is_some());

        assert_eq!(engine.document().no_connects.len(), 2);
        assert!(!engine.can_redo());
    }

    #[test]
    fn a_batch_leaves_earlier_history_reachable() {
        let mut engine = engine();

        engine.execute(place_no_connect()).unwrap();
        engine
            .execute_batch([place_no_connect(), place_no_connect()])
            .unwrap();

        assert_eq!(engine.document().no_connects.len(), 3);
        assert_eq!(engine.history.len(), 2);

        engine.undo().unwrap();
        assert_eq!(engine.document().no_connects.len(), 1);

        engine.undo().unwrap();
        assert_eq!(engine.document().no_connects.len(), 0);
        assert!(!engine.can_undo());
    }

    #[test]
    fn a_batch_that_changes_nothing_records_no_entry_and_keeps_redo() {
        let mut engine = engine();

        engine.execute(place_no_connect()).unwrap();
        engine.undo().unwrap();

        // One entry parked on the redo stack, history empty.
        assert!(engine.can_redo());
        assert!(!engine.can_undo());

        let result = engine.execute_batch([no_op(), no_op(), no_op()]).unwrap();

        assert!(!result.changed);
        assert!(result.patch_pair.is_none());
        assert!(!engine.can_undo());
        // A no-op batch must not eat a redoable step — the suppressed
        // `record_history` skips `redo_stack.clear()` along with the push.
        assert!(engine.can_redo());
    }

    #[test]
    fn a_changed_batch_clears_the_redo_stack() {
        let mut engine = engine();

        engine.execute(place_no_connect()).unwrap();
        engine.undo().unwrap();
        assert!(engine.can_redo());

        engine.execute_batch([place_no_connect()]).unwrap();

        assert!(!engine.can_redo());
    }

    #[test]
    fn the_returned_document_patch_is_the_union_of_the_batch() {
        let mut engine = engine();

        let result = engine
            .execute_batch([place_no_connect(), place_label()])
            .unwrap();

        let patch_pair = result.patch_pair.unwrap();

        assert_eq!(
            patch_pair.document,
            DocumentPatch::NO_CONNECTS | DocumentPatch::LABELS
        );
        assert_eq!(engine.history[0].patch_pair.document, patch_pair.document);
    }

    #[test]
    fn semantic_is_kept_when_every_command_agrees() {
        let mut engine = engine();

        // Both arms report `ObjectPlaced`.
        let result = engine
            .execute_batch([place_no_connect(), place_label()])
            .unwrap();

        assert_eq!(
            result.patch_pair.unwrap().semantic,
            SemanticPatch::ObjectPlaced
        );
    }

    #[test]
    fn semantic_is_widened_to_document_replaced_when_the_batch_mixes_kinds() {
        let mut engine = engine();

        // `ObjectPlaced` from the placement, `StyleUpdated` from the paper
        // change: no single variant describes the pair.
        let result = engine
            .execute_batch([
                place_no_connect(),
                Command::SetPaperSize {
                    paper_size: "A3".to_string(),
                },
            ])
            .unwrap();

        let patch_pair = result.patch_pair.unwrap();

        assert_eq!(patch_pair.semantic, SemanticPatch::DocumentReplaced);
        assert_eq!(
            patch_pair.document,
            DocumentPatch::NO_CONNECTS | DocumentPatch::PAPER
        );
    }

    #[test]
    fn a_batch_at_the_history_cap_costs_exactly_one_undo_slot() {
        // Regression guard for the shape this module rejects. Recording the
        // batch's commands individually and truncating them away afterwards
        // would evict `BATCH` genuine pre-batch entries from the front of a
        // full history and leave the user at
        // `MAX_HISTORY_ENTRIES - BATCH + 1`. Suppressing the inner writes
        // costs exactly one eviction.
        const BATCH: usize = 10;

        let mut engine = engine();

        for _ in 0..MAX_HISTORY_ENTRIES {
            engine.execute(place_no_connect()).unwrap();
        }
        assert_eq!(engine.history.len(), MAX_HISTORY_ENTRIES);
        assert_eq!(engine.document().no_connects.len(), MAX_HISTORY_ENTRIES);

        engine
            .execute_batch(std::iter::repeat_with(place_no_connect).take(BATCH))
            .unwrap();

        // One entry evicted for the one entry pushed. The truncate shape
        // would leave `MAX_HISTORY_ENTRIES - BATCH + 1` == 91 here.
        assert_eq!(engine.history.len(), MAX_HISTORY_ENTRIES);
        assert_eq!(
            engine.document().no_connects.len(),
            MAX_HISTORY_ENTRIES + BATCH
        );

        // Drain every undo step. The oldest reachable state is the one whose
        // snapshot survived the single eviction: one placement in.
        let mut undone = 0;
        while engine.undo().unwrap().is_some() {
            undone += 1;
        }

        assert_eq!(undone, MAX_HISTORY_ENTRIES);
        assert_eq!(engine.document().no_connects.len(), 1);
    }

    #[test]
    fn a_batch_longer_than_the_history_cap_is_still_one_entry() {
        let mut engine = engine();

        let count = MAX_HISTORY_ENTRIES + 50;
        engine
            .execute_batch(std::iter::repeat_with(place_no_connect).take(count))
            .unwrap();

        assert_eq!(engine.history.len(), 1);
        assert_eq!(engine.document().no_connects.len(), count);

        engine.undo().unwrap();

        assert_eq!(engine.document().no_connects.len(), 0);
        assert!(!engine.can_undo());
    }

    #[test]
    fn unchanged_commands_inside_a_batch_do_not_dilute_the_patch() {
        let mut engine = engine();

        let result = engine
            .execute_batch([no_op(), place_no_connect(), no_op()])
            .unwrap();

        let patch_pair = result.patch_pair.unwrap();

        assert!(result.changed);
        assert_eq!(patch_pair.semantic, SemanticPatch::ObjectPlaced);
        assert_eq!(patch_pair.document, DocumentPatch::NO_CONNECTS);
        assert_eq!(engine.history.len(), 1);
    }

    #[test]
    fn an_unrecorded_command_changes_the_document_but_leaves_no_undo_step() {
        let mut engine = engine();

        let result = engine.execute_unrecorded(place_no_connect()).unwrap();

        assert!(result.changed);
        assert_eq!(engine.document().no_connects.len(), 1);
        assert!(!engine.can_undo());
        engine.execute(place_no_connect()).unwrap();
        assert_eq!(engine.history.len(), 1, "later commands still record");
    }

    #[test]
    fn execute_unrecorded_is_refused_once_history_exists() {
        let mut engine = engine();
        engine.execute(place_no_connect()).unwrap();

        let refused = engine.execute_unrecorded(place_no_connect());

        assert!(matches!(refused, Err(EngineError::UnrecordedEditRefused)));
        assert_eq!(engine.document().no_connects.len(), 1, "untouched");

        engine.undo().unwrap();
        assert!(engine.can_redo());
        assert!(matches!(
            engine.execute_unrecorded(place_no_connect()),
            Err(EngineError::UnrecordedEditRefused)
        ));
    }

    #[test]
    fn execute_unrecorded_is_refused_inside_a_batch() {
        let mut engine = engine();
        engine.history_suppressed = true;
        assert!(matches!(
            engine.execute_unrecorded(place_no_connect()),
            Err(EngineError::UnrecordedEditRefused)
        ));
    }

    #[test]
    fn the_suppression_flag_is_cleared_so_later_commands_still_record() {
        let mut engine = engine();

        engine.execute_batch([place_no_connect()]).unwrap();
        engine.execute(place_no_connect()).unwrap();

        assert!(!engine.history_suppressed);
        assert_eq!(engine.history.len(), 2);
    }
}

#[cfg(test)]
mod snapshot_cost_tests {
    use super::*;
    use crate::test_support::test_sheet;
    use signex_types::schematic::{NoConnect, Point};

    fn place() -> Command {
        Command::PlaceNoConnect {
            no_connect: NoConnect {
                uuid: uuid::Uuid::now_v7(),
                position: Point::new(0.0, 0.0),
            },
        }
    }

    #[test]
    fn a_batch_of_n_takes_one_sheet_snapshot_not_n_plus_one() {
        let mut engine = Engine::new(test_sheet()).unwrap();
        engine
            .execute_batch(std::iter::repeat_with(place).take(8))
            .unwrap();
        assert_eq!(engine.history.len(), 1);
        assert_eq!(engine.pre_image_clones, 0, "inner commands must not clone");
    }

    #[test]
    fn a_single_command_takes_one_snapshot() {
        let mut engine = Engine::new(test_sheet()).unwrap();
        engine.execute(place()).unwrap();
        assert_eq!(engine.pre_image_clones, 1);
    }

    #[test]
    fn a_command_that_changes_nothing_adds_no_undo_entry() {
        let mut engine = Engine::new(test_sheet()).unwrap();
        engine.execute(place()).unwrap();
        engine.undo().unwrap();
        let result = engine
            .execute(Command::DeleteSelection { items: Vec::new() })
            .unwrap();
        assert!(!result.changed);
        assert!(!engine.can_undo());
        assert!(engine.can_redo(), "redo survives an unchanged command");
    }
}
