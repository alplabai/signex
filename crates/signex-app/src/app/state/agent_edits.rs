//! What an agent's edit list (#2178) keeps in the app while it runs and after:
//! the sheet it targets, the repaint it owes that sheet, and the label of each
//! undo step it made. See `docs/AGENT_EDITS.md`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use signex_engine::UndoGroupId;

use super::DocumentState;
use crate::schematic_runtime::RenderInvalidation;

#[derive(Debug, Default)]
pub struct AgentEdits {
    /// The sheet an edit list is running on. Set only for the length of one
    /// `apply_edits` call; a placement reads and writes this sheet's engine
    /// instead of the active one, and what it would refresh waits in `owed`.
    pub(crate) target: Option<PathBuf>,
    /// The repaint the edits so far owe the target sheet.
    pub(crate) owed: RenderInvalidation,
    /// The Undo label of each undo step an agent made ("Agent: ...").
    pub(crate) labels: HashMap<UndoGroupId, String>,
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
