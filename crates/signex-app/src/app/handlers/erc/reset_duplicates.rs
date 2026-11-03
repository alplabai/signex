//! Reset Duplicate Designators: the numbering half (#859). The sheets arrive
//! already read by `annotate_job`; the unopened ones leave as encoded bytes for
//! its write worker.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use iced::Task;
use signex_types::schematic::{InstancePath, SchematicSheet};

use super::super::super::*;
use super::annotate::{encode_unopened, read_only_sheets};
use super::annotate_job::{PassApplied, PendingWrite};
use super::annotate_placements::{WalkDocs, multi_placed_files, project_base_dir};
use crate::app::project_sheets::ProjectSheetSet;

const LABEL: &str = "Reset Duplicate Designators";

/// Sheet files placed more than once, with their placements (#1835).
type Placed = HashMap<PathBuf, Vec<InstancePath>>;

fn placements_of<'a>(placed: &'a Placed, path: &Path) -> &'a [InstancePath] {
    placed.get(path).map_or(&[], Vec::as_slice)
}

/// For each symbol whose reference is in `dupes`, reset it to `{prefix}?`.
/// A sheet placed more than once resets per placement: a duplicate designator
/// at a placement clears that placement's entry. Returns whether anything in
/// the sheet changed.
fn reset_in(sheet: &mut SchematicSheet, dupes: &HashSet<String>, places: &[InstancePath]) -> bool {
    if !places.is_empty() {
        return reset_placed(sheet, dupes, places);
    }
    let mut changed = false;
    for sym in sheet.symbols.iter_mut() {
        if sym.is_power || sym.reference.starts_with('#') {
            continue;
        }
        if dupes.contains(sym.reference.trim()) {
            let prefix = signex_types::designator::alphabetic_prefix(&sym.reference);
            if !prefix.is_empty() {
                sym.reference = format!("{prefix}?");
                changed = true;
            }
        }
    }
    changed
}

/// [`reset_in`] for a file placed at `places` (root first): each placement's
/// designator is judged where it resolves, the survivors keep their value as an
/// explicit entry, and `reference` follows the first placement.
fn reset_placed(
    sheet: &mut SchematicSheet,
    dupes: &HashSet<String>,
    places: &[InstancePath],
) -> bool {
    let mut changed = false;
    for sym in sheet.symbols.iter_mut() {
        if sym.is_power || sym.reference.starts_with('#') {
            continue;
        }
        let finals: Vec<String> = places
            .iter()
            .map(|place| {
                let designator = sym.designator_in(place);
                let prefix = signex_types::designator::alphabetic_prefix(designator);
                if dupes.contains(designator.trim()) && !prefix.is_empty() {
                    format!("{prefix}?")
                } else {
                    designator.to_string()
                }
            })
            .collect();
        let unchanged = places
            .iter()
            .zip(&finals)
            .all(|(place, last)| sym.designator_in(place) == last);
        if unchanged {
            continue;
        }
        sym.write_placement_designators(places, &finals);
        changed = true;
    }
    changed
}

impl Signex {
    /// Reset the duplicates of the project `set`, in memory. Open sheets are
    /// reset through their engines so the edit is undoable; each unopened
    /// sheet comes back as bytes to write. A designator is a duplicate when
    /// more than one PART holds it across the whole project; units of one
    /// multi-unit part sharing a designator are not (#691).
    pub(super) fn apply_reset_duplicates(&mut self, set: ProjectSheetSet) -> PassApplied {
        // The project, not "every open engine plus the declared pages": a loose
        // tab from another project must not collide with this project's R1,
        // and a hierarchical child missing from `data.sheets` must count (#406).
        // A file placed twice counts once per placement, each under the
        // designators it resolves to there (#1835).
        let placed = multi_placed_files(
            &set,
            project_base_dir(&self.document_state, &set).as_deref(),
        );
        let duplicates: HashSet<String> = {
            let mut paths: Vec<PathBuf> = set.sheets.keys().cloned().collect();
            paths.sort();
            let walk = WalkDocs::new(
                &paths,
                |path| set.sheets.get(path).map_or(&[], |s| s.symbols.as_slice()),
                &placed,
            );
            signex_types::schematic::duplicate_designators(
                walk.docs.iter().flat_map(|doc| doc.iter()),
            )
        };
        let read_only = read_only_sheets(&self.document_state, &set);
        for (path, why) in &set.unreadable {
            crate::diagnostics::log_info(format!("{LABEL}: sheet '{}' {why}", path.display()));
        }
        let (project_paths, mut unopened) = self.split_reset_targets(set, &read_only);
        // Deterministic write order: `sheets` is a `HashMap`.
        unopened.sort_by(|a, b| a.0.cmp(&b.0));
        if duplicates.is_empty() {
            crate::diagnostics::log_info(format!("{LABEL}: no duplicates found"));
            self.report_read_only_skipped(LABEL, &read_only);
            return PassApplied::without_writes(Task::none());
        }
        let resets = self.reset_active_sheet(&duplicates, &placed)
            + self.reset_cached_sheets(&duplicates, &project_paths, &placed);
        let writes = reset_unopened(unopened, &duplicates, &placed);
        if resets > 0 {
            // The active sheet's repaint is the gateway's job; the cached tabs
            // need their dirty dots refreshed.
            self.refresh_panel_ctx();
        }
        crate::diagnostics::log_info(format!(
            "{LABEL}: {} duplicate reference(s) found project-wide; reset on {resets} open \
             sheet(s), {} unopened sheet(s) to write",
            duplicates.len(),
            writes.len(),
        ));
        self.report_read_only_skipped(LABEL, &read_only);
        PassApplied {
            writes,
            task: Task::none(),
            undo: None,
        }
    }

    /// Split the writable sheets into "has an engine" (their paths) and
    /// "unopened" (path and parsed sheet). A sheet whose file a restore, a
    /// read or a save still owns is left out.
    fn split_reset_targets(
        &mut self,
        set: ProjectSheetSet,
        read_only: &std::collections::BTreeSet<PathBuf>,
    ) -> (HashSet<PathBuf>, Vec<(PathBuf, SchematicSheet)>) {
        let mut project_paths = HashSet::new();
        let mut unopened = Vec::new();
        for (path, sheet) in set.sheets {
            if read_only.contains(&path) {
                continue;
            }
            project_paths.insert(path.clone());
            if !self.document_state.engines.contains_key(&path) {
                if self.unopened_write_blocked(LABEL, &path) {
                    continue;
                }
                unopened.push((path, sheet));
            }
        }
        (project_paths, unopened)
    }

    /// The active engine goes through ReplaceDocument so undo records the
    /// snapshot, and through the mutation gateway so the sheet is marked dirty
    /// (#585). The clone is taken first and the engine borrow released,
    /// because the gateway needs `&mut self`. Returns the sheets reset (0/1).
    fn reset_active_sheet(&mut self, duplicates: &HashSet<String>, placed: &Placed) -> usize {
        let places = self
            .document_state
            .active_path
            .as_deref()
            .map_or(&[][..], |path| placements_of(placed, path));
        let reset_sheet = self.document_state.active_engine().and_then(|engine| {
            if engine.document().is_read_only() {
                return None;
            }
            let mut sheet = engine.document().clone();
            reset_in(&mut sheet, duplicates, places).then_some(sheet)
        });
        let Some(document) = reset_sheet else {
            return 0;
        };
        self.apply_engine_command(
            signex_engine::Command::ReplaceDocument { document },
            true,
            false,
        );
        1
    }

    /// Every other live engine of this project: a cached tab, or a dirty sheet
    /// whose tab was closed and whose engine is parked (#669). The project set
    /// counted parked engines, so they must be reset too, in memory: never
    /// written behind the user's back. Driven by `engines`, not `tabs`; a loose
    /// engine from another project was never counted, so it is not reset.
    fn reset_cached_sheets(
        &mut self,
        duplicates: &HashSet<String>,
        project_paths: &HashSet<PathBuf>,
        placed: &Placed,
    ) -> usize {
        let active_path = self.document_state.active_path.clone();
        let mut cached_paths: Vec<PathBuf> = self
            .document_state
            .engines
            .keys()
            .filter(|path| {
                project_paths.contains(*path) && active_path.as_deref() != Some(path.as_path())
            })
            .cloned()
            .collect();
        cached_paths.sort();
        let mut resets = 0;
        for path in cached_paths {
            if self.reset_cached_engine(&path, duplicates, placements_of(placed, &path)) {
                self.document_state.mark_document_dirty(&path);
                resets += 1;
            }
        }
        resets
    }

    /// Not the gateway: this is a cached, non-active engine, so the caller
    /// does the dirty bookkeeping. The result is still read rather than
    /// discarded: a failure here would report the sheet as reset when it was
    /// not (#585).
    fn reset_cached_engine(
        &mut self,
        path: &PathBuf,
        duplicates: &HashSet<String>,
        places: &[InstancePath],
    ) -> bool {
        let Some(engine) = self.document_state.engines.get_mut(path) else {
            return false;
        };
        let mut sheet = engine.document().clone();
        if !reset_in(&mut sheet, duplicates, places) {
            return false;
        }
        match engine.execute(signex_engine::Command::ReplaceDocument { document: sheet }) {
            Ok(result) => result.changed,
            Err(error) => {
                crate::diagnostics::log_error(
                    &format!("{LABEL} failed for {}", path.display()),
                    &anyhow::Error::new(error),
                );
                false
            }
        }
    }
}

/// Unopened sheets: mutate the already-parsed copy and encode it for the
/// write. Not undoable from within Signex; the user would need to re-open the
/// sheet and Ctrl+Z manually, which is the same as before.
fn reset_unopened(
    unopened: Vec<(PathBuf, SchematicSheet)>,
    duplicates: &HashSet<String>,
    placed: &Placed,
) -> Vec<PendingWrite> {
    let mut writes = Vec::new();
    for (path, mut sheet) in unopened {
        if !reset_in(&mut sheet, duplicates, placements_of(placed, &path)) {
            continue;
        }
        crate::app::sheet_repairs::take_and_log(&path, &mut sheet);
        let engine = match signex_engine::Engine::new(sheet) {
            Ok(engine) => engine,
            Err(error) => {
                crate::diagnostics::log_error(
                    &format!("{LABEL}: engine construct failed for {}", path.display()),
                    &anyhow::Error::new(error),
                );
                continue;
            }
        };
        if let Some(bytes) = encode_unopened(LABEL, &path, &engine) {
            writes.push(PendingWrite { path, bytes });
        }
    }
    writes
}
