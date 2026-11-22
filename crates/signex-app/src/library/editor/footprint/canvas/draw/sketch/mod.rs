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
mod bake_overlay;
mod constraints;
mod entities;
mod fills;
mod pad_link;
mod pattern_instances;
mod preview;
mod snap;
mod tool_ghosts;
// Values on the drawing, pattern glyphs and the conflict badge (mockup row 187).
mod conflict_badge;
mod dimension_labels;
mod pattern_glyphs;
// The sketch palette on the board ground (mockup row 187, mockup row 188).
mod sketch_ink;

/// The colour of selected sketch geometry, and of what draws as selected
/// with it: the pad-link profile and the accent of a value on the drawing.
const SELECTION: iced::Color = iced::Color {
    r: 1.00,
    g: 0.45,
    b: 0.05,
    a: 1.00,
};

// Entry points reached from the sibling `draw::overlays` layer methods
// via `use super::sketch::{…}`. Re-exported at their original
// `pub(super)` reach so the `draw::sketch::<entry>` paths keep resolving
// unchanged.
pub(super) use arrows::draw_dof_direction_arrows;
pub(super) use dimension_labels::draw_sketch_values;
pub(super) use entities::draw_sketch_overlay;
pub(super) use pad_link::draw_pad_link_profile;
pub(super) use pattern_instances::draw_pattern_instances;
pub(super) use preview::draw_sketch_tool_preview;
pub(super) use snap::draw_sketch_snap_glyph;
pub(super) use tool_ghosts::draw_sketch_tool_ghosts;

// The closed-loop walker + record are reached from `canvas::input`
// through the `draw::` re-export; keep them at their original
// canvas-wide reach.
pub(in crate::library::editor::footprint::canvas) use fills::{ClosedLoop, find_closed_loops};

// The snap badge's words beside the glyph (Graphite mockup row 188).
pub(super) use snap::draw_sketch_snap_badge;
