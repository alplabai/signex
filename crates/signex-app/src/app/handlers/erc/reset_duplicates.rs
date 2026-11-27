//! Reset Duplicate Designators: the numbering half (#859). The sheets arrive
//! already read by `annotate_job`; every changed sheet is one undo group, and
//! the unopened ones are parked unsaved (#2760).

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use iced::Task;
use signex_types::schematic::{InstancePath, SchematicSheet};

use super::super::super::*;
use super::annotate::read_only_sheets;
use super::annotate_placements::{WalkDocs, multi_placed_files, project_base_dir};
use crate::app::UndoGroupKind;
use crate::app::project_sheets::ProjectSheetSet;
use crate::app::sheet_write::{SheetWrite, SheetWriteRefusal};

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

/// Why a grouped reset changed nothing, for the refusal notice.
fn refusal_reason(refusal: &SheetWriteRefusal) -> String {
    let (path, why) = match refusal {
        SheetWriteRefusal::Busy(path) => (path, "is being saved or moved"),
        SheetWriteRefusal::ReadOnly(path) => (path, "is read-only"),
        SheetWriteRefusal::NotLoaded(path) => (path, "could not be loaded"),
        SheetWriteRefusal::NotChanged(path) => (path, "refused the change"),
    };
    format!("{} {why}. Nothing was changed.", path.display())
}

impl Signex {
    /// Reset the duplicates of the project `set`, in memory, as one undo group
    /// over every changed sheet. A designator is a duplicate when more than
    /// one PART holds it across the whole project; units of one multi-unit
    /// part sharing a designator are not (#691).
    pub(super) fn apply_reset_duplicates(&mut self, set: ProjectSheetSet) -> Task<Message> {
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
        // A sheet it cannot reach may hold the other half of a duplicate, and
        // one undo could not restore a project only partly reset.
        let mut paths: Vec<PathBuf> = set.sheets.keys().cloned().collect();
        paths.sort();
        if self.refuse_reset_all_that_skips(LABEL, &set, &paths) {
            return Task::none();
        }
        if duplicates.is_empty() {
            crate::diagnostics::log_info(format!("{LABEL}: no duplicates found"));
            self.report_read_only_skipped(LABEL, &read_only);
            return Task::none();
        }
        let edits = self.reset_targets(&set, &read_only, &duplicates, &placed);
        let result = if edits.is_empty() {
            Ok(0)
        } else {
            let write = SheetWrite {
                label: LABEL,
                outcome: "reset",
                kind: UndoGroupKind::Annotate,
                loaded: &set.sheets,
            };
            self.write_sheets_as_group(&write, edits, |document, places| {
                let mut sheet = document.clone();
                reset_in(&mut sheet, &duplicates, places);
                vec![signex_engine::Command::ReplaceDocument {
                    document: Box::new(sheet),
                }]
            })
        };
        match result {
            Ok(sheets) => crate::diagnostics::log_info(format!(
                "{LABEL}: {} duplicate reference(s) found project-wide; reset on {sheets} \
                 sheet(s) in memory, nothing is written until you save",
                duplicates.len(),
            )),
            Err(refusal) => self.refuse_run(LABEL, &refusal_reason(&refusal)),
        }
        self.report_read_only_skipped(LABEL, &read_only);
        Task::none()
    }

    /// The writable sheets the reset changes, each with its placements.
    fn reset_targets(
        &self,
        set: &ProjectSheetSet,
        read_only: &BTreeSet<PathBuf>,
        duplicates: &HashSet<String>,
        placed: &Placed,
    ) -> BTreeMap<PathBuf, Vec<InstancePath>> {
        let mut edits = BTreeMap::new();
        for (path, sheet) in &set.sheets {
            if read_only.contains(path) {
                continue;
            }
            let places = placements_of(placed, path);
            if reset_in(&mut sheet.clone(), duplicates, places) {
                edits.insert(path.clone(), places.to_vec());
            }
        }
        edits
    }
}
