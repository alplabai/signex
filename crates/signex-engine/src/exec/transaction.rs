//! `Engine::begin_transaction` / `commit_transaction` / `rollback_transaction`:
//! several `execute` and `execute_batch` calls that land as ONE undo entry, or
//! not at all (#2178).
//!
//! A caller that runs ordinary commands through their own handlers (an agent's
//! list of edits) cannot hand the engine one list up front, so the engine
//! keeps the pre-image and the coalesced patch while the commands run with
//! history suppressed, exactly as inside `execute_batch` (see `batch.rs` for
//! why the writes are suppressed rather than truncated afterwards).

use crate::*;

/// The state an open transaction keeps.
#[derive(Debug)]
pub(crate) struct Transaction {
    before: SchematicSheet,
    mark: RevisionMark,
    patch: Option<PatchPair>,
    /// A command inside the transaction failed, so only a rollback may end it.
    failed: bool,
    /// Notices already raised when it began: a rollback drops the rest.
    warnings: usize,
    unkept_pins: usize,
}

impl Transaction {
    /// Fold the patch of one more changed command into the coalesced one.
    fn absorb(&mut self, next: PatchPair) {
        self.patch = Some(match self.patch {
            None => next,
            Some(held) => PatchPair {
                semantic: if held.semantic == next.semantic {
                    held.semantic
                } else {
                    SemanticPatch::DocumentReplaced
                },
                document: held.document | next.document,
            },
        });
    }
}

impl Engine {
    /// Start collecting every following command into one undo entry.
    /// Refused when the sheet is read-only or a transaction or batch is
    /// already running; nothing changes then.
    pub fn begin_transaction(&mut self) -> Result<(), EngineError> {
        self.refuse_if_read_only(None)?;
        if self.history_suppressed || self.transaction.is_some() {
            return Err(EngineError::TransactionOpen);
        }
        // A run of typing in one cell ends here: the transaction is its own entry.
        self.field_edit_run = None;
        self.transaction = Some(Transaction {
            before: self.document.clone(),
            mark: self.revision_mark(),
            patch: None,
            failed: false,
            warnings: self.warnings.len(),
            unkept_pins: self.unkept_pins.len(),
        });
        self.history_suppressed = true;
        Ok(())
    }

    /// Whether a transaction is open.
    pub fn in_transaction(&self) -> bool {
        self.transaction.is_some()
    }

    /// Whether [`Self::commit_transaction`] would succeed: a transaction is
    /// open and no command inside it failed. A caller that commits several
    /// engines as one group checks every engine first, as a commit cannot be
    /// taken back once another engine has landed its entry.
    pub fn transaction_ready(&self) -> bool {
        self.transaction.as_ref().is_some_and(|tx| !tx.failed)
    }

    /// Record a command that failed inside the open transaction, so a commit
    /// is refused and the caller rolls back.
    pub fn fail_transaction(&mut self) {
        if let Some(tx) = self.transaction.as_mut() {
            tx.failed = true;
        }
    }

    /// End the transaction: one history entry stamped `group` for everything
    /// that changed, or none when nothing did (the revision then goes back
    /// to what it was). Refused after [`Self::fail_transaction`] or without a
    /// transaction; the caller rolls back.
    pub fn commit_transaction(
        &mut self,
        group: Option<UndoGroupId>,
    ) -> Result<CommandResult, EngineError> {
        match self.transaction.as_ref() {
            None => return Err(EngineError::NoTransaction),
            Some(tx) if tx.failed => return Err(EngineError::TransactionFailed),
            Some(_) => {}
        }
        let Some(tx) = self.transaction.take() else {
            return Err(EngineError::NoTransaction);
        };
        self.history_suppressed = false;
        // The commands inside each bumped; the transaction is one edit.
        self.base_revision = tx.mark.revision;
        match tx.patch {
            Some(patch_pair) => {
                self.record_grouped_history(tx.before, patch_pair, group);
                Ok(CommandResult::changed(patch_pair))
            }
            None => {
                self.restore_revision(tx.mark);
                Ok(CommandResult::unchanged())
            }
        }
    }

    /// Put the sheet and its revisions back to what they were at
    /// [`Self::begin_transaction`]: no history entry, redo untouched. A no-op
    /// without a transaction.
    pub fn rollback_transaction(&mut self) {
        let Some(tx) = self.transaction.take() else {
            return;
        };
        self.history_suppressed = false;
        self.document = tx.before;
        self.restore_revision(tx.mark);
        self.bump_connectivity_revision();
        // Notices about edits that never happened are not shown.
        self.warnings.truncate(tx.warnings);
        self.unkept_pins.truncate(tx.unkept_pins);
    }

    /// A batch run while a transaction is open: its commands join the
    /// transaction's one entry (their patches are absorbed as they record), and
    /// an error leaves the transaction failed for the caller to roll back.
    pub(crate) fn batch_in_transaction(
        &mut self,
        commands: impl IntoIterator<Item = Command>,
    ) -> Result<CommandResult, EngineError> {
        match self.run_batch(commands) {
            Ok(Some(patch_pair)) => Ok(CommandResult::changed(patch_pair)),
            Ok(None) => Ok(CommandResult::unchanged()),
            Err(error) => {
                self.fail_transaction();
                Err(error)
            }
        }
    }

    /// A changed command ran with history suppressed: inside a transaction its
    /// patch joins the coalesced one.
    pub(crate) fn absorb_into_transaction(&mut self, patch_pair: PatchPair) {
        if let Some(tx) = self.transaction.as_mut() {
            tx.absorb(patch_pair);
        }
    }
}

#[cfg(test)]
mod tests {
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

    fn engine() -> Engine {
        Engine::new(test_sheet()).unwrap()
    }

    #[test]
    fn several_commands_and_a_batch_are_one_undo_entry() {
        let mut engine = engine();
        engine.begin_transaction().unwrap();
        engine.execute(place()).unwrap();
        engine.execute_batch([place(), place()]).unwrap();
        engine.execute(place()).unwrap();
        let result = engine
            .commit_transaction(Some(UndoGroupId::new(4)))
            .unwrap();
        assert!(result.changed);
        assert_eq!(engine.document().no_connects.len(), 4);
        assert_eq!(engine.history.undo_len(), 1);
        assert_eq!(engine.next_undo_group(), Some(UndoGroupId::new(4)));
        engine.undo().unwrap();
        assert_eq!(engine.document().no_connects.len(), 0);
        assert!(!engine.can_undo());
    }

    /// The entry's patch is the union of every command's, and its semantic
    /// widens when the kinds differ: Undo of the entry repaints all of them.
    #[test]
    fn the_entry_of_a_mixed_transaction_carries_the_union_of_its_patches() {
        let mut engine = engine();
        engine.begin_transaction().unwrap();
        engine.execute(place()).unwrap();
        engine
            .execute(Command::SetPaperSize {
                paper_size: "A3".to_string(),
            })
            .unwrap();
        engine.commit_transaction(None).unwrap();
        let pair = engine.history.undo_steps()[0].edit().patch_pair;
        assert_eq!(
            pair.document,
            DocumentPatch::NO_CONNECTS | DocumentPatch::PAPER
        );
        assert_eq!(pair.semantic, SemanticPatch::DocumentReplaced);
        let undone = engine.undo().unwrap().unwrap();
        assert_eq!(undone.document, pair.document);

        engine.begin_transaction().unwrap();
        engine.execute(place()).unwrap();
        engine.execute(place()).unwrap();
        engine.commit_transaction(None).unwrap();
        let pair = engine.history.undo_steps()[0].edit().patch_pair;
        assert_eq!(pair.semantic, SemanticPatch::ObjectPlaced);
    }

    /// The commands inside each bump the revision; the transaction is one edit,
    /// so the revision before it is the base a render cache may copy from.
    #[test]
    fn a_committed_transaction_is_one_edit_for_the_base_revision() {
        let mut engine = engine();
        let before = engine.revision();
        engine.begin_transaction().unwrap();
        engine.execute(place()).unwrap();
        engine.execute(place()).unwrap();
        engine.commit_transaction(None).unwrap();
        assert_eq!(engine.base_revision(), before);
        assert_ne!(engine.revision(), before);
    }

    #[test]
    fn a_rollback_restores_the_sheet_the_history_and_the_revisions() {
        let mut engine = engine();
        engine.execute(place()).unwrap();
        let (revision, base) = (engine.revision(), engine.base_revision());
        engine.begin_transaction().unwrap();
        engine.execute(place()).unwrap();
        assert_eq!(engine.document().no_connects.len(), 2);
        engine.rollback_transaction();
        assert_eq!(engine.document().no_connects.len(), 1);
        assert_eq!(
            (engine.revision(), engine.base_revision()),
            (revision, base)
        );
        assert_eq!(engine.history.undo_len(), 1);
        assert!(!engine.in_transaction());
        // History records again afterwards.
        engine.execute(place()).unwrap();
        assert_eq!(engine.history.undo_len(), 2);
    }

    #[test]
    fn a_transaction_that_changed_nothing_records_nothing_and_keeps_redo() {
        let mut engine = engine();
        engine.execute(place()).unwrap();
        engine.undo().unwrap();
        let (revision, base) = (engine.revision(), engine.base_revision());
        engine.begin_transaction().unwrap();
        engine
            .execute(Command::DeleteSelection { items: Vec::new() })
            .unwrap();
        let result = engine.commit_transaction(None).unwrap();
        assert!(!result.changed);
        assert!(engine.can_redo());
        assert!(!engine.can_undo());
        assert_eq!(
            (engine.revision(), engine.base_revision()),
            (revision, base)
        );
    }

    #[test]
    fn a_failed_transaction_cannot_commit_and_a_second_cannot_open() {
        let mut engine = engine();
        engine.begin_transaction().unwrap();
        assert!(matches!(
            engine.begin_transaction(),
            Err(EngineError::TransactionOpen)
        ));
        engine.execute(place()).unwrap();
        engine.fail_transaction();
        assert!(matches!(
            engine.commit_transaction(None),
            Err(EngineError::TransactionFailed)
        ));
        engine.rollback_transaction();
        assert!(matches!(
            engine.commit_transaction(None),
            Err(EngineError::NoTransaction)
        ));
        assert!(engine.document().no_connects.is_empty());
    }

    #[test]
    fn a_transaction_is_ready_to_commit_only_while_open_and_not_failed() {
        let mut engine = engine();
        assert!(!engine.transaction_ready());
        engine.begin_transaction().unwrap();
        assert!(engine.transaction_ready());
        engine.fail_transaction();
        assert!(!engine.transaction_ready());
        engine.rollback_transaction();
        assert!(!engine.transaction_ready());
    }

    #[test]
    fn a_read_only_sheet_refuses_to_begin() {
        let mut sheet = test_sheet();
        sheet.unknown.newer_minor_source = Some("snxsch/9.9".into());
        let mut engine = Engine::new(sheet).unwrap();
        assert!(matches!(
            engine.begin_transaction(),
            Err(EngineError::ReadOnly)
        ));
        assert!(!engine.in_transaction());
    }
}
