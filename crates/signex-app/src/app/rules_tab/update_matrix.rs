//! `update` for the Rules tab's class matrix, rule sets and presets
//! (#2398). Each edit goes through the same path as its `rules_*` command
//! and is one undo step; a refusal is kept for the tab to show.

use super::{ClearanceView, MatrixEdit, RulesPanel, RulesTabMsg, SetFormField};
use crate::app::Signex;
use crate::app::state::ProjectId;

impl Signex {
    pub(super) fn apply_rules_panel_message(&mut self, msg: RulesTabMsg) {
        match msg {
            RulesTabMsg::ClearanceViewPicked { project_id, view } => {
                let tab = self.rules_tab_mut(project_id);
                tab.clearance_view = view;
                tab.edit = None;
                tab.matrix_edit = None;
                if view == ClearanceView::List {
                    tab.matrix_open = None;
                }
            }
            RulesTabMsg::MatrixCellOpened { project_id, key } => {
                let src = self.rules_text_of(project_id).map(|(text, _)| text);
                let rows = signex_drc_dsl::table::table(&src.unwrap_or_default());
                let text = signex_drc_dsl::table::matrix::cell(&rows, &key)
                    .and_then(|row| {
                        row.limit(signex_drc_dsl::table::LimitField::MinClearance)
                            .map(|l| l.value.clone())
                    })
                    .unwrap_or_default();
                let tab = self.rules_tab_mut(project_id);
                tab.matrix_open = Some((key.a.clone(), key.b.clone()));
                tab.matrix_edit = Some(MatrixEdit { key, text });
            }
            RulesTabMsg::MatrixCellEdited { project_id, text } => {
                if let Some(edit) = self.rules_tab_mut(project_id).matrix_edit.as_mut() {
                    edit.text = text;
                }
            }
            RulesTabMsg::MatrixCellCommitted { project_id } => {
                let Some(edit) = self.rules_tab_mut(project_id).matrix_edit.clone() else {
                    return;
                };
                let result = self.set_matrix_cell(project_id, &edit.key, &edit.text);
                if self.note_rules_result(project_id, result).is_some() {
                    self.rules_tab_mut(project_id).matrix_edit = None;
                }
            }
            RulesTabMsg::MatrixCellCancelled { project_id } => {
                self.rules_tab_mut(project_id).matrix_edit = None;
            }
            RulesTabMsg::MatrixCellCleared { project_id, key } => {
                let result = self.set_matrix_cell(project_id, &key, "");
                if self.note_rules_result(project_id, result).is_some() {
                    let tab = self.rules_tab_mut(project_id);
                    if tab
                        .matrix_edit
                        .as_ref()
                        .is_some_and(|e| e.key.same_as(&key))
                    {
                        tab.matrix_edit = None;
                    }
                }
            }
            RulesTabMsg::MatrixCollapsed { project_id } => {
                let tab = self.rules_tab_mut(project_id);
                tab.matrix_open = None;
                tab.matrix_edit = None;
            }
            RulesTabMsg::PanelPicked { project_id, panel } => {
                let tab = self.rules_tab_mut(project_id);
                tab.panel = panel;
                if panel != Some(RulesPanel::Presets) {
                    tab.preview = None;
                }
            }
            RulesTabMsg::SetFormEdited {
                project_id,
                field,
                text,
            } => {
                let form = &mut self.rules_tab_mut(project_id).set_form;
                match field {
                    SetFormField::Name => form.name = text,
                    SetFormField::Values => form.values = text,
                    SetFormField::Severity => form.severity = text,
                    SetFormField::Scope => form.scope = text,
                }
            }
            RulesTabMsg::SetCreated { project_id } => {
                let form = self.rules_tab_mut(project_id).set_form.clone();
                let severity = Some(form.severity.as_str()).filter(|s| !s.trim().is_empty());
                let result = self.create_rule_set(project_id, &form.name, &form.values, severity);
                if self.note_rules_result(project_id, result).is_some() {
                    self.rules_tab_mut(project_id).set_form.name.clear();
                }
            }
            RulesTabMsg::SetRedefined { project_id, set } => {
                let form = self.rules_tab_mut(project_id).set_form.clone();
                let severity = Some(form.severity.as_str()).filter(|s| !s.trim().is_empty());
                let result = self.redefine_rule_set(project_id, &set, &form.values, severity);
                self.note_rules_result(project_id, result);
            }
            RulesTabMsg::SetApplied { project_id, set } => {
                let scope = self.rules_tab_mut(project_id).set_form.scope.clone();
                let result = self.apply_rule_set(project_id, &set, &scope);
                self.note_rules_result(project_id, result);
            }
            RulesTabMsg::PresetPicked { project_id, preset } => {
                let result = self.preset_preview(project_id, &preset);
                if let Some(preview) = self.note_rules_result(project_id, result) {
                    self.rules_tab_mut(project_id).preview = Some(preview);
                }
            }
            RulesTabMsg::PresetApplied { project_id } => self.apply_previewed_preset(project_id),
            RulesTabMsg::PresetDismissed { project_id } => {
                self.rules_tab_mut(project_id).preview = None;
            }
            RulesTabMsg::OpenRequested { .. }
            | RulesTabMsg::DomainPicked { .. }
            | RulesTabMsg::FilterPicked { .. }
            | RulesTabMsg::SearchEdited { .. }
            | RulesTabMsg::RowSelected { .. }
            | RulesTabMsg::CellOpened { .. }
            | RulesTabMsg::CellEdited { .. }
            | RulesTabMsg::CellCommitted { .. }
            | RulesTabMsg::CellCancelled { .. }
            | RulesTabMsg::EnabledToggled { .. }
            | RulesTabMsg::RuleAdded { .. }
            | RulesTabMsg::RuleRemoved { .. }
            | RulesTabMsg::TextViewToggled { .. }
            | RulesTabMsg::TextActed { .. }
            | RulesTabMsg::TextApplied { .. }
            | RulesTabMsg::TextDiscarded { .. }
            | RulesTabMsg::ScopeShown { .. }
            | RulesTabMsg::DrcRequested { .. }
            | RulesTabMsg::PresetsListRequested { .. } => {}
        }
    }

    /// Apply the previewed preset; the preview closes once it is applied.
    fn apply_previewed_preset(&mut self, id: ProjectId) {
        let Some(preview) = self.rules_tab_mut(id).preview.clone() else {
            return;
        };
        let result = self.apply_preset(id, &preview);
        if self.note_rules_result(id, result).is_some() {
            self.rules_tab_mut(id).preview = None;
        }
    }
}
