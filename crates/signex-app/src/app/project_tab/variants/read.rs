//! The Variants section's parts read (#1293): every sheet of the project
//! (open ones from memory with their unsaved edits, closed ones from disk),
//! each child once per placement as the BOM counts it, turned into the parts
//! by variants matrix by `signex_output::variant_matrix`. Off the UI thread;
//! the result lands as `ProjectTabMsg::VariantPartsRead`.

use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;
use signex_output::{ExportContext, ProjectMetadata, VariantMatrix};

use super::super::general::tab_message;
use super::super::{ProjectSection, ProjectTabMsg};
use crate::app::handlers::menu::export::placements::netlist_placements;
use crate::app::project_sheets::{
    OpenSheets, assemble_project_sheets_quiet_from, open_sheets_snapshot, project_graph_quiet,
    project_pages, project_root_sheet_path, project_roots, sheet_key_with,
};
use crate::app::state::ProjectId;
use crate::app::state::scope::CaseFold;
use crate::app::{Message, Signex};

/// What the read needs, captured from memory.
struct PartsRead {
    open: OpenSheets,
    pages: Vec<PathBuf>,
    root: Option<PathBuf>,
    dir: PathBuf,
    fold: CaseFold,
    variants: Vec<String>,
}

impl Signex {
    /// Read the project's parts for the Variants section. Without `force`,
    /// only while the section is shown and when the last read no longer
    /// holds (another sheet edit count or other variant names). A closed
    /// project reads nothing.
    pub(in crate::app::project_tab) fn read_variant_parts(
        &mut self,
        project_id: ProjectId,
        force: bool,
    ) -> Task<Message> {
        let epoch = self.document_state.sheet_edit_epoch;
        let Some(project) = self.document_state.project_by_id(project_id) else {
            return Task::none();
        };
        let variants = project.data.variant_definitions.clone();
        let current = self
            .document_state
            .project_tabs
            .get(&project_id)
            .is_none_or(|tab| {
                tab.section != ProjectSection::Variants
                    || (tab.variant_parts_epoch == epoch
                        && tab.variant_parts.as_deref().is_some_and(|parts| {
                            parts
                                .as_ref()
                                .is_ok_and(|matrix| super::read_for(matrix, &variants))
                        }))
            });
        if current && !force {
            return Task::none();
        }
        let read = PartsRead {
            open: open_sheets_snapshot(&self.document_state),
            pages: project_pages(Some(project)),
            root: project_root_sheet_path(project),
            dir: project.dir().to_path_buf(),
            fold: crate::app::state::project_case::fold_of(Some(project)),
            variants,
        };
        let Some(tab) = self.document_state.project_tabs.get_mut(&project_id) else {
            return Task::none();
        };
        tab.variant_parts_generation += 1;
        tab.variant_parts_reading = true;
        let generation = tab.variant_parts_generation;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || read_parts(read))
                    .await
                    .map_err(|error| format!("the read did not finish: {error}"))
                    .and_then(|parts| parts)
            },
            move |parts| {
                tab_message(ProjectTabMsg::VariantPartsRead {
                    project_id,
                    generation,
                    epoch,
                    parts: Arc::new(parts),
                })
            },
        )
    }

    /// A parts read landed: keep it when it is the newest one started. A
    /// read for a project closed since is dropped.
    pub(in crate::app::project_tab) fn install_variant_parts(
        &mut self,
        project_id: ProjectId,
        generation: u64,
        epoch: u64,
        parts: Arc<Result<VariantMatrix, String>>,
    ) {
        let Some(tab) = self.document_state.project_tabs.get_mut(&project_id) else {
            return;
        };
        if tab.variant_parts_generation != generation {
            return;
        }
        tab.variant_parts = Some(parts);
        tab.variant_parts_epoch = epoch;
        tab.variant_parts_reading = false;
    }
}

/// The matrix of the project's parts. Blocking: reads closed sheets.
/// Refused when the project has no root sheet or it could not be read.
fn read_parts(read: PartsRead) -> Result<VariantMatrix, String> {
    let Some(root) = read.root else {
        return Err("the project has no root sheet".to_string());
    };
    let set = assemble_project_sheets_quiet_from(&read.open, &read.pages, &root, read.fold);
    let dir = Some(read.dir.as_path());
    let graph = project_graph_quiet(&set.sheets, dir, Some(root.as_path()), set.fold);
    let root_key = sheet_key_with(&root, dir, set.fold);
    if !graph.sheets.contains_key(&root_key) {
        return Err(format!(
            "the root sheet {} could not be read",
            root.display()
        ));
    }
    let roots = project_roots(root_key, &set, &graph, dir, set.fold);
    let walk = signex_net::ProjectGraph {
        sheets: &graph.sheets,
        resolved: &graph.resolved,
        roots: &roots,
    };
    let occurrences = signex_net::project_occurrences(&walk);
    let mut keyed: Vec<_> = graph.sheets.into_iter().collect();
    keyed.sort_by(|a, b| a.0.cmp(&b.0));
    let keys: Vec<_> = keyed.iter().map(|(key, _)| key.clone()).collect();
    let ctx = ExportContext {
        sheets: Vec::new(),
        metadata: ProjectMetadata::default(),
        netlist: None,
        netlist_placements: netlist_placements(&keys, occurrences),
        netlist_sheets: keyed.into_iter().map(|(_, sheet)| sheet).collect(),
        current_sheet: None,
    };
    Ok(signex_output::variant_matrix(&ctx, &read.variants))
}
