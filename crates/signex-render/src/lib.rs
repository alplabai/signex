//! wgpu rendering primitives for Signex — schematic and PCB drawing.
//!
//! This crate provides the rendering logic that bridges `signex-types`
//! domain objects to Iced Canvas draw calls. No Iced Application logic here —
//! just pure rendering functions.

pub mod schematic;
pub mod colors;

/// The schematic canvas font. Loaded as a binary asset in `main.rs` and
/// available by name once the application starts.
pub const IOSEVKA: iced::Font = iced::Font::with_name("Iosevka");
