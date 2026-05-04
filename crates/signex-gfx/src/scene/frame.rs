//! Scene container for frame primitives.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

use crate::primitive::arc::Arc;
use crate::primitive::circle::Circle;
use crate::primitive::line::LineSegment;
use crate::primitive::polygon::GpuPolygon;
use crate::primitive::text::TextItem;

use super::dirty::DirtyFlags;

/// Flat primitive collection consumed by GPU pipelines.
#[derive(Default)]
pub struct Scene {
    pub lines: Vec<LineSegment>,
    pub circles: Vec<Circle>,
    pub arcs: Vec<Arc>,
    pub polygons: Vec<GpuPolygon>,
    pub texts: Vec<TextItem>,
    pub dirty: DirtyFlags,
}

impl Scene {
    pub fn clear(&mut self) {
        self.lines.clear();
        self.circles.clear();
        self.arcs.clear();
        self.polygons.clear();
        self.texts.clear();
        self.dirty = DirtyFlags::ALL;
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
            && self.circles.is_empty()
            && self.arcs.is_empty()
            && self.polygons.is_empty()
            && self.texts.is_empty()
    }
}
