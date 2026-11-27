//! Parameters section of the Project tab (#1080): the title-block values
//! every sheet inherits (title, document number, revision, date, company,
//! drawn / checked / approved by) and user-defined variables, all usable as
//! `${NAME}` special strings. A sheet overrides a value in its own Title
//! Block; see `docs/USER_GUIDE.md`.
//!
//! The section is one bordered list (mockup row 30) with a Title block
//! and a Variables group, each folded shut by its header, and a filter over
//! both. A variable is renamed in place: Enter applies the typed name as one
//! General undo step, or shows why it is refused (`rename`).

mod rename;
mod view;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use signex_i18n::{MessageId, ids};

pub(super) use view::view;

use super::ProjectTabState;
use crate::app::Signex;
use crate::app::state::ProjectId;
use crate::panels::TitleBlockField;

/// A group of the Parameters list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterGroup {
    /// The sheet title-block fields, `TitleBlockField::ALL`.
    TitleBlock,
    /// Every other parameter: the user's variables.
    Variables,
}

/// The name of a title-block field in the Parameters list.
pub(super) fn field_words(field: TitleBlockField) -> MessageId {
    match field {
        TitleBlockField::Title => ids::PROJECT_PARAMETERS_FIELD_TITLE,
        TitleBlockField::DocumentNumber => ids::PROJECT_PARAMETERS_FIELD_DOCUMENT_NUMBER,
        TitleBlockField::Revision => ids::PROJECT_PARAMETERS_FIELD_REV,
        TitleBlockField::Date => ids::PROJECT_PARAMETERS_FIELD_DATE,
        TitleBlockField::Company => ids::PROJECT_PARAMETERS_FIELD_COMPANY,
        TitleBlockField::DrawnBy => ids::PROJECT_PARAMETERS_FIELD_DRAWN_BY,
        TitleBlockField::CheckedBy => ids::PROJECT_PARAMETERS_FIELD_CHECKED_BY,
        TitleBlockField::ApprovedBy => ids::PROJECT_PARAMETERS_FIELD_APPROVED_BY,
        TitleBlockField::Author => ids::PROJECT_PARAMETERS_FIELD_AUTHOR,
        TitleBlockField::Engineer => ids::PROJECT_PARAMETERS_FIELD_ENGINEER,
        TitleBlockField::Organization => ids::PROJECT_PARAMETERS_FIELD_ORGANIZATION,
        TitleBlockField::Address1 => ids::PROJECT_PARAMETERS_FIELD_ADDRESS_1,
        TitleBlockField::Address2 => ids::PROJECT_PARAMETERS_FIELD_ADDRESS_2,
        TitleBlockField::Address3 => ids::PROJECT_PARAMETERS_FIELD_ADDRESS_3,
        TitleBlockField::Address4 => ids::PROJECT_PARAMETERS_FIELD_ADDRESS_4,
    }
}

/// `key` is one of the title-block fields the Title block group shows.
pub(super) fn is_title_block_key(key: &str) -> bool {
    TitleBlockField::ALL.iter().any(|field| field.key() == key)
}

/// How many variables the project has, filtered or not.
pub(super) fn variable_count(parameters: &BTreeMap<String, String>) -> usize {
    parameters
        .keys()
        .filter(|key| !is_title_block_key(key))
        .count()
}

/// The rows of the Parameters list the filter leaves, in list order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct VisibleParameters<'a> {
    pub title_block: Vec<TitleBlockField>,
    pub variables: Vec<&'a str>,
}

/// The rows whose shown name or value holds `filter`, ignoring case. An
/// empty or blank filter leaves every row. `shown_name` is a title-block
/// field's name as the list shows it; a variable shows its key.
pub(super) fn visible_parameters<'a>(
    parameters: &'a BTreeMap<String, String>,
    filter: &str,
    shown_name: impl Fn(TitleBlockField) -> String,
) -> VisibleParameters<'a> {
    let needle = filter.trim().to_lowercase();
    let holds = |name: &str, value: &str| {
        needle.is_empty()
            || name.to_lowercase().contains(&needle)
            || value.to_lowercase().contains(&needle)
    };
    let value_of = |key: &str| parameters.get(key).map_or("", String::as_str);
    VisibleParameters {
        title_block: TitleBlockField::ALL
            .into_iter()
            .filter(|field| holds(&shown_name(*field), value_of(field.key())))
            .collect(),
        variables: parameters
            .iter()
            .filter(|(key, value)| !is_title_block_key(key) && holds(key, value))
            .map(|(key, _)| key.as_str())
            .collect(),
    }
}

impl ProjectTabState {
    /// The Parameters group is unfolded.
    pub(super) fn is_parameter_group_open(&self, group: ParameterGroup) -> bool {
        match group {
            ParameterGroup::TitleBlock => !self.parameter_title_block_closed,
            ParameterGroup::Variables => !self.parameter_variables_closed,
        }
    }
}

impl Signex {
    /// The Project tab state of a loaded project. `None` once the project
    /// is closed, so a late message creates no state for it.
    fn loaded_project_tab(&mut self, project_id: ProjectId) -> Option<&mut ProjectTabState> {
        self.document_state.project_by_id(project_id)?;
        Some(
            self.document_view
                .project_tabs
                .entry(project_id)
                .or_default(),
        )
    }

    pub(in crate::app::project_tab) fn edit_parameter_filter(
        &mut self,
        project_id: ProjectId,
        text: String,
    ) {
        if let Some(tab) = self.loaded_project_tab(project_id) {
            tab.parameter_filter = text;
        }
    }

    /// A group header clicked: fold the group shut, or open it again.
    pub(in crate::app::project_tab) fn toggle_parameter_group(
        &mut self,
        project_id: ProjectId,
        group: ParameterGroup,
    ) {
        if let Some(tab) = self.loaded_project_tab(project_id) {
            let closed = match group {
                ParameterGroup::TitleBlock => &mut tab.parameter_title_block_closed,
                ParameterGroup::Variables => &mut tab.parameter_variables_closed,
            };
            *closed = !*closed;
        }
    }
}
