//! Sketch-mode rendering — entity overlay, DOF arrows, snap glyph,
//! constraint icons, filled closed loops, and the live ghost preview
//! for multi-click drawing tools.
//!
//! Split by concern (the folder carries the namespace, so the children
//! keep no `sketch_`/`draw_` prefix):
//! - `constraints` — constraint-glyph overlay.
//! - `entities` — point / line / circle / arc entity overlay.
//! - `arrows` — DOF direction arrows.
//! - `snap` — inferred-constraint snap glyph.
//! - `fills` — filled closed loops plus the `ClosedLoop` records.
//! - `preview` — live ghost preview for multi-click drawing tools.
//!
//! The z-order the layer methods paint in is unchanged; each concern
//! keeps its original primitive-push sequence byte-for-byte.

mod arrows;
mod constraints;
mod entities;
mod fills;
mod pad_link;
mod preview;
mod snap;
mod tool_ghosts;

// Entry points reached from the sibling `draw::overlays` layer methods
// via `use super::sketch::{…}`. Re-exported at their original
// `pub(super)` reach so the `draw::sketch::<entry>` paths keep resolving
// unchanged.
pub(super) use arrows::draw_dof_direction_arrows;
pub(super) use entities::draw_sketch_overlay;
pub(super) use pad_link::draw_pad_link_profile;
pub(super) use preview::draw_sketch_tool_preview;
pub(super) use snap::draw_sketch_snap_glyph;
pub(super) use tool_ghosts::draw_sketch_tool_ghosts;

// The closed-loop walker + record are reached from `canvas::input`
// through the `draw::` re-export; keep them at their original
// canvas-wide reach.
pub(in crate::library::editor::footprint::canvas) use fills::{ClosedLoop, find_closed_loops};
