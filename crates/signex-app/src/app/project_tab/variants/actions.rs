//! The Variant Manager's top bar actions (#2949; DECISIONS 2026-11-25,
//! Caner): Compare variants... shows two variants side by side (mockup row
//! 59, view d) in place of the cards and the matrix, and Open in BOM Studio
//! opens the project's BOM with the variant picked. The comparison is view
//! state only; neither action changes the project.

use std::path::PathBuf;

use iced::Task;
use signex_i18n::{ids, t};
use signex_types::project::is_base_variant;

use super::manager::CompareSides;
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};

impl Signex {
    /// Show the comparison: Base on the left, the selected variant on the
    /// right (the first definition while Base is selected). Nothing while
    /// the project defines no variant besides Base.
    pub(in crate::app::project_tab) fn open_variant_compare(&mut self, project_id: ProjectId) {
        let Some(project) = self.document_state.project_by_id(project_id) else {
            return;
        };
        let Some(tab) = self.document_state.project_tabs.get(&project_id) else {
            return;
        };
        let definitions = &project.data.variant_definitions;
        let Some(first) = definitions.first() else {
            return;
        };
        // The list's selection, which follows the active variant while none
        // is clicked.
        let selected = tab
            .variant_selected
            .as_deref()
            .or(project.data.active_variant.as_deref())
            .filter(|name| !is_base_variant(name))
            .and_then(|name| {
                definitions
                    .iter()
                    .find(|defined| defined.eq_ignore_ascii_case(name.trim()))
            });
        let to = selected.unwrap_or(first).clone();
        if let Some(tab) = self.document_state.project_tabs.get_mut(&project_id) {
            tab.variant_manager.compare = Some(CompareSides {
                from: None,
                to: Some(to),
            });
        }
    }

    /// Pick one side of the comparison; `None` is Base.
    pub(in crate::app::project_tab) fn pick_compare_side(
        &mut self,
        project_id: ProjectId,
        from: bool,
        name: Option<String>,
    ) {
        let name = name.filter(|name| !is_base_variant(name));
        if let Some(sides) = self.compare_sides_mut(project_id) {
            if from {
                sides.from = name;
            } else {
                sides.to = name;
            }
        }
    }

    /// Swap the two sides of the comparison.
    pub(in crate::app::project_tab) fn swap_compare_sides(&mut self, project_id: ProjectId) {
        if let Some(sides) = self.compare_sides_mut(project_id) {
            std::mem::swap(&mut sides.from, &mut sides.to);
        }
    }

    /// Close the comparison: the cards and the matrix again.
    pub(in crate::app::project_tab) fn close_variant_compare(&mut self, project_id: ProjectId) {
        if let Some(tab) = self.document_state.project_tabs.get_mut(&project_id) {
            tab.variant_manager.compare = None;
        }
    }

    fn compare_sides_mut(&mut self, project_id: ProjectId) -> Option<&mut CompareSides> {
        let tab = self.document_state.project_tabs.get_mut(&project_id)?;
        tab.variant_manager.compare.as_mut()
    }

    /// Open in BOM Studio: the BOM of the project's sheets with `variant`
    /// picked. With no sheet of the project open, Messages says so and
    /// nothing opens.
    pub(in crate::app::project_tab) fn open_variant_bom(
        &mut self,
        project_id: ProjectId,
        variant: Option<String>,
    ) -> Task<Message> {
        match self.project_bom_source(project_id) {
            Some(source) => self.open_bom_preview_in(source, variant),
            None => {
                let words = t!(self.ui_state.i18n, ids::VARIANT_MANAGER_BOM_NO_SHEET);
                crate::diagnostics::log_warning(words.into_string());
                Task::none()
            }
        }
    }

    /// The open sheet the BOM of `project_id` is built from: its root sheet
    /// when it is open, else its first open sheet by path. `None` while no
    /// sheet of the project is open.
    pub(crate) fn project_bom_source(&self, project_id: ProjectId) -> Option<PathBuf> {
        let project = self.document_state.project_by_id(project_id)?;
        let root = crate::app::project_sheets::project_root_sheet_path(project)
            .filter(|root| self.document_state.engines.contains_key(root));
        if root.is_some() {
            return root;
        }
        let mut open: Vec<&PathBuf> = self
            .document_state
            .engines
            .keys()
            .filter(|path| {
                self.document_state
                    .document_project(path)
                    .is_some_and(|owner| owner.id == project_id)
            })
            .collect();
        open.sort();
        open.first().map(|path| (*path).clone())
    }
}
