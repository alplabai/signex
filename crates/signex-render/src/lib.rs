//! wgpu rendering primitives for Signex — schematic and PCB drawing.
//!
//! This crate provides the rendering logic that bridges `signex-types`
//! domain objects to Iced Canvas draw calls. No Iced Application logic here —
//! just pure rendering functions.

pub mod schematic;
pub mod colors;
