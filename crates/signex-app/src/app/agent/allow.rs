//! Which commands the agent channel accepts (#1924), derived from the command
//! catalog: the #2172 queries, and a command that names its document with the
//! `document` operand and no file path (`apply_edits` and `revert` are two of
//! them), plus the window-layout commands (#2249) and the writers whose path
//! stays inside the project (#2470). Everything else is refused with the
//! reason. See
//! `docs/AGENT_SOCKET.md`, "Allowlist".

use crate::app::command::queries::is_query;
use crate::keymap::{AppCommandId, CatalogTextExt, OperandKind, metadata_for};

/// Commands that name a document and are still the person's: their undo
/// stack is theirs, and an agent takes back its own step with `revert`.
/// `consistency` in the tests keeps this list to ids the catalog has.
pub(super) const PERSON_ONLY: &[&str] = &["undo", "redo"];

/// The window-layout commands (#2249, #2470): they name no document and act
/// on the main window's layout, its view or its dock, open to agents by the
/// maintainer's rule that everything is AI-controllable. Each answers with
/// what it left (`window_layout.rs`).
pub(super) const WINDOW_LAYOUT: &[&str] = &[
    "tile_windows_horizontally",
    "tile_windows_vertically",
    "untile_documents",
    // Mockup row 207: the document area's panes.
    "split_pane_right",
    "split_pane_down",
    "open_second_view",
    "tile_windows_grid",
    "move_tab_to_other_pane",
    "close_pane",
    "sync_pane_zoom",
    "equalize_panes",
    "zoom_to_fit",
    "set_dock_panel",
];

/// Commands whose `document` is optional in the catalog, for a script's old
/// call, but which the channel takes only with it named: the agent gate
/// (`check_agent_document`) refuses them without one. `update_pcb_from_schematic`
/// takes the active project's board elsewhere (Caner, 2026-11-25), never here.
pub(super) const DOCUMENT_NAMED_ON_CHANNEL: &[&str] =
    &["update_pcb_from_schematic", "annotate_schematic_quietly"];

/// Commands that write files into a `path` (#2470): on the channel the path
/// must lie inside the folder of the named document's project
/// (`write_target.rs`), and the write is confined there again when it runs.
pub(super) const WRITES_IN_PROJECT: &[&str] = &[
    "export_board_pdf",
    "export_board_report",
    "fabrication_outputs",
];

/// A window-only command and the form the channel takes in its place.
const CHANNEL_FORM: &[(&str, &str)] = &[("annotate_schematic", "annotate_schematic_quietly")];
/// Why an extension's command is refused on the agent channel.
pub(super) const EXTENSION_COMMANDS_CLOSED: &str =
    "extension commands are not open to the agent socket in version 1";

/// Is `command` one of [`WINDOW_LAYOUT`]?
pub(in crate::app) fn is_window_layout(command: &AppCommandId) -> bool {
    WINDOW_LAYOUT.contains(&command.as_str())
}

/// Is `id` a window-layout command, which the channel takes though it only
/// works in a window? `signex-cli run --attach` lets it through (#2470).
pub fn is_agent_window_command(id: &str) -> bool {
    WINDOW_LAYOUT.contains(&id)
}

/// Does the channel take `id` only with its optional `document` named
/// ([`DOCUMENT_NAMED_ON_CHANNEL`])? `signex-cli run --attach` then sends it
/// as a command, not as an edit of `apply_edits`.
pub fn names_document_on_channel(id: &str) -> bool {
    DOCUMENT_NAMED_ON_CHANNEL.contains(&id)
}

/// Does `id` write into its `path` inside the project ([`WRITES_IN_PROJECT`])?
pub(super) fn writes_in_project(id: &str) -> bool {
    WRITES_IN_PROJECT.contains(&id)
}

/// Why `command` is not on the agent channel, or `None` when it is.
pub(super) fn refusal(command: &AppCommandId) -> Option<String> {
    let id = command.as_str();
    // Installed or not (#2495, `docs/EXTENSION_API.md`, "Catalog commands").
    if command.is_extension() {
        return Some(EXTENSION_COMMANDS_CLOSED.to_string());
    }
    let Some(meta) = metadata_for(command) else {
        return Some(format!(
            "unknown command `{id}`{}",
            crate::keymap::did_you_mean(id)
        ));
    };
    if PERSON_ONLY.contains(&id) {
        return Some(format!(
            "`{id}` is the person's own; an agent takes back its step with `revert`"
        ));
    }
    if is_query(command) || is_window_layout(command) {
        return None;
    }
    if let Some((_, form)) = CHANNEL_FORM.iter().find(|(window, _)| *window == id) {
        return Some(format!(
            "`{id}` opens a dialog; on the agent channel send `{form}` with its `document`"
        ));
    }
    if let Some(reason) = meta.headless_refusal() {
        return Some(reason);
    }
    if !writes_in_project(id) && meta.operands.iter().any(|op| op.kind == OperandKind::Path) {
        return Some(format!(
            "`{id}` takes a file path; the agent channel does not read or write a path it names"
        ));
    }
    // A required `document`: a placement's optional one does not open it, so a
    // placement stays reachable only through `apply_edits` (#2199). A
    // [`DOCUMENT_NAMED_ON_CHANNEL`] command counts as required here.
    let names_document = meta.operands.iter().any(|op| {
        op.name == "document"
            && op.kind == OperandKind::Document
            && (op.required || DOCUMENT_NAMED_ON_CHANNEL.contains(&id))
    });
    if !names_document {
        return Some(format!(
            "`{id}` does not name a document, so it is not on the agent channel"
        ));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(name: &str) -> AppCommandId {
        AppCommandId::new(name).expect("a valid id")
    }

    fn allowed() -> Vec<&'static str> {
        let mut ids: Vec<&str> = crate::keymap::all_command_ids()
            .filter(|name| refusal(&id(name)).is_none())
            .collect();
        ids.sort_unstable();
        ids
    }

    /// The accepted set, pinned: a catalog row that joins it (or leaves it)
    /// is a decision for the maintainer, so it shows up here.
    #[test]
    fn the_allowlist_is_the_queries_and_the_document_commands() {
        assert_eq!(
            allowed(),
            [
                // #2424: a drawing added to, or a field of one set on, the
                // named board.
                "add_board_graphic",
                "add_net_class",
                "add_teardrops",
                // #2470: numbering on the named sheet's project.
                "annotate_schematic_quietly",
                // #2462: Compare Board with Schematic's Apply, by row id.
                "apply_board_schematic_differences",
                "apply_edits",
                "arrange_in_box",
                // #2457, #2941: the rooms commands, on the named board.
                "arrange_in_room",
                "assign_board_net_class",
                // #2937: a part's footprint on the named sheet.
                "assign_footprint",
                "assign_net_class",
                "auto_place",
                "autoplace_fields",
                // #2470: Back Annotate and Re-Annotate Board on the named
                // project or board.
                "back_annotate_schematics",
                // #2796: the block diagram edits, on the named diagram.
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
                "board_assembly_drawing",
                "board_panel",
                "board_report_bom",
                "board_report_components",
                "board_report_net_length",
                "board_report_netlist",
                "board_shape_from_selection",
                // #2460: the board status bar's counters, as JSON.
                "board_status",
                // #2938: a BOM line's alternates, on the named project.
                "bom_add_alternate",
                // #1988: the BOM lines of the named sheet's project, a query.
                "bom_lines",
                "bom_remove_alternate",
                "bom_set_alternate_order",
                // #2326 slice 6.P2b: the drawing order and Delete, on the
                // items `ids` names on the named sheet (or board, Delete).
                "bring_drawings_forward",
                "bring_drawings_to_front",
                // #1297: the part `id` names swapped, on the named sheet.
                "change_component",
                // #2937: a footprint swap on the named board.
                "change_footprint",
                // #2462: Compare Board with Schematic as a query.
                "check_board_against_schematic",
                "check_drc",
                "check_erc",
                "check_requirements",
                "clear_all_net_colors",
                "close_active_document",
                // Mockup row 207: the pane commands, as the window layout.
                "close_pane",
                "color_net",
                // #2949: the Variant Manager's top bar actions.
                "compare_variants",
                "create_rooms_from_sheets",
                "delete_net_class",
                "delete_selection",
                "document_info",
                "equalize_panes",
                // #2817: the pins of the named symbol file, one undo step.
                "edit_symbol_pins",
                // #2470: the board writers, into a path inside the project.
                "export_board_pdf",
                "export_board_report",
                "fabrication_outputs",
                "filter_drc_results",
                "find_items",
                "find_similar_items",
                // #2940: the DRC fixes, on the named board.
                "fix_drc",
                "fix_drc_violation",
                // #2945: the design report of the named project.
                "generate_design_notebook",
                "gloss_selected_routes",
                // #2394: the board view's layers, on the named board.
                "hide_layer",
                "hide_layer_group",
                "highlight_layer",
                "keepout_from_selection",
                // #2942: the named board's layout plan.
                "layout_plan_get",
                "layout_plan_set",
                "layout_plan_status",
                // #2797: the named board's length-matching groups.
                "length_matching",
                // Mockup row 198: the notebook's review records, by typed id.
                "list_annotations",
                // #2952: a query.
                "list_diff_pairs",
                "list_documents",
                // #1116: a query, the named sheet's project's image assets.
                "list_image_assets",
                // #1283: the snippets and templates in view, as JSON.
                "list_snippets",
                "list_templates",
                "lock_board_items",
                // Mockup row 199: the notebook's measurements, timelines and
                // attachments, and Mark still valid, on the named project.
                "mark_notebook_attachment_valid",
                "measure_board",
                "move_tab_to_other_pane",
                // Mockup row 207: the panes, as the window layout.
                "open_second_view",
                // #2795, mockup rows 195-199: the Design Notebook tab, its
                // references, badges and measurements, on the named project.
                "notebook_attachments",
                "notebook_badges",
                "notebook_compile",
                "notebook_fix_reference",
                "notebook_insert",
                "notebook_measurements",
                "notebook_reference_card",
                "notebook_reference_picker",
                "notebook_references",
                "notebook_timeline",
                "notebook_view",
                "panelize_board",
                // #2470: the routing tools, the via and the area tools, on
                // the named board.
                "place_board_diff_pair",
                "place_board_fill",
                "place_board_keepout",
                "place_board_multi_route",
                "place_board_polygon_pour",
                "place_board_pour_cutout",
                "place_board_quick_route",
                "place_board_region",
                // #2410: pin and gate swap and design reuse, on the named board.
                "place_board_reuse_block",
                "place_board_room",
                "place_board_route",
                "place_board_track",
                "place_board_via",
                // #2339: the Requirements section's edits and run.
                // #1291: the Design Checklist's read, edits and run.
                "project_add_checklist_item",
                "project_add_requirement",
                "project_add_variant",
                "project_checklist",
                "project_checklist_link_choices",
                "project_checklist_templates",
                // #3038: the Variant Manager's part edits and Duplicate.
                "project_duplicate_variant",
                "project_enable_requirement",
                "project_move_library",
                "project_move_sheet",
                "project_open_checklist_link",
                "project_remove_checklist_item",
                // #1178: the Fonts section's edits; `project_add_font`
                // takes a file path, so it is not on the agent channel.
                "project_remove_font",
                "project_remove_requirement",
                "project_remove_variant",
                "project_rename_variant",
                "project_reset_checklist",
                "project_run_checklist",
                "project_run_requirements",
                "project_save_checklist_template",
                "project_set_active_variant",
                "project_set_annotate_numbering",
                "project_set_annotate_undo",
                "project_set_checklist_item",
                "project_set_checklist_state",
                "project_set_default_variant",
                // #2943: the project's rails and their budget.
                "project_set_rail",
                "project_set_font_license",
                "project_set_requirement",
                "project_set_variant_alternate",
                "project_set_variant_fit",
                "project_set_variant_override",
                "project_set_version_control",
                "project_settings",
                "project_start_checklist",
                // #2796: a block diagram as JSON.
                "query_block_diagram",
                "query_board_nets",
                // #2938: the priced BOM of the named project.
                "query_bom",
                // #1273: a sheet's off-sheet cross references, as JSON.
                "query_cross_references",
                "query_footprint_assignments",
                // #2944, #2939: an open library's rows; each part's height.
                "query_library",
                // #1277: a sheet's or a board's net ties, as JSON.
                "query_net_ties",
                "query_part_heights",
                "query_pins",
                // #1313: the project's power tree.
                "query_power_tree",
                "query_rail_budget",
                "read_document",
                "reannotate_board",
                "remove_board_panel",
                "remove_length_group",
                "remove_teardrops",
                "rename_net_class",
                // #2216: each pour's fill of the named board, as JSON.
                "repour_all",
                // The Reset rows of the board menus, on the named board.
                "reset_board_origin",
                "reset_layer_colors",
                "resolve",
                "revert",
                // #2719: the routing options on the named board.
                "route_options",
                "rules_add",
                "rules_conflicts",
                "rules_effective",
                "rules_enable",
                "rules_explain",
                "rules_get",
                "rules_list",
                "rules_matrix_get",
                "rules_matrix_set",
                "rules_preset_apply",
                "rules_preset_diff",
                "rules_remove",
                "rules_set",
                "rules_set_apply",
                "rules_set_create",
                "rules_set_edit",
                "save_board_reuse_block",
                "save_document",
                "save_layer_set",
                // #1297: the parametric part search, a query.
                "search_parts",
                "send_drawings_backward",
                "send_drawings_to_back",
                // #2363: the Layers panel's board view configuration.
                "set_active_layer",
                "set_board_graphic",
                "set_board_panel",
                // #2585: a differential pair's track width.
                "set_diff_pair_width",
                // #2470: a dock panel opened, closed, collapsed or expanded.
                "set_dock_panel",
                // #2936: a footprint placed by id on the named board.
                "set_footprint_position",
                "set_footprint_rotation",
                "set_footprint_side",
                "set_footprint_text",
                "set_layer_color",
                "set_layer_sizes",
                "set_layer_stack",
                "set_layer_visible",
                "set_length_group",
                "set_net_class_rules",
                "set_object_transparency",
                "set_object_visible",
                "set_pad_shape",
                // #2578, #2579, #2580, #2593: pour shape, fill rules and
                // thermal relief, and a pad's own teardrop and relief
                // settings, on the named board.
                "set_pad_teardrop",
                "set_pad_thermal_relief",
                "set_pour_corners",
                "set_pour_fill",
                "set_pour_properties",
                "set_pour_thermal_relief",
                // The board's teardrop rule.
                "set_teardrop_rule",
                "set_via_span",
                "show_drc_violation",
                "show_layer_group",
                "show_layer_set",
                "show_used_layers_only",
                "split_pane_down",
                "split_pane_right",
                "stitch_vias",
                "swap_gates",
                "swap_pins",
                "sync_pane_zoom",
                // #1261: the same swaps started on the named sheet.
                "swap_schematic_gates",
                "swap_schematic_pins",
                // #2838: a curve shape drawn on the symbol named.
                "symbol_draw_bezier",
                "symbol_draw_elliptical_arc",
                "symbol_draw_pie",
                "symbol_draw_round_rectangle",
                // #2249: the window layout, by the maintainer's rule that
                // everything is AI-controllable.
                "tile_windows_grid",
                "tile_windows_horizontally",
                "tile_windows_vertically",
                // #2389: length tuning on the named board.
                "tune_diff_pair",
                "tune_net_length",
                "unlock_board_items",
                "untile_documents",
                "unwaive_drc_violation",
                // #2470: Update PCB for the project of the named document,
                // answered with the change list.
                "update_pcb_from_schematic",
                "variant_bom",
                "waive_drc_violation",
                // #2470: Fit on the main window's view.
                "zoom_to_fit",
            ]
        );
    }

    /// #2470: the Annotate dialog is refused with the form the channel
    /// takes, and a file path is refused unless the command writes inside
    /// the project.
    #[test]
    fn the_dialog_names_its_channel_form_and_only_the_project_writers_take_a_path() {
        let why = refusal(&id("annotate_schematic")).expect("refused");
        assert!(why.contains("`annotate_schematic_quietly`"), "{why}");
        for name in WRITES_IN_PROJECT {
            let meta = metadata_for(&id(name)).expect("in the catalog");
            assert!(
                meta.operands
                    .iter()
                    .any(|op| op.name == "path" && op.kind == OperandKind::Path),
                "`{name}` takes no path, so it needs no entry here"
            );
            assert!(
                meta.operands
                    .iter()
                    .any(|op| op.name == "document" && op.required),
                "`{name}` must name its document"
            );
        }
        for name in [
            "export_assembly_svg",
            "export_block_diagram",
            "export_board_step",
            "save_document_as",
        ] {
            let why = refusal(&id(name)).unwrap_or_else(|| panic!("`{name}` was accepted"));
            assert!(why.contains("takes a file path"), "{name}: {why}");
        }
    }

    /// #2495: no extension command is on the agent channel in version 1,
    /// with a reason that says so rather than "unknown command".
    #[test]
    fn an_extension_command_is_refused_as_closed_to_the_socket() {
        for name in ["ext.sample.say_hello", "ext.other.read_notes"] {
            assert_eq!(
                refusal(&id(name)).as_deref(),
                Some(EXTENSION_COMMANDS_CLOSED),
                "{name}"
            );
        }
    }

    #[test]
    fn the_person_only_list_names_catalog_commands_that_would_otherwise_pass() {
        for name in PERSON_ONLY {
            let meta = metadata_for(&id(name)).expect("in the catalog");
            assert!(
                meta.operands
                    .iter()
                    .any(|op| op.kind == OperandKind::Document),
                "`{name}` names no document, so it needs no entry here"
            );
            assert!(meta.headless_refusal().is_none(), "`{name}` is window-only");
        }
    }

    /// Each [`DOCUMENT_NAMED_ON_CHANNEL`] id takes an optional `document`:
    /// a required one needs no entry.
    #[test]
    fn the_named_on_channel_list_names_commands_with_an_optional_document() {
        for name in DOCUMENT_NAMED_ON_CHANNEL {
            let meta = metadata_for(&id(name)).expect("in the catalog");
            assert!(
                meta.operands.iter().any(|op| op.name == "document"
                    && op.kind == OperandKind::Document
                    && !op.required),
                "`{name}` takes no optional document, so it needs no entry here"
            );
        }
    }

    /// A placement takes an optional `document` (#2199); that does not put it
    /// on the agent channel, where it stays reachable through `apply_edits`.
    #[test]
    fn the_placements_stay_off_the_channel_though_they_take_a_document() {
        for name in [
            "place_net_label",
            "place_local_net_label",
            "place_global_label",
            "place_hierarchical_label",
            "place_power_symbol",
            "place_no_connect",
            "place_junction",
            "place_no_erc",
            "place_wire",
            "draw_wire",
            "place_bus",
            "draw_bus",
            "place_text",
            "place_text_string",
            "draw_line",
            "draw_graphic_line",
            "draw_rectangle",
            "draw_ellipse",
            "draw_elliptical_arc",
            "draw_round_rectangle",
        ] {
            let meta = metadata_for(&id(name)).expect("in the catalog");
            assert!(
                meta.operands
                    .iter()
                    .any(|op| op.name == "document" && !op.required),
                "`{name}` takes an optional document"
            );
            let why = refusal(&id(name)).unwrap_or_else(|| panic!("`{name}` was accepted"));
            assert!(why.contains("does not name a document"), "{name}: {why}");
        }
    }

    #[test]
    fn undo_tools_views_and_window_commands_are_refused_with_a_reason() {
        for (name, wanted) in [
            ("undo", "the person's own"),
            ("redo", "the person's own"),
            ("save_document_as", "takes a file path"),
            ("open_document", "takes a file path"),
            ("new_document", "takes a file path"),
            ("open_erc_dialog", "only works in a window"),
            ("place_wire", "does not name a document"),
            ("next_grid", "only works in a window"),
            ("select_all", "does not name a document"),
            ("rotate_clockwise", "does not name a document"),
        ] {
            let why = refusal(&id(name)).unwrap_or_else(|| panic!("`{name}` was accepted"));
            assert!(why.contains(wanted), "{name}: {why}");
        }
        assert!(
            refusal(&id("not_a_command"))
                .unwrap()
                .contains("unknown command")
        );
    }
}
