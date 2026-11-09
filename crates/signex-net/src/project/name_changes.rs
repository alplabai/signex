//! What an edit does to the names of the project's nets (#2199): a name that
//! reaches a net from another sheet is invisible to a one-sheet check, so the
//! name side of the headless intent checks is judged on the stitched project.
//! Built on [`multi_named_nets`], the one derivation of "a net with several
//! names", before and after; there is no second project connectivity model.

use std::collections::BTreeSet;

use super::{ProjectGraph, multi_named_nets};

/// The sets of names that a net carries after the edit and no net carried
/// before: two named nets made one, or a named net that gained a name. Names
/// are the text as written on a sheet; the caller refuses unless the edit
/// said `allow_merge`. Empty when the edit changes no net's names.
pub fn project_name_changes(before: &ProjectGraph, after: &ProjectGraph) -> Vec<Vec<String>> {
    let sets = |graph: &ProjectGraph| -> BTreeSet<Vec<String>> {
        multi_named_nets(graph)
            .into_iter()
            .map(|net| {
                let names: BTreeSet<String> =
                    net.sources.into_iter().map(|source| source.name).collect();
                names.into_iter().collect()
            })
            .collect()
    };
    let known = sets(before);
    sets(after)
        .into_iter()
        .filter(|names| !known.contains(names))
        .collect()
}
