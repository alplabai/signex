//! Polygon primitive type.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

/// Polygon primitive for filled or stroked schematic shapes.
#[derive(Clone, Debug, Default)]
pub struct GpuPolygon {
    pub vertices: Vec<[f32; 2]>,
    pub fill_color: [f32; 4],
    pub stroke_color: Option<[f32; 4]>,
    pub stroke_width: f32,
}
