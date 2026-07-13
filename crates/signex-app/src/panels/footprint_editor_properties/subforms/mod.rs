//! Role-specific sub-forms — Sketch-mode pad attributes, Pour,
//! Keepout, Cutout, Pattern (array). Each is rendered when the
//! Properties panel detects the matching role on the selected sketch
//! entity / pad and shows only the relevant fields.

mod cutout;
mod keepout;
mod pattern;
mod pour;
mod sketch_pad;

pub(super) use cutout::render_cutout_subform;
pub(super) use keepout::render_keepout_subform;
pub(super) use pattern::render_pattern_subform;
pub(super) use pour::render_pour_subform;
pub(super) use sketch_pad::render_sketch_pad_subform;
