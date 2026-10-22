//! "Sync Sheet Pins" — reconcile the selected sheet symbol's pins against the
//! hierarchical / global labels of the child schematic it references (#359).
//!
//! The port model and the reconcile semantics live in `signex-engine`
//! (`ports_for_pin_sync`, `Command::ReconcileChildSheetPins`); this file only
//! resolves and loads the child, reports what could not be done, and sends the
//! command through the mutation gateway so undo and netlist invalidation work
//! like any other edit.

impl crate::app::Signex {
    /// Run Sync Sheet Pins for the selected sheet symbol. Every outcome that is
    /// not a successful change is reported to the Messages panel.
    pub(crate) fn sync_selected_sheet_pins(&mut self) {
        let Some(filename) = self.selected_child_sheet_filename() else {
            crate::diagnostics::log_warning("Sync Sheet Pins: no sheet symbol is selected");
            return;
        };
        let Some(path) = self.resolve_child_sheet_path(&filename) else {
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
            return;
        };
        let ports = signex_engine::ports_for_pin_sync(&child, parent, &filename);
        if ports.is_empty() {
            crate::diagnostics::log_warning(format!(
                "Sync Sheet Pins: '{filename}' has no hierarchical or global labels, \
                 so no sheet pins were changed"
            ));
            return;
        }
        let changed = self.apply_engine_command(
            signex_engine::Command::ReconcileChildSheetPins {
                child_filename: filename.clone(),
                ports,
            },
            true,
            true,
        );
        if changed {
            crate::diagnostics::log_info(format!("Sync Sheet Pins: updated pins for '{filename}'"));
        } else {
            crate::diagnostics::log_info(format!(
                "Sync Sheet Pins: pins for '{filename}' already match its labels"
            ));
        }
    }
}
