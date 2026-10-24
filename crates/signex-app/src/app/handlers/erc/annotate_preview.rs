//! Annotate preview - the project-wide proposed-designator list the Annotate
//! modal shows before the user commits.
//!
//! The preview is cached in `ui_state.annotate_preview` and never computed in
//! `view` (#111). Loading the project's sheets reads unopened ones from disk,
//! so it runs in a `spawn_blocking` task; the cache is keyed by a content
//! fingerprint of the open documents and reloaded whenever that changes, so
//! the dialog cannot keep describing sheets the action would no longer see.

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use signex_types::schematic::{SchematicSheet, Symbol};

use crate::app::project_sheets::{self, OpenSheets};
use crate::app::{AnnotateMsg, Message, Signex};

/// One row of the project-wide proposed change list.
#[derive(Debug, Clone)]
pub struct AnnotatePreviewEntry {
    pub sheet: String,
    pub current: String,
    pub proposed: String,
    /// Symbol uuid - lets the row's lock checkbox toggle the global
    /// `ui_state.annotate_locked` set without re-looking-up the symbol.
    pub uuid: uuid::Uuid,
}

/// Cached preview state, owned by `UiState`.
#[derive(Debug, Default, Clone)]
pub struct AnnotatePreviewCache {
    /// Token of the newest load; a finished load applies only if it matches.
    generation: u64,
    /// Fingerprint of the documents the loaded (or loading) sheets reflect.
    /// `None` = nothing loaded for the current dialog session.
    fingerprint: Option<u64>,
    /// A load is in flight.
    pub loading: bool,
    /// The newest load failed; the dialog says so instead of claiming there
    /// is nothing to annotate.
    pub load_failed: bool,
    /// The project's sheets, in the order the action walks them.
    sheets: Arc<Vec<(String, SchematicSheet)>>,
    /// Rows derived from `sheets` and the lock set - what the dialog renders.
    pub entries: Vec<AnnotatePreviewEntry>,
}

/// The sheets (with display titles) a load produced, plus anything that could
/// not be read.
#[derive(Debug)]
pub struct AnnotatePreviewLoad {
    sheets: Vec<(String, SchematicSheet)>,
    warnings: Vec<String>,
}

/// Owned inputs for the blocking load, captured in `update`.
pub(crate) struct AnnotatePreviewInputs {
    open: OpenSheets,
    pages: Vec<PathBuf>,
    root: Option<PathBuf>,
    fold: crate::app::state::scope::CaseFold,
    active_path: Option<PathBuf>,
    active_doc: Option<SchematicSheet>,
    tab_titles: Vec<(PathBuf, String)>,
}

/// Power ports (`#PWR`, `#FLG`, `is_power`) aren't designators — they're net
/// anchors whose "reference" is the net name.
fn is_target(sym: &Symbol) -> bool {
    !sym.is_power && !sym.reference.starts_with('#')
}

/// The alphabetic head of a reference — `"R"` for both `"R12"` and `"R?"`.
fn prefix_of(reference: &str) -> String {
    reference
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect()
}

/// Highest number already claimed per prefix across `sheets` — the same seed
/// `handle_annotate`'s pass A builds, from the same sheet set.
fn seed_counters(sheets: &[(String, SchematicSheet)]) -> std::collections::HashMap<String, u32> {
    let mut next: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    for (_, sheet) in sheets {
        for sym in sheet.symbols.iter().filter(|s| is_target(s)) {
            let prefix = prefix_of(&sym.reference);
            if prefix.is_empty() {
                continue;
            }
            if let Ok(n) = sym.reference[prefix.len()..].parse::<u32>() {
                let e = next.entry(prefix).or_insert(0);
                *e = (*e).max(n);
            }
        }
    }
    next
}

/// Display title of a sheet: its tab title when open, its file stem otherwise.
fn sheet_display_title(tab_titles: &[(PathBuf, String)], path: &Path) -> String {
    if let Some((_, title)) = tab_titles.iter().find(|(p, _)| p.as_path() == path) {
        return title.clone();
    }
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Sheet")
        .to_string()
}

/// The sheets the change list spans, in the one order
/// [`project_sheets::ordered_project_sheet_paths`] defines - the same order
/// `handle_annotate` walks, so the row order (and the designator numbers
/// derived from it) can't drift from what the action assigns (#435).
///
/// The set comes from the one assembler - the same call `handle_annotate`
/// makes - rather than a private rule of its own (#406). Reads unopened
/// sheets from disk, so it only ever runs on a blocking thread.
fn load_preview_sheets(inputs: AnnotatePreviewInputs) -> AnnotatePreviewLoad {
    let set = match &inputs.root {
        Some(root) => project_sheets::assemble_project_sheets_from(
            &inputs.open,
            &inputs.pages,
            root,
            inputs.fold,
        ),
        None => project_sheets::ProjectSheetSet::empty(),
    };
    let warnings = set
        .unreadable
        .iter()
        .map(|(path, why)| format!("Annotate preview: sheet {} {why}", path.display()))
        .collect();
    let active_path = inputs.active_path.as_deref();
    let ordered_paths = project_sheets::ordered_project_sheet_paths(&set, active_path);
    let mut sheets = set.sheets;
    let mut out: Vec<(String, SchematicSheet)> = Vec::with_capacity(ordered_paths.len() + 1);
    for path in &ordered_paths {
        let sheet = match sheets.remove(path) {
            Some(sheet) => sheet,
            // The active document's own path, appended by the ordering
            // helper because the assembler doesn't cover it - read it from
            // the live engine's document instead of the (empty) set.
            None if Some(path.as_path()) == active_path => match &inputs.active_doc {
                Some(doc) => doc.clone(),
                None => continue,
            },
            None => continue,
        };
        out.push((sheet_display_title(&inputs.tab_titles, path), sheet));
    }
    // A never-saved active document has no path at all, so it can't appear in
    // `ordered_paths` either - same unconditional last slot `handle_annotate`
    // gives the active engine in that case.
    if active_path.is_none()
        && let Some(doc) = inputs.active_doc
    {
        out.push(("(untitled)".to_string(), doc));
    }
    AnnotatePreviewLoad {
        sheets: out,
        warnings,
    }
}

/// The proposed `(current, new)` reference designators Annotate would hand
/// out across the project. One row per symbol so the user sees the whole
/// project; rows where `current == proposed` are "no change". Pure CPU over
/// already-loaded sheets.
fn compute_entries(
    sheets: &[(String, SchematicSheet)],
    locked: &std::collections::HashSet<uuid::Uuid>,
    order: crate::app::state::AnnotateOrder,
) -> Vec<AnnotatePreviewEntry> {
    let mut next = seed_counters(sheets);
    // The same project-wide grouping the action builds, so the preview
    // promises one designator per multi-unit part, even one split across
    // sheets, and honours locks the way the pass does (#691).
    let documents: Vec<&[Symbol]> = sheets
        .iter()
        .map(|(_, sheet)| sheet.symbols.as_slice())
        .collect();
    let parts = signex_types::schematic::ProjectParts::new(&documents, locked, order);
    let mut designator_of_part: std::collections::HashMap<usize, String> =
        std::collections::HashMap::new();
    let mut out = Vec::new();
    for (sheet_ordinal, (title, sheet)) in sheets.iter().enumerate() {
        for i in signex_types::schematic::annotation_order(&sheet.symbols, order) {
            let sym = &sheet.symbols[i];
            if sym.reference.is_empty() || !is_target(sym) {
                continue;
            }
            let prefix = prefix_of(&sym.reference);
            let part = parts.part(sheet_ordinal, i);
            let pinned = locked.contains(&sym.uuid)
                || part.is_some_and(|p| parts.locked_designator(p).is_some());
            let proposed = if sym.reference.ends_with('?') && !prefix.is_empty() && !pinned {
                match part.and_then(|p| designator_of_part.get(&p)) {
                    Some(designator) => designator.clone(),
                    None => {
                        let n = next.entry(prefix.clone()).or_insert(0);
                        *n += 1;
                        let designator = format!("{prefix}{n}");
                        if let Some(part) = part {
                            designator_of_part.insert(part, designator.clone());
                        }
                        designator
                    }
                }
            } else {
                sym.reference.clone()
            };
            out.push(AnnotatePreviewEntry {
                sheet: title.clone(),
                current: sym.reference.clone(),
                proposed,
                uuid: sym.uuid,
            });
        }
    }
    out
}

/// Content fingerprint of everything the preview depends on in memory: the
/// tab list and, per open document, each symbol's identity, designator,
/// placement order inputs and child-sheet references. Changes on any edit,
/// undo/redo, open or close, whichever path made it.
fn fingerprint(state: &crate::app::state::DocumentState) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    state.active_path.hash(&mut h);
    // The declared pages decide which sheets the load covers.
    crate::app::project_sheets::declared_pages(state).hash(&mut h);
    for tab in &state.tabs {
        (&tab.path, &tab.title).hash(&mut h);
    }
    let mut paths: Vec<&PathBuf> = state.engines.keys().collect();
    paths.sort();
    for path in paths {
        path.hash(&mut h);
        let doc = state.engines[path].document();
        for sym in &doc.symbols {
            (
                sym.uuid,
                &sym.lib_id,
                &sym.reference,
                &sym.value,
                sym.unit,
                sym.is_power,
                sym.position.x.to_bits(),
                sym.position.y.to_bits(),
            )
                .hash(&mut h);
        }
        for child in &doc.child_sheets {
            child.filename.hash(&mut h);
        }
    }
    h.finish()
}

impl Signex {
    /// Symbols the Annotate dialog lists - what All On / All Off act on.
    pub(crate) fn annotate_preview_uuids(&self) -> Vec<uuid::Uuid> {
        self.ui_state
            .annotate_preview
            .entries
            .iter()
            .map(|e| e.uuid)
            .collect()
    }

    /// Re-derive the rows from the cached sheets and the current lock set.
    /// CPU only; called when locks change or a load lands.
    pub(crate) fn recompute_annotate_entries(&mut self) {
        let cache = &mut self.ui_state.annotate_preview;
        cache.entries = compute_entries(
            &cache.sheets,
            &self.ui_state.annotate_locked,
            self.ui_state.annotate_order,
        );
    }

    /// Forget the cached sheets so the next refresh reloads them (dialog
    /// opened, or the action just rewrote sheets - possibly on disk).
    ///
    /// The rows go too: All Off / lock-all act on the listed uuids, so stale
    /// rows would leave symbols placed since the last load unlocked.
    pub(crate) fn invalidate_annotate_preview(&mut self) {
        let cache = &mut self.ui_state.annotate_preview;
        cache.fingerprint = None;
        cache.sheets = Arc::default();
        cache.entries.clear();
    }

    /// The dialog is open and its rows are being reloaded - lock-all and Run
    /// must wait, or they act on a list that no longer matches the project.
    pub(crate) fn annotate_preview_is_loading(&self) -> bool {
        self.ui_state.annotate_dialog_open && self.ui_state.annotate_preview.loading
    }

    /// Capture the load's inputs from live state (memory only, no IO).
    fn preview_inputs(&self) -> AnnotatePreviewInputs {
        let ds = &self.document_state;
        AnnotatePreviewInputs {
            open: project_sheets::open_sheets_snapshot(ds),
            pages: project_sheets::declared_pages(ds),
            root: ds
                .active_path
                .clone()
                .map(|active| project_sheets::resolve_netlist_root(ds).unwrap_or(active)),
            fold: crate::app::state::project_case::fold_of(ds.active_document_project()),
            active_path: ds.active_path.clone(),
            active_doc: ds.active_engine().map(|e| e.document().clone()),
            tab_titles: ds
                .tabs
                .iter()
                .map(|t| (t.path.clone(), t.title.clone()))
                .collect(),
        }
    }

    /// If the dialog is open and the cache no longer matches the documents,
    /// claim a new generation and return the inputs for a reload.
    fn begin_annotate_preview_load(&mut self) -> Option<(u64, AnnotatePreviewInputs)> {
        if !self.ui_state.annotate_dialog_open {
            return None;
        }
        let fp = fingerprint(&self.document_state);
        if self.ui_state.annotate_preview.fingerprint == Some(fp) {
            return None;
        }
        let inputs = self.preview_inputs();
        let cache = &mut self.ui_state.annotate_preview;
        cache.generation += 1;
        cache.fingerprint = Some(fp);
        cache.loading = true;
        cache.load_failed = false;
        Some((cache.generation, inputs))
    }

    /// Reload the preview's sheets off the UI thread when they are stale.
    pub(crate) fn refresh_annotate_preview(&mut self) -> Task<Message> {
        let Some((generation, inputs)) = self.begin_annotate_preview_load() else {
            return Task::none();
        };
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || load_preview_sheets(inputs))
                    .await
                    .map_err(|e| format!("Annotate preview worker failed: {e}"))
            },
            move |result| {
                Message::Annotate(AnnotateMsg::PreviewLoaded {
                    generation,
                    result: Arc::new(result),
                })
            },
        )
    }

    /// Apply a finished load - unless a newer one superseded it.
    pub(crate) fn handle_annotate_preview_loaded(
        &mut self,
        generation: u64,
        result: &Result<AnnotatePreviewLoad, String>,
    ) {
        if generation != self.ui_state.annotate_preview.generation {
            return;
        }
        self.ui_state.annotate_preview.loading = false;
        match result {
            Ok(load) => {
                for warning in &load.warnings {
                    crate::diagnostics::log_warning(warning);
                }
                self.ui_state.annotate_preview.sheets = Arc::new(load.sheets.clone());
                self.recompute_annotate_entries();
            }
            Err(why) => {
                crate::diagnostics::log_warning(format!("Annotate preview failed: {why}"));
                // Not remembered as loaded: the next refresh retries.
                let cache = &mut self.ui_state.annotate_preview;
                cache.fingerprint = None;
                cache.load_failed = true;
                cache.sheets = Arc::default();
                cache.entries.clear();
            }
        }
    }

    /// Synchronous load + derive, for tests.
    #[cfg(test)]
    pub(super) fn preview_project_annotations(&self) -> Vec<AnnotatePreviewEntry> {
        let load = load_preview_sheets(self.preview_inputs());
        compute_entries(
            &load.sheets,
            &self.ui_state.annotate_locked,
            self.ui_state.annotate_order,
        )
    }
}

#[cfg(test)]
mod tests;
