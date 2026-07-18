//! Canvas draw layers — the `Program::draw` body decomposed by layer
//! into `impl SymbolCanvas` methods. The trait `draw` (in the parent
//! `canvas` module) stays a thin sequence that calls these in the
//! ORIGINAL z-order (bottom-to-top; the order is load-bearing).
//! Behaviour is byte-identical.
//!
//! Each layer recomputes the world→screen transform (`w2s`) from
//! `self.camera` — the same closure the pre-split god-function built
//! once; the arithmetic is unchanged.
//!
//! - [`background`] — background fill, adaptive grid, origin crosshair.
//! - [`scene`] — resize handles for the selected graphic(s).
//! - [`overlays`] — tool hint, rubber-band box selection, and the
//!   line / circle / arc multi-click placement previews.

mod background;
mod overlays;
mod scene;
