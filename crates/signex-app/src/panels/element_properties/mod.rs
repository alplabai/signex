//! Properties panel for a single selected schematic element (HI-22 / MD-20).
//!
//! Extracted from `panels/mod.rs`. Pure view code — zero behaviour change.
//! Routes between Symbol / Label / TextNote / Drawing / ChildSheet
//! contexts; the per-shape Drawing surface and per-child-sheet style
//! editor live in sibling submodules split out of the former
//! single-file module.

mod child_sheet;
mod drawing;
mod drawing_preview;
mod selected;

pub(super) use child_sheet::view_child_sheet_properties;
pub(super) use drawing::view_drawing_properties;
pub(super) use selected::view_selected_element_properties;
