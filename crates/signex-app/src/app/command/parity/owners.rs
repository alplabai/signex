//! The owner of a parity row, from what its route writes (#2551), by the
//! D / S / C / X rule recorded on #2326 ("Slice C: kill-check decisions"):
//! a route that writes document state (D) is `core`; one that writes only
//! session state (S) or machine settings (C) is `gui`, the adapter's; one
//! that chooses between a D-write and an S-write by what is in hand is
//! `split`, a GUI router with one core command behind it.
//!
//! The routing code says what some forms write ([`direct`]); the handlers'
//! writes are annotated here by hand ([`WRITES`]); a refusal or a headless
//! row takes the owner of the route it stands in front of ([`inherited`]).
//! Only the synchronous route counts: a dialog, a panel or a file picker it
//! opens is session state, and what the dialog's answer does later is not
//! the row (the module note of `parity`).

use super::forms::{Form, Owner};

/// What a handler's synchronous route writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Writes {
    /// Document or project content, history, dirty, owed work, check results.
    Document,
    /// Selection, clipboard, cursor, tool, gesture, camera, a window, a
    /// dialog or a panel.
    Session,
    /// Machine settings, written to the prefs: the adapter's (ADR-0006 D7).
    Settings,
    /// Branches on what is in hand between a D-write and an S-write.
    Split,
    /// Waits for a maintainer decision (`BoardSketchSession`, #2326 kill-check
    /// decision 8).
    Pending,
}

impl Writes {
    fn owner(self) -> Owner {
        match self {
            Self::Document => Owner::Core,
            Self::Session | Self::Settings => Owner::Gui,
            Self::Split => Owner::Split,
            Self::Pending => Owner::Undecided,
        }
    }
}

/// The surface of an annotated route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Surface {
    /// A key, menu row, palette, context menu or Active Bar route: the
    /// `bridge`, `bridge_context_menu` or `active_bar` row.
    Menu,
    /// The operand handler.
    Operand,
    /// One form only.
    Only(Form),
}

impl Surface {
    pub(super) fn covers(self, form: Form) -> bool {
        match self {
            Self::Menu => matches!(
                form,
                Form::Bridge | Form::BridgeContextMenu | Form::ActiveBar
            ),
            Self::Operand => form == Form::Operand,
            Self::Only(only) => only == form,
        }
    }
}

/// How a row's owner was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OwnerBasis {
    /// The form says what it writes.
    Form,
    /// [`WRITES`] annotates the handler.
    Annotated,
    /// The owner of the route the row stands in front of.
    Inherited,
}

/// The owner of the row `(id, form)` and its basis; `siblings` are the
/// forms of the id's other rows; `arms` is whether its bridge arm only arms a
/// placement tool. `None`: no basis, which `every_row_has_an_owner_basis`
/// refuses.
pub(super) fn owner(
    id: &str,
    form: Form,
    siblings: &[Form],
    arms: bool,
) -> Option<(Owner, OwnerBasis)> {
    if let Some(writes) = annotated(id, form) {
        return Some((writes.owner(), OwnerBasis::Annotated));
    }
    if let Some(owner) = direct(form, arms) {
        return Some((owner, OwnerBasis::Form));
    }
    let behind = inherited(form)?
        .iter()
        .find(|candidate| siblings.contains(candidate))?;
    let (owner, _) = owner(id, *behind, &[], arms)?;
    Some((owner, OwnerBasis::Inherited))
}

/// The annotated writes of `(id, form)`.
pub(super) fn annotated(id: &str, form: Form) -> Option<Writes> {
    WRITES
        .iter()
        .find(|(ids, surface, _)| surface.covers(form) && ids.contains(&id))
        .map(|(_, _, writes)| *writes)
}

/// What the form writes, when the routing code says it.
fn direct(form: Form, arms: bool) -> Option<Owner> {
    match form {
        // Which window, which document of it, which undo target: the GUI
        // adapter's routing, as is laying the windows out.
        f if f.needs_a_window() => Some(Owner::Gui),
        Form::AgentWindowLayout => Some(Owner::Gui),
        // Answers from D: a core query (rule 5).
        Form::Query | Form::Check => Some(Owner::Core),
        // The bridge arm only arms a placement tool: tool state.
        Form::Bridge | Form::ActiveBar if arms => Some(Owner::Gui),
        // The core command behind a split router.
        Form::SplitCore => Some(Owner::Core),
        // Reads the selection into operands and writes nothing itself: the
        // write is the id's `operand_handler` row, the core's.
        Form::Translator => Some(Owner::Gui),
        _ => None,
    }
}

/// The routes a refusal or a headless row stands in front of, in order.
fn inherited(form: Form) -> Option<&'static [Form]> {
    const GATED: &[Form] = &[
        Form::Bridge,
        Form::Translator,
        Form::ActiveBar,
        Form::Operand,
        Form::Query,
    ];
    const HEADLESS: &[Form] = &[
        Form::Operand,
        Form::Query,
        Form::Check,
        Form::Bridge,
        Form::ActiveBar,
    ];
    match form {
        Form::Needs | Form::StillRunsUnavailable | Form::StillRunsRefusal => Some(GATED),
        Form::HeadlessRefusal
        | Form::HeadlessMissingOperand
        | Form::HeadlessWithoutOperands
        | Form::HeadlessNewDocument => Some(HEADLESS),
        _ => None,
    }
}

use Surface::{Menu, Operand};
use Writes::{Document, Pending, Session, Settings, Split};

/// `(ids, surface, writes)`, read from the handlers.
pub(super) const WRITES: &[(&[&str], Surface, Writes)] = &[
    (MENU_DOCUMENT, Menu, Document),
    // #2326 kill-check decision 3: the turn ids stay GUI router ids; smart
    // paste and paste special split; cut copies then deletes; duplicate
    // carries the copy (sheet) or pastes it beside (board); Enter commits a
    // polyline or selects inside a lasso.
    (
        &[
            "rotate_clockwise",
            "rotate_counterclockwise",
            "mirror_x",
            "mirror_y",
            "rotate_selection",
            "rotate_selection_cw",
            "flip_selected_x",
            "flip_selected_y",
            "flip_selection",
            "smart_paste",
            "paste_special",
            "cut",
            "duplicate",
            "placement_accept",
        ],
        Menu,
        Split,
    ),
    // Smart Paste from operands (#2665) pastes the person's copy, which the
    // window holds: it splits as the menu route does.
    (&["smart_paste"], Operand, Split),
    (&["board_sketch"], Menu, Pending),
    // The layer strip's name style and numbers, written to the prefs.
    (&["toggle_layer_numbers"], Menu, Settings),
    // The live ERC preference (#1288), written to the prefs, from a key or
    // the palette and from its operand.
    (&["toggle_live_erc"], Menu, Settings),
    (&["toggle_live_erc"], Operand, Settings),
    // Applying or resetting a workspace layout arranges the panels and
    // saves the dock layout and the layout list to the prefs (mockup row
    // 166); Save and Manage only open their dialogs.
    (
        &[
            "apply_layout_schematic",
            "apply_layout_board",
            "apply_layout_review",
            "apply_layout_notebook",
            "reset_workspace_layout",
        ],
        Menu,
        Settings,
    ),
    (MENU_SESSION, Menu, Session),
    // An unmapped id: no route but the headless refusal.
    (
        &["reset_local_coordinates"],
        Surface::Only(Form::HeadlessRefusal),
        Session,
    ),
    // The board's view (`ui_state.pcb_layer_views`), the selection, the
    // selection filter, the view angle (#2550) and the DRC panel's list:
    // never the board, no undo step.
    (
        &[
            "select_board_net",
            "highlight_board_net",
            "set_selection_filters",
            "apply_selection_filter_preset",
            "view_rotate",
            "show_drc_violation",
            "filter_drc_results",
            "set_active_layer",
            "highlight_layer",
            "hide_layer",
            "set_layer_visible",
            "show_used_layers_only",
            "set_object_visible",
            "set_object_transparency",
            "hide_layer_group",
            "show_layer_group",
            "show_layer_set",
            // #3013: a library's Document Options, in memory for the session.
            "set_library_document_options",
            // #2795: the notebook tab's view, side panel and cursor.
            "notebook_view",
        ],
        Operand,
        Session,
    ),
    // Mockup rows 196 and 197: the notebook reads answer from a notebook
    // read of their own, which is the Annotations panel's scan state.
    (
        &[
            "notebook_references",
            "notebook_reference_card",
            "notebook_reference_picker",
            "notebook_badges",
        ],
        Operand,
        Session,
    ),
    // Saved layer sets and the tab-name style: machine settings.
    (
        &["save_layer_set", "delete_layer_set", "set_layer_tab_names"],
        Operand,
        Settings,
    ),
    // A custom keyboard profile, written to `keyboard_shortcuts.toml` (#2495).
    (&["keymap_assign", "keymap_clear"], Operand, Settings),
    // A dock panel by its key (#2470): the dock, saved to the prefs as the
    // panel list's open and close are.
    (&["set_dock_panel"], Operand, Settings),
    (OPERAND_DOCUMENT, Operand, Document),
    // A read of the named board answered as a query is (#2939): it waits
    // for the library models off the UI thread, so it is an operand
    // handler, and the core's like every query row.
    (&["query_part_heights"], Operand, Document),
    // Declared exception (#2326 slice 6.P2b): on a footprint editor tab the
    // Delete translator row deletes in the editor itself, a GUI route that
    // writes the footprint until slice S-L moves the editors onto the core.
    // Every other tab translates into the operand handler.
    (
        &["delete_selection"],
        Surface::Only(Form::Translator),
        Document,
    ),
];

/// Menu routes that write a document, a project or check results.
const MENU_DOCUMENT: &[&str] = &[
    "add_table_of_contents",
    "add_teardrops",
    "add_to_union",
    "align_bottom",
    "align_horizontal_centers",
    "align_left",
    "align_right",
    "align_to_grid",
    "align_top",
    "align_vertical_centers",
    "annotate_schematic_quietly",
    "autoplace_fields",
    "break_union",
    "bring_to_front",
    "clear_all_net_colors",
    "close_active_document",
    "close_all_documents",
    "compile_project",
    "distribute_horizontally",
    "distribute_vertically",
    "footprint_recompute_courtyard_outline",
    "force_annotate_all_schematics",
    "lock_board_items",
    "move_to_front",
    "new_block_diagram",
    "panelize_board",
    "redo",
    "remove_board_panel",
    "remove_from_union",
    "remove_teardrops",
    "repair_pin_junctions",
    "repour_all",
    "repour_selected",
    "reset_board_origin",
    "reset_layer_colors",
    "run_drc",
    "run_erc",
    "save_all",
    "save_document",
    "send_to_back",
    "snap_board_origin_to_corner",
    "stitch_vias",
    "sync_sheet_pins",
    "tune_diff_pair",
    "tune_net_length",
    "undo",
    "unfill_selected",
    "unlock_board_items",
];

/// Menu routes that write session state only: a dialog, a panel, a picker,
/// an armed tool, the view, the selection, the clipboard or a gesture.
const MENU_SESSION: &[&str] = &[
    "annotate_schematic",
    "arrange_in_box",
    "assign_footprints",
    "back_annotate_schematics",
    "board_assembly_drawing",
    "board_layer_stack",
    "board_report_bom",
    "board_report_components",
    "board_report_net_length",
    "board_report_netlist",
    "bring_to_front_of",
    "cancel_current_tool",
    "center_on_cursor",
    "center_view_at_cursor",
    "change_component",
    "clear_net_color",
    "clear_net_highlighting",
    "color_net",
    "compare_board_with_schematic",
    "copy",
    "copy_attributes_or_add_vertex",
    "copy_board_3d_screenshot",
    "create_rooms_from_sheets",
    "create_sheet_from_sheet_symbol",
    "create_sheet_symbol_from_sheet",
    "create_union",
    "cross_probe",
    "cycle_arc_mode",
    "cycle_board_3d_lighting",
    "cycle_board_snapping",
    "cycle_fast_grid",
    "cycle_route_mode",
    "cycle_selection_mode",
    "cycle_snap_grid_backward",
    "cycle_snap_grid_forward",
    "cycle_unit",
    "cycle_wire_bus_graphic_mode",
    "cycle_wiring_mode",
    "drag",
    "drag_keep_connections",
    "drag_selection",
    "draw_arc",
    "draw_bezier",
    "draw_bus",
    "draw_ellipse",
    "draw_elliptical_arc",
    "draw_full_circle",
    "draw_graphic_line",
    "draw_hierarchical_sheet",
    "draw_line",
    "draw_polygon",
    "draw_rectangle",
    "draw_round_rectangle",
    "draw_wire",
    "edit_footprint_field",
    "edit_layer_color",
    "edit_library_symbol",
    "edit_object_properties",
    "edit_reference_designator",
    "edit_selected_object_properties",
    "edit_selected_symbol_in_symbol_editor",
    "edit_text_in_place",
    "edit_value",
    "export_assembly_svg",
    "export_block_diagram",
    "export_board_pdf",
    "export_board_report",
    "export_board_step",
    "export_notebook_pdf",
    // #1295: a file dialog; the file and the import preview come later.
    "export_parameters",
    "fabrication_outputs",
    "fast_grid_1",
    "fast_grid_2",
    "find",
    "find_and_replace",
    "find_next",
    "find_previous",
    "find_similar_objects",
    "find_text",
    "flip_board_3d",
    "flip_board_view",
    "footprint_mode_pads",
    "footprint_mode_sketch",
    "footprint_mode_view_3d",
    // #2945: the Generate options dialog.
    "generate_design_notebook",
    "go_to_net_partner",
    "highlight_board_net",
    "highlight_net_under_cursor",
    "highlight_related_net_objects",
    "home_view",
    "import_dxf",
    "import_dxf_outline",
    "import_graphics",
    "import_parameters",
    "lasso_select",
    "leave_sheet",
    "manage_workspace_layouts",
    "move_object",
    "move_selection",
    "move_selection_xy",
    "navigate_up_hierarchy",
    "new_document",
    "new_pcb_document",
    "new_project_with_profile",
    "new_schematic_sheet",
    "next_document_tab",
    "next_grid",
    "next_highlighted_net_item",
    "next_layer",
    "next_sheet",
    "next_signal_layer",
    "number_schematic_sheets",
    "open_child_sheet",
    "open_command_palette",
    "open_compiled_design",
    "open_components_panel",
    "open_datasheet",
    "open_design_notebook",
    "open_diff_pair_classes",
    "open_document",
    "open_drc_panel",
    "open_erc_dialog",
    "open_grid_picker",
    "open_grid_properties",
    "open_harness_definitions",
    "open_library_browser",
    "open_net_classes",
    "open_net_classes_panel",
    "open_net_color_palette",
    "open_net_inspector",
    "open_parameter_manager",
    "open_preferences",
    "open_project_options",
    "open_rules",
    "open_schematic_preferences",
    "open_selection_filter",
    "paste",
    "paste_array",
    "place_aligned_dimension",
    "place_blanket",
    "place_bus",
    "place_bus_entry",
    "place_compile_mask",
    "place_component",
    "place_design_block",
    "place_device_sheet_symbol",
    "place_diff_pair",
    "place_global_label",
    "place_graphic",
    "place_harness_connector",
    "place_harness_entry",
    "place_hierarchical_label",
    "place_junction",
    "place_linear_dimension",
    "place_local_net_label",
    "place_net_label",
    "place_no_erc",
    "place_note",
    "place_off_sheet_connector",
    "place_parameter_set",
    "place_port",
    "place_power_symbol",
    "place_reuse_block",
    "place_sheet_entry",
    "place_sheet_symbol",
    "place_signal_harness",
    "place_snippet",
    "place_symbol",
    "place_table",
    "place_text",
    "place_text_frame",
    "place_text_string",
    "place_wire",
    "placement_properties",
    "polygon_manager",
    "prev_layer",
    "previous_document_tab",
    "previous_grid",
    "previous_highlighted_net_item",
    "previous_sheet",
    "print",
    "reannotate_board",
    "refresh_view",
    "remove_last_vertex",
    "repeat_last_item",
    "report_manager_bom",
    "reset_schematic_designators",
    "rubber_stamp_copy",
    "save_document_as",
    "save_project_as",
    "save_workspace_layout",
    "select_all",
    "select_board_net",
    "select_connection",
    "select_expand_connection",
    "select_node_or_connection_item",
    "send_to_back_of",
    "set_board_origin",
    "sheet_navigation_back",
    "sheet_navigation_forward",
    "show_all_design_objects",
    "show_all_layers",
    "show_current_command_hotkeys",
    "show_current_command_shortcuts",
    "switch_segment_posture",
    "sync_sheet_entries",
    "tile_windows_horizontally",
    "tile_windows_vertically",
    "toggle_auto_focus",
    "toggle_cross_select_mode",
    "toggle_electrical_grid",
    "toggle_floating_panels",
    "toggle_layer_visibility",
    "toggle_layers_panel",
    "toggle_notebook_badges",
    "toggle_origin_marker",
    "toggle_properties_panel",
    "toggle_ratsnest",
    "toggle_schematic_filter_panel",
    "toggle_schematic_list_panel",
    "toggle_search_panel",
    "toggle_selection",
    "toggle_single_layer_mode",
    "toggle_visible_grid",
    "tool_select",
    "undo_last_segment",
    "unselect_all",
    "untile_documents",
    "update_pcb_from_schematic",
    "view_board_3d",
    "view_rotate",
    "view_rotate_180",
    "view_rotate_ccw",
    "view_rotate_reset",
    "zoom_in_at_cursor",
    "zoom_out_at_cursor",
    "zoom_to_all_objects",
    "zoom_to_fit",
    "zoom_to_selection_area",
];

/// Operand handlers that write a document, a project or check results, or
/// write an export of one.
const OPERAND_DOCUMENT: &[&str] = &[
    "add_board_graphic",
    "add_net_class",
    "add_teardrops",
    "annotate_schematic_quietly",
    "apply_board_schematic_differences",
    "apply_edits",
    "apply_library_updates",
    "arrange_in_box",
    "arrange_in_room",
    "assign_board_net_class",
    "assign_footprint",
    "assign_net_class",
    "attach_notebook_file",
    "auto_place",
    "autoplace_fields",
    "block_add",
    "block_group",
    "block_group_set",
    "block_link",
    "block_link_set",
    "block_port_add",
    "block_port_remove",
    "block_remove",
    "block_set",
    "block_ungroup",
    "block_unlink",
    "back_annotate_schematics",
    "board_assembly_drawing",
    "board_report_bom",
    "board_report_components",
    "board_report_net_length",
    "board_report_netlist",
    "board_shape_from_selection",
    "bom_add_alternate",
    "bom_remove_alternate",
    "bom_set_alternate_order",
    "bring_drawings_forward",
    "bring_drawings_to_front",
    "change_component",
    "change_footprint",
    "clear_all_net_colors",
    "close_active_document",
    "color_net",
    "copy_stock_to_library",
    "create_rooms_from_sheets",
    "create_sheet_symbol_from_sheet",
    "delete_net_class",
    "delete_selection",
    "draw_bus",
    "draw_graphic_line",
    "draw_line",
    "draw_rectangle",
    "draw_wire",
    "edit_blanket",
    "edit_no_erc_rules",
    "export_assembly_svg",
    "export_block_diagram",
    "export_board_pdf",
    "export_board_report",
    "export_board_step",
    "export_notebook_pdf",
    "export_notebook_timeline",
    "fabrication_outputs",
    "fix_drc",
    "fix_drc_violation",
    "footprint_align_pads",
    "footprint_move_pads",
    "force_annotate_all_schematics",
    "generate_block_diagram",
    // #2945: the design report, written into the project with `save`.
    "generate_design_notebook",
    "gloss_selected_routes",
    "import_dxf",
    "import_dxf_outline",
    "import_graphics",
    "keepout_from_selection",
    "layout_plan_set",
    "lock_board_items",
    "mark_notebook_attachment_valid",
    "new_block_diagram",
    "new_document",
    "new_library",
    "new_pcb_document",
    "new_project_with_profile",
    "new_schematic_sheet",
    "notebook_compile",
    // Mockup row 196: the notebook tab's text, written on Save.
    "notebook_fix_reference",
    "notebook_insert",
    "open_document",
    "panelize_board",
    "place_blanket",
    "place_board_diff_pair",
    "place_board_fill",
    "place_board_keepout",
    "place_board_multi_route",
    "place_board_polygon_pour",
    "place_board_pour_cutout",
    "place_board_quick_route",
    "place_board_region",
    "place_board_reuse_block",
    "place_board_room",
    "place_board_route",
    "place_board_track",
    "place_board_via",
    "place_bus",
    "place_compile_mask",
    "place_component",
    "place_design_block",
    "place_diff_pair",
    "place_global_label",
    "place_hierarchical_label",
    "place_junction",
    "place_local_net_label",
    "place_net_label",
    "place_no_connect",
    "place_no_erc",
    "place_parameter_set",
    "place_power_symbol",
    "place_sheet_entry",
    "place_sheet_symbol",
    "place_snippet",
    "place_text",
    "place_text_string",
    "place_wire",
    "project_add_requirement",
    "project_add_variant",
    "project_duplicate_variant",
    "project_enable_requirement",
    "project_move_library",
    "project_move_sheet",
    "project_remove_requirement",
    "project_remove_variant",
    "project_rename_variant",
    "project_run_requirements",
    "project_set_active_variant",
    "project_set_annotate_numbering",
    "project_set_annotate_undo",
    "project_set_default_variant",
    "project_set_rail",
    "project_set_requirement",
    "project_set_variant_alternate",
    "project_set_variant_fit",
    "project_set_variant_override",
    "project_set_version_control",
    "reannotate_board",
    "recover_library",
    "redo",
    "regenerate_block_diagram",
    "remove_board_panel",
    "remove_teardrops",
    "rename_net_class",
    "repair_pin_junctions",
    "repour_all",
    "reset_board_origin",
    "reset_layer_colors",
    "revert",
    "rules_add",
    "rules_enable",
    "rules_matrix_set",
    "rules_preset_apply",
    "rules_remove",
    "rules_set",
    "rules_set_apply",
    "rules_set_create",
    "rules_set_edit",
    "save_board_reuse_block",
    "save_document",
    "save_document_as",
    "save_snippet",
    "send_drawings_backward",
    "send_drawings_to_back",
    "set_board_graphic",
    "set_board_panel",
    "set_footprint_position",
    "set_footprint_rotation",
    "set_footprint_side",
    "set_footprint_text",
    "set_layer_color",
    "set_layer_sizes",
    "set_layer_stack",
    "set_net_class_rules",
    "set_net_tie",
    "set_object_position",
    "set_object_rotation",
    "set_object_text",
    "set_pad_shape",
    "set_sheet_symbol_size",
    "set_sheet_variable",
    "set_symbol_parameter",
    "set_via_span",
    "stitch_vias",
    "swap_gates",
    "swap_pins",
    "swap_schematic_gates",
    "swap_schematic_pins",
    "sync_sheet_entries",
    "sync_sheet_pins",
    "tune_diff_pair",
    "tune_net_length",
    "undo",
    "unlock_board_items",
    "unwaive_drc_violation",
    "update_pcb_from_schematic",
    "waive_drc_violation",
];
