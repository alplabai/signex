//! Phase 6 regression and golden fixtures for renderer hardening.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

use serde::Deserialize;
use signex_gfx::debug_pass::{
    run_arc_smoke_pass, run_grid_overlay_text_composite_smoke_pass, run_grid_smoke_pass,
    run_line_circle_smoke_pass, run_polygon_smoke_pass, run_text_geometry_composite_smoke_pass,
    run_text_smoke_pass, CompositeStage,
};
use signex_gfx::primitive::arc::Arc;
use signex_gfx::primitive::circle::Circle;
use signex_gfx::primitive::line::LineSegment;
use signex_gfx::primitive::polygon::GpuPolygon;
use signex_gfx::primitive::text::{TextHAlign, TextItem, TextVAlign};
use signex_gfx::scene::{
    apply_dirty_uploads, apply_dirty_uploads_with_culling, DirtyFlags, Scene, SceneUploadTarget,
    TextUploadParams, UploadCounters, UploadCulling, ViewportAabbMm,
};

#[derive(Debug, Deserialize)]
struct GoldenBaseline {
    smoke: SmokeGolden,
    upload: UploadGolden,
}

#[derive(Debug, Deserialize)]
struct SmokeGolden {
    line_instances: u32,
    circle_instances: u32,
    arc_instances: u32,
    polygon_vertices: u32,
    text_instances: u32,
    grid_minor_lod_alpha: f32,
    grid_major_lod_alpha: f32,
    text_geometry_text_instances: u32,
    text_geometry_stage_order: Vec<GoldenStage>,
    overlay_geometry_vertices: u32,
    overlay_instances: u32,
    overlay_text_instances: u32,
    overlay_stage_order: Vec<GoldenStage>,
}

#[derive(Debug, Deserialize)]
struct UploadGolden {
    all_dirty_total_updates: u32,
    core_culling_total_updates: u32,
    core_visible_lines: usize,
    core_visible_circles: usize,
    core_visible_arcs: usize,
    core_visible_polygons: usize,
    core_visible_texts: usize,
    overlay_culling_total_updates: u32,
    overlay_visible_lines: usize,
    overlay_visible_circles: usize,
    overlay_visible_polygons: usize,
    overlay_visible_erc_lines: usize,
    overlay_visible_erc_circles: usize,
    overlay_visible_erc_polygons: usize,
    theme_only_total_updates: u32,
    theme_only_geometry_uploads: u32,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
enum GoldenStage {
    Grid,
    Geometry,
    Overlay,
    Text,
}

impl GoldenStage {
    fn as_runtime(self) -> CompositeStage {
        match self {
            Self::Grid => CompositeStage::Grid,
            Self::Geometry => CompositeStage::Geometry,
            Self::Overlay => CompositeStage::Overlay,
            Self::Text => CompositeStage::Text,
        }
    }
}

#[derive(Default)]
struct MockUploadTarget {
    calls: UploadCounters,
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

impl SceneUploadTarget for MockUploadTarget {
    type TextError = &'static str;

    fn upload_lines(&mut self, lines: &[LineSegment]) {
        self.uploaded_lines = lines.len();
        self.calls.line_uploads += 1;
    }

    fn upload_circles(&mut self, circles: &[Circle]) {
        self.uploaded_circles = circles.len();
        self.calls.circle_uploads += 1;
    }

    fn upload_arcs(&mut self, arcs: &[Arc]) {
        self.uploaded_arcs = arcs.len();
        self.calls.arc_uploads += 1;
    }

    fn upload_polygons(&mut self, polygons: &[GpuPolygon]) {
        self.uploaded_polygons = polygons.len();
        self.calls.polygon_uploads += 1;
    }

    fn upload_texts(
        &mut self,
        texts: &[TextItem],
        _params: TextUploadParams,
    ) -> Result<(), Self::TextError> {
        self.uploaded_texts = texts.len();
        self.calls.text_uploads += 1;
        Ok(())
    }

    fn refresh_grid(&mut self) {
        self.calls.grid_refreshes += 1;
    }

    fn upload_overlay_lines(&mut self, lines: &[LineSegment]) {
        self.uploaded_overlay_lines = lines.len();
        self.calls.overlay_line_uploads += 1;
    }

    fn upload_overlay_circles(&mut self, circles: &[Circle]) {
        self.uploaded_overlay_circles = circles.len();
        self.calls.overlay_circle_uploads += 1;
    }

    fn upload_overlay_polygons(&mut self, polygons: &[GpuPolygon]) {
        self.uploaded_overlay_polygons = polygons.len();
        self.calls.overlay_polygon_uploads += 1;
    }

    fn upload_erc_marker_lines(&mut self, lines: &[LineSegment]) {
        self.uploaded_erc_lines = lines.len();
        self.calls.erc_marker_line_uploads += 1;
    }

    fn upload_erc_marker_circles(&mut self, circles: &[Circle]) {
        self.uploaded_erc_circles = circles.len();
        self.calls.erc_marker_circle_uploads += 1;
    }

    fn upload_erc_marker_polygons(&mut self, polygons: &[GpuPolygon]) {
        self.uploaded_erc_polygons = polygons.len();
        self.calls.erc_marker_polygon_uploads += 1;
    }

    fn refresh_theme(&mut self) {
        self.calls.theme_refreshes += 1;
    }
}

fn load_golden_fixture() -> GoldenBaseline {
    serde_json::from_str(include_str!("golden/phase6_regression_golden.json"))
        .expect("valid phase 6 golden fixture")
}

fn expected_stage_order(stages: &[GoldenStage]) -> Vec<CompositeStage> {
    stages.iter().copied().map(GoldenStage::as_runtime).collect()
}

fn assert_float_eq(lhs: f32, rhs: f32, epsilon: f32) {
    assert!(
        (lhs - rhs).abs() <= epsilon,
        "float mismatch: left={lhs}, right={rhs}, epsilon={epsilon}"
    );
}

fn build_upload_fixture_scene() -> Scene {
    let mut scene = Scene::default();

    scene.lines.push(LineSegment {
        p0: [0.0, 0.0],
        p1: [1.0, 1.0],
        width: 0.2,
        color: [1.0, 1.0, 1.0, 1.0],
        style: 0,
        _pad: 0,
    });
    scene.lines.push(LineSegment {
        p0: [120.0, 120.0],
        p1: [121.0, 121.0],
        width: 0.2,
        color: [1.0, 0.0, 0.0, 1.0],
        style: 0,
        _pad: 0,
    });

    scene.circles.push(Circle {
        center: [2.0, 2.0],
        radius: 0.5,
        stroke_width: 0.1,
        color: [0.8, 0.4, 0.2, 1.0],
    });
    scene.circles.push(Circle {
        center: [130.0, 130.0],
        radius: 0.5,
        stroke_width: 0.1,
        color: [1.0, 0.0, 0.0, 1.0],
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
        vertices: vec![[0.0, 0.0], [1.0, 0.0], [0.5, 1.0]],
        fill_color: [0.2, 0.2, 0.8, 1.0],
        stroke_color: None,
        stroke_width: 0.0,
    });
    scene.polygons.push(GpuPolygon {
        vertices: vec![[150.0, 150.0], [151.0, 150.0], [150.5, 151.0]],
        fill_color: [1.0, 0.0, 0.0, 1.0],
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
        p0: [3.0, 3.0],
        p1: [4.0, 4.0],
        width: 0.15,
        color: [0.9, 0.9, 0.2, 1.0],
        style: 0,
        _pad: 0,
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
        center: [4.0, 4.0],
        radius: 0.25,
        stroke_width: 0.08,
        color: [0.2, 0.9, 0.9, 1.0],
    });
    scene.overlay_circles.push(Circle {
        center: [180.0, 180.0],
        radius: 0.5,
        stroke_width: 0.1,
        color: [1.0, 0.0, 0.0, 1.0],
    });

    scene.overlay_polygons.push(GpuPolygon {
        vertices: vec![[2.0, 2.0], [3.0, 2.0], [2.5, 3.0]],
        fill_color: [0.4, 0.6, 1.0, 0.3],
        stroke_color: Some([0.4, 0.6, 1.0, 1.0]),
        stroke_width: 0.1,
    });
    scene.overlay_polygons.push(GpuPolygon {
        vertices: vec![[190.0, 190.0], [191.0, 190.0], [190.5, 191.0]],
        fill_color: [1.0, 0.0, 0.0, 1.0],
        stroke_color: None,
        stroke_width: 0.0,
    });

    scene.erc_marker_lines.push(LineSegment {
        p0: [5.0, 5.0],
        p1: [5.0, 5.5],
        width: 0.12,
        color: [0.95, 0.35, 0.35, 1.0],
        style: 0,
        _pad: 0,
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
        center: [5.0, 5.0],
        radius: 0.3,
        stroke_width: 0.1,
        color: [0.95, 0.35, 0.35, 1.0],
    });
    scene.erc_marker_circles.push(Circle {
        center: [210.0, 210.0],
        radius: 0.5,
        stroke_width: 0.1,
        color: [1.0, 0.0, 0.0, 1.0],
    });

    scene.erc_marker_polygons.push(GpuPolygon {
        vertices: vec![[4.8, 4.7], [5.2, 4.7], [5.0, 5.2]],
        fill_color: [0.95, 0.35, 0.35, 0.3],
        stroke_color: Some([0.95, 0.35, 0.35, 1.0]),
        stroke_width: 0.1,
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
fn regression_golden_smoke_reports_match_fixture_baseline() {
    let golden = load_golden_fixture();

    let line_circle =
        pollster::block_on(run_line_circle_smoke_pass(32.0)).expect("line-circle smoke report");
    let arc_instances = pollster::block_on(run_arc_smoke_pass()).expect("arc smoke report");
    let polygon_vertices =
        pollster::block_on(run_polygon_smoke_pass()).expect("polygon smoke report");
    let text_instances = pollster::block_on(run_text_smoke_pass()).expect("text smoke report");
    let grid = pollster::block_on(run_grid_smoke_pass()).expect("grid smoke report");
    let text_geometry = pollster::block_on(run_text_geometry_composite_smoke_pass())
        .expect("text-geometry composite smoke report");
    let overlay = pollster::block_on(run_grid_overlay_text_composite_smoke_pass())
        .expect("grid-overlay-text composite smoke report");

    assert_eq!(line_circle.line_instances, golden.smoke.line_instances);
    assert_eq!(line_circle.circle_instances, golden.smoke.circle_instances);
    assert_eq!(arc_instances, golden.smoke.arc_instances);
    assert_eq!(polygon_vertices, golden.smoke.polygon_vertices);
    assert_eq!(text_instances, golden.smoke.text_instances);

    assert_float_eq(grid.minor_lod_alpha, golden.smoke.grid_minor_lod_alpha, 0.000001);
    assert_float_eq(grid.major_lod_alpha, golden.smoke.grid_major_lod_alpha, 0.000001);

    assert_eq!(text_geometry.polygon_vertices, golden.smoke.polygon_vertices);
    assert_eq!(
        text_geometry.text_instances,
        golden.smoke.text_geometry_text_instances
    );
    assert_eq!(
        text_geometry.stage_order,
        expected_stage_order(&golden.smoke.text_geometry_stage_order)
    );

    assert_eq!(overlay.geometry_vertices, golden.smoke.overlay_geometry_vertices);
    assert_eq!(overlay.overlay_instances, golden.smoke.overlay_instances);
    assert_eq!(overlay.text_instances, golden.smoke.overlay_text_instances);
    assert_eq!(
        overlay.stage_order,
        expected_stage_order(&golden.smoke.overlay_stage_order)
    );
}

#[test]
fn regression_golden_upload_gating_matches_fixture_baseline() {
    let golden = load_golden_fixture();
    let scene = build_upload_fixture_scene();

    let mut all_dirty_target = MockUploadTarget::default();
    let all_dirty_counters = apply_dirty_uploads(
        &scene,
        DirtyFlags::ALL,
        &mut all_dirty_target,
        TextUploadParams::new(32.0, [800, 600]),
    )
    .expect("all-dirty upload gating");

    assert_eq!(
        all_dirty_counters.total_updates(),
        golden.upload.all_dirty_total_updates
    );
    assert_eq!(
        all_dirty_target.calls.total_updates(),
        golden.upload.all_dirty_total_updates
    );

    let viewport = UploadCulling::viewport(ViewportAabbMm::new([-1.0, -1.0], [8.0, 8.0]));

    let mut core_target = MockUploadTarget::default();
    let core_counters = apply_dirty_uploads_with_culling(
        &scene,
        DirtyFlags::LINES
            | DirtyFlags::CIRCLES
            | DirtyFlags::ARCS
            | DirtyFlags::POLYGONS
            | DirtyFlags::TEXT,
        &mut core_target,
        TextUploadParams::new(32.0, [800, 600]),
        viewport,
    )
    .expect("core culling upload gating");

    assert_eq!(
        core_counters.total_updates(),
        golden.upload.core_culling_total_updates
    );
    assert_eq!(core_target.uploaded_lines, golden.upload.core_visible_lines);
    assert_eq!(core_target.uploaded_circles, golden.upload.core_visible_circles);
    assert_eq!(core_target.uploaded_arcs, golden.upload.core_visible_arcs);
    assert_eq!(core_target.uploaded_polygons, golden.upload.core_visible_polygons);
    assert_eq!(core_target.uploaded_texts, golden.upload.core_visible_texts);

    let mut overlay_target = MockUploadTarget::default();
    let overlay_counters = apply_dirty_uploads_with_culling(
        &scene,
        DirtyFlags::OVERLAY,
        &mut overlay_target,
        TextUploadParams::new(32.0, [800, 600]),
        viewport,
    )
    .expect("overlay culling upload gating");

    assert_eq!(
        overlay_counters.total_updates(),
        golden.upload.overlay_culling_total_updates
    );
    assert_eq!(
        overlay_target.uploaded_overlay_lines,
        golden.upload.overlay_visible_lines
    );
    assert_eq!(
        overlay_target.uploaded_overlay_circles,
        golden.upload.overlay_visible_circles
    );
    assert_eq!(
        overlay_target.uploaded_overlay_polygons,
        golden.upload.overlay_visible_polygons
    );
    assert_eq!(overlay_target.uploaded_erc_lines, golden.upload.overlay_visible_erc_lines);
    assert_eq!(
        overlay_target.uploaded_erc_circles,
        golden.upload.overlay_visible_erc_circles
    );
    assert_eq!(
        overlay_target.uploaded_erc_polygons,
        golden.upload.overlay_visible_erc_polygons
    );

    let mut theme_target = MockUploadTarget::default();
    let theme_counters = apply_dirty_uploads_with_culling(
        &scene,
        DirtyFlags::THEME,
        &mut theme_target,
        TextUploadParams::new(32.0, [800, 600]),
        viewport,
    )
    .expect("theme-only upload gating");

    assert_eq!(
        theme_counters.total_updates(),
        golden.upload.theme_only_total_updates
    );
    assert_eq!(
        theme_counters.geometry_uploads(),
        golden.upload.theme_only_geometry_uploads
    );
    assert!(theme_counters.is_theme_only_refresh());
}
