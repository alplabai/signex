//! The Variant Manager tab's edits (mockup row 32): a cell's fit, a part's
//! parameter values in a variant, Duplicate and Set as default. Each goes
//! through an engine section command and is one undo step of the General
//! history; a refused one shows its reason and records nothing. Selecting a
//! cell is view state only. A cell edit names the placed part from the parts
//! read, so the setting stays on it through a re-annotation.

use signex_engine::ProjectSectionCommand;
use signex_i18n::{I18n, ids};
use signex_output::VariantMatrix;
use signex_types::pcb::SymbolLink;
use signex_types::project::{BASE_VARIANT, PlacedPart, ProjectData, SheetFit, is_base_variant};

use super::super::{ProjectTabState, VariantRenameDraft};
use super::manager::{OverrideDraft, VariantCell, VariantManagerMsg};
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};

impl Signex {
    pub(in crate::app::project_tab) fn dispatch_variant_manager(
        &mut self,
        project_id: ProjectId,
        msg: VariantManagerMsg,
    ) -> iced::Task<Message> {
        match msg {
            VariantManagerMsg::CellSelected(cell) => {
                self.store_variant_draft(project_id);
                if let Some(tab) = self.document_state.project_tabs.get_mut(&project_id) {
                    tab.variant_manager.cell = Some(cell);
                    tab.variant_manager.draft = None;
                }
            }
            VariantManagerMsg::FitPicked { cell, fitted } => {
                self.pick_variant_fit(project_id, cell, fitted);
            }
            VariantManagerMsg::OverrideEdited { parameter, text } => {
                let moved_on = self
                    .variant_draft(project_id)
                    .is_some_and(|draft| draft.parameter != parameter);
                if moved_on {
                    self.store_variant_draft(project_id);
                }
                if let Some(tab) = self.document_state.project_tabs.get_mut(&project_id) {
                    tab.variant_manager.draft = Some(OverrideDraft { parameter, text });
                }
            }
            VariantManagerMsg::OverrideSubmitted { cell } => {
                self.submit_variant_override(project_id, cell);
            }
            VariantManagerMsg::OverrideReset { cell, parameter } => {
                self.set_variant_override(project_id, cell, parameter, None);
            }
            VariantManagerMsg::DuplicateRequested { from } => {
                self.duplicate_variant(project_id, from);
                return self.read_variant_parts(project_id, false);
            }
            VariantManagerMsg::DefaultPicked { name } => {
                let command = ProjectSectionCommand::SetDefaultVariant { name };
                self.apply_variant_manager_edit(project_id, &command);
            }
        }
        iced::Task::none()
    }

    /// Apply one Variant Manager edit; a refusal is shown in the list.
    fn apply_variant_manager_edit(
        &mut self,
        project_id: ProjectId,
        command: &ProjectSectionCommand,
    ) -> bool {
        let outcome = self.apply_project_section(project_id, command);
        let Some(tab) = self.document_state.project_tabs.get_mut(&project_id) else {
            return false;
        };
        tab.variant_problem = outcome.as_ref().err().cloned();
        matches!(outcome, Ok(true))
    }

    /// The parts read of the project's tab, whether or not it still holds.
    fn variant_read(&self, project_id: ProjectId) -> Option<&VariantMatrix> {
        let tab = self.document_state.project_tabs.get(&project_id)?;
        tab.variant_parts.as_deref()?.as_ref().ok()
    }

    /// The placed part the parts read lists under `designator`.
    fn variant_part_link(&self, project_id: ProjectId, designator: &str) -> Option<SymbolLink> {
        let matrix = self.variant_read(project_id)?;
        let row = matrix
            .rows
            .iter()
            .find(|row| row.designator == designator)?;
        row.link.clone()
    }

    fn variant_draft(&self, project_id: ProjectId) -> Option<&OverrideDraft> {
        let tab = self.document_state.project_tabs.get(&project_id)?;
        tab.variant_manager.draft.as_ref()
    }

    /// The Fitting switch. Picking the fit the sheets already give a cell
    /// that sets none of its own changes nothing and records nothing.
    fn pick_variant_fit(&mut self, project_id: ProjectId, cell: VariantCell, fitted: Option<bool>) {
        let Some(variant) = cell.variant.clone() else {
            return;
        };
        let part = self.variant_part_link(project_id, &cell.designator);
        if let Some(picked) = fitted
            && self.repeats_sheet_fit(project_id, &cell, part.as_ref(), picked)
        {
            return;
        }
        let command = ProjectSectionCommand::SetVariantPartFit {
            variant,
            designator: cell.designator,
            part,
            fitted,
        };
        self.apply_variant_manager_edit(project_id, &command);
    }

    /// Whether `cell` sets no fit of its own and its sheets fit it as
    /// `picked` already.
    fn repeats_sheet_fit(
        &self,
        project_id: ProjectId,
        cell: &VariantCell,
        part: Option<&SymbolLink>,
        picked: bool,
    ) -> bool {
        let Some(project) = self.document_state.project_by_id(project_id) else {
            return false;
        };
        let placed = PlacedPart::new(&cell.designator, part);
        let settings = &project.data.variant_settings;
        if settings
            .part(cell.variant.as_deref(), placed)
            .is_some_and(|own| own.fitted.is_some())
        {
            return false;
        }
        let Some(matrix) = self.variant_read(project_id) else {
            return false;
        };
        let sheets = matrix.column(cell.variant.as_deref()).and_then(|column| {
            let row = matrix
                .rows
                .iter()
                .find(|row| row.designator == cell.designator)?;
            row.fitted.get(column).copied()
        });
        sheets == Some(picked)
    }

    /// Typing moved on to another parameter or another cell: what was typed
    /// is stored, as Enter stores it.
    fn store_variant_draft(&mut self, project_id: ProjectId) {
        let cell = self
            .document_state
            .project_tabs
            .get(&project_id)
            .filter(|tab| tab.variant_manager.draft.is_some())
            .and_then(|tab| tab.variant_manager.cell.clone());
        if let Some(cell) = cell {
            self.submit_variant_override(project_id, cell);
        }
    }

    /// Enter in a parameter value: store what was typed; an empty value is
    /// the Base value again.
    fn submit_variant_override(&mut self, project_id: ProjectId, cell: VariantCell) {
        let Some(draft) = self
            .document_state
            .project_tabs
            .get_mut(&project_id)
            .and_then(|tab| tab.variant_manager.draft.take())
        else {
            return;
        };
        let value = Some(draft.text).filter(|text| !text.trim().is_empty());
        self.set_variant_override(project_id, cell, draft.parameter, value);
    }

    fn set_variant_override(
        &mut self,
        project_id: ProjectId,
        cell: VariantCell,
        parameter: String,
        value: Option<String>,
    ) {
        let Some(variant) = cell.variant else {
            return;
        };
        let command = ProjectSectionCommand::SetVariantPartParameter {
            variant,
            part: self.variant_part_link(project_id, &cell.designator),
            designator: cell.designator,
            parameter,
            value,
        };
        self.apply_variant_manager_edit(project_id, &command);
        if let Some(tab) = self.document_state.project_tabs.get_mut(&project_id) {
            tab.variant_manager.draft = None;
        }
    }

    /// Duplicate: add a copy of `from` (`None` is Base) under the first free
    /// "<name> copy", carrying the fits its sheets give it, then select the
    /// copy and open its name. Refused while the parts read does not hold,
    /// so the copy never misses a fit the sheets set.
    fn duplicate_variant(&mut self, project_id: ProjectId, from: Option<String>) {
        let Some(project) = self.document_state.project_by_id(project_id) else {
            return;
        };
        let tab = self.document_state.project_tabs.get(&project_id);
        let Some(sheet_fits) = tab.and_then(|tab| {
            sheet_fits_of(tab, project, self.document_state.sheet_edit_epoch, &from)
        }) else {
            return;
        };
        let source = from.clone().unwrap_or_else(|| BASE_VARIANT.to_string());
        let Some(name) = copy_name(&project.data, &source, &self.ui_state.i18n) else {
            return;
        };
        let command = ProjectSectionCommand::DuplicateVariant {
            from: source,
            name: name.clone(),
            sheet_fits,
        };
        if self.apply_variant_manager_edit(project_id, &command)
            && let Some(tab) = self.document_state.project_tabs.get_mut(&project_id)
        {
            tab.variant_selected = Some(name.clone());
            tab.variant_rename = Some(VariantRenameDraft {
                from: name.clone(),
                text: name,
            });
        }
    }
}

/// The fit `from`'s sheets give each part where it differs from Base, from
/// the parts read; `None` while no read holds for the project as it is.
fn sheet_fits_of(
    tab: &ProjectTabState,
    project: &crate::app::state::LoadedProject,
    sheet_epoch: u64,
    from: &Option<String>,
) -> Option<Vec<SheetFit>> {
    let matrix = tab.variant_parts.as_deref()?.as_ref().ok()?;
    if !super::read_holds(tab, matrix, project, sheet_epoch) {
        return None;
    }
    let Some(column) = matrix.column(from.as_deref()) else {
        return Some(Vec::new());
    };
    Some(
        matrix
            .rows
            .iter()
            .filter(|row| {
                row.fitted
                    .get(column)
                    .is_some_and(|&f| f != row.base_fitted())
            })
            .map(|row| SheetFit {
                designator: row.designator.clone(),
                part: row.link.clone(),
                fitted: !row.base_fitted(),
            })
            .collect(),
    )
}

/// The first free "<from> copy", then "<from> copy 2", ... in the language
/// of `i18n`; `None` when no candidate is a name the project may take.
pub(super) fn copy_name(data: &ProjectData, from: &str, i18n: &I18n) -> Option<String> {
    let from = if is_base_variant(from) {
        i18n.message_plain(ids::PROJECT_VARIANTS_BASE, &[])
    } else {
        from.to_string()
    };
    let first = i18n.message_plain(
        ids::VARIANT_MANAGER_COPY_NAME,
        &[("name", from.as_str().into())],
    );
    std::iter::once(first)
        .chain((2..=data.variant_definitions.len() + 2).map(|n| {
            i18n.message_plain(
                ids::VARIANT_MANAGER_COPY_NAME_N,
                &[("name", from.as_str().into()), ("n", n.into())],
            )
        }))
        .find(|name| data.check_variant_name(name, None).is_ok())
}
