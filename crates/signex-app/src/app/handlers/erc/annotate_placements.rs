//! Sheet files a project places more than once, and how Annotate walks them
//! (#1835): each placement is a numbering unit of its own.

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use signex_engine::AnnotatePlacement;
use signex_types::schematic::{InstancePath, Symbol};

use crate::app::Signex;
use crate::app::project_sheets::{self as ps, ProjectSheetSet};
use crate::app::state::DocumentState;

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

/// The directory sheet keys are made relative to: the active project's folder,
/// else the root sheet's.
pub(super) fn project_base_dir(
    documents: &DocumentState,
    set: &ProjectSheetSet,
) -> Option<PathBuf> {
    project_base_dir_of(
        documents
            .active_document_project()
            .map(|project| project.dir().to_path_buf())
            .as_deref(),
        set,
    )
}

/// [`project_base_dir`] from an already-read project folder.
pub(super) fn project_base_dir_of(
    project_dir: Option<&Path>,
    set: &ProjectSheetSet,
) -> Option<PathBuf> {
    project_dir.map(Path::to_path_buf).or_else(|| {
        set.root
            .as_deref()
            .and_then(Path::parent)
            .map(PathBuf::from)
    })
}

/// What a pass needs before it touches a sheet: the project's parts, the
/// counter seeded from the designators that stay, and per path of the walk its
/// first unit and placements.
pub(super) type Walked = (
    signex_types::schematic::ProjectParts,
    HashMap<String, u32>,
    Vec<usize>,
    Vec<Vec<AnnotatePlacement>>,
);

impl Signex {
    /// Group the project's units into parts and seed the shared counter.
    ///
    /// Parts are grouped across the WHOLE project before any sheet is touched:
    /// a part whose units sit on different sheets must stay one part through a
    /// reset (#691). A live engine is the document, so it wins over the
    /// assembler's copy. A file placed more than once is one unit per
    /// placement (#1835). `resets` says the run clears every unlocked
    /// designator, so those do not hold a number back.
    pub(super) fn walk_project(
        &self,
        project_set: &ProjectSheetSet,
        ordered_paths: &[PathBuf],
        locked: &std::collections::HashSet<uuid::Uuid>,
        resets: bool,
    ) -> Walked {
        let base_dir = project_base_dir(&self.document_state, project_set);
        let multi = multi_placed_files(project_set, base_dir.as_deref());
        let walk = WalkDocs::new(
            ordered_paths,
            |path| {
                self.document_state
                    .engines
                    .get(path)
                    .map(|engine| engine.document().symbols.as_slice())
                    .or_else(|| {
                        project_set
                            .sheets
                            .get(path)
                            .map(|sheet| sheet.symbols.as_slice())
                    })
                    .unwrap_or(&[])
            },
            &multi,
        );
        let seed = signex_types::designator::seed_counters(
            walk.docs
                .iter()
                .flat_map(|doc| doc.iter())
                .filter(|sym| !sym.is_power && !sym.reference.starts_with('#'))
                .filter(|sym| !resets || locked.contains(&sym.uuid))
                .map(|sym| sym.reference.as_str()),
        );
        let parts = signex_types::schematic::ProjectParts::new(
            &walk.slices(),
            locked,
            self.ui_state.annotate_order,
        );
        (parts, seed, walk.first_unit, walk.placements)
    }
}
