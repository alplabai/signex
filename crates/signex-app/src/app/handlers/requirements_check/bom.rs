//! The project's BOM for `parts_state` and `bom_cost` (#2339), built off the
//! UI thread as the BOM export builds it: every sheet of the project (open
//! ones from memory, closed ones from disk), each child once per placement,
//! the library's current part names, the active variant, and the parts whose
//! board footprint the BOM leaves out (#2365). Unpriced: the requirement
//! prices it at its own build quantity.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use signex_output::bom::{BomGrouping, BomOptions, BomTable, rollup_with_validation};
use signex_output::{ExportContext, ProjectMetadata};
use signex_types::pcb::PcbBoard;

use crate::app::Signex;
use crate::app::handlers::menu::export::placements::{
    netlist_placements, stamp_channel_designators,
};
use crate::app::project_sheets::{
    OpenSheets, assemble_project_sheets_guarded_from, open_sheets_snapshot, project_graph_quiet,
    project_pages, project_root_sheet_path, project_roots, sheet_key_with, stitch_issue_message,
};
use crate::app::saved_channel_naming::SavedChannelNaming;
use crate::app::state::LoadedProject;
use crate::app::state::scope::CaseFold;

/// What building the BOM needs, captured from memory. Also the BOM of a
/// generated design report (`app::notebook_generate`).
pub(crate) struct BomInputs {
    open: OpenSheets,
    pages: Vec<PathBuf>,
    root: Option<PathBuf>,
    dir: PathBuf,
    fold: CaseFold,
    /// The library's current part names (`library::drift`).
    names: HashMap<String, String>,
    variant: Option<String>,
    /// The project's saved channel naming.
    naming: SavedChannelNaming,
}

/// The BOM grouped as it is bought, one row per part, and what could not be
/// read on the way.
pub(crate) struct Boms {
    pub(crate) grouped: BomTable,
    pub(crate) parts: BomTable,
    pub(crate) warnings: Vec<String>,
}

impl BomInputs {
    /// The same inputs for `variant` (`None`: Base).
    pub(crate) fn for_variant(self, variant: Option<String>) -> Self {
        let variant = variant.filter(|v| !signex_types::project::is_base_variant(v));
        Self { variant, ..self }
    }
}

impl Signex {
    pub(crate) fn requirements_bom_inputs(&self, project: &LoadedProject) -> BomInputs {
        let variant = project
            .data
            .active_variant
            .clone()
            .filter(|v| !signex_types::project::is_base_variant(v));
        BomInputs {
            open: open_sheets_snapshot(&self.document_state),
            pages: project_pages(Some(project)),
            root: project_root_sheet_path(project),
            dir: project.dir().to_path_buf(),
            fold: crate::app::state::project_case::fold_of(Some(project)),
            names: self.library.drift.names.clone(),
            variant,
            naming: SavedChannelNaming::of(&project.data),
        }
    }
}

/// Build the BOM. Blocking: reads closed sheets, within `confine` when
/// given. Refused when the project has no root sheet or it could not be read.
pub(crate) fn project_boms(
    inputs: BomInputs,
    board: Option<&PcbBoard>,
    confine: Option<&Path>,
) -> Result<Boms, String> {
    let Some(root) = inputs.root else {
        return Err("the project has no root sheet".to_string());
    };
    let guard = |path: &Path| {
        confine.map_or(Ok(()), |dir| {
            crate::app::document_ids::confined_read(dir, path)
                .map_err(|why| format!("is not read: {why}"))
        })
    };
    let set = assemble_project_sheets_guarded_from(
        &inputs.open,
        &inputs.pages,
        &root,
        inputs.fold,
        &guard,
    );
    let mut warnings: Vec<String> = set
        .unreadable
        .iter()
        .map(|(path, why)| format!("BOM: sheet {} {why}", path.display()))
        .collect();
    let dir = Some(inputs.dir.as_path());
    let graph = project_graph_quiet(&set.sheets, dir, Some(root.as_path()), set.fold);
    warnings.extend(
        graph
            .issues
            .iter()
            .map(|issue| format!("BOM: {}", stitch_issue_message(issue))),
    );
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
    let naming = inputs
        .naming
        .for_design(&walk)
        .map_err(|refused| refused.to_string())?;
    let occurrences = signex_net::project_occurrences(&walk);
    let channels = signex_net::ChannelDesignators::of_project(&walk, naming);
    let mut keyed: Vec<_> = graph.sheets.into_iter().collect();
    keyed.sort_by(|a, b| a.0.cmp(&b.0));
    let keys: Vec<_> = keyed.iter().map(|(key, _)| key.clone()).collect();
    let placements = netlist_placements(&keys, occurrences);
    let mut netlist_sheets: Vec<_> = keyed.into_iter().map(|(_, sheet)| sheet).collect();
    stamp_channel_designators(&mut netlist_sheets, &placements, &channels);
    let mut ctx = ExportContext {
        sheets: Vec::new(),
        metadata: ProjectMetadata::default(),
        netlist: None,
        netlist_placements: placements,
        netlist_sheets,
        current_sheet: None,
    };
    crate::library::drift::apply_library_names(&mut ctx, &inputs.names);
    let left_out = board
        .map(|board| {
            board
                .footprints
                .iter()
                .filter(|fp| !fp.component.component_type.in_bom())
                .filter_map(|fp| fp.schematic_link.clone())
                .collect()
        })
        .unwrap_or_default();
    let options = BomOptions {
        active_variant: inputs.variant,
        board_left_out: left_out,
        ..BomOptions::default()
    };
    let (grouped, _) = rollup_with_validation(&ctx, &options);
    let ungrouped = BomOptions {
        grouping: BomGrouping::Ungrouped,
        ..options
    };
    let (parts, _) = rollup_with_validation(&ctx, &ungrouped);
    Ok(Boms {
        grouped,
        parts,
        warnings,
    })
}
