//! Command-driven editing of the project's General settings (#1202).
//!
//! The Project tab's General section edits the `.snxprj` `description` in
//! place on the project's one in-memory `ProjectData`. The project name is
//! not edited here: it is the file stem, changed only by the Rename flow.
//! Like the net-class table, the settings are tiny, so the history keeps
//! snapshots rather than inverse commands. A snapshot holds only the fields
//! this section owns: undo never reverts what another writer (a library add,
//! a net class, a rule change, a rename) did to the same project.
//!
//! Text is edited per keystroke. Consecutive edits coalesce into one undo
//! step (a "run"); the run closes on undo, redo, or
//! [`ProjectGeneralHistory::close_run`] (a section switch, a save).

use serde::{Deserialize, Serialize};
use signex_types::project::ProjectData;

/// How many undo steps are kept.
pub const HISTORY_LIMIT: usize = 100;

/// One edit of the General section. Plain data, so a script can emit it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "command", content = "text", rename_all = "snake_case")]
pub enum ProjectGeneralCommand {
    /// The description as typed; an empty text clears it.
    SetDescription(String),
}

/// The fields the General section owns, as one undo snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
struct GeneralSnapshot {
    description: Option<String>,
}

impl GeneralSnapshot {
    fn of(data: &ProjectData) -> Self {
        Self {
            description: data.description.clone(),
        }
    }

    /// Put these fields back, leaving every other field as it is now.
    fn restore(self, data: &mut ProjectData) {
        data.description = self.description;
    }
}

/// Undo/redo for one project's General settings.
#[derive(Debug, Clone, Default)]
pub struct ProjectGeneralHistory {
    undo: Vec<GeneralSnapshot>,
    redo: Vec<GeneralSnapshot>,
    /// An edit run is open: the next edit joins the last step.
    run_open: bool,
}

impl ProjectGeneralHistory {
    /// Apply `command` to `data`. Returns `true` when a field changed. A
    /// change opens a new undo step unless it continues the open run; a
    /// no-op records nothing.
    pub fn apply(&mut self, data: &mut ProjectData, command: ProjectGeneralCommand) -> bool {
        let before = GeneralSnapshot::of(data);
        match command {
            ProjectGeneralCommand::SetDescription(text) => {
                data.description = (!text.is_empty()).then_some(text);
            }
        }
        if GeneralSnapshot::of(data) == before {
            return false;
        }
        if !self.run_open {
            self.undo.push(before);
            if self.undo.len() > HISTORY_LIMIT {
                self.undo.remove(0);
            }
            self.run_open = true;
        }
        self.redo.clear();
        true
    }

    /// End the open edit run: the next edit is a new undo step.
    pub fn close_run(&mut self) {
        self.run_open = false;
    }

    /// Step back. `false` when there is nothing to undo.
    pub fn undo(&mut self, data: &mut ProjectData) -> bool {
        self.run_open = false;
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        self.redo.push(GeneralSnapshot::of(data));
        previous.restore(data);
        true
    }

    /// Step forward. `false` when there is nothing to redo.
    pub fn redo(&mut self, data: &mut ProjectData) -> bool {
        self.run_open = false;
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(GeneralSnapshot::of(data));
        next.restore(data);
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests;
