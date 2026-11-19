//! Properties panel for a single selected schematic element (HI-22 / MD-20).
//!
//! Extracted from `panels/mod.rs`. Pure view code — zero behaviour change.
//! Routes between Symbol / Label / TextNote / Drawing / ChildSheet
//! contexts; the per-shape Drawing surface and per-child-sheet style
//! editor live in sibling submodules split out of the former
//! single-file module.

mod body_row;
mod child_sheet;
mod directive;
mod drawing;
mod drawing_preview;
mod font_row;
mod footprint_card;
mod footprint_section;
mod header;
mod label;
mod library_rows;
mod no_erc;
mod pins;
mod placed_kinds;
mod placed_rows;
#[cfg(test)]
mod placed_rows_tests;
mod power_port;
mod rows;
mod selected;
mod symbol;
mod symbol_field;
mod text_note;

pub(crate) use body_row::{
    BODY_ALTERNATE, BODY_ALTERNATE_MISSING, BODY_NORMAL, BODY_NORMAL_CAN_SWITCH,
};
pub(super) use child_sheet::view_child_sheet_properties;
pub use directive::BlanketEdit;
// #1281: the Net Parameters and Net Blanket surfaces.
mod blanket;
#[cfg(test)]
mod net_directive_tests;
mod net_parameters;
pub use directive::{
    BlanketNetRow, DirectivePanel, DirectivePanelMsg, DraftField, NetParametersEdit, NetRule,
    OutlineField,
};
pub(super) use drawing::view_drawing_properties;
pub(super) use font_row::font_family_row;
pub use footprint_card::{
    FootprintCardFacts, FootprintCardKey, FootprintCardStatus, FootprintCardView, FootprintCards,
};
pub use no_erc::NoErcRuleEdit;
pub(super) use selected::view_selected_element_properties;
