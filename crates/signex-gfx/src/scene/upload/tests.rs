//! Tests for dirty-driven scene uploads + viewport culling.
use super::{
    SceneUploadTarget, TextUploadParams, UploadCounters, UploadCulling, ViewportAabbMm,
    apply_dirty_uploads, apply_dirty_uploads_with_culling,
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
