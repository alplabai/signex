//! `update` for the Rules tab: the only place its state and the rule text
//! change. Every rule edit goes through `commit_rules_text`, the path the
//! `rules_*` commands take too.

use iced::Task;
use signex_drc_dsl::table::{self, RuleDraft, RuleField, RuleKind};

use super::{CellEdit, RulesTabMsg, RulesTabState, RulesText};
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};

impl Signex {
    pub(crate) fn dispatch_rules_tab_message(&mut self, msg: RulesTabMsg) -> Task<Message> {
        match msg {
            RulesTabMsg::OpenRequested { project } => {
                match self.rules_project_for(project) {
                    Some(id) => self.open_rules_tab(id),
                    None => crate::diagnostics::log_info("Rules: no project is open"),
                }
                Task::none()
            }
            RulesTabMsg::DrcRequested { project_id } => self.run_rules_drc(project_id),
            RulesTabMsg::ScopeShown { project_id, rule } => {
                self.show_rule_scope(project_id, &rule);
                Task::none()
            }
            msg => {
                self.apply_rules_tab_message(msg);
                Task::none()
            }
        }
    }

    fn rules_tab_mut(&mut self, id: ProjectId) -> &mut RulesTabState {
        &mut self
            .document_state
            .project_tabs
            .entry(id)
            .or_default()
            .rules
    }

    fn apply_rules_tab_message(&mut self, msg: RulesTabMsg) {
        match msg {
            RulesTabMsg::DomainPicked { project_id, domain } => {
                let tab = self.rules_tab_mut(project_id);
                tab.domain = domain;
                tab.edit = None;
            }
            RulesTabMsg::FilterPicked { project_id, filter } => {
                self.rules_tab_mut(project_id).filter = filter;
            }
            RulesTabMsg::SearchEdited { project_id, text } => {
                self.rules_tab_mut(project_id).search = text;
            }
            RulesTabMsg::RowSelected { project_id, rule } => {
                self.rules_tab_mut(project_id).selected = Some(rule);
            }
            RulesTabMsg::CellOpened {
                project_id,
                rule,
                field,
            } => self.open_rule_cell(project_id, rule, field),
            RulesTabMsg::CellEdited { project_id, text } => {
                if let Some(edit) = self.rules_tab_mut(project_id).edit.as_mut() {
                    edit.text = text;
                }
            }
            RulesTabMsg::CellCommitted { project_id } => self.commit_rule_cell(project_id),
            RulesTabMsg::CellCancelled { project_id } => {
                self.rules_tab_mut(project_id).edit = None;
            }
            RulesTabMsg::EnabledToggled {
                project_id,
                rule,
                on,
            } => {
                self.edit_rules(project_id, |src| table::enable_rule(src, &rule, on));
            }
            RulesTabMsg::RuleAdded { project_id, kind } => self.add_rule_row(project_id, kind),
            RulesTabMsg::RuleRemoved { project_id, rule } => {
                self.edit_rules(project_id, |src| table::remove_rule(src, &rule));
                let tab = self.rules_tab_mut(project_id);
                if tab.selected.as_deref() == Some(rule.as_str()) {
                    tab.selected = None;
                }
            }
            RulesTabMsg::TextViewToggled { project_id, on } => {
                let text = on
                    .then(|| self.rules_text_of(project_id).map(|(text, _)| text))
                    .flatten();
                let tab = self.rules_tab_mut(project_id);
                tab.edit = None;
                tab.text = text.map(|text| RulesText::new(&text));
            }
            RulesTabMsg::TextActed { project_id, action } => {
                if let Some(text) = self.rules_tab_mut(project_id).text.as_mut() {
                    let edits = action.is_edit();
                    text.content.perform(action);
                    text.edited |= edits;
                }
            }
            RulesTabMsg::TextApplied { project_id } => self.apply_rules_text(project_id),
            RulesTabMsg::OpenRequested { .. }
            | RulesTabMsg::DrcRequested { .. }
            | RulesTabMsg::ScopeShown { .. } => {}
        }
    }

    /// Start typing in a cell, seeded with what it shows.
    fn open_rule_cell(&mut self, id: ProjectId, rule: String, field: RuleField) {
        let Some((src, _)) = self.rules_text_of(id) else {
            return;
        };
        let rows = table::table(&src);
        let Some(row) = rows.row(&rule) else {
            return;
        };
        let text = match field {
            RuleField::Name => row.name.clone(),
            RuleField::Scope => table::scope_text(&row.scope),
            RuleField::Layer => row.layer.clone().unwrap_or_default(),
            RuleField::Pair => row
                .pair
                .as_ref()
                .map(|[a, b]| format!("{a}-{b}"))
                .unwrap_or_default(),
            RuleField::Severity => super::json::severity_str(row.severity).to_string(),
            RuleField::Message => row.message.clone().unwrap_or_default(),
            RuleField::Limit(limit) => row
                .limit(limit)
                .map(|l| l.value.clone())
                .unwrap_or_default(),
            RuleField::Enabled => row.enabled.to_string(),
        };
        let tab = self.rules_tab_mut(id);
        tab.selected = Some(rule.clone());
        tab.edit = Some(CellEdit { rule, field, text });
    }

    /// Enter in a cell: write it back into its rule, or say why not and
    /// keep the cell open.
    fn commit_rule_cell(&mut self, id: ProjectId) {
        let Some(edit) = self.rules_tab_mut(id).edit.clone() else {
            return;
        };
        let committed = self.edit_rules(id, |src| {
            table::set_field(src, &edit.rule, edit.field, &edit.text)
        });
        if committed {
            let tab = self.rules_tab_mut(id);
            tab.edit = None;
            if edit.field == RuleField::Name {
                tab.selected = Some(edit.text.trim().to_string());
            }
        }
    }

    /// A new row in the sub-tab: a board-wide rule of `kind` with a
    /// starting limit, named `<kind>_<n>`, ready to scope and tune.
    fn add_rule_row(&mut self, id: ProjectId, kind: RuleKind) {
        let mut draft = RuleDraft::new(kind, "");
        let first = match kind {
            RuleKind::Clearance => "0.2mm",
            RuleKind::Track => "0.15mm",
            RuleKind::Via => "0.45mm",
        };
        if draft.set_values(first).is_err() {
            return;
        }
        let mut added = None;
        self.edit_rules(id, |src| {
            let (text, name) = table::add_rule(src, &draft)?;
            added = Some(name);
            Ok(text)
        });
        if let Some(name) = added {
            self.rules_tab_mut(id).selected = Some(name);
        }
    }

    /// Apply the text view's text as one undo step. Text with a syntax
    /// error is stored too (as Run DRC reads any stored text); the tab and
    /// Run DRC say what is not applied.
    fn apply_rules_text(&mut self, id: ProjectId) {
        // Nothing typed: nothing to store, so no step.
        let Some(text) = self
            .rules_tab_mut(id)
            .text
            .as_ref()
            .filter(|t| t.edited)
            .map(|t| t.content.text())
        else {
            return;
        };
        match self.commit_rules_text(id, text) {
            Ok(()) => {
                let tab = self.rules_tab_mut(id);
                tab.refused = None;
                if let Some(text) = tab.text.as_mut() {
                    text.edited = false;
                }
            }
            Err(reason) => self.rules_tab_mut(id).refused = Some(reason),
        }
    }

    /// Run `edit` on the rule text and store the result as one undo step;
    /// a refusal is kept for the tab to show. Whether it was stored.
    fn edit_rules(
        &mut self,
        id: ProjectId,
        edit: impl FnOnce(&str) -> Result<String, table::EditError>,
    ) -> bool {
        let Some((src, _)) = self.rules_text_of(id) else {
            return false;
        };
        let result = edit(&src)
            .map_err(|error| error.to_string())
            .and_then(|text| self.commit_rules_text(id, text));
        let ok = result.is_ok();
        self.rules_tab_mut(id).refused = result.err();
        ok
    }

    /// Run DRC on the project's board from its Rules tab.
    fn run_rules_drc(&mut self, id: ProjectId) -> Task<Message> {
        let Some((path, board)) = self.rules_board(id).map(|(p, b)| (p, b.clone())) else {
            crate::diagnostics::log_info("DRC: the project's board is not open");
            return Task::none();
        };
        self.start_pcb_drc(path, board)
    }

    /// Show what `rule` applies to on the board: its tab in view, the
    /// objects selected.
    fn show_rule_scope(&mut self, id: ProjectId, rule: &str) {
        let Some(items) = self
            .rules_resolved(id)
            .zip(self.rules_board(id))
            .and_then(|(rules, (_, board))| rules.scope_items(board, rule))
        else {
            return;
        };
        let Some(path) = self.rules_board_path(id) else {
            return;
        };
        let Some(idx) = self
            .document_state
            .tabs
            .iter()
            .position(|tab| matches!(tab.kind, crate::app::TabKind::Pcb) && tab.path == path)
        else {
            crate::diagnostics::log_info("Rules: open the board to see what a rule applies to");
            return;
        };
        if idx != self.document_state.active_tab {
            self.park_active_schematic_session();
            self.document_state.active_tab = idx;
            self.sync_active_tab();
        }
        self.select_pcb_items(&items);
    }
}
