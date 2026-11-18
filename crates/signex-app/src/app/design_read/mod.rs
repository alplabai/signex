//! The project design read shared by Update PCB and Back Annotate (#663): the
//! assembled sheet graph with its placed sheet occurrences, and footprint
//! resolution against the open libraries. Both run it on a worker; nothing
//! here touches the Model.

mod resolver;

pub(crate) use resolver::{LibraryRows, Resolver};

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use signex_types::schematic::{InstancePath, SchematicSheet};

use crate::app::project_sheets::{
    AssembledGraph, ProjectSheetSet, project_graph_with, project_roots, sheet_key_with,
};

/// The project's sheets as one hierarchy: every sheet occurrence placed from
/// the root, and the placements that could not be read.
pub(crate) struct DesignGraph {
    pub(crate) graph: AssembledGraph,
    roots: Vec<signex_net::ProjectRoot>,
    pub(crate) occurrences: Vec<signex_net::SheetOccurrence>,
    /// The placements nothing linked under may be removed from.
    pub(crate) unreadable: Vec<InstancePath>,
}

impl DesignGraph {
    /// Stitch `set` (assembled for the project in `project_dir`). `Err` says
    /// why there is no design.
    pub(crate) fn read(set: &ProjectSheetSet, project_dir: &Path) -> Result<Self, String> {
        let root = set
            .root
            .clone()
            .ok_or_else(|| "the project has no root sheet".to_string())?;
        let base = Some(project_dir);
        let graph = project_graph_with(&set.sheets, base, Some(&root), set.fold);
        let root_key = sheet_key_with(&root, base, set.fold);
        if !graph.sheets.contains_key(&root_key) {
            return Err(format!(
                "the root sheet {} could not be read",
                root.display()
            ));
        }
        let roots = project_roots(root_key, set, &graph, base, set.fold);
        let occurrences = signex_net::project_occurrences(&signex_net::ProjectGraph {
            sheets: &graph.sheets,
            resolved: &graph.resolved,
            roots: &roots,
        });
        let unreadable = unreadable_placements(set, &graph, &occurrences);
        Ok(Self {
            graph,
            roots,
            occurrences,
            unreadable,
        })
    }

    /// The hierarchy as the netlist stitcher takes it.
    pub(crate) fn project(&self) -> signex_net::ProjectGraph<'_> {
        signex_net::ProjectGraph {
            sheets: &self.graph.sheets,
            resolved: &self.graph.resolved,
            roots: &self.roots,
        }
    }

    /// Every placed sheet occurrence with its sheet, in walk order.
    pub(crate) fn sheets(&self) -> Vec<(InstancePath, &SchematicSheet)> {
        self.occurrences
            .iter()
            .filter_map(|o| Some((o.instance_path.clone(), self.graph.sheets.get(&o.key)?)))
            .collect()
    }

    /// Every placed sheet occurrence with its sheet and the file it was read
    /// from, in walk order.
    pub(crate) fn placed(&self) -> Vec<(InstancePath, &SchematicSheet, &PathBuf)> {
        self.occurrences
            .iter()
            .filter_map(|o| {
                Some((
                    o.instance_path.clone(),
                    self.graph.sheets.get(&o.key)?,
                    self.graph.paths.get(&o.key)?,
                ))
            })
            .collect()
    }

    /// Every sheet the read used, with its open engine's revision (`None`:
    /// read from disk).
    pub(crate) fn sheet_revisions(
        &self,
        open: &HashMap<PathBuf, u64>,
    ) -> Vec<(PathBuf, Option<u64>)> {
        self.graph
            .paths
            .values()
            .map(|path| (path.clone(), open.get(path).copied()))
            .collect()
    }
}

/// The placements nothing linked under may be removed from: every child
/// sheet whose file did not assemble, and the root placement when a page
/// with no parent failed (its parts sit at the root).
fn unreadable_placements(
    set: &ProjectSheetSet,
    graph: &AssembledGraph,
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
