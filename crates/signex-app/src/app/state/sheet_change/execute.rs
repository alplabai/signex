//! Running an edit or a history step on a sheet's engine: the gateway's form
//! (the sheet edits act on, with the read-only refusal and the lock rules)
//! and the by-path form (`execute_on_sheet`, `step_sheet_on`).

use std::path::Path;

use signex_engine::{Command, DocumentPatch, Engine, EngineError};

use super::notices::{engine_events, error_line, locked_skips, warning};
use super::{DocumentState, Settled, SheetEdit, SheetStep};
use crate::app::core::event::{ChangeCause, CoreEvent, DocumentChanged};
use crate::app::core::mutation_outcome::MutationOutcome;

/// `commands` without what the lock would refuse (see
/// `Engine::without_locked`), and the references skipped.
pub(crate) fn locked_items_pruned(
    engine: &Engine,
    commands: Vec<Command>,
) -> (Vec<Command>, Vec<String>) {
    let mut kept = Vec::with_capacity(commands.len());
    let mut skipped = Vec::new();
    for command in commands {
        let (command, references) = engine.without_locked(command);
        kept.extend(command);
        skipped.extend(references);
    }
    skipped.sort();
    skipped.dedup();
    (kept, skipped)
}

fn document_patch(pair: Option<signex_engine::PatchPair>) -> DocumentPatch {
    pair.map_or(DocumentPatch::NONE, |pair| pair.document)
}

impl DocumentState {
    /// The engine an edit acts on, ready for it: the active one, or the sheet
    /// an agent's edit list targets (#2178). Refused when the sheet is
    /// read-only or there is none.
    fn prepare_edit_engine(&mut self, events: &mut Vec<CoreEvent>) -> Result<&mut Engine, String> {
        if let Some(path) = self.edit_path().map(Path::to_path_buf)
            && let Some(refusal) = self.refuse_read_only_at(&path)
        {
            events.extend(refusal.notice.map(CoreEvent::from));
            return Err(refusal.reason);
        }
        self.edit_engine_mut()
            .ok_or_else(|| "There is no active schematic.".to_string())
    }

    /// The gateway's batch: one engine history entry for every command, a
    /// locked symbol skipped rather than allowed to cancel the batch.
    pub(crate) fn execute_batch_on_edit_path(&mut self, commands: Vec<Command>) -> SheetEdit {
        let mut events = Vec::new();
        if commands.is_empty() {
            return SheetEdit::unchanged(events);
        }
        let engine = match self.prepare_edit_engine(&mut events) {
            Ok(engine) => engine,
            Err(reason) => return SheetEdit::failed(reason, events),
        };
        let (commands, skipped) = locked_items_pruned(engine, commands);
        events.extend(locked_skips(&skipped));
        if commands.is_empty() {
            return SheetEdit::unchanged(events);
        }
        let batch = engine.execute_batch(commands);
        events.extend(engine_events(engine));
        let patch = match batch {
            Ok(result) => document_patch(result.patch_pair),
            Err(error) => {
                let reason = error.to_string();
                events.push(error_line("Engine command execution failed", error));
                return SheetEdit::failed(reason, events);
            }
        };
        self.settle_edit(patch, events)
    }

    /// The gateway's single command, recorded as an undo step or not
    /// (`record_history`). Held items are skipped and named, a wire on a
    /// locked pin included; a command that is nothing but locked items still
    /// reaches the engine, so the user is told why nothing happened.
    pub(crate) fn execute_on_edit_path(
        &mut self,
        command: Command,
        record_history: bool,
    ) -> SheetEdit {
        let mut events = Vec::new();
        let engine = match self.prepare_edit_engine(&mut events) {
            Ok(engine) => engine,
            Err(reason) => return SheetEdit::failed(reason, events),
        };
        let names_locked = engine.names_locked_item(&command);
        let command = if names_locked || command.is_selection_command() {
            let (mut kept, skipped) = locked_items_pruned(engine, vec![command.clone()]);
            match kept.pop() {
                Some(pruned) => {
                    events.extend(locked_skips(&skipped));
                    pruned
                }
                None if names_locked => command,
                None => {
                    events.extend(locked_skips(&skipped));
                    return SheetEdit::unchanged(events);
                }
            }
        } else {
            command
        };
        let executed = if record_history {
            engine.execute(command)
        } else {
            engine.execute_unrecorded(command)
        };
        events.extend(engine_events(engine));
        match executed {
            Ok(result) if result.changed => {
                self.settle_edit(document_patch(result.patch_pair), events)
            }
            Ok(_) => SheetEdit::unchanged(events),
            Err(error) => {
                let reason = error.to_string();
                if matches!(error, EngineError::ItemLocked { .. }) {
                    // Same level as the skip notice a batch raises.
                    events.push(warning(reason.clone()));
                } else {
                    events.push(error_line("Engine command failed", error));
                }
                SheetEdit::failed(reason, events)
            }
        }
    }

    /// One undo (`undo`) or redo of the active engine's own history, not a
    /// grouped entry. `None` when no engine backs the active document.
    pub(crate) fn step_active_history(&mut self, undo: bool) -> Option<SheetStep> {
        let path = self.active_path.clone()?;
        let engine = self.engines.get_mut(&path)?;
        let (stepped, failure) = if undo {
            (engine.undo(), "Engine undo failed")
        } else {
            (engine.redo(), "Engine redo failed")
        };
        let mut events = Vec::new();
        let patch = match stepped {
            Ok(Some(pair)) => pair.document,
            Ok(None) => return Some(Self::not_stepped(events)),
            Err(error) => {
                events.push(error_line(failure, error));
                return Some(Self::not_stepped(events));
            }
        };
        let settled = self.settle_active_into(patch, ChangeCause::Step, &mut events);
        if settled != Settled::Shown {
            // The history moved all the same: an empty patch on the active
            // canvas only asks for the library drift check.
            let patch = DocumentPatch::NONE;
            let cause = ChangeCause::Step;
            events.push(DocumentChanged { path, patch, cause }.into());
        }
        Some(SheetStep {
            stepped: settled != Settled::Unchanged,
            refusal: None,
            events,
        })
    }

    /// Run `command` on the engine of the sheet at `path` as one undo step,
    /// with no read-only refusal or lock pruning of its own. A command that
    /// changed nothing returns with no tail.
    pub(crate) fn execute_at(&mut self, path: &Path, command: Command) -> SheetEdit {
        let mut events = Vec::new();
        let Some(engine) = self.engines.get_mut(path) else {
            return SheetEdit::failed(format!("{} is not loaded", path.display()), events);
        };
        let executed = engine.execute(command);
        events.extend(engine_events(engine));
        let patch = match executed {
            Ok(result) if result.changed => document_patch(result.patch_pair),
            Ok(_) => return SheetEdit::unchanged(events),
            Err(error) => {
                let reason = error.to_string();
                events.push(error_line("Engine command failed", error));
                return SheetEdit::failed(reason, events);
            }
        };
        self.settle_at_into(path, patch, ChangeCause::Edit, &mut events);
        let outcome = if patch == DocumentPatch::NONE {
            MutationOutcome::Unchanged
        } else {
            MutationOutcome::Changed
        };
        SheetEdit {
            outcome,
            refusal: None,
            events,
        }
    }

    /// One undo (`undo`) or redo of the engine history of the sheet at
    /// `path`. `stepped` is `false` when there is nothing to step; a grouped
    /// entry is refused, it moves other sheets too.
    pub(crate) fn step_history_at(&mut self, path: &Path, undo: bool) -> SheetStep {
        let mut events = Vec::new();
        let Some(engine) = self.engines.get(path) else {
            return Self::step_refused(format!("{} is not loaded", path.display()), events);
        };
        let group = if undo {
            engine.next_undo_group()
        } else {
            engine.next_redo_group()
        };
        if let Some(group) = group {
            let reason = if self.agent_edits.labels.contains_key(&group) {
                "the next step is an agent's edit step; take it back with `revert`".to_string()
            } else {
                "the next step belongs to a project-wide run (annotate, replace or re-prefix) \
                 and moves other sheets too; undo it from the sheet it was run on"
                    .to_string()
            };
            return Self::step_refused(reason, events);
        }
        let Some(engine) = self.engines.get_mut(path) else {
            return Self::step_refused(format!("{} is not loaded", path.display()), events);
        };
        let stepped = if undo { engine.undo() } else { engine.redo() };
        let patch = match stepped {
            Ok(Some(pair)) => pair.document,
            Ok(None) => return Self::not_stepped(events),
            Err(error) => {
                let reason = error.to_string();
                let failure = if undo {
                    "Engine undo failed"
                } else {
                    "Engine redo failed"
                };
                events.push(error_line(failure, error));
                return Self::step_refused(reason, events);
            }
        };
        self.settle_at_into(path, patch, ChangeCause::Step, &mut events);
        SheetStep {
            stepped: true,
            refusal: None,
            events,
        }
    }

    fn settle_edit(&mut self, patch: DocumentPatch, mut events: Vec<CoreEvent>) -> SheetEdit {
        let outcome = match self.settle_active_into(patch, ChangeCause::Edit, &mut events) {
            Settled::Unchanged => MutationOutcome::Unchanged,
            Settled::Owed | Settled::Shown => MutationOutcome::Changed,
        };
        SheetEdit {
            outcome,
            refusal: None,
            events,
        }
    }

    fn not_stepped(events: Vec<CoreEvent>) -> SheetStep {
        SheetStep {
            stepped: false,
            refusal: None,
            events,
        }
    }

    fn step_refused(reason: String, events: Vec<CoreEvent>) -> SheetStep {
        SheetStep {
            stepped: false,
            refusal: Some(reason),
            events,
        }
    }
}
