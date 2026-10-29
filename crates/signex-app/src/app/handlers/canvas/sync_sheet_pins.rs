//! "Sync Sheet Pins" — reconcile the selected sheet symbol's pins against the
//! hierarchical / global labels of the child schematic it references (#359).
//!
//! The port model and the reconcile semantics live in `signex-engine`
//! (`ports_for_pin_sync`, `Command::ReconcileChildSheetPins`); this file only
//! resolves and loads the child, reports what could not be done, and sends the
//! command through the mutation gateway so undo and netlist invalidation work
//! like any other edit.

use crate::app::mutation_gateway::CommandOutcome;

/// One child file validated and ready to reconcile, with the state needed to
/// report its result after the batch runs.
struct PendingFileSync {
    filename: String,
    ports: Vec<signex_engine::SheetPort>,
    blank_labels: usize,
    before: Vec<String>,
    names_before: Vec<String>,
}

impl crate::app::Signex {
    /// Run Sync Sheet Pins for every distinct child file referenced by the
    /// selected sheet symbols, reporting each file separately. Every outcome
    /// that is not a successful change is reported to the Messages panel.
    pub(crate) fn sync_selected_sheet_pins(&mut self) {
        let filenames = self.selected_child_sheet_filenames();
        if filenames.is_empty() {
            crate::diagnostics::log_warning("Sync Sheet Pins: no sheet symbol is selected");
            return;
        }
        // Load and validate every file first, then apply all reconciles as
        // ONE batch so a single Ctrl+Z undoes the whole action (#533).
        let mut syncs: Vec<PendingFileSync> = Vec::new();
        for filename in filenames {
            if let Some(sync) = self.prepare_child_file_sync(&filename) {
                syncs.push(sync);
            }
        }
        if syncs.is_empty() {
            return;
        }
        let commands = syncs
            .iter()
            .map(|sync| signex_engine::Command::ReconcileChildSheetPins {
                child_filename: sync.filename.clone(),
                ports: sync.ports.clone(),
            })
            .collect();
        let (outcome, reason) = self.apply_engine_commands_reporting(commands, true, true);
        if outcome == CommandOutcome::Failed {
            let reason = reason.unwrap_or_else(|| "the engine refused the batch".to_string());
            for sync in &syncs {
                crate::diagnostics::log_warning(format!(
                    "Sync Sheet Pins: could not update pins for '{}': {reason}",
                    sync.filename
                ));
            }
            return;
        }
        for sync in &syncs {
            self.report_file_sync(sync);
        }
    }

    /// Resolve and load `filename`'s child schematic and derive its ports.
    /// Reports and returns `None` when the file cannot be used.
    fn prepare_child_file_sync(&self, filename: &str) -> Option<PendingFileSync> {
        let Some(path) = self.resolve_child_sheet_path(filename) else {
            crate::diagnostics::log_warning(format!(
                "Sync Sheet Pins: sheet symbol references no file ('{filename}')"
            ));
            return None;
        };
        let child = match crate::app::project_sheets::load_sheet(&self.document_state, &path) {
            Ok(Some(child)) => child,
            Ok(None) => {
                crate::diagnostics::log_warning(format!(
                    "Sync Sheet Pins: child schematic not found: {}",
                    path.display()
                ));
                return None;
            }
            Err(why) => {
                crate::diagnostics::log_warning(format!(
                    "Sync Sheet Pins: child schematic {} {why}",
                    path.display()
                ));
                return None;
            }
        };
        let Some(parent) = self.document_state.active_engine().map(|e| e.document()) else {
            crate::diagnostics::log_warning("Sync Sheet Pins: no active schematic to sync");
            return None;
        };
        Some(PendingFileSync {
            filename: filename.to_string(),
            ports: signex_engine::ports_for_pin_sync(&child, parent, filename),
            blank_labels: signex_engine::blank_port_label_count(&child),
            before: self.pin_snapshot_for(filename),
            names_before: self.pin_names_for(filename),
        })
    }

    /// Report what the batch did for one file, comparing its pins before and
    /// after so each file is reported on its own.
    fn report_file_sync(&self, sync: &PendingFileSync) {
        let filename = sync.filename.as_str();
        let changed = self.pin_snapshot_for(filename) != sync.before;
        self.report_unplaced_ports(filename, &sync.ports);
        if sync.blank_labels > 0 {
            crate::diagnostics::log_warning(format!(
                "Sync Sheet Pins: '{filename}' has {} Hierarchical or Global \
                 label(s) with blank text; they name no port and were ignored",
                sync.blank_labels
            ));
        }
        // A child with no ports must still be reconciled: that is what drops
        // the auto-generated pins its deleted labels left behind.
        if sync.ports.is_empty() {
            crate::diagnostics::log_warning(format!(
                "Sync Sheet Pins: '{filename}' has no hierarchical or global labels, \
                 so it exposes no ports{}",
                if changed {
                    "; its auto-generated sheet pins were removed"
                } else {
                    ""
                }
            ));
        } else if changed {
            crate::diagnostics::log_info(format!(
                "Sync Sheet Pins: updated pins for '{filename}'{}",
                pin_change_summary(&sync.names_before, &self.pin_names_for(filename))
            ));
        } else {
            crate::diagnostics::log_info(format!(
                "Sync Sheet Pins: pins for '{filename}' already match its labels"
            ));
        }
    }

    /// Full state of every sheet pin on the active sheet's symbols of
    /// `filename`, for before/after comparison.
    fn pin_snapshot_for(&self, filename: &str) -> Vec<String> {
        let Some(engine) = self.document_state.active_engine() else {
            return Vec::new();
        };
        engine
            .document()
            .child_sheets
            .iter()
            .filter(|symbol| symbol.filename == filename)
            .flat_map(|symbol| symbol.pins.iter().map(|pin| format!("{pin:?}")))
            .collect()
    }

    /// The engine leaves a port unplaced when the symbol's edge has no free
    /// slot (stacking pins on one point would short their nets).
    fn report_unplaced_ports(&self, filename: &str, ports: &[signex_engine::SheetPort]) {
        let Some(parent) = self.document_state.active_engine().map(|e| e.document()) else {
            return;
        };
        for (symbol, names) in signex_engine::unplaced_sheet_ports(parent, filename, ports) {
            crate::diagnostics::log_warning(format!(
                "Sync Sheet Pins: no free slot on sheet symbol '{symbol}' ('{filename}') for {}; enlarge the sheet symbol or move the wires on its edge, then sync again",
                names.join(", ")
            ));
        }
    }

    /// Every sheet pin name on the active sheet's symbols of `filename`.
    fn pin_names_for(&self, filename: &str) -> Vec<String> {
        let Some(engine) = self.document_state.active_engine() else {
            return Vec::new();
        };
        engine
            .document()
            .child_sheets
            .iter()
            .filter(|symbol| symbol.filename == filename)
            .flat_map(|symbol| symbol.pins.iter().map(|pin| pin.name.clone()))
            .collect()
    }
}

/// `: added A, B; removed C` for the pins that differ, or empty when only a
/// pin's non-name fields changed.
fn pin_change_summary(before: &[String], after: &[String]) -> String {
    let list = |names: Vec<&String>| {
        names
            .iter()
            .map(|n| n.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let added = list(after.iter().filter(|n| !before.contains(n)).collect());
    let removed = list(before.iter().filter(|n| !after.contains(n)).collect());
    let mut parts = Vec::new();
    if !added.is_empty() {
        parts.push(format!("added {added}"));
    }
    if !removed.is_empty() {
        parts.push(format!("removed {removed}"));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(": {}", parts.join("; "))
    }
}
