//! "Sync Sheet Pins" — reconcile the selected sheet symbol's pins against the
//! hierarchical / global labels of the child schematic it references (#359).
//!
//! The port model and the reconcile semantics live in `signex-engine`
//! (`ports_for_pin_sync`, `Command::ReconcileChildSheetPins`); this file only
//! resolves and loads the child, reports what could not be done, and sends the
//! command through the mutation gateway so undo and netlist invalidation work
//! like any other edit.

use crate::app::mutation_gateway::CommandOutcome;

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
        for filename in filenames {
            self.sync_child_file_pins(&filename);
        }
    }

    /// Reconcile every sheet symbol of the active sheet that references
    /// `filename` against that file's ports.
    fn sync_child_file_pins(&mut self, filename: &str) {
        let Some(path) = self.resolve_child_sheet_path(filename) else {
            crate::diagnostics::log_warning(format!(
                "Sync Sheet Pins: sheet symbol references no file ('{filename}')"
            ));
            return;
        };
        let child = match crate::app::project_sheets::load_sheet(&self.document_state, &path) {
            Ok(Some(child)) => child,
            Ok(None) => {
                crate::diagnostics::log_warning(format!(
                    "Sync Sheet Pins: child schematic not found: {}",
                    path.display()
                ));
                return;
            }
            Err(why) => {
                crate::diagnostics::log_warning(format!(
                    "Sync Sheet Pins: child schematic {} {why}",
                    path.display()
                ));
                return;
            }
        };
        let Some(parent) = self.document_state.active_engine().map(|e| e.document()) else {
            crate::diagnostics::log_warning("Sync Sheet Pins: no active schematic to sync");
            return;
        };
        let ports = signex_engine::ports_for_pin_sync(&child, parent, filename);
        // A child with no ports must still be reconciled: that is what drops
        // the auto-generated pins its deleted labels left behind.
        let exposes_no_ports = ports.is_empty();
        let reported_ports = ports.clone();
        let names_before = self.pin_names_for(filename);
        let outcome = self.apply_engine_command_checked(
            signex_engine::Command::ReconcileChildSheetPins {
                child_filename: filename.to_string(),
                ports,
            },
            true,
            true,
        );
        if outcome == CommandOutcome::Failed {
            crate::diagnostics::log_warning(format!(
                "Sync Sheet Pins: could not update pins for '{filename}'; see the error above"
            ));
            return;
        }
        let changed = outcome == CommandOutcome::Changed;
        self.report_unplaced_ports(filename, &reported_ports);
        let blank_labels = signex_engine::blank_port_label_count(&child);
        if blank_labels > 0 {
            crate::diagnostics::log_warning(format!(
                "Sync Sheet Pins: '{filename}' has {blank_labels} Hierarchical or Global \
                 label(s) with blank text; they name no port and were ignored"
            ));
        }
        if exposes_no_ports {
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
                pin_change_summary(&names_before, &self.pin_names_for(filename))
            ));
        } else {
            crate::diagnostics::log_info(format!(
                "Sync Sheet Pins: pins for '{filename}' already match its labels"
            ));
        }
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
