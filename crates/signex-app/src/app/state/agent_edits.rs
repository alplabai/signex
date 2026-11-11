//! What an agent's edit list (#2178) keeps in the app while it runs and after:
//! the sheet it targets, the repaint it owes that sheet, and the label of each
//! undo step it made. See `docs/AGENT_EDITS.md`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use signex_engine::UndoGroupId;

use super::DocumentState;
use crate::app::agent::Origin;
use crate::schematic_runtime::RenderInvalidation;

/// The project's sheets as read for the first checked edit of a list: the
/// other sheets do not change while the list runs, so the next edits reuse them
/// instead of reading the project again (the target sheet is replaced by its
/// current state each time). `None` outside a list.
#[derive(Default)]
pub(crate) struct ProjectSetCache(pub(crate) Option<crate::app::project_sheets::ProjectSheetSet>);

impl std::fmt::Debug for ProjectSetCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.is_some() {
            "ProjectSetCache(read)"
        } else {
            "ProjectSetCache(empty)"
        })
    }
}

#[derive(Debug, Default)]
pub struct AgentEdits {
    /// The sheet an edit list is running on. Set only for the length of one
    /// `apply_edits` call; a placement reads and writes this sheet's engine
    /// instead of the active one, and what it would refresh waits in `owed`.
    pub(crate) target: Option<PathBuf>,
    /// Every sheet the running list edits (more than one when its edits name
    /// other sheets of the project); empty outside a list.
    pub(crate) touched: Vec<PathBuf>,
    /// Whether an edit list (not one placement naming a `document`) is running.
    pub(crate) listing: bool,
    /// The project read the list's checks share.
    pub(crate) project_set: std::cell::RefCell<ProjectSetCache>,
    /// The repaint the edits so far owe the target sheet.
    pub(crate) owed: RenderInvalidation,
    /// The Undo label of each undo step an agent made ("Agent: ...").
    pub(crate) labels: HashMap<UndoGroupId, String>,
    /// Who made each of those steps, for the steps an agent made.
    pub(crate) origins: HashMap<UndoGroupId, Origin>,
    /// The undo group the newest recorded step made; the journal reads it
    /// after a command to name the step.
    pub(crate) last_group: Option<UndoGroupId>,
}

impl AgentEdits {
    /// Remember an agent's undo step: its label and who made it.
    pub(crate) fn record_step(&mut self, group: UndoGroupId, label: String, origin: &Origin) {
        self.labels.insert(group, label);
        self.last_group = Some(group);
        if origin.is_agent() {
            self.origins.insert(group, origin.clone());
        }
    }

    /// Forget a step that was taken back.
    pub(crate) fn forget_step(&mut self, group: UndoGroupId) {
        self.labels.remove(&group);
        self.origins.remove(&group);
    }

    /// The label and origin of `group` as Messages says them, "Agent: x on
    /// y (agent, pid 12)"; `None` when it is not an agent's step.
    pub(crate) fn step_text(&self, group: UndoGroupId) -> Option<String> {
        let label = self.labels.get(&group)?;
        Some(match self.origins.get(&group).and_then(Origin::describe) {
            Some(origin) => format!("{label} ({origin})"),
            None => label.clone(),
        })
    }
}

impl DocumentState {
    /// The sheet an edit acts on: the agent's target while its list runs,
    /// else the active sheet.
    pub(crate) fn edit_path(&self) -> Option<&Path> {
        self.agent_edits
            .target
            .as_deref()
            .or(self.active_path.as_deref())
    }

    /// The engine of [`Self::edit_path`].
    pub(crate) fn edit_engine(&self) -> Option<&signex_engine::Engine> {
        self.engines.get(self.edit_path()?)
    }

    /// The engine of [`Self::edit_path`], for a write.
    pub(crate) fn edit_engine_mut(&mut self) -> Option<&mut signex_engine::Engine> {
        let path = self.edit_path()?.to_path_buf();
        self.engines.get_mut(&path)
    }

    /// Whether an agent's edit list is running.
    pub(crate) fn agent_edit_running(&self) -> bool {
        self.agent_edits.target.is_some()
    }
}
