//! Canvas draw layers — the `Program::draw` body decomposed by layer
//! into `impl FootprintCanvas` methods, plus the free-function
//! renderers those layer methods call. The trait `draw` (in the parent
//! `canvas` module) stays a thin sequence that calls the layer methods
//! in the ORIGINAL z-order (order is load-bearing). Behaviour is
//! byte-identical.
//!
//! Layer-method modules (`impl FootprintCanvas`):
//! - [`background`] — background fill + grid, guides, origin crosshair.
//! - [`scene`] — silk graphics, courtyard, pads, array badges.
//! - [`ghosts`] — PlacePad / PlaceVia placement ghosts.
//! - [`overlays`] — sketch reticle, select cursor mark, touching-line
//!   / lasso ghosts, rubber-band rectangle, sketch-entity overlay.
//!
//! Free-function renderers (called by the layer methods above and, for
//! a few items, by the parent `canvas` / `input` modules through the
//! re-exports below):
//! - [`grid`] — fine + coarse grid rendering.
//! - [`pad`] — pad copper / hole / number + Pads-mode tool preview.
//! - [`silk`] — silk-front + silk-back graphics renderer.
//! - [`sketch`] — sketch entity overlay, DOF arrows, snap glyph,
//!   constraint icons, filled closed loops, and the multi-click ghost
//!   preview for sketch drawing tools.

mod background;
mod ghosts;
mod grid;
mod overlays;
mod pad;
mod scene;
mod silk;
mod sketch;

// Items reached from outside `draw/`: the parent `canvas` module
// imports `draw_pads_tool_preview`, and the `canvas::input` submodule
// imports the closed-loop helpers. They were `pub(super)` on the flat
// siblings and widen to `pub(in …canvas)` now that they sit one level
// deeper, re-exported here so consumers name `draw::<item>` rather than
// reaching into the private renderer submodules.
pub(in crate::library::editor::footprint::canvas) use pad::draw_pads_tool_preview;
pub(in crate::library::editor::footprint::canvas) use sketch::{ClosedLoop, find_closed_loops};
