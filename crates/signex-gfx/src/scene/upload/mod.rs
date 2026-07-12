//! Dirty-flag driven scene upload gating.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

use crate::primitive::arc::Arc;
use crate::primitive::circle::Circle;
use crate::primitive::line::LineSegment;
use crate::primitive::polygon::GpuPolygon;
use crate::primitive::text::TextItem;
use rstar::{AABB, RTree, RTreeObject};
use std::borrow::Cow;

use super::{DirtyFlags, Scene};

/// Runtime text upload context required by text pipelines.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextUploadParams {
    pub scale_px_per_mm: f32,
    pub viewport_size_px: [u32; 2],
}

impl TextUploadParams {
    pub fn new(scale_px_per_mm: f32, viewport_size_px: [u32; 2]) -> Self {
        Self {
            scale_px_per_mm,
            viewport_size_px,
        }
    }
}

impl Default for TextUploadParams {
    fn default() -> Self {
        Self {
            scale_px_per_mm: 1.0,
            viewport_size_px: [1, 1],
        }
    }
}

/// Viewport bounds in millimeters used for primitive culling.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewportAabbMm {
    pub min: [f32; 2],
    pub max: [f32; 2],
}

impl ViewportAabbMm {
    pub fn new(min: [f32; 2], max: [f32; 2]) -> Self {
        Self {
            min: [min[0].min(max[0]), min[1].min(max[1])],
            max: [min[0].max(max[0]), min[1].max(max[1])],
        }
    }

    fn envelope(self) -> AABB<[f32; 2]> {
        AABB::from_corners(self.min, self.max)
    }
}

/// Culling settings for dirty upload application.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UploadCulling {
    pub viewport_mm: Option<ViewportAabbMm>,
}

impl UploadCulling {
    pub fn disabled() -> Self {
        Self { viewport_mm: None }
    }

    pub fn viewport(viewport_mm: ViewportAabbMm) -> Self {
        Self {
            viewport_mm: Some(viewport_mm),
        }
    }

    fn envelope(self) -> Option<AABB<[f32; 2]>> {
        self.viewport_mm.map(ViewportAabbMm::envelope)
    }
}

impl Default for UploadCulling {
    fn default() -> Self {
        Self::disabled()
    }
}

#[derive(Clone, Copy, Debug)]
struct IndexedEnvelope {
    index: usize,
    envelope: AABB<[f32; 2]>,
}

impl RTreeObject for IndexedEnvelope {
    type Envelope = AABB<[f32; 2]>;

    fn envelope(&self) -> Self::Envelope {
        self.envelope
    }
}

fn line_envelope(line: &LineSegment) -> AABB<[f32; 2]> {
    let half_width = (line.width * 0.5).max(0.0);
    let min_x = line.p0[0].min(line.p1[0]) - half_width;
    let max_x = line.p0[0].max(line.p1[0]) + half_width;
    let min_y = line.p0[1].min(line.p1[1]) - half_width;
    let max_y = line.p0[1].max(line.p1[1]) + half_width;

    AABB::from_corners([min_x, min_y], [max_x, max_y])
}

fn circle_envelope(circle: &Circle) -> AABB<[f32; 2]> {
    let extent = (circle.radius + circle.stroke_width * 0.5).max(0.0);
    AABB::from_corners(
        [circle.center[0] - extent, circle.center[1] - extent],
        [circle.center[0] + extent, circle.center[1] + extent],
    )
}

fn arc_envelope(arc: &Arc) -> AABB<[f32; 2]> {
    let extent = (arc.radius + arc.width * 0.5).max(0.0);
    AABB::from_corners(
        [arc.center[0] - extent, arc.center[1] - extent],
        [arc.center[0] + extent, arc.center[1] + extent],
    )
}

fn polygon_envelope(polygon: &GpuPolygon) -> Option<AABB<[f32; 2]>> {
    let first = *polygon.vertices.first()?;
    let mut min_x = first[0];
    let mut max_x = first[0];
    let mut min_y = first[1];
    let mut max_y = first[1];

    for point in &polygon.vertices {
        min_x = min_x.min(point[0]);
        max_x = max_x.max(point[0]);
        min_y = min_y.min(point[1]);
        max_y = max_y.max(point[1]);
    }

    let half_stroke = (polygon.stroke_width * 0.5).max(0.0);
    Some(AABB::from_corners(
        [min_x - half_stroke, min_y - half_stroke],
        [max_x + half_stroke, max_y + half_stroke],
    ))
}

fn text_envelope(text: &TextItem) -> AABB<[f32; 2]> {
    let safe_size = text.size_mm.max(0.01);
    let char_count = text.content.chars().count().max(1) as f32;
    let width = (safe_size * char_count * 0.7).max(safe_size);
    let height = (safe_size * 1.5).max(safe_size);
    let half_diag = ((width * width + height * height).sqrt() * 0.5).max(safe_size * 0.5);

    AABB::from_corners(
        [text.position[0] - half_diag, text.position[1] - half_diag],
        [text.position[0] + half_diag, text.position[1] + half_diag],
    )
}

fn cull_items<'a, T: Clone>(
    items: &'a [T],
    viewport: Option<&AABB<[f32; 2]>>,
    envelope_of: impl Fn(&T) -> Option<AABB<[f32; 2]>>,
) -> Cow<'a, [T]> {
    let Some(viewport) = viewport else {
        return Cow::Borrowed(items);
    };

    if items.is_empty() {
        return Cow::Borrowed(items);
    }

    let mut indexed = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        if let Some(envelope) = envelope_of(item) {
            indexed.push(IndexedEnvelope { index, envelope });
        }
    }

    if indexed.is_empty() {
        return Cow::Owned(Vec::new());
    }

    let tree = RTree::bulk_load(indexed);
    let mut visible_indices: Vec<usize> = tree
        .locate_in_envelope_intersecting(viewport)
        .map(|entry| entry.index)
        .collect();

    visible_indices.sort_unstable();
    visible_indices.dedup();

    let mut visible_items = Vec::with_capacity(visible_indices.len());
    for index in visible_indices {
        visible_items.push(items[index].clone());
    }

    Cow::Owned(visible_items)
}

/// Per-category upload/update counters for instrumentation and tests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UploadCounters {
    pub line_uploads: u32,
    pub circle_uploads: u32,
    pub arc_uploads: u32,
    pub polygon_uploads: u32,
    pub text_uploads: u32,
    pub grid_refreshes: u32,
    pub overlay_line_uploads: u32,
    pub overlay_circle_uploads: u32,
    pub overlay_polygon_uploads: u32,
    pub erc_marker_line_uploads: u32,
    pub erc_marker_circle_uploads: u32,
    pub erc_marker_polygon_uploads: u32,
    pub theme_refreshes: u32,
}

impl UploadCounters {
    pub fn total_updates(&self) -> u32 {
        self.line_uploads
            + self.circle_uploads
            + self.arc_uploads
            + self.polygon_uploads
            + self.text_uploads
            + self.grid_refreshes
            + self.overlay_line_uploads
            + self.overlay_circle_uploads
            + self.overlay_polygon_uploads
            + self.erc_marker_line_uploads
            + self.erc_marker_circle_uploads
            + self.erc_marker_polygon_uploads
            + self.theme_refreshes
    }

    pub fn geometry_uploads(&self) -> u32 {
        self.line_uploads
            + self.circle_uploads
            + self.arc_uploads
            + self.polygon_uploads
            + self.text_uploads
            + self.overlay_line_uploads
            + self.overlay_circle_uploads
            + self.overlay_polygon_uploads
            + self.erc_marker_line_uploads
            + self.erc_marker_circle_uploads
            + self.erc_marker_polygon_uploads
    }

    pub fn is_theme_only_refresh(&self) -> bool {
        self.theme_refreshes == 1 && self.grid_refreshes == 0 && self.geometry_uploads() == 0
    }

    pub fn is_idle(&self) -> bool {
        self.total_updates() == 0
    }
}

/// Upload target abstraction used to apply dirty-gated scene uploads.
pub trait SceneUploadTarget {
    type TextError;

    fn upload_lines(&mut self, lines: &[LineSegment]);
    fn upload_circles(&mut self, circles: &[Circle]);
    fn upload_arcs(&mut self, arcs: &[Arc]);
    fn upload_polygons(&mut self, polygons: &[GpuPolygon]);
    fn upload_texts(
        &mut self,
        texts: &[TextItem],
        params: TextUploadParams,
    ) -> Result<(), Self::TextError>;
    fn refresh_grid(&mut self);
    fn upload_overlay_lines(&mut self, lines: &[LineSegment]);
    fn upload_overlay_circles(&mut self, circles: &[Circle]);
    fn upload_overlay_polygons(&mut self, polygons: &[GpuPolygon]);
    fn upload_erc_marker_lines(&mut self, lines: &[LineSegment]);
    fn upload_erc_marker_circles(&mut self, circles: &[Circle]);
    fn upload_erc_marker_polygons(&mut self, polygons: &[GpuPolygon]);
    fn refresh_theme(&mut self);
}

/// Apply dirty-flag gated uploads and return per-category update counters.
pub fn apply_dirty_uploads<T: SceneUploadTarget>(
    scene: &Scene,
    dirty: DirtyFlags,
    target: &mut T,
    text_params: TextUploadParams,
) -> Result<UploadCounters, T::TextError> {
    apply_dirty_uploads_with_culling(scene, dirty, target, text_params, UploadCulling::disabled())
}

/// Apply dirty-gated uploads with optional viewport culling.
pub fn apply_dirty_uploads_with_culling<T: SceneUploadTarget>(
    scene: &Scene,
    dirty: DirtyFlags,
    target: &mut T,
    text_params: TextUploadParams,
    culling: UploadCulling,
) -> Result<UploadCounters, T::TextError> {
    let culling_envelope = culling.envelope();
    let viewport = culling_envelope.as_ref();
    let mut counters = UploadCounters::default();

    if dirty.contains(DirtyFlags::LINES) {
        let lines = cull_items(&scene.lines, viewport, |line| Some(line_envelope(line)));
        target.upload_lines(lines.as_ref());
        counters.line_uploads += 1;
    }

    if dirty.contains(DirtyFlags::CIRCLES) {
        let circles = cull_items(&scene.circles, viewport, |circle| {
            Some(circle_envelope(circle))
        });
        target.upload_circles(circles.as_ref());
        counters.circle_uploads += 1;
    }

    if dirty.contains(DirtyFlags::ARCS) {
        let arcs = cull_items(&scene.arcs, viewport, |arc| Some(arc_envelope(arc)));
        target.upload_arcs(arcs.as_ref());
        counters.arc_uploads += 1;
    }

    if dirty.contains(DirtyFlags::POLYGONS) {
        let polygons = cull_items(&scene.polygons, viewport, polygon_envelope);
        target.upload_polygons(polygons.as_ref());
        counters.polygon_uploads += 1;
    }

    if dirty.contains(DirtyFlags::TEXT) {
        let texts = cull_items(&scene.texts, viewport, |text| Some(text_envelope(text)));
        target.upload_texts(texts.as_ref(), text_params)?;
        counters.text_uploads += 1;
    }

    if dirty.contains(DirtyFlags::GRID) {
        target.refresh_grid();
        counters.grid_refreshes += 1;
    }

    if dirty.contains(DirtyFlags::OVERLAY) {
        let overlay_lines = cull_items(&scene.overlay_lines, viewport, |line| {
            Some(line_envelope(line))
        });
        target.upload_overlay_lines(overlay_lines.as_ref());
        counters.overlay_line_uploads += 1;

        let overlay_circles = cull_items(&scene.overlay_circles, viewport, |circle| {
            Some(circle_envelope(circle))
        });
        target.upload_overlay_circles(overlay_circles.as_ref());
        counters.overlay_circle_uploads += 1;

        let overlay_polygons = cull_items(&scene.overlay_polygons, viewport, polygon_envelope);
        target.upload_overlay_polygons(overlay_polygons.as_ref());
        counters.overlay_polygon_uploads += 1;

        let erc_lines = cull_items(&scene.erc_marker_lines, viewport, |line| {
            Some(line_envelope(line))
        });
        target.upload_erc_marker_lines(erc_lines.as_ref());
        counters.erc_marker_line_uploads += 1;

        let erc_circles = cull_items(&scene.erc_marker_circles, viewport, |circle| {
            Some(circle_envelope(circle))
        });
        target.upload_erc_marker_circles(erc_circles.as_ref());
        counters.erc_marker_circle_uploads += 1;

        let erc_polygons = cull_items(&scene.erc_marker_polygons, viewport, polygon_envelope);
        target.upload_erc_marker_polygons(erc_polygons.as_ref());
        counters.erc_marker_polygon_uploads += 1;
    }

    if dirty.contains(DirtyFlags::THEME) {
        target.refresh_theme();
        counters.theme_refreshes += 1;
    }

    Ok(counters)
}

#[cfg(test)]
mod tests;
