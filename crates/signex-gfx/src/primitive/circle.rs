//! Circle primitive type.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

/// Circle primitive supporting filled and stroked modes.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Circle {
    pub center: [f32; 2],
    pub radius: f32,
    pub stroke_width: f32,
    pub color: [f32; 4],
}
