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
    /// Whether this contour carries a stroked outline on top of its fill: a
    /// stroke colour is enough. A zero width is a hairline, raised to the
    /// surface's screen-space floor on both paths — the CPU replay's
    /// `stroke_px`, the GPU's `polygon.wgsl`.
    ///
    /// Until #645 this also required a positive width, so a coloured
    /// zero-width outline drew on the CPU and vanished on the GPU. The CPU
    /// rule won: a colour with no outline would be a contradiction.
    pub fn is_stroked(&self) -> bool {
        self.stroke_color.is_some()
    }
}
