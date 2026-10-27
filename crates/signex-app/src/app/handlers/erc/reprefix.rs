//! Re-prefix to standard (#1315): the `update` half.
//!
//! The Annotate dialog's button asks the libraries off-thread (a `Task`) for
//! the class and prefix override of every row the project's parts came from.
//! The answer carries a generation token; when it lands, the plan is made
//! against the state of THAT moment (standard, locks, sheets), so nothing the
//! user did while the libraries were read can be overwritten. The plan then
//! runs as one Incremental annotate pass with a re-prefix map: one undo group
//! across the open sheets, one history entry per sheet.

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;

use super::super::super::*;
use super::annotate::read_only_sheets;
use super::annotate_pass::AnnotatePass;
use super::reprefix_plan::{PlanInput, PlanSheet, SkipReason, plan_reprefix};
use crate::library::reprefix_lookup::{ReprefixLookup, ReprefixLookupJob};

const LABEL: &str = "Re-prefix to standard";
/// Parts named per Messages line; the rest are counted, not dropped silently.
const NAMED_PER_LINE: usize = 40;

impl Signex {
    /// The Annotate dialog's "Re-prefix to standard" button.
    pub(crate) fn handle_reprefix_pressed(&mut self) -> Task<Message> {
        let project = self.document_state.active_document_project();
        if project.is_some_and(|p| p.data.has_unreadable_annotation()) {
            crate::diagnostics::log_warning(format!(
                "{LABEL}: the designator standard of this project cannot be read by this \
                 version of Signex, so nothing was re-prefixed."
            ));
            return Task::none();
        }
        if project.and_then(|p| p.data.designator_standard()).is_none() {
            crate::diagnostics::log_warning(format!(
                "{LABEL}: the project follows no designator standard. Pick one in Project \
                 Options first."
            ));
            return Task::none();
        }
        let wanted = self.linked_library_ids();
        let libs = self
            .mounted_libraries()
            .into_iter()
            .filter(|lib| wanted.contains(&lib.id))
            .collect();
        self.ui_state.reprefix_generation += 1;
        let job = ReprefixLookupJob {
            generation: self.ui_state.reprefix_generation,
            libs,
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

    /// The library ids the project's placed parts link to.
    fn linked_library_ids(&self) -> HashSet<uuid::Uuid> {
        let (_pages, set) =
            crate::app::project_sheets::assemble_active_project_sheets(&self.document_state);
        set.sheets
            .iter()
            .flat_map(|(path, sheet)| {
                let sheet = self
                    .document_state
                    .engines
                    .get(path)
                    .map_or(sheet, |engine| engine.document());
                sheet.symbols.iter().filter_map(|s| s.library_id)
            })
            .collect()
    }

    /// The libraries answered. Plan against the current state and run the pass.
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
        for problem in &lookup.problems {
            crate::diagnostics::log_warning(problem);
        }
        let Some(standard) = self
            .document_state
            .active_document_project()
            .and_then(|p| p.data.designator_standard())
        else {
            crate::diagnostics::log_warning(format!(
                "{LABEL}: the project's designator standard was removed meanwhile; nothing \
                 was re-prefixed."
            ));
            return Task::none();
        };
        let Some(plan) = self.plan_for(standard, lookup) else {
            return Task::none();
        };
        report_skipped(&plan);
        crate::diagnostics::log_info(format!(
            "{LABEL}: {} part(s) to re-issue, {} already on the standard's prefix.",
            plan.parts_changed, plan.parts_conforming
        ));
        if plan.targets.is_empty() {
            return Task::none();
        }
        crate::diagnostics::log_info(format!(
            "{LABEL}: PCB references are not updated; the board keeps the old designators \
             until it is synchronised, the same as Annotate. Sheets that are not open are \
             rewritten on disk and cannot be undone."
        ));
        let pass = self.annotate_project(AnnotatePass {
            mode: signex_engine::AnnotateMode::Incremental,
            reprefix: Some(&plan.targets),
            label: LABEL,
        });
        // Unopened sheets were rewritten on disk, which the preview cannot see.
        self.invalidate_annotate_preview();
        Task::batch([pass, self.refresh_annotate_preview()])
    }

    /// The plan over the project as it is now, or `None` (already logged)
    /// when the project cannot be walked.
    fn plan_for(
        &self,
        standard: signex_types::designator::DesignatorStandard,
        lookup: &ReprefixLookup,
    ) -> Option<super::reprefix_plan::ReprefixPlan> {
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
                    lib_symbols: document.map_or(&EMPTY_LIB_SYMBOLS, |d| &d.lib_symbols),
                    read_only: read_only.contains(path),
                }
            })
            .collect();
        Some(plan_reprefix(&PlanInput {
            standard,
            sheets: &sheets,
            lookup,
            locked: &self.ui_state.annotate_locked,
            order: self.ui_state.annotate_order,
        }))
    }
}

static EMPTY_LIB_SYMBOLS: std::sync::LazyLock<
    std::collections::HashMap<String, signex_types::schematic::LibSymbol>,
> = std::sync::LazyLock::new(Default::default);

/// One Messages line per reason, naming the parts left alone.
fn report_skipped(plan: &super::reprefix_plan::ReprefixPlan) {
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
}
