//! Canvas input handling — the `Program::update` body decomposed by
//! concern into `impl FootprintCanvas` methods. The trait `update`
//! (in the parent `canvas` module) stays a thin dispatcher that calls
//! these in the original order; behaviour is byte-identical.
//!
//! - [`camera`] — first-draw fit, Fit-to-Window, wheel zoom, pan.
//! - [`pointer`] — button-press / release / cursor-move dispatchers +
//!   the shared classification helpers (snap, drag ticks, hover tail).
//! - [`tools`] — per-tool left-press gesture arms.
//! - [`release`] — left-release commit arms (split from `tools` for
//!   the file-size cap).
//! - [`keys`] — modifier tracking + keyboard handling.

mod camera;
mod keys;
mod pointer;
mod release;
mod tools;
