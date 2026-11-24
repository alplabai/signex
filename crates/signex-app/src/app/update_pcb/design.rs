//! The worker half of Update PCB: stitch the project's design and its
//! netlist (`app/design_read`), resolve each part's footprint and diff the
//! board. Everything it reads is owned, so it runs on a blocking thread.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use signex_engine::pcb::eco::{
    EcoChangeList, EcoInput, EcoPart, Staging, design_eco_parts_named, diff_board,
};
use signex_types::net::Netlist;
use signex_types::pcb::PcbBoard;
use signex_types::schematic::InstancePath;

use super::Computed;
use crate::app::design_read::{DesignGraph, LibraryRows, Resolver};
use crate::app::project_sheets::ProjectSheetPlan;
use crate::app::saved_channel_naming::SavedChannelNaming;

/// The design and board one change list was computed from. Kept for the net
/// refresh after a tick and for Refresh, which keeps the ticks.
#[derive(Debug)]
pub(crate) struct DesignSnapshot {
    pub board: PcbBoard,
    pub revision: u64,
    pub parts: Vec<EcoPart>,
    pub netlist: Netlist,
    pub unreadable: Vec<InstancePath>,
    /// Every sheet the read used, with its open engine's revision then
    /// (`None`: read from disk). A different engine state now makes the
    /// list stale.
    pub sheet_revisions: Vec<(PathBuf, Option<u64>)>,
    /// Libraries whose adapter did not open, with why: their parts resolve
    /// to no footprint, and the update thread logs each to Messages.
    pub library_errors: Vec<(PathBuf, String)>,
    /// The rules the schematic's Net Parameters directives ask for (#1489).
    pub schematic_rules: Vec<super::rules_from_schematic::SchematicRule>,
    /// The rules they ask for that are left out, with why; logged to
    /// Messages like `library_errors`.
    pub rule_problems: Vec<super::rules_from_schematic::RuleProblem>,
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
    /// The project's saved channel naming.
    pub naming: SavedChannelNaming,
}

/// Read the design and diff the board. `Err` says why there is no list.
pub(crate) fn compute(request: DesignRequest) -> Result<Arc<Computed>, String> {
    let set = request.plan.assemble();
    let design = DesignGraph::read(&set, &request.project_dir)?;
    let project = design.project();
    let naming = request
        .naming
        .for_design(&project)
        .map_err(|refused| refused.to_string())?;
    let netlist = signex_net::build_project_netlist_named(&project, naming).netlist;
    let channels = signex_net::ChannelDesignators::of_project(&project, naming);
    let sheets = design.sheets();
    let resolver = Resolver::open(&request.libraries);
    let parts = design_eco_parts_named(&sheets, &channels, |_, symbol| resolver.resolve(symbol));
    let open: HashMap<PathBuf, u64> = request.open_revisions.into_iter().collect();
    let net_names: std::collections::HashSet<&str> =
        netlist.nets.iter().map(|n| n.name.as_str()).collect();
    let (schematic_rules, rule_problems) = super::rules_from_schematic::schematic_rules(
        sheets.iter().map(|(path, sheet)| (path, *sheet)),
        &net_names,
    );
    drop(net_names);
    let snapshot = DesignSnapshot {
        board: request.board,
        revision: request.revision,
        parts,
        netlist,
        unreadable: design.unreadable.clone(),
        sheet_revisions: design.sheet_revisions(&open),
        library_errors: resolver.errors,
        schematic_rules,
        rule_problems,
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
