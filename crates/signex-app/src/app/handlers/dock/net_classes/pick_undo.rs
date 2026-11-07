//! Ctrl+Z / Ctrl+Y for net-class picks (#1220, #661).
//!
//! A pick changes the project's `.snxprj` class table, not the sheet, so it
//! enters the sheet's one undo timeline as a marker
//! ([`signex_engine::ClassPickMarker`]) that touches no sheet content. The
//! timeline orders it against sheet edits and colour steps; the table itself
//! is restored through the project's `NetClassHistory` when the marker comes up.

use signex_engine::ClassPickMarker;

impl crate::app::Signex {
    /// Put a pick just applied to `project`'s table on the timeline of the
    /// sheet at `sheet`. `step` is the pick's step in the project's class
    /// history. A sheet with no engine has no timeline: nothing is recorded.
    pub(super) fn record_net_class_pick(
        &mut self,
        project: crate::app::ProjectId,
        step: signex_engine::NetClassStepId,
        sheet: Option<&std::path::Path>,
    ) {
        let Some(sheet) = sheet else {
            return;
        };
        self.end_parameter_edit_runs();
        let Some(engine) = self.document_state.engines.get_mut(sheet) else {
            return;
        };
        engine.record_class_pick(ClassPickMarker {
            project: project.raw(),
            step,
        });
        // A marker moves no save point, but an old edit may fall off the
        // depth: the tab's dirty dot follows the history.
        self.document_state.sync_history_dirty(sheet);
    }

    /// Whether `marker` is still the step its project's class history would
    /// revert (`undoing`) or re-apply, in a project that is still open.
    fn net_class_pick_is_live(&self, marker: ClassPickMarker, undoing: bool) -> bool {
        let document = &self.document_state;
        let Some(project) = document
            .projects
            .iter()
            .find(|p| p.id.raw() == marker.project)
        else {
            return false;
        };
        document
            .net_class_history
            .get(&project.id)
            .is_some_and(|history| {
                let top = if undoing {
                    history.top_undo_id()
                } else {
                    history.top_redo_id()
                };
                top == Some(marker.step)
            })
    }

    /// Whether the sheet at `path` has a step Ctrl+Z (`undoing`) or Ctrl+Y
    /// would move: an edit, or a pick that is still its project's newest
    /// class edit. Dead markers on the way are stepped over.
    pub(crate) fn sheet_has_step(&self, path: &std::path::Path, undoing: bool) -> bool {
        let Some(engine) = self.document_state.engines.get(path) else {
            return false;
        };
        engine
            .steps_from_top(undoing)
            .any(|marker| marker.is_none_or(|m| self.net_class_pick_is_live(m, undoing)))
    }

    /// Step the active sheet's timeline over the markers on top of it:
    /// apply the first live one, drop the dead ones on the way (their
    /// project history moved on, or the project closed). `true` when a pick
    /// was applied; `false` when an edit of the sheet, or nothing, is next.
    pub(crate) fn step_net_class_pick(&mut self, undoing: bool) -> bool {
        while let Some(marker) = self
            .document_state
            .active_engine()
            .and_then(|engine| engine.next_class_pick(undoing))
        {
            let applied =
                self.net_class_pick_is_live(marker, undoing) && self.apply_pick(marker, undoing);
            if let Some(engine) = self.document_state.active_engine_mut() {
                // A marker not applied is dead for good (see the engine).
                if applied {
                    engine.pass_class_pick(undoing);
                } else {
                    engine.pass_dead_class_pick(undoing);
                }
            }
            if applied {
                self.refresh_panel_ctx();
                return true;
            }
        }
        false
    }

    /// Pass the dead markers on top of the sheet at `path`'s timeline: a
    /// project-wide step below them is then the sheet's next one. A live
    /// marker stays and blocks it, like any later edit.
    pub(crate) fn pass_dead_class_picks(&mut self, path: &std::path::Path, undoing: bool) {
        while self
            .document_state
            .engines
            .get(path)
            .and_then(|engine| engine.next_class_pick(undoing))
            .is_some_and(|marker| !self.net_class_pick_is_live(marker, undoing))
        {
            if let Some(engine) = self.document_state.engines.get_mut(path) {
                engine.pass_dead_class_pick(undoing);
            }
        }
    }

    /// Revert (`undoing`) or re-apply the class-table step `marker` names.
    fn apply_pick(&mut self, marker: ClassPickMarker, undoing: bool) -> bool {
        let document = &mut self.document_state;
        let Some(project) = document
            .projects
            .iter_mut()
            .find(|p| p.id.raw() == marker.project)
        else {
            return false;
        };
        let id = project.id;
        let history = document.net_class_history.entry(id).or_default();
        let moved = if undoing {
            history.undo(&mut project.data.net_classes)
        } else {
            history.redo(&mut project.data.net_classes)
        };
        if moved {
            document.sync_project_history_dirty(id, false);
        }
        moved
    }
}
