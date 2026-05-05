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
        self.theme_refreshes == 1
            && self.grid_refreshes == 0
            && self.geometry_uploads() == 0
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
        let circles = cull_items(&scene.circles, viewport, |circle| Some(circle_envelope(circle)));
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
        let overlay_lines = cull_items(&scene.overlay_lines, viewport, |line| Some(line_envelope(line)));
        target.upload_overlay_lines(overlay_lines.as_ref());
        counters.overlay_line_uploads += 1;

        let overlay_circles =
            cull_items(&scene.overlay_circles, viewport, |circle| Some(circle_envelope(circle)));
        target.upload_overlay_circles(overlay_circles.as_ref());
        counters.overlay_circle_uploads += 1;

        let overlay_polygons = cull_items(&scene.overlay_polygons, viewport, polygon_envelope);
        target.upload_overlay_polygons(overlay_polygons.as_ref());
        counters.overlay_polygon_uploads += 1;

        let erc_lines = cull_items(&scene.erc_marker_lines, viewport, |line| Some(line_envelope(line)));
        target.upload_erc_marker_lines(erc_lines.as_ref());
        counters.erc_marker_line_uploads += 1;

        let erc_circles =
            cull_items(&scene.erc_marker_circles, viewport, |circle| Some(circle_envelope(circle)));
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
mod tests {
    use super::{
        apply_dirty_uploads, apply_dirty_uploads_with_culling, SceneUploadTarget,
        TextUploadParams, UploadCounters, UploadCulling, ViewportAabbMm,
    };
    use crate::primitive::arc::Arc;
    use crate::primitive::circle::Circle;
    use crate::primitive::line::LineSegment;
    use crate::primitive::polygon::GpuPolygon;
    use crate::primitive::text::{TextHAlign, TextItem, TextVAlign};
    use crate::scene::{DirtyFlags, Scene};

    #[derive(Default)]
    struct MockUploadTarget {
        calls: UploadCounters,
        text_params: Option<TextUploadParams>,
        fail_text_upload: bool,
        uploaded_lines: usize,
        uploaded_circles: usize,
        uploaded_arcs: usize,
        uploaded_polygons: usize,
        uploaded_texts: usize,
        uploaded_overlay_lines: usize,
        uploaded_overlay_circles: usize,
        uploaded_overlay_polygons: usize,
        uploaded_erc_lines: usize,
        uploaded_erc_circles: usize,
        uploaded_erc_polygons: usize,
    }

    impl MockUploadTarget {
        fn total_calls(&self) -> u32 {
            self.calls.total_updates()
        }
    }

    impl SceneUploadTarget for MockUploadTarget {
        type TextError = &'static str;

        fn upload_lines(&mut self, _lines: &[LineSegment]) {
            self.uploaded_lines = _lines.len();
            self.calls.line_uploads += 1;
        }

        fn upload_circles(&mut self, _circles: &[Circle]) {
            self.uploaded_circles = _circles.len();
            self.calls.circle_uploads += 1;
        }

        fn upload_arcs(&mut self, _arcs: &[Arc]) {
            self.uploaded_arcs = _arcs.len();
            self.calls.arc_uploads += 1;
        }

        fn upload_polygons(&mut self, _polygons: &[GpuPolygon]) {
            self.uploaded_polygons = _polygons.len();
            self.calls.polygon_uploads += 1;
        }

        fn upload_texts(
            &mut self,
            _texts: &[TextItem],
            params: TextUploadParams,
        ) -> Result<(), Self::TextError> {
            self.uploaded_texts = _texts.len();
            self.calls.text_uploads += 1;
            self.text_params = Some(params);

            if self.fail_text_upload {
                return Err("text upload failed");
            }

            Ok(())
        }

        fn refresh_grid(&mut self) {
            self.calls.grid_refreshes += 1;
        }

        fn upload_overlay_lines(&mut self, _lines: &[LineSegment]) {
            self.uploaded_overlay_lines = _lines.len();
            self.calls.overlay_line_uploads += 1;
        }

        fn upload_overlay_circles(&mut self, _circles: &[Circle]) {
            self.uploaded_overlay_circles = _circles.len();
            self.calls.overlay_circle_uploads += 1;
        }

        fn upload_overlay_polygons(&mut self, _polygons: &[GpuPolygon]) {
            self.uploaded_overlay_polygons = _polygons.len();
            self.calls.overlay_polygon_uploads += 1;
        }

        fn upload_erc_marker_lines(&mut self, _lines: &[LineSegment]) {
            self.uploaded_erc_lines = _lines.len();
            self.calls.erc_marker_line_uploads += 1;
        }

        fn upload_erc_marker_circles(&mut self, _circles: &[Circle]) {
            self.uploaded_erc_circles = _circles.len();
            self.calls.erc_marker_circle_uploads += 1;
        }

        fn upload_erc_marker_polygons(&mut self, _polygons: &[GpuPolygon]) {
            self.uploaded_erc_polygons = _polygons.len();
            self.calls.erc_marker_polygon_uploads += 1;
        }

        fn refresh_theme(&mut self) {
            self.calls.theme_refreshes += 1;
        }
    }

    fn fixture_scene() -> Scene {
        let mut scene = Scene::default();
        scene.lines.push(LineSegment {
            p0: [0.0, 0.0],
            p1: [1.0, 1.0],
            width: 0.2,
            color: [1.0, 1.0, 1.0, 1.0],
            style: 0,
            _pad: 0,
        });
        scene.circles.push(Circle {
            center: [2.0, 2.0],
            radius: 0.5,
            stroke_width: 0.1,
            color: [0.8, 0.4, 0.2, 1.0],
        });
        scene.arcs.push(Arc {
            center: [1.0, 2.0],
            radius: 0.75,
            start_angle: 0.0,
            end_angle: std::f32::consts::FRAC_PI_2,
            width: 0.1,
            color: [0.2, 0.6, 0.9, 1.0],
            _pad: [0.0; 3],
        });
        scene.polygons.push(GpuPolygon {
            vertices: vec![[0.0, 0.0], [1.0, 0.0], [0.5, 1.0]],
            fill_color: [0.2, 0.2, 0.8, 1.0],
            stroke_color: None,
            stroke_width: 0.0,
        });
        scene.texts.push(TextItem {
            content: "R1".to_string(),
            position: [1.0, 1.0],
            size_mm: 1.0,
            color: [1.0, 1.0, 1.0, 1.0],
            bold: false,
            italic: false,
            rotation: 0.0,
            h_align: TextHAlign::Left,
            v_align: TextVAlign::Top,
        });
        scene.overlay_lines.push(LineSegment {
            p0: [3.0, 3.0],
            p1: [4.0, 4.0],
            width: 0.15,
            color: [0.9, 0.9, 0.2, 1.0],
            style: 0,
            _pad: 0,
        });
        scene.overlay_circles.push(Circle {
            center: [4.0, 4.0],
            radius: 0.25,
            stroke_width: 0.08,
            color: [0.2, 0.9, 0.9, 1.0],
        });
        scene.overlay_polygons.push(GpuPolygon {
            vertices: vec![[2.0, 2.0], [3.0, 2.0], [2.5, 3.0]],
            fill_color: [0.4, 0.6, 1.0, 0.3],
            stroke_color: Some([0.4, 0.6, 1.0, 1.0]),
            stroke_width: 0.1,
        });
        scene.erc_marker_lines.push(LineSegment {
            p0: [5.0, 5.0],
            p1: [5.0, 5.5],
            width: 0.12,
            color: [0.95, 0.35, 0.35, 1.0],
            style: 0,
            _pad: 0,
        });
        scene.erc_marker_circles.push(Circle {
            center: [5.0, 5.0],
            radius: 0.3,
            stroke_width: 0.1,
            color: [0.95, 0.35, 0.35, 1.0],
        });
        scene.erc_marker_polygons.push(GpuPolygon {
            vertices: vec![[4.8, 4.7], [5.2, 4.7], [5.0, 5.2]],
            fill_color: [0.95, 0.35, 0.35, 0.3],
            stroke_color: Some([0.95, 0.35, 0.35, 1.0]),
            stroke_width: 0.1,
        });

        scene
    }

    fn fixture_scene_with_far_geometry() -> Scene {
        let mut scene = fixture_scene();

        scene.lines.push(LineSegment {
            p0: [120.0, 120.0],
            p1: [121.0, 121.0],
            width: 0.2,
            color: [1.0, 0.0, 0.0, 1.0],
            style: 0,
            _pad: 0,
        });
        scene.circles.push(Circle {
            center: [130.0, 130.0],
            radius: 0.5,
            stroke_width: 0.1,
            color: [1.0, 0.0, 0.0, 1.0],
        });
        scene.arcs.push(Arc {
            center: [140.0, 140.0],
            radius: 0.8,
            start_angle: 0.0,
            end_angle: 1.0,
            width: 0.1,
            color: [1.0, 0.0, 0.0, 1.0],
            _pad: [0.0; 3],
        });
        scene.polygons.push(GpuPolygon {
            vertices: vec![[150.0, 150.0], [151.0, 150.0], [150.5, 151.0]],
            fill_color: [1.0, 0.0, 0.0, 1.0],
            stroke_color: None,
            stroke_width: 0.0,
        });
        scene.texts.push(TextItem {
            content: "FAR_TEXT".to_string(),
            position: [160.0, 160.0],
            size_mm: 1.0,
            color: [1.0, 1.0, 1.0, 1.0],
            bold: false,
            italic: false,
            rotation: 0.0,
            h_align: TextHAlign::Left,
            v_align: TextVAlign::Top,
        });

        scene.overlay_lines.push(LineSegment {
            p0: [170.0, 170.0],
            p1: [171.0, 171.0],
            width: 0.2,
            color: [1.0, 0.0, 0.0, 1.0],
            style: 0,
            _pad: 0,
        });
        scene.overlay_circles.push(Circle {
            center: [180.0, 180.0],
            radius: 0.5,
            stroke_width: 0.1,
            color: [1.0, 0.0, 0.0, 1.0],
        });
        scene.overlay_polygons.push(GpuPolygon {
            vertices: vec![[190.0, 190.0], [191.0, 190.0], [190.5, 191.0]],
            fill_color: [1.0, 0.0, 0.0, 1.0],
            stroke_color: None,
            stroke_width: 0.0,
        });

        scene.erc_marker_lines.push(LineSegment {
            p0: [200.0, 200.0],
            p1: [201.0, 201.0],
            width: 0.2,
            color: [1.0, 0.0, 0.0, 1.0],
            style: 0,
            _pad: 0,
        });
        scene.erc_marker_circles.push(Circle {
            center: [210.0, 210.0],
            radius: 0.5,
            stroke_width: 0.1,
            color: [1.0, 0.0, 0.0, 1.0],
        });
        scene.erc_marker_polygons.push(GpuPolygon {
            vertices: vec![[220.0, 220.0], [221.0, 220.0], [220.5, 221.0]],
            fill_color: [1.0, 0.0, 0.0, 1.0],
            stroke_color: None,
            stroke_width: 0.0,
        });

        scene
    }

    #[test]
    fn noop_dirty_mask_skips_all_uploads() {
        let scene = fixture_scene();
        let mut target = MockUploadTarget::default();

        let counters = apply_dirty_uploads(
            &scene,
            DirtyFlags::empty(),
            &mut target,
            TextUploadParams::new(32.0, [128, 128]),
        )
        .expect("dirty gate");

        assert!(counters.is_idle());
        assert_eq!(target.total_calls(), 0);
    }

    #[test]
    fn dirty_mask_uploads_only_requested_groups() {
        let scene = fixture_scene();
        let mut target = MockUploadTarget::default();
        let text_params = TextUploadParams::new(24.0, [640, 480]);

        let counters = apply_dirty_uploads(
            &scene,
            DirtyFlags::LINES | DirtyFlags::TEXT,
            &mut target,
            text_params,
        )
        .expect("dirty gate");

        assert_eq!(counters.line_uploads, 1);
        assert_eq!(counters.text_uploads, 1);
        assert_eq!(counters.total_updates(), 2);
        assert_eq!(target.text_params, Some(text_params));
    }

    #[test]
    fn overlay_dirty_uploads_all_overlay_batches() {
        let scene = fixture_scene();
        let mut target = MockUploadTarget::default();

        let counters = apply_dirty_uploads(
            &scene,
            DirtyFlags::OVERLAY,
            &mut target,
            TextUploadParams::new(16.0, [320, 240]),
        )
        .expect("dirty gate");

        assert_eq!(counters.overlay_line_uploads, 1);
        assert_eq!(counters.overlay_circle_uploads, 1);
        assert_eq!(counters.overlay_polygon_uploads, 1);
        assert_eq!(counters.erc_marker_line_uploads, 1);
        assert_eq!(counters.erc_marker_circle_uploads, 1);
        assert_eq!(counters.erc_marker_polygon_uploads, 1);
        assert_eq!(counters.total_updates(), 6);
    }

    #[test]
    fn all_dirty_flags_trigger_each_category_once() {
        let scene = fixture_scene();
        let mut target = MockUploadTarget::default();

        let counters = apply_dirty_uploads(
            &scene,
            DirtyFlags::ALL,
            &mut target,
            TextUploadParams::new(32.0, [800, 600]),
        )
        .expect("dirty gate");

        assert_eq!(counters.line_uploads, 1);
        assert_eq!(counters.circle_uploads, 1);
        assert_eq!(counters.arc_uploads, 1);
        assert_eq!(counters.polygon_uploads, 1);
        assert_eq!(counters.text_uploads, 1);
        assert_eq!(counters.grid_refreshes, 1);
        assert_eq!(counters.overlay_line_uploads, 1);
        assert_eq!(counters.overlay_circle_uploads, 1);
        assert_eq!(counters.overlay_polygon_uploads, 1);
        assert_eq!(counters.erc_marker_line_uploads, 1);
        assert_eq!(counters.erc_marker_circle_uploads, 1);
        assert_eq!(counters.erc_marker_polygon_uploads, 1);
        assert_eq!(counters.theme_refreshes, 1);
        assert_eq!(counters.total_updates(), 13);
    }

    #[test]
    fn text_upload_error_is_returned() {
        let scene = fixture_scene();
        let mut target = MockUploadTarget {
            fail_text_upload: true,
            ..MockUploadTarget::default()
        };

        let result = apply_dirty_uploads(
            &scene,
            DirtyFlags::TEXT,
            &mut target,
            TextUploadParams::new(32.0, [128, 128]),
        );

        assert_eq!(result, Err("text upload failed"));
    }

    #[test]
    fn viewport_culling_filters_core_primitive_batches() {
        let scene = fixture_scene_with_far_geometry();
        let mut target = MockUploadTarget::default();

        let counters = apply_dirty_uploads_with_culling(
            &scene,
            DirtyFlags::LINES
                | DirtyFlags::CIRCLES
                | DirtyFlags::ARCS
                | DirtyFlags::POLYGONS
                | DirtyFlags::TEXT,
            &mut target,
            TextUploadParams::new(32.0, [800, 600]),
            UploadCulling::viewport(ViewportAabbMm::new([-1.0, -1.0], [8.0, 8.0])),
        )
        .expect("culling gate");

        assert_eq!(counters.total_updates(), 5);
        assert_eq!(target.uploaded_lines, 1);
        assert_eq!(target.uploaded_circles, 1);
        assert_eq!(target.uploaded_arcs, 1);
        assert_eq!(target.uploaded_polygons, 1);
        assert_eq!(target.uploaded_texts, 1);
    }

    #[test]
    fn viewport_culling_filters_overlay_and_erc_batches() {
        let scene = fixture_scene_with_far_geometry();
        let mut target = MockUploadTarget::default();

        let counters = apply_dirty_uploads_with_culling(
            &scene,
            DirtyFlags::OVERLAY,
            &mut target,
            TextUploadParams::new(32.0, [800, 600]),
            UploadCulling::viewport(ViewportAabbMm::new([-1.0, -1.0], [8.0, 8.0])),
        )
        .expect("culling gate");

        assert_eq!(counters.total_updates(), 6);
        assert_eq!(target.uploaded_overlay_lines, 1);
        assert_eq!(target.uploaded_overlay_circles, 1);
        assert_eq!(target.uploaded_overlay_polygons, 1);
        assert_eq!(target.uploaded_erc_lines, 1);
        assert_eq!(target.uploaded_erc_circles, 1);
        assert_eq!(target.uploaded_erc_polygons, 1);
    }

    #[test]
    fn disabled_culling_keeps_full_batches() {
        let scene = fixture_scene_with_far_geometry();
        let mut target = MockUploadTarget::default();

        let counters = apply_dirty_uploads_with_culling(
            &scene,
            DirtyFlags::LINES | DirtyFlags::TEXT,
            &mut target,
            TextUploadParams::new(32.0, [800, 600]),
            UploadCulling::disabled(),
        )
        .expect("culling gate");

        assert_eq!(counters.total_updates(), 2);
        assert_eq!(target.uploaded_lines, scene.lines.len());
        assert_eq!(target.uploaded_texts, scene.texts.len());
    }

    #[test]
    fn theme_dirty_refreshes_without_geometry_uploads() {
        let scene = fixture_scene_with_far_geometry();
        let mut target = MockUploadTarget::default();

        let counters = apply_dirty_uploads_with_culling(
            &scene,
            DirtyFlags::THEME,
            &mut target,
            TextUploadParams::new(32.0, [800, 600]),
            UploadCulling::viewport(ViewportAabbMm::new([-1.0, -1.0], [8.0, 8.0])),
        )
        .expect("theme dirty gate");

        assert!(counters.is_theme_only_refresh());
        assert_eq!(counters.total_updates(), 1);
        assert_eq!(target.total_calls(), 1);
        assert_eq!(target.uploaded_lines, 0);
        assert_eq!(target.uploaded_circles, 0);
        assert_eq!(target.uploaded_arcs, 0);
        assert_eq!(target.uploaded_polygons, 0);
        assert_eq!(target.uploaded_texts, 0);
    }

    #[test]
    fn theme_dirty_can_coexist_with_geometry_updates() {
        let scene = fixture_scene_with_far_geometry();
        let mut target = MockUploadTarget::default();

        let counters = apply_dirty_uploads_with_culling(
            &scene,
            DirtyFlags::THEME | DirtyFlags::LINES,
            &mut target,
            TextUploadParams::new(32.0, [800, 600]),
            UploadCulling::viewport(ViewportAabbMm::new([-1.0, -1.0], [8.0, 8.0])),
        )
        .expect("theme+lines dirty gate");

        assert_eq!(counters.theme_refreshes, 1);
        assert_eq!(counters.line_uploads, 1);
        assert_eq!(counters.total_updates(), 2);
        assert_eq!(target.uploaded_lines, 1);
    }
}