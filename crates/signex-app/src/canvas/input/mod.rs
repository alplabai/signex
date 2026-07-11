//! Input handling for the schematic canvas, split by concern:
//! camera (zoom/fit), pointer (buttons + motion), keys (keyboard).
//!
//! Each file adds `impl SchematicCanvas` methods that
//! `canvas::Program::update` dispatches to in the original event order.

mod camera;
mod keys;
mod pointer;
