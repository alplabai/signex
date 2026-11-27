//! Sheet Variables edits from the Properties panel (#1987). A value commits
//! on Enter as one undoable `SetSheetVariable` through
//! [`Signex::set_sheet_variable`], the same call the `set_sheet_variable`
//! command runs from operands, so the panel, the CLI and an agent end in one
//! behaviour. A variable whose expression does not evaluate (a cycle, an
//! unknown name) is reported in Messages.

use super::super::super::*;
use crate::app::mutation_gateway::MutationOutcome;
use crate::panels::SheetVariableMsg;

impl Signex {
    pub(crate) fn edit_sheet_variables(&mut self, edit: SheetVariableMsg) {
        let Some(sheet) = self.document_state.active_path.clone() else {
            return;
        };
        let drafts = &self.document_view.drafts;
        match edit {
            SheetVariableMsg::ValueTyped { name, text } => {
                drafts.set_sheet_variable(&sheet, Some(name.as_str()), &text);
            }
            SheetVariableMsg::ValueSubmitted(name) => {
                if let Some(draft) = drafts.take_sheet_variable(&sheet, Some(name.as_str())) {
                    let value = Some(draft.text.trim().to_string());
                    self.set_sheet_variable_from_panel(&name, value);
                }
            }
            SheetVariableMsg::Removed(name) => {
                // A half-typed value of a removed variable has nothing to
                // commit to.
                drafts.take_sheet_variable(&sheet, Some(name.as_str()));
                self.set_sheet_variable_from_panel(&name, None);
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
        let drafts = &self.document_view.drafts;
        let name = drafts
            .sheet_variable_text(sheet, None)
            .unwrap_or_default()
            .trim()
            .to_string();
        if name.is_empty() {
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
        match self.set_sheet_variable(&name, Some(String::new())) {
            Ok(_) => {
                self.document_view.drafts.take_sheet_variable(sheet, None);
            }
            Err(why) => crate::diagnostics::log_warning(format!(
                "Sheet variable '{name}' was not added: {why}"
            )),
        }
    }

    /// A panel edit: [`Self::set_sheet_variable`], its refusal in Messages.
    fn set_sheet_variable_from_panel(&mut self, name: &str, value: Option<String>) {
        if let Err(why) = self.set_sheet_variable(name, value) {
            crate::diagnostics::log_warning(format!("Sheet variable '{name}': {why}"));
        }
    }

    /// Set the variable `name` of the sheet an edit acts on to `value`, or
    /// remove it (`None`), as one undo step; then its variables that do not
    /// evaluate are reported. A new name must be an identifier (letters,
    /// digits and `_`, not starting with a digit); removing names one the
    /// sheet has. `Ok(changed)`, or why nothing was.
    pub(crate) fn set_sheet_variable(
        &mut self,
        name: &str,
        value: Option<String>,
    ) -> Result<bool, String> {
        let name = name.trim();
        let engine = self
            .document_state
            .edit_engine()
            .ok_or("no schematic is open")?;
        let exists = engine.document().variables.contains_key(name);
        if value.is_none() && !exists {
            return Err(format!("the sheet has no variable `{name}`"));
        }
        if !exists && !signex_types::variables::is_variable_name(name) {
            return Err(format!(
                "`{name}` is not a variable name: a name is letters, digits and _, \
                 and does not start with a digit"
            ));
        }
        let command = signex_engine::Command::SetSheetVariable {
            name: name.to_string(),
            value,
        };
        let (outcome, refusal) = self.apply_engine_command_reporting(command, false, false);
        if let Some(reason) = refusal {
            return Err(reason);
        }
        match outcome {
            MutationOutcome::Failed => Err("the edit was refused".to_string()),
            MutationOutcome::Unchanged => Ok(false),
            MutationOutcome::Changed => {
                self.report_sheet_variable_issues();
                Ok(true)
            }
        }
    }

    /// Write the issues of the edited sheet's variables to Messages.
    fn report_sheet_variable_issues(&self) {
        let Some(path) = self.document_state.edit_path() else {
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
            .document_view
            .drafts
            .take_sheet_variables_where(matches);
        let active = self.document_state.active_path.clone();
        for draft in picked {
            let Some(name) = draft.name else {
                continue;
            };
            let value = Some(draft.text.trim().to_string());
            if Some(&draft.sheet) == active.as_ref() {
                self.set_sheet_variable_from_panel(&name, value);
            } else {
                self.apply_command_to_sheet(
                    &draft.sheet,
                    signex_engine::Command::SetSheetVariable { name, value },
                );
            }
        }
    }
}
