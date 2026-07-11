//! Pads-mode Properties-panel surface: the pad form (types, message
//! helpers, row primitives, and the three render functions), the pad-
//! stack preview + Choice enums, and the pad-properties table cells.
//!
//! Folded from the former flat `pad_form` / `pad_stack_preview` /
//! `pad_table` siblings; `form` carries the cross-module surface the
//! parent panel and its sub-forms consume, re-exported below.

mod form;
mod stack_preview;
mod table;

pub(super) use form::{
    PadEditTarget, PadFormValues, pad_check_row, pad_copper_offset_x_msg, pad_copper_offset_y_msg,
    pad_corner_radius_msg, pad_designator_msg, pad_drill_diameter_msg, pad_drill_slot_length_msg,
    pad_electrical_type_msg, pad_feature_bottom_msg, pad_feature_top_msg, pad_hole_rotation_msg,
    pad_hole_tolerance_minus_msg, pad_hole_tolerance_plus_msg, pad_input_row, pad_locked_msg,
    pad_mask_bottom_msg, pad_mask_tented_bottom_msg, pad_mask_tented_top_msg, pad_mask_top_msg,
    pad_net_msg, pad_paste_bottom_msg, pad_paste_enabled_bottom_msg, pad_paste_enabled_top_msg,
    pad_paste_top_msg, pad_pick_row, pad_plated_msg, pad_rotation_msg, pad_shape_msg, pad_side_msg,
    pad_size_x_msg, pad_size_y_msg, pad_template_library_msg, pad_template_msg,
    pad_testpoint_bottom_assembly_msg, pad_testpoint_bottom_fab_msg,
    pad_testpoint_top_assembly_msg, pad_testpoint_top_fab_msg, pad_thermal_relief_msg,
    render_pad_form_pad_features, render_pad_form_pad_stack, render_pad_form_properties,
};
