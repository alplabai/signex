//! Properties panel for a single selected schematic element (HI-22 / MD-20).
//!
//! Extracted from `panels/mod.rs`. Pure view code — zero behaviour change.
//! Routes between Symbol / Label / TextNote / Drawing / ChildSheet
//! contexts; the per-shape Drawing surface and per-child-sheet style
//! editor live in sibling submodules split out of the former
//! single-file module.

mod annotation;
mod bezier;
mod body_row;
mod channel_naming_menu;
mod channel_variant;
#[cfg(test)]
mod channel_variant_tests;
mod channel_view;
mod child_sheet;
mod child_sheet_repeat;
mod dimension;
mod directive;
mod drawing;
pub(in crate::panels) mod font_row;
mod footprint_card;
mod footprint_section;
mod harness_connector;
mod harness_definitions;
mod harness_wire;
pub(in crate::panels) mod header;
mod illustrations;
mod label;
mod library_rows;
#[cfg(test)]
mod narrow_dock_tests;
mod no_erc;
mod part_panel;
mod pins;
mod placed_kinds;
mod placed_rows;
#[cfg(test)]
mod placed_rows_tests;
mod power_port;
pub(in crate::panels) mod rows;
mod selected;
mod symbol;
mod symbol_field;
mod table;
mod text_note;

pub(super) use annotation::view_dimension_tool_properties;
pub use annotation::{
    AnnotationField, AnnotationPanelMsg, AnnotationPanelState, DIMENSION_OVERRIDE_INPUT,
    DIMENSION_TEXT_SECTION, DimensionToolForm,
};
pub(crate) use body_row::{
    BODY_ALTERNATE, BODY_ALTERNATE_MISSING, BODY_NORMAL, BODY_NORMAL_CAN_SWITCH,
};
pub(super) use channel_view::channel_view_properties;
pub(crate) use child_sheet::sheet_file_missing;
pub(super) use child_sheet::view_child_sheet_properties;
pub use directive::BlanketEdit;
// #1281, #1489: the Net Parameters and Net Blanket surfaces.
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
pub use harness_connector::{HarnessField, HarnessMismatchRow, HarnessPanel, HarnessPanelMsg};
pub(super) use harness_definitions::view_harness_definitions;
pub use illustrations::footprint_thumb::FootprintThumb;
pub use no_erc::NoErcRuleEdit;
pub use part_panel::{PartPanelEffect, PartPanelMsg, PartPanelState};
pub(super) use selected::view_selected_element_properties;
