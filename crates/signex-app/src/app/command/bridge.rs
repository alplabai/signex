//! The app's single id→[`Message`] bridge.
//!
//! [`core_to_message`] turns a stable `AppCommandId` (the id a profile
//! TOML binds a key to) into the app's namespaced [`Message`] tree
//! (ADR-0001 D3). It is not keymap-specific — menus, the command
//! palette, and a future CLI all route a command id through here on
//! their way to `update`.

use crate::app::contracts::BoardOriginMsg;
use crate::app::pcb_tools::BoardPlaceTool;
use crate::keymap::{AppCommandId, CatalogTextExt};
use signex_types::pcb_dimension_kind::DimensionKind;

use super::CommandArgs;
#[cfg(test)]
use super::args::InvocationContext;

use super::super::*;

/// Resolve a command for DISPATCH, reporting when it cannot be resolved.
///
/// Maps a stable command id onto the app's namespaced [`Message`] tree.
/// Commands without a live dispatch arm return `None`, and
/// [`log_unmapped`] says so — naming the id and which of the two
/// failure modes it hit — so an invocation can no longer vanish without
/// a trace. The catalog ids in `UNMAPPED_CATALOG_IDS` are in that state, every one of them bound to
/// a trigger in a shipped profile; the set is pinned by
/// `tests::UNMAPPED_CATALOG_IDS` and may only shrink.
///
/// Use [`is_dispatchable`] instead when you are merely *asking* whether a
/// command resolves. #619: the palette filtered its rows through this
/// function, so building the palette view emitted one warning per
/// unmapped id — 2432 records in a single session, against a 200-entry
/// ring buffer, which evicted every real diagnostic before anyone could
/// read it. The message was also false on that path: nothing had been
/// invoked.
pub(crate) fn core_to_message(command: &AppCommandId, args: &CommandArgs) -> Option<Message> {
    let resolved = resolve(command, args);
    if resolved.is_none() {
        log_unmapped(command);
    }
    resolved
}

/// Does this command resolve to a message? Silent — no diagnostics.
///
/// The counterpart to [`core_to_message`] for callers that are asking a
/// question rather than dispatching. A query must not narrate.
pub(crate) fn is_dispatchable(command: &AppCommandId) -> bool {
    resolve(command, &CommandArgs::none()).is_some()
}

/// The resolution itself. Pure: no logging, no side effects, so both
/// entry points above are built from one table and cannot drift.
pub(super) fn resolve(command: &AppCommandId, args: &CommandArgs) -> Option<Message> {
    use crate::library::editor::footprint::state::EditorMode;
    use crate::library::{LibraryMessage, PlacedPartAction, PlacedPartMsg};

    let net_nav = |msg| Message::Ui(UiMsg::NetNav(msg));
    let placed_part = |action| {
        Message::Library(LibraryMessage::PlacedPart(PlacedPartMsg::ActionRequested(
            action,
        )))
    };
    let context_action = |action| Message::ContextMenu(ContextMenuMsg::Action(action));
    // The Active Bar's own action for a tool: arms it and remembers it as the
    // group's last tool, as the Active Bar button does.
    let active_bar = |action| Message::ActiveBar(crate::active_bar::ActiveBarMsg::Action(action));
    let text_edit = |target| Message::TextEdit(TextEditMsg::Requested(target));
    let pcb_view = |event| Message::Pcb(PcbMsg::View(event));
    let view_turn = |event| Message::Pcb(PcbMsg::ViewRotation(event));
    let board_tool = |tool| Message::Pcb(PcbMsg::PlaceToolPicked(tool));
    let board_origin = |event| Message::Pcb(PcbMsg::Origin(event));
    let board_report = |kind| Message::from(crate::app::BoardOutputMsg::ReportRequested(kind));
    let board_tools = |event| Message::Pcb(PcbMsg::Tools(event));
    let part_swap = |kind| Message::Pcb(PcbMsg::PartSwap(crate::app::PartSwapMsg::Requested(kind)));
    let extras = |msg| Message::Edit(EditMsg::Extras(msg));
    let unions = |msg| Message::Edit(EditMsg::Unions(msg));
    let message = match command.as_str() {
        "annotate_schematic" => Message::Annotate(AnnotateMsg::OpenDialog),
        "annotate_schematic_quietly" => {
            Message::Annotate(AnnotateMsg::Run(signex_engine::AnnotateMode::Incremental))
        }
        // No originating window: this Esc was invoked as a command, not
        // typed. `None` resolves as a main-window Esc (#547).
        "cancel_current_tool" => Message::Input(InputMsg::EscapePressed { window: None }),
        // Recentres without zooming; it used to share the fit arm below (#525).
        // `center_on_cursor` is the Classic profile's name for it (#1671).
        "center_view_at_cursor" | "center_on_cursor" => {
            Message::CanvasEvent(CanvasEvent::CenterAtCursor)
        }
        "refresh_view" => Message::CanvasEvent(CanvasEvent::RedrawRequested),
        // One fit primitive; it frames object extents, as all three labels promise.
        "show_all_design_objects" | "zoom_to_all_objects" | "zoom_to_fit" => {
            Message::CanvasEvent(CanvasEvent::FitAll)
        }
        "drag_keep_connections" => {
            Message::Selection(selection_request::SelectionRequest::DragKeepingConnections)
        }
        "find_similar_objects" => {
            Message::Selection(selection_request::SelectionRequest::FindSimilar(
                crate::find_similar::FindSimilarMsg::OpenRequested {
                    at: args.context.origin,
                },
            ))
        }
        "zoom_to_selection_area" => Message::CanvasEvent(CanvasEvent::FitSelection),
        "measure_distance" => Message::Tool(ToolMessage::SelectTool(Tool::Measure)),
        "break_wire" => Message::Tool(ToolMessage::SelectTool(Tool::BreakWire)),
        "copy" => Message::Edit(EditMsg::Copy),
        "cycle_selection_mode" => {
            Message::Selection(selection_request::SelectionRequest::CycleMode)
        }
        "previous_grid" | "cycle_snap_grid_backward" => Message::Ui(UiMsg::GridCycleBackward),
        "toggle_properties_panel" => Message::Overlay(OverlayMsg::PanelToggled(
            crate::panels::PanelKind::Properties,
        )),
        // SCH Filter is retired into Filter (mockup rows 40 and 55).
        "toggle_schematic_filter_panel" => {
            Message::Overlay(OverlayMsg::PanelToggled(crate::panels::PanelKind::Filter))
        }
        "toggle_schematic_list_panel" => Message::Overlay(OverlayMsg::PanelToggled(
            crate::panels::PanelKind::ObjectList,
        )),
        "next_document_tab" => Message::Menu(MenuMessage::NextDocumentTab),
        "previous_document_tab" => Message::Menu(MenuMessage::PreviousDocumentTab),
        "find_next" => Message::FindReplaceMsg(crate::find_replace::FindReplaceMsg::NextRequested),
        "find_previous" => {
            Message::FindReplaceMsg(crate::find_replace::FindReplaceMsg::PreviousRequested)
        }
        "next_grid" => Message::Ui(UiMsg::GridCycle),
        "fast_grid_1" => Message::Ui(UiMsg::FastGridPicked(0)),
        "fast_grid_2" => Message::Ui(UiMsg::FastGridPicked(1)),
        "cycle_fast_grid" => Message::Ui(UiMsg::FastGridCycled),
        "cycle_snap_grid_forward" | "open_grid_picker" => Message::Ui(UiMsg::GridPickerOpen),
        "cycle_unit" => Message::Ui(UiMsg::UnitCycled),
        "cycle_wire_bus_graphic_mode" => Message::Tool(ToolMessage::CycleDrawMode),
        "switch_segment_posture" => Message::Tool(ToolMessage::CornerFlipRequested),
        "cycle_wiring_mode" => Message::Tool(ToolMessage::DragWiringCycleRequested),
        "cut" => Message::Edit(EditMsg::Cut),
        "delete_selection" => Message::Edit(EditMsg::DeleteSelected),
        "remove_last_vertex" => Message::Tool(ToolMessage::RemoveLastVertex),
        // The Classic profile's Backspace: Remove Last Vertex without its
        // delete-selection fallback, so it only acts mid-draw (#1669).
        "undo_last_segment" => Message::Tool(ToolMessage::LastSegmentUndoRequested),
        // Aliases of `draw_line` and `place_sheet_symbol`: one route each.
        "draw_graphic_line" => active_bar(crate::active_bar::ActiveBarAction::DrawLine),
        "draw_hierarchical_sheet" => {
            active_bar(crate::active_bar::ActiveBarAction::PlaceSheetSymbol)
        }
        "duplicate" => Message::Edit(EditMsg::Duplicate),
        // The selected placed part's library actions (#1301, #1262).
        "open_datasheet" => placed_part(PlacedPartAction::OpenDatasheet),
        "edit_library_symbol" | "edit_selected_symbol_in_symbol_editor" => {
            placed_part(PlacedPartAction::EditSymbol)
        }
        "bring_drawings_to_front" => Message::Edit(EditMsg::ArrangeDrawings(
            signex_engine::DrawingArrange::ToFront,
        )),
        "send_drawings_to_back" => Message::Edit(EditMsg::ArrangeDrawings(
            signex_engine::DrawingArrange::ToBack,
        )),
        "bring_drawings_forward" => Message::Edit(EditMsg::ArrangeDrawings(
            signex_engine::DrawingArrange::Forward,
        )),
        "send_drawings_backward" => Message::Edit(EditMsg::ArrangeDrawings(
            signex_engine::DrawingArrange::Backward,
        )),
        // Docking the Properties panel IS how this app shows the
        // selection's properties: the status bar's
        // `OpenPropertiesForSelection` and the canvas context menu's
        // "Properties..." row both send this same message.
        "edit_object_properties" | "edit_selected_object_properties" => {
            Message::Menu(MenuMessage::OpenPropertiesPanel)
        }
        // The selected sheet symbol's child-sheet actions: the canvas menu
        // rows send the same context-menu action (#271). These three go round
        // the message loop twice (row -> Command -> here); harmless, the
        // second `context_menu.take()` finds None.
        "open_child_sheet" => context_action(ContextAction::OpenChildSheet),
        "create_sheet_from_sheet_symbol" => context_action(ContextAction::CreateChildSheet),
        "sync_sheet_pins" => context_action(ContextAction::SyncSheetPins),
        // #1276: bottom-up hierarchy.
        "create_sheet_symbol_from_sheet" => {
            Message::from(crate::app::SheetSyncMsg::CreateSymbolRequested)
        }
        "sync_sheet_entries" => Message::from(crate::app::SheetSyncMsg::Requested),
        "change_component" => placed_part(PlacedPartAction::ChangeComponent),
        // Mockup row 158: named unions of the sheet in view.
        "create_union" => unions(crate::app::unions::UnionsMsg::CreateRequested),
        "add_to_union" => unions(crate::app::unions::UnionsMsg::AddSelectionRequested),
        "remove_from_union" => unions(crate::app::unions::UnionsMsg::RemoveSelectionRequested),
        "break_union" => unions(crate::app::unions::UnionsMsg::BreakRequested),
        // Opens the options tab of the project the call names (`args.project`,
        // a project-tree row), else of the active document's project.
        "open_project_options" => match &args.context.project {
            Some(id) => Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
                crate::app::ProjectTreeAction::OpenProjectOptions(vec![id.clone()]),
            )),
            None => Message::Menu(MenuMessage::OpenProjectOptions),
        },
        "find" | "find_text" => Message::Overlay(OverlayMsg::OpenFind),
        "toggle_search_panel" | "toggle_floating_panels" => {
            Message::Overlay(OverlayMsg::FindToggled)
        }
        "find_and_replace" => Message::Overlay(OverlayMsg::OpenReplace),
        "footprint_mode_pads" => {
            Message::Input(InputMsg::FootprintModeShortcut(EditorMode::Normal))
        }
        "footprint_mode_sketch" => {
            Message::Input(InputMsg::FootprintModeShortcut(EditorMode::Sketch))
        }
        "footprint_recompute_courtyard_outline" => {
            Message::Input(InputMsg::FootprintCourtyardOutlineRequested)
        }
        "footprint_mode_view_3d" => {
            Message::Input(InputMsg::FootprintModeShortcut(EditorMode::View3d))
        }
        "force_annotate_all_schematics" => Message::Annotate(AnnotateMsg::Run(
            signex_engine::AnnotateMode::ResetAndRenumber,
        )),
        // Visual-flip semantics (preserved from the pre-keymap hardcoded map):
        // the `X` key = a horizontal (left-right) flip = internal MirrorSelectedY,
        // and `Y` = vertical (top-bottom) flip = MirrorSelectedX. The presets
        // bind physical `X`->mirror_x / `Y`->mirror_y, so the command id names
        // the KEY, and the arm names the AXIS it flips — hence the cross.
        "mirror_x" => Message::Edit(EditMsg::MirrorSelectedY),
        "mirror_y" => Message::Edit(EditMsg::MirrorSelectedX),
        "flip_selection" => Message::Edit(EditMsg::FlipSelection),
        // No part in hand: choose one in the picker (#1767). The board's
        // Component tool opens the same picker under its own gate (#2505).
        "open_components_panel" | "place_symbol" => {
            Message::Library(crate::library::LibraryMessage::OpenPicker)
        }
        "place_board_component" => board_tool(BoardPlaceTool::Component),
        "new_document" | "new_project_with_profile" => Message::Menu(MenuMessage::NewProject),
        "open_command_palette" => Message::CommandPalette(CommandPaletteMsg::Open),
        "open_document" => Message::Menu(MenuMessage::OpenProject),
        "open_grid_properties" => Message::GridProperties(GridPropertiesMsg::Open),
        "open_net_color_palette" => Message::NetColor(NetColorMsg::Open),
        // One dialog, two ids. `PrefNav` has no schematic page —
        // Appearance / Keyboard / Erc / LibraryDistributors /
        // ComponentClasses — and the open handler picks no page, so
        // both ids land on whichever page was last visited. The label
        // "Open schematic preferences" therefore over-promises; the
        // dialog it opens is the only preferences dialog there is.
        "open_preferences" | "open_schematic_preferences" => {
            Message::Preferences(PreferencesMsg::Open)
        }
        "repeat_last_item" => Message::Edit(EditMsg::RepeatLastItem),
        "rubber_stamp_copy" => Message::Edit(EditMsg::RubberStampCopy),
        "paste_array" => Message::Edit(EditMsg::PasteArrayRequested),
        "paste" => Message::Edit(EditMsg::Paste),
        "paste_special" | "smart_paste" => Message::Edit(EditMsg::SmartPaste),
        // Aliases of `draw_bus` / `draw_wire`: one route each, which also
        // remembers the tool as the Active Bar's wiring default.
        "place_bus" => active_bar(crate::active_bar::ActiveBarAction::DrawBus),
        // An alias of Place Net Label, which is an Active Bar command.
        "place_local_net_label" => Message::ActiveBar(crate::active_bar::ActiveBarMsg::Action(
            crate::active_bar::ActiveBarAction::PlaceNetLabel,
        )),
        "place_no_connect" => Message::Tool(ToolMessage::SelectTool(Tool::NoConnect)),
        // The same Active Bar actions the buttons send, so a key and a click
        // arm the same tool. The Place menu's Power Port row sends GND too.
        "place_power_symbol" => Message::ActiveBar(crate::active_bar::ActiveBarMsg::Action(
            crate::active_bar::ActiveBarAction::PlacePowerGND,
        )),
        // The Active Bar's "Port" arms a global label; "Off Sheet
        // Connector" arms a hierarchical one.
        "place_global_label" => Message::ActiveBar(crate::active_bar::ActiveBarMsg::Action(
            crate::active_bar::ActiveBarAction::PlacePort,
        )),
        "place_hierarchical_label" => Message::ActiveBar(crate::active_bar::ActiveBarMsg::Action(
            crate::active_bar::ActiveBarAction::PlaceOffSheetConnector,
        )),
        // An alias of `place_text_string`.
        "place_text" => active_bar(crate::active_bar::ActiveBarAction::PlaceTextString),
        "place_wire" => active_bar(crate::active_bar::ActiveBarAction::DrawWire),
        "place_wire_to_bus_entry" => Message::Tool(ToolMessage::SelectTool(Tool::BusEntry)),
        "placement_accept" => Message::Selection(selection_request::SelectionRequest::LassoCommit),
        "placement_properties" => Message::Tool(ToolMessage::PrePlacementTab),
        // The same message File > Close Document sends.
        "close_active_document" => Message::Menu(MenuMessage::CloseDocument),
        // The project tree's Close Project Documents and the tab menu's Close
        // All Documents close every tab the same way, parking dirty drafts.
        "close_all_documents" => Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
            crate::app::ProjectTreeAction::CloseAllDocuments,
        )),
        // Tools ▸ Library Editor (#2247).
        "open_library_browser" => Message::Library(LibraryMessage::LibraryEditorRequested),
        // Tools ▸ Annotation ▸ Number Schematic Sheets and View ▸ Window ▸
        // Tile (#2247).
        "number_schematic_sheets" => Message::from(crate::app::SheetNumberingMsg::Requested),
        "tile_windows_horizontally" => {
            Message::Window(WindowMsg::TileRequested(crate::app::TileLayout::Stacked))
        }
        "tile_windows_vertically" => {
            Message::Window(WindowMsg::TileRequested(crate::app::TileLayout::SideBySide))
        }
        // View ▸ Window ▸ Untile (#2249).
        "untile_documents" => Message::Window(WindowMsg::UntileRequested),
        // View ▸ Window ▸ Layouts (mockup row 166).
        "apply_layout_schematic"
        | "apply_layout_board"
        | "apply_layout_review"
        | "apply_layout_notebook" => {
            use crate::workspace_layouts::{BuiltinLayout, LayoutId, WorkspaceLayoutMsg};
            let builtin = BuiltinLayout::ALL
                .into_iter()
                .find(|b| b.command_id() == command.as_str())?;
            WorkspaceLayoutMsg::ApplyRequested(LayoutId::Builtin(builtin)).into()
        }
        "save_workspace_layout" => crate::workspace_layouts::SaveLayoutMsg::Requested.into(),
        "manage_workspace_layouts" => crate::workspace_layouts::ManageLayoutsMsg::Requested.into(),
        "reset_workspace_layout" => {
            crate::workspace_layouts::WorkspaceLayoutMsg::ResetActiveRequested.into()
        }
        "open_parameter_manager" => Message::ParameterManager(ParameterManagerMsg::Open),
        // #1295: the Parameter Manager footer's Export… and Import….
        "export_parameters" => crate::app::parameter_io::ParameterIoMsg::ExportRequested.into(),
        "import_parameters" => crate::app::parameter_io::ParameterIoMsg::ImportRequested.into(),
        "print" => Message::PrintPreview(PrintPreviewMsg::Requested),
        "redo" => Message::Edit(EditMsg::Redo),
        // Tools ▸ Assign Footprints (#2248) and View ▸ Net Inspector (#1269).
        "assign_footprints" => Message::from(crate::app::AssignFootprintsMsg::Requested),
        "open_net_inspector" => Message::Overlay(OverlayMsg::OpenPanel(
            crate::panels::PanelKind::NetInspector,
        )),
        // Design ▸ Compile Project and View ▸ Compiled Design (#1279, #1308).
        "compile_project" => Message::from(crate::app::CompiledDesignMsg::CompileRequested),
        "open_compiled_design" => Message::Overlay(OverlayMsg::OpenPanel(
            crate::panels::PanelKind::CompiledDesign,
        )),
        // #2416: View ▸ Selection Filter opens the Active Bar's filter.
        "open_selection_filter" => Message::ActiveBar(crate::active_bar::ActiveBarMsg::ToggleMenu(
            crate::active_bar::ActiveBarMenu::Filter,
        )),
        // Design ▸ Net Classes (#2416) and its alias Tools ▸ Net Classes
        // (#2400): one route to the panel of the project's net-class table.
        "open_net_classes" | "open_net_classes_panel" => {
            Message::Overlay(OverlayMsg::OpenPanel(crate::panels::PanelKind::NetClasses))
        }
        // #1280: the harness definitions view, in Properties.
        "open_harness_definitions" => Message::Dock(crate::dock::DockMessage::Panel(
            crate::panels::PanelMsg::Harness(crate::panels::HarnessPanelMsg::DefinitionsOpened),
        )),
        // `ExportBom` reaches `handle_bom_preview_open`, the BOM dialog;
        // Tools ▸ Reports ▸ Bill of Materials opens the same one.
        "report_manager_bom" => Message::Menu(MenuMessage::ExportBom),
        "reset_schematic_designators" => Message::Annotate(AnnotateMsg::OpenResetConfirm),
        "rotate_clockwise" => Message::Edit(EditMsg::RotateSelectedClockwise),
        "rotate_counterclockwise" => Message::Edit(EditMsg::RotateSelectedCounterclockwise),
        "open_erc_dialog" => Message::Erc(ErcMsg::OpenDialog),
        // #1288: the live ERC preference flips; with `enabled`, the operand
        // handler sets it.
        "toggle_live_erc" => {
            Message::Erc(ErcMsg::Live(crate::app::handlers::erc::LiveErcMsg::Toggled))
        }
        "repair_pin_junctions" => Message::Erc(ErcMsg::PinJunctionRepairRequested),
        // #1300: the contents sheet of the active document's project.
        "add_table_of_contents" => Message::from(crate::app::TablesMsg::TableOfContentsRequested),
        "run_drc" => Message::Pcb(PcbMsg::DrcRequested),
        // Design ▸ Rules (#2398): the active document's project.
        "open_rules" => {
            Message::Project(ProjectMsg::Rules(crate::app::RulesTabMsg::OpenRequested {
                project: None,
            }))
        }
        // The Design Notebook of the active document's project (mockup rows
        // 195, 201, 202) and the schematic's Notebook badges (row 197).
        "open_design_notebook" => NotebookMsg::OpenRequested { project: None }.into(),
        "generate_design_notebook" => NotebookMsg::GenerateRequested { project: None }.into(),
        "export_notebook_pdf" => NotebookMsg::ExportPdfRequested { project: None }.into(),
        "toggle_notebook_badges" => NotebookMsg::BadgesToggled.into(),
        // #2399: the board's DRC results panel.
        "open_drc_panel" => Message::Overlay(OverlayMsg::OpenPanel(crate::panels::PanelKind::Drc)),
        "reannotate_board" => Message::from(crate::app::BoardReannotateMsg::Requested),
        "fabrication_outputs" => Message::from(crate::app::FabOutputsMsg::Requested),
        // #1282: Design ▸ Differential Pair Classes….
        "open_diff_pair_classes" => Message::from(crate::app::DiffPairClassesMsg::Requested),
        // #2404, #2402, #2488: Board Reports on a table, its CSV, Board Print /
        // PDF, the STEP model.
        "board_report_bom" => board_report(signex_output::BoardReportKind::Bom),
        "board_report_netlist" => board_report(signex_output::BoardReportKind::Netlist),
        "board_report_net_length" => board_report(signex_output::BoardReportKind::NetLength),
        "board_report_components" => board_report(signex_output::BoardReportKind::Components),
        "export_board_report" => Message::from(crate::app::BoardOutputMsg::CsvExportRequested),
        "export_board_pdf" => Message::from(crate::app::BoardOutputMsg::PrintRequested),
        // #2486: the assembly drawings as SVG; one drawing opens Board Print /
        // PDF on the assembly set of its `side`.
        "export_assembly_svg" => Message::from(crate::app::BoardOutputMsg::SvgExportRequested),
        "board_assembly_drawing" => {
            Message::from(crate::app::BoardOutputMsg::AssemblyPrintRequested(
                crate::app::board_output::side_operand(args),
            ))
        }
        "export_board_step" => Message::from(crate::app::BoardOutputMsg::StepExportRequested),
        // #2410: Tools ▸ Panelize Board and Remove Panel, on the board in view.
        "panelize_board" => Message::from(crate::app::BoardPanelMsg::PanelizeRequested),
        "remove_board_panel" => Message::from(crate::app::BoardPanelMsg::RemoveRequested),
        "run_erc" => Message::Erc(ErcMsg::Run),
        "save_document" => Message::File(FileMsg::Save),
        "save_document_as" => Message::Menu(MenuMessage::SaveAs),
        "save_all" => Message::Menu(MenuMessage::SaveAll),
        // #2908: the dialog on the active document's project.
        "save_project_as" => {
            Message::from(crate::app::SaveProjectAsMsg::Requested { project: None })
        }
        "select_all" => Message::Selection(selection_request::SelectionRequest::SelectAll),
        // An alias of `select_connection`, the Active Bar's Select > Connection (#1668).
        "select_expand_connection" => {
            active_bar(crate::active_bar::ActiveBarAction::SelectConnection)
        }
        "select_node_or_connection_item" => {
            Message::Selection(selection_request::SelectionRequest::ConnectionItemAtCursorRequested)
        }
        "show_current_command_hotkeys" | "show_current_command_shortcuts" => {
            Message::Menu(MenuMessage::OpenKeyboardShortcuts)
        }
        "toggle_visible_grid" => Message::Ui(UiMsg::GridToggle),
        "toggle_ratsnest" => pcb_view(PcbViewMsg::RatsnestVisibilityToggled),
        "toggle_origin_marker" => pcb_view(PcbViewMsg::OriginMarkerVisibilityToggled),
        "board_sketch" => Message::Pcb(PcbMsg::Sketch(crate::app::BoardSketchMsg::ModeToggled)),
        "view_board_3d" => Message::Pcb(PcbMsg::Board3d(crate::app::Board3dMsg::Toggled)),
        // Mockup row 175: the 3D view toolbar's Flip, lighting and screenshot
        // buttons; each does nothing while the board is flat.
        "flip_board_3d" => Message::Pcb(PcbMsg::Board3d(crate::app::Board3dMsg::FlipRequested)),
        "cycle_board_3d_lighting" => {
            Message::Pcb(PcbMsg::Board3d(crate::app::Board3dMsg::LightingCycled))
        }
        "copy_board_3d_screenshot" => Message::Pcb(PcbMsg::Board3d(
            crate::app::Board3dMsg::ScreenshotCopyRequested,
        )),
        "place_board_via" => board_tool(BoardPlaceTool::Via),
        "place_board_track" => board_tool(BoardPlaceTool::Track),
        "place_board_fill" => board_tool(BoardPlaceTool::Fill),
        "place_board_region" => board_tool(BoardPlaceTool::SolidRegion),
        "place_board_polygon_pour" => board_tool(BoardPlaceTool::PolygonPour),
        "place_board_pour_cutout" => board_tool(BoardPlaceTool::PourCutout),
        "place_board_slice_pour" => board_tool(BoardPlaceTool::SlicePour),
        // #2216: fill the pours of the board in view again and say what
        // their copper came to.
        "repour_selected" => Message::Pcb(PcbMsg::RepourRequested { selected: true }),
        "repour_all" => Message::Pcb(PcbMsg::RepourRequested { selected: false }),
        // #2506: clear the copper of the selected pours until Repour.
        "unfill_selected" => Message::Pcb(PcbMsg::UnfillRequested),
        "place_board_keepout" => board_tool(BoardPlaceTool::Keepout),
        // #2457: Place ▸ Room and Design ▸ Rooms ▸ Create Rooms from Sheets.
        "place_board_room" => board_tool(BoardPlaceTool::Room),
        "create_rooms_from_sheets" => Message::Pcb(PcbMsg::CreateRooms(
            crate::app::rooms::CreateRoomsMsg::Requested,
        )),
        // #2457, #2941: translated to their operand handlers.
        "arrange_in_room" => Message::Pcb(PcbMsg::RoomTool(
            crate::app::rooms::RoomToolMsg::ArrangeRequested,
        )),
        "auto_place" => Message::Pcb(PcbMsg::RoomTool(
            crate::app::rooms::RoomToolMsg::AutoPlaceRequested,
        )),
        "place_board_route" => board_tool(BoardPlaceTool::InteractiveRouting),
        // #2424: the Graphics tools.
        "place_board_line" => board_tool(BoardPlaceTool::Line),
        "place_board_graphic_arc" => board_tool(BoardPlaceTool::GraphicArc),
        "place_board_circle" => board_tool(BoardPlaceTool::FullCircle),
        "place_board_rectangle" => board_tool(BoardPlaceTool::Rectangle),
        "place_board_text_frame" => board_tool(BoardPlaceTool::TextFrame),
        "place_board_diff_pair" => board_tool(BoardPlaceTool::DiffPairRouting),
        "place_board_quick_route" => board_tool(BoardPlaceTool::QuickRoute),
        "place_board_multi_route" => board_tool(BoardPlaceTool::MultiRoute),
        "tune_net_length" => Message::Pcb(PcbMsg::Tune(TuneMsg::NetLengthRequested)),
        "tune_diff_pair" => Message::Pcb(PcbMsg::Tune(TuneMsg::DiffPairRequested)),
        "cycle_route_mode" => Message::Pcb(PcbMsg::RouteModeStepped),
        "import_dxf" => crate::app::handlers::pcb_dxf_import::requested(false),
        "import_dxf_outline" => crate::app::handlers::pcb_dxf_import::requested(true),
        "place_board_pad" => board_tool(BoardPlaceTool::Pad),
        "place_board_arc" => board_tool(BoardPlaceTool::Arc),
        "place_board_string" => board_tool(BoardPlaceTool::TextString),
        // #2507: one tool per dimension kind.
        "place_board_dimension" => board_tool(BoardPlaceTool::Dimension(DimensionKind::Aligned)),
        "place_board_dimension_horizontal" => {
            board_tool(BoardPlaceTool::Dimension(DimensionKind::Horizontal))
        }
        "place_board_dimension_vertical" => {
            board_tool(BoardPlaceTool::Dimension(DimensionKind::Vertical))
        }
        "place_board_dimension_radial" => {
            board_tool(BoardPlaceTool::Dimension(DimensionKind::Radial))
        }
        "cycle_arc_mode" => Message::Pcb(PcbMsg::ArcModeStepped),
        "change_footprint" => Message::Pcb(PcbMsg::FootprintSwap(
            crate::app::FootprintSwapMsg::Requested,
        )),
        // #2401 / #2407: the layer stack manager and the selection's lock.
        "board_layer_stack" => {
            Message::Overlay(OverlayMsg::OpenPanel(crate::panels::PanelKind::LayerStack))
        }
        // Mockup row 178: the Polygon Manager tab, the message its PCB
        // panel link sends too.
        "polygon_manager" => Message::Dock(crate::dock::DockMessage::Panel(
            crate::panels::PanelMsg::PcbPanel(crate::panels::PcbPanelMsg::Polygons(
                crate::panels::PolygonMsg::ManagerOpened,
            )),
        )),
        "lock_board_items" => Message::Pcb(PcbMsg::LockRequested { locked: true }),
        "unlock_board_items" => Message::Pcb(PcbMsg::LockRequested { locked: false }),
        // #2403: the net `net` names, else that of the board selection or
        // of what is under the cursor.
        "select_board_net" => Message::Pcb(PcbMsg::Net(crate::app::BoardNetMsg::SelectRequested(
            super::operand_handlers::board_net_operand(args),
        ))),
        "highlight_board_net" => {
            Message::Pcb(PcbMsg::Net(crate::app::BoardNetMsg::HighlightRequested(
                super::operand_handlers::board_net_operand(args),
            )))
        }
        // #2408, #2409: the board Tools edits on the board in view.
        "stitch_vias" => board_tools(BoardToolsMsg::StitchRequested),
        "add_teardrops" => board_tools(BoardToolsMsg::TeardropsAddRequested),
        "remove_teardrops" => board_tools(BoardToolsMsg::TeardropsRemoveRequested),
        "gloss_selected_routes" => board_tools(BoardToolsMsg::GlossRequested),
        "board_shape_from_selection" => board_tools(BoardToolsMsg::BoardShapeRequested),
        "keepout_from_selection" => board_tools(BoardToolsMsg::KeepoutRequested),
        "arrange_in_box" => board_tools(BoardToolsMsg::ArrangeRequested),
        // #2410: the two pads selected on the board name the pins or gates.
        "swap_pins" => part_swap(signex_engine::pcb::pad_swap::SwapKind::Pins),
        "swap_gates" => part_swap(signex_engine::pcb::pad_swap::SwapKind::Gates),
        "swap_schematic_pins" => Message::from(crate::app::SheetSwapMsg::PinsRequested),
        "swap_schematic_gates" => Message::from(crate::app::SheetSwapMsg::GatesRequested),
        "save_board_reuse_block" => {
            Message::Pcb(PcbMsg::ReuseBlock(crate::app::ReuseBlockMsg::SaveRequested))
        }
        "next_layer" => pcb_view(PcbViewMsg::ActiveLayerStepped { forward: true }),
        "prev_layer" => pcb_view(PcbViewMsg::ActiveLayerStepped { forward: false }),
        "next_signal_layer" => pcb_view(PcbViewMsg::SignalLayerStepped { forward: true }),
        "toggle_single_layer_mode" => pcb_view(PcbViewMsg::SingleLayerModeToggled),
        "toggle_layer_visibility" => pcb_view(PcbViewMsg::ActiveLayerVisibilityToggled),
        "show_all_layers" => pcb_view(PcbViewMsg::AllLayersShown),
        // #2394: the board seen from below, and the layer tabs' numbers.
        "flip_board_view" => pcb_view(PcbViewMsg::BoardFlipToggled),
        // 2026-11-18: the layer colours, saved with the board.
        "edit_layer_color" => Message::Pcb(PcbMsg::LayerTabMenu(
            crate::app::LayerTabMenuMsg::ColourEditRequested(None),
        )),
        "reset_layer_colors" => pcb_view(PcbViewMsg::LayerColoursReset),
        // 2026-11-18: Edit ▸ Origin.
        "set_board_origin" => board_origin(BoardOriginMsg::SetPicked),
        "reset_board_origin" => board_origin(BoardOriginMsg::ResetPicked),
        "snap_board_origin_to_corner" => board_origin(BoardOriginMsg::SnapToCornerPicked),
        "toggle_layer_numbers" => Message::Pcb(PcbMsg::LayerStrip(
            crate::app::LayerStripMsg::LayerNumbersToggled,
        )),
        // #2316: the board view's turn; `view_rotate` takes `degrees`.
        "view_rotate" => view_turn(ViewRotationMsg::TurnedBy(
            super::operand_handlers::view_turn_degrees(args),
        )),
        "view_rotate_ccw" => view_turn(ViewRotationMsg::TurnedBy(-90.0)),
        "view_rotate_180" => view_turn(ViewRotationMsg::TurnedBy(180.0)),
        "view_rotate_reset" => view_turn(ViewRotationMsg::AngleSet(0.0)),
        // Mockup row 194: rotation 0°, Flip off, Fit.
        "home_view" => Message::Ui(UiMsg::HomeViewRequested),
        "toggle_layers_panel" => {
            Message::Overlay(OverlayMsg::PanelToggled(crate::panels::PanelKind::Layers))
        }
        "toggle_auto_focus" => Message::Overlay(OverlayMsg::ToggleAutoFocus),
        "toggle_electrical_grid" => Message::Ui(UiMsg::ToggleSnapHotspots),
        // #2879: Shift+E on a board steps the Snap Options' Snapping.
        "cycle_board_snapping" => Message::Dock(crate::dock::DockMessage::Panel(
            crate::panels::PanelMsg::PcbBoard(crate::panels::PcbBoardMsg::SnappingCycled),
        )),
        "cross_probe" => Message::CrossProbe(CrossProbeMsg::ProbeRequested),
        "toggle_cross_select_mode" => Message::CrossProbe(CrossProbeMsg::CrossSelectModeToggled),
        // #1312: the Update PCB dialog for the document in view. Its headless
        // `apply=true` form runs through `run_operands`, not here.
        "update_pcb_from_schematic" => Message::UpdatePcb(UpdatePcbMsg::Requested { apply: false }),
        // #2462: the Compare Board with Schematic report of the project in view.
        "compare_board_with_schematic" => Message::from(crate::app::CompareBoardMsg::Requested),
        "back_annotate_schematics" => Message::from(crate::app::BackAnnotateMsg::Requested),
        "new_pcb_document" => Message::UpdatePcb(UpdatePcbMsg::NewPcbRequested(None)),
        // #2232: the setup dialogs; a headless run with `profile` makes the
        // document through `run_operands` instead.
        "new_schematic_sheet" => Message::from(crate::app::DocumentSetupMsg::OpenRequested {
            kind: crate::app::SetupKind::Sheet,
            project: None,
        }),
        "undo" => Message::Edit(EditMsg::Undo),
        "unselect_all" => Message::Selection(selection_request::SelectionRequest::ClearAll),
        // Tools > Up Hierarchy: the sheet that references the focused one.
        "leave_sheet" | "navigate_up_hierarchy" => Message::Menu(MenuMessage::AscendToParentSheet),
        "next_sheet" => Message::Menu(MenuMessage::NextSheet),
        "previous_sheet" => Message::Menu(MenuMessage::PreviousSheet),
        // Zooms about the view centre, as View > Zoom In/Out does.
        "zoom_in_at_cursor" => Message::Menu(MenuMessage::ZoomIn),
        "zoom_out_at_cursor" => Message::Menu(MenuMessage::ZoomOut),
        // Cross-sheet net navigation (#1270, #1271, #1272).
        "sheet_navigation_back" => net_nav(NetNavMsg::SheetBackRequested),
        "sheet_navigation_forward" => net_nav(NetNavMsg::SheetForwardRequested),
        "go_to_net_partner" => net_nav(NetNavMsg::PartnerJumpRequested),
        // From a context menu: the net where the menu was opened (#2419).
        "highlight_net_under_cursor" => match args.context.origin {
            Some(at) => net_nav(NetNavMsg::NetHighlightAtRequested { at }),
            None => net_nav(NetNavMsg::NetHighlightRequested),
        },
        "highlight_related_net_objects" => net_nav(NetNavMsg::RelatedNetHighlightRequested),
        "next_highlighted_net_item" => net_nav(NetNavMsg::HighlightStepRequested { forward: true }),
        "previous_highlighted_net_item" => {
            net_nav(NetNavMsg::HighlightStepRequested { forward: false })
        }
        "clear_net_highlighting" => net_nav(NetNavMsg::HighlightClearRequested),
        // In-place field edits (#1255, #1670): the object under the cursor,
        // else the single selection; Messages says so when there is none.
        "edit_value" => text_edit(TextEditTarget::Value),
        "edit_reference_designator" => text_edit(TextEditTarget::Designator),
        "edit_footprint_field" => text_edit(TextEditTarget::FootprintField),
        "edit_text_in_place" => text_edit(TextEditTarget::Text),
        // Placement extras (#1667). Copy attributes takes the object where a
        // context menu was opened, else the one under the cursor.
        "autoplace_fields" => extras(PlacementExtrasMsg::AutoplaceFieldsRequested),
        "copy_attributes_or_add_vertex" => extras(PlacementExtrasMsg::CopyAttributesRequested {
            at: args.context.origin,
        }),
        "place_design_block" => extras(PlacementExtrasMsg::DesignBlockRequested),
        "import_graphics" => extras(PlacementExtrasMsg::ImportGraphicsRequested),
        // #1283: the snippets dialog, and the selection saved as a snippet.
        "place_snippet" => extras(PlacementExtrasMsg::SnippetRequested),
        "save_snippet" => extras(PlacementExtrasMsg::SnippetSaveRequested),
        // BRIDGE_ARMS_END — `tests::match_block` scans for command-id
        // literals up to this marker and no further. Everything below is
        // fallthrough machinery whose own string literals must not be
        // mistaken for bridge arms. Move the marker, not the anchor.
        //
        // The schematic Active Bar's 59 actions are a table, not 59 arms
        // here — see `super::active_bar`. Consulted before giving up, so
        // adding an Active Bar command means adding one table row.
        id => Message::ActiveBar(crate::active_bar::ActiveBarMsg::Action(
            super::active_bar::action_for_id(id)?,
        )),
    };
    Some(message)
}

/// Report an id that reached the bridge and found no arm.
///
/// Lives here, not at a call site, so every consumer of the registry
/// inherits it — the keyboard today, and the menu bar, command palette
/// and CLI as they are rewired onto `dispatch_command` (#367, #366).
///
/// The two failure modes want different words because they need
/// different fixes:
/// - a real catalog id with no arm — the binding is advertised in the
///   Keyboard Shortcuts pane and does nothing. the ids in `UNMAPPED_CATALOG_IDS` are in this state;
///   see `tests::UNMAPPED_CATALOG_IDS`.
/// - an id that is in no catalog at all — almost always a typo in a
///   user-edited keymap TOML, which `keymap::profile` accepts without
///   ever validating against the catalog.
///
/// Deliberately `log_warning`, not `log_error`: neither case loses data
/// or leaves the app wrong, and neither is actionable mid-edit.
fn log_unmapped(command: &AppCommandId) {
    let id = command.as_str();
    if let Some(metadata) = crate::keymap::metadata_for(command) {
        crate::diagnostics::log_warning(format!(
            "command '{id}': {} is not available yet",
            // The log is English whatever the UI language.
            metadata.label_in(&signex_i18n::I18n::en_us())
        ));
    } else {
        crate::diagnostics::log_warning(format!(
            "command '{id}' is not in the command catalog — check the id in \
             your keyboard-shortcuts profile{}",
            crate::keymap::did_you_mean(id)
        ));
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::keymap::metadata_for;

    /// `core_to_message`'s own source, embedded at compile time so the
    /// guard below scans the exact arms it checks instead of a
    /// hand-kept duplicate list ("kept in step" copies are banned).
    const BRIDGE_SRC: &str = include_str!("bridge.rs");

    /// The `core_to_message` match block alone, cut out of `src`.
    ///
    /// Load-bearing: `BRIDGE_SRC` is the WHOLE file, tests included, and
    /// this module now holds `UNMAPPED_CATALOG_IDS` — the string literals
    /// on lines that start with `"`. Scanning the whole file would read
    /// every one of them as a bridge arm and quietly invert the ratchet.
    fn match_block(src: &str) -> &str {
        let start = src
            .find("let message = match command.as_str() {")
            .expect("core_to_message's match block moved — update match_block");
        let rest = &src[start..];
        // Ends at an explicit marker rather than at the fallthrough arm's
        // text, which has already changed shape once. Everything after it
        // — `log_unmapped`'s message literals, the pinned id list — must
        // stay out of the scan.
        let end = rest
            .find("BRIDGE_ARMS_END")
            .expect("the BRIDGE_ARMS_END marker is gone — restore it in core_to_message");
        &rest[..end]
    }

    /// Pull every quoted command-id literal out of `src`'s match-arm
    /// lines (each starts, once trimmed, with `"` or — on a wrapped
    /// multi-id arm — with `|`). Deliberately tiny, no regex dependency
    /// — mirrors `keymap::menu_command_tests`'s `ids_from_call`.
    /// Every id `core_to_message` can resolve: the match arms it names
    /// literally, plus the Active Bar table its fallthrough consults.
    /// A source scan alone would miss the table and report all 59 Active
    /// Bar commands as dead.
    fn resolvable_command_ids(src: &str) -> HashSet<String> {
        let mut ids: HashSet<String> = bridged_command_ids(src).into_iter().collect();
        ids.extend(crate::app::command::active_bar::command_ids().map(str::to_string));
        ids.extend(crate::app::command::queries::query_command_ids().map(str::to_string));
        // A handler-only command (refused without operands, by design) has
        // no bridge arm; a handler id that does not say so is still caught.
        ids.extend(
            crate::app::command::operand_handlers::handled_ids()
                .filter(|id| {
                    AppCommandId::new(*id).is_ok_and(|command| {
                        crate::app::command::operand_handlers::needs_operands_reason(&command)
                            .is_some()
                    })
                })
                .map(str::to_string),
        );
        ids
    }

    fn bridged_command_ids(src: &str) -> Vec<String> {
        let mut ids = Vec::new();
        for line in match_block(src).lines() {
            // Once an arm lists enough ids to pass the width limit,
            // rustfmt puts each one on its own line starting with `|`.
            // Reading only `"`-lines drops those ids, and the ratchet
            // then reports live commands as dead — it fails loudly
            // rather than silently, but it still fails.
            let trimmed = line.trim_start();
            if !trimmed.starts_with('"') && !trimmed.starts_with('|') {
                continue;
            }
            // Only the pattern side of an arm can name a command id.
            // Everything past `=>` is the body — a trailing comment or a
            // `&str` argument there is not an id, and scanning it would
            // mark a command bridged that has no arm. That failure is
            // SILENT: the ratchet would pass while the shortcut stays
            // dead, which is the one thing it exists to prevent.
            let mut rest = trimmed.split_once("=>").map_or(trimmed, |(head, _)| head);
            while let Some(open) = rest.find('"') {
                let after = &rest[open + 1..];
                match after.find('"') {
                    Some(close) => {
                        ids.push(after[..close].to_string());
                        rest = &after[close + 1..];
                    }
                    None => break,
                }
            }
        }
        ids
    }

    /// Drift guard: every id `core_to_message` matches must resolve in
    /// the command catalog via [`metadata_for`]. `menu_command_tests`
    /// guards the menu *views*; this is the analogous guard for the
    /// bridge itself, which nothing scanned before this slice.
    #[test]
    fn every_bridged_command_id_resolves_in_the_catalog() {
        let ids = bridged_command_ids(BRIDGE_SRC);
        assert!(
            ids.len() >= 59,
            "bridge scan found only {} command ids — core_to_message's \
             match arms may have drifted from this source scan",
            ids.len()
        );

        let orphans: Vec<String> = ids
            .into_iter()
            .filter(|id| {
                AppCommandId::new(id.as_str())
                    .ok()
                    .and_then(|command| metadata_for(&command))
                    .is_none()
            })
            .collect();

        assert!(
            orphans.is_empty(),
            "core_to_message maps command ids with no CommandMetadata \
             entry (add them to signex-commands/src/catalog): {orphans:?}"
        );
    }

    /// Catalog ids that `core_to_message` has no arm for, as measured on
    /// `trunk` @ `f211e7d1` (2026-07-25). Every one of them is bound to a
    /// trigger in the shipped `assets/keyboard-shortcuts/{signex_standard,classic}.toml`
    /// (a key sequence or a pointer gesture), so the user presses
    /// the key, `take_keymap_match` consumes the stroke, and nothing happens.
    ///
    /// This list may only SHRINK. Adding a catalog entry without a bridge
    /// arm, or deleting an arm, fails `unmapped_command_ids_only_shrink`.
    /// Wiring one up means deleting its line here, in the same commit.
    ///
    /// The ids listed there are a mix, and the distinction decides the fix:
    /// - **unimplemented** — the catalog entry and the binding are the only
    ///   occurrences in the tree (`place_junction`,
    ///   `repeat_last_item`). A feature, not an arm.
    ///   This is most of what is left. It also covers ids whose obvious
    ///   target turns out to be a STUB, where an arm would trade a logged
    ///   warning for a silent no-op. Read the handler body, not just its
    ///   existence.
    /// - **needs an argument** — the target `Message` carries data this
    ///   entry point cannot supply. Unblocked by `dispatch(id, args)` (#367),
    ///   not by a match arm.
    /// - **no model concept** — `reset_local_coordinates` resets a local
    ///   coordinate origin, and the app has none (no relative origin, no
    ///   delta readout). The document's `PageOrigin`
    ///   (`PanelMsg::SetPageOrigin`) is a different thing, a saved sheet
    ///   setting, so this id must not be wired to it.
    /// - **not addressable from a bare id** — the operation exists but
    ///   only in a form this entry point cannot name. `DockMessage` does
    ///   have `ClosePanel` and `ToggleCollapse`, but both are addressed
    ///   by `(PanelPosition, index)`; the panel toggles go through the
    ///   kind-addressed `OverlayMsg::PanelToggled`. `unselect_all` is
    ///   stranded for the opposite reason: every `selected.clear()` in the tree is a step inside
    ///   some larger operation, never a message of its own.
    ///
    /// Full analysis:
    /// `docs/audit/command-registry-action-surface-2026-07-25.md` §5.
    #[rustfmt::skip]
    const UNMAPPED_CATALOG_IDS: &[&str] = &[
        "reset_local_coordinates",
    ];

    /// Coverage ratchet: the set of catalog ids with no dispatch arm may
    /// only shrink.
    ///
    /// The bridge's `_ => return None` fallthrough makes an unmapped id
    /// indistinguishable from a mapped one at the call site — the keymap
    /// resolves it, consumes the stroke, and no-ops. Nothing measured that
    /// gap before this test, so it grew unnoticed.
    ///
    /// Deliberately two assertions rather than one set equality: the
    /// "no longer unmapped" direction is a fix and gets its own message
    /// telling you to update the list, while the "newly unmapped"
    /// direction is the regression this exists to catch.
    #[test]
    fn unmapped_command_ids_only_shrink() {
        let bridged = resolvable_command_ids(BRIDGE_SRC);
        let pinned: HashSet<&str> = UNMAPPED_CATALOG_IDS.iter().copied().collect();

        let newly_unmapped: Vec<&'static str> = crate::keymap::all_command_ids()
            .filter(|id| !bridged.contains(*id) && !pinned.contains(id))
            .collect();
        assert!(
            newly_unmapped.is_empty(),
            "these command ids are in the catalog with no core_to_message \
             arm, and are not pinned: {newly_unmapped:?}. A catalog entry \
             whose key binding silently no-ops is a dead shortcut — add the \
             dispatch arm, or add the id to UNMAPPED_CATALOG_IDS with a note \
             saying why it cannot be wired yet."
        );

        let now_mapped: Vec<&str> = UNMAPPED_CATALOG_IDS
            .iter()
            .copied()
            .filter(|id| bridged.contains(*id))
            .collect();
        assert!(
            now_mapped.is_empty(),
            "these ids now have a dispatch arm but are still pinned as \
             unmapped: {now_mapped:?}. Delete them from \
             UNMAPPED_CATALOG_IDS in the same commit that wires them up — \
             the ratchet only means anything if it tightens."
        );
    }

    /// The ratchet proves the seven ids this slice drained now *resolve*;
    /// it cannot tell a right arm from a wrong one. A typo that sent
    /// `place_no_connect` to `Tool::Wire` would leave the ratchet green
    /// and the shortcut quietly drawing wires, so pin each target here.
    #[test]
    fn the_newly_wired_ids_reach_their_named_messages() {
        fn resolve(id: &str) -> Message {
            let command = AppCommandId::new(id).expect("a catalog id is a valid AppCommandId");
            core_to_message(&command, &CommandArgs::none())
                .unwrap_or_else(|| panic!("'{id}' no longer resolves"))
        }

        // One route with its `draw_line` alias (#1922).
        assert!(matches!(
            resolve("draw_graphic_line"),
            Message::ActiveBar(crate::active_bar::ActiveBarMsg::Action(
                crate::active_bar::ActiveBarAction::DrawLine
            ))
        ));
        assert!(matches!(
            resolve("edit_selected_object_properties"),
            Message::Menu(MenuMessage::OpenPropertiesPanel)
        ));
        assert!(matches!(
            resolve("open_schematic_preferences"),
            Message::Preferences(PreferencesMsg::Open)
        ));
        assert!(matches!(
            resolve("place_no_connect"),
            Message::Tool(ToolMessage::SelectTool(Tool::NoConnect))
        ));
        assert!(matches!(
            resolve("place_power_symbol"),
            Message::ActiveBar(crate::active_bar::ActiveBarMsg::Action(
                crate::active_bar::ActiveBarAction::PlacePowerGND
            ))
        ));
        assert!(matches!(
            resolve("place_global_label"),
            Message::ActiveBar(crate::active_bar::ActiveBarMsg::Action(
                crate::active_bar::ActiveBarAction::PlacePort
            ))
        ));
        assert!(matches!(
            resolve("place_hierarchical_label"),
            Message::ActiveBar(crate::active_bar::ActiveBarMsg::Action(
                crate::active_bar::ActiveBarAction::PlaceOffSheetConnector
            ))
        ));
        // The key reaches the Active Bar button, which arms Tool::Junction.
        assert!(matches!(
            resolve("place_junction"),
            Message::ActiveBar(crate::active_bar::ActiveBarMsg::Action(
                crate::active_bar::ActiveBarAction::PlaceJunction
            ))
        ));
        // Place Net Label arms the net label and its ghost, the same as the
        // Active Bar button; the local alias shares the arm (#1920).
        for id in ["place_net_label", "place_local_net_label"] {
            assert!(
                matches!(
                    resolve(id),
                    Message::ActiveBar(crate::active_bar::ActiveBarMsg::Action(
                        crate::active_bar::ActiveBarAction::PlaceNetLabel
                    ))
                ),
                "{id}"
            );
        }
        assert!(matches!(
            resolve("place_wire_to_bus_entry"),
            Message::Tool(ToolMessage::SelectTool(Tool::BusEntry))
        ));
        // The BOM command opens the BOM dialog (`ExportBom`).
        assert!(matches!(
            resolve("report_manager_bom"),
            Message::Menu(MenuMessage::ExportBom)
        ));
        assert!(matches!(
            resolve("zoom_to_all_objects"),
            Message::CanvasEvent(CanvasEvent::FitAll)
        ));
        // #525: centring is not a fit.
        assert!(matches!(
            resolve("center_view_at_cursor"),
            Message::CanvasEvent(CanvasEvent::CenterAtCursor)
        ));
        // #1672: the fast grids.
        assert!(matches!(
            resolve("fast_grid_1"),
            Message::Ui(UiMsg::FastGridPicked(0))
        ));
        assert!(matches!(
            resolve("fast_grid_2"),
            Message::Ui(UiMsg::FastGridPicked(1))
        ));
        assert!(matches!(
            resolve("cycle_fast_grid"),
            Message::Ui(UiMsg::FastGridCycled)
        ));
    }

    /// #1301 / #1262: the placed-part command ids reach the library's
    /// placed-part action, not a stub.
    #[test]
    fn the_placed_part_ids_reach_their_actions() {
        use crate::library::{LibraryMessage, PlacedPartAction, PlacedPartMsg};
        fn resolve(id: &str) -> Message {
            let command = AppCommandId::new(id).expect("a catalog id is a valid AppCommandId");
            core_to_message(&command, &CommandArgs::none())
                .unwrap_or_else(|| panic!("'{id}' no longer resolves"))
        }
        let action = |id: &str| match resolve(id) {
            Message::Library(LibraryMessage::PlacedPart(PlacedPartMsg::ActionRequested(a))) => a,
            other => panic!("'{id}' resolved to {other:?}"),
        };
        assert_eq!(action("open_datasheet"), PlacedPartAction::OpenDatasheet);
        assert_eq!(action("edit_library_symbol"), PlacedPartAction::EditSymbol);
        assert_eq!(
            action("edit_selected_symbol_in_symbol_editor"),
            PlacedPartAction::EditSymbol
        );
    }

    /// #1670: the in-place edit ids ask the in-place editor for their field.
    #[test]
    fn the_field_edit_ids_request_their_target() {
        let target = |id: &str| {
            let command = AppCommandId::new(id).expect("a catalog id is a valid AppCommandId");
            match core_to_message(&command, &CommandArgs::none()) {
                Some(Message::TextEdit(TextEditMsg::Requested(target))) => target,
                other => panic!("'{id}' resolved to {other:?}"),
            }
        };
        assert_eq!(target("edit_value"), TextEditTarget::Value);
        assert_eq!(
            target("edit_reference_designator"),
            TextEditTarget::Designator
        );
        assert_eq!(
            target("edit_footprint_field"),
            TextEditTarget::FootprintField
        );
        assert_eq!(target("edit_text_in_place"), TextEditTarget::Text);
    }

    /// #103 - the cheap shortcuts that used to be dead keys now reach
    /// the handlers that already exist.
    #[test]
    fn the_103_shortcuts_reach_their_named_messages() {
        fn resolve(id: &str) -> Message {
            let command = AppCommandId::new(id).expect("a catalog id is a valid AppCommandId");
            core_to_message(&command, &CommandArgs::none())
                .unwrap_or_else(|| panic!("'{id}' no longer resolves"))
        }

        assert!(matches!(
            resolve("zoom_in_at_cursor"),
            Message::Menu(MenuMessage::ZoomIn)
        ));
        assert!(matches!(
            resolve("zoom_out_at_cursor"),
            Message::Menu(MenuMessage::ZoomOut)
        ));
        assert!(matches!(
            resolve("next_sheet"),
            Message::Menu(MenuMessage::NextSheet)
        ));
        assert!(matches!(
            resolve("previous_sheet"),
            Message::Menu(MenuMessage::PreviousSheet)
        ));
        assert!(matches!(
            resolve("unselect_all"),
            Message::Selection(selection_request::SelectionRequest::ClearAll)
        ));
    }

    /// #271 — `move_object` (the `M M` chord) resolves to the interactive
    /// Move, not to Move Selection or an unmapped no-op.
    #[test]
    fn move_object_resolves_to_the_interactive_move_action() {
        let command = AppCommandId::new("move_object").expect("a catalog id is valid");
        assert!(matches!(
            core_to_message(&command, &CommandArgs::none()),
            Some(Message::ActiveBar(crate::active_bar::ActiveBarMsg::Action(
                crate::active_bar::ActiveBarAction::Move
            )))
        ));
    }

    /// #1473 — the courtyard-outline command id reaches the footprint
    /// editor and is offered by the palette (it is dispatchable).
    #[test]
    fn courtyard_outline_command_resolves_to_the_footprint_message() {
        let command = AppCommandId::new("footprint_recompute_courtyard_outline")
            .expect("a catalog id is valid");
        assert!(is_dispatchable(&command));
        assert!(matches!(
            core_to_message(&command, &CommandArgs::none()),
            Some(Message::Input(InputMsg::FootprintCourtyardOutlineRequested))
        ));
    }

    /// Mockup row 175: the 3D view toolbar's Flip, lighting and screenshot
    /// buttons as commands reach the 3D view's own requests.
    #[test]
    fn the_board_3d_commands_reach_the_3d_view() {
        use crate::app::Board3dMsg;

        for (id, want) in [
            ("flip_board_3d", Board3dMsg::FlipRequested),
            ("cycle_board_3d_lighting", Board3dMsg::LightingCycled),
            (
                "copy_board_3d_screenshot",
                Board3dMsg::ScreenshotCopyRequested,
            ),
        ] {
            let command = AppCommandId::new(id).expect("a catalog id is valid");
            assert!(is_dispatchable(&command), "{id}");
            match core_to_message(&command, &CommandArgs::none()) {
                Some(Message::Pcb(PcbMsg::Board3d(got))) => assert_eq!(got, want, "{id}"),
                other => panic!("{id} resolved to {other:?}"),
            }
        }
    }

    /// Mockup row 175: on a board, `H` steps the 3D view's lighting and
    /// Ctrl+Shift+C (Cmd+Shift+C on macOS) copies its screenshot, in both
    /// built-in profiles; `F` stays Change Footprint on a flat board and
    /// turns the board over while its 3D view is shown (the `board_3d`
    /// context, Caner 2026-11-24).
    #[test]
    fn the_board_3d_keys_run_their_commands_on_a_board() {
        use std::str::FromStr;

        use crate::keymap::{
            CompiledKeymap, KeyStroke, Platform, ShortcutContext, ShortcutProfileSet,
        };

        let on_a_board = [ShortcutContext::Global, ShortcutContext::Pcb];
        let in_3d = [
            ShortcutContext::Global,
            ShortcutContext::Pcb,
            ShortcutContext::Board3d,
        ];
        let set = ShortcutProfileSet::built_ins().expect("built-in profiles");
        for profile in set.profiles() {
            for (platform, copy) in [
                (Platform::Windows, "Ctrl+Shift+C"),
                (Platform::Linux, "Ctrl+Shift+C"),
                (Platform::MacOs, "Cmd+Shift+C"),
            ] {
                let keymap = CompiledKeymap::compile_for(profile, platform);
                for (contexts, key, want) in [
                    (&on_a_board[..], "H", "cycle_board_3d_lighting"),
                    (&on_a_board[..], copy, "copy_board_3d_screenshot"),
                    (&on_a_board[..], "F", "change_footprint"),
                    (&in_3d[..], "H", "cycle_board_3d_lighting"),
                    (&in_3d[..], copy, "copy_board_3d_screenshot"),
                    (&in_3d[..], "F", "flip_board_3d"),
                ] {
                    let stroke = KeyStroke::from_str(key).expect("a key");
                    let got = keymap.lookup(&[stroke], contexts).command;
                    assert_eq!(
                        got.as_ref().map(AppCommandId::as_str),
                        Some(want),
                        "{} on {platform:?}: {key} in {contexts:?}",
                        profile.id
                    );
                }
            }
        }
    }

    /// The pinned ids must still exist in the catalog, so the list cannot
    /// rot into a set of names that no longer mean anything.
    #[test]
    fn pinned_unmapped_ids_still_exist_in_the_catalog() {
        let catalog: HashSet<&str> = crate::keymap::all_command_ids().collect();
        let stale: Vec<&str> = UNMAPPED_CATALOG_IDS
            .iter()
            .copied()
            .filter(|id| !catalog.contains(id))
            .collect();
        assert!(
            stale.is_empty(),
            "UNMAPPED_CATALOG_IDS names ids that are no longer in the \
             catalog: {stale:?}. Remove them from the pinned list."
        );
    }

    /// The diagnostics ring is one process-wide `Mutex<VecDeque<_>>`
    /// (`diagnostics.rs`), and cargo runs the `signex-app --lib` tests in
    /// parallel threads. The two reporting tests below both read it around
    /// an action, so they have to take turns — otherwise one sees the
    /// other's record land between its `before` and its assert.
    ///
    /// Poison is recovered rather than propagated: a panic in one of these
    /// two is a real failure to report, not a reason to fail the other one
    /// for an unrelated reason.
    fn serial() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Records currently in the ring that name this command id.
    ///
    /// Counting *total* entries instead would make these tests depend on
    /// whatever else the binary happened to log in a neighbouring thread —
    /// which is exactly how the first version of
    /// `asking_whether_a_command_resolves_reports_nothing` failed in CI
    /// (`left: 2, right: 1`). The claim under test is about records naming
    /// this id, so count those.
    fn records_naming(id: &str) -> usize {
        let needle = format!("command '{id}'");
        crate::diagnostics::recent_entries()
            .iter()
            .filter(|entry| entry.message.contains(&needle))
            .count()
    }

    /// #619 — a query must not narrate. The palette filters its rows by
    /// asking whether each catalog id resolves; routing that through the
    /// dispatch entry point put one warning per unmapped id into the
    /// Messages panel on every rebuild — 2432 records in one observed
    /// session, against a 200-entry ring buffer, so every real
    /// diagnostic was evicted before it could be read.
    #[test]
    fn asking_whether_a_command_resolves_reports_nothing() {
        // Arrange — an id the catalog carries with no dispatch arm, i.e.
        // exactly what the palette filters out.
        let _guard = serial();
        let _ = crate::diagnostics::init_logging();
        let id: &str = UNMAPPED_CATALOG_IDS
            .first()
            .expect("the pinned list is non-empty while any id is unmapped");
        let command = AppCommandId::new(id).expect("a pinned id is a valid command id");
        let before = records_naming(id);

        // Act — the query, run as often as a few palette rebuilds would.
        for _ in 0..25 {
            assert!(
                !is_dispatchable(&command),
                "{id} is pinned as unmapped, so it must not resolve"
            );
        }

        // Assert
        assert_eq!(
            records_naming(id),
            before,
            "asking whether `{id}` resolves must leave the Messages panel untouched"
        );
    }

    /// #1350 - Backspace/Delete mid-wire pops the pending vertex; it must not
    /// be a plain delete-selection alias.
    #[test]
    fn remove_last_vertex_ends_a_wire_draw_instead_of_deleting() {
        let (mut app, _task) = Signex::new();
        let cmd = AppCommandId::new("remove_last_vertex").unwrap();
        let msg = core_to_message(&cmd, &CommandArgs::none()).unwrap();
        assert!(matches!(msg, Message::Tool(ToolMessage::RemoveLastVertex)));
        assert!(app.interaction_state.active_canvas().selected.is_empty());
        app.interaction_state.wire_drawing = true;
        app.interaction_state
            .wire_points
            .push(signex_types::schematic::Point::new(0.0, 0.0));

        let _ = app.dispatch_command(&cmd, crate::app::command::args::CommandArgs::none(), None);

        assert!(!app.interaction_state.wire_drawing);
        assert!(app.interaction_state.wire_points.is_empty());
    }

    /// A schematic tab whose one wire is selected, with no draw in progress.
    fn app_with_a_selected_wire() -> (Signex, std::path::PathBuf) {
        use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind, Wire};
        let (mut app, _task) = Signex::new();
        let path = std::path::PathBuf::from("vertex.snxsch");
        let wire = uuid::Uuid::from_u128(5);
        let mut engine = signex_engine::Engine::new(SchematicSheet::default()).unwrap();
        engine
            .execute(signex_engine::Command::PlaceWireSegment {
                wire: Wire {
                    uuid: wire,
                    start: Point::new(10.16, 10.16),
                    end: Point::new(20.32, 10.16),
                    ..Default::default()
                },
            })
            .unwrap();
        app.document_state.engines.insert(path.clone(), engine);
        app.document_state.tabs.push(crate::app::TabInfo {
            title: "vertex".to_string(),
            path: path.clone(),
            project_id: None,
            kind: crate::app::TabKind::Schematic,
        });
        app.document_state.active_tab = app.document_state.tabs.len() - 1;
        app.document_state.active_path = Some(path.clone());
        app.interaction_state.active_canvas_mut().selected =
            vec![SelectedItem::new(wire, SelectedKind::Wire)];
        (app, path)
    }

    fn wire_count(app: &Signex, path: &std::path::Path) -> usize {
        app.document_state.engines[path].document().wires.len()
    }

    /// #1669 - outside a draw `undo_last_segment` is disabled and deletes
    /// nothing, even with a selection; the sheet stays clean.
    #[test]
    fn undo_last_segment_outside_a_draw_is_disabled_and_deletes_nothing() {
        let (mut app, path) = app_with_a_selected_wire();
        let cmd = AppCommandId::new("undo_last_segment").unwrap();
        let revision = app.document_state.engines[&path].revision();

        let (outcome, _task) = app.dispatch_command_reporting(&cmd, CommandArgs::none(), None);

        assert!(
            matches!(outcome, super::super::CommandOutcome::Disabled { .. }),
            "{outcome:?}"
        );
        // Invoked anyway (a message that skips the gate), it still does nothing.
        let _ = app.update(Message::Tool(ToolMessage::LastSegmentUndoRequested));
        assert_eq!(wire_count(&app, &path), 1);
        assert_eq!(app.document_state.engines[&path].revision(), revision);
        assert!(!app.document_state.has_dirty_documents());
        assert_eq!(app.interaction_state.active_canvas().selected.len(), 1);
    }

    /// Trunk behaviour pinned: `remove_last_vertex` with a selection and no
    /// draw deletes the selection.
    #[test]
    fn remove_last_vertex_outside_a_draw_deletes_the_selection() {
        let (mut app, path) = app_with_a_selected_wire();
        let cmd = AppCommandId::new("remove_last_vertex").unwrap();

        let _ = app.dispatch_command(&cmd, CommandArgs::none(), None);

        assert_eq!(wire_count(&app, &path), 0);
    }

    /// #1669 - the Classic profile's Backspace, `undo_last_segment`, removes
    /// the pending vertex of a wire draw, ending it.
    #[test]
    fn undo_last_segment_removes_the_pending_vertex_while_drawing() {
        let (mut app, _task) = Signex::new();
        let cmd = AppCommandId::new("undo_last_segment").unwrap();
        app.interaction_state.wire_drawing = true;
        app.interaction_state
            .wire_points
            .push(signex_types::schematic::Point::new(0.0, 0.0));

        let _ = app.dispatch_command(&cmd, CommandArgs::none(), None);

        assert!(!app.interaction_state.wire_drawing);
        assert!(app.interaction_state.wire_points.is_empty());
    }

    /// #1350 - Backspace mid-polyline pops the last buffered point and
    /// leaves the selection alone.
    #[test]
    fn remove_last_vertex_pops_a_polyline_point_and_keeps_selection() {
        let (mut app, _task) = Signex::new();
        let cmd = AppCommandId::new("remove_last_vertex").unwrap();
        let pts = vec![
            signex_types::schematic::Point::new(0.0, 0.0),
            signex_types::schematic::Point::new(5.0, 0.0),
        ];
        app.interaction_state.polyline_points = pts.clone();
        app.interaction_state.active_canvas_mut().polyline_points = pts;
        app.interaction_state.active_canvas_mut().selected.push(
            signex_types::schematic::SelectedItem::new(
                uuid::Uuid::now_v7(),
                signex_types::schematic::SelectedKind::Wire,
            ),
        );

        let _ = app.dispatch_command(&cmd, crate::app::command::args::CommandArgs::none(), None);

        assert_eq!(app.interaction_state.polyline_points.len(), 1);
        assert_eq!(
            app.interaction_state.active_canvas().polyline_points.len(),
            1
        );
        assert_eq!(app.interaction_state.active_canvas().selected.len(), 1);
    }

    /// #1247 - the update-PCB shortcut must not be an alias for ERC. It opens
    /// the Update PCB dialog (#1312), while Run ERC keeps its own arm.
    #[test]
    fn update_pcb_from_schematic_does_not_run_erc() {
        let update_pcb = AppCommandId::new("update_pcb_from_schematic").unwrap();
        let run_erc = AppCommandId::new("run_erc").unwrap();

        assert!(matches!(
            resolve(&update_pcb, &CommandArgs::none()),
            Some(Message::UpdatePcb(UpdatePcbMsg::Requested { apply: false }))
        ));
        assert!(matches!(
            resolve(&run_erc, &CommandArgs::none()),
            Some(Message::Erc(ErcMsg::Run))
        ));
    }

    /// #1991 - Project > Electrical Rules Check opens the ERC dialog through
    /// its own id; `run_erc` (F8) keeps running ERC.
    #[test]
    fn open_erc_dialog_opens_the_dialog_and_run_erc_runs() {
        let open = AppCommandId::new("open_erc_dialog").unwrap();
        let run = AppCommandId::new("run_erc").unwrap();
        assert!(matches!(
            resolve(&open, &CommandArgs::none()),
            Some(Message::Erc(ErcMsg::OpenDialog))
        ));
        assert!(matches!(
            resolve(&run, &CommandArgs::none()),
            Some(Message::Erc(ErcMsg::Run))
        ));
    }

    /// #1245 - the catalog command that opens the Parameter Manager resolves
    /// to its `Open` message, so the palette and a keymap binding reach it.
    #[test]
    fn open_parameter_manager_resolves_to_the_open_message() {
        let id = AppCommandId::new("open_parameter_manager").unwrap();
        assert!(is_dispatchable(&id));
        assert!(matches!(
            resolve(&id, &CommandArgs::none()),
            Some(Message::ParameterManager(ParameterManagerMsg::Open))
        ));
    }

    /// #1295 - the palette's Export… and Import… of the Parameter Manager
    /// reach the same messages as its footer buttons.
    #[test]
    fn export_and_import_parameters_resolve_to_the_footer_messages() {
        use crate::app::parameter_io::ParameterIoMsg;
        let export = AppCommandId::new("export_parameters").unwrap();
        let import = AppCommandId::new("import_parameters").unwrap();
        assert!(is_dispatchable(&export) && is_dispatchable(&import));
        assert!(matches!(
            resolve(&export, &CommandArgs::none()),
            Some(Message::ParameterManager(ParameterManagerMsg::Io(
                ParameterIoMsg::ExportRequested
            )))
        ));
        assert!(matches!(
            resolve(&import, &CommandArgs::none()),
            Some(Message::ParameterManager(ParameterManagerMsg::Io(
                ParameterIoMsg::ImportRequested
            )))
        ));
    }

    /// #103 - ids that gained an existing handler resolve; the rest of the
    /// pinned set stays unresolved.
    #[test]
    fn the_103_wired_ids_resolve_to_their_handlers() {
        for (id, expect_properties) in [
            ("edit_object_properties", true),
            ("leave_sheet", false),
            ("navigate_up_hierarchy", false),
        ] {
            let command = AppCommandId::new(id).unwrap();
            let message = resolve(&command, &CommandArgs::none())
                .unwrap_or_else(|| panic!("'{id}' must resolve"));
            if expect_properties {
                assert!(matches!(
                    message,
                    Message::Menu(MenuMessage::OpenPropertiesPanel)
                ));
            } else {
                assert!(matches!(
                    message,
                    Message::Menu(MenuMessage::AscendToParentSheet)
                ));
            }
        }
        for id in UNMAPPED_CATALOG_IDS {
            assert!(!is_dispatchable(&AppCommandId::new(*id).unwrap()), "{id}");
        }
    }

    /// #1256 / #1257 / #1266 - the canvas commands resolve to their messages.
    #[test]
    fn the_canvas_commands_resolve() {
        let resolve_id = |id: &str| resolve(&AppCommandId::new(id).unwrap(), &CommandArgs::none());
        assert!(matches!(
            resolve_id("measure_distance"),
            Some(Message::Tool(ToolMessage::SelectTool(Tool::Measure)))
        ));
        assert!(matches!(
            resolve_id("find_similar_objects"),
            Some(Message::Selection(
                selection_request::SelectionRequest::FindSimilar(
                    crate::find_similar::FindSimilarMsg::OpenRequested { at: None }
                )
            ))
        ));
        assert!(matches!(
            resolve_id("drag_keep_connections"),
            Some(Message::Selection(
                selection_request::SelectionRequest::DragKeepingConnections
            ))
        ));
        assert!(matches!(
            resolve_id("break_wire"),
            Some(Message::Tool(ToolMessage::SelectTool(Tool::BreakWire)))
        ));
        assert!(matches!(
            resolve_id("zoom_to_selection_area"),
            Some(Message::CanvasEvent(CanvasEvent::FitSelection))
        ));
    }

    /// #1669 - `cycle_wiring_mode` resolves to its tool message and steps the
    /// drag wiring mode through all six (the 45° two since #2516) and back
    /// to auto.
    #[test]
    fn cycle_wiring_mode_resolves_and_cycles_the_drag_wiring_mode() {
        use signex_types::corner::DragWiringMode;
        let cmd = AppCommandId::new("cycle_wiring_mode").unwrap();
        assert!(matches!(
            resolve(&cmd, &CommandArgs::none()),
            Some(Message::Tool(ToolMessage::DragWiringCycleRequested))
        ));
        let (mut app, _task) = Signex::new();
        assert_eq!(app.interaction_state.drag_wiring, DragWiringMode::Auto);
        let mut seen = Vec::new();
        for _ in 0..6 {
            let _ = app.dispatch_command(&cmd, CommandArgs::none(), None);
            seen.push(app.interaction_state.drag_wiring);
        }
        assert_eq!(
            seen,
            [
                DragWiringMode::HorizontalFirst,
                DragWiringMode::VerticalFirst,
                DragWiringMode::Straight,
                DragWiringMode::Angle45Start,
                DragWiringMode::Angle45End,
                DragWiringMode::Auto
            ]
        );
    }

    /// #103 - pressing an unmapped command's key says "<label> is not
    /// available yet" in the Messages panel instead of doing nothing.
    #[test]
    fn an_unmapped_command_reports_that_it_is_not_available_yet() {
        let _guard = serial();
        let _ = crate::diagnostics::init_logging();
        let command = AppCommandId::new("reset_local_coordinates").unwrap();

        assert!(core_to_message(&command, &CommandArgs::none()).is_none());

        assert!(crate::diagnostics::recent_entries().iter().any(|entry| {
            entry.message.contains("command 'reset_local_coordinates'")
                && entry
                    .message
                    .contains("Reset local coordinates is not available yet")
        }));
    }

    /// ADR-0006 slice 7: a keymap-profile typo is warned about with the
    /// catalog ids close to it.
    #[test]
    fn an_unknown_id_warns_with_the_close_catalog_ids() {
        let _guard = serial();
        let _ = crate::diagnostics::init_logging();
        let command = AppCommandId::new("plce_wire").unwrap();

        assert!(core_to_message(&command, &CommandArgs::none()).is_none());

        let want = "command 'plce_wire' is not in the command catalog — check the id in \
                    your keyboard-shortcuts profile; did you mean `place_wire`";
        assert!(
            crate::diagnostics::recent_entries()
                .iter()
                .any(|entry| entry.message.starts_with(want)),
            "no warning starting {want:?}"
        );
    }

    /// The other half: dispatching an unmapped command still reports, so
    /// the split silenced the query and not the failure.
    #[test]
    fn dispatching_an_unmapped_command_still_reports_it() {
        // Arrange
        let _guard = serial();
        let _ = crate::diagnostics::init_logging();
        let id: &str = UNMAPPED_CATALOG_IDS
            .first()
            .expect("the pinned list is non-empty while any id is unmapped");
        let command = AppCommandId::new(id).expect("a pinned id is a valid command id");
        let before = records_naming(id);

        // Act
        assert!(core_to_message(&command, &CommandArgs::none()).is_none());

        // Assert — exactly one new record, so a future change that reports
        // the same failure twice is caught too.
        assert_eq!(
            records_naming(id),
            before + 1,
            "dispatching `{id}` must still say it did nothing"
        );
    }
    /// The invocation context travels in `CommandArgs`: the click point seeds
    /// Find Similar, and a project id sends Project Options to that project.
    #[test]
    fn command_args_reach_the_commands_that_read_them() {
        let resolve_with = |id: &str, args: &CommandArgs| {
            resolve(&AppCommandId::new(id).unwrap(), args).unwrap_or_else(|| panic!("{id}"))
        };
        assert!(matches!(
            resolve_with(
                "find_similar_objects",
                &CommandArgs::in_context(InvocationContext {
                    origin: Some((5.0, 20.0)),
                    project: None
                })
            ),
            Message::Selection(selection_request::SelectionRequest::FindSimilar(
                crate::find_similar::FindSimilarMsg::OpenRequested {
                    at: Some((5.0, 20.0))
                }
            ))
        ));
        // #2419: Highlight Net from a context menu names the opening point.
        assert!(matches!(
            resolve_with(
                "highlight_net_under_cursor",
                &CommandArgs::in_context(InvocationContext {
                    origin: Some((5.0, 20.0)),
                    project: None
                })
            ),
            Message::Ui(UiMsg::NetNav(NetNavMsg::NetHighlightAtRequested {
                at: (5.0, 20.0)
            }))
        ));
        assert!(matches!(
            resolve_with("open_project_options", &CommandArgs::none()),
            Message::Menu(MenuMessage::OpenProjectOptions)
        ));
        match resolve_with(
            "open_project_options",
            &CommandArgs::in_context(InvocationContext {
                origin: None,
                project: Some("p1".to_string()),
            }),
        ) {
            Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
                crate::app::ProjectTreeAction::OpenProjectOptions(key),
            )) => assert_eq!(key, ["p1"]),
            other => panic!("resolved to {other:?}"),
        }
    }
}
