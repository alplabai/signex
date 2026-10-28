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
#[derive(Default, Debug, Clone)]
pub struct Scene {
    pub lines: Vec<LineSegment>,
    pub circles: Vec<Circle>,
    pub arcs: Vec<Arc>,
    pub polygons: Vec<GpuPolygon>,
    /// Polygons that composite ABOVE the stroke buckets (lines, arcs,
    /// circles) but below text and every overlay. Covered by
    /// [`DirtyFlags::POLYGONS`] and culled/uploaded with [`Self::polygons`].
    /// The PCB emitter puts pads here so a pad lands on top of the traces
    /// and vias that run into it; zone pours stay in `polygons`, under them.
    pub top_polygons: Vec<GpuPolygon>,
    pub texts: Vec<TextItem>,
    pub overlay_lines: Vec<LineSegment>,
    pub overlay_circles: Vec<Circle>,
    pub overlay_polygons: Vec<GpuPolygon>,
    pub erc_marker_lines: Vec<LineSegment>,
    pub erc_marker_circles: Vec<Circle>,
    pub erc_marker_polygons: Vec<GpuPolygon>,
    pub dirty: DirtyFlags,
}

impl Scene {
    pub fn clear(&mut self) {
        self.lines.clear();
        self.circles.clear();
        self.arcs.clear();
        self.polygons.clear();
        self.top_polygons.clear();
        self.texts.clear();
        self.overlay_lines.clear();
        self.overlay_circles.clear();
        self.overlay_polygons.clear();
        self.erc_marker_lines.clear();
        self.erc_marker_circles.clear();
        self.erc_marker_polygons.clear();
        self.dirty = DirtyFlags::ALL;
    }

    /// Which geometry categories differ from `prev`, as dirty flags.
    ///
    /// Compares primitive buckets by value, which is far cheaper than
    /// re-uploading them. Colours are part of each primitive, so a theme change
    /// shows up here as the buckets whose colours moved. Text is not compared
    /// (it is re-prepared every frame) and neither are the grid and theme-only
    /// flags, which carry no geometry.
    pub fn changed_geometry_since(&self, prev: &Scene) -> DirtyFlags {
        let mut changed = DirtyFlags::empty();
        changed.set(DirtyFlags::LINES, self.lines != prev.lines);
        changed.set(DirtyFlags::CIRCLES, self.circles != prev.circles);
        changed.set(DirtyFlags::ARCS, self.arcs != prev.arcs);
        changed.set(
            DirtyFlags::POLYGONS,
            self.polygons != prev.polygons || self.top_polygons != prev.top_polygons,
        );
        changed.set(
            DirtyFlags::OVERLAY,
            self.overlay_lines != prev.overlay_lines
                || self.overlay_circles != prev.overlay_circles
                || self.overlay_polygons != prev.overlay_polygons
                || self.erc_marker_lines != prev.erc_marker_lines
                || self.erc_marker_circles != prev.erc_marker_circles
                || self.erc_marker_polygons != prev.erc_marker_polygons,
        );
        changed
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
            && self.circles.is_empty()
            && self.arcs.is_empty()
            && self.polygons.is_empty()
            && self.top_polygons.is_empty()
            && self.texts.is_empty()
            && self.overlay_lines.is_empty()
            && self.overlay_circles.is_empty()
            && self.overlay_polygons.is_empty()
            && self.erc_marker_lines.is_empty()
            && self.erc_marker_circles.is_empty()
            && self.erc_marker_polygons.is_empty()
    }
}
