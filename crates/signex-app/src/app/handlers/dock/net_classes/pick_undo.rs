//! Ctrl+Z / Ctrl+Y for net-class picks (#1220). See
//! [`crate::app::state::net_class_picks`] for the ordering rule.

use crate::app::state::net_class_picks::NetClassPickStep;

impl crate::app::Signex {
    /// Remember a pick just applied to `project`'s table, made from the active
    /// sheet. `step` is the pick's step in the project's class history. A pick
    /// from a sheet with no engine has no timeline to order against and is
    /// not recorded.
    pub(super) fn record_net_class_pick(
        &mut self,
        project: crate::app::ProjectId,
        step: signex_engine::NetClassStepId,
    ) {
        let Some(path) = self.document_state.active_path.clone() else {
            return;
        };
        self.end_parameter_edit_runs();
        let Some(at) = self
            .document_state
            .active_engine()
            .map(|engine| engine.undo_position())
        else {
            return;
        };
        let steps = self.document_state.net_class_picks.entry(path).or_default();
        steps.redo.clear();
        steps.undo.push(NetClassPickStep { project, at, step });
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

    /// Forget a sheet's picks: its engine is going away or being replaced, so
    /// the positions they were stamped with no longer mean anything.
    pub(crate) fn forget_net_class_picks_on(&mut self, path: &std::path::Path) {
        self.document_state.net_class_picks.remove(path);
    }

    /// Whether `step` is still the step its project's class history would
    /// revert (`undoing`) or re-apply.
    fn net_class_pick_is_live(&self, step: &NetClassPickStep, undoing: bool) -> bool {
        let Some(history) = self.document_state.net_class_history.get(&step.project) else {
            return false;
        };
        let top = if undoing {
            history.top_undo_id()
        } else {
            history.top_redo_id()
        };
        top == Some(step.step)
            && self
                .document_state
                .projects
                .iter()
                .any(|p| p.id == step.project)
    }

    /// Whether the sheet at `path` has a pick Ctrl+Z / Ctrl+Y would move:
    /// some recorded pick that is still its project's newest class edit.
    pub(crate) fn has_net_class_pick(&self, path: &std::path::Path, undoing: bool) -> bool {
        let Some(steps) = self.document_state.net_class_picks.get(path) else {
            return false;
        };
        let stack = if undoing { &steps.undo } else { &steps.redo };
        stack
            .iter()
            .any(|step| self.net_class_pick_is_live(step, undoing))
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
        let Some(path) = self.document_state.active_path.clone() else {
            return false;
        };
        let position = self
            .document_state
            .active_engine()
            .map_or(0, |e| e.undo_position());
        // Drop picks that are no longer their project's newest class edit
        // (panel edits, a project closed) until a live one is on top.
        let step = loop {
            let Some(step) = self
                .document_state
                .net_class_picks
                .get(&path)
                .and_then(|s| {
                    if undoing {
                        s.undo.last()
                    } else {
                        s.redo.last()
                    }
                })
                .copied()
            else {
                return false;
            };
            if self.net_class_pick_is_live(&step, undoing) {
                break step;
            }
            if let Some(steps) = self.document_state.net_class_picks.get_mut(&path) {
                if undoing {
                    steps.undo.pop();
                } else {
                    steps.redo.pop();
                }
            }
        };
        let in_order = if undoing {
            step.at >= position
        } else {
            step.at <= position
        };
        if !in_order {
            return false;
        }
        let document = &mut self.document_state;
        let Some(project) = document.projects.iter_mut().find(|p| p.id == step.project) else {
            return false;
        };
        let history = document.net_class_history.entry(step.project).or_default();
        let moved = if undoing {
            history.undo(&mut project.data.net_classes)
        } else {
            history.redo(&mut project.data.net_classes)
        };
        if !moved {
            return false;
        }
        let project_file = project.path.clone();
        document.mark_document_dirty_display_only(&project_file);
        let steps = document.net_class_picks.entry(path).or_default();
        let (from, to) = if undoing {
            (&mut steps.undo, &mut steps.redo)
        } else {
            (&mut steps.redo, &mut steps.undo)
        };
        from.pop();
        to.push(step);
        self.refresh_panel_ctx();
        true
    }
}
