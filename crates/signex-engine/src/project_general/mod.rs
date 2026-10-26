//! Command-driven editing of the project's General settings (#1202).
//!
//! The Project tab's General section edits two `.snxprj` fields, `name` and
//! `description`, in place on the project's one in-memory `ProjectData`.
//! Like the net-class table, the settings are tiny, so the history keeps
//! snapshots rather than inverse commands. A snapshot holds only the fields
//! this section owns: undo never reverts what another writer (a library add,
//! a net class, a rule change) did to the same project.
//!
//! Text fields are edited per keystroke. Consecutive edits of one field
//! coalesce into one undo step (a "run"); the run closes on undo, redo, an
//! edit of the other field, or [`ProjectGeneralHistory::close_run`].

use serde::{Deserialize, Serialize};
use signex_types::project::ProjectData;

/// How many undo steps are kept.
pub const HISTORY_LIMIT: usize = 100;

/// A field of the General section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectGeneralField {
    Name,
    Description,
}

/// One edit of the General section. Plain data, so a script can emit it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "command", content = "text", rename_all = "snake_case")]
pub enum ProjectGeneralCommand {
    /// The project name as typed. It becomes the `.snxprj` file stem when
    /// the project is saved; validating it is the save's job.
    SetName(String),
    /// The description as typed; an empty text clears it.
    SetDescription(String),
}

impl ProjectGeneralCommand {
    pub fn field(&self) -> ProjectGeneralField {
        match self {
            Self::SetName(_) => ProjectGeneralField::Name,
            Self::SetDescription(_) => ProjectGeneralField::Description,
        }
    }
}

/// The fields the General section owns, as one undo snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
struct GeneralSnapshot {
    name: String,
    description: Option<String>,
}

impl GeneralSnapshot {
    fn of(data: &ProjectData) -> Self {
        Self {
            name: data.name.clone(),
            description: data.description.clone(),
        }
    }

    /// Put these fields back, leaving every other field as it is now.
    fn restore(self, data: &mut ProjectData) {
        data.name = self.name;
        data.description = self.description;
    }
}

/// Undo/redo for one project's General settings.
#[derive(Debug, Clone, Default)]
pub struct ProjectGeneralHistory {
    undo: Vec<GeneralSnapshot>,
    redo: Vec<GeneralSnapshot>,
    /// The field whose edit run is open: its next edit joins the last step.
    open_run: Option<ProjectGeneralField>,
}

impl ProjectGeneralHistory {
    /// Apply `command` to `data`. Returns `true` when a field changed. A
    /// change opens a new undo step unless it continues the open run of the
    /// same field; a no-op records nothing.
    pub fn apply(&mut self, data: &mut ProjectData, command: ProjectGeneralCommand) -> bool {
        let field = command.field();
        let before = GeneralSnapshot::of(data);
        match command {
            ProjectGeneralCommand::SetName(text) => data.name = text,
            ProjectGeneralCommand::SetDescription(text) => {
                data.description = (!text.is_empty()).then_some(text);
            }
        }
        if GeneralSnapshot::of(data) == before {
            return false;
        }
        if self.open_run != Some(field) {
            self.undo.push(before);
            if self.undo.len() > HISTORY_LIMIT {
                self.undo.remove(0);
            }
            self.open_run = Some(field);
        }
        self.redo.clear();
        true
    }

    /// End the open edit run: the next edit is a new undo step.
    pub fn close_run(&mut self) {
        self.open_run = None;
    }

    /// Step back. `false` when there is nothing to undo.
    pub fn undo(&mut self, data: &mut ProjectData) -> bool {
        self.open_run = None;
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        self.redo.push(GeneralSnapshot::of(data));
        previous.restore(data);
        true
    }

    /// Step forward. `false` when there is nothing to redo.
    pub fn redo(&mut self, data: &mut ProjectData) -> bool {
        self.open_run = None;
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
