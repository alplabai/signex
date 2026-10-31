//! Findings against the document as it is now (#1768).
//!
//! The per-sheet cache keeps what the last run found, untouched; the canvas
//! markers, the panel rows, F8 navigation and the report all read it through
//! [`Signex::findings_present_in`], which leaves out a finding whose object is
//! no longer in the sheet. Undo therefore brings a finding back. An edit also
//! marks the results stale, since a moved or renewed object cannot be told
//! from a changed result without a new run.

use std::collections::HashSet;
use std::path::Path;

use signex_erc::{LocationSpace, Violation};
use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind};
use uuid::Uuid;

use crate::app::Signex;

/// The ids of a sheet's objects, built once per check.
struct SheetObjects {
    ids: HashSet<Uuid>,
    line_ends: Vec<Point>,
}

impl SheetObjects {
    fn of(sheet: &SchematicSheet) -> Self {
        let mut ids = HashSet::new();
        ids.extend(sheet.symbols.iter().map(|o| o.uuid));
        ids.extend(sheet.wires.iter().map(|o| o.uuid));
        ids.extend(sheet.buses.iter().map(|o| o.uuid));
        ids.extend(sheet.bus_entries.iter().map(|o| o.uuid));
        ids.extend(sheet.labels.iter().map(|o| o.uuid));
        ids.extend(sheet.junctions.iter().map(|o| o.uuid));
        ids.extend(sheet.no_connects.iter().map(|o| o.uuid));
        ids.extend(sheet.no_erc_directives.iter().map(|o| o.uuid));
        ids.extend(sheet.text_notes.iter().map(|o| o.uuid));
        ids.extend(sheet.child_sheets.iter().map(|o| o.uuid));
        let line_ends = sheet
            .wires
            .iter()
            .flat_map(|w| [w.start, w.end])
            .chain(sheet.buses.iter().flat_map(|b| [b.start, b.end]))
            .collect();
        Self { ids, line_ends }
    }

    /// Whether `item` is still there. A kind this check does not index counts
    /// as present, so a finding is never hidden by a gap here. A wire or bus
    /// that was split or merged gets new ids, so one with an end at `at` stands in.
    fn holds(&self, item: &SelectedItem, at: Point) -> bool {
        use SelectedKind::*;
        match item.kind {
            Symbol | SymbolRefField | SymbolValField | BusEntry | Junction | NoConnect
            | NoErcDirective | Label | TextNote | ChildSheet => self.ids.contains(&item.uuid),
            Wire | Bus => {
                self.ids.contains(&item.uuid)
                    || self
                        .line_ends
                        .iter()
                        .any(|p| signex_net::pt_key(p) == signex_net::pt_key(&at))
            }
            SheetPin | Drawing => true,
        }
    }
}

impl Signex {
    /// For each finding of `list` (filed under `path`), whether its objects are
    /// still in that sheet. A sheet that is not open, or a finding in another
    /// sheet's space, is present: there is nothing to compare it with.
    pub(crate) fn findings_present_in(&self, path: &Path, list: &[Violation]) -> Vec<bool> {
        let Some(engine) = self.document_state.engines.get(path) else {
            return vec![true; list.len()];
        };
        let objects = SheetObjects::of(engine.document());
        list.iter()
            .map(|v| {
                !matches!(
                    v.location_space,
                    LocationSpace::ThisSheet | LocationSpace::Net
                ) || [v.primary, v.peer]
                    .iter()
                    .flatten()
                    .all(|item| objects.holds(item, v.location))
            })
            .collect()
    }

    /// Any document change makes the last run's results possibly stale, a run
    /// still in flight included (it read the sheets before the change).
    pub(crate) fn mark_erc_results_stale(&mut self) {
        if self.ui_state.erc_running || !self.ui_state.erc_violations_by_path.is_empty() {
            self.ui_state.erc_results_stale = true;
        }
    }

    /// After an edit: the results are stale, and the active sheet's markers and
    /// list follow the document again.
    pub(crate) fn refresh_erc_after_edit(&mut self) {
        self.mark_erc_results_stale();
        if let Some(path) = self.document_state.active_path.clone() {
            self.refresh_active_erc_from_cache(Some(&path));
        }
    }
}
