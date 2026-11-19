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
    /// Copy and Duplicate: a board selection Copy carries, or a selection on
    /// a sheet (#2361).
    CopyTarget,
    /// Cut: a board selection with no locked footprint, or a selection on a
    /// sheet.
    CutTarget,
    /// Paste and Paste Special: board objects on the clipboard on a board,
    /// a sheet otherwise.
    PasteTarget,
    /// Select All: a sheet or a board.
    SelectAllTarget,
    DrawingSelection,
    SelectedObject,
    Schematic,
    ParentSheet,
    DeleteTarget,
    VertexTarget,
    /// A wire, bus or polyline draw is in progress.
    DrawInProgress,
    Sheets,
    ActiveSheet,
    Canvas,
    FitTarget,
    FootprintEditor,
    Pcb,
    /// A board edit tool (Place, Board Sketch): a board in view that is not
    /// read-only (#2405).
    EditablePcb,
    /// Measure Distance: a sheet or a board in view (#2403).
    MeasureTarget,
    /// Reset View Rotation: a board in view that is turned (#2316).
    TurnedPcbView,
    /// Rotate and mirror: on a board, an unlocked footprint selected
    /// (#2240); on a sheet, a selection or something in hand.
    Turn,
    /// Move Selection: an unlocked footprint selected on a board, or a
    /// selection on a sheet.
    MoveSelection,
    /// Move: a sheet or a board to pick from.
    MoveObject,
    /// Find Similar Objects: a selection on a sheet to seed from (#2416), or
    /// one on the board in view (#2403). A key or the sheet's canvas menu
    /// may seed from the object under the cursor, so they still reach the
    /// handler (`explains_when_unavailable`).
    SimilarSeed,
    /// Flip Selection: a board with an unlocked footprint selected.
    PcbPlacement,
    /// Change Footprint: a board with unlocked footprints selected (#2366).
    SwapFootprints,
    /// The board Tools edits that act on a selection (#2408, #2409):
    /// something selected on the board in view.
    PcbSelection,
    /// Swap Pins and Swap Gates: two pads of one unlocked footprint selected
    /// on a board (#2410).
    PadPair,
    /// Save as Reuse Block: board objects selected (#2410).
    ReuseSource,
    /// Place Reuse Block: one unlocked footprint selected, the anchor.
    ReuseAnchor,
    /// The Net Classes panel (#2400, #2416): the project net-class table of a
    /// sheet or a board. Design ▸ Net Classes and Tools ▸ Net Classes open
    /// the same panel, so they share this gate.
    NetClassTable,
    /// Cross Probe: something selected on the board or sheet in view (#1312).
    ProbeTarget,
    /// Update PCB: a schematic or a board in view, with a project open
    /// (#1312).
    PcbSync,
    /// New PCB: a project is open.
    OpenProject,
    /// Create Rooms from Sheets (#2457): a board in view that is not
    /// read-only, with a project open to read the sheets from.
    PcbInProject,
    Library,
    ProjectSheet,
    TileableWindows,
    /// Untile: the main window shows its documents in tile panes (#2249).
    Tiled,
}

impl Needs {
    pub(crate) fn is_met(self, ctx: &CommandContext) -> bool {
        match self {
            Self::Planned => false,
            Self::SaveTarget => {
                ctx.document_is_schematic
                    || ctx.document_is_editor
                    || ctx.document_is_dirty_board
                    || (ctx.document_has_file && ctx.project_dirty)
            }
            Self::SaveAsTarget => ctx.document_is_schematic || ctx.document_is_editor,
            // A board prints through Board Print / PDF (#2402).
            Self::PrintTarget => ctx.document_is_schematic || ctx.pcb.shown,
            Self::Document => ctx.has_document,
            Self::Undo => ctx.can_undo,
            Self::Redo => ctx.can_redo,
            Self::CopyTarget if ctx.pcb.shown => ctx.pcb.copyable,
            Self::CopyTarget => ctx.has_selection,
            Self::CutTarget if ctx.pcb.shown => ctx.pcb.deletable,
            Self::CutTarget => editable_selection(ctx),
            Self::PasteTarget if ctx.pcb.shown => ctx.pcb.pasteable,
            Self::PasteTarget => ctx.has_schematic,
            Self::SelectAllTarget => ctx.has_schematic || ctx.pcb.shown,
            // Arrange acts on drawings only; a selection of circuit elements
            // leaves it nothing to do (#1583).
            Self::DrawingSelection => ctx.has_schematic && ctx.has_drawing_selection,
            // E edits the selected object in the schematic and in the
            // footprint / symbol editors, whose selections `has_selection`
            // does not read, so a keystroke must not be dropped there. On a
            // board it shows the selection's Properties (#1312).
            Self::SelectedObject => {
                ctx.has_selection
                    || ctx.has_footprint_editor
                    || ctx.has_symbol_editor
                    || ctx.pcb.any
            }
            Self::Schematic => ctx.has_schematic,
            // Up Hierarchy opens the sheet whose symbol places the active
            // one; a top-level page has none (#1772).
            Self::ParentSheet => ctx.has_parent_sheet,
            // The footprint editor deletes its own selection, which is not
            // the schematic canvas's; with none it is a no-op. A board
            // deletes its own selection, never a locked footprint.
            Self::DeleteTarget if ctx.pcb.shown => ctx.pcb.deletable,
            Self::DeleteTarget => editable_selection(ctx) || ctx.has_footprint_editor,
            Self::VertexTarget => ctx.has_selection || ctx.has_footprint_editor || ctx.drawing,
            Self::DrawInProgress => ctx.drawing,
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
            Self::NetClassTable => ctx.has_schematic || ctx.has_pcb,
            Self::EditablePcb => ctx.has_pcb && !ctx.pcb.read_only,
            Self::MeasureTarget => ctx.has_schematic || ctx.pcb.shown,
            Self::TurnedPcbView => ctx.has_pcb && ctx.pcb_view_turned,
            // On a sheet, rotate also turns a part on the cursor with
            // nothing selected; on a board too (#2364).
            Self::Turn if ctx.pcb.shown => {
                ctx.pcb.placeable || ctx.pcb.part_in_hand || ctx.pcb.routing
            }
            Self::Turn => ctx.has_schematic && (editable_selection(ctx) || ctx.turn_in_hand),
            Self::MoveSelection if ctx.pcb.shown => ctx.pcb.placeable,
            Self::MoveSelection => ctx.has_schematic && editable_selection(ctx),
            Self::MoveObject => ctx.has_schematic || ctx.pcb.shown,
            Self::SimilarSeed => {
                (ctx.has_schematic && ctx.has_selection) || (ctx.pcb.shown && ctx.pcb.any)
            }
            Self::PcbPlacement => ctx.pcb.shown && (ctx.pcb.placeable || ctx.pcb.part_in_hand),
            Self::SwapFootprints => ctx.pcb.shown && ctx.pcb.placeable,
            Self::PcbSelection => ctx.pcb.shown && ctx.pcb.any,
            Self::PadPair => ctx.pcb.shown && ctx.pcb.pad_pair,
            Self::ReuseSource => ctx.pcb.shown && ctx.pcb.copyable,
            Self::ReuseAnchor => ctx.pcb.shown && ctx.pcb.one_footprint,
            Self::ProbeTarget if ctx.pcb.shown => ctx.pcb.any,
            Self::ProbeTarget => ctx.has_schematic && ctx.has_selection,
            Self::PcbSync => {
                ctx.has_open_project
                    && (ctx.has_schematic
                        || ctx.has_pcb
                        || ctx.document_is_schematic
                        || ctx.pcb.shown)
            }
            Self::OpenProject => ctx.has_open_project,
            Self::PcbInProject => ctx.has_pcb && !ctx.pcb.read_only && ctx.has_open_project,
            // Library Editor focuses an open Library Browser tab, else opens the
            // first open library.
            Self::Library => ctx.has_open_library,
            // Sheet numbers are the project's; a loose sheet has none.
            Self::ProjectSheet => ctx.has_project_sheet,
            // Tile shows the main window's documents in panes (#2249) and
            // arranges the main window with the undocked document
            // windows; alone, or before the main window's screen is known,
            // it has nothing to do.
            Self::TileableWindows => ctx.can_tile_windows,
            Self::Tiled => ctx.is_tiled,
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
            Self::PrintTarget => "Print needs an open schematic or PCB",
            Self::Document => "No document is open",
            Self::Undo => "Nothing to undo",
            Self::Redo => "Nothing to redo",
            Self::CopyTarget => "Nothing is selected to copy",
            Self::CutTarget => "Nothing is selected to cut, or the selection is locked",
            Self::PasteTarget => {
                "Nothing to paste here: no schematic is open, or the clipboard holds no board objects"
            }
            Self::SelectAllTarget => "No schematic or PCB is open",
            Self::DrawingSelection => "No drawing is selected",
            Self::SelectedObject => "No object is selected",
            Self::Schematic => "No schematic is open",
            Self::ParentSheet => "The active sheet has no parent sheet",
            Self::DeleteTarget => "Nothing is selected to delete, or the selection is locked",
            Self::VertexTarget => "Nothing is selected and no wire is being drawn",
            Self::DrawInProgress => "No wire, bus or polyline is being drawn",
            Self::Sheets => "No schematic or project is open",
            Self::ActiveSheet => "No sheet is open",
            Self::Canvas => "No schematic or PCB is open",
            Self::FitTarget => "No schematic or PCB is open",
            Self::FootprintEditor => "No footprint editor is open",
            Self::Pcb => "No PCB is open",
            Self::NetClassTable => "No schematic or PCB is open",
            Self::EditablePcb => "No PCB is open, or the board is read-only",
            Self::MeasureTarget => "No schematic or PCB is in view",
            Self::TurnedPcbView => "The board view is not rotated",
            Self::Turn => "Nothing is selected or in hand to turn, or the selection is locked",
            Self::MoveSelection => "Nothing is selected to move, or the selection is locked",
            Self::MoveObject => "No schematic or PCB is open",
            Self::SimilarSeed => "Nothing is selected on a sheet to find similar objects to",
            Self::PcbPlacement => "No unlocked footprint is selected on a PCB",
            Self::SwapFootprints => "No unlocked footprint is selected on a PCB",
            Self::PcbSelection => "Nothing is selected on the PCB in view",
            Self::PadPair => "Select two pads of one unlocked component on a PCB",
            Self::ReuseSource => "Nothing is selected on a PCB to save as a reuse block",
            Self::ReuseAnchor => {
                "Select one unlocked component of the channel to place the block on"
            }
            Self::ProbeTarget => "Nothing is selected on the sheet or board to cross probe",
            Self::PcbSync => "No schematic or PCB of an open project is in view",
            Self::OpenProject => "No project is open",
            Self::PcbInProject => "No board of an open project is in view, or it is read-only",
            Self::Library => "No library is open",
            Self::ProjectSheet => "No sheet of a project is open",
            Self::TileableWindows => {
                "Tile needs two documents in the main window, or a document in its own window"
            }
            Self::Tiled => "The documents are not tiled",
        }
    }
}

/// A sheet selection an edit can change: something is selected and not all
/// of it is locked (#2419). The engine skips locked objects, so a selection
/// that is all locked leaves an edit nothing to do.
fn editable_selection(ctx: &CommandContext) -> bool {
    ctx.has_selection && !ctx.selection_all_locked
}

/// The commands that move, turn, flip or delete a board selection (#2240),
/// or a sheet selection.
pub(crate) fn is_placement_command(command: &AppCommandId) -> bool {
    matches!(
        needs(command),
        Some(
            Needs::Turn
                | Needs::MoveSelection
                | Needs::PcbPlacement
                | Needs::DeleteTarget
                | Needs::CutTarget
        )
    )
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
        "copy" | "duplicate" => Needs::CopyTarget,
        "cut" => Needs::CutTarget,
        "paste" | "smart_paste" | "paste_special" => Needs::PasteTarget,
        "select_all" => Needs::SelectAllTarget,
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
        "rotate_clockwise"
        | "rotate_counterclockwise"
        | "rotate_selection"
        | "rotate_selection_cw"
        | "mirror_x"
        | "mirror_y"
        | "flip_selected_x"
        | "flip_selected_y" => Needs::Turn,
        "move_selection" => Needs::MoveSelection,
        "move_object" => Needs::MoveObject,
        "find_similar_objects" => Needs::SimilarSeed,
        "flip_selection" => Needs::PcbPlacement,
        "change_footprint" => Needs::SwapFootprints,
        "swap_pins" | "swap_gates" => Needs::PadPair,
        "save_board_reuse_block" => Needs::ReuseSource,
        "place_board_reuse_block" => Needs::ReuseAnchor,
        "remove_last_vertex" => Needs::VertexTarget,
        "undo_last_segment" => Needs::DrawInProgress,
        "paste_array"
        | "repeat_last_item"
        | "rubber_stamp_copy"
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
        // #2415: the rest of the Place menu's tools on a sheet, with the
        // ids that run the same message (`draw_graphic_line` is `draw_line`,
        // `draw_hierarchical_sheet` is `place_sheet_symbol`, `place_text`
        // is `place_text_string`).
        | "place_bus_entry"
        | "place_sheet_symbol"
        | "draw_hierarchical_sheet"
        | "place_no_connect"
        | "place_no_erc"
        | "place_text"
        | "place_text_string"
        | "place_text_frame"
        | "place_note"
        | "draw_line"
        | "draw_graphic_line"
        | "draw_arc"
        | "draw_full_circle"
        | "draw_rectangle"
        | "draw_polygon"
        // #2416: the selection filter acts on the
        // sheet in view.
        | "open_selection_filter"
        | "open_components_panel"
        | "place_symbol"
        | "break_wire"
        // #2420: the directive tools and the Blanket edit.
        | "place_blanket"
        | "place_compile_mask"
        | "place_diff_pair"
        | "edit_blanket"
        // #1667: the file rows place onto the sheet in view.
        | "place_design_block"
        | "import_graphics"
        // The table lists the active sheet's symbols.
        | "open_parameter_manager"
        // Lists the parts of the active schematic's project.
        | "assign_footprints" => Needs::Schematic,
        "run_erc" | "open_erc_dialog" => Needs::Sheets,
        "run_drc"
        | "reannotate_board"
        | "fabrication_outputs"
        | "open_drc_panel" => Needs::Pcb,
        "open_net_classes" | "open_net_classes_panel" => Needs::NetClassTable,
        // #2216: the pours of the board in view.
        "repour_selected" | "repour_all" => Needs::Pcb,
        // #2404, #2402, #2488: the board reports, Board Print / PDF and STEP.
        "board_report_bom"
        | "board_report_netlist"
        | "board_report_net_length"
        | "board_report_components"
        | "export_board_report"
        | "export_board_pdf"
        | "export_assembly_svg"
        | "board_assembly_drawing"
        | "export_board_step" => Needs::Pcb,
        "measure_distance" => Needs::MeasureTarget,
        // #2410: panelisation acts on the board in view.
        "panelize_board" | "remove_board_panel" | "set_board_panel" => Needs::Pcb,
        // #2373: the board placement tools, and Board Sketch; greyed on a
        // read-only board (#2405).
        "place_board_via"
        | "place_board_track"
        | "place_board_fill"
        | "place_board_region"
        | "place_board_polygon_pour"
        | "place_board_pour_cutout"
        | "place_board_slice_pour"
        | "place_board_keepout"
        // #2389: Interactive Routing and the other routing tools.
        | "place_board_diff_pair"
        | "place_board_quick_route"
        | "place_board_multi_route"
        | "place_board_route"
        // #2457: a room by two corners.
        | "place_board_room"
        // #2373: free pads, arcs, strings and dimensions.
        | "place_board_pad"
        | "place_board_arc"
        | "place_board_string"
        | "place_board_dimension"
        // #2424: the Graphics tools.
        | "place_board_line"
        | "place_board_graphic_arc"
        | "place_board_circle"
        | "place_board_rectangle"
        | "place_board_text_frame"
        | "board_sketch" => Needs::EditablePcb,
        // #2389, #2373: the routing and arc modes change no board.
        "cycle_route_mode" | "cycle_arc_mode" => Needs::Pcb,
        // #2424: a drawing's edits on the board `document` names.
        "add_board_graphic" | "set_board_graphic" => Needs::Pcb,
        // #2368: a footprint text on the board `document` names.
        "set_footprint_text" => Needs::Pcb,
        // #2389: length tuning of the selected tracks' nets.
        "tune_net_length" | "tune_diff_pair" => Needs::Pcb,
        // #2401, #2407: the layer stack and the track and via edits.
        "board_layer_stack"
        | "set_layer_stack"
        | "lock_board_items"
        | "unlock_board_items"
        | "set_via_span"
        | "set_layer_sizes"
        | "set_pad_shape" => Needs::Pcb,
        // #2408: with nothing selected these act on every pour or net.
        "stitch_vias" | "add_teardrops" | "remove_teardrops" => Needs::Pcb,
        // #2408, #2409: these act on the selection only.
        "gloss_selected_routes" | "board_shape_from_selection" | "keepout_from_selection" => {
            Needs::PcbSelection
        }
        "arrange_in_box" => Needs::SwapFootprints,
        // #2410: DXF import onto the board in view.
        "import_dxf" | "import_dxf_outline" => Needs::Pcb,
        "cross_probe" => Needs::ProbeTarget,
        "update_pcb_from_schematic" | "back_annotate_schematics" => Needs::PcbSync,
        // #2462: gated like Update PCB.
        "compare_board_with_schematic" => Needs::PcbSync,
        "new_pcb_document" | "new_schematic_sheet" | "open_rules" => Needs::OpenProject,
        // #2457: the dialog reads the sheets of the board's project.
        "create_rooms_from_sheets" => Needs::PcbInProject,
        "annotate_schematic_quietly" | "force_annotate_all_schematics" => Needs::ActiveSheet,
        "zoom_in_at_cursor" | "zoom_out_at_cursor" => Needs::Canvas,
        // Redraw, like a fit, acts on a main or undocked sheet or board canvas.
        "zoom_to_fit" | "zoom_to_all_objects" | "show_all_design_objects" | "refresh_view" => {
            Needs::FitTarget
        }
        "cycle_snap_grid_forward" | "open_grid_picker" => Needs::FootprintEditor,
        "toggle_ratsnest"
        | "toggle_origin_marker"
        | "select_board_net"
        | "highlight_board_net"
        | "view_board_3d"
        | "next_layer"
        | "prev_layer"
        | "next_signal_layer"
        | "toggle_single_layer_mode"
        | "toggle_layer_visibility"
        | "show_all_layers"
        | "view_rotate"
        | "view_rotate_ccw"
        | "view_rotate_180"
        // #2394: the layer strip under the board.
        | "show_layer_set"
        | "save_layer_set"
        | "delete_layer_set"
        | "hide_layer"
        | "highlight_layer"
        | "hide_layer_group"
        | "show_layer_group"
        | "set_layer_tab_names"
        | "toggle_layer_numbers"
        | "flip_board_view"
        // #2363: the Layers panel's board view configuration.
        | "set_layer_visible"
        | "set_active_layer"
        | "set_layer_color"
        | "show_used_layers_only"
        | "set_object_visible"
        | "set_object_transparency" => Needs::Pcb,
        // At 0° Reset has nothing to do, so it is greyed.
        "view_rotate_reset" => Needs::TurnedPcbView,
        "open_library_browser" => Needs::Library,
        "number_schematic_sheets" => Needs::ProjectSheet,
        "tile_windows_horizontally" | "tile_windows_vertically" => Needs::TileableWindows,
        "untile_documents" => Needs::Tiled,
        _ => return None,
    })
}
