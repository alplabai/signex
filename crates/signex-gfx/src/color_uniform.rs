//! Theme palette uniform definitions.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

/// Schematic color palette uploaded as a single uniform block.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SchematicColorUniform {
    pub slots: [[f32; 4]; 32],
}

impl Default for SchematicColorUniform {
    fn default() -> Self {
        Self {
            slots: [[0.0, 0.0, 0.0, 1.0]; 32],
        }
    }
}
