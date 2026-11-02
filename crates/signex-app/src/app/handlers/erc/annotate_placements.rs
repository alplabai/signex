//! Sheet files a project places more than once, and how Annotate walks them
//! (#1835): each placement is a numbering unit of its own.

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use signex_engine::AnnotatePlacement;
use signex_types::schematic::{InstancePath, Symbol};

use crate::app::project_sheets::{self as ps, ProjectSheetSet};

/// Every sheet file the project reaches more than once, with the placement
/// (root first) of each occurrence. Files placed once are absent.
pub(super) fn multi_placed_files(
    set: &ProjectSheetSet,
    base_dir: Option<&Path>,
) -> HashMap<PathBuf, Vec<InstancePath>> {
    let Some(root_path) = set.root.as_deref() else {
        return HashMap::new();
    };
    // Nothing is placed twice in a project without a single sheet symbol.
    if set
        .sheets
        .values()
        .all(|sheet| sheet.child_sheets.is_empty())
    {
        return HashMap::new();
    }
    let graph = ps::project_graph_with(&set.sheets, base_dir, Some(root_path), set.fold);
    let root_key = ps::sheet_key_with(root_path, base_dir, set.fold);
    if !graph.sheets.contains_key(&root_key) {
        return HashMap::new();
    }
    let roots = ps::project_roots(root_key, set, &graph, base_dir, set.fold);
    let occurrences = signex_net::project_occurrences(&signex_net::ProjectGraph {
        sheets: &graph.sheets,
        resolved: &graph.resolved,
        roots: &roots,
    });
    let mut by_key: HashMap<signex_net::SheetKey, Vec<InstancePath>> = HashMap::new();
    for occurrence in occurrences {
        by_key
            .entry(occurrence.key)
            .or_default()
            .push(occurrence.instance_path);
    }
    by_key
        .into_iter()
        .filter_map(|(key, mut paths)| {
            paths.sort();
            paths.dedup();
            let file = graph.paths.get(&key)?.clone();
            (paths.len() > 1).then_some((file, paths))
        })
        .collect()
}

/// The numbering units of a pass: one per sheet file, or one per placement of
/// a file placed more than once.
pub(super) struct WalkDocs<'a> {
    /// Symbols per numbering unit, in walk order. A placement sees the
    /// designators it resolves to there.
    pub docs: Vec<Cow<'a, [Symbol]>>,
    /// Per path of the walk: its first unit's ordinal.
    pub first_unit: Vec<usize>,
    /// Per path of the walk: its placements, empty for a file placed once.
    pub placements: Vec<Vec<AnnotatePlacement>>,
}

impl<'a> WalkDocs<'a> {
    pub(super) fn new(
        paths: &[PathBuf],
        symbols_of: impl Fn(&Path) -> &'a [Symbol],
        multi: &HashMap<PathBuf, Vec<InstancePath>>,
    ) -> Self {
        let mut docs = Vec::new();
        let mut first_unit = Vec::with_capacity(paths.len());
        let mut placements = Vec::with_capacity(paths.len());
        for path in paths {
            let symbols = symbols_of(path);
            first_unit.push(docs.len());
            let Some(places) = multi.get(path) else {
                docs.push(Cow::Borrowed(symbols));
                placements.push(Vec::new());
                continue;
            };
            let mut here = Vec::with_capacity(places.len());
            for place in places {
                let view = symbols
                    .iter()
                    .cloned()
                    .map(|mut symbol| {
                        symbol.reference = symbol.designator_in(place).to_string();
                        symbol
                    })
                    .collect();
                here.push(AnnotatePlacement {
                    sheet: docs.len(),
                    path: place.clone(),
                });
                docs.push(Cow::Owned(view));
            }
            placements.push(here);
        }
        Self {
            docs,
            first_unit,
            placements,
        }
    }

    pub(super) fn slices(&self) -> Vec<&[Symbol]> {
        self.docs.iter().map(AsRef::as_ref).collect()
    }
}
