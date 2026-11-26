//! `update` for the Rules tab: the only place its state and the rule text
//! change. Every rule edit goes through `commit_rules_text`, the path the
//! `rules_*` commands take too.

use iced::Task;
use signex_drc::WrittenRuleEdit;
use signex_drc_dsl::table::{self, RuleDraft, RuleField, RuleKind};

use super::source::{TEXT_UNAPPLIED, field_follow, rule_name};
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
            RulesTabMsg::OpenedAt {
                project_id,
                domain,
                rule,
            } => self.open_rules_on(project_id, domain, rule),
            RulesTabMsg::DrcRequested { project_id } => self.run_rules_drc(project_id),
            RulesTabMsg::GridActed { project_id, msg } => self.act_on_rules_grid(project_id, msg),
            RulesTabMsg::PreviewActed { project_id, msg } => {
                self.act_on_rules_drawing(project_id, msg)
            }
            RulesTabMsg::ScopeShown { project_id, rule } => {
                self.show_rule_scope(project_id, &rule);
                Task::none()
            }
            // The profile library is read off the update loop; its profiles
            // become presets when it lands.
            RulesTabMsg::PresetsListRequested { .. } => Self::list_profiles_task(),
            msg => {
                self.apply_rules_tab_message(msg);
                Task::none()
            }
        }
    }

    pub(super) fn rules_tab_mut(&mut self, id: ProjectId) -> &mut RulesTabState {
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
                self.pick_rules_domain(project_id, domain);
            }
            RulesTabMsg::SetsToggled { project_id } => self.toggle_rules_sets(project_id),
            RulesTabMsg::InheritedToggled { project_id } => {
                let tab = self.rules_tab_mut(project_id);
                tab.hide_inherited = !tab.hide_inherited;
            }
            RulesTabMsg::OverriddenOnlyToggled { project_id } => {
                self.toggle_rules_overridden_only(project_id);
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
                // Switched on, an `off` severity entry would keep it off.
                let follows: Vec<_> = on
                    .then(|| WrittenRuleEdit::SwitchedOn(rule_name(&rule)))
                    .into_iter()
                    .collect();
                self.edit_rules(project_id, &follows, |src| {
                    table::enable_rule(src, &rule, on)
                });
            }
            RulesTabMsg::RuleAdded { project_id, kind } => self.add_rule_row(project_id, kind),
            RulesTabMsg::RuleRemoved { project_id, rule } => {
                let follows = [WrittenRuleEdit::Dropped(rule_name(&rule))];
                self.edit_rules(project_id, &follows, |src| table::remove_rule(src, &rule));
                let tab = self.rules_tab_mut(project_id);
                if tab.selected.as_deref() == Some(rule.as_str()) {
                    tab.selected = None;
                }
            }
            // Leaving (or reopening) the text view would drop typed text
            // not applied.
            RulesTabMsg::TextViewToggled { project_id, on }
                if self.rules_text_unapplied(project_id) =>
            {
                if !on {
                    self.rules_tab_mut(project_id).refused = Some(TEXT_UNAPPLIED.to_string());
                }
            }
            RulesTabMsg::TextViewToggled { project_id, on } => {
                let text = on
                    .then(|| self.rules_text_of(project_id).map(|(text, _)| text))
                    .flatten();
                let tab = self.rules_tab_mut(project_id);
                tab.edit = None;
                tab.refused = None;
                tab.text = text.map(|text| RulesText::new(&text));
            }
            RulesTabMsg::TextDiscarded { project_id } => {
                let stored = self.rules_text_of(project_id).map(|(text, _)| text);
                let tab = self.rules_tab_mut(project_id);
                if let (Some(view), Some(stored)) = (tab.text.as_mut(), stored) {
                    *view = RulesText::new(&stored);
                    tab.refused = None;
                }
            }
            RulesTabMsg::TextActed { project_id, action } => {
                if let Some(text) = self.rules_tab_mut(project_id).text.as_mut() {
                    text.perform(action);
                }
            }
            RulesTabMsg::TextApplied { project_id } => self.apply_rules_text(project_id),
            // These rewrite the stored text, which Apply of the typed text
            // would then replace.
            RulesTabMsg::SetRedefined { project_id, .. }
            | RulesTabMsg::SetApplied { project_id, .. }
            | RulesTabMsg::PresetApplied { project_id }
                if self.rules_text_unapplied(project_id) =>
            {
                self.rules_tab_mut(project_id).refused = Some(TEXT_UNAPPLIED.to_string());
            }
            msg @ (RulesTabMsg::ClearanceViewPicked { .. }
            | RulesTabMsg::MatrixCellCommitted { .. }
            | RulesTabMsg::MatrixCellCleared { .. }
            | RulesTabMsg::MatrixCollapsed { .. }
            | RulesTabMsg::PanelPicked { .. }
            | RulesTabMsg::SetFormEdited { .. }
            | RulesTabMsg::SetCreated { .. }
            | RulesTabMsg::SetRedefined { .. }
            | RulesTabMsg::SetApplied { .. }
            | RulesTabMsg::PresetPicked { .. }
            | RulesTabMsg::PresetApplied { .. }
            | RulesTabMsg::PresetDismissed { .. }
            | RulesTabMsg::MatrixActed { .. }
            | RulesTabMsg::MatrixHovered { .. }
            | RulesTabMsg::MatrixPairPinned { .. }
            | RulesTabMsg::MatrixPairTyped { .. }) => self.apply_rules_panel_message(msg),
            RulesTabMsg::OpenRequested { .. }
            | RulesTabMsg::OpenedAt { .. }
            | RulesTabMsg::PresetsListRequested { .. }
            | RulesTabMsg::DrcRequested { .. }
            | RulesTabMsg::GridActed { .. }
            | RulesTabMsg::PreviewActed { .. }
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
            // The severity the cell shows: a layer entry outranks the word.
            RuleField::Severity => {
                let set = self
                    .rules_drc_layer(id)
                    .and_then(|layer| signex_drc::written_rule_override(layer, &row.name))
                    .flatten();
                super::json::severity_str(set.unwrap_or(row.severity)).to_string()
            }
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
        let follows: Vec<_> = field_follow(&edit.rule, edit.field, &edit.text)
            .into_iter()
            .collect();
        let committed = self.edit_rules(id, &follows, |src| {
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
            RuleKind::Hole => "0.2mm",
            RuleKind::Edge => "0.3mm",
            RuleKind::Silk => "0.15mm",
            RuleKind::Mask => "0.1mm",
            RuleKind::Paste => "0mm",
            RuleKind::Courtyard => "0mm",
            RuleKind::Component => "6mm",
        };
        if draft.set_values(first).is_err() {
            return;
        }
        let Some((src, _)) = self.rules_text_of(id) else {
            return;
        };
        let result = table::add_rule(&src, &draft)
            .map_err(|error| error.to_string())
            .and_then(|(text, name)| {
                // A stale entry for the name must not speak for the new rule.
                let follows = [WrittenRuleEdit::Dropped(&name)];
                self.commit_rules_text(id, text, &follows).map(|()| name)
            });
        if let Some(name) = self.note_rules_result(id, result) {
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
        match self.commit_rules_text(id, text, &[]) {
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

    /// Run `edit` on the rule text and store the result, with the severity
    /// entries following `follows`, as one undo step; a refusal is kept for
    /// the tab to show. Whether it was stored.
    pub(super) fn edit_rules(
        &mut self,
        id: ProjectId,
        follows: &[WrittenRuleEdit<'_>],
        edit: impl FnOnce(&str) -> Result<String, table::EditError>,
    ) -> bool {
        let Some((src, _)) = self.rules_text_of(id) else {
            return false;
        };
        let result = edit(&src)
            .map_err(|error| error.to_string())
            .and_then(|text| self.commit_rules_text(id, text, follows));
        self.note_rules_result(id, result).is_some()
    }

    /// Keep an edit's refusal for the tab to show, or clear the last one.
    pub(super) fn note_rules_result<T>(
        &mut self,
        id: ProjectId,
        result: Result<T, String>,
    ) -> Option<T> {
        match result {
            Ok(value) => {
                self.rules_tab_mut(id).refused = None;
                Some(value)
            }
            Err(reason) => {
                self.rules_tab_mut(id).refused = Some(reason);
                None
            }
        }
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
            .ok()
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
