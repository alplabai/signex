//! What a command needs before it can act: the one table behind
//! [`super::is_enabled`] (the menu greys by it, a keystroke is dropped by it)
//! and behind the reason [`super::CommandOutcome::Disabled`] reports.

use crate::keymap::AppCommandId;

use super::CommandContext;

/// A precondition on the window's [`CommandContext`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Needs {
    /// An Active Bar tool that is a planned placeholder (#1249): never met.
    Planned,
    SaveTarget,
    SaveAsTarget,
    PrintTarget,
    Document,
    Undo,
    Redo,
    Selection,
    DrawingSelection,
    SelectedObject,
    Schematic,
    ParentSheet,
    DeleteTarget,
    VertexTarget,
    Sheets,
    ActiveSheet,
    Canvas,
    FitTarget,
    FootprintEditor,
    Pcb,
    Library,
    ProjectSheet,
    TileableWindows,
}

impl Needs {
    pub(crate) fn is_met(self, ctx: &CommandContext) -> bool {
        match self {
            Self::Planned => false,
            Self::SaveTarget => {
                ctx.document_is_schematic
                    || ctx.document_is_editor
                    || (ctx.document_has_file && ctx.project_dirty)
            }
            Self::SaveAsTarget => ctx.document_is_schematic || ctx.document_is_editor,
            Self::PrintTarget => ctx.document_is_schematic,
            Self::Document => ctx.has_document,
            Self::Undo => ctx.can_undo,
            Self::Redo => ctx.can_redo,
            Self::Selection => ctx.has_selection,
            // Arrange acts on drawings only; a selection of circuit elements
            // leaves it nothing to do (#1583).
            Self::DrawingSelection => ctx.has_schematic && ctx.has_drawing_selection,
            // E edits the selected object in the schematic and in the
            // footprint / symbol editors, whose selections `has_selection`
            // does not read, so a keystroke must not be dropped there.
            Self::SelectedObject => {
                ctx.has_selection || ctx.has_footprint_editor || ctx.has_symbol_editor
            }
            Self::Schematic => ctx.has_schematic,
            // Up Hierarchy opens the sheet whose symbol places the active
            // one; a top-level page has none (#1772).
            Self::ParentSheet => ctx.has_parent_sheet,
            // The footprint editor deletes its own selection, which is not
            // the schematic canvas's; with none it is a no-op.
            Self::DeleteTarget => ctx.has_selection || ctx.has_footprint_editor,
            Self::VertexTarget => ctx.has_selection || ctx.has_footprint_editor || ctx.drawing,
            // These assemble the project's sheets, not only the active tab's.
            Self::Sheets => ctx.has_schematic || ctx.has_sheets,
            // Annotate acts from the active document's path; with none open it
            // only reports that there is nothing to do.
            Self::ActiveSheet => ctx.has_active_path,
            // Zoom steps the main window's schematic or PCB canvas.
            Self::Canvas => ctx.has_schematic || ctx.has_pcb,
            Self::FitTarget => ctx.has_schematic || ctx.has_pcb || ctx.undocked_has_canvas,
            // The keystroke opens the footprint editor's grid picker and does
            // nothing elsewhere.
            Self::FootprintEditor => ctx.has_footprint_editor,
            Self::Pcb => ctx.has_pcb,
            // Library Editor focuses an open Library Browser tab, else opens the
            // first open library.
            Self::Library => ctx.has_open_library,
            // Sheet numbers are the project's; a loose sheet has none.
            Self::ProjectSheet => ctx.has_project_sheet,
            // Tile arranges the main window with the undocked document
            // windows; alone, or before the main window's screen is known,
            // it has nothing to do.
            Self::TileableWindows => ctx.can_tile_windows,
        }
    }

    /// Why the command has nothing to act on while this is unmet.
    pub(crate) fn reason(self) -> &'static str {
        match self {
            Self::Planned => "Planned, not available yet",
            Self::SaveTarget => {
                "Save needs a schematic or editor document, or a project with unsaved changes"
            }
            Self::SaveAsTarget => "Save As needs a schematic, symbol or footprint document",
            Self::PrintTarget => "Print needs an open schematic",
            Self::Document => "No document is open",
            Self::Undo => "Nothing to undo",
            Self::Redo => "Nothing to redo",
            Self::Selection => "Nothing is selected",
            Self::DrawingSelection => "No drawing is selected",
            Self::SelectedObject => "No object is selected",
            Self::Schematic => "No schematic is open",
            Self::ParentSheet => "The active sheet has no parent sheet",
            Self::DeleteTarget => "Nothing is selected to delete",
            Self::VertexTarget => "Nothing is selected and no wire is being drawn",
            Self::Sheets => "No schematic or project is open",
            Self::ActiveSheet => "No sheet is open",
            Self::Canvas => "No schematic or PCB is open",
            Self::FitTarget => "No schematic or PCB is open",
            Self::FootprintEditor => "No footprint editor is open",
            Self::Pcb => "No PCB is open",
            Self::Library => "No library is open",
            Self::ProjectSheet => "No sheet of a project is open",
            Self::TileableWindows => "No document is in its own window",
        }
    }
}

/// What `command` needs; `None` for a command with no gate, which is always
/// enabled.
pub(crate) fn needs(command: &AppCommandId) -> Option<Needs> {
    if super::enablement::is_planned_command(command) {
        return Some(Needs::Planned);
    }
    Some(match command.as_str() {
        "save_document" => Needs::SaveTarget,
        "save_document_as" => Needs::SaveAsTarget,
        "print" => Needs::PrintTarget,
        "close_active_document" | "close_all_documents" => Needs::Document,
        "undo" => Needs::Undo,
        "redo" => Needs::Redo,
        "cut" | "copy" | "duplicate" => Needs::Selection,
        "bring_drawings_to_front"
        | "send_drawings_to_back"
        | "bring_drawings_forward"
        | "send_drawings_backward" => Needs::DrawingSelection,
        "edit_object_properties" | "edit_selected_object_properties" => Needs::SelectedObject,
        // The Export menu row is gated on `has_schematic`; without an
        // engine the handler does nothing.
        "report_manager_bom" => Needs::Schematic,
        "leave_sheet" | "navigate_up_hierarchy" => Needs::ParentSheet,
        "delete_selection" => Needs::DeleteTarget,
        "remove_last_vertex" => Needs::VertexTarget,
        "paste"
        | "smart_paste"
        | "paste_special"
        | "paste_array"
        | "repeat_last_item"
        | "rubber_stamp_copy"
        | "select_all"
        | "place_wire"
        | "draw_wire"
        | "place_bus"
        | "draw_bus"
        | "place_net_label"
        | "place_local_net_label"
        | "place_junction"
        | "place_power_symbol"
        | "place_global_label"
        | "place_hierarchical_label"
        | "place_port"
        | "place_off_sheet_connector"
        | "place_sheet_entry"
        | "open_components_panel"
        | "place_symbol"
        | "measure_distance"
        | "break_wire"
        // The table lists the active sheet's symbols.
        | "open_parameter_manager"
        // Lists the parts of the active schematic's project.
        | "assign_footprints" => Needs::Schematic,
        "run_erc" | "open_erc_dialog" => Needs::Sheets,
        "run_drc" => Needs::Pcb,
        "annotate_schematic_quietly" | "force_annotate_all_schematics" => Needs::ActiveSheet,
        "zoom_in_at_cursor" | "zoom_out_at_cursor" => Needs::Canvas,
        "zoom_to_fit" | "zoom_to_all_objects" | "show_all_design_objects" => Needs::FitTarget,
        "cycle_snap_grid_forward" | "open_grid_picker" => Needs::FootprintEditor,
        "open_library_browser" => Needs::Library,
        "number_schematic_sheets" => Needs::ProjectSheet,
        "tile_windows_horizontally" | "tile_windows_vertically" => Needs::TileableWindows,
        _ => return None,
    })
}
