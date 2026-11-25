//! The write path of the project-scoped ERC rule set (#1085): seed it from
//! this machine's preferences on an explicit action, edit it through undoable
//! steps, and leave the `.snxprj` write to the existing explicit save (these
//! handlers only mutate memory and flip the project dirty).

use iced::Task;
use signex_types::project::{ErcRules, ProjectRules};

use super::super::super::state::rules_history::{RuleState, RulesHistory};
use super::super::super::*;

type HistoryStep = fn(&mut RulesHistory, RuleState) -> Option<RuleState>;

impl Signex {
    /// Whether the project the ERC dialog edits stores its own rule set.
    pub(crate) fn active_project_stores_rules(&self) -> bool {
        self.document_state
            .active_document_project()
            .is_some_and(|p| p.data.rules.is_some())
    }

    /// Whether the project's `.snxprj` keeps a `rules` section this build
    /// cannot read (#1180); storing a fresh set would overwrite it on save.
    pub(crate) fn active_project_has_unreadable_rules(&self) -> bool {
        self.document_state
            .active_document_project()
            .is_some_and(|p| p.data.has_unreadable_rules())
    }

    /// How many rules the ERC dialog shows at each severity, in the order
    /// Error, Warning, Info, Off: the project's stored rules, else the
    /// machine's. The dialog's severity chips (#2413).
    pub fn erc_dialog_severity_counts(&self) -> [(signex_erc::Severity, usize); 4] {
        let stored = self
            .document_state
            .active_document_project()
            .and_then(|p| p.data.rules.as_ref())
            .map(|rules| signex_erc::project_rules::resolve(&rules.erc).severity);
        signex_erc::project_rules::severity_counts(
            stored
                .as_ref()
                .unwrap_or(&self.ui_state.erc_severity_override),
        )
    }

    /// "Store ERC rules in this project": snapshot the machine preferences
    /// into the project. A no-op when the project already has rules or there
    /// is no project.
    pub(crate) fn handle_store_erc_rules_in_project(&mut self) -> Task<Message> {
        if self.active_project_stores_rules() {
            return Task::none();
        }
        if self.active_project_has_unreadable_rules() {
            crate::diagnostics::log_warning(
                "Not storing ERC rules: this project holds a rule set from a newer Signex that \
                 storing would replace.",
            );
            return Task::none();
        }
        let rules = self.seeded_project_rules();
        self.replace_project_rules(Some(rules), true);
        Task::none()
    }

    /// Apply `edit` to the project's stored ERC rules as one undoable step.
    /// Returns false when the project stores no rules (the caller edits the
    /// preferences instead); true otherwise, even when the edit changed
    /// nothing.
    pub(crate) fn edit_project_erc_rules(&mut self, edit: impl FnOnce(&mut ErcRules)) -> bool {
        let Some(mut rules) = self
            .document_state
            .active_document_project()
            .and_then(|p| p.data.rules.clone())
        else {
            return false;
        };
        let before = rules.clone();
        edit(&mut rules.erc);
        // An edit that changes nothing (the selected severity clicked again)
        // records no undo step and leaves the project clean.
        if rules != before {
            self.replace_project_rules(Some(rules), true);
        }
        true
    }

    /// After an undo or redo of the rule set: re-run ERC so the results and
    /// waivers shown match the rules again, when there are results to
    /// refresh.
    pub(crate) fn rerun_erc_after_rules_edit(&mut self, changed: bool) -> Task<Message> {
        if changed && !self.ui_state.erc_violations_by_path.is_empty() {
            self.handle_run_erc()
        } else {
            Task::none()
        }
    }

    /// Step the active project's rule set back. False when there is nothing
    /// to undo.
    pub(crate) fn undo_project_rules(&mut self) -> bool {
        self.step_project_rules(true)
    }

    /// Step the active project's rule set forward again.
    pub(crate) fn redo_project_rules(&mut self) -> bool {
        self.step_project_rules(false)
    }

    fn step_project_rules(&mut self, undo: bool) -> bool {
        match self.document_state.active_document_project().map(|p| p.id) {
            Some(id) => self.step_rules_of(id, undo),
            None => false,
        }
    }

    /// Step `id`'s rule history (the ERC dialog's and the Rules tab's) back
    /// (`undo`) or forward. Whether a step moved.
    pub(crate) fn step_rules_of(&mut self, id: ProjectId, undo: bool) -> bool {
        let step: HistoryStep = if undo {
            RulesHistory::undo
        } else {
            RulesHistory::redo
        };
        let Some(current) = self.rule_state_of(id) else {
            return false;
        };
        let Some(restored) = self
            .document_state
            .rules_history
            .get_mut(&id)
            .and_then(|history| step(history, current))
        else {
            return false;
        };
        self.replace_rule_state_of(id, restored, false);
        true
    }

    /// What a rule-history step of `id` keeps: its rules and router tool.
    fn rule_state_of(&self, id: ProjectId) -> Option<RuleState> {
        let data = &self.document_state.project_by_id(id)?.data;
        Some(RuleState {
            rules: data.rules.clone(),
            router_tool_diameter: data.router_tool_diameter(),
        })
    }

    /// [`Self::replace_rules_of`] the active document's project.
    pub(super) fn replace_project_rules(&mut self, rules: Option<ProjectRules>, record: bool) {
        match self.document_state.active_document_project().map(|p| p.id) {
            Some(id) => self.replace_rules_of(id, rules, record),
            None => self.invalidate_pending_profile_update(),
        }
    }

    /// Swap `id`'s rule set, flip it dirty (the explicit save writes it)
    /// and, for a fresh edit, remember the prior state for undo.
    pub(crate) fn replace_rules_of(
        &mut self,
        id: ProjectId,
        rules: Option<ProjectRules>,
        record: bool,
    ) {
        let Some(router_tool_diameter) = self
            .document_state
            .project_by_id(id)
            .map(|p| p.data.router_tool_diameter())
        else {
            return;
        };
        let state = RuleState {
            rules,
            router_tool_diameter,
        };
        self.replace_rule_state_of(id, state, record);
    }

    /// [`Self::replace_rules_of`] with the router tool diameter too: a
    /// fab preset's rules and router tool are one step, and an undo or
    /// redo restores both.
    pub(crate) fn replace_rule_state_of(&mut self, id: ProjectId, state: RuleState, record: bool) {
        // Any change to the rules (edit, undo, redo, store) outdates a diff
        // computed against the old ones.
        self.invalidate_pending_profile_update();
        let Some(before) = self.rule_state_of(id) else {
            return;
        };
        let min_slot_of = |state: &RuleState| state.rules.as_ref().and_then(|r| r.min_slot());
        let fab_moved = before.router_tool_diameter != state.router_tool_diameter
            || min_slot_of(&before) != min_slot_of(&state);
        let ds = &mut self.document_state;
        if record {
            ds.note_project_clean_state(id);
            let general = &mut ds.project_tabs.entry(id).or_default().general_history;
            // Typing after this step is a step after it, not an older run.
            general.close_run();
            let position = general.position();
            ds.rules_history
                .entry(id)
                .or_default()
                .record_at(before, position);
        }
        if let Some(loaded) = ds.project_by_id_mut(id) {
            loaded.data.rules = state.rules;
            loaded
                .data
                .set_router_tool_diameter(state.router_tool_diameter);
        }
        ds.sync_project_history_dirty(id, true);
        if fab_moved {
            self.sync_board_sketch_fab();
        }
        self.refresh_panel_ctx();
    }
}
