//! Tests for GPU smoke passes.
use super::composite::run_grid_overlay_text_composite_smoke_pass_with;
use super::{
    CompositeStage, run_arc_smoke_pass, run_arc_smoke_pass_with,
    run_grid_overlay_text_composite_smoke_pass, run_grid_smoke_pass, run_grid_smoke_pass_with,
    run_line_circle_smoke_pass, run_polygon_smoke_pass, run_polygon_smoke_pass_with,
    run_text_geometry_composite_smoke_pass, run_text_smoke_pass, run_text_smoke_pass_with,
};
use crate::primitive::arc::Arc;
use crate::primitive::line::LineSegment;
use crate::primitive::polygon::GpuPolygon;
use crate::primitive::text::{TextHAlign, TextItem, TextVAlign};

#[test]
fn line_circle_smoke_pass_runs_for_multiple_scales() {
    let low_zoom = pollster::block_on(run_line_circle_smoke_pass(8.0)).expect("low zoom pass");
    let high_zoom = pollster::block_on(run_line_circle_smoke_pass(64.0)).expect("high zoom pass");

    assert_eq!(low_zoom.line_instances, 2);
    assert_eq!(low_zoom.circle_instances, 1);
    assert_eq!(high_zoom.line_instances, 2);
    assert_eq!(high_zoom.circle_instances, 1);
}

#[test]
fn arc_smoke_pass_runs() {
    let count = pollster::block_on(run_arc_smoke_pass()).expect("arc pass");
    assert_eq!(count, 1);
}

#[test]
fn arc_smoke_pass_handles_wraparound_sweep() {
    let wrap_start = 2.0 * std::f32::consts::PI - std::f32::consts::FRAC_PI_6;
    let wrap_end = std::f32::consts::FRAC_PI_6;

    let arcs = [Arc {
        center: [4.0, 4.0],
        radius: 2.0,
        start_angle: wrap_start,
        end_angle: wrap_end,
        width: 0.2,
        color: [1.0, 1.0, 1.0, 1.0],
        _pad: [0.0; 3],
    }];

    let count =
        pollster::block_on(run_arc_smoke_pass_with(8.0, &arcs)).expect("wraparound arc pass");
    assert_eq!(count, 1);
}

#[test]
fn arc_smoke_pass_handles_tiny_radius() {
    let arcs = [Arc {
        center: [4.0, 4.0],
        radius: 0.01,
        start_angle: 0.0,
        end_angle: 1.5707964,
        width: 0.005,
        color: [1.0, 1.0, 1.0, 1.0],
        _pad: [0.0; 3],
    }];

    let count =
        pollster::block_on(run_arc_smoke_pass_with(64.0, &arcs)).expect("tiny radius arc pass");
    assert_eq!(count, 1);
}

#[test]
fn polygon_smoke_pass_runs() {
    let vertex_count = pollster::block_on(run_polygon_smoke_pass()).expect("polygon pass");
    assert_eq!(vertex_count, 6);
}

#[test]
fn polygon_smoke_pass_handles_low_and_high_zoom() {
    let polygons = [GpuPolygon {
        vertices: vec![[2.0, 2.0], [8.0, 2.0], [8.0, 8.0], [2.0, 8.0]],
        fill_color: [0.2, 0.7, 0.9, 1.0],
        stroke_color: None,
        stroke_width: 0.0,
    }];

    let low_zoom_count =
        pollster::block_on(run_polygon_smoke_pass_with(8.0, &polygons)).expect("polygon low");
    let high_zoom_count =
        pollster::block_on(run_polygon_smoke_pass_with(64.0, &polygons)).expect("polygon high");

    assert_eq!(low_zoom_count, 6);
    assert_eq!(high_zoom_count, 6);
}

#[test]
fn polygon_smoke_pass_ignores_degenerate_geometry() {
    let polygons = [
        GpuPolygon {
            vertices: vec![[0.0, 0.0], [1.0, 0.0]],
            fill_color: [1.0, 0.0, 0.0, 1.0],
            stroke_color: None,
            stroke_width: 0.0,
        },
        GpuPolygon {
            vertices: vec![[2.0, 2.0], [8.0, 2.0], [8.0, 8.0], [2.0, 8.0]],
            fill_color: [0.2, 0.7, 0.9, 1.0],
            stroke_color: None,
            stroke_width: 0.0,
        },
    ];

    let count = pollster::block_on(run_polygon_smoke_pass_with(32.0, &polygons))
        .expect("polygon degenerate filter");
    assert_eq!(count, 6);
}

#[test]
fn grid_smoke_pass_runs() {
    let report = pollster::block_on(run_grid_smoke_pass()).expect("grid pass");
    assert!((0.0..=1.0).contains(&report.minor_lod_alpha));
    assert!((0.0..=1.0).contains(&report.major_lod_alpha));
}

#[test]
fn grid_smoke_pass_lod_changes_with_zoom() {
    let low_zoom = pollster::block_on(run_grid_smoke_pass_with(0.5)).expect("grid low");
    let high_zoom = pollster::block_on(run_grid_smoke_pass_with(64.0)).expect("grid high");

    assert!(high_zoom.minor_lod_alpha > low_zoom.minor_lod_alpha);
    assert!(high_zoom.major_lod_alpha > low_zoom.major_lod_alpha);
}

#[test]
fn text_smoke_pass_runs() {
    let text_count = pollster::block_on(run_text_smoke_pass()).expect("text pass");
    assert_eq!(text_count, 2);
}

#[test]
fn text_smoke_pass_handles_scale_rotation_and_empty_content() {
    let texts = [
        TextItem {
            content: String::new(),
            position: [0.4, 0.6],
            size_mm: 0.0,
            color: [0.9, 0.9, 0.9, 1.0],
            bold: false,
            italic: false,
            rotation: 0.0,
            h_align: TextHAlign::Left,
            v_align: TextVAlign::Top,
        },
        TextItem {
            content: "VOUT".to_string(),
            position: [1.2, 0.8],
            size_mm: 2.0,
            color: [0.2, 0.8, 0.9, 1.0],
            bold: true,
            italic: false,
            rotation: -std::f32::consts::FRAC_PI_2,
            h_align: TextHAlign::Left,
            v_align: TextVAlign::Top,
        },
        TextItem {
            content: "A".to_string(),
            position: [1.0, 1.0],
            size_mm: 0.5,
            color: [0.8, 0.4, 0.2, 1.0],
            bold: false,
            italic: true,
            rotation: std::f32::consts::TAU + 0.25,
            h_align: TextHAlign::Left,
            v_align: TextVAlign::Top,
        },
    ];

    let low_zoom_count =
        pollster::block_on(run_text_smoke_pass_with(8.0, &texts)).expect("text pass low");
    let high_zoom_count =
        pollster::block_on(run_text_smoke_pass_with(64.0, &texts)).expect("text pass high");

    assert_eq!(low_zoom_count, 3);
    assert_eq!(high_zoom_count, 3);
}

#[test]
fn text_smoke_pass_clips_fully_outside_viewport() {
    let texts = [
        TextItem {
            content: "INSIDE".to_string(),
            position: [1.0, 1.0],
            size_mm: 1.0,
            color: [1.0, 1.0, 1.0, 1.0],
            bold: false,
            italic: false,
            rotation: 0.0,
            h_align: TextHAlign::Left,
            v_align: TextVAlign::Top,
        },
        TextItem {
            content: "OUTSIDE".to_string(),
            position: [999.0, 999.0],
            size_mm: 1.0,
            color: [1.0, 0.2, 0.2, 1.0],
            bold: false,
            italic: false,
            rotation: 0.0,
            h_align: TextHAlign::Left,
            v_align: TextVAlign::Top,
        },
    ];

    let count =
        pollster::block_on(run_text_smoke_pass_with(32.0, &texts)).expect("text clipping pass");
    assert_eq!(count, 1);
}

#[test]
fn text_smoke_pass_handles_dense_overlap_cluster() {
    let texts = [
        TextItem {
            content: "NET_A".to_string(),
            position: [3.0, 3.0],
            size_mm: 1.0,
            color: [1.0, 1.0, 1.0, 1.0],
            bold: false,
            italic: false,
            rotation: 0.0,
            h_align: TextHAlign::Left,
            v_align: TextVAlign::Top,
        },
        TextItem {
            content: "NET_B".to_string(),
            position: [3.1, 3.0],
            size_mm: 1.0,
            color: [0.8, 1.0, 0.8, 1.0],
            bold: false,
            italic: false,
            rotation: 0.0,
            h_align: TextHAlign::Left,
            v_align: TextVAlign::Top,
        },
        TextItem {
            content: "NET_C".to_string(),
            position: [3.2, 3.0],
            size_mm: 1.0,
            color: [0.8, 0.8, 1.0, 1.0],
            bold: false,
            italic: false,
            rotation: 0.0,
            h_align: TextHAlign::Left,
            v_align: TextVAlign::Top,
        },
    ];

    let count =
        pollster::block_on(run_text_smoke_pass_with(32.0, &texts)).expect("text overlap pass");
    assert_eq!(count, 3);
}

#[test]
fn text_compositing_order_places_text_above_geometry() {
    let report = pollster::block_on(run_text_geometry_composite_smoke_pass())
        .expect("text geometry composite pass");

    assert_eq!(report.polygon_vertices, 6);
    assert_eq!(report.text_instances, 1);
    assert_eq!(
        report.stage_order,
        vec![CompositeStage::Geometry, CompositeStage::Text]
    );
}

#[test]
fn overlay_compositing_order_places_overlay_between_geometry_and_text() {
    let report = pollster::block_on(run_grid_overlay_text_composite_smoke_pass())
        .expect("grid overlay text composite pass");

    assert_eq!(report.geometry_vertices, 6);
    assert_eq!(report.overlay_instances, 2);
    assert_eq!(report.text_instances, 1);
    assert_eq!(
        report.stage_order,
        vec![
            CompositeStage::Grid,
            CompositeStage::Geometry,
            CompositeStage::Overlay,
            CompositeStage::Text,
        ]
    );
}

#[test]
fn grid_overlay_toggles_do_not_change_geometry_draw_work() {
    let polygons = [GpuPolygon {
        vertices: vec![[0.5, 0.5], [4.5, 0.5], [4.5, 4.5], [0.5, 4.5]],
        fill_color: [0.18, 0.22, 0.78, 1.0],
        stroke_color: None,
        stroke_width: 0.0,
    }];
    let overlays = [LineSegment {
        p0: [0.75, 0.75],
        p1: [4.25, 4.25],
        width: 0.12,
        color: [1.0, 0.83, 0.27, 1.0],
        style: 0,
        _pad: 0,
    }];
    let texts = [TextItem {
        content: "TOP".to_string(),
        position: [1.2, 1.2],
        size_mm: 0.9,
        color: [1.0, 1.0, 1.0, 1.0],
        bold: true,
        italic: false,
        rotation: 0.0,
        h_align: TextHAlign::Left,
        v_align: TextVAlign::Top,
    }];

    let baseline = pollster::block_on(run_grid_overlay_text_composite_smoke_pass_with(
        32.0, true, true, true, &polygons, &overlays, &texts,
    ))
    .expect("baseline composite pass");
    let overlay_off = pollster::block_on(run_grid_overlay_text_composite_smoke_pass_with(
        32.0, true, false, true, &polygons, &overlays, &texts,
    ))
    .expect("overlay toggle off pass");
    let grid_off = pollster::block_on(run_grid_overlay_text_composite_smoke_pass_with(
        32.0, false, true, true, &polygons, &overlays, &texts,
    ))
    .expect("grid toggle off pass");

    assert_eq!(baseline.geometry_vertices, overlay_off.geometry_vertices);
    assert_eq!(baseline.geometry_vertices, grid_off.geometry_vertices);
    assert_eq!(overlay_off.overlay_instances, 0);
    assert_eq!(
        overlay_off.stage_order,
        vec![
            CompositeStage::Grid,
            CompositeStage::Geometry,
            CompositeStage::Text,
        ]
    );
    assert_eq!(
        grid_off.stage_order,
        vec![
            CompositeStage::Geometry,
            CompositeStage::Overlay,
            CompositeStage::Text,
        ]
    );
}
