//! Footprint editor module — submodule re-exports only.
//!
//! Footprint editing happens via the standalone `.snxfpt` document
//! editor in [`crate::library::editor::standalone`], which re-uses
//! [`canvas`] / [`layers`] / [`state`] verbatim.
//!
//! `body3d`, `preview3d`, and `step_attach` remain on disk for the
//! eventual standalone Body 3D / STEP attach editor side-pane.

pub mod active_bar_dropdowns;
pub mod align_modal;
pub mod body3d;
pub mod body3d_mint;
pub mod canvas;
pub mod context_menu;
pub mod filter_presets;
pub mod layers;
pub mod layers_panel;
pub(crate) mod legacy_silk;
pub mod move_by_modal;
pub mod pad_stack_check;
pub mod pad_to_sketch;
pub mod pads_active_bar;
pub mod preview3d;
#[cfg(test)]
mod silk_ownership_tests;
mod sketch_bake;
mod sketch_bake_fields;
#[cfg(test)]
mod sketch_bake_tests;
mod sketch_conflict;
#[cfg(test)]
mod sketch_conflict_tests;
pub mod sketch_dispatch;
#[cfg(test)]
mod sketch_dispatch_tests;
pub mod sketch_mode;
pub mod sketch_redundancy_task;
#[cfg(test)]
mod sketch_redundancy_task_tests;
pub mod sketch_solve_task;
#[cfg(test)]
mod sketch_solve_task_tests;
pub mod snap;
pub mod state;
pub mod step_attach;
#[cfg(test)]
mod swallowed_input_tests;
pub mod text_frame;
pub mod unified_active_bar;
pub(crate) mod updates;

// The footprint preview the rest of the app draws (component details and
// the Properties footprint card), named here so callers outside the slice do
// not reach into `canvas`.
pub use canvas::PreviewFootprint;
pub(crate) use canvas::graphite_look_for;

#[cfg(test)]
mod tests;
