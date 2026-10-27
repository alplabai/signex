//! Re-prefix to standard (#1315): the `update` half.
//!
//! The Annotate dialog's button asks the libraries off-thread (a `Task`) for
//! the class and prefix override of every row of every mounted library. The
//! answer carries a generation token and the project it was asked for; when
//! it lands, the plan is made against the state of THAT moment and shown in a
//! confirm. Confirm re-plans against the state of ITS moment and runs the plan
//! as one Incremental annotate pass with a re-prefix map: one undo group
//! across every sheet, with unopened sheets parked dirty and unsaved.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;
use signex_types::designator::DesignatorStandard;

use super::super::super::*;
use super::annotate::read_only_sheets;
use super::annotate_pass::AnnotatePass;
use super::reprefix_confirm::ReprefixConfirm;
use super::reprefix_plan::{
    PartChange, PlanInput, PlanSheet, ReprefixPlan, SkipReason, plan_reprefix,
};
use crate::library::reprefix_lookup::{ReprefixLookup, ReprefixLookupJob};

const LABEL: &str = "Re-prefix to standard";
/// Parts named per Messages line; the rest are counted, not dropped silently.
const NAMED_PER_LINE: usize = 40;

impl Signex {
    /// The `.snxprj` path of the active project: the identity a lookup
    /// result is checked against.
    fn active_project_path(&self) -> Option<PathBuf> {
        self.document_state
            .active_document_project()
            .map(|p| p.path.clone())
    }

    /// The project's standard, or `None` after saying why there is none.
    fn reprefix_standard(&self) -> Option<DesignatorStandard> {
        let project = self.document_state.active_document_project();
        if project.is_some_and(|p| p.data.has_unreadable_annotation()) {
            crate::diagnostics::log_warning(format!(
                "{LABEL}: the designator standard of this project cannot be read by this \
                 version of Signex, so nothing was re-prefixed."
            ));
            return None;
        }
        let standard = project.and_then(|p| p.data.designator_standard());
        if standard.is_none() {
            crate::diagnostics::log_warning(format!(
                "{LABEL}: the project follows no designator standard. Pick one in Project \
                 Options first."
            ));
        }
        standard
    }

    /// The Annotate dialog's "Re-prefix to standard" button.
    pub(crate) fn handle_reprefix_pressed(&mut self) -> Task<Message> {
        if self.reprefix_standard().is_none() {
            return Task::none();
        }
        // A newer request replaces whatever is open or in flight.
        self.ui_state.reprefix_generation += 1;
        self.ui_state.reprefix_confirm = None;
        let job = ReprefixLookupJob {
            generation: self.ui_state.reprefix_generation,
            project: self.active_project_path(),
            libs: self.mounted_libraries(),
        };
        let generation = job.generation;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || job.run())
                    .await
                    .map_err(|e| format!("{LABEL}: the library lookup failed: {e}"))
            },
            move |result| {
                Message::Annotate(AnnotateMsg::ReprefixRowsResolved {
                    generation,
                    result: Arc::new(result),
                })
            },
        )
    }

    /// Make every in-flight lookup and any open confirm stale.
    pub(crate) fn invalidate_reprefix(&mut self) {
        self.ui_state.reprefix_generation += 1;
        self.ui_state.reprefix_confirm = None;
    }

    /// The libraries answered. Plan against the current state and show it.
    pub(crate) fn handle_reprefix_rows_resolved(
        &mut self,
        generation: u64,
        result: &Result<ReprefixLookup, String>,
    ) -> Task<Message> {
        if generation != self.ui_state.reprefix_generation {
            crate::diagnostics::log_info(format!(
                "{LABEL}: a library lookup was dropped because a newer request replaced it."
            ));
            return Task::none();
        }
        let lookup = match result {
            Ok(lookup) => lookup,
            Err(problem) => {
                crate::diagnostics::log_warning(problem);
                return Task::none();
            }
        };
        if lookup.project != self.active_project_path() {
            crate::diagnostics::log_warning(format!(
                "{LABEL}: a library lookup for another project was dropped; press the button \
                 again in this project."
            ));
            return Task::none();
        }
        for problem in &lookup.problems {
            crate::diagnostics::log_warning(problem);
        }
        let Some(standard) = self.reprefix_standard() else {
            return Task::none();
        };
        let plan = self.plan_for(standard, lookup);
        report_plan(&plan);
        if plan.changes.is_empty() {
            return Task::none();
        }
        self.ui_state.reprefix_confirm = Some(ReprefixConfirm::from_plan(
            generation,
            lookup.project.clone(),
            Arc::new(lookup.clone()),
            standard,
            &plan,
        ));
        self.handle_detach_modal(super::super::super::state::ModalId::ReprefixConfirm)
    }

    /// Cancel or Esc on the confirm: nothing changes.
    pub(crate) fn handle_reprefix_cancelled(&mut self) -> Task<Message> {
        if self.ui_state.reprefix_confirm.take().is_some() {
            crate::diagnostics::log_info(format!("{LABEL}: cancelled, nothing was changed."));
        }
        self.close_detached_modal(super::super::super::state::ModalId::ReprefixConfirm)
    }

    /// Confirm: re-plan against the state of now, and apply it unless it no
    /// longer matches what the user was shown.
    pub(crate) fn handle_reprefix_confirmed(&mut self) -> Task<Message> {
        let Some(confirm) = self.ui_state.reprefix_confirm.take() else {
            return Task::none();
        };
        let close = self.close_detached_modal(super::super::super::state::ModalId::ReprefixConfirm);
        if confirm.generation != self.ui_state.reprefix_generation
            || confirm.project != self.active_project_path()
        {
            crate::diagnostics::log_warning(format!(
                "{LABEL}: the request is out of date (the project or the dialog changed); \
                 nothing was re-prefixed. Press the button again."
            ));
            return close;
        }
        let Some(standard) = self.reprefix_standard() else {
            return close;
        };
        let plan = self.plan_for(standard, &confirm.lookup);
        let targets = plan.targets();
        if targets.is_empty() {
            crate::diagnostics::log_info(format!(
                "{LABEL}: nothing is left to re-prefix; the project changed since the list \
                 was shown."
            ));
            return close;
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
            return Task::batch([
                close,
                self.handle_detach_modal(super::super::super::state::ModalId::ReprefixConfirm),
            ]);
        }
        crate::diagnostics::log_info(format!(
            "{LABEL}: PCB references are not updated; the board keeps the old designators \
             until it is synchronised, the same as Annotate."
        ));
        let pass = self.annotate_project(AnnotatePass {
            mode: signex_engine::AnnotateMode::Incremental,
            reprefix: Some(&targets),
            label: LABEL,
        });
        self.report_split_parts(&plan.changes);
        // The pass parked unopened sheets; the preview must re-read them.
        self.invalidate_annotate_preview();
        Task::batch([close, pass, self.refresh_annotate_preview()])
    }

    /// Name every part whose units ended up under different prefixes (a sheet
    /// that failed mid-pass is rolled back whole, and its parts are then only
    /// half converted).
    fn report_split_parts(&self, changes: &[PartChange]) {
        let by_uuid: std::collections::HashMap<uuid::Uuid, &str> = self
            .document_state
            .engines
            .values()
            .flat_map(|engine| engine.document().symbols.iter())
            .map(|s| (s.uuid, s.reference.as_str()))
            .collect();
        for change in changes {
            let prefixes: std::collections::BTreeSet<&str> = change
                .units
                .iter()
                .filter_map(|u| by_uuid.get(u))
                .map(|r| signex_types::designator::alphabetic_prefix(r))
                .collect();
            if prefixes.len() > 1 {
                crate::diagnostics::log_warning(format!(
                    "{LABEL}: part {} was left with units under different prefixes ({}). \
                     Undo the re-prefix, or fix its units by hand.",
                    change.reference,
                    prefixes.into_iter().collect::<Vec<_>>().join(", ")
                ));
            }
        }
    }

    /// The plan over the project as it is now.
    fn plan_for(&self, standard: DesignatorStandard, lookup: &ReprefixLookup) -> ReprefixPlan {
        let (_pages, set) =
            crate::app::project_sheets::assemble_active_project_sheets(&self.document_state);
        if !set.unreadable.is_empty() {
            crate::diagnostics::log_warning(format!(
                "{LABEL}: {} sheet(s) could not be read and are left alone.",
                set.unreadable.len()
            ));
        }
        let read_only = read_only_sheets(&self.document_state, &set);
        let ordered: Vec<PathBuf> = crate::app::project_sheets::ordered_project_sheet_paths(
            &set,
            self.document_state.active_path.as_deref(),
        );
        let no_lib_symbols = std::collections::HashMap::new();
        let sheets: Vec<PlanSheet<'_>> = ordered
            .iter()
            .map(|path| {
                let document = self
                    .document_state
                    .engines
                    .get(path)
                    .map(|engine| engine.document())
                    .or_else(|| set.sheets.get(path));
                PlanSheet {
                    symbols: document.map_or(&[][..], |d| d.symbols.as_slice()),
                    lib_symbols: document.map_or(&no_lib_symbols, |d| &d.lib_symbols),
                    read_only: read_only.contains(path),
                }
            })
            .collect();
        plan_reprefix(&PlanInput {
            standard,
            sheets: &sheets,
            lookup,
            locked: &self.ui_state.annotate_locked,
            order: self.ui_state.annotate_order,
        })
    }
}

/// Everything a plan found that the user must hear about: refused overrides,
/// one line per skip reason, and the counts.
fn report_plan(plan: &ReprefixPlan) {
    for invalid in plan.invalid_overrides.values() {
        crate::diagnostics::log_warning(format!(
            "{LABEL}: the designator prefix override {:?} of library row {} is ignored: {}.",
            invalid.raw, invalid.internal_pn, invalid.why
        ));
    }
    let mut by_reason: BTreeMap<SkipReason, Vec<&str>> = BTreeMap::new();
    for skipped in &plan.skipped {
        by_reason
            .entry(skipped.reason)
            .or_default()
            .push(&skipped.reference);
    }
    for (reason, mut names) in by_reason {
        names.sort_unstable();
        let total = names.len();
        let shown = names
            .iter()
            .take(NAMED_PER_LINE)
            .copied()
            .collect::<Vec<_>>();
        let more = if total > shown.len() {
            format!(" and {} more", total - shown.len())
        } else {
            String::new()
        };
        crate::diagnostics::log_warning(format!(
            "{LABEL}: {total} part(s) {}, left unchanged: {}{more}.",
            reason.heading(),
            shown.join(", ")
        ));
    }
    crate::diagnostics::log_info(format!(
        "{LABEL}: {} part(s) to re-issue, {} already on the standard's prefix.",
        plan.changes.len(),
        plan.parts_conforming
    ));
}
