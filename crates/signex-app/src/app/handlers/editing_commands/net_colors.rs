//! Net colours are an edit of the sheet, run on its engine like any other
//! (#661): one `Command::SetNetColors` per user action, so the colours share
//! the sheet's undo timeline, its depth limit and its save point.

use super::super::super::*;
use crate::app::mutation_gateway::CommandOutcome;
use signex_types::theme::Color;

/// What [`Signex::clear_net_colors`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ColorsCleared {
    /// Sheets whose colours were cleared, one undo step each.
    pub sheets: usize,
    /// Read-only sheets that kept their colours (#1402).
    pub kept_read_only: usize,
}

impl Signex {
    /// Colour `elements` (wire and junction ids) of the active sheet with
    /// `color` in one undo step; an alpha of 0 clears them instead. A read-only
    /// sheet refuses and says so in Messages (#1402). Painting a net the colour
    /// it has is no edit: the outcome says `Unchanged`. The refusal text rides
    /// along for a caller that must show why.
    pub(crate) fn commit_net_colors(
        &mut self,
        elements: &[uuid::Uuid],
        color: Color,
    ) -> (CommandOutcome, Option<String>) {
        let Some(engine) = self.document_state.active_engine() else {
            return (CommandOutcome::Failed, None);
        };
        let mut colors = engine.document().net_colors.clone();
        for id in elements {
            if color.a == 0 {
                colors.remove(id);
            } else {
                colors.insert(*id, color);
            }
        }
        self.end_parameter_edit_runs();
        self.apply_engine_command_reporting(
            signex_engine::Command::SetNetColors { colors },
            false,
            false,
        )
    }

    /// The Active Bar's "Clear All Net Colors" and the command of the same id
    /// (`only`: the one sheet its `path` operand names): clear, disarm the
    /// pen and say what happened in Messages.
    pub(crate) fn run_clear_all_net_colors(
        &mut self,
        only: Option<&std::path::Path>,
    ) -> ColorsCleared {
        // #1005: each sheet keeps its own undo step for what this removes.
        let cleared = self.clear_net_colors(only);
        self.ui_state.pending_net_color = None;
        self.interaction_state.active_canvas_mut().pending_net_color = None;
        self.interaction_state
            .active_canvas_mut()
            .clear_content_cache();
        let kept = cleared.kept_read_only;
        if kept == 0 {
            crate::diagnostics::log_info("Cleared all net-color overrides");
        } else {
            crate::diagnostics::log_info(format!(
                "Cleared net-color overrides, except on {kept} read-only sheet(s)"
            ));
        }
        cleared
    }

    /// Clear the net colours of `only`, or of every open sheet, as one undo
    /// step per sheet, so Ctrl+Z in a sheet restores that sheet's colours
    /// only. A read-only sheet keeps its colours and is counted (#1402).
    pub(crate) fn clear_net_colors(&mut self, only: Option<&std::path::Path>) -> ColorsCleared {
        self.end_parameter_edit_runs();
        let mut paths: Vec<std::path::PathBuf> = self
            .document_state
            .engines
            .iter()
            .filter(|(path, _)| only.is_none_or(|only| only == path.as_path()))
            .filter(|(_, engine)| has_painted_element(engine.document()))
            .map(|(path, _)| path.clone())
            .collect();
        paths.sort();
        let mut cleared = ColorsCleared {
            sheets: 0,
            kept_read_only: 0,
        };
        for path in paths {
            if self.document_state.engines[&path].document().is_read_only() {
                cleared.kept_read_only += 1;
            } else if self.document_state.active_path.as_deref() == Some(path.as_path()) {
                let command = signex_engine::Command::SetNetColors {
                    colors: Default::default(),
                };
                cleared.sheets += usize::from(self.apply_engine_command(command, false, false));
            } else {
                cleared.sheets += usize::from(self.clear_colors_of_parked_sheet(&path));
            }
        }
        if cleared.kept_read_only > 0 {
            crate::diagnostics::log_warning(format!(
                "Not changed: net colours on {} read-only sheet(s) written by a newer Signex were kept.",
                cleared.kept_read_only
            ));
        }
        cleared
    }

    /// [`Self::clear_net_colors`] on an open sheet that is not the active
    /// one: the same engine edit, with the dirty mark taken from its history
    /// and the windows showing it redrawn. Whether the colours changed.
    fn clear_colors_of_parked_sheet(&mut self, path: &std::path::Path) -> bool {
        let Some(engine) = self.document_state.engines.get_mut(path) else {
            return false;
        };
        let result = engine.execute(signex_engine::Command::SetNetColors {
            colors: Default::default(),
        });
        match result {
            Ok(done) if done.changed => {
                self.document_state.sync_history_dirty(path);
                self.refresh_slots_from_engine(
                    path,
                    false,
                    crate::schematic_runtime::RenderInvalidation::NET_COLORS,
                    false,
                );
                self.refresh_panel_ctx();
                true
            }
            Ok(_) => false,
            Err(error) => {
                crate::diagnostics::log_warning(format!(
                    "Net colours of {} were not cleared: {error}",
                    path.display()
                ));
                false
            }
        }
    }
}

/// Does the sheet hold a colour for a wire or junction it has? (A colour kept
/// for a deleted element is not on screen, so clearing it is no visible edit.)
fn has_painted_element(sheet: &signex_types::schematic::SchematicSheet) -> bool {
    sheet
        .wires
        .iter()
        .map(|w| w.uuid)
        .chain(sheet.junctions.iter().map(|j| j.uuid))
        .any(|id| sheet.net_colors.contains_key(&id))
}
