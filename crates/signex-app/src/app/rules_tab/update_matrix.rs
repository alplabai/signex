//! `update` for the Rules tab's class matrix, rule sets and presets
//! (#2398). Each edit goes through the same path as its `rules_*` command
//! and is one undo step; a refusal is kept for the tab to show.

use iced::Task;
use signex_drc_dsl::table::RuleTable;
use signex_types::coord::Unit;
use signex_widgets::LengthFormat;
use signex_widgets::constraint_grid::{ClassPair, MatrixChange, MatrixMessage};

use super::grid::{mm_text, typed_value};
use super::matrix::{ClassClearances, MatrixDefaults};
use super::{ClearanceView, MatrixEdit, RulesPanel, RulesTabMsg, SetFormField};
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};

/// What the matrix widget reads its clearances from: the class names, the
/// rows of the stored text and the fallbacks.
struct MatrixBook {
    classes: Vec<String>,
    table: RuleTable,
    defaults: MatrixDefaults,
}

impl MatrixBook {
    fn values(&self) -> ClassClearances<'_> {
        ClassClearances {
            classes: &self.classes,
            table: &self.table,
            defaults: &self.defaults,
        }
    }
}

/// A value typed on the preview drawing: written to the pinned matrix cell
/// or object-pair row as one undo step.
pub(super) fn preview_committed(
    app: &mut Signex,
    project_id: ProjectId,
    value_mm: f64,
) -> Task<Message> {
    let Some(key) = app.rules_tab_mut(project_id).matrix_pin.clone() else {
        return Task::none();
    };
    if !value_mm.is_finite() || value_mm < 0.0 {
        return Task::none();
    }
    let result = app.set_matrix_cell(project_id, &key, &mm_text(value_mm));
    app.note_rules_result(project_id, result);
    Task::none()
}

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
                    tab.matrix_hover = None;
                    tab.matrix_pin = None;
                }
            }
            RulesTabMsg::MatrixCellCommitted { project_id } => {
                let Some(edit) = self.rules_tab_mut(project_id).matrix_edit.clone() else {
                    return;
                };
                let value = typed_value(&edit.text);
                let result = self.set_matrix_cell(project_id, &edit.key, &value);
                if self.note_rules_result(project_id, result).is_some() {
                    self.rules_tab_mut(project_id).matrix_edit = None;
                }
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
                tab.matrix_hover = None;
                // The preview stays on the class pair the card was for.
                if let Some(pin) = tab.matrix_pin.as_mut() {
                    pin.pair = None;
                }
            }
            RulesTabMsg::MatrixActed { project_id, msg } => self.act_on_matrix(project_id, msg),
            RulesTabMsg::MatrixHovered { project_id, key } => {
                self.rules_tab_mut(project_id).matrix_hover = key;
            }
            RulesTabMsg::MatrixPairPinned { project_id, key } => {
                self.rules_tab_mut(project_id).matrix_pin = Some(key);
            }
            RulesTabMsg::MatrixPairTyped {
                project_id,
                key,
                text,
            } => {
                // One cell is typed in at a time: a row of the card, or the
                // matrix cell.
                if self.rules_tab_mut(project_id).matrix.edit().is_some() {
                    self.matrix_widget_update(project_id, MatrixMessage::EditCancelled);
                }
                let tab = self.rules_tab_mut(project_id);
                tab.matrix_pin = Some(key.clone());
                tab.matrix_edit = Some(MatrixEdit { key, text });
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
            | RulesTabMsg::OpenedAt { .. }
            | RulesTabMsg::DomainPicked { .. }
            | RulesTabMsg::GridActed { .. }
            | RulesTabMsg::PreviewActed { .. }
            | RulesTabMsg::SetsToggled { .. }
            | RulesTabMsg::InheritedToggled { .. }
            | RulesTabMsg::OverriddenOnlyToggled { .. }
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

    /// The class names, rule rows and fallbacks the matrix reads.
    fn matrix_book(&self, id: ProjectId) -> MatrixBook {
        let src = self
            .rules_text_of(id)
            .map(|(text, _)| text)
            .unwrap_or_default();
        MatrixBook {
            classes: self.rules_classes(id).into_iter().map(|c| c.name).collect(),
            table: signex_drc_dsl::table::table(&src),
            defaults: self.rules_matrix_defaults(id),
        }
    }

    /// Hand `msg` to the matrix widget; the value change it returns, if any.
    fn matrix_widget_update(&mut self, id: ProjectId, msg: MatrixMessage) -> Option<MatrixChange> {
        self.sync_rules_matrix(id);
        let book = self.matrix_book(id);
        let format = LengthFormat::new(&self.ui_state.i18n, Unit::Mm);
        let tab = &mut self
            .document_state
            .project_tabs
            .entry(id)
            .or_default()
            .rules;
        tab.matrix.update(msg, &book.values(), format)
    }

    /// A message of the matrix widget. Only the lower triangle is live: a
    /// pair is folded onto it before the widget sees it. A pressed cell
    /// opens its object pairs and pins the preview; a value change is
    /// written as one undo step.
    fn act_on_matrix(&mut self, id: ProjectId, msg: MatrixMessage) {
        if msg == MatrixMessage::EditSubmitted {
            self.submit_matrix_cell(id);
            return;
        }
        let msg = match msg {
            MatrixMessage::CellPressed(pair) => {
                MatrixMessage::CellPressed(ClassPair::new(pair.row, pair.col))
            }
            MatrixMessage::CellCleared(pair) => {
                MatrixMessage::CellCleared(ClassPair::new(pair.row, pair.col))
            }
            other => other,
        };
        let pressed = match &msg {
            MatrixMessage::CellPressed(pair) => Some(*pair),
            _ => None,
        };
        let change = self.matrix_widget_update(id, msg);
        let book = self.matrix_book(id);
        if let Some(key) = pressed.and_then(|pair| book.values().key(pair)) {
            let tab = self.rules_tab_mut(id);
            tab.matrix_open = Some((key.a.clone(), key.b.clone()));
            tab.matrix_pin = Some(key);
            tab.matrix_edit = None;
        }
        let Some(change) = change else {
            return;
        };
        let (pair, value) = match change {
            MatrixChange::ClearanceSet { pair, mm } => (pair, mm_text(mm)),
            MatrixChange::ClearanceCleared { pair } => (pair, String::new()),
        };
        let Some(key) = book.values().key(pair) else {
            return;
        };
        let result = self.set_matrix_cell(id, &key, &value);
        self.note_rules_result(id, result);
    }

    /// Enter in a matrix cell: the text is written as typed (a bare number
    /// in mm, `8mil` kept), as a grid cell's is. Untouched text writes
    /// nothing; a refusal keeps the cell open.
    fn submit_matrix_cell(&mut self, id: ProjectId) {
        let Some(edit) = self.rules_tab_mut(id).matrix.edit().cloned() else {
            return;
        };
        if edit.text.trim() == edit.opened.trim() {
            self.matrix_widget_update(id, MatrixMessage::EditCancelled);
            return;
        }
        let Some(key) = self.matrix_book(id).values().key(edit.pair) else {
            return;
        };
        let result = self.set_matrix_cell(id, &key, &typed_value(&edit.text));
        if self.note_rules_result(id, result).is_some() {
            self.matrix_widget_update(id, MatrixMessage::EditCancelled);
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
