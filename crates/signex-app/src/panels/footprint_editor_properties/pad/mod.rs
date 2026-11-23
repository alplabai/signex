//! Pads-mode Properties-panel surface (mockup row 125): the pad header, the
//! property illustration, the pad form sections, the Rule / Manual
//! expansion row, the shape and hole choices and the per-layer pad stack
//! table.
//!
//! `form` carries the cross-module surface the parent panel and its
//! sub-forms consume, re-exported below.

mod choices;
mod driven_by;
mod field;
mod form;
mod header;
mod illustration;
mod layer_titles;
mod rule_row;
mod table;

pub(super) use driven_by::view_driven_by;
pub(in crate::panels::footprint_editor_properties) use field::{
    Shown, number_input, with_refusals,
};
pub(super) use form::{PadEditTarget, view_pad_form};
#[cfg(test)]
pub(super) use header::selected_pad_heading;
pub(super) use header::view_pad_header;
pub(in crate::panels::footprint_editor_properties) use layer_titles::layer_title;
pub(in crate::panels::footprint_editor_properties) use rule_row::rule_or_manual_row;
