//! The commands that run their operands (#1934, #2155): a command acts on the
//! operands a caller parsed ([`CommandArgs::parse_operands`]) only if it has
//! a handler here. One that declares operands and has none is refused
//! ([`Signex::run_operands`]), so no caller is ever told `Ran` for operands
//! nobody read. There is no flag to switch: a handler is code that reads them.
//!
//! A handler runs the same app commit path as the click or the button of the
//! same id, so one command id has one behaviour on every surface. A command
//! that names a document (`document_operands`) acts on that document alone and
//! is not gated by what the active tab can do.

use iced::Task;

use super::agent_edits as agent;
use super::board_area_operands as area;
use super::board_edit_operands as board_edit;
use super::board_graphic_operands as graphic;
use super::board_operands as board;
use super::board_route_operands as route;
use super::board_view_operands as board_view;
use super::curve_operands as curve;
use super::dimension_operands as dimension;
use super::directive_operands as directive;
use super::document_operands as doc;
use super::file_operands as file;
use super::footprint_assign_operands as fp_assign;
use super::footprint_place_operands as fp_place;
use super::footprint_text_operands as fp_text;
use super::hierarchy_operands as hierarchy;
use super::layer_strip_operands as strip;
use super::live_erc_operands as live_erc;
use super::part_height_operands as part_heights;
use super::part_operands as part;
use super::placed_operands as placed;
use super::placement_extras_operands as extras;
use super::placement_operands as place;
use super::repair_reset_operands as repair_reset;
use super::reuse_operands as snippet;
use super::rich_note_operands as rich_note;
use super::schematic_pdf_operands as schematic_pdf;
use super::selection_filter_operands as filter;
use super::selection_operands as selection;
use super::sheet_swap_operands as sheet_swap;
use super::sheet_sync_operands as sheet_sync;
use super::smart_paste_operands as smart_paste;
use super::symbol_pin_operands as symbol_pins;
use super::symbol_shape_operands as symbol_shape;
use super::{CommandArgs, CommandOutcome, enablement};
use crate::app::block_diagram as diagram;
use crate::app::board_output as output;
use crate::app::board_reuse as reuse;
use crate::app::board_tools as tools;
use crate::app::handlers::pcb_component::part_swap_operands as swap;
use crate::app::handlers::pcb_tune as tune;
use crate::app::notebook_references::commands as notebook;
use crate::app::rooms;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

/// What running one command from its operands did, and any follow-up task.
type Ran = (CommandOutcome, Task<Message>);

/// Runs one command from its parsed operands.
pub(crate) type OperandHandler = fn(&mut Signex, &CommandArgs) -> Ran;

/// Every command that runs its operands, by id. Each needs a click-equivalence
/// test (`operand_handler_tests`, `placement_operand_tests`);
/// `every_operand_command_has_a_handler_or_is_refused`
/// keeps this table and the catalog in step.
const HANDLERS: &[(&str, OperandHandler)] = &[
    (
        "new_project_with_profile",
        crate::app::new_project::new_project_with_profile,
    ),
    ("color_net", doc::color_net),
    ("clear_all_net_colors", doc::clear_all_net_colors),
    ("undo", doc::undo),
    ("redo", doc::redo),
    ("save_document", doc::save),
    ("save_document_as", doc::save_as),
    ("close_active_document", doc::close),
    ("open_document", file::open),
    ("new_document", file::new_project),
    (
        "new_pcb_document",
        crate::app::document_setup::new_pcb_document,
    ),
    (
        "new_schematic_sheet",
        crate::app::document_setup::new_schematic_sheet,
    ),
    ("assign_net_class", doc::assign_net_class),
    (
        "update_pcb_from_schematic",
        crate::app::update_pcb::update_pcb_from_schematic,
    ),
    // #2470: numbering on the document named, answered with each change.
    (
        "annotate_schematic_quietly",
        crate::app::handlers::erc::annotate_quietly_by_operands,
    ),
    (
        "force_annotate_all_schematics",
        crate::app::handlers::erc::force_annotate_by_operands,
    ),
    (
        "reannotate_board",
        crate::app::board_reannotate::reannotate_board,
    ),
    (
        "back_annotate_schematics",
        crate::app::back_annotate::back_annotate_schematics,
    ),
    // #2462: the Compare Board with Schematic dialog's Apply, by row id.
    (
        "apply_board_schematic_differences",
        crate::app::compare_board::apply_board_schematic_differences,
    ),
    ("apply_edits", agent::apply_edits),
    ("revert", agent::revert),
    ("place_net_label", place::net_label),
    ("place_local_net_label", place::net_label),
    ("place_global_label", place::global_label),
    ("place_hierarchical_label", place::hierarchical_label),
    ("place_power_symbol", place::power),
    ("place_no_connect", place::no_connect),
    ("place_junction", place::junction),
    ("place_no_erc", place::no_erc),
    ("place_wire", place::wire),
    ("draw_wire", place::wire),
    ("place_bus", place::bus),
    ("draw_bus", place::bus),
    ("place_text", place::text_note),
    ("place_text_string", place::text_note),
    ("draw_line", place::line),
    ("draw_graphic_line", place::line),
    ("draw_rectangle", place::rectangle),
    // #1267: the curve drawings.
    ("draw_ellipse", curve::ellipse),
    ("draw_elliptical_arc", curve::elliptical_arc),
    ("draw_round_rectangle", curve::round_rectangle),
    // #2836: the dimension tools' three clicks and form.
    ("place_linear_dimension", dimension::linear),
    ("place_aligned_dimension", dimension::aligned),
    ("place_blanket", directive::blanket),
    ("edit_blanket", directive::edit_blanket),
    ("edit_no_erc_rules", directive::edit_no_erc_rules),
    ("place_compile_mask", directive::compile_mask),
    ("place_diff_pair", directive::diff_pair),
    ("place_parameter_set", directive::net_parameters),
    ("autoplace_fields", extras::autoplace_fields),
    ("place_design_block", extras::place_design_block),
    // #1283: snippets, kept per project and in the user's library.
    ("save_snippet", snippet::save_snippet),
    ("place_snippet", snippet::place_snippet),
    ("import_graphics", extras::import_graphics),
    // #1116: an image as a project asset, its fields, Relink and its folder.
    ("place_graphic", super::image_operands::place_graphic),
    ("set_image", super::image_operands::set_image),
    ("relink_image", super::image_operands::relink_image),
    (
        "show_image_in_folder",
        super::image_operands::show_image_in_folder,
    ),
    // #2665: the Smart Paste dialog's choices, placed at once.
    ("smart_paste", smart_paste::smart_paste),
    ("place_component", part::place_component),
    ("place_sheet_symbol", hierarchy::place_sheet_symbol),
    ("place_sheet_entry", hierarchy::place_sheet_entry),
    ("sync_sheet_pins", hierarchy::sync_sheet_pins),
    // #1276: bottom-up hierarchy.
    (
        "create_sheet_symbol_from_sheet",
        sheet_sync::create_sheet_symbol_from_sheet,
    ),
    ("sync_sheet_entries", sheet_sync::sync_sheet_entries),
    // #2968: a block diagram from the sheet hierarchy.
    (
        "generate_block_diagram",
        crate::app::block_diagram_sync::generate_block_diagram,
    ),
    (
        "regenerate_block_diagram",
        crate::app::block_diagram_sync::regenerate_block_diagram,
    ),
    ("set_object_position", placed::set_object_position),
    ("set_object_rotation", placed::set_object_rotation),
    ("set_sheet_symbol_size", placed::set_sheet_symbol_size),
    ("set_symbol_parameter", placed::set_symbol_parameter),
    ("set_object_text", placed::set_object_text),
    ("set_sheet_variable", placed::set_sheet_variable),
    ("set_text_note_format", placed::set_text_note_format),
    // #1114, mockup row 203: a rich text note and its in-place editor.
    ("set_text_note_frame", rich_note::set_text_note_frame),
    ("set_text_note_size", rich_note::set_text_note_size),
    ("apply_rich_note_edit", rich_note::apply_rich_note_edit),
    ("edit_rich_note", crate::app::rich_note::edit_rich_note),
    (
        "cancel_rich_note_edit",
        crate::app::rich_note::cancel_rich_note_edit,
    ),
    (
        "wrap_rich_note_selection",
        crate::app::rich_note::wrap_rich_note_selection,
    ),
    (
        "open_rich_note_link",
        crate::app::rich_note::open_rich_note_link,
    ),
    (
        "compile_rich_note",
        crate::app::rich_note::compile_rich_note,
    ),
    ("set_net_tie", placed::set_net_tie),
    ("repair_pin_junctions", repair_reset::repair_pin_junctions),
    // #1288: the live ERC preference.
    ("toggle_live_erc", live_erc::toggle_live_erc),
    ("reset_board_origin", repair_reset::reset_board_origin),
    ("reset_layer_colors", repair_reset::reset_layer_colors),
    ("view_rotate", view_rotate),
    ("show_layer_set", strip::show_layer_set),
    ("save_layer_set", strip::save_layer_set),
    ("delete_layer_set", strip::delete_layer_set),
    ("hide_layer", strip::hide_layer),
    ("highlight_layer", strip::highlight_layer),
    ("hide_layer_group", strip::hide_layer_group),
    ("show_layer_group", strip::show_layer_group),
    ("set_layer_tab_names", strip::set_layer_tab_names),
    // #2363: the Layers panel's board view configuration.
    ("set_layer_visible", board_view::set_layer_visible),
    ("set_active_layer", board_view::set_active_layer),
    ("set_layer_color", board_view::set_layer_color),
    ("show_used_layers_only", board_view::show_used_layers_only),
    ("set_object_visible", board_view::set_object_visible),
    (
        "set_object_transparency",
        board_view::set_object_transparency,
    ),
    ("board_report_bom", output::board_report_bom),
    ("board_report_netlist", output::board_report_netlist),
    ("board_report_net_length", output::board_report_net_length),
    ("board_report_components", output::board_report_components),
    ("export_board_report", output::export_board_report),
    ("export_board_pdf", output::export_board_pdf),
    ("export_assembly_svg", output::export_assembly_svg),
    ("board_assembly_drawing", output::board_assembly_drawing),
    ("export_board_step", output::export_board_step),
    // #2796: a block diagram's saved file as SVG or PDF.
    (
        "export_block_diagram",
        crate::app::block_diagram_export::export_block_diagram,
    ),
    // Mockup row 202: the project's notebook as PDF.
    (
        "export_notebook_pdf",
        crate::app::notebook_pdf::export_notebook_pdf,
    ),
    ("export_schematic_pdf", schematic_pdf::export_schematic_pdf),
    // #2470: the Fabrication Outputs dialog's Generate from operands.
    (
        "fabrication_outputs",
        crate::app::fab_outputs::fabrication_outputs,
    ),
    ("add_board_graphic", graphic::add_board_graphic),
    ("set_board_graphic", graphic::set_board_graphic),
    // #2470: the area tools on the board `document` names.
    ("place_board_fill", area::place_fill),
    ("place_board_region", area::place_region),
    ("place_board_polygon_pour", area::place_polygon_pour),
    ("place_board_keepout", area::place_keepout),
    ("place_board_pour_cutout", area::place_pour_cutout),
    ("set_footprint_text", fp_text::set_footprint_text),
    // #2838: the symbol editor's curve shapes on the symbol named.
    ("symbol_draw_elliptical_arc", symbol_shape::elliptical_arc),
    ("symbol_draw_bezier", symbol_shape::bezier),
    ("symbol_draw_round_rectangle", symbol_shape::round_rectangle),
    ("symbol_draw_pie", symbol_shape::pie),
    // #2817: the Pin table's bulk edit and Paste rows on a symbol file.
    ("edit_symbol_pins", symbol_pins::edit_symbol_pins),
    // #2936, #2937: board footprints by id, and footprint assignment.
    ("set_footprint_position", fp_place::set_footprint_position),
    ("set_footprint_rotation", fp_place::set_footprint_rotation),
    ("set_footprint_side", fp_place::set_footprint_side),
    ("assign_footprint", fp_assign::assign_footprint),
    ("change_footprint", fp_assign::change_footprint),
    // #2470: a dock panel by its layout key.
    (
        "set_dock_panel",
        crate::app::handlers::dock::panel_command::set_dock_panel,
    ),
    ("set_selection_filters", filter::set_selection_filters),
    (
        "apply_selection_filter_preset",
        filter::apply_selection_filter_preset,
    ),
    // #2495: a key binding in a custom keyboard profile.
    ("keymap_assign", super::keymap_operands::keymap_assign),
    ("keymap_clear", super::keymap_operands::keymap_clear),
    ("rules_add", crate::app::rules_tab::rules_add),
    ("rules_set", crate::app::rules_tab::rules_set),
    ("rules_remove", crate::app::rules_tab::rules_remove),
    ("rules_enable", crate::app::rules_tab::rules_enable),
    ("show_drc_violation", board::show_drc_violation),
    // #2939: each part's height and where it came from.
    ("query_part_heights", part_heights::query_part_heights),
    ("waive_drc_violation", board::waive_drc_violation),
    ("unwaive_drc_violation", board::unwaive_drc_violation),
    ("fix_drc_violation", board::fix_drc_violation),
    ("fix_drc", board::fix_drc),
    ("filter_drc_results", board::filter_drc_results),
    ("add_net_class", board::add_net_class),
    ("rename_net_class", board::rename_net_class),
    ("delete_net_class", board::delete_net_class),
    ("set_net_class_rules", board::set_net_class_rules),
    ("assign_board_net_class", board::assign_board_net_class),
    ("set_layer_stack", board_edit::set_layer_stack),
    ("lock_board_items", board_edit::lock_board_items),
    ("unlock_board_items", board_edit::unlock_board_items),
    ("set_via_span", board_edit::set_via_span),
    ("set_layer_sizes", board_edit::set_layer_sizes),
    ("set_pad_shape", board_edit::set_pad_shape),
    ("select_board_net", select_board_net),
    ("highlight_board_net", highlight_board_net),
    // #2408, #2409: the board Tools edits on the board `document` names.
    ("stitch_vias", tools::stitch_by_operands),
    ("add_teardrops", tools::add_teardrops_by_operands),
    ("remove_teardrops", tools::remove_teardrops_by_operands),
    ("gloss_selected_routes", tools::gloss_by_operands),
    ("board_shape_from_selection", tools::board_shape_by_operands),
    ("keepout_from_selection", tools::keepout_by_operands),
    ("arrange_in_box", tools::arrange_in_box_by_operands),
    // #2470: the routing tools and the via, net by net on a named board.
    ("place_board_via", route::place_board_via),
    ("place_board_track", route::place_board_track),
    ("place_board_route", route::place_board_route),
    ("place_board_diff_pair", route::place_board_diff_pair),
    ("place_board_quick_route", route::place_board_quick_route),
    ("place_board_multi_route", route::place_board_multi_route),
    // #2457, #2941, #2935: rooms on the board `document` names.
    ("place_board_room", rooms::place_room_by_operands),
    ("create_rooms_from_sheets", rooms::create_rooms_by_operands),
    ("arrange_in_room", rooms::arrange_in_room_by_operands),
    ("auto_place", rooms::auto_place_by_operands),
    (
        "import_dxf",
        crate::app::handlers::pcb_dxf_import::import_dxf,
    ),
    (
        "import_dxf_outline",
        crate::app::handlers::pcb_dxf_import::import_dxf_outline,
    ),
    // Mockup row 199: the notebook's timeline export, Mark still valid and
    // the copy of a file into notebook/attachments/.
    (
        "export_notebook_timeline",
        crate::app::notebook_measurements::export_notebook_timeline,
    ),
    (
        "mark_notebook_attachment_valid",
        crate::app::notebook_measurements::mark_notebook_attachment_valid,
    ),
    (
        "attach_notebook_file",
        crate::app::notebook_measurements::attach_notebook_file,
    ),
    // #2945, mockup row 201: the design report, answered once it is built.
    (
        "generate_design_notebook",
        crate::app::notebook_generate::generate_design_notebook,
    ),
    ("rules_matrix_set", crate::app::rules_tab::rules_matrix_set),
    ("rules_set_create", crate::app::rules_tab::rules_set_create),
    ("rules_set_edit", crate::app::rules_tab::rules_set_edit),
    ("rules_set_apply", crate::app::rules_tab::rules_set_apply),
    (
        "rules_preset_apply",
        crate::app::rules_tab::rules_preset_apply,
    ),
    ("repour_all", crate::app::handlers::pcb_pour::repour_all),
    // #2389: length tuning on a named board.
    ("tune_net_length", tune::tune_net_length),
    ("tune_diff_pair", tune::tune_diff_pair),
    ("panelize_board", crate::app::board_panel::panelize_board),
    (
        "remove_board_panel",
        crate::app::board_panel::remove_board_panel,
    ),
    ("set_board_panel", crate::app::board_panel::set_board_panel),
    // #2942: one stage of the named board's layout plan.
    ("layout_plan_set", crate::app::layout_plan::layout_plan_set),
    ("swap_pins", swap::swap_pins),
    ("swap_gates", swap::swap_gates),
    // #1261: the same swaps started on the named sheet.
    ("swap_schematic_pins", sheet_swap::swap_schematic_pins),
    ("swap_schematic_gates", sheet_swap::swap_schematic_gates),
    ("save_board_reuse_block", reuse::save_board_reuse_block),
    ("place_board_reuse_block", reuse::place_board_reuse_block),
    // #2326: the drawing order and Delete on the `document` named.
    ("bring_drawings_forward", selection::bring_drawings_forward),
    (
        "bring_drawings_to_front",
        selection::bring_drawings_to_front,
    ),
    ("send_drawings_backward", selection::send_drawings_backward),
    ("send_drawings_to_back", selection::send_drawings_to_back),
    ("delete_selection", selection::delete_selection),
    // #1689-#1693: the Project tab's section edits.
    (
        "project_move_sheet",
        crate::app::project_tab::project_move_sheet,
    ),
    (
        "project_add_variant",
        crate::app::project_tab::project_add_variant,
    ),
    (
        "project_rename_variant",
        crate::app::project_tab::project_rename_variant,
    ),
    (
        "project_remove_variant",
        crate::app::project_tab::project_remove_variant,
    ),
    (
        "project_set_active_variant",
        crate::app::project_tab::project_set_active_variant,
    ),
    (
        "project_set_default_variant",
        crate::app::project_tab::project_set_default_variant,
    ),
    // #3038: the Variant Manager's part edits and Duplicate.
    (
        "project_set_variant_fit",
        crate::app::project_tab::project_set_variant_fit,
    ),
    (
        "project_set_variant_alternate",
        crate::app::project_tab::project_set_variant_alternate,
    ),
    (
        "project_set_variant_override",
        crate::app::project_tab::project_set_variant_override,
    ),
    (
        "project_duplicate_variant",
        crate::app::project_tab::project_duplicate_variant,
    ),
    (
        "project_move_library",
        crate::app::project_tab::project_move_library,
    ),
    (
        "project_set_annotate_undo",
        crate::app::project_tab::project_set_annotate_undo,
    ),
    (
        "project_set_annotate_numbering",
        crate::app::project_tab::project_set_annotate_numbering,
    ),
    (
        "project_set_version_control",
        crate::app::project_tab::project_set_version_control,
    ),
    // #2339: the Project tab's Requirements section.
    (
        "project_add_requirement",
        crate::app::project_tab::project_add_requirement,
    ),
    (
        "project_set_requirement",
        crate::app::project_tab::project_set_requirement,
    ),
    (
        "project_remove_requirement",
        crate::app::project_tab::project_remove_requirement,
    ),
    (
        "project_enable_requirement",
        crate::app::project_tab::project_enable_requirement,
    ),
    // #2943: the project's power rails.
    (
        "project_set_rail",
        crate::app::project_tab::project_set_rail,
    ),
    (
        "project_run_requirements",
        crate::app::project_tab::project_run_requirements,
    ),
    // #2796: the block diagram document (mockup row 171).
    ("new_block_diagram", diagram::new_block_diagram),
    ("block_add", diagram::block_add),
    ("block_set", diagram::block_set),
    ("block_remove", diagram::block_remove),
    ("block_port_add", diagram::block_port_add),
    ("block_port_remove", diagram::block_port_remove),
    ("block_link", diagram::block_link),
    ("block_link_set", diagram::block_link_set),
    ("block_unlink", diagram::block_unlink),
    ("block_group", diagram::block_group),
    ("block_group_set", diagram::block_group_set),
    ("block_ungroup", diagram::block_ungroup),
    // #2795: the Design Notebook tab, on the project `document` names.
    (
        "notebook_compile",
        crate::app::notebook_tab::notebook_compile,
    ),
    ("notebook_insert", crate::app::notebook_tab::notebook_insert),
    ("notebook_view", crate::app::notebook_tab::notebook_view),
    // Mockup rows 196 and 197: the notebook's references and badges.
    ("notebook_references", notebook::notebook_references),
    ("notebook_reference_card", notebook::notebook_reference_card),
    (
        "notebook_reference_picker",
        notebook::notebook_reference_picker,
    ),
    ("notebook_badges", notebook::notebook_badges),
    ("notebook_fix_reference", notebook::notebook_fix_reference),
    // #1178: the Project tab's Fonts section.
    (
        "project_add_font",
        crate::app::project_tab::project_add_font,
    ),
    (
        "project_remove_font",
        crate::app::project_tab::project_remove_font,
    ),
    (
        "project_set_font_license",
        crate::app::project_tab::project_set_font_license,
    ),
    // #2938: a BOM line's alternates, one General step each.
    (
        "bom_add_alternate",
        crate::app::project_tab::bom_add_alternate,
    ),
    (
        "bom_remove_alternate",
        crate::app::project_tab::bom_remove_alternate,
    ),
    (
        "bom_set_alternate_order",
        crate::app::project_tab::bom_set_alternate_order,
    ),
    // #1297, #2954: the placed part `id` names swapped for a library row,
    // the pins mapped by hand if given.
    (
        "change_component",
        super::change_component_operands::change_component,
    ),
];

/// The turn `view_rotate` asks for: its `degrees`, else a quarter turn
/// clockwise (#2316).
pub(super) fn view_turn_degrees(args: &CommandArgs) -> f32 {
    match args.operand("degrees") {
        Some(super::OperandValue::Angle(degrees)) => *degrees as f32,
        _ => 90.0,
    }
}

/// `view_rotate` from its operands: the message its menu row and Active Bar
/// button send, so the board view turns the same way on every surface.
fn view_rotate(app: &mut Signex, args: &CommandArgs) -> Ran {
    let turn = crate::app::ViewRotationMsg::TurnedBy(view_turn_degrees(args));
    let task = app.update(Message::Pcb(crate::app::PcbMsg::ViewRotation(turn)));
    (CommandOutcome::Ran, task)
}

/// The board net `net` names, for Select Net and Highlight Net (#2403).
pub(super) fn board_net_operand(args: &CommandArgs) -> Option<String> {
    super::queries::text_operand(args, "net").map(str::to_owned)
}

/// The board net `net` names, refused when the board in view has no net of
/// that name; `None` for the net of the selection.
fn named_board_net(app: &Signex, args: &CommandArgs) -> Result<Option<String>, Ran> {
    let Some(name) = board_net_operand(args) else {
        return Ok(None);
    };
    match app.active_pcb() {
        Some(board) if pcb_geom::net_named(board, &name).is_none() => {
            Err(refused(format!("the board has no net named `{name}`")))
        }
        _ => Ok(Some(name)),
    }
}

/// `select_board_net net=..` (#2403): what the Net Inspector row's Select
/// sends.
fn select_board_net(app: &mut Signex, args: &CommandArgs) -> Ran {
    let name = match named_board_net(app, args) {
        Ok(name) => name,
        Err(refusal) => return refusal,
    };
    let net = crate::app::BoardNetMsg::SelectRequested(name);
    let task = app.update(Message::Pcb(crate::app::PcbMsg::Net(net)));
    (CommandOutcome::Ran, task)
}

/// `highlight_board_net net=..` (#2403): what the Net Inspector row's
/// Highlight sends.
fn highlight_board_net(app: &mut Signex, args: &CommandArgs) -> Ran {
    let name = match named_board_net(app, args) {
        Ok(name) => name,
        Err(refusal) => return refusal,
    };
    let net = crate::app::BoardNetMsg::HighlightRequested(name);
    let task = app.update(Message::Pcb(crate::app::PcbMsg::Net(net)));
    (CommandOutcome::Ran, task)
}

/// The handler of `command`, if it runs its operands.
pub(crate) fn operand_handler(command: &AppCommandId) -> Option<OperandHandler> {
    HANDLERS
        .iter()
        .find(|(id, _)| *id == command.as_str())
        .map(|(_, handler)| *handler)
}

/// Commands a headless run hands to their handler even with no operands:
/// the GUI form opens a setup dialog, the headless form makes the document
/// from the project's own setup (#2232), as it did before the dialog.
/// `create_sheet_symbol_from_sheet` (#1276) goes there too: its GUI form only
/// opens a file picker, which a headless run refuses, so the handler refuses
/// it for the missing `path` rather than the picker's refusal reading as run.
/// `repair_pin_junctions` goes there too, so a headless run on the sheet in
/// view answers how many dots it added (0 when nothing needed one) instead of
/// a bare `Ran`. `toggle_live_erc` too, so a headless flip answers with the
/// preference it left. `update_pcb_from_schematic` (#2470) goes there so a
/// bare headless run is refused for `apply=true` or `dry_run=true` rather
/// than opening a dialog nobody sees.
pub(crate) fn runs_headless_without_operands(command: &AppCommandId) -> bool {
    matches!(
        command.as_str(),
        "new_pcb_document"
            | "new_schematic_sheet"
            | "create_sheet_symbol_from_sheet"
            | "repair_pin_junctions"
            | "new_block_diagram"
            | "toggle_live_erc"
            | "update_pcb_from_schematic"
    )
}

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

/// The ids that run their operands.
#[cfg(test)]
pub(super) fn handled_ids() -> impl Iterator<Item = &'static str> {
    HANDLERS.iter().map(|(id, _)| *id)
}

/// Does `command` always name its document (a required `document` operand)?
/// A placement takes an optional one and keeps the active tab's gate without it.
pub(crate) fn names_its_document(command: &AppCommandId) -> bool {
    crate::keymap::metadata_for(command).is_some_and(|meta| {
        meta.operands
            .iter()
            .any(|op| op.name == "document" && op.required)
    })
}

/// Does `command`, run from operands without `document`, pick its project
/// itself rather than through the active tab's gate? The catalog says so
/// (`CommandMetadata::picks_own_project`): `update_pcb_from_schematic`
/// (#2470) takes the active project's board, or the one project's when none
/// is active, and refuses with the boards to name otherwise (Caner,
/// 2026-11-25).
pub(crate) fn picks_its_own_project(command: &AppCommandId) -> bool {
    crate::keymap::metadata_for(command).is_some_and(|meta| meta.picks_own_project)
}

impl Signex {
    /// Run `command` from the operands in `args`: its handler. A command with
    /// no handler is refused. One that names its document (`document`) is
    /// run on that document, with ids made unique first and without the
    /// active tab's enablement; one that [picks its own
    /// project](picks_its_own_project) runs without that gate too; any other
    /// is behind the same enablement gate as a dispatch.
    pub(crate) fn run_operands(&mut self, command: &AppCommandId, args: &CommandArgs) -> Ran {
        let Some(handler) = operand_handler(command) else {
            return refused("operands are declared but not run yet (#1934)");
        };
        if names_its_document(command) || args.operand("document").is_some() {
            self.reconcile_document_ids();
        } else if !picks_its_own_project(command)
            && let Some(reason) = enablement::disabled_reason(command, &self.command_context(None))
        {
            let outcome = CommandOutcome::Disabled {
                reason: reason.into(),
            };
            return (outcome, Task::none());
        }
        // What its checks read from disk is read off the UI thread first, and
        // the command runs when that lands (#2199).
        if let Some(deferred) = self.defer_for_reads(command, args) {
            return deferred;
        }
        self.document_state.agent_edits.project_wanted.set(false);
        // The socket only: a headless run has no person, and its own `undo`
        // refuses a grouped agent step (docs/AGENT_EDITS.md, "Origin").
        let ran = if self.agent.serving && agent::runs_as_one_step(command) {
            agent::run_as_one_step(self, command, args, handler)
        } else {
            handler(self, args)
        };
        // A check found that it needs the project's closed sheets: it refused
        // with nothing applied; read them and run the command again.
        if self.document_state.agent_edits.project_wanted.take()
            && matches!(ran.0, CommandOutcome::Disabled { .. })
            && let Some(deferred) = self.defer_for_project(command, args)
        {
            return deferred;
        }
        ran
    }
}

pub(crate) fn refused(reason: impl Into<String>) -> Ran {
    (CommandOutcome::disabled(reason), Task::none())
}
