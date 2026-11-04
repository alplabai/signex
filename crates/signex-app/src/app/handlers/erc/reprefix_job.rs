//! Re-prefix to standard without sheet reads in `update` (#1756).
//!
//! Policy: see docs/architecture/path-ops.md, "Re-prefix to standard".

use std::sync::Arc;

use iced::Task;

use super::super::super::*;
use super::annotate_job::{PassKind, ReadStage, Refused, SheetsRead};
use super::annotate_pass::AnnotatePass;
use super::reprefix::{LABEL, report_plan};
use super::reprefix_confirm::ReprefixConfirm;
use crate::app::project_sheets::ProjectSheetSet;
use crate::library::reprefix_lookup::ReprefixLookup;

/// What the plan-stage worker found, with the state it was asked under.
#[derive(Debug)]
pub struct ReprefixSheets {
    lookup: Arc<ReprefixLookup>,
    stage: ReadStage,
    sheets: Result<SheetsRead, String>,
}

impl Signex {
    /// Read the project's sheets on a worker; the plan is made when they arrive.
    pub(super) fn start_reprefix_plan_read(
        &mut self,
        generation: u64,
        lookup: Arc<ReprefixLookup>,
    ) -> Task<Message> {
        let Some((stage, plan)) = self.capture_sheet_read() else {
            crate::diagnostics::log_info(format!("{LABEL}: no sheet is open, nothing to do"));
            return Task::none();
        };
        Task::perform(
            crate::app::path_op_jobs::run_blocking(move || SheetsRead::assemble(&plan)),
            move |sheets| {
                Message::Annotate(AnnotateMsg::ReprefixSheetsRead {
                    generation,
                    result: Arc::new(ReprefixSheets {
                        lookup,
                        stage,
                        sheets,
                    }),
                })
            },
        )
    }

    /// The plan-stage read arrived: plan over it and show the confirm, or say
    /// why nothing is shown.
    pub(crate) fn handle_reprefix_sheets_read(
        &mut self,
        generation: u64,
        landed: &ReprefixSheets,
    ) -> Task<Message> {
        if generation != self.ui_state.reprefix_generation {
            crate::diagnostics::log_info(format!(
                "{LABEL}: a sheet read was dropped because a newer request replaced it."
            ));
            return Task::none();
        }
        if landed.lookup.project != self.active_project_path() {
            crate::diagnostics::log_warning(format!(
                "{LABEL}: a sheet read for another project was dropped; press the button again \
                 in this project."
            ));
            return Task::none();
        }
        let Some(standard) = self.reprefix_standard() else {
            return Task::none();
        };
        let Some(set) = self.reprefix_set(&landed.stage, &landed.sheets) else {
            return Task::none();
        };
        let plan = self.plan_for(standard, &landed.lookup, &set);
        report_plan(&plan);
        if plan.changes.is_empty() {
            return Task::none();
        }
        self.ui_state.reprefix_confirm = Some(ReprefixConfirm::from_plan(
            generation,
            landed.lookup.project.clone(),
            landed.lookup.clone(),
            standard,
            &plan,
        ));
        self.handle_detach_modal(super::super::super::state::ModalId::ReprefixConfirm)
    }

    /// The apply-stage read arrived (through the Annotate run): re-plan over
    /// the sheets and apply the plan unless it no longer matches what the user
    /// was shown.
    pub(super) fn apply_reprefix(
        &mut self,
        confirm: ReprefixConfirm,
        set: ProjectSheetSet,
    ) -> Task<Message> {
        if confirm.generation != self.ui_state.reprefix_generation
            || confirm.project != self.active_project_path()
        {
            crate::diagnostics::log_warning(format!(
                "{LABEL}: the request is out of date (the project or the dialog changed); \
                 nothing was re-prefixed. Press the button again."
            ));
            return Task::none();
        }
        let Some(standard) = self.reprefix_standard() else {
            return Task::none();
        };
        if self.refuse_unreadable(&set) {
            return Task::none();
        }
        let plan = self.plan_for(standard, &confirm.lookup, &set);
        let targets = plan.targets();
        if targets.is_empty() {
            crate::diagnostics::log_info(format!(
                "{LABEL}: nothing is left to re-prefix; the project changed since the list \
                 was shown."
            ));
            return Task::none();
        }
        if targets != confirm.shown {
            // Changed under the user's eyes: show the new list, apply nothing.
            crate::diagnostics::log_warning(format!(
                "{LABEL}: the project changed while the list was open, so nothing was applied. \
                 Review the updated list."
            ));
            self.ui_state.reprefix_confirm = Some(ReprefixConfirm::from_plan(
                confirm.generation,
                confirm.project,
                confirm.lookup,
                standard,
                &plan,
            ));
            return self.handle_detach_modal(super::super::super::state::ModalId::ReprefixConfirm);
        }
        let groups_before = self.document_state.next_undo_group;
        let applied = self.number_project(
            AnnotatePass {
                mode: signex_engine::AnnotateMode::Incremental,
                reprefix: Some(&targets),
                label: LABEL,
            },
            set,
        );
        debug_assert!(applied.writes.is_empty(), "a re-prefix writes nothing");
        // A run that was rolled back or changed nothing records no group; its
        // own message already said so, so the follow-up lines would mislead.
        if self.document_state.next_undo_group > groups_before
            && self.last_undo_group_record().is_some()
        {
            crate::diagnostics::log_info(format!(
                "{LABEL}: PCB references are not updated; the board keeps the old designators \
                 until it is synchronised, the same as Annotate."
            ));
            self.report_split_parts(&plan.changes);
            self.report_undo_sheets();
        }
        applied.task
    }

    /// Accept a read for planning: still current, every sheet readable.
    /// `None` after saying why not.
    fn reprefix_set(
        &mut self,
        stage: &ReadStage,
        sheets: &Result<SheetsRead, String>,
    ) -> Option<ProjectSheetSet> {
        let set = match self.accept_read(PassKind::Reprefix, stage, sheets) {
            Ok(set) => set,
            Err(Refused::Reported) => return None,
            Err(Refused::Because(why)) => {
                self.refuse_run(
                    LABEL,
                    &format!("{why}. Nothing was changed; press the button again."),
                );
                return None;
            }
        };
        (!self.refuse_unreadable(&set)).then_some(set)
    }

    /// A sheet that exists but cannot be read would be left out of the plan
    /// and split its parts across two prefixes: refuse the whole run, naming
    /// it. True when refused.
    fn refuse_unreadable(&mut self, set: &ProjectSheetSet) -> bool {
        if set.unreadable.is_empty() {
            return false;
        }
        let mut named: Vec<String> = set
            .unreadable
            .iter()
            .map(|(path, why)| format!("{} ({why})", path.display()))
            .collect();
        named.sort();
        self.refuse_run(
            LABEL,
            &format!(
                "{} sheet(s) could not be read: {}. Nothing was changed; fix them and press \
                 the button again.",
                named.len(),
                named.join(", ")
            ),
        );
        true
    }
}
