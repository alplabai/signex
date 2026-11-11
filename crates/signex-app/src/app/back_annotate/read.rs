//! The worker half of Back Annotate: assemble the project's sheets, find
//! each part's sheet file, resolve its footprint and diff the board against
//! the design. Everything it reads is owned, so it runs on a blocking thread.
//! The assembly is Update PCB's (`update_pcb/design.rs`).

use std::collections::HashMap;
use std::path::PathBuf;

use signex_engine::pcb::eco::{
    BackChangeList, BackInput, BackPart, EcoPart, design_eco_parts, diff_schematic,
};
use signex_types::pcb::PcbBoard;
use signex_types::schematic::{InstancePath, SchematicSheet};
use uuid::Uuid;

use crate::app::project_sheets::{
    DiskStamps, ProjectSheetPlan, project_graph_with, project_roots, sheet_key_with,
};
use crate::app::update_pcb::design::{LibraryRows, Resolver, unreadable_placements};

/// Where one part lives: the sheet file its placement reads, and whether
/// that file is placed only once in the project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PartFile {
    pub file: PathBuf,
    pub single: bool,
}

/// What one read produced.
#[derive(Debug)]
pub struct BackRead {
    pub(crate) list: BackChangeList,
    /// The file of each part, by placement and units.
    pub(crate) files: HashMap<(InstancePath, Vec<Uuid>), PartFile>,
    /// Every sheet the read used, with its open engine's revision then
    /// (`None`: read from disk). A different engine state now makes the
    /// list stale.
    pub(crate) sheet_revisions: Vec<(PathBuf, Option<u64>)>,
    /// The sheets no engine held, as read, for Execute to load.
    pub(crate) loaded: HashMap<PathBuf, SchematicSheet>,
    /// The stamp of each `loaded` file, taken just before it was read.
    pub(crate) stamps: DiskStamps,
    /// Libraries whose adapter did not open, with why.
    pub(crate) library_errors: Vec<(PathBuf, String)>,
}

/// Everything the worker reads, taken on the update thread.
pub(crate) struct BackRequest {
    pub plan: ProjectSheetPlan,
    pub project_dir: PathBuf,
    pub board: PcbBoard,
    pub revision: u64,
    pub libraries: Vec<LibraryRows>,
    /// Revision of every open sheet engine, by path.
    pub open_revisions: Vec<(PathBuf, u64)>,
    /// The list before a Refresh, whose ticks the new one keeps.
    pub previous: Option<BackChangeList>,
}

/// Read the design and diff it against the board. `Err` says why there is
/// no list.
pub(crate) fn compute(request: BackRequest) -> Result<BackRead, String> {
    let (set, mut stamps) = request.plan.assemble_stamped();
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
    let occurrences = signex_net::project_occurrences(&project);
    let placed: Vec<(InstancePath, &SchematicSheet, &PathBuf)> = occurrences
        .iter()
        .filter_map(|o| {
            Some((
                o.instance_path.clone(),
                graph.sheets.get(&o.key)?,
                graph.paths.get(&o.key)?,
            ))
        })
        .collect();
    let mut placements: HashMap<&PathBuf, usize> = HashMap::new();
    for (_, _, file) in &placed {
        *placements.entry(*file).or_default() += 1;
    }
    let sheets: Vec<(InstancePath, &SchematicSheet)> = placed
        .iter()
        .map(|(path, sheet, _)| (path.clone(), *sheet))
        .collect();
    let unreadable = unreadable_placements(&set, &graph, &occurrences);
    let resolver = Resolver::open(&request.libraries);
    let eco_parts = design_eco_parts(&sheets, |_, symbol| resolver.resolve(symbol));
    let mut files = HashMap::new();
    let parts: Vec<BackPart> = eco_parts
        .into_iter()
        .map(|eco| {
            let file = part_file(&placed, &eco);
            let shared_sheet = file.is_some_and(|f| placements.get(f).copied().unwrap_or(0) > 1);
            if let Some(file) = file {
                files.insert(
                    (eco.part.sheet.clone(), eco.part.units.clone()),
                    PartFile {
                        file: file.clone(),
                        single: !shared_sheet,
                    },
                );
            }
            BackPart { eco, shared_sheet }
        })
        .collect();
    let mut list = diff_schematic(&BackInput {
        board: &request.board,
        revision: request.revision,
        parts: &parts,
        unreadable: &unreadable,
    });
    if let Some(previous) = &request.previous {
        list.keep_choices_from(previous);
    }
    let open: HashMap<PathBuf, u64> = request.open_revisions.into_iter().collect();
    let sheet_revisions = graph
        .paths
        .values()
        .map(|path| (path.clone(), open.get(path).copied()))
        .collect();
    let loaded: HashMap<PathBuf, SchematicSheet> = files
        .values()
        .map(|f: &PartFile| &f.file)
        .filter(|file| !open.contains_key(*file))
        .filter_map(|file| Some((file.clone(), set.sheets.get(file)?.clone())))
        .collect();
    let stamps = loaded
        .keys()
        .map(|path| (path.clone(), stamps.remove(path).flatten()))
        .collect();
    Ok(BackRead {
        list,
        files,
        sheet_revisions,
        loaded,
        stamps,
        library_errors: resolver.errors,
    })
}

/// The file of the placement that holds `part`'s first unit.
fn part_file<'a>(
    placed: &'a [(InstancePath, &SchematicSheet, &PathBuf)],
    part: &EcoPart,
) -> Option<&'a PathBuf> {
    let first = part.part.units.first()?;
    placed
        .iter()
        .find(|(path, sheet, _)| {
            *path == part.part.sheet && sheet.symbols.iter().any(|s| s.uuid == *first)
        })
        .map(|(_, _, file)| *file)
}

/// One read is equal only to itself: messages compare reads by identity,
/// never by the sheets they hold.
impl PartialEq for BackRead {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}
