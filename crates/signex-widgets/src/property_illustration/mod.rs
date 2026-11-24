//! Property illustration: a small line-art drawing for the Properties panel.
//!
//! The drawing sits on the panel ground (no background of its own). Strokes
//! and labels use the theme's ink tokens, objects get a light fill at low
//! alpha, and dimension callouts carry the names of the fields below. The
//! fields stay the only place to edit: there are no handles, no on-image
//! editing and no rotation arrow.
//!
//! An [`Illustration`] is plain data built in `view` from the selected
//! object: [`Part`]s made of [`Primitive`]s, each linked to the [`FieldId`]s
//! it shows. A part that needs a feature switch ([`Part::requires`]) is
//! dropped while [`Illustration::feature`] has that switch off.
//!
//! Hover is model state. Keep an [`IllustrationHover`] in the panel state,
//! route [`IllustrationMsg`] into it, emit
//! [`IllustrationMsg::FieldHovered`] / [`IllustrationMsg::FieldLeft`] from
//! the field rows, and ask [`IllustrationHover::is_field_highlighted`] when
//! drawing a row. Hovering a field highlights every part linked to it; the
//! drawing itself does not follow the pointer.

mod callout_layout;
mod draw;
mod field;
mod geometry;
mod hover;
mod illustration;
mod metrics;
mod palette;
mod primitive;
mod program;
mod style;

pub use field::FieldId;
pub use hover::{IllustrationHover, IllustrationMsg};
pub use illustration::{DEFAULT_HEIGHT_EM, Illustration, Part};
pub use metrics::Metrics;
pub use palette::Palette;
pub use primitive::{Anchor, Callout, Cap, Leader, Primitive, Text, TextLabel, TextRole};
pub use program::property_illustration;
pub use style::{Fill, Ink, Paint, Stroke, Style, Tint, Weight};

#[cfg(test)]
mod tests;
