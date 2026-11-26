//! What a command needs before it can act: the one table behind
//! [`super::is_enabled`] (the menu greys by it, a keystroke is dropped by it)
//! and behind the reason [`super::CommandOutcome::Disabled`] reports.
//!
//! Each need reads core facts only or core and UI facts (ADR-0006 slice 7):
//! a `CoreOnly` need reads only the document, engine and project facts of
//! [`CoreContext`], so the application core can answer it; a `CoreAndUi`
//! need also reads the window's [`UiContext`]. Which one a need is, is the
//! function its arm sits in: `is_met_from_core` or `is_met_with_ui`. The
//! category says what the predicate reads; it says nothing about who owns
//! the commands it gates, which is the parity table's `owner` column.

use signex_i18n::{MessageId, ids};

use crate::keymap::AppCommandId;

use super::CommandContext;
use super::need_context::{CoreContext, UiContext};

/// What a need's predicate reads.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NeedReads {
    /// Document, engine and project state alone: [`CoreContext`].
    CoreOnly,
    /// Also window, canvas, selection, tool or clipboard state: [`UiContext`].
    CoreAndUi,
}

/// A precondition on the window's [`CommandContext`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Needs {
    /// An Active Bar tool that is a planned placeholder (#1249): never met.
    Planned,
    SaveTarget,
    SaveAsTarget,
    /// Save All: some document or Component row has unsaved work (#2844).
    UnsavedWork,
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
    /// The 3D view's lighting and screenshot copy (mockup row 175): a board
    /// shown in 3D in the window.
    Board3dView,
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
    /// What the predicate reads: whether the need is answered from a
    /// [`CoreContext`] alone.
    #[cfg(test)]
    pub(crate) fn reads(self) -> NeedReads {
        if self.is_met_from_core(&CoreContext::default()).is_some() {
            NeedReads::CoreOnly
        } else {
            NeedReads::CoreAndUi
        }
    }

    pub(crate) fn is_met(self, ctx: &CommandContext) -> bool {
        let core = ctx.core();
        match self.is_met_from_core(&core) {
            Some(met) => met,
            None => self.is_met_with_ui(&core, &ctx.ui()),
        }
    }

    /// A `CoreOnly` need, from the core facts alone; `None` for a
    /// `CoreAndUi` need.
    fn is_met_from_core(self, ctx: &CoreContext) -> Option<bool> {
        Some(match self {
            Self::Planned => false,
            Self::SaveTarget => {
                ctx.document_is_schematic
                    || ctx.document_is_editor
                    || ctx.document_is_dirty_board
                    || (ctx.document_has_file && ctx.project_dirty)
            }
            Self::SaveAsTarget => ctx.document_is_schematic || ctx.document_is_editor,
            Self::UnsavedWork => ctx.has_dirty_documents,
            // A board prints through Board Print / PDF (#2402).
            Self::PrintTarget => ctx.document_is_schematic || ctx.pcb.shown,
            Self::Document => ctx.has_document,
            Self::Undo => ctx.can_undo,
            Self::Redo => ctx.can_redo,
            Self::SelectAllTarget => ctx.has_schematic || ctx.pcb.shown,
            Self::Schematic => ctx.has_schematic,
            // These assemble the project's sheets, not only the active tab's.
            Self::Sheets => ctx.has_schematic || ctx.has_sheets,
            // Annotate acts from the active document's path; with none open it
            // only reports that there is nothing to do.
            Self::ActiveSheet => ctx.has_active_path,
            // Zoom steps the main window's schematic or PCB canvas.
            Self::Canvas => ctx.has_schematic || ctx.has_pcb,
            // The keystroke opens the footprint editor's grid picker and does
            // nothing elsewhere.
            Self::FootprintEditor => ctx.has_footprint_editor,
            Self::Pcb => ctx.has_pcb,
            Self::NetClassTable => ctx.has_schematic || ctx.has_pcb,
            Self::EditablePcb => ctx.has_pcb && !ctx.pcb.read_only,
            Self::MeasureTarget => ctx.has_schematic || ctx.pcb.shown,
            Self::MoveObject => ctx.has_schematic || ctx.pcb.shown,
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
            // The CoreAndUi needs, met in `is_met_with_ui`.
            Self::CopyTarget
            | Self::CutTarget
            | Self::PasteTarget
            | Self::DrawingSelection
            | Self::SelectedObject
            | Self::DeleteTarget
            | Self::VertexTarget
            | Self::DrawInProgress
            | Self::ParentSheet
            | Self::FitTarget
            | Self::TurnedPcbView
            | Self::Board3dView
            | Self::Turn
            | Self::MoveSelection
            | Self::SimilarSeed
            | Self::PcbPlacement
            | Self::SwapFootprints
            | Self::PcbSelection
            | Self::PadPair
            | Self::ReuseSource
            | Self::ReuseAnchor
            | Self::ProbeTarget
            | Self::TileableWindows
            | Self::Tiled => return None,
        })
    }

    /// A `CoreAndUi` need, from the core facts and the window's.
    fn is_met_with_ui(self, core: &CoreContext, ctx: &UiContext) -> bool {
        match self {
            Self::CopyTarget if core.pcb.shown => ctx.pcb.copyable,
            Self::CopyTarget => ctx.has_selection,
            Self::CutTarget if core.pcb.shown => ctx.pcb.deletable,
            Self::CutTarget => editable_selection(ctx),
            Self::PasteTarget if core.pcb.shown => ctx.pcb.pasteable,
            Self::PasteTarget => core.has_schematic,
            // Arrange acts on drawings only; a selection of circuit elements
            // leaves it nothing to do (#1583).
            Self::DrawingSelection => core.has_schematic && ctx.has_drawing_selection,
            // E edits the selected object in the schematic and in the
            // footprint / symbol editors, whose selections `has_selection`
            // does not read, so a keystroke must not be dropped there. On a
            // board it shows the selection's Properties (#1312).
            Self::SelectedObject => {
                ctx.has_selection
                    || core.has_footprint_editor
                    || core.has_symbol_editor
                    || ctx.pcb.any
            }
            // The footprint editor deletes its own selection, which is not
            // the schematic canvas's; with none it is a no-op. A board
            // deletes its own selection, never a locked footprint.
            Self::DeleteTarget if core.pcb.shown => ctx.pcb.deletable,
            Self::DeleteTarget => editable_selection(ctx) || core.has_footprint_editor,
            Self::VertexTarget => ctx.has_selection || core.has_footprint_editor || ctx.drawing,
            Self::DrawInProgress => ctx.drawing,
            // Up Hierarchy opens the sheet whose symbol places the active
            // one; a top-level page has none (#1772). Read from the
            // Navigator panel's hierarchy cache, so it is a UI fact.
            Self::ParentSheet => ctx.has_parent_sheet,
            Self::FitTarget => core.has_schematic || core.has_pcb || ctx.undocked_has_canvas,
            Self::TurnedPcbView => core.has_pcb && ctx.pcb_view_turned,
            Self::Board3dView => core.has_pcb && ctx.board_3d_shown,
            // On a sheet, rotate also turns a part on the cursor with
            // nothing selected; on a board too (#2364).
            // Split into core and in-hand parts in slice 6 (#2326 kill-check decision 3).
            Self::Turn if core.pcb.shown => {
                ctx.pcb.placeable || ctx.pcb.part_in_hand || ctx.pcb.routing
            }
            Self::Turn => core.has_schematic && (editable_selection(ctx) || ctx.turn_in_hand),
            Self::MoveSelection if core.pcb.shown => ctx.pcb.placeable,
            Self::MoveSelection => core.has_schematic && editable_selection(ctx),
            Self::SimilarSeed => {
                (core.has_schematic && ctx.has_selection) || (core.pcb.shown && ctx.pcb.any)
            }
            Self::PcbPlacement => core.pcb.shown && (ctx.pcb.placeable || ctx.pcb.part_in_hand),
            Self::SwapFootprints => core.pcb.shown && ctx.pcb.placeable,
            Self::PcbSelection => core.pcb.shown && ctx.pcb.any,
            Self::PadPair => core.pcb.shown && ctx.pcb.pad_pair,
            Self::ReuseSource => core.pcb.shown && ctx.pcb.copyable,
            Self::ReuseAnchor => core.pcb.shown && ctx.pcb.one_footprint,
            Self::ProbeTarget if core.pcb.shown => ctx.pcb.any,
            Self::ProbeTarget => core.has_schematic && ctx.has_selection,
            // Tile shows the main window's documents in panes (#2249) and
            // arranges the main window with the undocked document
            // windows; alone, or before the main window's screen is known,
            // it has nothing to do.
            Self::TileableWindows => ctx.can_tile_windows,
            Self::Tiled => ctx.is_tiled,
            // The CoreOnly needs, met in `is_met_from_core`.
            Self::Planned
            | Self::SaveTarget
            | Self::SaveAsTarget
            | Self::UnsavedWork
            | Self::PrintTarget
            | Self::Document
            | Self::Undo
            | Self::Redo
            | Self::SelectAllTarget
            | Self::Schematic
            | Self::Sheets
            | Self::ActiveSheet
            | Self::Canvas
            | Self::FootprintEditor
            | Self::Pcb
            | Self::NetClassTable
            | Self::EditablePcb
            | Self::MeasureTarget
            | Self::MoveObject
            | Self::PcbSync
            | Self::OpenProject
            | Self::PcbInProject
            | Self::Library
            | Self::ProjectSheet => unreachable!("a CoreOnly need is met in is_met_from_core"),
        }
    }

    /// Why the command has nothing to act on while this is unmet: a catalog
    /// message (`command_needs.ftl`), rendered by the front end.
    pub(crate) fn reason(self) -> MessageId {
        match self {
            Self::Planned => ids::COMMAND_NEEDS_PLANNED,
            Self::SaveTarget => ids::COMMAND_NEEDS_SAVE_TARGET,
            Self::SaveAsTarget => ids::COMMAND_NEEDS_SAVE_AS_TARGET,
            Self::UnsavedWork => ids::COMMAND_NEEDS_UNSAVED_WORK,
            Self::PrintTarget => ids::COMMAND_NEEDS_PRINT_TARGET,
            Self::Document => ids::COMMAND_NEEDS_DOCUMENT,
            Self::Undo => ids::COMMAND_NEEDS_UNDO,
            Self::Redo => ids::COMMAND_NEEDS_REDO,
            Self::CopyTarget => ids::COMMAND_NEEDS_COPY_TARGET,
            Self::CutTarget => ids::COMMAND_NEEDS_CUT_TARGET,
            Self::PasteTarget => ids::COMMAND_NEEDS_PASTE_TARGET,
            Self::SelectAllTarget
            | Self::Canvas
            | Self::FitTarget
            | Self::NetClassTable
            | Self::MoveObject => ids::COMMAND_NEEDS_SCHEMATIC_OR_PCB,
            Self::DrawingSelection => ids::COMMAND_NEEDS_DRAWING_SELECTION,
            Self::SelectedObject => ids::COMMAND_NEEDS_SELECTED_OBJECT,
            Self::Schematic => ids::COMMAND_NEEDS_SCHEMATIC,
            Self::ParentSheet => ids::COMMAND_NEEDS_PARENT_SHEET,
            Self::DeleteTarget => ids::COMMAND_NEEDS_DELETE_TARGET,
            Self::VertexTarget => ids::COMMAND_NEEDS_VERTEX_TARGET,
            Self::DrawInProgress => ids::COMMAND_NEEDS_DRAW_IN_PROGRESS,
            Self::Sheets => ids::COMMAND_NEEDS_SHEETS,
            Self::ActiveSheet => ids::COMMAND_NEEDS_ACTIVE_SHEET,
            Self::FootprintEditor => ids::COMMAND_NEEDS_FOOTPRINT_EDITOR,
            Self::Pcb => ids::COMMAND_NEEDS_PCB,
            Self::EditablePcb => ids::COMMAND_NEEDS_EDITABLE_PCB,
            Self::MeasureTarget => ids::COMMAND_NEEDS_MEASURE_TARGET,
            Self::TurnedPcbView => ids::COMMAND_NEEDS_TURNED_PCB_VIEW,
            Self::Board3dView => ids::COMMAND_NEEDS_BOARD_3D_VIEW,
            Self::Turn => ids::COMMAND_NEEDS_TURN,
            Self::MoveSelection => ids::COMMAND_NEEDS_MOVE_SELECTION,
            Self::SimilarSeed => ids::COMMAND_NEEDS_SIMILAR_SEED,
            Self::PcbPlacement | Self::SwapFootprints => ids::COMMAND_NEEDS_UNLOCKED_FOOTPRINT,
            Self::PcbSelection => ids::COMMAND_NEEDS_PCB_SELECTION,
            Self::PadPair => ids::COMMAND_NEEDS_PAD_PAIR,
            Self::ReuseSource => ids::COMMAND_NEEDS_REUSE_SOURCE,
            Self::ReuseAnchor => ids::COMMAND_NEEDS_REUSE_ANCHOR,
            Self::ProbeTarget => ids::COMMAND_NEEDS_PROBE_TARGET,
            Self::PcbSync => ids::COMMAND_NEEDS_PCB_SYNC,
            Self::OpenProject => ids::COMMAND_NEEDS_OPEN_PROJECT,
            Self::PcbInProject => ids::COMMAND_NEEDS_PCB_IN_PROJECT,
            Self::Library => ids::COMMAND_NEEDS_LIBRARY,
            Self::ProjectSheet => ids::COMMAND_NEEDS_PROJECT_SHEET,
            Self::TileableWindows => ids::COMMAND_NEEDS_TILEABLE_WINDOWS,
            Self::Tiled => ids::COMMAND_NEEDS_TILED,
        }
    }
}

/// A sheet selection an edit can change: something is selected and not all
/// of it is locked (#2419). The engine skips locked objects, so a selection
/// that is all locked leaves an edit nothing to do.
fn editable_selection(ctx: &UiContext) -> bool {
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
        "save_all" => Needs::UnsavedWork,
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
        // #1280: the harness tools and the definitions view.
        | "place_signal_harness"
        | "place_harness_connector"
        | "place_harness_entry"
        | "open_harness_definitions"
        // #1489: the Net Parameters flag sits on a wire of the sheet in view.
        | "place_parameter_set"
        | "edit_blanket"
        // #1699: a No-ERC directive's rule list.
        | "edit_no_erc_rules"
        // #1667: the file rows place onto the sheet in view.
        | "place_design_block"
        | "import_graphics"
        // #1276: a sheet symbol lands on, or is synced from, the sheet in
        // view; with no sheet symbol selected the sync says what to select.
        | "create_sheet_symbol_from_sheet"
        | "sync_sheet_entries"
        // The table lists the active sheet's symbols.
        | "open_parameter_manager"
        // #1295: the table's CSV, of and into the active sheet.
        | "export_parameters"
        | "import_parameters"
        // Lists the parts of the active schematic's project.
        | "assign_footprints"
        // Dots the bare pin touches of the sheet in view.
        | "repair_pin_junctions"
        // #1267, #1300: the Bezier, dimension and table tools.
        | "draw_bezier"
        | "place_table"
        | "place_linear_dimension"
        | "place_aligned_dimension"
        // Mockup row 158: the unions of the sheet in view; the handler says what to
        // select when nothing fits.
        | "create_union"
        | "add_to_union"
        | "remove_from_union"
        | "break_union" => Needs::Schematic,
        "run_erc" | "open_erc_dialog" => Needs::Sheets,
        // #1300: the contents sheet of the active sheet's project.
        "add_table_of_contents" => Needs::Sheets,
        "run_drc"
        | "reannotate_board"
        | "fabrication_outputs"
        | "open_drc_panel" => Needs::Pcb,
        "open_net_classes" | "open_net_classes_panel" => Needs::NetClassTable,
        // #2216, #2506, mockup row 178: the pours of the board in view.
        "repour_selected" | "repour_all" | "unfill_selected" | "polygon_manager" => Needs::Pcb,
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
        // #2505: a library part's footprint, from the part picker.
        | "place_board_component"
        // #2373: free pads, arcs, strings and dimensions.
        | "place_board_pad"
        | "place_board_arc"
        | "place_board_string"
        | "place_board_dimension"
        // #2507: the other dimension kinds.
        | "place_board_dimension_horizontal"
        | "place_board_dimension_vertical"
        | "place_board_dimension_radial"
        // #2424: the Graphics tools.
        | "place_board_line"
        | "place_board_graphic_arc"
        | "place_board_circle"
        | "place_board_rectangle"
        | "place_board_text_frame"
        | "board_sketch"
        // 2026-11-18: Edit ▸ Origin changes the board.
        | "set_board_origin"
        | "reset_board_origin"
        | "snap_board_origin_to_corner"
        // Layer colours are saved with the board: a read-only board keeps them.
        | "set_layer_color"
        | "edit_layer_color"
        | "reset_layer_colors" => Needs::EditablePcb,
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
        // #2796: the diagram goes in the project's folder.
        "new_block_diagram" => Needs::OpenProject,
        // #1282: the class dialog edits the project.
        "open_diff_pair_classes" => Needs::OpenProject,
        // #2908: a copy of the project in a new folder.
        "save_project_as" => Needs::OpenProject,
        // #2457: the dialog reads the sheets of the board's project.
        "create_rooms_from_sheets" => Needs::PcbInProject,
        "annotate_schematic_quietly" | "force_annotate_all_schematics" => Needs::ActiveSheet,
        // Home View acts on the main window's sheet or board (mockup row 194).
        "zoom_in_at_cursor" | "zoom_out_at_cursor" | "home_view" => Needs::Canvas,
        // Redraw, like a fit, acts on a main or undocked sheet or board canvas.
        "zoom_to_fit" | "zoom_to_all_objects" | "show_all_design_objects" | "refresh_view" => {
            Needs::FitTarget
        }
        "cycle_snap_grid_forward" | "open_grid_picker" => Needs::FootprintEditor,
        "toggle_ratsnest"
        // #2879: Shift+E on a board.
        | "cycle_board_snapping"
        | "toggle_origin_marker"
        | "select_board_net"
        | "highlight_board_net"
        | "view_board_3d"
        // Mockup row 175: the 3D view's Flip (its F is live in the 3D view
        // only, through the `board_3d` shortcut context).
        | "flip_board_3d"
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
        | "show_used_layers_only"
        | "set_object_visible"
        | "set_object_transparency" => Needs::Pcb,
        // At 0° Reset has nothing to do, so it is greyed.
        "view_rotate_reset" => Needs::TurnedPcbView,
        // Mockup row 175 (Caner, 2026-11-24): on a flat board H and
        // Ctrl+Shift+C are greyed, as they need the 3D view.
        "cycle_board_3d_lighting" | "copy_board_3d_screenshot" => Needs::Board3dView,
        "open_library_browser" => Needs::Library,
        "number_schematic_sheets" => Needs::ProjectSheet,
        "compile_project" => Needs::ProjectSheet,
        "generate_block_diagram" | "regenerate_block_diagram" => Needs::ProjectSheet,
        "tile_windows_horizontally" | "tile_windows_vertically" => Needs::TileableWindows,
        "untile_documents" => Needs::Tiled,
        _ => return None,
    })
}
