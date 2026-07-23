//! Properties panel — General + Parameters tabs (HI-22 / MD-20).
//!
//! Extracted from `panels/mod.rs`. Pure view code, zero behaviour change.
//! Split into concern-sibling submodules:
//!
//! - `general`   — the panel views (Custom Selection Filters, General +
//!   Page Options, document-parameter table) and their button chrome.
//! - `form_rows` — generic form-field row builders shared by every
//!   Properties surface.
//! - `net_params` — net-attribute rows, the Parameters (Net) section
//!   chrome, and the justification-grid pickers.

mod form_rows;
mod general;
mod net_params;

pub use form_rows::{
    canvas_font_popup, font_style_row, form_check_row, form_check_row_shortcut, form_edit_row,
    form_font_link_row, form_grid_row, form_grid_size_row, form_input_row, form_int_edit_row,
    form_label, form_label_row, form_mm_edit_row, form_pick_row, section_hdr, thin_sep,
};
pub use general::{
    custom_filter_tab, param_table_row, preset_chip, props_tab_btn, seg_btn, tag_btn,
    view_custom_selection_filters_section, view_properties_general, view_properties_parameters,
};
pub use net_params::{
    empty_section_row, justification_grid, net_numeric_row, net_params_add_bar, net_params_header,
    net_params_tabs, preplacement_justification_grid,
};
