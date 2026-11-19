pub mod active_bar;
pub mod autosave;
pub mod autosave_pcb;
pub mod autosave_primitives;
pub mod autosave_project;
pub mod autosave_recovery;
pub mod autosave_row;
pub(crate) mod board_3d;
mod board_3d_screenshot;
pub(crate) mod board_net_classes;
mod board_sketch;
pub mod canvas;
pub mod clipboard_workflows;
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
pub mod document_tabs;
mod document_tiling;
pub mod drawing_order;
pub mod editing_commands;
pub mod erc;
pub mod find_replace;
pub mod find_similar;
mod intro_wrap;
pub(in crate::app) mod keymap_mod_prompt;
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
mod pan_zoom_bench;
pub mod parameter_manager;
pub mod paste;
pub mod pcb_drc;
mod pcb_drc_check;
mod pcb_drc_results;
#[cfg(test)]
mod pcb_drc_results_tests;
pub(crate) mod pcb_dxf_import;
pub(crate) use pcb_drc_check::DRC_CHECK;
pub(crate) use pcb_drc_check::LOOSE_BOARD_WARNING;
mod pcb_align;
mod pcb_board_options;
mod pcb_board_panel;
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
pub(crate) mod pcb_footprint_text;
pub(crate) mod pcb_item_edits;
pub(crate) mod pcb_layer_stack;
pub(crate) mod pcb_layer_strip;
mod pcb_layers_panel;
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
mod pcb_text_move;
#[cfg(test)]
mod pcb_text_pick_tests;
pub(crate) mod pcb_tool;
pub(crate) mod pcb_tune;
#[cfg(test)]
mod pcb_tune_tests;
pub mod pcb_view;
pub mod pcb_view_rotation;
pub mod placed_part;
pub(crate) mod placement_extras;
pub mod placement_session;
pub mod preferences;
pub mod project_case;
mod project_fs;
mod repeat_placement;
mod requirements_check;
pub(crate) use requirements_check::REQUIREMENTS_CHECK;
pub mod selection_transform;
pub mod selection_workflow;
pub mod window_controls;
pub mod window_geometry_events;
pub mod window_placement;
pub mod window_tiling;
