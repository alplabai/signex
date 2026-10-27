//! The write path of the project-scoped ERC rule set (#1085): seed it from
//! this machine's preferences on an explicit action, edit it through undoable
//! steps, and leave the `.snxprj` write to the existing explicit save (these
//! handlers only mutate memory and flip the project dirty).

use iced::Task;
use signex_types::project::{ErcRules, ProjectRules};

use super::super::super::state::rules_history::RulesHistory;
use super::super::super::*;

type HistoryStep = fn(&mut RulesHistory, Option<ProjectRules>) -> Option<Option<ProjectRules>>;

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
        let erc = signex_erc::project_rules::seed_from_overrides(
            &self.ui_state.erc_severity_override,
            &self.ui_state.pin_matrix_overrides,
        );
        // Provenance only when the snapshot is exactly the shipped profile;
        // a customised machine has no named profile to point at yet.
        let shipped = signex_erc::project_rules::profile::builtin_profile();
        let profile = (erc.sha256_hex() == shipped.content_sha256()).then(|| {
            signex_erc::project_rules::profile::profile_ref(
                &shipped,
                signex_erc::project_rules::profile::SOURCE_BUILTIN,
            )
        });
        let rules = ProjectRules {
            erc,
            profile,
            ..ProjectRules::default()
        };
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

    /// Step the active project's rule set back. False when there is nothing
    /// to undo.
    pub(crate) fn undo_project_rules(&mut self) -> bool {
        self.step_project_rules(RulesHistory::undo)
    }

    /// Step the active project's rule set forward again.
    pub(crate) fn redo_project_rules(&mut self) -> bool {
        self.step_project_rules(RulesHistory::redo)
    }

    fn step_project_rules(&mut self, step: HistoryStep) -> bool {
        let Some((id, current)) = self
            .document_state
            .active_document_project()
            .map(|p| (p.id, p.data.rules.clone()))
        else {
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
        self.replace_project_rules(restored, false);
        true
    }

    /// Swap the active project's rule set, flip it dirty (the explicit save
    /// writes it) and, for a fresh edit, remember the prior state for undo.
    pub(super) fn replace_project_rules(&mut self, rules: Option<ProjectRules>, record: bool) {
        // Any change to the rules (edit, undo, redo, store) outdates a diff
        // computed against the old ones.
        self.invalidate_pending_profile_update();
        let ds = &mut self.document_state;
        let Some((id, path, before)) = ds
            .active_document_project()
            .map(|p| (p.id, p.path.clone(), p.data.rules.clone()))
        else {
            return;
        };
        if record {
            ds.rules_history.entry(id).or_default().record(before);
        }
        if let Some(loaded) = ds.projects.iter_mut().find(|p| p.id == id) {
            loaded.data.rules = rules;
        }
        ds.dirty_paths.insert(path);
        self.refresh_panel_ctx();
    }
}
