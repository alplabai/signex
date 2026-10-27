//! The project-wide index behind the command palette's "search everywhere"
//! rows (#1268): every designator at every placement, and every sheet
//! occurrence of the active project, closed sheets included.
//!
//! The palette's catalog is rebuilt on every keystroke from memory only, so
//! the index is built off the UI thread (it reads the sheets nobody has
//! open) and cached on the palette state. Open sheets are read from a
//! snapshot of their engines, so unsaved edits are in the index.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use super::project_sheets::{self, OpenSheets};
use super::sheet_hierarchy::build_sheet_hierarchy;
use super::state::DocumentState;
use super::state::scope::CaseFold;
use super::*;

/// One designator at one placement of its sheet.
#[derive(Debug, Clone)]
pub struct IndexedDesignator {
    pub path: PathBuf,
    /// The placement the designator was read at; `None` for a sheet the walk
    /// does not reach (its `reference` is read directly).
    pub placement: Option<InstancePath>,
    pub designator: String,
    pub value: String,
    pub symbol: Uuid,
    /// "placement 2 of 3" when the sheet is placed more than once.
    pub placement_label: Option<String>,
}

/// One occurrence of a sheet in the hierarchy.
#[derive(Debug, Clone)]
pub struct IndexedSheet {
    pub path: PathBuf,
    pub name: String,
    pub placement: InstancePath,
}

/// A net and the terminal a search row for it jumps to.
#[derive(Debug, Clone)]
pub struct IndexedNet {
    pub name: String,
    pub terminals: usize,
    /// The first terminal that sits on a sheet the index can name; `None`
    /// when none does, and then the net gets no row.
    pub target: Option<NetTarget>,
}

#[derive(Debug, Clone)]
pub struct NetTarget {
    pub path: PathBuf,
    pub placement: InstancePath,
    pub symbol: Uuid,
}

#[derive(Debug, Clone, Default)]
pub struct ProjectSearchIndex {
    /// [`index_stamp`] of the project and documents it was built from.
    pub stamp: u64,
    pub designators: Vec<IndexedDesignator>,
    pub sheets: Vec<IndexedSheet>,
    /// Nets from the project netlist, derived by the same `signex_net`
    /// project build the export and net-class styling use, on the job's thread.
    pub nets: Vec<IndexedNet>,
}

/// What the index job needs, captured in `update`.
pub(crate) struct IndexInputs {
    open: OpenSheets,
    pages: Vec<PathBuf>,
    root: PathBuf,
    project_dir: Option<PathBuf>,
    fold: CaseFold,
    stamp: u64,
}

/// Hash of what the index reads: which project it is of (root, declared
/// pages, key rules), which documents are open, what they hold on disk (the
/// documents epoch) and each open sheet's revision.
pub fn index_stamp(documents: &DocumentState) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    project_sheets::resolve_netlist_root(documents).hash(&mut h);
    project_sheets::declared_pages(documents).hash(&mut h);
    format!(
        "{:?}",
        crate::app::state::project_case::fold_of(documents.active_document_project())
    )
    .hash(&mut h);
    documents.documents_epoch.hash(&mut h);
    let mut engines: Vec<_> = documents.engines.iter().collect();
    engines.sort_by(|a, b| a.0.cmp(b.0));
    for (path, engine) in engines {
        (path, engine.revision()).hash(&mut h);
    }
    h.finish()
}

pub(crate) fn build_index(inputs: IndexInputs) -> ProjectSearchIndex {
    let set = project_sheets::assemble_project_sheets_from(
        &inputs.open,
        &inputs.pages,
        &inputs.root,
        inputs.fold,
    );
    let hierarchy = build_sheet_hierarchy(
        &set.sheets,
        &set.unreadable,
        &inputs.root,
        &set.pages_outside_the_hierarchy,
        inputs.fold,
    );
    let sheets = hierarchy
        .occurrences()
        .into_iter()
        .map(|(placement, node)| IndexedSheet {
            path: node.path.clone(),
            name: node.name.clone(),
            placement,
        })
        .collect();
    let mut index = ProjectSearchIndex {
        stamp: inputs.stamp,
        sheets,
        ..ProjectSearchIndex::default()
    };
    for path in project_sheets::ordered_project_sheet_paths(&set, None) {
        let Some(sheet) = set.sheets.get(&path) else {
            continue;
        };
        let scope = hierarchy.designator_scope(&path);
        for symbol in sheet.symbols.iter().filter(|s| !s.reference.is_empty()) {
            let mut push = |placement: Option<&InstancePath>, label: Option<String>| {
                let designator =
                    placement.map_or(symbol.reference.as_str(), |p| symbol.designator_in(p));
                index.designators.push(IndexedDesignator {
                    path: path.clone(),
                    placement: placement.cloned(),
                    designator: designator.to_string(),
                    value: symbol.value.clone(),
                    symbol: symbol.uuid,
                    placement_label: label,
                });
            };
            if scope.placements.is_empty() {
                push(None, None);
                continue;
            }
            let many = scope.placements.len() > 1;
            for (i, placement) in scope.placements.iter().enumerate() {
                let label =
                    many.then(|| format!("placement {} of {}", i + 1, scope.placements.len()));
                push(Some(placement), label);
            }
        }
    }
    index.nets = index_nets(&set, &hierarchy, &index.designators, inputs.project_dir);
    index
}

/// The nets of the project, each with the terminal its row jumps to. A
/// terminal names a symbol and the occurrence it sits at; the sheet is the
/// one whose designator row has exactly that `(symbol, placement)`, so a
/// flat project's second page (which shares the root chain with the first)
/// is found by its symbol. A terminal no row names falls back to the sheet
/// the occurrence chain reaches, but only for a non-root chain.
fn index_nets(
    set: &project_sheets::ProjectSheetSet,
    hierarchy: &super::sheet_hierarchy::SheetHierarchy,
    designators: &[IndexedDesignator],
    project_dir: Option<PathBuf>,
) -> Vec<IndexedNet> {
    let Some(netlist) = super::stitched_netlist::netlist_of_set(set, project_dir).netlist else {
        return Vec::new();
    };
    let mut by_symbol: HashMap<(Uuid, InstancePath), &PathBuf> = HashMap::new();
    for d in designators {
        if let Some(placement) = &d.placement {
            by_symbol
                .entry((d.symbol, placement.clone()))
                .or_insert(&d.path);
        }
    }
    netlist
        .nets
        .iter()
        .filter(|net| !net.name.is_empty())
        .map(|net| {
            let target = net.terminals.iter().find_map(|t| {
                if t.symbol.is_nil() {
                    return None;
                }
                let path = match by_symbol.get(&(t.symbol, t.occurrence.clone())) {
                    Some(path) => (*path).clone(),
                    None if !t.occurrence.is_root() => hierarchy.sheet_path_at(&t.occurrence)?,
                    None => return None,
                };
                Some(NetTarget {
                    path,
                    placement: t.occurrence.clone(),
                    symbol: t.symbol,
                })
            });
            IndexedNet {
                name: net.name.clone(),
                terminals: net.terminals.len(),
                target,
            }
        })
        .collect()
}

impl Signex {
    /// Build the index off-thread when the open documents no longer match
    /// the cached one. A newer request supersedes an older one in flight.
    pub(crate) fn start_palette_index(&mut self) -> Task<Message> {
        // The stamp first: a keystroke with nothing changed must not clone
        // every open document just to find that out.
        let stamp = index_stamp(&self.document_state);
        let palette = &mut self.ui_state.command_palette;
        if palette.index.as_ref().map(|i| i.stamp) == Some(stamp)
            || palette.index_building == Some(stamp)
        {
            return Task::none();
        }
        let Some(inputs) = self.capture_index_inputs(stamp) else {
            // No project to search: an index of the last one would mislead.
            let palette = &mut self.ui_state.command_palette;
            palette.index = None;
            palette.index_building = None;
            return Task::none();
        };
        let palette = &mut self.ui_state.command_palette;
        palette.index_generation += 1;
        palette.index_building = Some(stamp);
        let generation = palette.index_generation;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || build_index(inputs))
                    .await
                    .map(Arc::new)
                    .map_err(|e| format!("Search index worker failed: {e}"))
            },
            move |result| {
                Message::CommandPalette(CommandPaletteMsg::IndexBuilt {
                    generation,
                    result: Arc::new(result),
                })
            },
        )
    }

    /// Run the index job on the calling thread and apply it - tests only.
    #[cfg(test)]
    pub(crate) fn build_palette_index_blocking(&mut self) {
        let stamp = index_stamp(&self.document_state);
        let Some(inputs) = self.capture_index_inputs(stamp) else {
            self.ui_state.command_palette.index = None;
            return;
        };
        self.ui_state.command_palette.index_generation += 1;
        let generation = self.ui_state.command_palette.index_generation;
        self.apply_palette_index(generation, &Ok(Arc::new(build_index(inputs))));
    }

    fn capture_index_inputs(&mut self, stamp: u64) -> Option<IndexInputs> {
        let ds = &self.document_state;
        let root = project_sheets::resolve_netlist_root(ds)?;
        #[cfg(test)]
        {
            self.ui_state.command_palette.index_snapshots_taken += 1;
        }
        let ds = &self.document_state;
        Some(IndexInputs {
            open: project_sheets::open_sheets_snapshot(ds),
            pages: project_sheets::declared_pages(ds),
            root,
            project_dir: ds.active_document_project().map(|p| p.dir().to_path_buf()),
            fold: crate::app::state::project_case::fold_of(ds.active_document_project()),
            stamp,
        })
    }

    /// An index arrived: kept unless a newer request superseded it, and the
    /// open palette re-ranks against it.
    pub(crate) fn apply_palette_index(
        &mut self,
        generation: u64,
        result: &Result<Arc<ProjectSearchIndex>, String>,
    ) {
        let palette = &mut self.ui_state.command_palette;
        if generation != palette.index_generation {
            return;
        }
        palette.index_building = None;
        match result {
            Ok(index) => palette.index = Some(Arc::clone(index)),
            Err(why) => {
                crate::diagnostics::log_warning(why);
                return;
            }
        }
        if self.ui_state.command_palette.open {
            self.refresh_command_palette_results();
        }
    }
}
