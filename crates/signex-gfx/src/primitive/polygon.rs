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

impl GpuPolygon {
    /// Whether this contour carries a stroked outline on top of its fill: it
    /// needs both a stroke colour and a positive width. Shared CPU↔GPU
    /// predicate — the GPU tessellator (`append_stroke`) and the parity test
    /// both use it.
    ///
    /// NOTE: the CPU `renderer_scene_canvas`/`pcb_canvas` polygon paths stroke
    /// whenever `stroke_color` is `Some`, even at zero width (clamped to a
    /// minimum). That laxer rule is a known CPU↔GPU divergence the parity test
    /// documents; this predicate is the GPU-side truth.
    pub fn is_stroked(&self) -> bool {
        self.stroke_color.is_some() && self.stroke_width > 0.0
    }
}
