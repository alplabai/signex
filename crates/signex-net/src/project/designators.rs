//! Which symbols take their designator from a per-placement entry (#829).

use signex_types::schematic::Symbol;

use super::{ProjectGraph, walk_occurrences};

/// The designators symbols resolve through a `Symbol.instances` entry whose
/// path matches a real placement in the walk and that differs from the
/// symbol's own `reference`, one per symbol per placement, in
/// walk then document order. Pure: reads only the graph.
///
/// An entry whose path matches no placement (stale, imported, malformed) is
/// not honoured by the resolver and so is not listed.
pub fn instance_entry_designators(graph: &ProjectGraph) -> Vec<String> {
    let mut issues = Vec::new();
    let (occs, _edges) = walk_occurrences(graph, &mut issues);
    occs.iter()
        .flat_map(|occ| {
            occ.sheet
                .symbols
                .iter()
                .filter(|sym: &&Symbol| sym.retargets_designator_in(&occ.path))
                .map(|sym| sym.designator_in(&occ.path).to_string())
        })
        .collect()
}
