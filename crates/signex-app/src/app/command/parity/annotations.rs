//! Hand annotations of the parity table: what a route reads past its gate,
//! which the routing code does not say. Read from the handlers; a regenerate
//! (`UPDATE_GOLDEN=1`) keeps them, since they live here and not in the
//! golden. An empty list says the route was read and reads nothing implicit.
//!
//! Tool state or settings: a value the person flips while working (grid,
//! snap, Move Keeps Wires, the Shift+E hotspots) is tool state, even when it
//! is persisted. A saved library or profile value (machine rules, saved
//! layer sets, filter presets, new-project defaults, the locale where it
//! shapes what a command writes or answers) is settings. A translated
//! refusal, log line or undo label is not a read of the locale.

use super::forms::Form::Translator;
use super::forms::Form::{self, ActiveBar, Bridge, BridgeContextMenu, HeadlessWithoutOperands};
use super::forms::Form::{AgentWindowLayout, Check, CloseInWindow, HeadlessNewDocument};
use super::forms::Form::{Operand, Query, SplitCore, StillRunsRefusal, StillRunsUnavailable};
use super::inputs::Input::{self, *};

mod gerber_viewer;

/// `(ids, form, inputs)`: each id's row of `form` reads `inputs` too.
pub(super) const ANNOTATIONS: &[(&[&str], Form, &[Input])] = &[
    // Core edits and runs that read past their gate.
    // A project without rules runs with the machine's ERC overrides
    // (`erc_rule_settings`).
    (&["run_erc"], Bridge, &[RenderCache, Settings]),
    // The active document's project: its sheets to compile or to list.
    (
        &["compile_project", "add_table_of_contents"],
        Bridge,
        &[ActiveDocument],
    ),
    // #2968: the project in view becomes the `document` operand.
    (
        &["generate_block_diagram", "regenerate_block_diagram"],
        Translator,
        &[ActiveDocument],
    ),
    // #2968: the named document's project and its sheets, read off the UI
    // thread; a diagram open in a tab refuses a run that writes.
    (
        &["generate_block_diagram", "regenerate_block_diagram"],
        Operand,
        &[],
    ),
    // The canvas selection and the union picked in the Unions panel (mockup row 158).
    (
        &["add_to_union", "remove_from_union", "break_union"],
        Bridge,
        &[ActiveDocument, Selection],
    ),
    (
        &[
            "select_all",
            "zoom_to_fit",
            "zoom_to_all_objects",
            "show_all_design_objects",
            // Mockup row 194: Home ends in the same Fit.
            "home_view",
        ],
        Bridge,
        &[RenderCache],
    ),
    // The offset is the grid margin past the sheet's content.
    (
        &["smart_paste", "paste_special"],
        Bridge,
        &[Clipboard, RenderCache, ToolState],
    ),
    // The placement epoch and the grid of the pasted copy.
    (&["paste"], Bridge, &[Clipboard, ToolState]),
    (
        &["select_board_net", "highlight_board_net"],
        Bridge,
        &[Selection, Cursor],
    ),
    (
        &["select_board_net", "highlight_board_net"],
        Operand,
        &[Selection, Cursor],
    ),
    // The clearances come from `board_design_rules`, which falls back on the
    // machine rules (`drc_inputs`).
    (
        &["tune_net_length", "tune_diff_pair"],
        Operand,
        &[ActiveDocument, Selection, Settings],
    ),
    // The board tools' clearances: `run` -> `clearance_rules` ->
    // `board_design_rules` -> `drc_inputs` (board_tools, handlers/pcb_drc.rs).
    (
        &[
            "stitch_vias",
            "add_teardrops",
            "remove_teardrops",
            "gloss_selected_routes",
            "board_shape_from_selection",
            "keepout_from_selection",
            "arrange_in_box",
        ],
        Operand,
        &[Settings],
    ),
    // The PDF's printed text is in the UI locale (`print_text`); the sheet
    // PDF draws in the theme's palette (`sheet_pdf_options`).
    (
        &["export_board_pdf", "export_schematic_pdf"],
        Operand,
        &[Settings],
    ),
    // #1299: checks the job's project and starts its read; the build is later.
    // The tab's commands (mockup row 114) read the job's draft or its file.
    (
        &[
            "run_output_job",
            "new_output_job",
            "open_output_job",
            "read_output_job",
            "add_job_output",
            "set_job_output",
            "set_job_profile",
            "set_job_triggers",
            "stop_output_job",
            "view_output_job_run",
        ],
        Operand,
        &[],
    ),
    // The routing tools from their operands (#2470): the rules the route
    // keeps come from `board_design_rules` -> `drc_inputs`; none reads the
    // person's tool form, corner rule or route mode (handlers/pcb_tool/
    // route_order.rs). A via reads nothing past its operands and the board.
    (
        &[
            "place_board_track",
            "place_board_quick_route",
            "place_board_route",
            "place_board_diff_pair",
            "place_board_multi_route",
        ],
        Operand,
        &[Settings],
    ),
    (&["place_board_via"], Operand, &[]),
    // Mockup row 202: the stale check reads the named project's design on
    // the worker (`notebook_design_inputs`), its open sheets as edited; the
    // dialog opens on the project in view.
    (&["export_notebook_pdf"], Operand, &[]),
    (&["export_notebook_pdf"], Bridge, &[ActiveDocument]),
    // Save As of a background sheet is refused (`active_path`).
    (&["save_document_as"], Operand, &[ActiveDocument]),
    // The rfd picker (`save_as_dialog_for_active_tab`).
    (&["save_document_as"], Bridge, &[DialogAnswer]),
    // `close_tab` commits the sheet's title-block drafts and settles a board
    // sketch, whose refused bake keeps the tab; an agent's close of another
    // tab parks it in the background (`agent.serving`).
    (
        &["close_active_document"],
        Operand,
        &[ActiveDocument, InProgressEdit, Origin],
    ),
    (&["close_active_document"], Bridge, &[InProgressEdit]),
    (&["close_active_document"], CloseInWindow, &[InProgressEdit]),
    // The person busy on the sheet in any window (`person_busy_on`, as
    // `revert`), or rules text not applied, refuses the step.
    (&["undo", "redo"], Operand, &[InProgressEdit]),
    // Save settles owed work first: `begin_document_path_save` ->
    // `save_primitive_tab_at` -> `prepare_primitive_save` ->
    // `snapshot_primitive` runs `settle_pending_solve`
    // (dispatch/library/editor/primitive_save.rs). Owed work and drafts are
    // core-bound (ADR-0006), so nothing implicit.
    (&["save_document"], Operand, &[]),
    // Machine configuration (ADR-0006 `CoreSettings`): the saved layer sets
    // and tab-name style of `ui_state.layer_strip`, written to the prefs.
    (
        &[
            "show_layer_set",
            "save_layer_set",
            "delete_layer_set",
            "set_layer_tab_names",
            "project_checklist_templates",
            "project_save_checklist_template",
        ],
        Operand,
        &[Settings],
    ),
    // The live ERC preference (#1288): a flip reads the value it flips.
    (&["toggle_live_erc"], Bridge, &[Settings]),
    (&["toggle_live_erc"], Operand, &[Settings]),
    (&["toggle_live_erc"], HeadlessWithoutOperands, &[Settings]),
    // A project without rules of its own edits from the machine rules
    // (`rules_text_of` falls back on `ui_state.machine_drc_rules`); a preset
    // comes from the machine's profile library.
    (
        &[
            "rules_add",
            "rules_set",
            "rules_remove",
            "rules_enable",
            "rules_matrix_set",
            "rules_set_create",
            "rules_set_edit",
            "rules_set_apply",
            "rules_preset_apply",
        ],
        Operand,
        &[Settings],
    ),
    // Read: they act on the named document and their operands only. A board
    // edit repaints the board's canvas when that board is in view; the
    // repaint is a projection and reads nothing into the result.
    (
        &[
            "clear_all_net_colors",
            "color_net",
            "export_board_report",
            "export_board_step",
            "export_sheet_png",
            "fabrication_outputs",
            "export_assembly_svg",
            "board_assembly_drawing",
            "board_report_bom",
            "board_report_components",
            "board_report_net_length",
            "board_report_netlist",
            "add_net_class",
            "rename_net_class",
            "delete_net_class",
            "set_net_class_rules",
            "assign_board_net_class",
            "set_layer_stack",
            "lock_board_items",
            "unlock_board_items",
            "set_via_span",
            "set_layer_sizes",
            "set_pad_shape",
            "hide_layer",
            "highlight_layer",
            "hide_layer_group",
            "show_layer_group",
            "set_layer_visible",
            "set_active_layer",
            "set_layer_color",
            "show_used_layers_only",
            "set_object_visible",
            "set_object_transparency",
            "set_board_graphic",
            "set_footprint_text",
            "repour_all",
            "panelize_board",
            "remove_board_panel",
            "set_board_panel",
            "layout_plan_set",
            "swap_pins",
            "swap_gates",
            "swap_schematic_pins",
            "swap_schematic_gates",
            "save_board_reuse_block",
            "place_board_reuse_block",
            "autoplace_fields",
            "sync_sheet_pins",
            "set_sheet_symbol_size",
            "set_symbol_parameter",
            "set_net_tie",
            "project_move_sheet",
            "project_add_variant",
            "project_rename_variant",
            "project_remove_variant",
            "project_set_active_variant",
            "project_move_library",
            "project_set_annotate_undo",
            "project_set_annotate_numbering",
            "project_set_default_variant",
            // #3038: the Variant Manager's part edits and Duplicate.
            "project_set_variant_fit",
            "project_set_variant_alternate",
            "project_set_variant_override",
            "project_duplicate_variant",
            "project_set_version_control",
            "project_set_requirement",
            "project_enable_requirement",
            "project_start_checklist",
            "project_add_checklist_item",
            "project_set_checklist_item",
            "project_set_checklist_state",
            "project_remove_checklist_item",
            "project_reset_checklist",
            "project_import_checklist",
            "project_export_checklist_csv",
            "project_open_checklist_link",
            "project_set_rail",
            "bring_drawings_forward",
            "bring_drawings_to_front",
            "send_drawings_backward",
            "send_drawings_to_back",
            "delete_selection",
            // Mockup row 199: the named project's notebook files.
            "export_notebook_timeline",
            "mark_notebook_attachment_valid",
            "attach_notebook_file",
            // #2470: the area tools take their layer and net as operands,
            // never the active layer or the tool's form.
            "place_board_fill",
            "place_board_region",
            "place_board_polygon_pour",
            "place_board_keepout",
            "place_board_pour_cutout",
            // #2936, #2937: a footprint placed, turned, flipped, assigned or
            // swapped by its operands, never the selection.
            "set_footprint_position",
            "set_footprint_rotation",
            "set_footprint_side",
            "assign_footprint",
            "change_footprint",
            // #1297, #2938: the part swap and a BOM line's alternates.
            "change_component",
            "bom_add_alternate",
            "bom_remove_alternate",
            "bom_set_alternate_order",
            // #2470, #2462: board numbering, back annotation and the board
            // against schematic Apply, on the named board or project.
            "reannotate_board",
            "back_annotate_schematics",
            "apply_board_schematic_differences",
            // #2457, #2941: the rooms commands take the room, the sheets'
            // parts and the weights from their operands and the board.
            "place_board_room",
            "create_rooms_from_sheets",
            "arrange_in_room",
            "auto_place",
        ],
        Operand,
        &[],
    ),
    // #2457: from the palette, Arrange in Room acts on the room Properties
    // shows on the board in view.
    (
        &["arrange_in_room"],
        Translator,
        &[ActiveDocument, Selection],
    ),
    // #2470: a headless New PCB with no `document` joins the project in view.
    (
        &["new_pcb_document"],
        HeadlessNewDocument,
        &[ActiveDocument],
    ),
    // #2470: Annotate with no `document` runs on the active sheet's project.
    (
        &[
            "annotate_schematic_quietly",
            "force_annotate_all_schematics",
        ],
        Operand,
        &[ActiveDocument],
    ),
    // The DRC results act on the board in view only (`drc_results_board`,
    // handlers/pcb_drc_results.rs); Show also centres that window's camera
    // (`center_on_world` reads the canvas `viewport`). The last DRC run
    // (`interaction_state.pcb_drc`, `PcbDrc` in app/state/pcb_drc.rs) is
    // core-bound session state, check results being an ADR-0006 core
    // category listed in scripts/uifree-state.txt, so it is no implicit input.
    (&["show_drc_violation"], Operand, &[ActiveDocument, Window]),
    (&["filter_drc_results"], Operand, &[ActiveDocument]),
    // The DRC fixes check the named board with Run DRC's rules: a project
    // without rules is checked with the machine rules (`fix_drc` ->
    // `board_design_rules` -> `drc_inputs`), configuration.
    (&["fix_drc", "fix_drc_violation"], Operand, &[Settings]),
    // A waiver is refused on a stale last run (`set_drc_waiver`): the core
    // session state above, so nothing implicit.
    (
        &["waive_drc_violation", "unwaive_drc_violation"],
        Operand,
        &[],
    ),
    // The net must be in the project netlist stitched from the documents
    // (`project_has_net`, closed sheets read first), not the GUI's style
    // cache (#2549), so nothing implicit.
    (&["assign_net_class"], Operand, &[]),
    // An agent's revert is refused while the person draws, types into or
    // moves on that sheet in any window (`person_busy_on`).
    (
        &["revert"],
        Operand,
        &[ActiveDocument, Window, InProgressEdit],
    ),
    // Adding or removing a requirement clears the Project tab's typed
    // requirement drafts.
    (
        &["project_add_requirement", "project_remove_requirement"],
        Operand,
        &[InProgressEdit],
    ),
    // A run already going in the Project tab (or the Design Checklist tab)
    // skips this one.
    (
        &["project_run_requirements", "project_run_checklist"],
        Operand,
        &[Window],
    ),
    // #2795: a compile reads the project's notebook as its tab holds it (or
    // its file) and the view writes that tab's session, nothing implicit; an
    // insert lands at the tab's cursor unless `line` names one.
    (&["notebook_compile", "notebook_view"], Operand, &[]),
    (&["notebook_insert"], Operand, &[Cursor]),
    // Mockup rows 196 and 197: each reads the project it names first and
    // answers from that read; a fix edits the notebook tab's text.
    (
        &[
            "notebook_references",
            "notebook_reference_card",
            "notebook_reference_picker",
            "notebook_badges",
            "notebook_fix_reference",
        ],
        Operand,
        &[],
    ),
    // The board update acts on the project its `document` names (#2470),
    // else on the active project (the workspace pointer, not the tab in
    // view); an open Update PCB dialog only refuses it.
    (&["update_pcb_from_schematic"], Operand, &[ActiveDocument]),
    (
        &["update_pcb_from_schematic"],
        HeadlessWithoutOperands,
        &[ActiveDocument],
    ),
    // The new document joins the active document's project.
    (
        &[
            "new_pcb_document",
            "new_schematic_sheet",
            "new_block_diagram",
        ],
        Operand,
        &[ActiveDocument],
    ),
    // The DXF file comes from the picker; it lands on the active layer.
    (
        &["import_dxf", "import_dxf_outline"],
        Bridge,
        &[DialogAnswer, ToolState],
    ),
    // Without a `layer` operand it lands on the board's active layer.
    (&["import_dxf", "import_dxf_outline"], Operand, &[ToolState]),
    // The turn adds to the angle the board's canvas shows now: view state of
    // the window, like its zoom, not the document's render cache.
    (&["view_rotate"], Bridge, &[Window]),
    (&["view_rotate"], Operand, &[Window]),
    // Rotate and mirror bend an open wire run (corner rule), turn the part
    // or the Bus Entry on the cursor (tool), else the selection.
    (
        &[
            "rotate_clockwise",
            "rotate_counterclockwise",
            "mirror_x",
            "mirror_y",
        ],
        Bridge,
        &[ActiveDocument, Selection, ToolState, InProgressEdit],
    ),
    (
        &[
            "rotate_selection",
            "rotate_selection_cw",
            "flip_selected_x",
            "flip_selected_y",
        ],
        ActiveBar,
        &[ActiveDocument, Selection, ToolState, InProgressEdit],
    ),
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
        ],
        StillRunsRefusal,
        &[ActiveDocument, Selection, ToolState, InProgressEdit],
    ),
    // The part on the cursor flips, else the board selection.
    (
        &["flip_selection"],
        Bridge,
        &[ActiveDocument, Selection, ToolState],
    ),
    (
        &["flip_selection"],
        StillRunsRefusal,
        &[ActiveDocument, Selection, ToolState],
    ),
    // The active tab's selection: a footprint editor's, a board's or a
    // sheet's, which the translators fill in as `document` and `ids`
    // (`delete_pcb_selection`, `delete_sheet_selection`,
    // `handle_drawings_arrange_requested`).
    (
        &[
            "delete_selection",
            "bring_drawings_forward",
            "bring_drawings_to_front",
            "send_drawings_backward",
            "send_drawings_to_back",
        ],
        Translator,
        &[ActiveDocument, Selection],
    ),
    (
        &["delete_selection"],
        StillRunsRefusal,
        &[ActiveDocument, Selection],
    ),
    // The same handler as the bridge row.
    (
        &["cut"],
        StillRunsRefusal,
        &[ActiveDocument, Selection, ToolState],
    ),
    (
        &["smart_paste", "paste_special"],
        StillRunsRefusal,
        &[Clipboard, RenderCache, ToolState],
    ),
    // The annotate pass uses the dialog's lock set and order.
    (
        &[
            "annotate_schematic_quietly",
            "force_annotate_all_schematics",
        ],
        Bridge,
        &[ActiveDocument, DialogAnswer],
    ),
    (
        &[
            "annotate_schematic_quietly",
            "force_annotate_all_schematics",
        ],
        StillRunsUnavailable,
        &[ActiveDocument, DialogAnswer],
    ),
    (&["save_document_as"], StillRunsUnavailable, &[DialogAnswer]),
    // DRC of the board in view, unless one is running for it; a project
    // without rules is checked with the machine rules (`start_pcb_drc` ->
    // `drc_inputs`).
    (&["run_drc"], Bridge, &[ActiveDocument, Settings]),
    // Checks read from memory, the active tab's render snapshot not used
    // (`erc_check_inputs`), confined by the origin. A project without rules
    // is checked with the machine's ERC overrides (`erc_rule_settings_quiet`)
    // or DRC rules (`ui_state.machine_drc_rules`): configuration.
    (
        &["check_erc", "check_drc", "check_requirements"],
        Check,
        &[Settings],
    ),
    // The design report runs the same checks (#2945).
    (&["generate_design_notebook"], Operand, &[Settings]),
    // The answer names the view angle of the board's canvas.
    (&["document_info"], Query, &[Window]),
    // `rules_explain` and `rules_get` read the board's last DRC run
    // (`interaction_state.pcb_drc`, `PcbDrc` in app/state/pcb_drc.rs), which
    // is core-bound session state (check results, an ADR-0006 core category
    // in scripts/uifree-state.txt), not an implicit input. A project without
    // rules reads the machine rules (`ui_state.machine_drc_rules`), a preset
    // diff the machine's profile library, and the project settings the
    // machine's annotate undo default: configuration.
    (
        &[
            "project_settings",
            "rules_conflicts",
            "rules_effective",
            "rules_explain",
            "rules_get",
            "rules_list",
            "rules_matrix_get",
            "rules_preset_diff",
        ],
        Query,
        &[Settings],
    ),
    // Read: operands only. `bom_lines` (#1988) reads the named sheet's
    // project, its open board and the library's cached prices, as documents.
    // `layout_plan_status` reads the board's last DRC run, core-bound session
    // state as for `rules_get` above.
    (
        &[
            "board_panel",
            "bom_lines",
            // #2949: the named project's variants and sheets.
            "compare_variants",
            "find_items",
            "find_similar_items",
            // #2952: the named project's open sheets and classes.
            "list_diff_pairs",
            "layout_plan_get",
            "layout_plan_status",
            "measure_board",
            // #2796: the named diagram, open or saved.
            "query_block_diagram",
            "query_board_nets",
            "query_cross_references",
            "query_net_ties",
            "query_pins",
            // #1313: the project's sheets, read off the UI thread.
            "query_power_tree",
            // #2943: the project's rails and its parts, read off the UI thread.
            "query_rail_budget",
            // #2937, #2462, #2460, #2938, #1297: the footprint list, the board
            // against the schematic, the board's counters, the priced BOM
            // and the part search.
            "query_footprint_assignments",
            "check_board_against_schematic",
            "board_status",
            "query_bom",
            "search_parts",
            "read_document",
            "variant_bom",
            // Mockup row 199: the named project's notebook and sheets.
            "notebook_measurements",
            "notebook_timeline",
            "notebook_attachments",
            // Mockup row 198: the review records of the named project's
            // notebook files, read from disk.
            "list_annotations",
        ],
        Query,
        &[],
    ),
    // The checklist's last run is check results, core-bound session state
    // (scripts/uifree-state.txt), so it is no implicit input.
    (
        &["project_checklist", "project_checklist_link_choices"],
        Query,
        &[],
    ),
    // #1283: the snippet and template folders of the project in view
    // (`reuse_roots_in_view`) and of the user's library.
    (
        &["list_snippets", "list_templates"],
        Query,
        &[ActiveDocument],
    ),
    // #2944: a relative library path is read from the edited sheet's
    // project folder (`open_library_named`), as `place_component` reads it.
    (&["query_library"], Query, &[ActiveDocument]),
    // #2939: the named board and the open libraries' models.
    (&["query_part_heights"], Operand, &[]),
    // What the open or the new project dialog answers.
    (
        &["new_document", "open_document", "new_project_with_profile"],
        HeadlessNewDocument,
        &[DialogAnswer],
    ),
    // The menu row's gate, the main window's tiles and the active tab.
    (
        &[
            "tile_windows_horizontally",
            "tile_windows_vertically",
            "untile_documents",
            "split_pane_right",
            "split_pane_down",
            "open_second_view",
            "tile_windows_grid",
            "move_tab_to_other_pane",
            "close_pane",
            "sync_pane_zoom",
            "equalize_panes",
            // #2470: Fit frames the main window's active document.
            "zoom_to_fit",
        ],
        AgentWindowLayout,
        &[Window, ActiveDocument],
    ),
    // #2470: the main window's dock; opening Net Classes on a board opens
    // the PCB panel (`show_panel`), so the active tab is read too.
    (
        &["set_dock_panel"],
        AgentWindowLayout,
        &[Window, ActiveDocument],
    ),
    (&["set_dock_panel"], Operand, &[Window, ActiveDocument]),
    // The listings narrow to an agent's documents (`document_entries`).
    (&["list_documents", "resolve"], Query, &[Origin]),
    // The grid the copy is stored on (`copy_selection`).
    (&["copy", "cut", "duplicate"], Bridge, &[ToolState]),
    (&["new_document", "open_document"], Bridge, &[DialogAnswer]),
    // A new project takes the machine's default rule profile and annotate
    // undo setting (`start_new_project`): configuration.
    (
        &["new_document", "new_project_with_profile"],
        Operand,
        &[Settings],
    ),
    (&["open_document"], Operand, &[]),
    // Headless with no `document`: they act on the sheet in view (#2522).
    (
        &["create_sheet_symbol_from_sheet", "repair_pin_junctions"],
        HeadlessWithoutOperands,
        &[ActiveDocument],
    ),
    // The `at` point is checked against the grid (#2522).
    (&["create_sheet_symbol_from_sheet"], Operand, &[ToolState]),
    // The target sheet comes from `document` (#2522): nothing implicit.
    (
        &[
            "edit_no_erc_rules",
            "repair_pin_junctions",
            "set_sheet_variable",
            "sync_sheet_entries",
        ],
        Operand,
        &[],
    ),
    // The named board only (#2522): nothing implicit.
    (&["reset_board_origin", "reset_layer_colors"], Operand, &[]),
    (
        &[
            "new_pcb_document",
            "new_schematic_sheet",
            "new_block_diagram",
        ],
        HeadlessWithoutOperands,
        &[ActiveDocument],
    ),
    // #2796: File ▸ New ▸ Block Diagram makes the diagram in the active
    // document's project.
    (&["new_block_diagram"], Bridge, &[ActiveDocument]),
    // #2796: a block diagram edit acts on the named diagram and its
    // operands only.
    (
        &[
            "block_add",
            "block_set",
            "block_remove",
            "block_port_add",
            "block_port_remove",
            "block_link",
            "block_link_set",
            "block_unlink",
            "block_group",
            "block_group_set",
            "block_ungroup",
        ],
        Operand,
        &[],
    ),
    // #2796: the diagram in view or the one the open dialog picks, then the
    // file the save dialog answers.
    (
        &["export_block_diagram"],
        Bridge,
        &[ActiveDocument, DialogAnswer],
    ),
    // The PDF title block's labels follow the locale.
    (&["export_block_diagram"], Operand, &[Settings]),
    // The preset is looked up among the saved filter presets
    // (`interaction_state.custom_filter_presets`, prefs-backed through
    // fonts/presets.rs): configuration.
    (&["apply_selection_filter_preset"], Operand, &[Settings]),
    // The live keyboard profiles (`ui_state.keymap_profiles`, the saved
    // `keyboard_shortcuts.toml`) and the installed extensions' commands (#2495).
    (&["keymap_assign", "keymap_clear"], Operand, &[Settings]),
    (&["set_selection_filters"], Operand, &[]),
    (
        &[
            "align_bottom",
            "align_horizontal_centers",
            "align_left",
            "align_right",
            "align_to_grid",
            "align_top",
            "align_vertical_centers",
            "distribute_horizontally",
            "distribute_vertically",
            "bring_to_front",
            "move_to_front",
            "send_to_back",
        ],
        ActiveBar,
        &[ActiveDocument, Selection],
    ),
    (&["unselect_all"], Bridge, &[Selection]),
    // The placement handlers: a point must be where a click lands (grid,
    // and for a wire the electrical grid of the edited sheet's engine, #2542);
    // what the operands leave out comes from the project defaults, never the
    // pre-placement form (#2326).
    (&["place_wire", "draw_wire"], Operand, &[ToolState]),
    (&["place_bus", "draw_bus"], Operand, &[ToolState]),
    (
        &[
            "draw_line",
            "draw_graphic_line",
            "draw_rectangle",
            "place_net_label",
            "place_local_net_label",
            "place_global_label",
            "place_hierarchical_label",
            "place_power_symbol",
            "place_no_connect",
            "place_junction",
            "place_no_erc",
            "place_text",
            "place_text_string",
            "place_blanket",
            "edit_blanket",
            "place_compile_mask",
            "place_diff_pair",
            "place_parameter_set",
            "place_component",
            "place_sheet_symbol",
            "place_sheet_entry",
            "place_design_block",
            "import_graphics",
            // #1283: a snippet is placed as a design block.
            "place_snippet",
        ],
        Operand,
        &[ToolState],
    ),
    // #1283: without `ids`, the selection on the sheet in view is saved.
    (&["save_snippet"], Operand, &[Selection]),
    // #2665: the in-app copy, and the grid its default offset is measured on.
    (&["smart_paste"], Operand, &[ToolState, Clipboard]),
    // Move Keeps Wires is not read: both take `keep_wires` as an operand
    // (#2552; rotation, Hakan 2026-11-25, #2326 comment 7167743982).
    (
        &["set_object_position", "set_object_rotation"],
        Operand,
        &[],
    ),
    // #2953: a designator is written for the placement the sheet in view
    // shows, and only on that sheet (`designator_edit_commands`).
    (&["set_object_text"], Operand, &[ActiveDocument]),
    // The tool defaults, and the active layer when `layer` is not given.
    // Declared 6.P3 exception: still the person's form until #2876 (docs/DECISIONS.md, 2026-11-24).
    (
        &["add_board_graphic"],
        Operand,
        &[PrePlacementForm, ToolState],
    ),
    // GUI routes.
    (&["find_similar_objects"], Bridge, &[Cursor, RenderCache]),
    (&["find_similar_objects"], BridgeContextMenu, &[RenderCache]),
    (
        &["cancel_current_tool"],
        Bridge,
        &[ToolState, InProgressEdit],
    ),
    (
        &["center_on_cursor", "center_view_at_cursor"],
        Bridge,
        &[Cursor, Window],
    ),
    (
        &[
            "change_component",
            "edit_library_symbol",
            "edit_selected_symbol_in_symbol_editor",
            "open_datasheet",
            "create_sheet_from_sheet_symbol",
            "open_child_sheet",
            "sync_sheet_pins",
            "drag_keep_connections",
            "go_to_net_partner",
            "highlight_related_net_objects",
        ],
        Bridge,
        &[Selection],
    ),
    (&["autoplace_fields"], Bridge, &[ActiveDocument, Selection]),
    (&["clear_all_net_colors"], ActiveBar, &[ActiveDocument]),
    (&["clear_net_color", "color_net"], ActiveBar, &[Selection]),
    (
        &[
            "bring_to_front_of",
            "send_to_back_of",
            "select_connection",
            "move_selection_xy",
            "drag_selection",
        ],
        ActiveBar,
        &[Selection],
    ),
    (&["select_expand_connection"], Bridge, &[Selection]),
    (
        &[
            "drag",
            "lasso_select",
            "toggle_selection",
            "tool_select",
            "place_component",
        ],
        ActiveBar,
        &[],
    ),
    (&["highlight_net_under_cursor"], Bridge, &[Cursor]),
    (
        &["highlight_net_under_cursor"],
        BridgeContextMenu,
        &[ActiveDocument],
    ),
    // `copy_attributes_at` (#1669): mid-draw it adds a vertex through the
    // canvas click path (tool state, the draw in progress); otherwise it
    // copies from the object under the point or the selection.
    (
        &["copy_attributes_or_add_vertex"],
        Bridge,
        &[Cursor, Selection, ToolState, InProgressEdit],
    ),
    (
        &["copy_attributes_or_add_vertex"],
        BridgeContextMenu,
        &[ActiveDocument, Selection, ToolState, InProgressEdit],
    ),
    (
        &[
            "edit_value",
            "edit_reference_designator",
            "edit_footprint_field",
            "edit_text_in_place",
        ],
        Bridge,
        &[Cursor, Selection],
    ),
    (
        &["select_node_or_connection_item"],
        Bridge,
        &[Cursor, RenderCache],
    ),
    (
        &["placement_accept"],
        Bridge,
        &[InProgressEdit, RenderCache],
    ),
    (
        &["zoom_to_selection_area"],
        Bridge,
        &[Selection, RenderCache],
    ),
    (
        &[
            "cycle_fast_grid",
            "cycle_snap_grid_backward",
            "previous_grid",
            "next_grid",
            "fast_grid_1",
            "fast_grid_2",
            "toggle_visible_grid",
            "toggle_electrical_grid",
            "cycle_unit",
            "cycle_selection_mode",
            "cycle_wire_bus_graphic_mode",
            "cycle_wiring_mode",
            "placement_properties",
        ],
        Bridge,
        &[ToolState],
    ),
    (
        &["switch_segment_posture"],
        Bridge,
        &[ToolState, InProgressEdit],
    ),
    (
        &[
            "footprint_mode_pads",
            "footprint_mode_sketch",
            "footprint_mode_view_3d",
            "footprint_recompute_courtyard_outline",
            "next_sheet",
            "previous_sheet",
            "open_project_options",
        ],
        Bridge,
        &[ActiveDocument],
    ),
    (
        &[
            "find_next",
            "find_previous",
            "clear_net_highlighting",
            "next_highlighted_net_item",
            "previous_highlighted_net_item",
            "next_document_tab",
            "previous_document_tab",
            "sheet_navigation_back",
            "sheet_navigation_forward",
            "toggle_auto_focus",
            "toggle_floating_panels",
            "toggle_search_panel",
            "toggle_layers_panel",
            "toggle_properties_panel",
            "toggle_schematic_filter_panel",
            "toggle_schematic_list_panel",
        ],
        Bridge,
        &[Window],
    ),
    // Workspace layouts live in prefs.json (settings). Apply and Reset also
    // read which panels are open, detached or floating (`arrange_dock`).
    (
        &[
            "apply_layout_schematic",
            "apply_layout_board",
            "apply_layout_review",
            "apply_layout_notebook",
            "reset_workspace_layout",
        ],
        Bridge,
        &[Window, Settings],
    ),
    // The dialogs open on the saved list: Save's first free name, Manage's
    // active or first own layout.
    (
        &["save_workspace_layout", "manage_workspace_layouts"],
        Bridge,
        &[Settings],
    ),
    // Dialogs, pickers and panels that open on nothing implicit, and the
    // tools that are only armed.
    (
        &[
            "annotate_schematic",
            "reset_schematic_designators",
            "find",
            "find_text",
            "find_and_replace",
            "open_command_palette",
            "open_compiled_design",
            "open_grid_properties",
            "open_net_color_palette",
            "open_net_inspector",
            "open_preferences",
            "open_schematic_preferences",
            "show_current_command_hotkeys",
            "show_current_command_shortcuts",
            "new_project_with_profile",
            "toggle_cross_select_mode",
            "toggle_notebook_badges",
            "place_wire_to_bus_entry",
        ],
        Bridge,
        &[],
    ),
    (&["open_project_options"], BridgeContextMenu, &[]),
    // Board edits from the menu (#2551): the board in view, as their gate
    // says, and its selection where the tool reads it.
    (
        &[
            "panelize_board",
            "remove_board_panel",
            "repair_pin_junctions",
            "repour_all",
            "reset_board_origin",
            "reset_layer_colors",
        ],
        Bridge,
        &[],
    ),
    (
        &[
            "lock_board_items",
            "unlock_board_items",
            "repour_selected",
            "unfill_selected",
            "remove_teardrops",
        ],
        Bridge,
        &[Selection],
    ),
    // `handle_board_tools` and `handle_tune`: the selection's nets and
    // tracks, the clearances from `board_design_rules` (machine rules
    // without project rules).
    (
        &[
            "add_teardrops",
            "stitch_vias",
            "tune_diff_pair",
            "tune_net_length",
        ],
        Bridge,
        &[Selection, Settings],
    ),
    // The translators read the board in view and its selection
    // (`translated_targets`, `save_selection_as_block`,
    // `swap_selected_pads`); the clearances are the operand handler's.
    (
        &[
            "board_shape_from_selection",
            "gloss_selected_routes",
            "keepout_from_selection",
            "save_board_reuse_block",
            "swap_gates",
            "swap_pins",
            "swap_schematic_gates",
            "swap_schematic_pins",
        ],
        Translator,
        &[Selection],
    ),
    // The origin moves to the outline corner nearest the pointer.
    (&["snap_board_origin_to_corner"], Bridge, &[Cursor]),
    // The person's own step: no "person busy" refusal; an inline text edit
    // closes, a reaction to the step (rule 4 on #2326).
    (&["undo", "redo"], Bridge, &[]),
    // As the operand row: owed work and drafts are core-bound.
    (&["save_document"], Bridge, &[]),
    // Every tab closes as `close_tab` does: its title-block drafts commit and
    // a board sketch settles first.
    (&["close_all_documents"], Bridge, &[InProgressEdit]),
    // `save_all_dirty` commits the title-block drafts and settles a board
    // sketch, whose refused bake holds the save.
    (&["save_all"], Bridge, &[InProgressEdit]),
    // The core command after a split router (#2326 kill-check decisions 3
    // and "homes"): the router resolves what is in hand; the targets, Move
    // Keeps Wires and the grid reach the command as operands.
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
        ],
        SplitCore,
        &[ActiveDocument, Selection, ToolState],
    ),
    (&["flip_selection"], SplitCore, &[ActiveDocument, Selection]),
    // The GUI reads the clipboard and the landing point; the core places
    // the fragment at a point, new ids as operands.
    (
        &["smart_paste", "paste_special"],
        SplitCore,
        &[ActiveDocument, Clipboard, ToolState],
    ),
    // Cut: the GUI copies, the core deletes the targets.
    (&["cut"], SplitCore, &[ActiveDocument, Selection]),
    // Duplicate on a board pastes the copy beside it.
    (
        &["duplicate"],
        SplitCore,
        &[ActiveDocument, Selection, ToolState],
    ),
    // Enter on a polyline: its points are the operand.
    (
        &["placement_accept"],
        SplitCore,
        &[ActiveDocument, ToolState],
    ),
    // Planned Active Bar tools: they only say they are not built yet.
    (PLANNED, ActiveBar, &[]),
    (PLANNED, StillRunsUnavailable, &[]),
];

const PLANNED: &[&str] = &[
    "draw_ellipse",
    "draw_elliptical_arc",
    "draw_round_rectangle",
    "place_device_sheet_symbol",
    "place_graphic",
    "place_reuse_block",
];

pub(super) type Annotation = (&'static [&'static str], Form, &'static [Input]);

/// Every annotation: [`ANNOTATIONS`], then the tables split out by area.
pub(super) fn all() -> impl Iterator<Item = &'static Annotation> {
    ANNOTATIONS.iter().chain(gerber_viewer::GERBER_VIEWER)
}

/// The hand annotation of `(id, form)`, if any.
pub(super) fn of(id: &str, form: Form) -> Option<&'static [Input]> {
    all()
        .find(|(ids, annotated, _)| *annotated == form && ids.contains(&id))
        .map(|(_, _, inputs)| *inputs)
}
