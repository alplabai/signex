//! Sheet Variables edits from the Properties panel (#1987). A value commits
//! on Enter as one undoable `SetSheetVariable`; a variable whose expression
//! does not evaluate (a cycle, an unknown name) is reported in Messages.

use super::super::super::*;
use crate::panels::SheetVariableMsg;

impl Signex {
    pub(crate) fn edit_sheet_variables(&mut self, edit: SheetVariableMsg) {
        let Some(sheet) = self.document_state.active_path.clone() else {
            return;
        };
        let drafts = &self.document_state.drafts;
        match edit {
            SheetVariableMsg::ValueTyped { name, text } => {
                drafts.set_sheet_variable(&sheet, Some(name.as_str()), &text);
            }
            SheetVariableMsg::ValueSubmitted(name) => {
                if let Some(draft) = drafts.take_sheet_variable(&sheet, Some(name.as_str())) {
                    self.set_sheet_variable(name, Some(draft.text.trim().to_string()));
                }
            }
            SheetVariableMsg::Removed(name) => {
                // A half-typed value of a removed variable has nothing to
                // commit to.
                drafts.take_sheet_variable(&sheet, Some(name.as_str()));
                self.set_sheet_variable(name, None);
            }
            SheetVariableMsg::NewNameTyped(text) => {
                drafts.set_sheet_variable(&sheet, None, &text);
            }
            SheetVariableMsg::Added => self.add_sheet_variable(&sheet),
        }
    }

    /// Add the name typed in the Add row with an empty value. A name that is
    /// not an identifier, or that the sheet already has, is refused in
    /// Messages and stays typed.
    fn add_sheet_variable(&mut self, sheet: &std::path::Path) {
        let drafts = &self.document_state.drafts;
        let name = drafts
            .sheet_variable_text(sheet, None)
            .unwrap_or_default()
            .trim()
            .to_string();
        if name.is_empty() {
            return;
        }
        if !signex_types::variables::is_variable_name(&name) {
            crate::diagnostics::log_warning(format!(
                "Sheet variable '{name}' was not added: a name is letters, digits and _, \
                 and does not start with a digit."
            ));
            return;
        }
        let exists = self
            .document_state
            .engines
            .get(sheet)
            .is_some_and(|engine| engine.document().variables.contains_key(&name));
        if exists {
            crate::diagnostics::log_warning(format!(
                "Sheet variable '{name}' was not added: the sheet already has it."
            ));
            return;
        }
        drafts.take_sheet_variable(sheet, None);
        self.set_sheet_variable(name, Some(String::new()));
    }

    /// One sheet-variable edit of the active sheet, then its variables that
    /// do not evaluate are reported.
    fn set_sheet_variable(&mut self, name: String, value: Option<String>) {
        let changed = self.apply_engine_command(
            signex_engine::Command::SetSheetVariable { name, value },
            false,
            false,
        );
        if changed {
            self.report_sheet_variable_issues();
        }
    }

    /// Write the active sheet's issues of its variables to Messages.
    fn report_sheet_variable_issues(&self) {
        let Some(path) = self.document_state.active_path.as_deref() else {
            return;
        };
        let Some(engine) = self.document_state.engines.get(path) else {
            return;
        };
        let no_project = std::collections::BTreeMap::new();
        let project = self
            .document_state
            .document_project(path)
            .map_or(&no_project, |project| &project.data.parameters);
        let variables = &engine.document().variables;
        for issue in signex_types::variables::sheet_variable_issues(variables, project) {
            crate::diagnostics::log_warning(format!("{}: {issue}", path.display()));
        }
    }

    /// Commit the pending sheet-variable values of the sheets `matches`
    /// picks, as a save, an export or a close commits title-block drafts.
    pub(crate) fn commit_sheet_variable_drafts_where(
        &mut self,
        matches: impl Fn(&std::path::Path) -> bool,
    ) {
        let picked = self
            .document_state
            .drafts
            .take_sheet_variables_where(matches);
        let active = self.document_state.active_path.clone();
        for draft in picked {
            let Some(name) = draft.name else {
                continue;
            };
            let value = Some(draft.text.trim().to_string());
            if Some(&draft.sheet) == active.as_ref() {
                self.set_sheet_variable(name, value);
            } else {
                self.apply_command_to_sheet(
                    &draft.sheet,
                    signex_engine::Command::SetSheetVariable { name, value },
                );
            }
        }
    }
}
