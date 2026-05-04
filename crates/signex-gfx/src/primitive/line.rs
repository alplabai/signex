//! Line primitive type.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

/// Straight line segment with width and style.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LineSegment {
    pub p0: [f32; 2],
    pub p1: [f32; 2],
    pub width: f32,
    pub color: [f32; 4],
    pub style: u32,
    pub _pad: u32,
}
