//! What an agent's edit list (#2178) keeps in the app while it runs and after:
//! the sheet it targets, the repaint it owes that sheet, and the label of each
//! undo step it made. See `docs/AGENT_EDITS.md`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use signex_engine::UndoGroupId;

use super::DocumentState;
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
    /// Whether an edit list (not one placement naming a `document`) is running.
    pub(crate) listing: bool,
    /// The project read the list's checks share.
    pub(crate) project_set: std::cell::RefCell<ProjectSetCache>,
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
