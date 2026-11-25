//! The core half of the schematic edit tails (epic #2326, slice 2): run an
//! edit or a history step on a sheet's engine, keep the document's own state
//! (dirty mark, ERC epoch, the agent's owed refresh) and return what happened
//! as events. The GUI half is `app::projector`.
//!
//! Only the tail wrappers in `scripts/sheet-change-wrappers.txt` call in here
//! (#2704); `scripts/check_sheet_change_seam.sh` keeps it that way and keeps
//! this module off the GUI's state and the diagnostics log.

mod execute;
mod group;
#[cfg(test)]
mod group_tests;
mod notices;
#[cfg(test)]
mod notices_tests;
mod run;
#[cfg(test)]
mod run_tests;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use signex_engine::DocumentPatch;

use super::DocumentState;
use crate::app::core::event::{
    ChangeCause, CoreEvent, DocumentChanged, Notice, NoticeLevel, read_only_refusal,
    read_only_refusal_message,
};
use crate::app::core::mutation_outcome::MutationOutcome;

pub(crate) use group::GroupStep;
pub(crate) use run::SheetRun;

/// An edit run by the core half: its outcome, the refusal text of a failed
/// one, and the events to project.
#[derive(Debug)]
#[must_use = "project the events"]
pub(crate) struct SheetEdit {
    pub(crate) outcome: MutationOutcome,
    pub(crate) refusal: Option<String>,
    pub(crate) events: Vec<CoreEvent>,
}

impl SheetEdit {
    fn unchanged(events: Vec<CoreEvent>) -> Self {
        Self {
            outcome: MutationOutcome::Unchanged,
            refusal: None,
            events,
        }
    }

    fn failed(reason: String, events: Vec<CoreEvent>) -> Self {
        Self {
            outcome: MutationOutcome::Failed,
            refusal: Some(reason),
            events,
        }
    }
}

/// A history step run by the core half. `stepped` is what the wrapper
/// returns; `refusal` the reason a by-path step was refused.
#[derive(Debug)]
#[must_use = "project the events"]
pub(crate) struct SheetStep {
    pub(crate) stepped: bool,
    pub(crate) refusal: Option<String>,
    pub(crate) events: Vec<CoreEvent>,
}

/// A read-only sheet refused an edit: the reason, and the Messages line the
/// first refusal of that engine raises.
#[derive(Debug)]
#[must_use = "log the notice"]
pub(crate) struct ReadOnlyRefusal {
    pub(crate) reason: String,
    pub(crate) notice: Option<Notice>,
}

/// What the active tail did with a patch.
#[derive(Debug, PartialEq, Eq)]
enum ActiveSettlement {
    /// Nothing to show: an empty patch, or no engine behind the active path.
    Unchanged,
    /// An agent's edit list is running: the refresh is owed to its end.
    Owed,
    /// Dirty mark synced and a `DocumentChanged` pushed.
    Shown,
}

impl DocumentState {
    /// The active tail (the gateway's): settle `patch` on the active sheet.
    #[must_use = "project the events"]
    pub(crate) fn settle_active_sheet(
        &mut self,
        patch: DocumentPatch,
        cause: ChangeCause,
    ) -> SheetEdit {
        let mut events = Vec::new();
        let outcome = match self.settle_active_into(patch, cause, &mut events) {
            ActiveSettlement::Unchanged => MutationOutcome::Unchanged,
            ActiveSettlement::Owed | ActiveSettlement::Shown => MutationOutcome::Changed,
        };
        SheetEdit {
            outcome,
            refusal: None,
            events,
        }
    }

    /// The by-path tail: settle `patch` on the sheet at `path`. Unlike the
    /// active tail it emits even for an empty patch or a sheet with no
    /// engine (#2645 keeps the two rule sets apart until it is decided).
    #[must_use = "project the events"]
    pub(crate) fn settle_sheet_at(
        &mut self,
        path: &Path,
        patch: DocumentPatch,
        cause: ChangeCause,
    ) -> Vec<CoreEvent> {
        let mut events = Vec::new();
        self.settle_at_into(path, patch, cause, &mut events);
        events
    }

    /// An edit of the active sheet made outside the engine's commands: the
    /// ERC epoch moves and the dirty mark follows the history. `false` when
    /// no engine backs the active document. No event: nothing to repaint.
    pub(crate) fn note_active_edit(&mut self) -> bool {
        self.note_sheet_edit();
        self.sync_active_history_dirty().is_some()
    }

    /// `Some(reason)` when the open sheet at `path` is read-only; the notice
    /// comes once per opened engine (`instance_id`).
    pub(crate) fn refuse_read_only_at(&mut self, path: &Path) -> Option<ReadOnlyRefusal> {
        let engine = self.engines.get(path)?;
        let document = engine.document();
        if !document.is_read_only() {
            return None;
        }
        let token = document.unknown.newer_minor_source.as_deref().unwrap_or("");
        let reason = read_only_refusal_message(Some(path), token);
        let instance = engine.instance_id();
        let notice = self
            .read_only_refusal_noted
            .insert(instance)
            .then(|| Notice::new(NoticeLevel::Warning, read_only_refusal(Some(path), token)));
        Some(ReadOnlyRefusal { reason, notice })
    }

    fn settle_active_into(
        &mut self,
        patch: DocumentPatch,
        cause: ChangeCause,
        events: &mut Vec<CoreEvent>,
    ) -> ActiveSettlement {
        if patch == DocumentPatch::NONE {
            return ActiveSettlement::Unchanged;
        }
        // An agent's edit list refreshes the sheet once, when it ends, and
        // never through the person's canvas (docs/AGENT_EDITS.md).
        if self.agent_edit_running() {
            self.agent_edits.owed |= patch;
            return ActiveSettlement::Owed;
        }
        if !patch.is_display_only() {
            self.note_sheet_edit();
        }
        let Some(path) = self.sync_active_history_dirty() else {
            return ActiveSettlement::Unchanged;
        };
        self.mark_compiled_stale(&path, patch);
        events.push(DocumentChanged { path, patch, cause }.into());
        ActiveSettlement::Shown
    }

    fn settle_at_into(
        &mut self,
        path: &Path,
        patch: DocumentPatch,
        cause: ChangeCause,
        events: &mut Vec<CoreEvent>,
    ) {
        // Inside an agent's edit list the refresh is owed once, at its end.
        // A step never runs there: `edit_refusal` refuses undo, redo and revert.
        if self.agent_edits.target.as_deref() == Some(path) {
            debug_assert_eq!(cause, ChangeCause::Edit, "a step inside an agent edit");
            self.agent_edits.owed |= patch;
            return;
        }
        if !patch.is_display_only() {
            self.note_sheet_edit();
        }
        self.sync_history_dirty(path);
        self.mark_compiled_stale(path, patch);
        let path = path.to_path_buf();
        events.push(DocumentChanged { path, patch, cause }.into());
    }

    /// Re-derive the active sheet's dirty mark from its engine history after
    /// a step (edit, undo, redo); its path, `None` when no engine backs the
    /// active document.
    fn sync_active_history_dirty(&mut self) -> Option<PathBuf> {
        let path = self.active_path.clone()?;
        if !self.engines.contains_key(&path) {
            return None;
        }
        self.sync_history_dirty(&path);
        Some(path)
    }
}
