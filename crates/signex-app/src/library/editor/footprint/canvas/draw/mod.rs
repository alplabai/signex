//! Canvas draw layers — the `Program::draw` body decomposed by layer
//! into `impl FootprintCanvas` methods. The trait `draw` (in the
//! parent `canvas` module) stays a thin sequence that calls these in
//! the ORIGINAL z-order (order is load-bearing). Behaviour is
//! byte-identical.
//!
//! - [`background`] — background fill + grid, guides, origin crosshair.
//! - [`scene`] — silk graphics, courtyard, pads, array badges.
//! - [`ghosts`] — PlacePad / PlaceVia placement ghosts.
//! - [`overlays`] — sketch reticle, select cursor mark, touching-line
//!   / lasso ghosts, rubber-band rectangle, sketch-entity overlay.

mod background;
mod ghosts;
mod overlays;
mod scene;
