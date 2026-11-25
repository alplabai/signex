//! The Variants section's parts read (#1293): every sheet of the project
//! (open ones from memory with their unsaved edits, closed ones from disk),
//! each child once per placement as the BOM counts it, turned into the parts
//! by variants matrix by `signex_output::variant_matrix`. Off the UI thread;
//! the result lands as `ProjectTabMsg::VariantPartsRead`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;
use signex_output::{ExportContext, MatrixChannels, ProjectMetadata, VariantMatrix};

use super::super::ProjectTabMsg;
use super::super::general::tab_message;
use crate::app::core::parts_read_error::PartsReadError;
use crate::app::handlers::menu::export::export_metadata_with;
use crate::app::handlers::menu::export::placements::{
    netlist_placements, stamp_channel_designators,
};
use crate::app::project_sheets::{
    OpenSheets, assemble_project_sheets_quiet_from, open_sheets_snapshot, project_graph_quiet,
    project_pages, project_root_sheet_path, project_roots, sheet_key_with,
};
use crate::app::saved_channel_naming::SavedChannelNaming;
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
    /// The project's parameters and the document facts (clock, file times),
    /// so a value evaluates on its sheet as the BOM evaluates it (#1986).
    metadata: ProjectMetadata,
    /// The project's saved channel naming.
    naming: SavedChannelNaming,
}

impl Signex {
    /// Read the project's parts for the Variant Manager. Without `force`,
    /// only while its tab is open and when the last read no longer holds
    /// (another sheet edit count, other variant names or other project
    /// parameters). A closed project reads nothing.
    pub(in crate::app::project_tab) fn read_variant_parts(
        &mut self,
        project_id: ProjectId,
        force: bool,
    ) -> Task<Message> {
        let epoch = self.document_state.sheet_edit_epoch;
        let shown = self.variant_manager_is_open(project_id);
        let Some(project) = self.document_state.project_by_id(project_id) else {
            return Task::none();
        };
        let current = self
            .document_state
            .project_tabs
            .get(&project_id)
            .is_none_or(|tab| {
                !shown
                    || tab.variant_parts.as_deref().is_some_and(|parts| {
                        parts
                            .as_ref()
                            .is_ok_and(|matrix| super::read_holds(tab, matrix, project, epoch))
                    })
            });
        if current && !force {
            return Task::none();
        }
        let mut metadata = export_metadata_with(Some(project), &HashMap::new());
        // The clock and file times the computed tokens print, as the BOM
        // export reads them.
        metadata.document_facts = self.document_state.times.facts();
        let parameters = project.data.parameters.clone();
        let read = PartsRead {
            open: open_sheets_snapshot(&self.document_state),
            pages: project_pages(Some(project)),
            root: project_root_sheet_path(project),
            dir: project.dir().to_path_buf(),
            fold: crate::app::state::project_case::fold_of(Some(project)),
            variants: project.data.variant_definitions.clone(),
            metadata,
            naming: SavedChannelNaming::of(&project.data),
        };
        let Some(tab) = self.document_state.project_tabs.get_mut(&project_id) else {
            return Task::none();
        };
        tab.variant_parts_parameters = Some(parameters);
        tab.variant_parts_generation += 1;
        tab.variant_parts_reading = true;
        let generation = tab.variant_parts_generation;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || read_parts(read))
                    .await
                    .map_err(|error| PartsReadError::Interrupted(error.to_string()))
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
        parts: Arc<Result<VariantMatrix, PartsReadError>>,
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
fn read_parts(read: PartsRead) -> Result<VariantMatrix, PartsReadError> {
    let Some(root) = read.root else {
        return Err(PartsReadError::NoRootSheet);
    };
    let set = assemble_project_sheets_quiet_from(&read.open, &read.pages, &root, read.fold);
    let dir = Some(read.dir.as_path());
    let graph = project_graph_quiet(&set.sheets, dir, Some(root.as_path()), set.fold);
    let root_key = sheet_key_with(&root, dir, set.fold);
    if !graph.sheets.contains_key(&root_key) {
        return Err(PartsReadError::RootUnreadable(root.clone()));
    }
    let roots = project_roots(root_key, &set, &graph, dir, set.fold);
    let walk = signex_net::ProjectGraph {
        sheets: &graph.sheets,
        resolved: &graph.resolved,
        roots: &roots,
    };
    let naming = read
        .naming
        .for_design(&walk)
        .map_err(|_| PartsReadError::ChannelNamingUnreadable)?;
    let occurrences = signex_net::project_occurrences(&walk);
    let channels = signex_net::ChannelDesignators::of_project(&walk, naming);
    let mut keyed: Vec<_> = graph.sheets.into_iter().collect();
    keyed.sort_by(|a, b| a.0.cmp(&b.0));
    let keys: Vec<_> = keyed.iter().map(|(key, _)| key.clone()).collect();
    let placements = netlist_placements(&keys, occurrences);
    let mut netlist_sheets: Vec<_> = keyed.into_iter().map(|(_, sheet)| sheet).collect();
    stamp_channel_designators(&mut netlist_sheets, &placements, &channels);
    let ctx = ExportContext {
        sheets: Vec::new(),
        metadata: read.metadata,
        netlist: None,
        netlist_placements: placements,
        netlist_sheets,
        current_sheet: None,
    };
    Ok(signex_output::variant_matrix_in_channels(
        &ctx,
        &read.variants,
        &matrix_channels(&channels),
    ))
}

/// The walk's channel placements as the matrix reads them. A channel inside
/// another is named by its channels, outermost first (`A1/CH2`).
fn matrix_channels(channels: &signex_net::ChannelDesignators) -> MatrixChannels {
    let mut out = MatrixChannels::default();
    for (path, logical, inside) in channels.channel_placements() {
        let name = inside
            .iter()
            .map(|channel| channel.name.as_str())
            .collect::<Vec<_>>()
            .join("/");
        out.insert(path.clone(), logical.clone(), name);
    }
    out
}
