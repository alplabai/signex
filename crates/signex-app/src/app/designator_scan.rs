//! Every designator a project uses, for numbering a new part (#1381).
//!
//! A part placed on one sheet must not reuse a number another sheet of the
//! project already carries, including sheets nobody has open. Reading those
//! blocks, so the scan runs off the UI thread when a placement session
//! arms, and `update` only reads the cached answer. It does not depend on
//! the command palette's index.
//!
//! The cache is keyed on the project, its page list and the documents
//! epoch (bumped when a document is opened, saved or closed), never on an
//! open engine's revision, so placing a part does not invalidate it. Open
//! sheets are always read live from their engines; the cache only supplies
//! the closed ones and the placements each sheet is reached through.
//! Until a scan lands the suggestion is provisional: it sees open sheets
//! only, and the placement ghost refreshes when the answer arrives.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use signex_types::schematic::InstancePath;

use super::project_sheets::{self, OpenSheets};
use super::sheet_hierarchy::build_sheet_hierarchy;
use super::state::ProjectId;
use super::state::scope::CaseFold;
use super::{Message, Signex};
use crate::library::messages::LibraryMessage;

/// What the last scan found, and which inputs it was for.
#[derive(Debug, Clone, Default)]
pub struct DesignatorCache {
    project: Option<ProjectId>,
    stamp: Option<u64>,
    building: Option<u64>,
    generation: u64,
    placements: HashMap<PathBuf, Vec<InstancePath>>,
    designators: HashMap<PathBuf, Vec<String>>,
}

/// One finished scan.
#[derive(Debug)]
pub struct DesignatorScan {
    project: ProjectId,
    stamp: u64,
    placements: HashMap<PathBuf, Vec<InstancePath>>,
    designators: HashMap<PathBuf, Vec<String>>,
}

/// What a scan reads, captured in `update`.
pub(crate) struct ScanInputs {
    project: ProjectId,
    stamp: u64,
    open: OpenSheets,
    pages: Vec<PathBuf>,
    root: PathBuf,
    fold: CaseFold,
}

/// Walk the project and list, per sheet, the designators it shows at each
/// placement. Blocking: call it off the UI thread.
pub(crate) fn scan(inputs: ScanInputs) -> DesignatorScan {
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
    let mut placements = HashMap::new();
    let mut designators = HashMap::new();
    for (path, sheet) in &set.sheets {
        let at = hierarchy.designator_scope(path).placements;
        let mut used = Vec::new();
        for symbol in sheet.symbols.iter().filter(|s| !s.reference.is_empty()) {
            used.push(symbol.reference.clone());
            used.extend(at.iter().map(|p| symbol.designator_in(p).to_string()));
        }
        placements.insert(path.clone(), at);
        designators.insert(path.clone(), used);
    }
    DesignatorScan {
        project: inputs.project,
        stamp: inputs.stamp,
        placements,
        designators,
    }
}

impl Signex {
    /// Hash of what a scan of `sheet`'s project reads from disk.
    fn designator_scan_stamp(&self, project: ProjectId, sheet: &Path) -> u64 {
        let ds = &self.document_state;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        project.hash(&mut h);
        project_sheets::resolve_netlist_root_for(ds, sheet).hash(&mut h);
        project_sheets::project_pages(ds.document_project(sheet)).hash(&mut h);
        ds.documents_epoch.hash(&mut h);
        h.finish()
    }

    fn capture_designator_inputs(&self, id: ProjectId, sheet: &Path, stamp: u64) -> ScanInputs {
        let ds = &self.document_state;
        ScanInputs {
            project: id,
            stamp,
            open: project_sheets::open_sheets_snapshot(ds),
            pages: project_sheets::project_pages(ds.document_project(sheet)),
            root: project_sheets::resolve_netlist_root_for(ds, sheet),
            fold: crate::app::state::project_case::fold_of(ds.document_project(sheet)),
        }
    }

    /// Scan the project that owns `sheet` off-thread unless the cache is
    /// already for the current inputs. A sheet with no project needs none.
    pub(crate) fn start_designator_scan(&mut self, sheet: &Path) -> Task<Message> {
        let ds = &self.document_state;
        let Some(project) = ds.document_project(sheet) else {
            return Task::none();
        };
        let id = project.id;
        let stamp = self.designator_scan_stamp(id, sheet);
        let cache = &self.document_state.designator_cache;
        if (cache.project == Some(id) && cache.stamp == Some(stamp))
            || cache.building == Some(stamp)
        {
            return Task::none();
        }
        let inputs = self.capture_designator_inputs(id, sheet, stamp);
        let cache = &mut self.document_state.designator_cache;
        cache.generation += 1;
        cache.building = Some(stamp);
        let generation = cache.generation;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || scan(inputs))
                    .await
                    .map(Arc::new)
                    .map_err(|e| format!("Designator scan worker failed: {e}"))
            },
            move |result| {
                Message::Library(LibraryMessage::DesignatorsScanned {
                    generation,
                    result: Arc::new(result),
                })
            },
        )
    }

    /// Run the scan on the calling thread and apply it - tests only.
    #[cfg(test)]
    pub(crate) fn scan_designators_blocking(&mut self, sheet: &Path) {
        let Some(id) = self.document_state.document_project(sheet).map(|p| p.id) else {
            return;
        };
        let stamp = self.designator_scan_stamp(id, sheet);
        let inputs = self.capture_designator_inputs(id, sheet, stamp);
        let cache = &mut self.document_state.designator_cache;
        cache.generation += 1;
        let generation = cache.generation;
        self.apply_designator_scan(generation, &Ok(Arc::new(scan(inputs))));
    }

    /// A scan arrived: kept unless a newer request superseded it.
    pub(crate) fn apply_designator_scan(
        &mut self,
        generation: u64,
        result: &Result<Arc<DesignatorScan>, String>,
    ) {
        let cache = &mut self.document_state.designator_cache;
        if generation != cache.generation {
            return;
        }
        cache.building = None;
        match result {
            Ok(found) => {
                cache.project = Some(found.project);
                cache.stamp = Some(found.stamp);
                cache.placements = found.placements.clone();
                cache.designators = found.designators.clone();
            }
            Err(why) => crate::diagnostics::log_warning(why),
        }
    }

    /// Every designator in use on `sheet`'s project, for numbering a new
    /// part. Open sheets come from their engines, at every placement the
    /// last scan found for them; closed sheets come from the scan. A sheet
    /// with no project yields only its own.
    pub(crate) fn used_designators(&self, sheet: &Path) -> Vec<&str> {
        let ds = &self.document_state;
        let Some(project) = ds.document_project(sheet) else {
            return live_designators(ds.engines.get(sheet), &[]);
        };
        let cache = &ds.designator_cache;
        let known = cache.project == Some(project.id);
        let mut used = Vec::new();
        for (path, engine) in &ds.engines {
            if ds
                .document_project(path)
                .is_some_and(|owner| owner.id == project.id)
            {
                let at = known
                    .then(|| cache.placements.get(path))
                    .flatten()
                    .map_or(&[][..], Vec::as_slice);
                used.extend(live_designators(Some(engine), at));
            }
        }
        if known {
            for (path, found) in &cache.designators {
                if !ds.engines.contains_key(path) {
                    used.extend(found.iter().map(String::as_str));
                }
            }
        }
        used
    }
}

/// What an open sheet uses: each symbol's own reference and what it shows
/// at each of `placements`.
fn live_designators<'a>(
    engine: Option<&'a signex_engine::Engine>,
    placements: &[InstancePath],
) -> Vec<&'a str> {
    let Some(engine) = engine else {
        return Vec::new();
    };
    let mut used = Vec::new();
    for symbol in &engine.document().symbols {
        used.push(symbol.reference.as_str());
        used.extend(placements.iter().map(|p| symbol.designator_in(p)));
    }
    used
}
