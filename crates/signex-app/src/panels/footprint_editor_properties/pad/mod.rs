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
    PadEditTarget, PadFormValues, pad_check_row, pad_input_row, pad_pick_row,
    render_pad_form_pad_features, render_pad_form_pad_stack, render_pad_form_properties,
};
