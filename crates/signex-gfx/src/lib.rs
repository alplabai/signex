//! GPU foundation crate for Signex renderer.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

// Re-export the exact `wgpu` this crate pins (aligned with iced's wgpu 27 in
// #169) so consumers can wire signex-gfx pipelines into iced's shader widget
// against one shared wgpu instance instead of adding a second dependency.
pub use wgpu;

pub mod camera;
pub mod color_uniform;
pub mod context;
pub mod debug_pass;
pub mod pipeline;
pub mod primitive;
pub mod scene;
pub mod shader;
pub mod style;
