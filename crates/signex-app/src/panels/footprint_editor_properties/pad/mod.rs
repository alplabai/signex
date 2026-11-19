//! Pads-mode Properties-panel surface (#125): the pad header, the property
//! illustration, the pad form sections, the Rule / Manual expansion row,
//! the shape and hole choices and the per-layer pad stack table.
//!
//! `form` carries the cross-module surface the parent panel and its
//! sub-forms consume, re-exported below.

mod choices;
mod field;
mod form;
mod header;
mod illustration;
mod rule_row;
mod table;

pub(super) use form::{PadEditTarget, view_pad_form};
#[cfg(test)]
pub(super) use header::selected_pad_heading;
pub(super) use header::view_pad_header;
pub(in crate::panels::footprint_editor_properties) use rule_row::rule_or_manual_row;
