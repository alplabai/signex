//! Shader source module namespace.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

pub const LINE_WGSL: &str = include_str!("line.wgsl");
pub const CIRCLE_WGSL: &str = include_str!("circle.wgsl");
pub const ARC_WGSL: &str = include_str!("arc.wgsl");
pub const POLYGON_WGSL: &str = include_str!("polygon.wgsl");
pub const TEXT_WGSL: &str = include_str!("text.wgsl");
