//! Sheet files a project places more than once, and how Annotate walks them
//! (#1835): each placement is a numbering unit of its own.

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use signex_engine::AnnotatePlacement;
use signex_types::designator::{NumberBlocks, NumberingScheme};
use signex_types::schematic::{InstancePath, Symbol};

use crate::app::Signex;
use crate::app::project_sheets::{self as ps, ProjectSheetSet};
use crate::app::state::DocumentState;

/// Every sheet file the project reaches more than once, or whose symbols hold
/// an instance entry at its one placement (such a file resolves through the
/// entry, not `reference`), with the placement (root first) of each
/// occurrence. Other files are absent.
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
    let project = signex_net::ProjectGraph {
        sheets: &graph.sheets,
        resolved: &graph.resolved,
        roots: &roots,
    };
    let occurrences = signex_net::project_occurrences(&project);
    // The channels of a repeat are one logical sheet: it is numbered once and
    // nothing is written under a compiled channel (#2740). Naming plays no
    // part in the placement a channel collapses to.
    let channels = signex_net::ChannelDesignators::of_project(&project, &Default::default());
    let mut by_key: HashMap<signex_net::SheetKey, Vec<InstancePath>> = HashMap::new();
    for occurrence in occurrences {
        let placement = channels.logical_path(&occurrence.instance_path).clone();
        by_key.entry(occurrence.key).or_default().push(placement);
    }
    by_key
        .into_iter()
        .filter_map(|(key, mut paths)| {
            paths.sort();
            paths.dedup();
            let file = graph.paths.get(&key)?.clone();
            let resolves_by_entry = |path: &InstancePath| {
                set.sheets.get(&file).is_some_and(|sheet| {
                    sheet
                        .symbols
                        .iter()
                        .any(|symbol| symbol.has_instance_entry_in(path))
                })
            };
            (paths.len() > 1 || paths.iter().any(resolves_by_entry)).then_some((file, paths))
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

/// The page number of every numbering unit of a walk over `paths` (one unit
/// per path, or per placement of a file in `multi`, as [`WalkDocs`] lays
/// them out), from the project's sheet numbering: the numbers the title block
/// and Number Schematic Sheets show (`signex_net::sheet_numbering`). A
/// placement takes the page of its own instance path, a file placed once the
/// lowest page it prints on; `None` where the numbering reaches no page.
/// `printed` is [`printed_pages`] of the project.
pub(super) fn unit_pages(
    printed: &PrintedPages,
    paths: &[PathBuf],
    multi: &HashMap<PathBuf, Vec<InstancePath>>,
) -> Vec<Option<usize>> {
    let mut pages = Vec::with_capacity(paths.len());
    for path in paths {
        let on_file = printed.get(path).map(Vec::as_slice).unwrap_or(&[]);
        let lowest = on_file.iter().map(|(_, number)| *number).min();
        let Some(places) = multi.get(path) else {
            pages.push(lowest);
            continue;
        };
        for place in places {
            let own = (on_file.iter())
                .find(|(at, _)| at == place)
                .map(|(_, number)| *number);
            pages.push(own.or(lowest));
        }
    }
    pages
}

/// The numbering scheme of the active document's project (#1292), read by
/// the pass and the preview alike; a sheet in no project numbers
/// continuously.
pub(super) fn numbering_scheme(documents: &DocumentState) -> NumberingScheme {
    documents
        .active_document_project()
        .map(|project| project.data.annotate_numbering())
        .unwrap_or_default()
}

/// Every printed page of a project, by sheet file: its instance path and
/// number.
pub(super) type PrintedPages = HashMap<PathBuf, Vec<(InstancePath, usize)>>;

/// [`PrintedPages`] of `set`; empty when the root sheet did not assemble.
pub(super) fn printed_pages(
    set: &ProjectSheetSet,
    base_dir: Option<&Path>,
    project_name: &str,
) -> PrintedPages {
    let mut printed = PrintedPages::new();
    let Some(root_path) = set.root.as_deref() else {
        return printed;
    };
    let graph = ps::project_graph_with(&set.sheets, base_dir, Some(root_path), set.fold);
    let root_key = ps::sheet_key_with(root_path, base_dir, set.fold);
    if !graph.sheets.contains_key(&root_key) {
        return printed;
    }
    let roots = ps::project_roots(root_key, set, &graph, base_dir, set.fold);
    let project = signex_net::ProjectGraph {
        sheets: &graph.sheets,
        resolved: &graph.resolved,
        roots: &roots,
    };
    let (numbering, _) = signex_net::sheet_numbering(&project, project_name);
    for page in numbering.pages() {
        if let Some(file) = graph.paths.get(&page.key) {
            (printed.entry(file.clone()).or_default())
                .push((page.instance_path.clone(), page.number));
        }
    }
    printed
}

/// What a pass needs before it touches a sheet: the project's parts, the
/// counter seeded from the designators that stay, per path of the walk its
/// first unit and placements, and (#1292) the page blocks of the project's
/// numbering scheme.
pub(super) struct Walked {
    pub parts: signex_types::schematic::ProjectParts,
    pub seed: HashMap<String, u32>,
    pub first_unit: Vec<usize>,
    pub placements: Vec<Vec<AnnotatePlacement>>,
    /// `None` for the continuous scheme; `Err` names why the project's page
    /// scheme cannot number this run.
    pub blocks: Result<Option<NumberBlocks>, String>,
}

impl Signex {
    /// Group the project's units into parts and seed the shared counter.
    ///
    /// Parts are grouped across the WHOLE project before any sheet is touched:
    /// a part whose units sit on different sheets must stay one part through a
    /// reset (#691). A live engine is the document, so it wins over the
    /// assembler's copy. A file placed more than once is one unit per
    /// placement (#1835). `resets` says the run clears every unlocked
    /// designator, so those do not hold a number back. `in_run` are the paths
    /// the run numbers: under a page scheme (#1292) only their units get a
    /// block, and each of them needs a page of its own.
    pub(super) fn walk_project(
        &self,
        project_set: &ProjectSheetSet,
        ordered_paths: &[PathBuf],
        locked: &std::collections::HashSet<uuid::Uuid>,
        resets: bool,
        in_run: &[PathBuf],
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
        let kept = || {
            (walk.docs.iter().flat_map(|doc| doc.iter()))
                .filter(|sym| !sym.is_power && !sym.reference.starts_with('#'))
                .filter(|sym| !resets || locked.contains(&sym.uuid))
                .map(|sym| sym.reference.as_str())
        };
        let seed = signex_types::designator::seed_counters(kept());
        let scheme = numbering_scheme(&self.document_state);
        let blocks = if scheme == NumberingScheme::Continuous {
            Ok(None)
        } else {
            let project = self.document_state.active_document_project();
            let name = project.map_or("", |p| p.data.name.as_str());
            let printed = printed_pages(project_set, base_dir.as_deref(), name);
            let mut pages = unit_pages(&printed, ordered_paths, &multi);
            only_numbered_units(&mut pages, ordered_paths, &walk.first_unit, in_run).and_then(
                |()| {
                    NumberBlocks::new(scheme, &pages, kept()).map_err(|problem| problem.to_string())
                },
            )
        };
        let parts = signex_types::schematic::ProjectParts::new(
            &walk.slices(),
            locked,
            self.ui_state.annotate_order,
        );
        Walked {
            parts,
            seed,
            first_unit: walk.first_unit,
            placements: walk.placements,
            blocks,
        }
    }
}

/// Clear the page of every unit whose path the run leaves alone, so it gets
/// no block; a unit the run numbers without a page refuses the run.
fn only_numbered_units(
    pages: &mut [Option<usize>],
    paths: &[PathBuf],
    first_unit: &[usize],
    in_run: &[PathBuf],
) -> Result<(), String> {
    for (index, path) in paths.iter().enumerate() {
        let start = first_unit[index];
        let end = first_unit.get(index + 1).copied().unwrap_or(pages.len());
        let numbered = in_run.contains(path);
        for page in pages.iter_mut().take(end).skip(start) {
            if !numbered {
                *page = None;
            } else if page.is_none() {
                return Err(format!(
                    "{} has no page number for the page numbering scheme; number the sheets first",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place(n: u128) -> InstancePath {
        InstancePath::from_chain(vec![uuid::Uuid::from_u128(n)])
    }

    /// A placement takes its own page; a file placed once its lowest; a file
    /// the numbering never reached none.
    #[test]
    fn each_unit_takes_the_page_it_prints_on() {
        let (top, sensors, loose) = (
            PathBuf::from("top.snxsch"),
            PathBuf::from("sensors.snxsch"),
            PathBuf::from("loose.snxsch"),
        );
        let printed = PrintedPages::from([
            (top.clone(), vec![(InstancePath::default(), 1)]),
            (sensors.clone(), vec![(place(2), 3), (place(1), 2)]),
        ]);
        let multi = HashMap::from([(sensors.clone(), vec![place(1), place(2)])]);
        let pages = unit_pages(&printed, &[top, sensors, loose], &multi);
        assert_eq!(pages, [Some(1), Some(2), Some(3), None]);
    }

    /// Units the run leaves alone lose their page; a numbered unit with none
    /// refuses the run, naming its sheet.
    #[test]
    fn only_numbered_units_keep_a_page() {
        let paths = [PathBuf::from("a.snxsch"), PathBuf::from("b.snxsch")];
        let mut pages = vec![Some(1), Some(2), Some(3)];
        only_numbered_units(&mut pages, &paths, &[0, 1], &paths[..1]).unwrap();
        assert_eq!(pages, [Some(1), None, None]);
        let mut pages = vec![Some(1), None];
        let why = only_numbered_units(&mut pages, &paths, &[0, 1], &paths).unwrap_err();
        assert!(why.contains("b.snxsch"), "{why}");
    }
}
