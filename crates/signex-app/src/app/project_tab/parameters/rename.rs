//! Renaming a variable in place (mockup row 30). The typed name is a
//! draft until Enter; the name is checked here, before the engine sees it,
//! and a refused one stays in its box with the reason under the row.

use std::collections::BTreeMap;

use signex_engine::ProjectGeneralCommand;
use signex_i18n::{I18n, Localized, ids, t};

use crate::app::Signex;
use crate::app::state::ProjectId;

/// Why a typed variable name is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ParameterNameProblem {
    /// Nothing but blanks.
    Empty,
    /// A line break or another control character, or not a variable name
    /// (letters, digits and `_`, not starting with a digit).
    Invalid,
    /// The key of a title-block field.
    TitleBlock,
    /// Another parameter has the name; names are case-sensitive.
    Taken,
}

/// The name `from` is renamed to: `text` trimmed, `None` when that is `from`
/// itself (nothing to do), or why it is refused.
pub(super) fn checked_rename(
    parameters: &BTreeMap<String, String>,
    from: &str,
    text: &str,
) -> Result<Option<String>, ParameterNameProblem> {
    let name = text.trim();
    if name.is_empty() {
        return Err(ParameterNameProblem::Empty);
    }
    if name.chars().any(char::is_control) {
        return Err(ParameterNameProblem::Invalid);
    }
    if name == from {
        return Ok(None);
    }
    // A parameter is a variable (#1987): `${NAME}` and an expression can
    // only name an identifier, as the Add row already requires.
    if !signex_types::variables::is_variable_name(name) {
        return Err(ParameterNameProblem::Invalid);
    }
    if super::is_title_block_key(name) {
        return Err(ParameterNameProblem::TitleBlock);
    }
    if parameters.contains_key(name) {
        return Err(ParameterNameProblem::Taken);
    }
    Ok(Some(name.to_string()))
}

/// What the row says under a name box being typed over: a refusal shown
/// after Enter, or, while the typed name would be accepted, the warning that
/// sheets referring to the old name stop resolving it (the rename moves only
/// the project key; see `docs/USER_GUIDE.md`, "Parameters").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RenameNote {
    Refused(ParameterNameProblem),
    ReferencesKeepOldName,
}

/// The note under the name box of `from` with `typed` in it; `refused` is
/// whether Enter was refused since the last keystroke.
pub(super) fn rename_note(
    parameters: &BTreeMap<String, String>,
    from: &str,
    typed: &str,
    refused: bool,
) -> Option<RenameNote> {
    match checked_rename(parameters, from, typed) {
        Err(problem) => refused.then_some(RenameNote::Refused(problem)),
        Ok(Some(_)) => Some(RenameNote::ReferencesKeepOldName),
        Ok(None) => None,
    }
}

/// The words of a refused name; `typed` is the name as typed.
pub(super) fn problem_words(problem: ParameterNameProblem, typed: &str, i18n: &I18n) -> Localized {
    let name = typed.trim().to_string();
    match problem {
        ParameterNameProblem::Empty => t!(i18n, ids::PROJECT_PARAMETER_NAME_EMPTY),
        ParameterNameProblem::Invalid => t!(i18n, ids::PROJECT_PARAMETER_NAME_INVALID),
        ParameterNameProblem::TitleBlock => {
            t!(i18n, ids::PROJECT_PARAMETER_NAME_TITLE_BLOCK, "name" => name)
        }
        ParameterNameProblem::Taken => t!(i18n, ids::PROJECT_PARAMETER_NAME_TAKEN, "name" => name),
    }
}

impl Signex {
    /// A variable's name as typed over: kept as the draft, and a shown
    /// refusal goes with the next keystroke.
    pub(in crate::app::project_tab) fn edit_parameter_rename(
        &mut self,
        project_id: ProjectId,
        from: String,
        text: String,
    ) {
        if let Some(tab) = self.loaded_project_tab(project_id) {
            tab.parameter_rename = Some((from, text));
            tab.parameter_rename_refused = false;
        }
    }

    /// Enter in a name box: rename the variable to what was typed, as one
    /// General undo step. A refused name changes nothing and stays in the
    /// box with its reason.
    pub(in crate::app::project_tab) fn submit_parameter_rename(&mut self, project_id: ProjectId) {
        let Some(project) = self.document_state.project_by_id(project_id) else {
            return;
        };
        let Some((from, text)) = self
            .document_state
            .project_tabs
            .get(&project_id)
            .and_then(|tab| tab.parameter_rename.clone())
        else {
            return;
        };
        let outcome = checked_rename(&project.data.parameters, &from, &text);
        let Some(tab) = self.document_state.project_tabs.get_mut(&project_id) else {
            return;
        };
        let to = match outcome {
            Err(_) => {
                tab.parameter_rename_refused = true;
                return;
            }
            Ok(to) => to,
        };
        tab.parameter_rename = None;
        tab.parameter_rename_refused = false;
        let Some(to) = to else {
            return;
        };
        self.close_general_run(project_id);
        self.apply_project_general(
            project_id,
            ProjectGeneralCommand::RenameParameter { from, to },
        );
        self.close_general_run(project_id);
    }
}
