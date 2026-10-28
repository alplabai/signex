//! Tests for GPU smoke passes.
use super::composite::run_grid_overlay_text_composite_smoke_pass_with;
use super::{
    ARC_READBACK_PX, ARC_READBACK_SCALE, CompositeStage, POLYGON_SQUARE_PX, POLYGON_SQUARE_SCALE,
    run_arc_readback_smoke_pass, run_arc_smoke_pass, run_arc_smoke_pass_with,
    run_grid_overlay_text_composite_smoke_pass, run_grid_smoke_pass, run_grid_smoke_pass_with,
    run_line_circle_smoke_pass, run_line_readback_smoke_pass, run_ordered_readback_smoke_pass,
    run_polygon_readback_smoke_pass, run_polygon_smoke_pass, run_polygon_smoke_pass_with,
    run_polygon_square_readback_smoke_pass, run_text_geometry_composite_smoke_pass,
    run_text_smoke_pass, run_text_smoke_pass_with,
};
use crate::primitive::arc::Arc;
use crate::primitive::circle::Circle;
use crate::primitive::line::LineSegment;
use crate::primitive::ordered::OrderedItem;
use crate::primitive::polygon::GpuPolygon;
use crate::primitive::text::{TextHAlign, TextItem, TextVAlign};

/// Run one GPU smoke pass at a time (#889). Creating and using wgpu devices
/// on the software adapter from several test threads at once crashed this
/// test binary with SIGSEGV (3 runs in 40 here, none in 40 with one test
/// thread), and the crash aborts every test still to run.
fn on_gpu<T>(pass: impl std::future::Future<Output = T>) -> T {
    static GPU: std::sync::Mutex<()> = std::sync::Mutex::new(());
    // A failed assertion inside one pass must not poison the others.
    let _one_at_a_time = GPU
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    pollster::block_on(pass)
}

#[test]
fn line_circle_smoke_pass_runs_for_multiple_scales() {
    let low_zoom = on_gpu(run_line_circle_smoke_pass(8.0)).expect("low zoom pass");
    let high_zoom = on_gpu(run_line_circle_smoke_pass(64.0)).expect("high zoom pass");

    assert_eq!(low_zoom.line_instances, 2);
    assert_eq!(low_zoom.circle_instances, 1);
    assert_eq!(high_zoom.line_instances, 2);
    assert_eq!(high_zoom.circle_instances, 1);
}

/// Correctness — thread #5: `line.wgsl` must actually paint gaps for a
/// `STYLE_DASHED` segment, not just carry the unused attribute. Samples the
/// rendered red channel at pixel x offsets chosen to land mid-dash and
/// mid-gap under the shader's `dash_mm = 8 * mm_per_px` / `gap_mm = 5 *
/// mm_per_px` pattern (see `run_line_dash_readback_smoke_pass`). Before the
/// fix every sample reads bright (solid line); after it, gap samples read
/// dark (background shows through).
#[test]
fn line_wgsl_actually_renders_a_dashed_pattern() {
    // Period = 13px (8px dash + 5px gap); 8*13 = 104 puts a period boundary
    // well clear of the line's start/end so the sample isn't an edge effect.
    let dash_samples = [107u32, 111]; // offsets 3, 7 into the dash (< 8)
    let gap_samples = [113u32, 116]; // offsets 9, 12 into the gap (>= 8)

    let mut sample_points = dash_samples.to_vec();
    sample_points.extend_from_slice(&gap_samples);

    // Spans almost the full viewport so several dash/gap periods land inside
    // it. No stroke floor here: this test is about the dash pattern, and a
    // floor would change the very width under test.
    let dashed = LineSegment {
        p0: [0.0, 1.0],
        p1: [30.0, 1.0],
        width: 1.0,
        color: [1.0, 1.0, 1.0, 1.0],
        style: LineSegment::STYLE_DASHED,
        _pad: 0,
    };

    let reds = on_gpu(run_line_readback_smoke_pass(dashed, 0.0, &sample_points))
        .expect("dash readback pass");

    for (x, red) in dash_samples.iter().zip(&reds[..dash_samples.len()]) {
        assert!(
            *red > 200,
            "expected a bright (dash) pixel at x={x}, got red={red}"
        );
    }
    for (x, red) in gap_samples.iter().zip(&reds[dash_samples.len()..]) {
        assert!(
            *red < 50,
            "expected a dark (gap) pixel at x={x}, got red={red} — the GPU \
             line pipeline is still ignoring the dash style bit"
        );
    }
}

/// #645: the GPU had no screen-space minimum stroke width where both CPU
/// replays clamp (0.6 px schematic, 0.5 px PCB), so a hairline faded toward
/// invisible as you zoomed out. The floor now rides in the camera uniform and
/// `line.wgsl` applies it as `max(width, min_stroke_px * mm_per_px)`.
///
/// Rendered, not reasoned about: a width the pass renders at ~0.08 px is
/// nearly black without the floor and solid with it. The two runs differ only
/// in `min_stroke_px`, so anything else that changed brightness would move
/// both.
#[test]
fn a_stroke_floor_keeps_a_hairline_visible_when_the_cpu_would_clamp_it() {
    // 0.02 mm at the pass's 4.0 px/mm is 0.08 px — well under one pixel.
    let hairline = LineSegment {
        p0: [0.0, 1.0],
        p1: [30.0, 1.0],
        width: 0.02,
        color: [1.0, 1.0, 1.0, 1.0],
        style: 0,
        _pad: 0,
    };
    // Mid-span, clear of the end caps.
    let samples = [60u32, 64, 68];

    let without_floor = on_gpu(run_line_readback_smoke_pass(hairline, 0.0, &samples))
        .expect("hairline readback pass, no floor");
    for (x, red) in samples.iter().zip(&without_floor) {
        assert!(
            *red < 60,
            "a 0.08 px hairline should be nearly invisible without a floor, \
             but x={x} read red={red} — if this is bright, the test no longer \
             proves the floor is what makes the line below visible"
        );
    }

    // 2.0 px floor -> 0.5 mm at this zoom -> 2 px on screen. Deliberately far
    // above the real 0.5/0.6 px floors so the assertion is not fighting
    // antialiasing at the one-pixel boundary.
    let with_floor = on_gpu(run_line_readback_smoke_pass(hairline, 2.0, &samples))
        .expect("hairline readback pass, with floor");
    for (x, red) in samples.iter().zip(&with_floor) {
        assert!(
            *red > 200,
            "the stroke floor should have widened the hairline to 2 px, but \
             x={x} read red={red} — line.wgsl is ignoring camera.min_stroke_px"
        );
    }
}

/// The polygon half of the floor: a coloured zero-width outline used to emit
/// no geometry on the GPU while the CPU replay drew it as a hairline. Now its
/// quads are emitted at zero half-width and `polygon.wgsl` widens them to
/// `min_stroke_px`. Same two-run shape as the line test above — with no floor
/// the edge has zero area and reads black, so the floor is what lights it.
#[test]
fn a_stroke_floor_draws_a_zero_width_polygon_outline() {
    // Transparent fill, so only the outline can light a pixel. Its bottom
    // edge runs along y = 1.0 mm, the sampled row's pixel boundary.
    let outline = GpuPolygon {
        holes: Vec::new(),
        vertices: vec![[0.0, 1.0], [30.0, 1.0], [30.0, 10.0], [0.0, 10.0]],
        fill_color: [0.0, 0.0, 0.0, 0.0],
        stroke_color: Some([1.0, 1.0, 1.0, 1.0]),
        stroke_width: 0.0,
    };
    // Mid-edge, clear of the vertical edges at x = 0 and x = 120 px.
    let samples = [60u32, 64, 68];

    let without_floor = on_gpu(run_polygon_readback_smoke_pass(
        outline.clone(),
        0.0,
        &samples,
    ))
    .expect("outline readback pass, no floor");
    assert!(
        without_floor.iter().all(|red| *red < 60),
        "a zero-width outline with no floor has no area, read {without_floor:?}"
    );

    let with_floor = on_gpu(run_polygon_readback_smoke_pass(outline, 2.0, &samples))
        .expect("outline readback pass, with floor");
    for (x, red) in samples.iter().zip(&with_floor) {
        assert!(
            *red > 200,
            "the stroke floor should have drawn the zero-width outline 2 px \
             wide, but x={x} read red={red} — polygon.wgsl is ignoring \
             camera.min_stroke_px"
        );
    }
}

#[test]
fn arc_smoke_pass_runs() {
    let count = on_gpu(run_arc_smoke_pass()).expect("arc pass");
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

    let count = on_gpu(run_arc_smoke_pass_with(8.0, &arcs)).expect("wraparound arc pass");
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

    let count = on_gpu(run_arc_smoke_pass_with(64.0, &arcs)).expect("tiny radius arc pass");
    assert_eq!(count, 1);
}

#[test]
fn polygon_smoke_pass_runs() {
    let vertex_count = on_gpu(run_polygon_smoke_pass()).expect("polygon pass");
    assert_eq!(vertex_count, 6);
}

#[test]
fn polygon_smoke_pass_handles_low_and_high_zoom() {
    let polygons = [GpuPolygon {
        holes: Vec::new(),
        vertices: vec![[2.0, 2.0], [8.0, 2.0], [8.0, 8.0], [2.0, 8.0]],
        fill_color: [0.2, 0.7, 0.9, 1.0],
        stroke_color: None,
        stroke_width: 0.0,
    }];

    let low_zoom_count = on_gpu(run_polygon_smoke_pass_with(8.0, &polygons)).expect("polygon low");
    let high_zoom_count =
        on_gpu(run_polygon_smoke_pass_with(64.0, &polygons)).expect("polygon high");

    assert_eq!(low_zoom_count, 6);
    assert_eq!(high_zoom_count, 6);
}

#[test]
fn polygon_smoke_pass_ignores_degenerate_geometry() {
    let polygons = [
        GpuPolygon {
            holes: Vec::new(),
            vertices: vec![[0.0, 0.0], [1.0, 0.0]],
            fill_color: [1.0, 0.0, 0.0, 1.0],
            stroke_color: None,
            stroke_width: 0.0,
        },
        GpuPolygon {
            holes: Vec::new(),
            vertices: vec![[2.0, 2.0], [8.0, 2.0], [8.0, 8.0], [2.0, 8.0]],
            fill_color: [0.2, 0.7, 0.9, 1.0],
            stroke_color: None,
            stroke_width: 0.0,
        },
    ];

    let count =
        on_gpu(run_polygon_smoke_pass_with(32.0, &polygons)).expect("polygon degenerate filter");
    assert_eq!(count, 6);
}

#[test]
fn grid_smoke_pass_runs() {
    let report = on_gpu(run_grid_smoke_pass()).expect("grid pass");
    assert!((0.0..=1.0).contains(&report.minor_lod_alpha));
    assert!((0.0..=1.0).contains(&report.major_lod_alpha));
}

#[test]
fn grid_smoke_pass_lod_changes_with_zoom() {
    let low_zoom = on_gpu(run_grid_smoke_pass_with(0.5)).expect("grid low");
    let high_zoom = on_gpu(run_grid_smoke_pass_with(64.0)).expect("grid high");

    assert!(high_zoom.minor_lod_alpha > low_zoom.minor_lod_alpha);
    assert!(high_zoom.major_lod_alpha > low_zoom.major_lod_alpha);
}

#[test]
fn text_smoke_pass_runs() {
    let text_count = on_gpu(run_text_smoke_pass()).expect("text pass");
    // Two items, one rotated 0.2 rad: only the unrotated one reaches the
    // atlas. A cryoglyph `TextArea` cannot rotate, so staging the other drew
    // it axis-aligned; it is the CPU layer's (`TextItem::is_axis_aligned`).
    assert_eq!(text_count, 1);
}

/// Scale and empty content still stage; rotation never does — whatever the
/// angle's sign or how many turns it wraps.
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
            family: None,
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
            family: None,
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
            family: None,
        },
    ];

    let low_zoom_count = on_gpu(run_text_smoke_pass_with(8.0, &texts)).expect("text pass low");
    let high_zoom_count = on_gpu(run_text_smoke_pass_with(64.0, &texts)).expect("text pass high");

    assert_eq!(low_zoom_count, 1);
    assert_eq!(high_zoom_count, 1);
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
            family: None,
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
            family: None,
        },
    ];

    let count = on_gpu(run_text_smoke_pass_with(32.0, &texts)).expect("text clipping pass");
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
            family: None,
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
            family: None,
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
            family: None,
        },
    ];

    let count = on_gpu(run_text_smoke_pass_with(32.0, &texts)).expect("text overlap pass");
    assert_eq!(count, 3);
}

#[test]
fn text_compositing_order_places_text_above_geometry() {
    let report =
        on_gpu(run_text_geometry_composite_smoke_pass()).expect("text geometry composite pass");

    assert_eq!(report.polygon_vertices, 6);
    assert_eq!(report.text_instances, 1);
    assert_eq!(
        report.stage_order,
        vec![CompositeStage::Geometry, CompositeStage::Text]
    );
}

#[test]
fn overlay_compositing_order_places_overlay_between_geometry_and_text() {
    let report = on_gpu(run_grid_overlay_text_composite_smoke_pass())
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
        holes: Vec::new(),
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
        family: None,
    }];

    let baseline = on_gpu(run_grid_overlay_text_composite_smoke_pass_with(
        32.0, true, true, true, &polygons, &overlays, &texts,
    ))
    .expect("baseline composite pass");
    let overlay_off = on_gpu(run_grid_overlay_text_composite_smoke_pass_with(
        32.0, true, false, true, &polygons, &overlays, &texts,
    ))
    .expect("overlay toggle off pass");
    let grid_off = on_gpu(run_grid_overlay_text_composite_smoke_pass_with(
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

// Pixel proofs for `arc.wgsl`. The arc is centred at world (8, 8) mm with
// radius 5 mm on a 64 px square at 4 px/mm, so the ring sits 20 px from pixel
// (32, 32). Angles are world radians from +X, increasing and wrapping; world
// +Y runs down the rows, so +90 degrees lands BELOW the centre and an
// increasing angle turns clockwise on screen.
const ARC_CENTER_MM: f32 = 8.0;
const ARC_RADIUS_MM: f32 = 5.0;

fn ring_arc(start_deg: f32, end_deg: f32) -> Arc {
    Arc {
        center: [ARC_CENTER_MM, ARC_CENTER_MM],
        radius: ARC_RADIUS_MM,
        start_angle: start_deg.to_radians(),
        end_angle: end_deg.to_radians(),
        width: 1.0,
        color: [1.0, 1.0, 1.0, 1.0],
        _pad: [0.0; 3],
    }
}

/// Pixel on the ring at `deg` (world angle, increasing from +X with Y down), the CPU convention.
fn ring_px(deg: f32) -> (u32, u32) {
    let (sin, cos) = deg.to_radians().sin_cos();
    let to_px = |mm: f32| (mm * ARC_READBACK_SCALE).round() as u32;
    (
        to_px(ARC_CENTER_MM + ARC_RADIUS_MM * cos),
        to_px(ARC_CENTER_MM + ARC_RADIUS_MM * sin),
    )
}

/// Reads the red channel at each ring angle, in order.
fn arc_reds(arc: Arc, angles_deg: &[f32]) -> Vec<u8> {
    let samples: Vec<(u32, u32)> = angles_deg.iter().map(|&a| ring_px(a)).collect();
    on_gpu(run_arc_readback_smoke_pass(arc, &samples)).expect("arc readback pass")
}

fn assert_lit(arc: Arc, angles_deg: &[f32]) {
    for (deg, red) in angles_deg.iter().zip(arc_reds(arc, angles_deg)) {
        assert!(red > 200, "ring at {deg} deg should be lit, red={red}");
    }
}

fn assert_dark(arc: Arc, angles_deg: &[f32]) {
    for (deg, red) in angles_deg.iter().zip(arc_reds(arc, angles_deg)) {
        assert!(red < 60, "ring at {deg} deg should be dark, red={red}");
    }
}

/// A 0..90 degree arc is lit through its sweep and dark everywhere a mirrored
/// (-90..0) or wrong-way (the 270 degree complement) shader would light.
#[test]
fn a_quarter_arc_is_lit_in_its_sweep_and_dark_outside_it() {
    let arc = ring_arc(0.0, 90.0);
    assert_lit(arc, &[20.0, 45.0, 70.0]);
    // Mirror image across the X axis, then the complement's far side.
    assert_dark(arc, &[-20.0, -45.0, -70.0, 135.0, 180.0, 225.0, 270.0]);
}

/// `330 -> 30` sweeps 60 degrees of increasing angle through the 0/360 seam, not the 300
/// degree complement.
#[test]
fn a_wrapped_arc_lights_the_side_through_the_seam() {
    let arc = ring_arc(330.0, 30.0);
    assert_lit(arc, &[340.0, 0.0, 20.0]);
    assert_dark(arc, &[90.0, 180.0, 270.0, 60.0, 300.0]);
}

/// A whole turn is a circle, not the zero-length sweep it wraps to.
#[test]
fn a_full_turn_arc_lights_all_four_quadrants() {
    let arc = ring_arc(0.0, 360.0);
    assert_lit(arc, &[45.0, 135.0, 225.0, 315.0]);
    // The ring only: the centre stays dark.
    let centre = (ARC_READBACK_PX / 2, ARC_READBACK_PX / 2);
    let reds = on_gpu(run_arc_readback_smoke_pass(arc, &[centre])).expect("centre");
    assert!(
        reds[0] < 60,
        "circle interior should be dark, red={}",
        reds[0]
    );
}

/// A regular `sides`-gon around the middle of the square readback target.
fn ring_polygon(radius_mm: f32, sides: usize, width_mm: f32, stroke: [f32; 4]) -> GpuPolygon {
    let c = POLYGON_SQUARE_PX as f32 / POLYGON_SQUARE_SCALE / 2.0;
    GpuPolygon {
        holes: Vec::new(),
        vertices: (0..sides)
            .map(|i| {
                let t = std::f32::consts::TAU * i as f32 / sides as f32;
                [c + radius_mm * t.cos(), c + radius_mm * t.sin()]
            })
            .collect(),
        fill_color: [0.0; 4],
        stroke_color: Some(stroke),
        stroke_width: width_mm,
    }
}

/// Pixels well inside the stroke band of [`ring_polygon`] (radius `r` px, half
/// the band `h` px), all around it, clear of the band's edges.
fn band_samples(r_px: f32, h_px: f32) -> Vec<(u32, u32)> {
    let c = POLYGON_SQUARE_PX as f32 / 2.0;
    let mut out = Vec::new();
    for step in 0..72 {
        let t = std::f32::consts::TAU * (step as f32 + 0.37) / 72.0;
        for frac in [-0.6, 0.0, 0.6] {
            let rr = r_px + frac * h_px;
            out.push(((c + rr * t.cos()) as u32, (c + rr * t.sin()) as u32));
        }
    }
    out
}

/// A stroke thicker than its edges are long used to leave gaps between the edge
/// rectangles, so a tiny circle's outline vanished (#199). Mitered corners close
/// them, and since neighbouring edges then share their corner exactly, a
/// translucent stroke is blended once everywhere, not twice at the joints.
///
/// 48 sides at radius 4 mm (16 px) have 2.1 px edges under a 3.2 px half-width
/// stroke: well into the mitered regime, and wide enough that the band's inner
/// offset does not cross itself.
#[test]
fn a_stroke_thicker_than_its_edges_has_no_gap_and_no_double_blend() {
    let samples = band_samples(16.0, 3.2);

    let opaque = ring_polygon(4.0, 48, 1.6, [1.0, 0.0, 0.0, 1.0]);
    let reds = on_gpu(run_polygon_square_readback_smoke_pass(opaque, &samples))
        .expect("opaque ring readback");
    let gaps = reds.iter().filter(|red| **red < 240).count();
    assert_eq!(
        gaps,
        0,
        "{gaps} of {} band pixels are not covered",
        reds.len()
    );

    // 50% red over black: one layer reads 127; a second blend would read 191.
    let translucent = ring_polygon(4.0, 48, 1.6, [1.0, 0.0, 0.0, 0.5]);
    let reds = on_gpu(run_polygon_square_readback_smoke_pass(
        translucent,
        &samples,
    ))
    .expect("translucent ring readback");
    for (sample, red) in samples.iter().zip(&reds) {
        assert!(
            (110..=145).contains(red),
            "band pixel {sample:?} reads {red}: below 110 is a gap, above 145 a double blend"
        );
    }
}

/// Half the stroke of the join-band tests below, in pixels, and the scale they
/// draw at (`POLYGON_SQUARE_SCALE` px per mm).
const JOIN_HALF_PX: f32 = 3.2;

/// A rounded square around the middle of the target: long straight edges
/// joined by arcs whose chords are about two pixels, so a corner pairs a long
/// edge with a short one.
fn rounded_square(half_mm: f32, radius_mm: f32) -> Vec<[f32; 2]> {
    let c = POLYGON_SQUARE_PX as f32 / POLYGON_SQUARE_SCALE / 2.0;
    let inner = half_mm - radius_mm;
    let steps = 12;
    let mut ring = Vec::new();
    for (quarter, (sx, sy)) in [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)]
        .into_iter()
        .enumerate()
    {
        for step in 0..=steps {
            let t = (quarter as f32 + step as f32 / steps as f32) * std::f32::consts::FRAC_PI_2;
            // Quarter `q` sweeps the angles q*90 .. (q+1)*90 around its corner.
            ring.push([
                c + sx * inner + radius_mm * t.cos(),
                c + sy * inner + radius_mm * t.sin(),
            ]);
        }
    }
    ring
}

/// Pixels on the stroke band at every vertex that touches an edge shorter than
/// twice the half-width (the mitered regime): on the corner's own bisector, on
/// the centreline and half a half-width to either side.
fn join_samples(ring: &[[f32; 2]]) -> Vec<(u32, u32)> {
    let scale = POLYGON_SQUARE_SCALE;
    let normal = |a: [f32; 2], b: [f32; 2]| {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = (dx * dx + dy * dy).sqrt();
        ([-dy / len, dx / len], len * scale)
    };
    let mut out = Vec::new();
    for i in 0..ring.len() {
        let (prev, p, next) = (
            ring[(i + ring.len() - 1) % ring.len()],
            ring[i],
            ring[(i + 1) % ring.len()],
        );
        let ((n1, l1), (n2, l2)) = (normal(prev, p), normal(p, next));
        if l1.min(l2) >= 2.0 * JOIN_HALF_PX {
            continue;
        }
        let (bx, by) = (n1[0] + n2[0], n1[1] + n2[1]);
        let blen = (bx * bx + by * by).sqrt();
        for frac in [-0.5, 0.0, 0.5] {
            let d = frac * JOIN_HALF_PX / blen;
            out.push((
                (p[0] * scale + bx * d) as u32,
                (p[1] * scale + by * d) as u32,
            ));
        }
    }
    out
}

/// Every join-band pixel of `polygon` reads one 50% layer: no gap, no double
/// blend, whichever of its edges are long and whichever short.
fn assert_one_layer_at_joins(polygon: GpuPolygon, samples: &[(u32, u32)]) {
    assert!(samples.len() >= 6, "the shape has joins to sample");
    let reds = on_gpu(run_polygon_square_readback_smoke_pass(polygon, samples))
        .expect("join band readback");
    let bad: Vec<_> = samples
        .iter()
        .zip(&reds)
        .filter(|(_, red)| !(110..=145).contains(*red))
        .collect();
    assert!(
        bad.is_empty(),
        "{} of {} join pixels are not one 50% layer (below 110 a gap, above 145 a double blend): {bad:?}",
        bad.len(),
        samples.len()
    );
}

fn half_red_stroke(ring: Vec<[f32; 2]>, holes: Vec<Vec<[f32; 2]>>) -> GpuPolygon {
    GpuPolygon {
        vertices: ring,
        holes,
        fill_color: [0.0; 4],
        stroke_color: Some([1.0, 0.0, 0.0, 0.5]),
        stroke_width: 2.0 * JOIN_HALF_PX / POLYGON_SQUARE_SCALE,
    }
}

/// A square with every corner cut by a short diagonal, so each cut joins two
/// long edges through an edge no longer than the half-width: fully mitered
/// (a part-way miter overlaps on the inside by design).
fn chamfered_square(half_mm: f32, cut_mm: f32) -> Vec<[f32; 2]> {
    let c = POLYGON_SQUARE_PX as f32 / POLYGON_SQUARE_SCALE / 2.0;
    let (lo, hi) = (c - half_mm, c + half_mm);
    vec![
        [lo + cut_mm, lo],
        [hi - cut_mm, lo],
        [hi, lo + cut_mm],
        [hi, hi - cut_mm],
        [hi - cut_mm, hi],
        [lo + cut_mm, hi],
        [lo, hi - cut_mm],
        [lo, lo + cut_mm],
    ]
}

/// A corner that joins a short edge to a long one: the long edge's quad used to
/// keep its plain corner while the short edge's moved to the miter point, so
/// the two missed each other (a notch outside, a double blend inside).
#[test]
fn a_long_edge_meeting_a_short_one_joins_the_band_exactly() {
    let ring = chamfered_square(6.0, 0.5);
    let samples = join_samples(&ring);
    assert_one_layer_at_joins(half_red_stroke(ring, Vec::new()), &samples);
}

/// Long edges joined by arcs of two-pixel chords: the turns are gentle, but
/// the join must stay exact all the way round.
#[test]
fn a_rounded_square_joins_the_band_exactly() {
    let ring = rounded_square(6.0, 4.0);
    let samples = join_samples(&ring);
    assert_one_layer_at_joins(half_red_stroke(ring, Vec::new()), &samples);
}

/// An L whose reflex corner is cut by a short diagonal: the chamfer's two
/// corners join it to long edges at a reflex and a convex turn.
#[test]
fn an_l_outline_with_a_short_reflex_cut_joins_the_band_exactly() {
    let chamfer = 0.5;
    let ring = vec![
        [2.0, 2.0],
        [14.0, 2.0],
        [14.0, 7.0],
        [8.0 + chamfer, 7.0],
        [8.0, 7.0 + chamfer],
        [8.0, 14.0],
        [2.0, 14.0],
    ];
    let samples = join_samples(&ring);
    assert_one_layer_at_joins(half_red_stroke(ring, Vec::new()), &samples);
}

/// A hole's outline is a stroke ring of its own, with the same joins.
#[test]
fn a_polygon_with_a_hole_joins_the_hole_band_exactly() {
    let hole = chamfered_square(3.0, 0.5);
    let samples = join_samples(&hole);
    let outer = vec![[1.5, 1.5], [14.5, 1.5], [14.5, 14.5], [1.5, 14.5]];
    assert_one_layer_at_joins(half_red_stroke(outer, vec![hole]), &samples);
}

/// World mm at the middle of the 64 px, 4 px/mm readback target, and its pixel.
const ORDERED_CENTRE_MM: f32 = POLYGON_SQUARE_PX as f32 / POLYGON_SQUARE_SCALE / 2.0;
const ORDERED_CENTRE_PX: (u32, u32) = (POLYGON_SQUARE_PX / 2, POLYGON_SQUARE_PX / 2);

fn centre_circle(radius_mm: f32, stroke_width: f32, rgba: [f32; 4]) -> OrderedItem {
    OrderedItem::Circle(Circle {
        center: [ORDERED_CENTRE_MM; 2],
        radius: radius_mm,
        stroke_width,
        color: rgba,
    })
}

/// A rectangle covering the whole target, filled `rgba`.
fn full_rect(rgba: [f32; 4]) -> OrderedItem {
    let edge = ORDERED_CENTRE_MM * 2.0;
    OrderedItem::Polygon(GpuPolygon {
        vertices: vec![[0.0, 0.0], [edge, 0.0], [edge, edge], [0.0, edge]],
        fill_color: rgba,
        ..GpuPolygon::default()
    })
}

/// The red channel of one pixel after `source` is blended over a backdrop of
/// red `backdrop` (alpha blending: `src * a + dst * (1 - a)`).
fn blended_red(source: f32, alpha: f32, backdrop: f32) -> f32 {
    source * alpha + backdrop * (1.0 - alpha)
}

fn centre_red(items: Vec<OrderedItem>) -> u8 {
    on_gpu(run_ordered_readback_smoke_pass(items, &[ORDERED_CENTRE_PX])).expect("ordered readback")
        [0]
}

fn assert_red_near(got: u8, expected: f32) {
    let expected = (expected * 255.0).round();
    assert!(
        (f32::from(got) - expected).abs() <= 3.0,
        "centre reads red {got}, expected about {expected}"
    );
}

/// The reason the bucket exists (#1579): kinds interleave in LIST order, not
/// in pipeline order. A translucent disc over an opaque rectangle blends with
/// it; the reversed list leaves the rectangle on top and the disc invisible
/// at the centre. The two orders must differ, and each must match the blend a
/// painter would compute.
#[test]
fn ordered_drawings_blend_in_list_order_across_kinds() {
    let rect = [0.8, 0.0, 0.0, 1.0];
    let disc = [0.2, 0.0, 0.0, 0.5];

    let disc_over_rect = centre_red(vec![full_rect(rect), centre_circle(3.0, 0.0, disc)]);
    assert_red_near(disc_over_rect, blended_red(0.2, 0.5, 0.8));

    let rect_over_disc = centre_red(vec![centre_circle(3.0, 0.0, disc), full_rect(rect)]);
    assert_red_near(rect_over_disc, 0.8);

    assert_ne!(disc_over_rect, rect_over_disc);
}

/// A drawing circle is a fill disc then a ring, as two instances. With a ring
/// wider than the circle's diameter the centre lies inside the stroke, so it
/// must read the stroke colour, not the fill (the CPU 40-gon crossover bug
/// this replaces).
#[test]
fn a_ring_wider_than_its_diameter_covers_the_fill_at_the_centre() {
    let fill = [0.2, 0.0, 0.0, 1.0];
    let stroke = [1.0, 0.0, 0.0, 1.0];
    let radius = 0.5;
    let wider_than_diameter = 4.0 * radius;

    let red = centre_red(vec![
        centre_circle(radius, 0.0, fill),
        centre_circle(radius, wider_than_diameter, stroke),
    ]);

    assert_red_near(red, 1.0);
}

/// A run that starts past the first instance (`first_instance != 0`) must
/// draw ITS instances: the circle after the line is instance 1 of the circle
/// buffer, and it paints over instance 0 at the centre.
#[test]
fn a_later_run_draws_its_own_instances_not_the_first() {
    let far_line = OrderedItem::Line(LineSegment {
        p0: [1.0, 1.0],
        p1: [2.0, 1.0],
        width: 0.2,
        color: [1.0; 4],
        style: 0,
        _pad: 0,
    });

    let red = centre_red(vec![
        centre_circle(3.0, 0.0, [0.4, 0.0, 0.0, 1.0]),
        far_line,
        centre_circle(3.0, 0.0, [0.9, 0.0, 0.0, 1.0]),
    ]);

    assert_red_near(red, 0.9);
}
