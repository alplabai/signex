//! Properties panel — General + Parameters tabs (HI-22 / MD-20).
//!
//! Extracted from `panels/mod.rs`. Pure view code, zero behaviour change.
//! Split into concern-sibling submodules:
//!
//! - `general`   — the General tab (General, Page Options, Net Classes,
//!   Custom Selection Filters) and the Properties button chrome.
//! - `page_options` — the Page Options section of the General tab.
//! - `doc_rows` — the General tab's row builders (label column, segmented
//!   choice, enable box, typed pairs).
//! - `document_options` — Document Options view state and messages: the
//!   Parameters filter, the zone drafts and the illustration hover.
//! - `grid_illustration` / `sheet_illustration` — the General tab's two
//!   line-art illustrations, built as data.
//! - `parameters_tab` — the Parameters tab: title-block fields, sheet
//!   variables and the built-in special strings, in groups.
//! - `form_rows` — generic form-field row builders shared by every
//!   Properties surface.
//! - `net_params` — net-attribute rows, the Parameters (Net) section
//!   chrome, and the justification-grid pickers.

mod doc_rows;
mod document_options;
mod form_rows;
mod general;
mod graphite_table;
mod grid_illustration;
mod net_params;
mod page_options;
mod parameters_tab;
mod sheet_illustration;
mod sheet_variables;
mod title_block;

#[cfg(test)]
pub(crate) use document_options::ParametersFilter;
pub use document_options::{DocumentOptionsMsg, DocumentOptionsUi};
#[cfg(test)]
pub(crate) use form_rows::check_state_color;
pub(crate) use form_rows::form_choice_row;
pub use form_rows::{
    PaperEdge, PaperEdgeDraft, canvas_font_popup, font_style_row, form_check_row,
    form_check_row_shortcut, form_edit_row, form_font_link_row, form_input_row,
    form_input_row_untipped, form_label, form_pick_row, section_hdr, thin_sep, typed_mm,
};
#[cfg(test)]
pub(crate) use general::seg_text;
pub(crate) use general::{chip_label, graphite_filter_chip};
pub use general::{props_tab_btn, seg_btn, view_properties_general};
pub use net_params::{
    empty_section_row, justification_grid, net_numeric_row, net_params_add_bar, net_params_header,
    net_params_tabs,
};
pub use parameters_tab::{BuiltinParameter, view_properties_parameters};
pub use sheet_variables::SheetVariableMsg;
pub use title_block::{
    TitleBlockDraft, TitleBlockField, TitleBlockSource, shown_title_block_value, title_block_source,
};
