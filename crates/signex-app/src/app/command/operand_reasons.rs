//! The refusal a command gives on a surface that gives it no operands
//! (palette, key binding, menu), read through `operand_handlers`.

use crate::keymap::AppCommandId;

/// Why `command` does nothing without operands on a surface that gives it
/// none (palette, key binding, menu): its GUI surface is another widget, and
/// a command never opens, closes or focuses one as a side effect.
pub(crate) fn needs_operands_reason(command: &AppCommandId) -> Option<&'static str> {
    match command.as_str() {
        "assign_net_class" => Some(
            "`assign_net_class` needs the operand `net_name` — \
             use the Net Class row in Properties",
        ),
        "apply_edits" => Some(
            "`apply_edits` needs the operands `document`, `expect_revision` and `edits` — \
             it has no surface but an agent's call",
        ),
        "apply_board_schematic_differences" => Some(
            "`apply_board_schematic_differences` needs the operands `document` and \
             `to_board` or `to_schematic` — use Apply in Compare Board with Schematic",
        ),
        "revert" => Some(
            "`revert` needs the operands `document` and `group` — \
             Undo is the person's way back",
        ),
        // #2394: a person picks these on the layer strip under the board.
        "show_layer_set" | "save_layer_set" => Some(
            "this command needs the operands `name` and `document` — \
             use the layer set button under the board",
        ),
        "delete_layer_set" => Some(
            "`delete_layer_set` needs the operand `name` — \
             use Manage sets on the layer set button under the board",
        ),
        "hide_layer" | "highlight_layer" => Some(
            "this command needs the operands `layer` and `document` — \
             right-click a layer tab under the board",
        ),
        "hide_layer_group" | "show_layer_group" => Some(
            "this command needs the operands `group` and `document` — \
             right-click a layer tab under the board",
        ),
        "set_layer_tab_names" => Some(
            "`set_layer_tab_names` needs the operand `style` — \
             right-click a layer tab under the board",
        ),
        // Mockup row 114: a person uses the Export Job tab.
        "read_output_job"
        | "add_job_output"
        | "set_job_output"
        | "set_job_profile"
        | "set_job_triggers"
        | "set_job_gerber_units"
        | "set_job_gerber_format"
        | "set_job_gerber_zeros"
        | "set_job_gerber_layers"
        | "set_job_gerber_apertures"
        | "stop_output_job"
        | "view_output_job_run" => Some(
            "this command needs the operand `path` and the job's settings — \
             use the Export Job tab",
        ),
        // #2470: for agents and scripts; a person uses the dock itself.
        "set_dock_panel" => Some(
            "`set_dock_panel` needs the operands `panel` and `state` — \
             use the panel list, a tab's close button or the region's arrow",
        ),
        // #2363: a person uses the Layers panel.
        "set_layer_visible" | "set_active_layer" | "set_layer_color" => Some(
            "this command needs the operands `layer` and `document` — \
             use the layer's row in the Layers panel",
        ),
        "show_used_layers_only" => Some(
            "`show_used_layers_only` needs the operands `on` and `document` — \
             use Used On in the Layers panel",
        ),
        "set_object_visible" | "set_object_transparency" => Some(
            "this command needs the operands `kind` and `document` — \
             use View Options in the Layers panel",
        ),
        // #2410: a person edits the panel in the board's Properties.
        "set_board_panel" => Some(
            "`set_board_panel` needs the operands `document`, `field` and `value` — \
             use the Panel section of the board's Properties",
        ),
        // #2942: the layout plan has no surface but its commands.
        "layout_plan_set" => Some(
            "`layout_plan_set` needs the operands `document`, `stage` and a setting \
             of that stage — it has no surface but its command",
        ),
        "set_selection_filters" => Some(
            "`set_selection_filters` needs the operand `kinds` — \
             use the Filter panel or the Active Bar's filter",
        ),
        "apply_selection_filter_preset" => Some(
            "`apply_selection_filter_preset` needs the operand `name` — \
             click the preset in the Filter panel",
        ),
        // #2495: a person records the key in Preferences.
        "keymap_assign" | "keymap_clear" => Some(
            "this command needs the operands `command` and `profile` — \
             use Preferences ▸ Keyboard shortcuts",
        ),
        "set_object_position" => Some(
            "`set_object_position` needs the operands `id` and `at` — \
             use the Position row in Properties",
        ),
        "set_object_rotation" => Some(
            "`set_object_rotation` needs the operands `id` and `rotation` — \
             use the Rotation row in Properties",
        ),
        "set_sheet_symbol_size" => Some(
            "`set_sheet_symbol_size` needs the operands `id` and `size` — \
             use the Size row in Properties",
        ),
        "set_symbol_parameter" => Some(
            "`set_symbol_parameter` needs the operands `id` and `name` — \
             use the Parameters rows in Properties",
        ),
        "set_object_text" => Some(
            "`set_object_text` needs the operands `id` and `field` — \
             use Properties or double-click the cell in the Object List",
        ),
        "set_sheet_variable" => Some(
            "`set_sheet_variable` needs the operand `name` — \
             use the Sheet Variables rows in Properties",
        ),
        "set_text_note_format" => Some(
            "`set_text_note_format` needs the operands `id` and `format` — \
             use the Format row of the note in Properties",
        ),
        // #1114, mockup row 203: a person sets these in the Frame and Size
        // sections and works the editor with its buttons and keys.
        "set_text_note_frame" | "set_text_note_size" => Some(
            "the Frame and Size of a rich text note need the operand `id` — \
             use the Frame and Size sections of the note in Properties",
        ),
        "apply_rich_note_edit"
        | "edit_rich_note"
        | "cancel_rich_note_edit"
        | "wrap_rich_note_selection"
        | "open_rich_note_link"
        | "compile_rich_note" => Some(
            "the rich text note commands need the operand `id` — \
             double-click the note to edit it in place",
        ),
        // #1116: a person edits an image in Properties ▸ Image.
        "set_image" => Some(
            "`set_image` needs the operands `id`, `field` and `value` — \
             select the image and use Properties",
        ),
        "relink_image" => Some(
            "`relink_image` needs the operand `path` and `file` or `id` — \
             use Relink… or Replace… in Properties ▸ Image",
        ),
        "show_image_in_folder" => Some(
            "`show_image_in_folder` needs the operand `id` — \
             use Show in folder in Properties ▸ Image",
        ),
        // #1277: no Properties row yet; a part placed from a Net Tie library
        // symbol is a tie already.
        "set_net_tie" => Some(
            "`set_net_tie` needs the operands `id` and `on` — \
             place the part from a Net Tie library symbol, or run it with them",
        ),
        // #2424: a person draws with the Graphics tools and edits in
        // Properties.
        "add_board_graphic" => Some(
            "`add_board_graphic` needs the operands `document` and `kind` — \
             use Place ▸ Graphics on a board",
        ),
        "set_board_graphic" => Some(
            "`set_board_graphic` needs the operands `document`, `id`, `field` and `value` — \
             select the drawing and use Properties",
        ),
        // #2838: a person draws with the symbol editor's Shapes tools.
        "symbol_draw_elliptical_arc"
        | "symbol_draw_bezier"
        | "symbol_draw_round_rectangle"
        | "symbol_draw_pie" => Some(
            "this command needs the operand `document` and the shape's points — \
             use the Shapes tools of the symbol editor",
        ),
        // #2368: a person drags the text on the board.
        "set_footprint_text" => Some(
            "`set_footprint_text` needs the operands `document`, `designator`, `text`, \
             `field` and `value` — drag the designator or comment on the board",
        ),
        // #2817: a person edits the pins in the Pin table tab.
        "edit_symbol_pins" => Some(
            "`edit_symbol_pins` needs the operands `document` and `pins` or `paste` — \
             use Edit pins in table… on the symbol's Pins tab",
        ),
        // #2936: a person drags the footprint, or uses the Location and
        // Layer rows of the Component view.
        "set_footprint_position" => Some(
            "`set_footprint_position` needs the operands `document`, `id` and `at` — \
             drag the footprint on the board",
        ),
        "set_footprint_rotation" | "set_footprint_side" => Some(
            "this command needs the operands `document`, `id` and its value — \
             use the Rotation and Layer rows of the Component view in Properties",
        ),
        // #2937: a person uses Tools ▸ Assign Footprints.
        "assign_footprint" => Some(
            "`assign_footprint` needs the operands `document`, `id` and `footprint` — \
             use Tools ▸ Assign Footprints",
        ),
        // #3013: a person uses the two dialogs on the footprint's selection.
        "footprint_move_pads" | "footprint_align_pads" => Some(
            "this command needs the operands `document` and `pad_numbers` — \
             select the pads and use Move Selection By or Align Selection",
        ),
        // #3013: a person answers the library dialogs themselves.
        "set_library_document_options" => Some(
            "`set_library_document_options` needs the operand `library` — \
             use Tools ▸ Document Options in a library editor",
        ),
        "apply_library_updates" => Some(
            "`apply_library_updates` needs the operand `document` — \
             use the Library Updates dialog the sheet opens with",
        ),
        "copy_stock_to_library" => Some(
            "`copy_stock_to_library` needs the operands `name`, `kind` and `path` — \
             open the stock symbol or footprint and use Copy to Library",
        ),
        "recover_library" => Some(
            "`recover_library` needs the operands `path` and `action` — \
             use the recovery dialog the library raises",
        ),
        "new_library" => Some(
            "`new_library` needs the operands `document` and `path` — \
             use Add New to Project ▸ Component Library in the project tree",
        ),
        "rules_add" | "rules_set" | "rules_remove" | "rules_enable" | "rules_matrix_set"
        | "rules_set_create" | "rules_set_edit" | "rules_set_apply" | "rules_preset_apply" => Some(
            "the `rules_*` edits need their operands — \
             use the Rules tab (Design ▸ Rules)",
        ),
        "show_drc_violation" | "waive_drc_violation" | "unwaive_drc_violation" => Some(
            "this command needs the operands `document` and `id` — \
             use the rows of the DRC panel",
        ),
        // #2940: the fixes have no window form yet; they run from operands.
        "fix_drc_violation" => Some(
            "`fix_drc_violation` needs the operands `document` and `violation` — \
             the key is a finding's `evidence.waiver_key` in `check_drc`",
        ),
        "fix_drc" => Some(
            "`fix_drc` needs the operand `document`, and `rule` for one kind — \
             run it from the command line with its operands",
        ),
        "query_part_heights" => Some(
            "`query_part_heights` needs the operand `document` — \
             a part's height is its Height row in Properties",
        ),
        "filter_drc_results" => Some(
            "`filter_drc_results` needs the operand `document` — \
             use the filters of the DRC panel",
        ),
        // #2719: a person routes with Shift+Space and `cycle_route_mode`,
        // and sets the width in Properties while Tab pauses the route.
        "route_options" => Some(
            "`route_options` needs the operand `document` — \
             while routing, Shift+Space steps the corner mode and Tab pauses \
             the route for its width in Properties",
        ),
        // Mockup row 199: a person uses the notebook's margin lane.
        "export_notebook_timeline" => Some(
            "`export_notebook_timeline` needs the operands `document`, a target and `path` — \
             use export CSV under the timeline in the notebook",
        ),
        "mark_notebook_attachment_valid" => Some(
            "`mark_notebook_attachment_valid` needs the operands `document` and `file` or `id` — \
             use Mark still valid on the stale figure in the notebook",
        ),
        "attach_notebook_file" => Some(
            "`attach_notebook_file` needs the operands `document` and `path` — \
             drop the file on the notebook page or use Figure",
        ),
        "add_net_class"
        | "rename_net_class"
        | "delete_net_class"
        | "set_net_class_rules"
        | "assign_board_net_class" => Some(
            "this command needs its operands, `document` first — \
             use the board's Net Classes panel",
        ),
        "set_layer_stack" => Some(
            "`set_layer_stack` needs the operands `document` and `stack` — \
             use the Layer Stack manager",
        ),
        // #2585: a person types the width in the Track panel.
        "set_diff_pair_width" => Some(
            "`set_diff_pair_width` needs the operands `document` and `width` — \
             select a track of the pair and use Width in Properties",
        ),
        // #2797: a person edits the groups in the Length Matching tab.
        "set_length_group" | "remove_length_group" => Some(
            "this command needs the operands `document` and `name` — \
             use the Length Matching tab (Length Matching...)",
        ),
        "set_via_span" => Some(
            "`set_via_span` needs the operands `document`, `ids` and `layers` — \
             use the From and To rows of the Via panel in Properties",
        ),
        "set_layer_sizes" => Some(
            "`set_layer_sizes` needs the operands `document`, `ids` and `sizes` — \
             use the stack rows of the Via or Pad panel in Properties",
        ),
        "set_pad_shape" => Some(
            "`set_pad_shape` needs the operands `document`, `ids` and `shape`, \
             with `delta` for a trapezoid or `corners` for a custom pad — \
             trapezoid and custom pads are drawn in the footprint editor",
        ),
        "set_pour_corners" => Some(
            "`set_pour_corners` needs the operands `document`, `ids` and `corners` — \
             use the Corners row of a pour's panel in Properties",
        ),
        "set_pour_fill" => Some(
            "`set_pour_fill` needs the operands `document`, `ids` and `fill` — \
             use the Fill Mode row of a pour's panel in Properties",
        ),
        "set_pour_thermal_relief" => Some(
            "`set_pour_thermal_relief` needs the operands `document` and `ids`, with \
             `on`, `spoke_width` or `air_gap` — use the Thermal Relief rows of the \
             pour panel in Properties",
        ),
        "set_pad_thermal_relief" => Some(
            "`set_pad_thermal_relief` needs the operands `document` and `ids`, with \
             `air_gap`, `spoke_width` or `spokes` — set a library pad's values in the \
             footprint editor's pad Properties",
        ),
        "set_teardrop_rule" | "set_pad_teardrop" => Some(
            "the teardrop rule edits need their operands, `document` first — \
             no panel shows the teardrop rule yet",
        ),
        "set_pour_properties" => Some(
            "`set_pour_properties` needs the operands `document` and `ids` — \
             use the Priority and Remove Islands rows of the pour in Properties",
        ),
        "edit_blanket" => Some(
            "`edit_blanket` needs the operand `id` and the values to set — \
             select the Blanket to read them in Properties",
        ),
        "edit_no_erc_rules" => Some(
            "`edit_no_erc_rules` needs the operand `id` and the rules to set — \
             tick the rules under Suppressed Rules in Properties",
        ),
        // #2410: a person picks the block on a component's board menu.
        "place_board_reuse_block" => Some(
            "`place_board_reuse_block` needs the operands `document`, `name` and `ids` or `designator` — \
             right-click a component on the board and pick the block",
        ),
        "project_move_sheet"
        | "project_add_variant"
        | "project_rename_variant"
        | "project_remove_variant"
        | "project_set_active_variant"
        | "project_set_default_variant"
        | "project_set_variant_fit"
        | "project_set_variant_alternate"
        | "project_set_variant_override"
        | "project_duplicate_variant"
        | "project_move_library"
        | "project_set_annotate_undo"
        | "project_set_annotate_numbering"
        | "project_set_version_control"
        | "project_add_requirement"
        | "project_set_requirement"
        | "project_remove_requirement"
        | "project_enable_requirement"
        | "project_run_requirements"
        | "project_add_font"
        | "project_remove_font"
        | "project_set_font_license" => Some(
            "the `project_*` edits need their operands — \
             use the Project tab (Project ▸ Project Options...)",
        ),
        "project_start_checklist"
        | "project_add_checklist_item"
        | "project_set_checklist_item"
        | "project_set_checklist_state"
        | "project_remove_checklist_item"
        | "project_run_checklist"
        | "project_reset_checklist"
        | "project_checklist_templates"
        | "project_save_checklist_template"
        | "project_export_checklist_csv"
        | "project_import_checklist"
        | "project_open_checklist_link" => Some(
            "the design checklist edits need their operands — \
             use the Design Checklist tab (Project ▸ Design Checklist)",
        ),
        // #2796: a person edits on the block diagram's tab.
        "block_add" | "block_set" | "block_remove" | "block_port_add" | "block_port_remove"
        | "block_link" | "block_link_set" | "block_unlink" | "block_group" | "block_group_set"
        | "block_ungroup" => Some(
            "the `block_*` edits need their operands, `document` first — \
             use the toolbar, the canvas and Properties of the block diagram's tab",
        ),
        "notebook_compile" | "notebook_insert" | "notebook_view" => Some(
            "the `notebook_*` commands need their operands — \
             use the Design Notebook tab",
        ),
        // Mockup rows 196 and 197: a person reads references on the
        // notebook tab's References panel and cards, and the badges on the
        // sheet (View ▸ Notebook Badges).
        "notebook_references"
        | "notebook_reference_card"
        | "notebook_reference_picker"
        | "notebook_badges"
        | "notebook_fix_reference" => Some(
            "the notebook reference commands need their operands — \
             use the References panel of the Design Notebook tab",
        ),
        // #2943: no panel edits the rails yet.
        "project_set_rail" => Some(
            "`project_set_rail` needs the operands `document` and `net`, with `voltage`, \
             `tolerance_percent` or `remove` — run it from the command line",
        ),
        // #2938: alternates have no window surface yet.
        "bom_add_alternate" | "bom_remove_alternate" | "bom_set_alternate_order" => Some(
            "the `bom_*` alternate edits need their operands, `document` first — \
             `query_bom` answers each line's `key`",
        ),
        _ => None,
    }
}
