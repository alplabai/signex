//! Which commands the agent channel accepts (#1924), derived from the command
//! catalog: the #2172 queries, and a command that names its document with the
//! `document` operand and no file path (`apply_edits` and `revert` are two of
//! them), plus the window-layout commands (#2249). Everything else is refused
//! with the reason. See
//! `docs/AGENT_SOCKET.md`, "Allowlist".

use crate::app::command::queries::is_query;
use crate::keymap::{AppCommandId, CatalogTextExt, OperandKind, metadata_for};

/// Commands that name a document and are still the person's: their undo
/// stack is theirs, and an agent takes back its own step with `revert`.
/// `consistency` in the tests keeps this list to ids the catalog has.
pub(super) const PERSON_ONLY: &[&str] = &["undo", "redo"];

/// The window-layout commands (#2249): they name no document and act on the
/// main window's layout, open to agents by the maintainer's rule that
/// everything is AI-controllable. Each answers with the layout it left.
pub(super) const WINDOW_LAYOUT: &[&str] = &[
    "tile_windows_horizontally",
    "tile_windows_vertically",
    "untile_documents",
];

/// Is `command` one of [`WINDOW_LAYOUT`]?
pub(in crate::app) fn is_window_layout(command: &AppCommandId) -> bool {
    WINDOW_LAYOUT.contains(&command.as_str())
}

/// Why `command` is not on the agent channel, or `None` when it is.
pub(super) fn refusal(command: &AppCommandId) -> Option<String> {
    let id = command.as_str();
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
    if let Some(reason) = meta.headless_refusal() {
        return Some(reason);
    }
    if meta.operands.iter().any(|op| op.kind == OperandKind::Path) {
        return Some(format!(
            "`{id}` takes a file path; the agent channel does not read or write a path it names"
        ));
    }
    // A required `document`: a placement's optional one does not open it, so a
    // placement stays reachable only through `apply_edits` (#2199).
    let names_document = meta
        .operands
        .iter()
        .any(|op| op.name == "document" && op.kind == OperandKind::Document && op.required);
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
                "apply_edits",
                "arrange_in_box",
                "assign_board_net_class",
                "assign_net_class",
                "autoplace_fields",
                "board_assembly_drawing",
                "board_panel",
                "board_report_bom",
                "board_report_components",
                "board_report_net_length",
                "board_report_netlist",
                "board_shape_from_selection",
                // #2326 slice 6.P2b: the drawing order and Delete, on the
                // items `ids` names on the named sheet (or board, Delete).
                "bring_drawings_forward",
                "bring_drawings_to_front",
                "check_drc",
                "check_erc",
                "check_requirements",
                "clear_all_net_colors",
                "close_active_document",
                "color_net",
                "delete_net_class",
                "delete_selection",
                "document_info",
                "filter_drc_results",
                "find_items",
                "find_similar_items",
                "gloss_selected_routes",
                // #2394: the board view's layers, on the named board.
                "hide_layer",
                "hide_layer_group",
                "highlight_layer",
                "keepout_from_selection",
                "list_documents",
                "lock_board_items",
                "measure_board",
                "panelize_board",
                // #2410: pin and gate swap and design reuse, on the named board.
                "place_board_reuse_block",
                // #2339: the Requirements section's edits and run.
                "project_add_requirement",
                "project_add_variant",
                "project_enable_requirement",
                "project_move_library",
                "project_move_sheet",
                "project_remove_requirement",
                "project_remove_variant",
                "project_rename_variant",
                "project_run_requirements",
                "project_set_active_variant",
                "project_set_annotate_undo",
                "project_set_requirement",
                "project_set_version_control",
                "project_settings",
                "query_board_nets",
                "query_pins",
                "read_document",
                "remove_board_panel",
                "remove_teardrops",
                "rename_net_class",
                // #2216: each pour's fill of the named board, as JSON.
                "repour_all",
                // The Reset rows of the board menus, on the named board.
                "reset_board_origin",
                "reset_layer_colors",
                "resolve",
                "revert",
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
                "send_drawings_backward",
                "send_drawings_to_back",
                // #2363: the Layers panel's board view configuration.
                "set_active_layer",
                "set_board_graphic",
                "set_board_panel",
                "set_footprint_text",
                "set_layer_color",
                "set_layer_sizes",
                "set_layer_stack",
                "set_layer_visible",
                "set_net_class_rules",
                "set_object_transparency",
                "set_object_visible",
                "set_pad_shape",
                // The board's teardrop rule, a pad's own teardrop setting
                // and a pour's fill rules, on the named board.
                "set_pad_teardrop",
                "set_pour_properties",
                "set_teardrop_rule",
                "set_via_span",
                "show_drc_violation",
                "show_layer_group",
                "show_layer_set",
                "show_used_layers_only",
                "stitch_vias",
                "swap_gates",
                "swap_pins",
                // #2249: the window layout, by the maintainer's rule that
                // everything is AI-controllable.
                "tile_windows_horizontally",
                "tile_windows_vertically",
                // #2389: length tuning on the named board.
                "tune_diff_pair",
                "tune_net_length",
                "unlock_board_items",
                "untile_documents",
                "unwaive_drc_violation",
                "waive_drc_violation",
            ]
        );
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
