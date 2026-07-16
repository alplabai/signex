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

impl LineSegment {
    /// `style` bit that marks a dashed (rather than solid) segment.
    pub const STYLE_DASHED: u32 = 1;

    /// Whether this segment renders dashed. The low `style` bit selects the
    /// dash pattern; the rest is reserved. This is the shared predicate the CPU
    /// renderer honours and the GPU `line.wgsl` shader must match — the CPU↔GPU
    /// parity test locks both paths to it.
    pub fn is_dashed(&self) -> bool {
        (self.style & Self::STYLE_DASHED) == Self::STYLE_DASHED
    }
}
