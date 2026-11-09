//! The worker half of Update PCB: assemble the project's sheets, stitch the
//! netlist, resolve each part's footprint from its library row and diff the
//! board. Everything it reads is owned, so it runs on a blocking thread.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use signex_engine::pcb::eco::{
    EcoChangeList, EcoInput, EcoPart, Staging, design_eco_parts, diff_board,
};
use signex_library::ComponentRow;
use signex_library::LibraryAdapter as _;
use signex_library::adapters::local_git::LocalGitAdapter;
use signex_library::primitive::PrimitiveRef;
use signex_types::net::Netlist;
use signex_types::part::{FootprintResolveError, ResolvedFootprint};
use signex_types::pcb::PcbBoard;
use signex_types::schematic::{InstancePath, SchematicSheet, Symbol};
use uuid::Uuid;

use super::Computed;
use crate::app::project_sheets::{
    ProjectSheetPlan, ProjectSheetSet, project_graph_with, project_roots, sheet_key_with,
};

/// The design and board one change list was computed from. Kept for the net
/// refresh after a tick and for Refresh, which keeps the ticks.
#[derive(Debug)]
pub(crate) struct DesignSnapshot {
    pub board: PcbBoard,
    pub revision: u64,
    pub parts: Vec<EcoPart>,
    pub netlist: Netlist,
    pub unreadable: Vec<InstancePath>,
    /// Revision of each open sheet of the design when it was read; a sheet
    /// edited since makes the list stale.
    pub sheet_revisions: Vec<(PathBuf, u64)>,
}

impl DesignSnapshot {
    pub(crate) fn input(&self) -> EcoInput<'_> {
        EcoInput {
            board: &self.board,
            revision: self.revision,
            parts: &self.parts,
            netlist: &self.netlist,
            unreadable: &self.unreadable,
            staging: Staging::DEFAULT,
        }
    }
}

/// One open library: its id, `.snxlib` path and rows.
#[derive(Debug, Clone)]
pub(crate) struct LibraryRows {
    pub id: Uuid,
    pub root: PathBuf,
    pub rows: Vec<ComponentRow>,
}

/// Everything the worker reads, taken on the update thread.
pub(crate) struct DesignRequest {
    pub plan: ProjectSheetPlan,
    pub project_dir: PathBuf,
    pub board: PcbBoard,
    pub revision: u64,
    pub libraries: Vec<LibraryRows>,
    /// Revision of every open sheet engine, by path.
    pub open_revisions: Vec<(PathBuf, u64)>,
    /// The list before a Refresh, whose ticks the new one keeps.
    pub previous: Option<EcoChangeList>,
}

/// Read the design and diff the board. `Err` says why there is no list.
pub(crate) fn compute(request: DesignRequest) -> Result<Arc<Computed>, String> {
    let set = request.plan.assemble();
    let root = set
        .root
        .clone()
        .ok_or_else(|| "the project has no root sheet".to_string())?;
    let base = Some(request.project_dir.as_path());
    let graph = project_graph_with(&set.sheets, base, Some(&root), set.fold);
    let root_key = sheet_key_with(&root, base, set.fold);
    if !graph.sheets.contains_key(&root_key) {
        return Err(format!(
            "the root sheet {} could not be read",
            root.display()
        ));
    }
    let roots = project_roots(root_key, &set, &graph, base, set.fold);
    let project = signex_net::ProjectGraph {
        sheets: &graph.sheets,
        resolved: &graph.resolved,
        roots: &roots,
    };
    let netlist = signex_net::build_project_netlist(&project).netlist;
    let occurrences = signex_net::project_occurrences(&project);
    let sheets: Vec<(InstancePath, &SchematicSheet)> = occurrences
        .iter()
        .filter_map(|o| Some((o.instance_path.clone(), graph.sheets.get(&o.key)?)))
        .collect();
    let unreadable = unreadable_placements(&set, &graph, &occurrences);
    let resolver = Resolver::open(&request.libraries);
    let parts = design_eco_parts(&sheets, |_, symbol| resolver.resolve(symbol));
    let read_paths: Vec<&PathBuf> = graph.paths.values().collect();
    let sheet_revisions = request
        .open_revisions
        .into_iter()
        .filter(|(path, _)| read_paths.contains(&path))
        .collect();
    let snapshot = DesignSnapshot {
        board: request.board,
        revision: request.revision,
        parts,
        netlist,
        unreadable,
        sheet_revisions,
    };
    let mut list = diff_board(&snapshot.input());
    if let Some(previous) = &request.previous {
        list.keep_choices_from(previous, &snapshot.input());
    }
    Ok(Arc::new(Computed {
        snapshot: Arc::new(snapshot),
        list,
    }))
}

/// The placements nothing linked under may be removed from: every child
/// sheet whose file did not assemble, and the root placement when a page
/// with no parent failed (its parts sit at the root).
fn unreadable_placements(
    set: &ProjectSheetSet,
    graph: &crate::app::project_sheets::AssembledGraph,
    occurrences: &[signex_net::SheetOccurrence],
) -> Vec<InstancePath> {
    let mut out = Vec::new();
    for occurrence in occurrences {
        let Some(sheet) = graph.sheets.get(&occurrence.key) else {
            continue;
        };
        let resolved = graph.resolved.get(&occurrence.key);
        for child in &sheet.child_sheets {
            let assembled = resolved
                .and_then(|r| r.get(&child.filename))
                .is_some_and(|key| graph.sheets.contains_key(key));
            if !assembled {
                out.push(occurrence.instance_path.child(child.uuid));
            }
        }
    }
    let failed = set.unreadable.len() + set.absent.len();
    if failed > out.len() && !out.contains(&InstancePath::root()) {
        out.push(InstancePath::root());
    }
    out
}

/// Footprint resolution against the open libraries, with one adapter per
/// library opened once.
struct Resolver<'a> {
    libraries: &'a [LibraryRows],
    adapters: HashMap<Uuid, LocalGitAdapter>,
}

impl<'a> Resolver<'a> {
    fn open(libraries: &'a [LibraryRows]) -> Self {
        let adapters = libraries
            .iter()
            .filter_map(|lib| {
                LocalGitAdapter::open(&lib.root)
                    .ok()
                    .map(|adapter| (lib.id, adapter))
            })
            .collect();
        Self {
            libraries,
            adapters,
        }
    }

    /// The board footprint of the part whose first unit is `symbol`: its
    /// library row, the alternate it was built with, the library footprint.
    fn resolve(&self, symbol: &Symbol) -> Result<ResolvedFootprint, FootprintResolveError> {
        let (Some(library), Some(row_id)) = (symbol.library_id, symbol.row_id) else {
            return Err(FootprintResolveError::NoFootprint);
        };
        let row = self
            .libraries
            .iter()
            .filter(|lib| lib.id == library)
            .flat_map(|lib| &lib.rows)
            .find(|row| row.row_id == row_id)
            .ok_or(FootprintResolveError::RowNotFound {
                library,
                row: row_id,
            })?;
        let chosen = chosen_binding(symbol);
        signex_library::part_footprint::resolve_part_footprint(
            row,
            chosen.as_ref(),
            |address, _version| {
                self.adapters
                    .get(&address.library_id)?
                    .get_footprint(address.uuid)
                    .ok()
            },
        )
    }
}

/// The alternate footprint a part was built with, from its valid stamp;
/// `None` for the row's default.
fn chosen_binding(symbol: &Symbol) -> Option<PrimitiveRef> {
    let key = symbol.valid_stamp()?.footprint_ref.as_deref()?;
    let (library, footprint) = crate::library::drift::split_lib_id(key)?;
    Some(PrimitiveRef::new(library, footprint))
}
