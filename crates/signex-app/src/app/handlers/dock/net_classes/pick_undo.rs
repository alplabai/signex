//! Ctrl+Z / Ctrl+Y for net-class picks (#1220). See
//! [`crate::app::state::net_class_picks`] for the ordering rule.

use crate::app::state::net_class_picks::NetClassPickStep;

impl crate::app::Signex {
    /// Remember a pick just applied to `project`'s table, made from the active
    /// sheet. `depth` is the project's class-history depth right after it.
    pub(super) fn record_net_class_pick(&mut self, project: crate::app::ProjectId, depth: usize) {
        let Some(path) = self.document_state.active_path.clone() else {
            return;
        };
        let at = self
            .document_state
            .active_engine()
            .map_or(0, |engine| engine.undo_position());
        let steps = self.document_state.net_class_picks.entry(path).or_default();
        steps.redo.clear();
        steps.undo.push(NetClassPickStep { project, at, depth });
    }

    /// Forget a sheet's redoable picks: a new sheet edit ends that branch.
    pub(crate) fn clear_net_class_pick_redo(&mut self, path: &std::path::Path) {
        if let Some(steps) = self.document_state.net_class_picks.get_mut(path) {
            steps.redo.clear();
        }
    }

    /// Forget every pick of a closed project.
    pub(crate) fn forget_net_class_picks_of(&mut self, project: crate::app::ProjectId) {
        for steps in self.document_state.net_class_picks.values_mut() {
            steps.undo.retain(|s| s.project != project);
            steps.redo.retain(|s| s.project != project);
        }
    }

    /// Whether the active sheet has a pick Ctrl+Z / Ctrl+Y would move.
    pub(crate) fn has_net_class_pick(&self, undoing: bool) -> bool {
        let Some(path) = self.document_state.active_path.as_ref() else {
            return false;
        };
        self.document_state
            .net_class_picks
            .get(path)
            .is_some_and(|s| !(if undoing { &s.undo } else { &s.redo }).is_empty())
    }

    /// Revert the newest pick of the active sheet if it is also the newest
    /// thing on the sheet's timeline. A pick whose project history has moved
    /// on (panel edits, closed project) is dropped. Returns whether it moved.
    pub(crate) fn undo_net_class_pick(&mut self) -> bool {
        self.step_net_class_pick(true)
    }

    /// Re-apply the newest undone pick when it precedes the next sheet edit.
    pub(crate) fn redo_net_class_pick(&mut self) -> bool {
        self.step_net_class_pick(false)
    }

    fn step_net_class_pick(&mut self, undoing: bool) -> bool {
        let document = &mut self.document_state;
        let Some(path) = document.active_path.clone() else {
            return false;
        };
        let position = document.active_engine().map_or(0, |e| e.undo_position());
        let Some(steps) = document.net_class_picks.get_mut(&path) else {
            return false;
        };
        let stack = if undoing {
            &mut steps.undo
        } else {
            &mut steps.redo
        };
        let Some(step) = stack.last().copied() else {
            return false;
        };
        let in_order = if undoing {
            step.at >= position
        } else {
            step.at <= position
        };
        if !in_order {
            return false;
        }
        stack.pop();
        let Some(project) = document.projects.iter_mut().find(|p| p.id == step.project) else {
            return false;
        };
        let history = document.net_class_history.entry(step.project).or_default();
        // Undoing needs the pick on top of the project's history; redoing needs
        // the history exactly one step below where the pick left it.
        let expected = if undoing { step.depth } else { step.depth - 1 };
        if history.undo_len() != expected {
            return false;
        }
        let moved = if undoing {
            history.undo(&mut project.data.net_classes)
        } else {
            history.redo(&mut project.data.net_classes)
        };
        if !moved {
            return false;
        }
        document.dirty_paths.insert(project.path.clone());
        let steps = document.net_class_picks.entry(path).or_default();
        if undoing {
            steps.redo.push(step);
        } else {
            steps.undo.push(step);
        }
        self.refresh_panel_ctx();
        true
    }
}
