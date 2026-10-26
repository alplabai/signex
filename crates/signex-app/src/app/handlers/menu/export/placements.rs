//! Which sheet placements a netlist export walks.
//!
//! The Protel file keys every component on its designator, and a sheet file
//! placed twice can carry a different designator per placement (#829), so the
//! exporter needs one entry per placement, not one per file.

use std::collections::HashMap;

use std::path::{Path, PathBuf};

use signex_net::{SheetKey, SheetOccurrence};
use signex_output::SheetSnapshot;

use crate::app::state::scope::CaseFold;
use signex_types::net::SheetPlacement;
use signex_types::schematic::InstancePath;

/// The occurrences the stitcher walked, with how sheet paths key against them:
/// the authority on whether a remembered placement (#1133) still exists. A
/// sheet-symbol deletion, a rebind to another file, an undo or a reload all
/// change the walk, so a placement recorded earlier is checked here, not
/// trusted.
#[derive(Default)]
pub(crate) struct LiveOccurrences {
    occurrences: Vec<SheetOccurrence>,
    keying: Option<(Option<PathBuf>, CaseFold)>,
}

impl LiveOccurrences {
    pub(super) fn new(
        occurrences: Vec<SheetOccurrence>,
        keying: Option<(Option<PathBuf>, CaseFold)>,
    ) -> Self {
        Self {
            occurrences,
            keying,
        }
    }

    /// Whether the walk reached the sheet at `path` through `placement`. With
    /// no derived hierarchy nothing is live.
    pub(crate) fn contains(&self, path: &Path, placement: &InstancePath) -> bool {
        let Some((base_dir, fold)) = &self.keying else {
            return false;
        };
        let key = crate::app::project_sheets::sheet_key_with(path, base_dir.as_deref(), *fold);
        self.occurrences
            .iter()
            .any(|o| o.key == key && &o.instance_path == placement)
    }
}

/// One [`SheetPlacement`] per occurrence the stitcher walked, indexed into
/// `keys` (the export's sheet list, in key order) and ordered by that index
/// then walk order, so a project whose files are each placed once exports its
/// components in the same order as before. A sheet the walk never reached gets
/// no placement, so the BOM counts it zero (#730).
pub(super) fn netlist_placements(
    keys: &[SheetKey],
    occurrences: Vec<SheetOccurrence>,
) -> Vec<SheetPlacement> {
    let index: HashMap<&SheetKey, usize> = keys.iter().enumerate().map(|(i, k)| (k, i)).collect();
    let mut placements: Vec<SheetPlacement> = occurrences
        .into_iter()
        .filter_map(|occ| {
            Some(SheetPlacement {
                sheet_index: *index.get(&occ.key)?,
                instance_path: occ.instance_path,
            })
        })
        .collect();
    // Stable: equal indices keep their walk order.
    placements.sort_by_key(|p| p.sheet_index);
    placements
}

/// The printed pages, one copy per placement of each page's sheet (#829).
///
/// A page whose sheet the walk reached once prints as that placement, with its
/// designators and net names resolved there; one reached N times prints N
/// copies, each titled with its sheet-symbol name chain (a singly placed page
/// keeps its own name). A page the walk never
/// reached stays as it is (root placement). When a page gained copies the pages
/// are renumbered in order; that is the numbering a loose document keeps. A
/// project's pages are then renumbered from the hierarchy
/// (`ScopeNumbering::numbered_pages`, #1081).
pub(super) fn page_copies(
    pages: Vec<SheetSnapshot>,
    occurrences: &[SheetOccurrence],
    base_dir: Option<&Path>,
    fold: CaseFold,
) -> Vec<SheetSnapshot> {
    let mut out: Vec<SheetSnapshot> = Vec::with_capacity(pages.len());
    let mut added_copies = false;
    for page in pages {
        let key = crate::app::project_sheets::sheet_key_with(&page.path, base_dir, fold);
        let here: Vec<&SheetOccurrence> = occurrences.iter().filter(|o| o.key == key).collect();
        if here.is_empty() {
            out.push(page);
            continue;
        }
        added_copies |= here.len() > 1;
        // Only a file with several copies needs telling apart; a singly
        // placed child keeps its own name (bookmarks, picker rows).
        let many = here.len() > 1;
        for occ in here {
            let label = if many {
                occ.name_chain.join("/")
            } else {
                String::new()
            };
            out.push(
                page.clone()
                    .for_placement(occ.instance_path.clone(), &label),
            );
        }
    }
    if added_copies {
        let count = out.len();
        for (i, page) in out.iter_mut().enumerate() {
            page.sheet_number = i + 1;
            page.sheet_count = count;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(s: &str) -> SheetKey {
        SheetKey::new(s)
    }

    fn occ(k: &SheetKey, chain: &[u128]) -> SheetOccurrence {
        SheetOccurrence {
            key: k.clone(),
            instance_path: InstancePath::from_chain(
                chain.iter().map(|n| uuid::Uuid::from_u128(*n)).collect(),
            ),
            name_chain: Vec::new(),
        }
    }

    #[test]
    fn a_twice_placed_file_gets_two_placements_and_an_unreached_one_none() {
        let (a, b, c) = (key("a"), key("b"), key("c"));
        let keys = vec![a.clone(), b.clone(), c.clone()];
        let got = netlist_placements(&keys, vec![occ(&a, &[]), occ(&b, &[1]), occ(&b, &[2])]);
        let shape: Vec<(usize, String)> = got
            .iter()
            .map(|p| (p.sheet_index, p.instance_path.to_string()))
            .collect();
        assert_eq!(shape.len(), 3);
        assert_eq!((shape[0].0, shape[1].0, shape[2].0), (0, 1, 1));
        assert_ne!(shape[1].1, shape[2].1);
    }
}
