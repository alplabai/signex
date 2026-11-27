pub mod active_bar;
pub(crate) mod annotations;
pub mod autosave;
pub mod autosave_pcb;
pub mod autosave_primitives;
pub mod autosave_project;
#[cfg(test)]
mod autosave_project_tests;
pub mod autosave_recovery;
pub mod autosave_row;
pub(crate) mod board_3d;
#[cfg(test)]
mod board_3d_model_tests;
mod board_3d_screenshot;
pub(crate) mod board_net_classes;
mod board_sketch;
pub mod canvas;
pub mod clipboard_workflows;
pub(crate) mod command_log;
mod component_history;
mod cross_probe;
#[cfg(test)]
mod detached_confirm_tests;
#[cfg(test)]
mod detached_library_tests;
pub mod detached_modals;
#[cfg(test)]
mod detached_new_project_tests;
#[cfg(test)]
mod detached_wrap_tests;
mod dialog_window;
pub mod dock;
pub mod document_files;
mod document_identity;
pub(crate) mod document_pane_actions;
#[cfg(test)]
mod document_pane_actions_tests;
pub(crate) mod document_panes;
#[cfg(test)]
pub(crate) mod document_panes_tests;
pub mod document_tabs;
#[cfg(test)]
mod document_tabs_tests;
mod document_tiling;
pub mod drawing_order;
pub mod editing_commands;
pub mod erc;
pub mod find_replace;
pub mod find_similar;
pub(crate) mod grid_snap_popover;
mod intro_wrap;
pub(in crate::app) mod keymap_mod_prompt;
pub(crate) mod length_matching;
pub mod library_drift;
pub mod library_footprint_choice;
pub mod library_place;
pub mod library_symbol_choice;
pub mod menu;
pub mod missing_fonts;
mod mixed_versions;
pub(crate) mod modal_sizing;
pub mod native_menu;
pub mod net_nav;
mod object_list;
mod object_list_edit;
mod pan_zoom_bench;
pub(crate) mod pane_views;
#[cfg(test)]
mod pane_views_tests;
mod parameter_closed_sheets;
pub mod parameter_manager;
pub mod paste;
pub mod pcb_drc;
pub(crate) mod pcb_drc_bodies;
mod pcb_drc_check;
pub(crate) mod pcb_drc_fix;
mod pcb_drc_results;
#[cfg(test)]
mod pcb_drc_results_tests;
pub(crate) mod pcb_dxf_import;
pub(crate) mod pcb_dxf_sketch;
pub(crate) use pcb_drc_check::DRC_CHECK;
pub(crate) use pcb_drc_check::LOOSE_BOARD_WARNING;
pub(crate) use pcb_drc_results::DrcWaiverText;
#[cfg(test)]
mod held_rotate_window_tests;
mod home_view;
#[cfg(test)]
mod home_view_tests;
mod pcb_align;
mod pcb_board_options;
mod pcb_board_panel;
#[cfg(test)]
mod pcb_board_store_tests;
mod pcb_clipboard;
#[cfg(test)]
mod pcb_clipboard_tests;
pub(crate) mod pcb_component;
#[cfg(test)]
mod pcb_drc_tests;
mod pcb_edit;
#[cfg(test)]
mod pcb_edit_tests;
mod pcb_filter_presets;
mod pcb_footprint_place;
pub(crate) mod pcb_footprint_text;
pub(crate) mod pcb_item_edits;
mod pcb_item_move;
#[cfg(test)]
mod pcb_item_move_tests;
pub(crate) mod pcb_layer_stack;
pub(crate) mod pcb_layer_strip;
mod pcb_layers_panel;
mod pcb_net_focus;
#[cfg(test)]
mod pcb_net_focus_tests;
#[cfg(test)]
mod pcb_net_focus_window_tests;
mod pcb_nets_panel;
#[cfg(test)]
mod pcb_nets_panel_tests;
#[cfg(test)]
mod pcb_one_grid_tests;
mod pcb_origin;
#[cfg(test)]
mod pcb_origin_tests;
mod pcb_parking;
mod pcb_part_place;
pub(crate) mod pcb_placement;
#[cfg(test)]
mod pcb_placement_tests;
#[cfg(test)]
mod pcb_placement_window_tests;
pub(crate) mod pcb_pour;
pub(crate) mod pcb_primitive;
pub(crate) mod pcb_query;
pub mod pcb_select;
#[cfg(test)]
mod pcb_select_tests;
#[cfg(test)]
mod pcb_snap_options_tests;
mod pcb_text_move;
#[cfg(test)]
mod pcb_text_pick_tests;
pub(crate) mod pcb_tool;
pub(crate) mod pcb_tune;
#[cfg(test)]
mod pcb_tune_tests;
#[cfg(test)]
mod pcb_unselected_drag_tests;
mod pcb_via_move;
#[cfg(test)]
mod pcb_via_move_tests;
pub mod pcb_view;
mod pcb_view_persist;
pub mod pcb_view_rotation;
pub mod placed_part;
pub(crate) mod placement_extras;
pub mod placement_session;
mod polygon_manager;
pub mod preferences;
pub mod project_case;
mod project_fs;
mod repeat_placement;
mod requirements_check;
pub(crate) use requirements_check::REQUIREMENTS_CHECK;
pub(crate) use requirements_check::board_path as project_board_path;
pub(crate) use requirements_check::bom::{BomInputs, project_boms};
pub mod selection_transform;
pub mod selection_workflow;
pub mod window_controls;
pub mod window_geometry_events;
pub mod window_placement;
pub mod window_tiling;
pub(crate) mod workspace_layouts;
#[cfg(test)]
mod workspace_layouts_undo_tests;
