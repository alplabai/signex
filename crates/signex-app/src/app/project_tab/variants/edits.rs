//! The Variants section's edits (#1690, #1293): add, rename in place, remove
//! and pick active go through the General history; selecting a variant in
//! the list is view state only.

use signex_i18n::{I18n, Localized, ids, t};
use signex_types::project::{MAX_VARIANT_NAME_CHARS, ProjectData, VariantProblem};

use super::super::VariantRenameDraft;
use crate::app::Signex;
use crate::app::state::ProjectId;

/// The words of a refused variant edit.
pub(super) fn problem_words(problem: &VariantProblem, i18n: &I18n) -> Localized {
    match problem {
        VariantProblem::Empty => t!(i18n, ids::PROJECT_VARIANT_EMPTY),
        VariantProblem::TooLong => {
            t!(i18n, ids::PROJECT_VARIANT_TOO_LONG, "max" => MAX_VARIANT_NAME_CHARS)
        }
        VariantProblem::Invalid => t!(i18n, ids::PROJECT_VARIANT_INVALID),
        VariantProblem::Reserved => t!(i18n, ids::PROJECT_VARIANT_RESERVED),
        VariantProblem::Taken(name) => {
            t!(i18n, ids::PROJECT_VARIANT_TAKEN, "name" => name.as_str())
        }
        VariantProblem::Unknown(name) => {
            t!(i18n, ids::PROJECT_VARIANT_UNKNOWN, "name" => name.as_str())
        }
    }
}

/// The first "Variant N" (N from 1) the project may take, in the language of
/// `i18n`. One of the first `definitions + 1` numbers is always free.
pub(super) fn first_free_variant_name(data: &ProjectData, i18n: &I18n) -> Option<String> {
    (1..=data.variant_definitions.len() + 1)
        .map(|n| i18n.message_plain(ids::PROJECT_VARIANTS_NEW_NAME, &[("n", n.into())]))
        .find(|name| data.check_variant_name(name, None).is_ok())
}

impl Signex {
    /// `+` or New variant from Base: add the first free "Variant N", select
    /// it and open its name for typing, so Enter keeps or renames it.
    pub(in crate::app::project_tab) fn add_new_variant(&mut self, project_id: ProjectId) {
        let Some(project) = self.document_state.project_by_id(project_id) else {
            crate::diagnostics::log_warning(
                "Project tab: the project was closed; the edit was dropped",
            );
            return;
        };
        let Some(name) = first_free_variant_name(&project.data, &self.ui_state.i18n) else {
            return;
        };
        let command = signex_engine::ProjectSectionCommand::AddVariant { name: name.clone() };
        let outcome = self.apply_project_section(project_id, &command);
        if let Some(tab) = self.document_state.project_tabs.get_mut(&project_id) {
            match outcome {
                Ok(true) => {
                    tab.variant_selected = Some(name.clone());
                    tab.variant_rename = Some(VariantRenameDraft {
                        from: name.clone(),
                        text: name,
                    });
                    tab.variant_problem = None;
                }
                Ok(false) => {}
                Err(refused) => tab.variant_problem = Some(refused),
            }
        }
    }

    /// A variant clicked in the list; `None` is Base.
    pub(in crate::app::project_tab) fn select_variant(
        &mut self,
        project_id: ProjectId,
        name: Option<String>,
    ) {
        let base = signex_types::project::BASE_VARIANT.to_string();
        if let Some(tab) = self.document_state.project_tabs.get_mut(&project_id) {
            // A half-typed rename of another variant is dropped with it.
            if tab
                .variant_rename
                .as_ref()
                .is_some_and(|draft| Some(&draft.from) != name.as_ref())
            {
                tab.variant_rename = None;
            }
            tab.variant_selected = Some(name.unwrap_or(base));
        }
    }

    /// Rename (or a double-click): the variant's name opens for typing.
    pub(in crate::app::project_tab) fn start_variant_rename(
        &mut self,
        project_id: ProjectId,
        name: String,
    ) {
        if let Some(tab) = self.document_state.project_tabs.get_mut(&project_id) {
            tab.variant_selected = Some(name.clone());
            tab.variant_rename = Some(VariantRenameDraft {
                from: name.clone(),
                text: name,
            });
            tab.variant_problem = None;
        }
    }

    pub(in crate::app::project_tab) fn edit_variant_rename(
        &mut self,
        project_id: ProjectId,
        from: String,
        text: String,
    ) {
        if let Some(tab) = self.document_state.project_tabs.get_mut(&project_id) {
            tab.variant_rename = Some(VariantRenameDraft { from, text });
            tab.variant_problem = None;
        }
    }

    /// Enter in a name box: rename the variant to what was typed. A refused
    /// name stays in the box with its reason; the selection follows a rename.
    pub(in crate::app::project_tab) fn submit_variant_rename(&mut self, project_id: ProjectId) {
        let Some(draft) = self
            .document_state
            .project_tabs
            .get_mut(&project_id)
            .and_then(|tab| tab.variant_rename.take())
        else {
            return;
        };
        if draft.text == draft.from {
            return;
        }
        let command = signex_engine::ProjectSectionCommand::RenameVariant {
            from: draft.from.clone(),
            to: draft.text.clone(),
        };
        let outcome = self.apply_project_section(project_id, &command);
        let Some(tab) = self.document_state.project_tabs.get_mut(&project_id) else {
            return;
        };
        match outcome {
            Ok(_) => {
                if tab
                    .variant_selected
                    .as_deref()
                    .is_some_and(|selected| selected.eq_ignore_ascii_case(&draft.from))
                {
                    tab.variant_selected = Some(draft.text.trim().to_string());
                }
            }
            Err(refused) => {
                tab.variant_problem = Some(refused);
                tab.variant_rename = Some(draft);
            }
        }
    }

    pub(in crate::app::project_tab) fn remove_project_variant(
        &mut self,
        project_id: ProjectId,
        name: String,
    ) {
        let command = signex_engine::ProjectSectionCommand::RemoveVariant { name: name.clone() };
        let outcome = self.apply_project_section(project_id, &command);
        if let Some(tab) = self.document_state.project_tabs.get_mut(&project_id) {
            // A half-typed rename of the removed variant goes with it.
            if tab.variant_rename.as_ref().is_some_and(|d| d.from == name) {
                tab.variant_rename = None;
            }
            // The selection falls back to the active variant.
            if outcome.is_ok()
                && tab
                    .variant_selected
                    .as_deref()
                    .is_some_and(|selected| selected.eq_ignore_ascii_case(&name))
            {
                tab.variant_selected = None;
            }
            tab.variant_problem = outcome.err();
        }
    }

    /// The Active variant select; `None` is Base.
    pub(in crate::app::project_tab) fn pick_active_variant(
        &mut self,
        project_id: ProjectId,
        name: Option<String>,
    ) {
        let command = signex_engine::ProjectSectionCommand::SetActiveVariant { name };
        let outcome = self.apply_project_section(project_id, &command);
        if let Some(tab) = self.document_state.project_tabs.get_mut(&project_id) {
            tab.variant_problem = outcome.err();
        }
    }
}
