//! The picker's and the popover's events. Apply, OK and Reset to base run
//! the section command `project_set_variant_alternate` and
//! `project_set_variant_override` run (`variant_parameters_command`), one
//! step of the General history; a refused one keeps the card open with its
//! reason.

use iced::Task;
use signex_i18n::{ids, t};

use super::VariantPartEditMsg;
use super::candidates::alternate_values;
use super::state::{CandidateRead, VariantPartEdit};
use crate::app::project_tab::{alternate_parameters, variant_parameters_command};
use crate::app::{Message, Signex};

/// What an event leaves to do once the card's own state is updated.
enum Then {
    Nothing,
    Close,
    ReadCandidates,
    ApplyAlternate,
    ApplyValue(Option<String>),
    PickParameter(String),
}

impl Signex {
    pub(crate) fn dispatch_variant_part_edit_message(
        &mut self,
        msg: VariantPartEditMsg,
    ) -> Task<Message> {
        match self.update_variant_part_edit(msg) {
            Then::Nothing => {}
            Then::Close => self.ui_state.variant_part_edit = None,
            Then::ReadCandidates => return self.read_variant_candidates(),
            Then::ApplyAlternate => self.apply_variant_alternate(),
            Then::ApplyValue(value) => self.apply_variant_value(value),
            Then::PickParameter(parameter) => self.pick_variant_parameter(parameter),
        }
        Task::none()
    }

    /// Update the open card for `msg`; what is left to do.
    fn update_variant_part_edit(&mut self, msg: VariantPartEditMsg) -> Then {
        let Some(edit) = self.ui_state.variant_part_edit.as_deref_mut() else {
            return Then::Nothing;
        };
        match (edit, msg) {
            (_, VariantPartEditMsg::CancelPressed) => Then::Close,
            (
                VariantPartEdit::Alternate(picker),
                VariantPartEditMsg::CandidatesRead {
                    generation,
                    part,
                    query,
                    read,
                },
            ) => {
                let awaited = generation == picker.generation
                    && part == picker.part.this_part.designator
                    && query == picker.query.trim();
                if awaited {
                    picker.read = match read {
                        Ok(candidates) => {
                            if candidates.wired.is_some() {
                                picker.wired.clone_from(&candidates.wired);
                            }
                            CandidateRead::Read(candidates)
                        }
                        Err(why) => CandidateRead::Failed(why),
                    };
                }
                Then::Nothing
            }
            (VariantPartEdit::Alternate(picker), VariantPartEditMsg::QueryEdited(query)) => {
                picker.query = query;
                Then::ReadCandidates
            }
            (VariantPartEdit::Alternate(picker), VariantPartEditMsg::SameFootprintToggled(on)) => {
                picker.same_footprint_only = on;
                Then::Nothing
            }
            (VariantPartEdit::Alternate(picker), VariantPartEditMsg::CandidatePicked(at)) => {
                if let CandidateRead::Read(candidates) = &picker.read {
                    picker.picked = candidates.all().nth(at).cloned();
                    picker.problem = None;
                }
                Then::Nothing
            }
            (VariantPartEdit::Alternate(picker), VariantPartEditMsg::ScopePicked(scope)) => {
                picker.scope = scope;
                Then::Nothing
            }
            (VariantPartEdit::Value(popover), VariantPartEditMsg::ScopePicked(scope)) => {
                popover.scope = scope;
                Then::Nothing
            }
            (VariantPartEdit::Alternate(_), VariantPartEditMsg::ApplyPressed) => {
                Then::ApplyAlternate
            }
            (VariantPartEdit::Value(popover), VariantPartEditMsg::ParameterListToggled) => {
                popover.parameters_open = !popover.parameters_open;
                Then::Nothing
            }
            (VariantPartEdit::Value(_), VariantPartEditMsg::ParameterPicked(parameter)) => {
                Then::PickParameter(parameter)
            }
            (VariantPartEdit::Value(popover), VariantPartEditMsg::ValueEdited(text)) => {
                popover.text = text;
                popover.problem = None;
                Then::Nothing
            }
            (VariantPartEdit::Value(popover), VariantPartEditMsg::ApplyPressed) => {
                if popover.can_apply() {
                    Then::ApplyValue(Some(popover.text.trim().to_string()))
                } else {
                    Then::Nothing
                }
            }
            (VariantPartEdit::Value(_), VariantPartEditMsg::ResetPressed) => Then::ApplyValue(None),
            // An event of the other card (a late read, a stale press).
            _ => Then::Nothing,
        }
    }

    /// Apply the picked candidate in the picker's scope; the picker closes
    /// on success and says why on a refusal.
    fn apply_variant_alternate(&mut self) {
        let Some(VariantPartEdit::Alternate(picker)) = self.ui_state.variant_part_edit.as_deref()
        else {
            return;
        };
        let Some(picked) = picker.picked.as_ref() else {
            return;
        };
        let part = &picker.part;
        let (mpn, value, footprint) =
            alternate_values(picked, &part.base.value, &part.base.footprint);
        let parts = part.parts(picker.scope);
        let parameters = alternate_parameters(Some(mpn.clone()), value, footprint);
        let Some(command) = variant_parameters_command(&part.variant, &parts, parameters) else {
            return;
        };
        let (project, designator, variant) = (
            part.project,
            part.this_part.designator.clone(),
            part.variant.clone(),
        );
        let done = t!(
            self.ui_state.i18n,
            ids::VARIANT_EDIT_ALTERNATE_DONE,
            "part" => designator,
            "mpn" => mpn,
            "variant" => variant
        );
        self.finish_variant_part_edit(project, &command, done.into_string());
    }

    /// Give the popover's parameter `value` in its scope, or the Base value
    /// again (`None`).
    fn apply_variant_value(&mut self, value: Option<String>) {
        let Some(VariantPartEdit::Value(popover)) = self.ui_state.variant_part_edit.as_deref()
        else {
            return;
        };
        let part = &popover.part;
        let parts = part.parts(popover.scope);
        let parameters = vec![(popover.parameter.clone(), value.clone())];
        let Some(command) = variant_parameters_command(&part.variant, &parts, parameters) else {
            return;
        };
        let (project, designator, variant, parameter) = (
            part.project,
            part.this_part.designator.clone(),
            part.variant.clone(),
            popover.parameter.clone(),
        );
        let done = match value {
            Some(value) => t!(
                self.ui_state.i18n,
                ids::VARIANT_EDIT_VALUE_DONE,
                "part" => designator,
                "parameter" => parameter,
                "value" => value,
                "variant" => variant
            ),
            None => t!(
                self.ui_state.i18n,
                ids::VARIANT_EDIT_RESET_DONE,
                "part" => designator,
                "parameter" => parameter,
                "variant" => variant
            ),
        };
        self.finish_variant_part_edit(project, &command, done.into_string());
    }

    /// Run `command`, one step of the General history: the card closes and
    /// Messages says `done`, or the card stays with the refusal.
    fn finish_variant_part_edit(
        &mut self,
        project: crate::app::ProjectId,
        command: &signex_engine::ProjectSectionCommand,
        done: String,
    ) {
        match self.apply_project_section(project, command) {
            Ok(_) => {
                self.ui_state.variant_part_edit = None;
                crate::diagnostics::log_info(done);
            }
            Err(problem) => {
                let why = Some(problem.to_string());
                match self.ui_state.variant_part_edit.as_deref_mut() {
                    Some(VariantPartEdit::Alternate(picker)) => picker.problem = why,
                    Some(VariantPartEdit::Value(popover)) => popover.problem = why,
                    None => {}
                }
            }
        }
    }

    /// The popover now edits `parameter`: its field holds the value the
    /// variant gives it, else the Base value.
    fn pick_variant_parameter(&mut self, parameter: String) {
        let settings = match self.ui_state.variant_part_edit.as_deref() {
            Some(VariantPartEdit::Value(popover)) => self
                .document_state
                .project_by_id(popover.part.project)
                .map(|project| project.data.variant_settings.clone())
                .unwrap_or_default(),
            _ => return,
        };
        let Some(VariantPartEdit::Value(popover)) = self.ui_state.variant_part_edit.as_deref_mut()
        else {
            return;
        };
        popover.text = popover
            .part
            .own_value(&settings, &parameter)
            .unwrap_or(popover.part.base.base_value(&parameter))
            .to_string();
        popover.parameter = parameter;
        popover.parameters_open = false;
        popover.problem = None;
    }
}
